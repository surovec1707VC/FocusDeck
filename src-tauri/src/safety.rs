//! Safety layer shared by modules that act on the system: the catalog of
//! sensitive actions, Safe Mode (only whitelisted actions run, every change
//! is confirmed), and the Action Log with undo.
//!
//! Flow of a guarded action: `authorize` (permission → Safe Mode whitelist →
//! confirmation) → do it → `record` (result + undo data).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{AppError, AppResult};
use crate::permissions::{self, *};
use crate::state::AppState;

pub const EVENT: &str = "focusdeck://action-log";
const SAFE_MODE_KEY: &str = "safe_mode.enabled";
const WHITELIST_KEY: &str = "safe_mode.whitelist";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    /// Only reads state.
    Read,
    /// Changes something, easy to revert.
    Change,
    /// Hard to revert or may lose data: always confirmed.
    Dangerous,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionDef {
    pub id: &'static str,
    pub name: &'static str,
    pub module: &'static str,
    pub permission: Option<&'static str>,
    pub risk: Risk,
    pub undoable: bool,
}

const fn a(id: &'static str, name: &'static str, module: &'static str, permission: Option<&'static str>, risk: Risk, undoable: bool) -> ActionDef {
    ActionDef { id, name, module, permission, risk, undoable }
}

const WIN: &str = "windows_integration";

pub const ACTIONS: &[ActionDef] = &[
    a("windows.list", "Список окон и активное окно", WIN, Some(PERM_WINDOWS), Risk::Read, false),
    a("windows.focus", "Переключиться на окно", WIN, Some(PERM_WINDOWS), Risk::Change, false),
    a("windows.minimize", "Свернуть окно", WIN, Some(PERM_WINDOWS), Risk::Change, true),
    a("windows.maximize", "Развернуть окно", WIN, Some(PERM_WINDOWS), Risk::Change, true),
    a("windows.restore", "Восстановить окно", WIN, Some(PERM_WINDOWS), Risk::Change, true),
    a("windows.close", "Закрыть окно", WIN, Some(PERM_WINDOWS), Risk::Dangerous, false),
    a("apps.search", "Поиск установленных приложений", WIN, None, Risk::Read, false),
    a("apps.launch", "Запуск приложения", WIN, Some(PERM_LAUNCH_APPS), Risk::Change, false),
    a("desktops.read", "Виртуальные рабочие столы: состояние", WIN, None, Risk::Read, false),
    a("desktops.switch", "Переключить рабочий стол", WIN, Some(PERM_KEYBOARD), Risk::Change, true),
    a("desktops.new", "Создать рабочий стол", WIN, Some(PERM_KEYBOARD), Risk::Change, false),
    a("desktops.close", "Закрыть рабочий стол", WIN, Some(PERM_KEYBOARD), Risk::Dangerous, false),
    a("volume.read", "Громкость: чтение", WIN, None, Risk::Read, false),
    a("volume.set", "Изменить громкость", WIN, Some(PERM_SYSTEM_SETTINGS), Risk::Change, true),
    a("brightness.read", "Яркость: чтение", WIN, None, Risk::Read, false),
    a("brightness.set", "Изменить яркость", WIN, Some(PERM_SYSTEM_SETTINGS), Risk::Change, true),
    a("wifi.read", "Состояние Wi-Fi", WIN, None, Risk::Read, false),
    a("bluetooth.read", "Состояние Bluetooth", WIN, None, Risk::Read, false),
    a("power.read", "Схемы электропитания: чтение", WIN, None, Risk::Read, false),
    a("power.set", "Сменить схему электропитания", WIN, Some(PERM_SYSTEM_SETTINGS), Risk::Dangerous, true),
    a("screen.capture", "Скриншот", WIN, Some(PERM_SCREEN), Risk::Change, false),
    a("clipboard.read", "Прочитать буфер обмена", WIN, Some(PERM_CLIPBOARD_READ), Risk::Read, false),
    a("clipboard.write", "Записать в буфер обмена", WIN, Some(PERM_CLIPBOARD_WRITE), Risk::Change, true),
    a("notify.app", "Уведомление в FocusDeck", WIN, Some(PERM_NOTIFICATIONS), Risk::Change, false),
    a("notify.native", "Уведомление Windows", WIN, Some(PERM_NOTIFICATIONS), Risk::Change, false),
    a("uia.inspect", "UI Automation: структура окна", WIN, Some(PERM_UI_AUTOMATION), Risk::Read, false),
    // Actions of other modules that go through the log and Safe Mode.
    a("automations.run", "Выполнение автоматизации", "automations", None, Risk::Change, false),
    a("organizer.move", "Перемещение файлов рабочего стола", "desktop_organizer", Some(PERM_MOVE_FILES), Risk::Change, true),
    a("clipboard.quick_action", "Быстрое действие Clipboard Hub", "clipboard_history", None, Risk::Change, false),
    // Developer Tools
    a("git.stage", "Git: добавить или убрать файлы из индекса", "dev_git", Some(PERM_GIT), Risk::Change, false),
    a("git.commit", "Git: создать коммит", "dev_git", Some(PERM_GIT), Risk::Change, false),
    a("git.pull", "Git: pull (только fast-forward)", "dev_git", Some(PERM_GIT), Risk::Dangerous, false),
    a("git.push", "Git: push", "dev_git", Some(PERM_GIT), Risk::Dangerous, false),
    a("projects.open", "Открыть проект в редакторе, терминале или проводнике", "dev_projects", Some(PERM_LAUNCH_APPS), Risk::Change, false),
    a("projects.run", "Запустить команду проекта", "dev_projects", Some(PERM_RUN_COMMANDS), Risk::Change, false),
    a("terminal.open", "Открыть терминал в окне", "dev_terminal", Some(PERM_LAUNCH_APPS), Risk::Change, false),
    a("terminal.run", "Выполнить команду во вкладке терминала", "dev_terminal", Some(PERM_RUN_COMMANDS), Risk::Change, false),
    a("api.send", "API Tester: отправить запрос", "dev_api", Some(PERM_NETWORK_API), Risk::Change, false),
];

pub fn find(id: &str) -> AppResult<&'static ActionDef> {
    ACTIONS.iter().find(|a| a.id == id).ok_or_else(|| AppError::Invalid(format!("неизвестное действие {id}")))
}

// --- Safe Mode ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeMode {
    pub enabled: bool,
    /// Action ids allowed in Safe Mode.
    pub whitelist: Vec<String>,
}

/// Read-only actions are allowed by default.
pub fn default_whitelist() -> Vec<String> {
    ACTIONS.iter().filter(|a| a.risk == Risk::Read).map(|a| a.id.to_string()).collect()
}

async fn setting(pool: &SqlitePool, key: &str) -> AppResult<Option<Value>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value_json FROM settings WHERE key = ?").bind(key).fetch_optional(pool).await?;
    Ok(row.and_then(|r| serde_json::from_str(&r.0).ok()))
}

async fn put_setting(pool: &SqlitePool, key: &str, v: &Value) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO settings (key, value_json) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = datetime('now')",
    )
    .bind(key)
    .bind(v.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn safe_mode(pool: &SqlitePool) -> AppResult<SafeMode> {
    let enabled = setting(pool, SAFE_MODE_KEY).await?.and_then(|v| v.as_bool()).unwrap_or(false);
    let whitelist = match setting(pool, WHITELIST_KEY).await? {
        Some(v) => serde_json::from_value(v).unwrap_or_else(|_| default_whitelist()),
        None => default_whitelist(),
    };
    Ok(SafeMode { enabled, whitelist })
}

pub async fn set_safe_mode(pool: &SqlitePool, mode: &SafeMode) -> AppResult<()> {
    if let Some(bad) = mode.whitelist.iter().find(|id| find(id).is_err()) {
        return Err(AppError::Invalid(format!("неизвестное действие {bad}")));
    }
    put_setting(pool, SAFE_MODE_KEY, &json!(mode.enabled)).await?;
    put_setting(pool, WHITELIST_KEY, &json!(mode.whitelist)).await
}

// --- authorization and log ---

/// Checks the permission, the Safe Mode whitelist and confirmation. Denials
/// and blocks are written to the Action Log.
pub async fn authorize(pool: &SqlitePool, action: &str, target: &str, confirmed: bool) -> AppResult<&'static ActionDef> {
    let def = find(action)?;
    if let Some(p) = def.permission {
        if let Err(e) = permissions::require(pool, p, def.module).await {
            record(pool, def, target, "denied", &e.to_string(), None).await?;
            return Err(e);
        }
    }
    let mode = safe_mode(pool).await?;
    if mode.enabled && !mode.whitelist.iter().any(|w| w == action) {
        let msg = format!("действие «{}» не входит в белый список", def.name);
        record(pool, def, target, "blocked", &msg, None).await?;
        return Err(AppError::Blocked(msg));
    }
    let needs_confirm = def.risk == Risk::Dangerous || (mode.enabled && def.risk == Risk::Change);
    if needs_confirm && !confirmed {
        let why = if def.risk == Risk::Dangerous { "потенциально опасное действие" } else { "Safe Mode: подтвердите изменение" };
        return Err(AppError::ConfirmationRequired(format!(
            "{}{}: {why}. Продолжить?",
            def.name,
            if target.is_empty() { String::new() } else { format!(" «{target}»") }
        )));
    }
    Ok(def)
}

/// Safe Mode check for actions of other modules (they confirm on their own).
pub async fn allowed_in_safe_mode(pool: &SqlitePool, action: &str, target: &str) -> AppResult<()> {
    let mode = safe_mode(pool).await?;
    if mode.enabled && !mode.whitelist.iter().any(|w| w == action) {
        let def = find(action)?;
        let msg = format!("действие «{}» не входит в белый список", def.name);
        record(pool, def, target, "blocked", &msg, None).await?;
        return Err(AppError::Blocked(msg));
    }
    Ok(())
}

pub async fn record(pool: &SqlitePool, def: &ActionDef, target: &str, result: &str, message: &str, undo: Option<Value>) -> AppResult<i64> {
    let target: String = target.chars().take(300).collect();
    let message: String = message.chars().take(1000).collect();
    let id = sqlx::query("INSERT INTO action_log (module_id, action, target, result, message, permission, undo_json) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(def.module)
        .bind(def.id)
        .bind(&target)
        .bind(result)
        .bind(&message)
        .bind(def.permission)
        .bind(undo.map(|u| u.to_string()))
        .execute(pool)
        .await?
        .last_insert_rowid();
    // Keep the log bounded.
    sqlx::query("DELETE FROM action_log WHERE id <= (SELECT MAX(id) FROM action_log) - 5000").execute(pool).await?;
    Ok(id)
}

/// Records the outcome of an authorized action and notifies the UI.
pub async fn finish<T>(app: &AppHandle, def: &ActionDef, target: &str, result: &AppResult<T>, message: impl FnOnce(&T) -> String, undo: impl FnOnce(&T) -> Option<Value>) {
    let db = &app.state::<AppState>().db;
    let r = match result {
        Ok(v) => record(db, def, target, "ok", &message(v), if def.undoable { undo(v) } else { None }).await,
        Err(e) => record(db, def, target, "error", &e.to_string(), None).await,
    };
    if let Err(e) = r {
        tracing::warn!("action log: {e}");
    }
    let _ = app.emit(EVENT, ());
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct LogEntry {
    pub id: i64,
    pub created_at: String,
    pub module_id: String,
    pub action: String,
    pub target: String,
    pub result: String,
    pub message: String,
    pub permission: Option<String>,
    /// Can be undone now.
    pub can_undo: bool,
    pub undone_at: Option<String>,
}

pub async fn list(pool: &SqlitePool, module: Option<&str>, limit: i64) -> AppResult<Vec<LogEntry>> {
    Ok(sqlx::query_as::<_, LogEntry>(
        "SELECT id, created_at, module_id, action, target, result, message, permission,
                (undo_json IS NOT NULL AND undone_at IS NULL AND result = 'ok') AS can_undo, undone_at
         FROM action_log WHERE (?1 IS NULL OR module_id = ?1) ORDER BY id DESC LIMIT ?2",
    )
    .bind(module)
    .bind(limit.clamp(1, 1000))
    .fetch_all(pool)
    .await?)
}

/// Reverts a logged action using its undo data.
pub async fn undo(app: &AppHandle, id: i64) -> AppResult<String> {
    let db = &app.state::<AppState>().db;
    let row: Option<(String, Option<String>, Option<String>, String)> =
        sqlx::query_as("SELECT action, undo_json, undone_at, result FROM action_log WHERE id = ?").bind(id).fetch_optional(db).await?;
    let (action, undo_json, undone_at, result) = row.ok_or_else(|| AppError::NotFound("запись журнала".into()))?;
    let (Some(data), None, "ok") = (undo_json, undone_at, result.as_str()) else {
        return Err(AppError::Invalid("это действие нельзя отменить".into()));
    };
    let data: Value = serde_json::from_str(&data)?;
    let message = crate::windows_integration::undo(app, &action, &data).await?;
    sqlx::query("UPDATE action_log SET undone_at = datetime('now') WHERE id = ?").bind(id).execute(db).await?;
    let def = find(&action)?;
    record(db, def, "", "undone", &format!("отменена запись №{id}: {message}"), None).await?;
    let _ = app.emit(EVENT, ());
    Ok(message)
}

// --- commands ---

#[tauri::command]
pub fn safety_actions() -> Vec<ActionDef> {
    ACTIONS.to_vec()
}

#[tauri::command]
pub async fn safe_mode_get(state: State<'_, AppState>) -> AppResult<SafeMode> {
    safe_mode(&state.db).await
}

/// Turning Safe Mode off is a security change: it needs the Security module.
#[tauri::command]
pub async fn safe_mode_set(app: AppHandle, state: State<'_, AppState>, mode: SafeMode) -> AppResult<SafeMode> {
    if !state.module_enabled(crate::modules::catalog::SECURITY) {
        return Err(AppError::ModuleDisabled("Безопасность".into()));
    }
    set_safe_mode(&state.db, &mode).await?;
    tracing::info!(enabled = mode.enabled, whitelist = mode.whitelist.len(), "safe mode");
    let _ = app.emit(EVENT, ());
    safe_mode(&state.db).await
}

#[tauri::command]
pub async fn action_log_list(state: State<'_, AppState>, module: Option<String>, limit: Option<i64>) -> AppResult<Vec<LogEntry>> {
    list(&state.db, module.as_deref(), limit.unwrap_or(200)).await
}

#[tauri::command]
pub async fn action_log_undo(app: AppHandle, id: i64) -> AppResult<String> {
    undo(&app, id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog() {
        let ids: std::collections::HashSet<_> = ACTIONS.iter().map(|a| a.id).collect();
        assert_eq!(ids.len(), ACTIONS.len());
        for a in ACTIONS {
            assert!(crate::modules::catalog::find(a.module).is_some(), "{}", a.id);
            assert!(!(a.risk == Risk::Read && a.undoable), "{}", a.id);
        }
        assert!(default_whitelist().iter().all(|id| find(id).unwrap().risk == Risk::Read));
    }

    #[tokio::test]
    async fn authorize_permission_safe_mode_and_confirmation() {
        let pool = crate::db::test_pool().await;
        crate::permissions::seed(&pool).await.unwrap();
        crate::modules::registry::seed(&pool).await.unwrap();

        // No permission → denied and logged.
        assert!(matches!(authorize(&pool, "volume.set", "50%", false).await, Err(AppError::PermissionDenied(_))));
        crate::permissions::set(&pool, PERM_SYSTEM_SETTINGS, true, "test").await.unwrap();
        crate::permissions::set(&pool, PERM_WINDOWS, true, "test").await.unwrap();

        // Normal mode: changes run, dangerous ones need confirmation.
        assert!(authorize(&pool, "volume.set", "50%", false).await.is_ok());
        assert!(matches!(authorize(&pool, "windows.close", "Блокнот", false).await, Err(AppError::ConfirmationRequired(_))));
        assert!(authorize(&pool, "windows.close", "Блокнот", true).await.is_ok());

        // Safe Mode: only whitelisted, and every change confirmed.
        set_safe_mode(&pool, &SafeMode { enabled: true, whitelist: vec!["windows.list".into(), "volume.set".into()] }).await.unwrap();
        assert!(authorize(&pool, "windows.list", "", false).await.is_ok());
        assert!(matches!(authorize(&pool, "windows.minimize", "x", true).await, Err(AppError::Blocked(_))));
        assert!(matches!(authorize(&pool, "volume.set", "50%", false).await, Err(AppError::ConfirmationRequired(_))));
        assert!(authorize(&pool, "volume.set", "50%", true).await.is_ok());
        assert!(allowed_in_safe_mode(&pool, "organizer.move", "").await.is_err());
        assert!(set_safe_mode(&pool, &SafeMode { enabled: true, whitelist: vec!["rm.rf".into()] }).await.is_err());

        let log = list(&pool, None, 10).await.unwrap();
        let results: Vec<&str> = log.iter().map(|l| l.result.as_str()).collect();
        assert_eq!(results, ["blocked", "blocked", "denied"]);

        let def = find("volume.set").unwrap();
        let id = record(&pool, def, "50%", "ok", "громкость 50%", Some(json!({ "level": 0.3 }))).await.unwrap();
        assert!(list(&pool, Some(WIN), 1).await.unwrap()[0].can_undo);
        assert_eq!(list(&pool, None, 1).await.unwrap()[0].id, id);

        let center = crate::permissions::center(&pool).await.unwrap();
        let settings = center.iter().find(|p| p.permission.id == PERM_SYSTEM_SETTINGS).unwrap();
        assert_eq!(settings.uses, 1);
        assert_eq!(settings.category, "system");
        assert!(settings.used_by.iter().any(|m| m.id == WIN));
        let access = crate::permissions::access_log(&pool, PERM_SYSTEM_SETTINGS, 10).await.unwrap();
        let kinds: Vec<&str> = access.iter().map(|a| a.kind.as_str()).collect();
        assert!(kinds.contains(&"used") && kinds.contains(&"granted") && kinds.contains(&"denied"));
    }
}
