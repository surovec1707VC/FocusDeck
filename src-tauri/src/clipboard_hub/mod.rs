//! Clipboard Hub: local clipboard history (text, links, files, images, HTML,
//! RTF), quick paste, private mode and quick actions. Everything belongs to
//! the `clipboard_history` module: while it is off nothing is recorded, its
//! commands fail with `module_disabled` and its hotkeys are not registered.

pub mod actions;
pub mod store;
pub mod win;

use std::sync::Mutex;
use std::time::Duration;

use base64::Engine;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::error::{AppError, AppResult};
use crate::modules::{catalog::CLIPBOARD, registry};
use crate::permissions::{self, PERM_CLIPBOARD_READ, PERM_PASTE};
use crate::state::AppState;
use store::{ClipFilter, ClipPage, ClipStats};
use win::{Clip, ReadOptions};

pub const CHANGED_EVENT: &str = "focusdeck://clipboard-changed";
pub const PRIVATE_EVENT: &str = "focusdeck://clipboard-private";
const POLL: Duration = Duration::from_millis(1000);

/// Private Mode: `None` = off, `Some(None)` = until turned off, `Some(Some(t))` = until epoch ms `t`.
static PRIVATE: Mutex<Option<Option<i64>>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize)]
pub struct PrivateMode {
    pub active: bool,
    /// Epoch ms when it ends by itself.
    pub until: Option<i64>,
}

pub fn private_mode() -> PrivateMode {
    let mut p = PRIVATE.lock().unwrap();
    if let Some(Some(until)) = *p {
        if until <= Utc::now().timestamp_millis() {
            *p = None;
        }
    }
    PrivateMode { active: p.is_some(), until: p.flatten() }
}

pub fn set_private(app: &AppHandle, enabled: bool, minutes: Option<i64>) -> PrivateMode {
    *PRIVATE.lock().unwrap() = enabled.then(|| minutes.filter(|m| *m > 0).map(|m| Utc::now().timestamp_millis() + m * 60_000));
    let state = private_mode();
    tracing::info!(active = state.active, "clipboard private mode");
    let _ = app.emit(PRIVATE_EVENT, &state);
    state
}

/// Module settings used while recording.
struct Settings {
    max_items: i64,
    retention_days: i64,
    save_images: bool,
    max_image_bytes: usize,
    rich: bool,
    max_length: usize,
    excluded: Vec<String>,
    exclude_private: bool,
    skip_sensitive: bool,
}

fn settings_of(v: &Value) -> Settings {
    let i = |k: &str, d: i64| v.get(k).and_then(Value::as_i64).unwrap_or(d);
    let b = |k: &str, d: bool| v.get(k).and_then(Value::as_bool).unwrap_or(d);
    Settings {
        max_items: i("max_items", 500),
        retention_days: i("retention_days", 30),
        save_images: b("save_images", true),
        max_image_bytes: (i("max_image_mb", 10) as usize) << 20,
        rich: b("save_rich", true),
        max_length: i("max_length", 100_000) as usize,
        excluded: excluded_apps(v.get("excluded_apps").and_then(Value::as_str).unwrap_or("")),
        exclude_private: b("exclude_private_windows", true),
        skip_sensitive: b("skip_sensitive", true),
    }
}

/// "KeePass.exe, bitwarden" → ["keepass.exe", "bitwarden.exe"].
pub fn excluded_apps(list: &str) -> Vec<String> {
    list.split([',', ';', '\n'])
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .map(|s| if s.ends_with(".exe") { s } else { format!("{s}.exe") })
        .collect()
}

/// Why a copy isn't recorded (None = record it).
fn skip_reason(s: &Settings, exe: &str, title: &str) -> Option<&'static str> {
    if private_mode().active {
        return Some("private mode");
    }
    if !exe.is_empty() && s.excluded.contains(&exe.to_lowercase()) {
        return Some("excluded app");
    }
    if s.exclude_private && win::is_private_title(title) {
        return Some("private window");
    }
    None
}

/// Background task of the module: watches the clipboard sequence number.
pub async fn poll_loop(app: AppHandle) {
    // Only what is copied while FocusDeck runs is recorded.
    let mut last_seq = win::sequence();
    let mut ticks: u64 = 0;
    loop {
        tokio::time::sleep(POLL).await;
        ticks += 1;
        if ticks % 3600 == 1 {
            let db = &app.state::<AppState>().db;
            if let Ok(v) = registry::settings(db, CLIPBOARD).await {
                let _ = store::purge_older_than(db, settings_of(&v).retention_days).await;
            }
        }
        let seq = win::sequence();
        if seq == last_seq {
            continue;
        }
        last_seq = seq;
        if let Err(e) = capture(&app).await {
            tracing::warn!("clipboard history: {e}");
        }
    }
}

async fn capture(app: &AppHandle) -> AppResult<()> {
    let db = &app.state::<AppState>().db;
    if !permissions::is_granted(db, PERM_CLIPBOARD_READ).await? {
        return Ok(());
    }
    let s = settings_of(&registry::settings(db, CLIPBOARD).await?);
    let (exe, title) = tauri::async_runtime::spawn_blocking(win::source).await.map_err(anyhow::Error::from)?;
    if let Some(reason) = skip_reason(&s, &exe, &title) {
        tracing::debug!(reason, "clipboard change not recorded");
        return Ok(());
    }
    let opts = ReadOptions { images: s.save_images, max_image_bytes: s.max_image_bytes, rich: s.rich, skip_hidden: s.skip_sensitive };
    let Some(mut clip) = tauri::async_runtime::spawn_blocking(move || win::read(opts)).await.map_err(anyhow::Error::from)? else {
        return Ok(());
    };
    if clip.text.as_ref().is_some_and(|t| t.chars().count() > s.max_length) {
        if clip.files.is_none() && clip.image.is_none() {
            return Ok(());
        }
        clip.text = None;
    }
    if store::add(db, &clip, &exe, s.max_items).await? {
        let _ = app.emit(CHANGED_EVENT, ());
    }
    Ok(())
}

/// At start and exit: without «persist history», unpinned entries are removed.
pub async fn apply_persistence(pool: &SqlitePool) -> AppResult<()> {
    let v = registry::settings(pool, CLIPBOARD).await?;
    if v.get("persist_history").and_then(Value::as_bool) == Some(false) {
        let n = store::clear_all(pool, false).await?;
        tracing::info!(removed = n, "clipboard history not persisted");
    }
    store::purge_older_than(pool, settings_of(&v).retention_days).await?;
    Ok(())
}

pub async fn copy_text(text: &str) -> AppResult<()> {
    write(Clip { text: Some(text.to_string()), ..Default::default() }).await
}

pub async fn write(clip: Clip) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || win::write(&clip))
        .await
        .map_err(anyhow::Error::from)?
        .map_err(|e| AppError::Other(anyhow::anyhow!("не удалось записать в буфер обмена: {e}")))
}

/// Hides FocusDeck so the previous window gets focus, then presses Ctrl+V there.
pub async fn paste_into_previous(app: &AppHandle) -> AppResult<()> {
    let delay = {
        let db = &app.state::<AppState>().db;
        registry::settings(db, crate::modules::catalog::SNIPPETS).await.ok().and_then(|s| s["paste_delay_ms"].as_u64()).unwrap_or(250)
    };
    crate::window::hide(app);
    tokio::time::sleep(Duration::from_millis(delay)).await;
    tauri::async_runtime::spawn_blocking(crate::productivity::snippets::paste_keys).await.map_err(anyhow::Error::from)?;
    Ok(())
}

fn require(state: &AppState) -> AppResult<()> {
    crate::productivity::require(state, CLIPBOARD)
}

// --- commands: history ---

#[tauri::command]
pub async fn clipboard_list(state: State<'_, AppState>, filter: Option<ClipFilter>) -> AppResult<ClipPage> {
    require(&state)?;
    store::query(&state.db, &filter.unwrap_or_default()).await
}

/// The image of an entry as a `data:image/bmp;base64,…` URL.
#[tauri::command]
pub async fn clipboard_image(state: State<'_, AppState>, id: i64) -> AppResult<String> {
    require(&state)?;
    let bmp = store::image(&state.db, id).await?;
    Ok(format!("data:image/bmp;base64,{}", base64::engine::general_purpose::STANDARD.encode(bmp)))
}

/// Full details of an entry for the preview pane (HTML/RTF as text, never rendered).
#[derive(Debug, Serialize)]
pub struct ClipDetails {
    pub text: Option<String>,
    pub html_text: Option<String>,
    pub rtf: Option<String>,
    pub files: Option<Vec<String>>,
}

#[tauri::command]
pub async fn clipboard_details(state: State<'_, AppState>, id: i64) -> AppResult<ClipDetails> {
    require(&state)?;
    let c = store::clip(&state.db, id).await?;
    Ok(ClipDetails {
        html_text: c.html.as_deref().map(win::html_text),
        rtf: c.rtf.map(|r| r.chars().take(20_000).collect()),
        text: c.text,
        files: c.files,
    })
}

/// Puts an entry back on the clipboard with all its formats.
#[tauri::command]
pub async fn clipboard_copy(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    require(&state)?;
    write(store::clip(&state.db, id).await?).await?;
    store::touch(&state.db, id).await
}

/// Copies the entry and pastes it into the program you were working in.
#[tauri::command]
pub async fn clipboard_paste(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<()> {
    require(&state)?;
    permissions::require(&state.db, PERM_PASTE, "clipboard_hub").await?;
    write(store::clip(&state.db, id).await?).await?;
    store::touch(&state.db, id).await?;
    paste_into_previous(&app).await
}

#[tauri::command]
pub async fn clipboard_pin(state: State<'_, AppState>, id: i64, pinned: bool) -> AppResult<()> {
    require(&state)?;
    store::set_pinned(&state.db, id, pinned).await
}

#[tauri::command]
pub async fn clipboard_delete(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    require(&state)?;
    store::delete(&state.db, id).await
}

#[tauri::command]
pub async fn clipboard_clear(app: AppHandle, state: State<'_, AppState>, include_pinned: Option<bool>) -> AppResult<u64> {
    require(&state)?;
    let n = store::clear_all(&state.db, include_pinned.unwrap_or(false)).await?;
    let _ = app.emit(CHANGED_EVENT, ());
    Ok(n)
}

#[derive(Debug, Serialize)]
pub struct HubStatus {
    /// The background task runs and the permission is granted.
    pub recording: bool,
    pub permission: bool,
    pub private_mode: PrivateMode,
    pub stats: ClipStats,
}

#[tauri::command]
pub async fn clipboard_status(app: AppHandle, state: State<'_, AppState>) -> AppResult<HubStatus> {
    require(&state)?;
    let permission = permissions::is_granted(&state.db, PERM_CLIPBOARD_READ).await?;
    Ok(HubStatus {
        recording: permission && crate::background::running(&app, CLIPBOARD) && !private_mode().active,
        permission,
        private_mode: private_mode(),
        stats: store::stats(&state.db).await?,
    })
}

/// Turns Private Mode on (for `minutes`, or until turned off) or off.
#[tauri::command]
pub async fn clipboard_set_private(app: AppHandle, state: State<'_, AppState>, enabled: bool, minutes: Option<i64>) -> AppResult<PrivateMode> {
    require(&state)?;
    if minutes.is_some_and(|m| !(0..=1440).contains(&m)) {
        return Err(AppError::Invalid("от 0 до 1440 минут".into()));
    }
    Ok(set_private(&app, enabled, minutes))
}

// --- commands: quick actions ---

#[tauri::command]
pub async fn quick_actions_list(state: State<'_, AppState>) -> AppResult<Vec<actions::QuickAction>> {
    require(&state)?;
    actions::list(&state.db).await
}

#[tauri::command]
pub async fn quick_actions_save(app: AppHandle, state: State<'_, AppState>, id: Option<String>, input: actions::ActionInput) -> AppResult<Vec<actions::QuickAction>> {
    require(&state)?;
    let has_hotkey = input.hotkey.as_deref().is_some_and(|h| !h.trim().is_empty());
    if let Some(h) = input.hotkey.as_deref().filter(|_| has_hotkey) {
        <tauri_plugin_global_shortcut::Shortcut as std::str::FromStr>::from_str(h.trim())
            .map_err(|e| AppError::Invalid(format!("сочетание «{h}» не распознано: {e}")))?;
    }
    let saved = actions::save(&state.db, id.as_deref(), input).await?;
    let target = has_hotkey.then(|| crate::hotkeys::HotkeyTarget::QuickAction(saved.id.clone()));
    if let Err(e) = crate::hotkeys::sync_reporting(&app, target).await {
        // Keep the action, drop the shortcut that didn't work.
        sqlx::query("UPDATE quick_actions SET hotkey = NULL WHERE id = ?").bind(&saved.id).execute(&state.db).await?;
        let _ = crate::hotkeys::sync(&app).await;
        return Err(e);
    }
    actions::list(&state.db).await
}

#[tauri::command]
pub async fn quick_actions_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<Vec<actions::QuickAction>> {
    require(&state)?;
    actions::delete(&state.db, &id).await?;
    let _ = crate::hotkeys::sync(&app).await;
    actions::list(&state.db).await
}

#[tauri::command]
pub async fn quick_actions_reorder(state: State<'_, AppState>, id: String, delta: i64) -> AppResult<Vec<actions::QuickAction>> {
    require(&state)?;
    actions::reorder(&state.db, &id, delta).await?;
    actions::list(&state.db).await
}

#[tauri::command]
pub async fn quick_actions_run(app: AppHandle, state: State<'_, AppState>, id: String, confirm: Option<bool>) -> AppResult<actions::RunResult> {
    require(&state)?;
    actions::run(&app, &id, confirm.unwrap_or(false)).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PickKind {
    App,
    File,
    Folder,
}

/// File/folder dialog for the quick action editor; nothing is opened.
#[tauri::command]
pub async fn quick_actions_pick(app: AppHandle, state: State<'_, AppState>, kind: PickKind) -> AppResult<Option<String>> {
    require(&state)?;
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let d = app.dialog().file();
        match kind {
            PickKind::App => d.set_title("Выберите программу").add_filter("Программы и ярлыки", crate::widgets::APP_EXTENSIONS).blocking_pick_file(),
            PickKind::File => d.set_title("Выберите файл").blocking_pick_file(),
            PickKind::Folder => d.set_title("Выберите папку").blocking_pick_folder(),
        }
    })
    .await
    .map_err(anyhow::Error::from)?;
    picked.map(|p| p.into_path().map(|p| p.display().to_string()).map_err(|e| AppError::Other(e.into()))).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        settings_of(&serde_json::json!({ "excluded_apps": "KeePass.exe; bitwarden" }))
    }

    #[test]
    fn exclusions_and_private_mode() {
        assert_eq!(excluded_apps(" KeePass.exe, bitwarden ,,"), ["keepass.exe", "bitwarden.exe"]);
        let s = settings();
        assert_eq!(skip_reason(&s, "KEEPASS.EXE", "База"), Some("excluded app"));
        assert_eq!(skip_reason(&s, "msedge.exe", "Вкладка — InPrivate"), Some("private window"));
        assert_eq!(skip_reason(&s, "notepad.exe", "Блокнот"), None);

        *PRIVATE.lock().unwrap() = Some(None);
        assert!(private_mode().active);
        assert_eq!(skip_reason(&s, "notepad.exe", "Блокнот"), Some("private mode"));
        // A finished timed Private Mode turns itself off.
        *PRIVATE.lock().unwrap() = Some(Some(Utc::now().timestamp_millis() - 1));
        assert!(!private_mode().active);
        assert_eq!(skip_reason(&s, "notepad.exe", "Блокнот"), None);
    }
}
