//! Desktop Organizer: sorts desktop files into folders by rules, always with
//! a preview, a move journal and undo. Never deletes files. Modes: manual
//! (page), button (dashboard/automations), schedule, login, file limit.
//! See `docs/desktop-organizer.md`.

pub mod mover;
pub mod rules;

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{Datelike, Local};
use serde::Serialize;
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{AppError, AppResult};
use crate::modules::{catalog::ORGANIZER, registry};
use crate::state::AppState;
use mover::{ApplyReport, Move, Operation, Plan, PlanItem, ScanOptions, UndoReport};
use rules::{Rule, RuleInput};

pub const EVENT: &str = "focusdeck://organizer-changed";

struct Settings {
    root: String,
    exclude: Vec<String>,
    min_age: u32,
    auto_move: bool,
    schedule: Option<(String, bool)>,
    run_on_login: bool,
    limit: Option<usize>,
    notify: bool,
}

fn settings_of(v: &Value) -> Settings {
    let s = |k: &str, d: &str| v.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
    let b = |k: &str, d: bool| v.get(k).and_then(Value::as_bool).unwrap_or(d);
    let i = |k: &str, d: i64| v.get(k).and_then(Value::as_i64).unwrap_or(d);
    Settings {
        root: s("target_root", "%USERPROFILE%\\Documents\\Desktop Organizer"),
        exclude: mover::patterns(&s("exclude_patterns", "")),
        min_age: i("min_age_minutes", 5).max(0) as u32,
        auto_move: s("auto_mode", "preview") == "move",
        schedule: b("schedule_enabled", false).then(|| (s("schedule_time", "18:00"), b("schedule_weekdays_only", true))),
        run_on_login: b("run_on_login", false),
        limit: b("limit_enabled", false).then(|| i("file_limit", 40).max(1) as usize),
        notify: b("notify", true),
    }
}

async fn settings(pool: &SqlitePool) -> AppResult<Settings> {
    Ok(settings_of(&registry::settings(pool, ORGANIZER).await?))
}

fn desktop(app: &AppHandle) -> AppResult<PathBuf> {
    app.path().desktop_dir().map_err(|e| AppError::Other(anyhow::anyhow!("не найдена папка рабочего стола: {e}")))
}

/// The scan runs on a blocking thread (a full desktop can be slow).
async fn make_plan(app: &AppHandle) -> AppResult<Plan> {
    let db = &app.state::<AppState>().db;
    let s = settings(db).await?;
    let rules = rules::list(db).await?;
    let dir = desktop(app)?;
    let opts = ScanOptions { root: s.root, exclude: s.exclude, min_age_minutes: s.min_age };
    tauri::async_runtime::spawn_blocking(move || mover::plan(&dir, &rules, &opts))
        .await
        .map_err(anyhow::Error::from)?
        .map_err(|e| AppError::Other(anyhow::anyhow!("не удалось прочитать рабочий стол: {e}")))
}

/// A preview prepared by an automatic run (`auto_mode = preview`), waiting on the page.
#[derive(Debug, Clone, Serialize)]
pub struct PendingPreview {
    pub source: String,
    pub created_at: String,
    pub files: usize,
}

static PENDING: Mutex<Option<PendingPreview>> = Mutex::new(None);

async fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = crate::notifications::push(app, "info", title, body, "Desktop Organizer").await;
}

/// Automatic run: moves (auto_mode = move) or prepares a preview and asks the user.
pub async fn auto_run(app: &AppHandle, source: &str) -> AppResult<()> {
    let db = &app.state::<AppState>().db;
    let s = settings(db).await?;
    let plan = make_plan(app).await?;
    if plan.items.is_empty() {
        return Ok(());
    }
    if s.auto_move {
        let report = mover::apply(db, &desktop(app)?, &plan.items, source).await?;
        if s.notify {
            notify(app, "Рабочий стол упорядочен", &format!("Перемещено файлов: {}. Отменить можно в Desktop Organizer → Журнал.", report.moved)).await;
        }
    } else {
        *PENDING.lock().unwrap() = Some(PendingPreview { source: source.into(), created_at: chrono::Utc::now().to_rfc3339(), files: plan.items.len() });
        if s.notify {
            notify(app, "Можно упорядочить рабочий стол", &format!("Готов предпросмотр: {} файлов. Откройте Desktop Organizer, чтобы проверить и переместить.", plan.items.len())).await;
        }
    }
    let _ = app.emit(EVENT, ());
    Ok(())
}

/// Background task of the module: schedule, login and file limit.
pub async fn watch_loop(app: AppHandle) {
    let state = app.state::<AppState>();
    // «At login»: FocusDeck started within 15 minutes after Windows (autostart).
    if let Ok(s) = settings(&state.db).await {
        let uptime = tauri::async_runtime::spawn_blocking(crate::automation::system::uptime_secs).await.unwrap_or(u64::MAX);
        if s.run_on_login && uptime < 15 * 60 {
            tokio::time::sleep(Duration::from_secs(20)).await;
            if let Err(e) = auto_run(&app, "login").await {
                tracing::warn!("organizer login run: {e}");
            }
        }
    }
    let mut last_minute = String::new();
    let mut over_limit = false;
    let mut ticker = tokio::time::interval(Duration::from_secs(30));
    loop {
        ticker.tick().await;
        let Ok(s) = settings(&state.db).await else { continue };
        let now = Local::now();
        let minute = now.format("%H:%M").to_string();
        if let Some((time, weekdays)) = &s.schedule {
            if minute != last_minute && *time == minute && !(*weekdays && now.weekday().number_from_monday() > 5) {
                if let Err(e) = auto_run(&app, "schedule").await {
                    tracing::warn!("organizer schedule: {e}");
                }
            }
        }
        last_minute = minute;
        if let Some(limit) = s.limit {
            if let Ok(dir) = desktop(&app) {
                // Count only files that could be sorted: once per crossing, not every tick.
                let count = tauri::async_runtime::spawn_blocking(move || {
                    std::fs::read_dir(dir)
                        .map(|r| r.filter_map(Result::ok).filter(|e| e.file_type().is_ok_and(|t| t.is_file())).filter(|e| !e.file_name().to_string_lossy().to_lowercase().ends_with(".lnk")).count())
                        .unwrap_or(0)
                })
                .await
                .unwrap_or(0);
                if count > limit && !over_limit {
                    over_limit = true;
                    if let Err(e) = auto_run(&app, "limit").await {
                        tracing::warn!("organizer limit: {e}");
                    }
                } else if count <= limit {
                    over_limit = false;
                }
            }
        }
    }
}

fn require(state: &AppState) -> AppResult<()> {
    crate::productivity::require(state, ORGANIZER)
}

// --- commands ---

#[tauri::command]
pub async fn organizer_preview(app: AppHandle, state: State<'_, AppState>) -> AppResult<Plan> {
    require(&state)?;
    rules::seed(&state.db).await?;
    make_plan(&app).await
}

/// Moves the files the user kept in the preview. `source`: manual | button.
#[tauri::command]
pub async fn organizer_apply(app: AppHandle, state: State<'_, AppState>, items: Vec<PlanItem>, source: Option<String>) -> AppResult<ApplyReport> {
    require(&state)?;
    let source = match source.as_deref() {
        Some("button") => "button",
        Some(s) if ["schedule", "login", "limit"].contains(&s) => s,
        _ => "manual",
    };
    let report = mover::apply(&state.db, &desktop(&app)?, &items, source).await?;
    *PENDING.lock().unwrap() = None;
    if settings(&state.db).await?.notify && report.moved > 0 {
        notify(&app, "Рабочий стол упорядочен", &format!("Перемещено файлов: {}", report.moved)).await;
    }
    let _ = app.emit(EVENT, ());
    Ok(report)
}

/// One click: scan and move everything the rules match (dashboard button, automations).
#[tauri::command]
pub async fn organizer_run_now(app: AppHandle, state: State<'_, AppState>) -> AppResult<ApplyReport> {
    require(&state)?;
    let plan = make_plan(&app).await?;
    if plan.items.is_empty() {
        return Ok(ApplyReport { operation_id: String::new(), moved: 0, failed: 0, errors: vec![] });
    }
    organizer_apply(app, state, plan.items, Some("button".into())).await
}

#[tauri::command]
pub fn organizer_pending(state: State<'_, AppState>) -> AppResult<Option<PendingPreview>> {
    require(&state)?;
    Ok(PENDING.lock().unwrap().clone())
}

#[tauri::command]
pub fn organizer_dismiss_pending(state: State<'_, AppState>) -> AppResult<()> {
    require(&state)?;
    *PENDING.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub async fn organizer_operations(state: State<'_, AppState>, limit: Option<i64>) -> AppResult<Vec<Operation>> {
    require(&state)?;
    mover::operations(&state.db, limit.unwrap_or(100)).await
}

#[tauri::command]
pub async fn organizer_moves(state: State<'_, AppState>, operation_id: String) -> AppResult<Vec<Move>> {
    require(&state)?;
    mover::moves(&state.db, &operation_id).await
}

async fn after_undo(app: &AppHandle, r: &UndoReport) {
    let db = &app.state::<AppState>().db;
    if settings(db).await.is_ok_and(|s| s.notify) && r.restored > 0 {
        notify(app, "Файлы возвращены", &format!("Восстановлено: {}{}", r.restored, if r.failed > 0 { format!(", не удалось: {}", r.failed) } else { String::new() })).await;
    }
    let _ = app.emit(EVENT, ());
}

#[tauri::command]
pub async fn organizer_undo_last(app: AppHandle, state: State<'_, AppState>) -> AppResult<UndoReport> {
    require(&state)?;
    let op = mover::last_undoable(&state.db).await?.ok_or_else(|| AppError::Invalid("нечего отменять".into()))?;
    let r = mover::undo_operation(&state.db, &op).await?;
    after_undo(&app, &r).await;
    Ok(r)
}

#[tauri::command]
pub async fn organizer_undo_operation(app: AppHandle, state: State<'_, AppState>, operation_id: String) -> AppResult<UndoReport> {
    require(&state)?;
    let r = mover::undo_operation(&state.db, &operation_id).await?;
    after_undo(&app, &r).await;
    Ok(r)
}

/// Restores chosen files of any operation.
#[tauri::command]
pub async fn organizer_restore(app: AppHandle, state: State<'_, AppState>, move_ids: Vec<i64>) -> AppResult<UndoReport> {
    require(&state)?;
    let r = mover::undo_moves(&state.db, &move_ids).await?;
    after_undo(&app, &r).await;
    Ok(r)
}

#[tauri::command]
pub async fn organizer_rules(state: State<'_, AppState>) -> AppResult<Vec<Rule>> {
    require(&state)?;
    rules::seed(&state.db).await?;
    rules::list(&state.db).await
}

#[tauri::command]
pub async fn organizer_save_rule(state: State<'_, AppState>, id: Option<String>, input: RuleInput) -> AppResult<Vec<Rule>> {
    require(&state)?;
    rules::save(&state.db, id.as_deref(), &input).await?;
    rules::list(&state.db).await
}

#[tauri::command]
pub async fn organizer_delete_rule(state: State<'_, AppState>, id: String) -> AppResult<Vec<Rule>> {
    require(&state)?;
    rules::delete(&state.db, &id).await?;
    rules::list(&state.db).await
}

#[tauri::command]
pub async fn organizer_reorder_rules(state: State<'_, AppState>, ids: Vec<String>) -> AppResult<Vec<Rule>> {
    require(&state)?;
    rules::reorder(&state.db, &ids).await?;
    rules::list(&state.db).await
}

#[tauri::command]
pub async fn organizer_reset_rules(state: State<'_, AppState>) -> AppResult<Vec<Rule>> {
    require(&state)?;
    rules::reset(&state.db).await?;
    rules::list(&state.db).await
}

/// Adds a file name to the module's exclusions («исключить навсегда» in the preview).
#[tauri::command]
pub async fn organizer_exclude(app: AppHandle, state: State<'_, AppState>, name: String) -> AppResult<()> {
    require(&state)?;
    let name = name.trim();
    if name.is_empty() || name.contains(['\\', '/']) {
        return Err(AppError::Invalid("укажите имя файла или маску".into()));
    }
    let mut s = registry::settings(&state.db, ORGANIZER).await?;
    let list = s.get("exclude_patterns").and_then(Value::as_str).unwrap_or("").to_string();
    if !mover::patterns(&list).iter().any(|p| p.eq_ignore_ascii_case(name)) {
        s["exclude_patterns"] = Value::String(if list.trim().is_empty() { name.into() } else { format!("{list}, {name}") });
        registry::update_settings(&state.db, ORGANIZER, &s, "user").await?;
        crate::modules::effects::notify_changed(&app);
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct Paths {
    pub desktop: String,
    pub root: String,
}

#[tauri::command]
pub async fn organizer_paths(app: AppHandle, state: State<'_, AppState>) -> AppResult<Paths> {
    require(&state)?;
    let s = settings(&state.db).await?;
    Ok(Paths { desktop: desktop(&app)?.display().to_string(), root: crate::automation::system::expand_env(&s.root) })
}

/// Opens a folder in Explorer (the target root or a target folder).
#[tauri::command]
pub async fn organizer_open_folder(app: AppHandle, state: State<'_, AppState>, path: String) -> AppResult<()> {
    require(&state)?;
    use tauri_plugin_opener::OpenerExt;
    let p = PathBuf::from(&path);
    let target = if p.is_dir() { p } else { p.parent().map(PathBuf::from).filter(|d| d.is_dir()).ok_or_else(|| AppError::NotFound(path.clone()))? };
    app.opener().open_path(target.display().to_string(), None::<&str>).map_err(anyhow::Error::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_parsing() {
        let s = settings_of(&serde_json::json!({ "auto_mode": "move", "schedule_enabled": true, "schedule_time": "09:30", "limit_enabled": true, "file_limit": 10, "exclude_patterns": "*.lnk, a.txt" }));
        assert!(s.auto_move);
        assert_eq!(s.schedule, Some(("09:30".into(), true)));
        assert_eq!(s.limit, Some(10));
        assert_eq!(s.exclude, ["*.lnk", "a.txt"]);
        let d = settings_of(&serde_json::json!({}));
        assert!(!d.auto_move && d.schedule.is_none() && d.limit.is_none() && !d.run_on_login && d.notify);
    }
}
