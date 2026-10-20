//! Windows Integration: windows, apps, virtual desktops, volume, brightness,
//! Wi-Fi, Bluetooth, power plans, screenshots, clipboard, notifications and
//! (only with its own permission) UI Automation.
//!
//! Every command: module on → `safety::authorize` (permission, Safe Mode,
//! confirmation) → action → `safety::finish` (Action Log + undo data).
//! Implementations: Win32 (`win`), Core Audio (`audio`), PowerShell and
//! Windows tools (`ps`), `tauri-plugin-opener` for launching.

pub mod audio;
pub mod ps;
pub mod win;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::error::{AppError, AppResult};
use crate::modules::catalog::WINDOWS_INTEGRATION;
use crate::safety::{self, ActionDef};
use crate::state::AppState;
use win::{ShowState, WindowInfo};

fn require(state: &AppState) -> AppResult<()> {
    crate::productivity::require(state, WINDOWS_INTEGRATION)
}

/// Authorizes, runs and logs one action.
async fn guarded<T, F, Fut>(app: &AppHandle, action: &str, target: &str, confirm: bool, run: F, message: impl FnOnce(&T) -> String, undo: impl FnOnce(&T) -> Option<Value>) -> AppResult<T>
where
    F: FnOnce(&'static ActionDef) -> Fut,
    Fut: std::future::Future<Output = AppResult<T>>,
{
    let state = app.state::<AppState>();
    require(&state)?;
    let def = safety::authorize(&state.db, action, target, confirm).await?;
    let result = run(def).await;
    safety::finish(app, def, target, &result, message, undo).await;
    result
}

fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> impl std::future::Future<Output = AppResult<T>> {
    async move { tauri::async_runtime::spawn_blocking(f).await.map_err(|e| AppError::Other(e.into())) }
}

fn window_label(hwnd: i64) -> String {
    win::windows().into_iter().find(|w| w.hwnd == hwnd).map(|w| format!("{} ({})", w.title, w.process)).unwrap_or_else(|| format!("окно {hwnd}"))
}

// --- windows ---

#[tauri::command]
pub async fn win_windows(app: AppHandle) -> AppResult<Vec<WindowInfo>> {
    guarded(&app, "windows.list", "", false, |_| blocking(win::windows), |l| format!("окон: {}", l.len()), |_| None).await
}

#[tauri::command]
pub async fn win_active_window(app: AppHandle) -> AppResult<Option<WindowInfo>> {
    guarded(&app, "windows.list", "активное окно", false, |_| blocking(win::active), |w| w.as_ref().map(|w| w.title.clone()).unwrap_or_default(), |_| None).await
}

/// `action`: focus | minimize | maximize | restore | close.
#[tauri::command]
pub async fn win_window(app: AppHandle, hwnd: i64, action: String, confirm: Option<bool>) -> AppResult<()> {
    let id = match action.as_str() {
        "focus" | "minimize" | "maximize" | "restore" | "close" => format!("windows.{action}"),
        other => return Err(AppError::Invalid(format!("неизвестное действие с окном {other}"))),
    };
    if !win::exists(hwnd) {
        return Err(AppError::NotFound("окно (оно уже закрыто)".into()));
    }
    let label = window_label(hwnd);
    let before = win::state(hwnd);
    guarded(
        &app,
        &id,
        &label,
        confirm.unwrap_or(false),
        |_| async move {
            let ok = match action.as_str() {
                "focus" => win::focus(hwnd),
                "minimize" => {
                    win::show(hwnd, ShowState::Minimized);
                    true
                }
                "maximize" => {
                    win::show(hwnd, ShowState::Maximized);
                    true
                }
                "restore" => {
                    win::show(hwnd, ShowState::Normal);
                    true
                }
                _ => win::close(hwnd),
            };
            if ok { Ok(()) } else { Err(AppError::Other(anyhow::anyhow!("Windows не выполнил действие (окно другой программы могло запретить переключение)"))) }
        },
        |_| "выполнено".into(),
        |_| Some(json!({ "hwnd": hwnd, "state": before })),
    )
    .await
}

// --- apps ---

static APPS: Mutex<Option<(Instant, Vec<ps::InstalledApp>)>> = Mutex::new(None);

#[tauri::command]
pub async fn win_apps(app: AppHandle, query: Option<String>, refresh: Option<bool>) -> AppResult<Vec<ps::InstalledApp>> {
    let q = query.unwrap_or_default().trim().to_lowercase();
    let label = q.clone();
    guarded(
        &app,
        "apps.search",
        &label,
        false,
        |_| async move {
            let cached = APPS.lock().unwrap().clone().filter(|(at, _)| at.elapsed() < Duration::from_secs(300) && refresh != Some(true));
            let all = match cached {
                Some((_, list)) => list,
                None => {
                    let list = ps::start_apps().await?;
                    *APPS.lock().unwrap() = Some((Instant::now(), list.clone()));
                    list
                }
            };
            Ok(all.into_iter().filter(|a| q.is_empty() || a.name.to_lowercase().contains(&q)).take(200).collect::<Vec<_>>())
        },
        |l| format!("найдено: {}", l.len()),
        |_| None,
    )
    .await
}

/// Launches an installed app by its AppUserModelID (from `win_apps`).
#[tauri::command]
pub async fn win_launch(app: AppHandle, app_id: String, confirm: Option<bool>) -> AppResult<()> {
    let known = APPS.lock().unwrap().as_ref().and_then(|(_, l)| l.iter().find(|a| a.app_id == app_id).map(|a| a.name.clone()));
    let name = known.ok_or_else(|| AppError::Invalid("приложение не найдено в списке установленных — обновите поиск".into()))?;
    let a2 = app.clone();
    guarded(
        &app,
        "apps.launch",
        &name,
        confirm.unwrap_or(false),
        |_| async move {
            // Works for classic and Store apps alike.
            a2.opener().open_path(format!("shell:AppsFolder\\{app_id}"), None::<&str>).map_err(|e| AppError::Other(e.into()))
        },
        |_| "запущено".into(),
        |_| None,
    )
    .await
}

// --- virtual desktops ---

#[tauri::command]
pub async fn win_desktops(app: AppHandle) -> AppResult<ps::Desktops> {
    guarded(&app, "desktops.read", "", false, |_| ps::desktops(), |d| format!("столов: {}", d.count), |_| None).await
}

/// `action`: next | previous | new | close.
#[tauri::command]
pub async fn win_desktop(app: AppHandle, action: String, confirm: Option<bool>) -> AppResult<()> {
    let (id, key) = match action.as_str() {
        "next" => ("desktops.switch", win::VK_RIGHT),
        "previous" => ("desktops.switch", win::VK_LEFT),
        "new" => ("desktops.new", win::VK_D),
        "close" => ("desktops.close", win::VK_F4),
        other => return Err(AppError::Invalid(format!("неизвестное действие {other}"))),
    };
    guarded(
        &app,
        id,
        &action,
        confirm.unwrap_or(false),
        |_| blocking(move || win::win_ctrl(key)),
        |_| "сочетание нажато".into(),
        |_| match action.as_str() {
            "next" => Some(json!({ "direction": "previous" })),
            "previous" => Some(json!({ "direction": "next" })),
            _ => None,
        },
    )
    .await
}

// --- volume, brightness, power ---

#[tauri::command]
pub async fn win_volume(app: AppHandle) -> AppResult<audio::Volume> {
    guarded(&app, "volume.read", "", false, |_| async { blocking(audio::get).await?.map_err(|e| AppError::Other(anyhow::anyhow!(e))) }, |v| format!("{:.0}%", v.level * 100.0), |_| None).await
}

/// `level` 0–100; `muted` on/off. Either may be omitted.
#[tauri::command]
pub async fn win_set_volume(app: AppHandle, level: Option<u8>, muted: Option<bool>, confirm: Option<bool>) -> AppResult<audio::Volume> {
    let before = blocking(audio::get).await?.ok();
    let target = match (level, muted) {
        (Some(l), _) => format!("{}%", l.min(100)),
        (None, Some(m)) => if m { "без звука" } else { "со звуком" }.into(),
        _ => return Err(AppError::Invalid("укажите громкость или «без звука»".into())),
    };
    guarded(
        &app,
        "volume.set",
        &target,
        confirm.unwrap_or(false),
        |_| async move { blocking(move || audio::set(level.map(|l| l.min(100) as f32 / 100.0), muted)).await?.map_err(|e| AppError::Other(anyhow::anyhow!(e))) },
        |v| format!("громкость {:.0}%{}", v.level * 100.0, if v.muted { ", без звука" } else { "" }),
        |_| before.map(|b| json!({ "level": b.level, "muted": b.muted })),
    )
    .await
}

#[tauri::command]
pub async fn win_brightness(app: AppHandle) -> AppResult<Option<u8>> {
    guarded(&app, "brightness.read", "", false, |_| ps::brightness(), |b| b.map(|b| format!("{b}%")).unwrap_or_else(|| "недоступна".into()), |_| None).await
}

#[tauri::command]
pub async fn win_set_brightness(app: AppHandle, level: u8, confirm: Option<bool>) -> AppResult<()> {
    let before = ps::brightness().await.ok().flatten();
    let level = level.min(100);
    guarded(&app, "brightness.set", &format!("{level}%"), confirm.unwrap_or(false), |_| ps::set_brightness(level), |_| format!("яркость {level}%"), |_| before.map(|b| json!({ "level": b }))).await
}

#[tauri::command]
pub async fn win_power_plans(app: AppHandle) -> AppResult<Vec<ps::PowerPlan>> {
    guarded(&app, "power.read", "", false, |_| ps::power_plans(), |p| format!("схем: {}", p.len()), |_| None).await
}

#[tauri::command]
pub async fn win_set_power_plan(app: AppHandle, guid: String, confirm: Option<bool>) -> AppResult<Vec<ps::PowerPlan>> {
    let plans = ps::power_plans().await?;
    let target = plans.iter().find(|p| p.guid == guid.to_lowercase()).ok_or_else(|| AppError::NotFound("схема электропитания".into()))?;
    let previous = plans.iter().find(|p| p.active).map(|p| p.guid.clone());
    let g = target.guid.clone();
    guarded(&app, "power.set", &target.name, confirm.unwrap_or(false), |_| ps::set_power_plan(&g), |_| "схема включена".into(), |_| previous.map(|p| json!({ "guid": p }))).await?;
    ps::power_plans().await
}

// --- status ---

#[tauri::command]
pub async fn win_wifi(app: AppHandle) -> AppResult<ps::WifiStatus> {
    guarded(&app, "wifi.read", "", false, |_| ps::wifi(), |w| if w.connected { w.ssid.clone().unwrap_or_default() } else { "не подключено".into() }, |_| None).await
}

#[tauri::command]
pub async fn win_bluetooth(app: AppHandle) -> AppResult<ps::BluetoothStatus> {
    guarded(&app, "bluetooth.read", "", false, |_| ps::bluetooth(), |b| if b.enabled { "включён".into() } else { "выключен или недоступен".into() }, |_| None).await
}

// --- screenshot, clipboard, notifications, UI Automation ---

#[derive(Debug, Serialize)]
pub struct Screenshot {
    pub path: String,
    pub copied: bool,
}

#[tauri::command]
pub async fn win_screenshot(app: AppHandle, copy: Option<bool>) -> AppResult<Screenshot> {
    let dir = app.path().picture_dir().map_err(|e| AppError::Other(e.into()))?.join("FocusDeck");
    guarded(
        &app,
        "screen.capture",
        "весь экран",
        false,
        |_| async move {
            let bmp = blocking(crate::automation::system::screenshot_bmp).await?.map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;
            std::fs::create_dir_all(&dir).map_err(anyhow::Error::from)?;
            let path = dir.join(format!("screenshot-{}.bmp", chrono::Local::now().format("%Y%m%d-%H%M%S")));
            std::fs::write(&path, &bmp).map_err(anyhow::Error::from)?;
            let copied = copy == Some(true) && {
                let (w, h) = (i32::from_le_bytes(bmp[18..22].try_into().unwrap()) as i64, i32::from_le_bytes(bmp[22..26].try_into().unwrap()) as i64);
                crate::clipboard_hub::write(crate::clipboard_hub::win::Clip { image: Some((bmp, w, h)), ..Default::default() }).await.is_ok()
            };
            Ok(Screenshot { path: path.display().to_string(), copied })
        },
        |s| s.path.clone(),
        |_| None,
    )
    .await
}

#[tauri::command]
pub async fn win_clipboard_read(app: AppHandle) -> AppResult<Option<String>> {
    guarded(
        &app,
        "clipboard.read",
        "",
        false,
        |_| async {
            let opts = crate::clipboard_hub::win::ReadOptions { images: false, max_image_bytes: 0, rich: false, skip_hidden: true };
            Ok(blocking(move || crate::clipboard_hub::win::read(opts)).await?.and_then(|c| c.text))
        },
        |t| t.as_ref().map(|t| format!("{} символов", t.chars().count())).unwrap_or_else(|| "пусто".into()),
        |_| None,
    )
    .await
}

#[tauri::command]
pub async fn win_clipboard_write(app: AppHandle, text: String, confirm: Option<bool>) -> AppResult<()> {
    if text.chars().count() > 100_000 {
        return Err(AppError::Invalid("текст — не больше 100 000 символов".into()));
    }
    // The previous text (if any) is kept for undo.
    let opts = crate::clipboard_hub::win::ReadOptions { images: false, max_image_bytes: 0, rich: false, skip_hidden: true };
    let previous = blocking(move || crate::clipboard_hub::win::read(opts)).await?.and_then(|c| c.text);
    let preview: String = text.chars().take(40).collect();
    guarded(&app, "clipboard.write", &preview, confirm.unwrap_or(false), |_| crate::clipboard_hub::copy_text(&text), |_| "записано".into(), |_| previous.map(|p| json!({ "text": p }))).await
}

/// `native`: Windows toast; otherwise a FocusDeck notification.
#[tauri::command]
pub async fn win_notify(app: AppHandle, title: String, body: Option<String>, native: Option<bool>) -> AppResult<()> {
    let n = title.trim().chars().count();
    if n == 0 || n > 80 {
        return Err(AppError::Invalid("заголовок — от 1 до 80 символов".into()));
    }
    let body = body.unwrap_or_default();
    let native = native.unwrap_or(false);
    let a2 = app.clone();
    let label = title.clone();
    guarded(
        &app,
        if native { "notify.native" } else { "notify.app" },
        &label,
        false,
        |_| async move {
            if native {
                ps::toast(&title, &body).await
            } else {
                crate::notifications::push(&a2, "info", &title, &body, "Windows Integration").await.map(|_| ())
            }
        },
        |_| "показано".into(),
        |_| None,
    )
    .await
}

#[tauri::command]
pub async fn win_uia_inspect(app: AppHandle, hwnd: i64) -> AppResult<Vec<ps::UiElement>> {
    if !win::exists(hwnd) {
        return Err(AppError::NotFound("окно".into()));
    }
    let label = window_label(hwnd);
    guarded(&app, "uia.inspect", &label, false, |_| ps::uia_inspect(hwnd), |l| format!("элементов: {}", l.len()), |_| None).await
}

// --- undo (called from the Action Log) ---

pub async fn undo(app: &AppHandle, action: &str, data: &Value) -> AppResult<String> {
    let state = app.state::<AppState>();
    if action != "organizer.move" {
        require(&state)?;
    }
    match action {
        "windows.minimize" | "windows.maximize" | "windows.restore" => {
            let hwnd = data["hwnd"].as_i64().unwrap_or(0);
            if !win::exists(hwnd) {
                return Err(AppError::NotFound("окно (оно уже закрыто)".into()));
            }
            let s: ShowState = serde_json::from_value(data["state"].clone()).unwrap_or(ShowState::Normal);
            win::show(hwnd, s);
            Ok("состояние окна возвращено".into())
        }
        "volume.set" => {
            let level = data["level"].as_f64().map(|l| l as f32);
            let muted = data["muted"].as_bool();
            blocking(move || audio::set(level, muted)).await?.map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;
            Ok(format!("громкость {:.0}%", level.unwrap_or(0.0) * 100.0))
        }
        "brightness.set" => {
            let level = data["level"].as_u64().unwrap_or(50) as u8;
            ps::set_brightness(level).await?;
            Ok(format!("яркость {level}%"))
        }
        "power.set" => {
            let guid = data["guid"].as_str().unwrap_or("");
            ps::set_power_plan(guid).await?;
            Ok("прежняя схема электропитания".into())
        }
        "clipboard.write" => {
            crate::clipboard_hub::copy_text(data["text"].as_str().unwrap_or("")).await?;
            Ok("прежний текст в буфере".into())
        }
        "desktops.switch" => {
            require(&state)?;
            crate::permissions::require(&state.db, crate::permissions::PERM_KEYBOARD, "undo").await?;
            let key = if data["direction"] == "next" { win::VK_RIGHT } else { win::VK_LEFT };
            blocking(move || win::win_ctrl(key)).await?;
            Ok("предыдущий рабочий стол".into())
        }
        "organizer.move" => {
            let op = data["operation_id"].as_str().unwrap_or("");
            let r = crate::organizer::mover::undo_operation(&state.db, op).await?;
            Ok(format!("восстановлено файлов: {}", r.restored))
        }
        other => Err(AppError::Invalid(format!("отмена для {other} не поддерживается"))),
    }
}
