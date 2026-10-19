//! Read-only overview data for the Dashboard and About pages.

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::error::AppResult;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub tauri_version: String,
    pub data_dir: String,
    pub log_dir: String,
    pub db_file: String,
}

#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub modules_enabled: i64,
    pub modules_total: i64,
    pub automations_total: i64,
    pub automations_enabled: i64,
    pub layouts: i64,
    pub active_layout: Option<String>,
    pub widgets_active: i64,
    pub tasks_open: i64,
    pub permissions_granted: i64,
    pub permissions_total: i64,
    pub hotkeys_registered: usize,
    pub autostart: bool,
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppResult<AppInfo> {
    let data_dir = app.path().app_data_dir().map_err(anyhow::Error::from)?;
    let log_dir = app.path().app_log_dir().map_err(anyhow::Error::from)?;
    Ok(AppInfo {
        name: app.package_info().name.clone(),
        version: app.package_info().version.to_string(),
        tauri_version: tauri::VERSION.to_string(),
        db_file: data_dir.join(crate::db::DB_FILE).display().to_string(),
        data_dir: data_dir.display().to_string(),
        log_dir: log_dir.display().to_string(),
    })
}

// sqlx only accepts static SQL strings here (no runtime-built queries).
async fn count(db: &sqlx::SqlitePool, sql: &'static str) -> AppResult<i64> {
    let (n,): (i64,) = sqlx::query_as(sql).fetch_one(db).await?;
    Ok(n)
}

#[tauri::command]
pub async fn dashboard_stats(app: AppHandle, state: State<'_, AppState>) -> AppResult<DashboardStats> {
    // Read the lock before any await: a held MutexGuard would make the future non-Send.
    let hotkeys_registered = state.hotkeys.lock().unwrap().len();
    let db = &state.db;
    let active_layout: Option<(String,)> = sqlx::query_as("SELECT name FROM widget_layouts WHERE is_active = 1 LIMIT 1")
        .fetch_optional(db)
        .await?;
    Ok(DashboardStats {
        modules_enabled: count(db, "SELECT COUNT(*) FROM modules WHERE enabled = 1").await?,
        modules_total: count(db, "SELECT COUNT(*) FROM modules").await?,
        automations_total: count(db, "SELECT COUNT(*) FROM automations").await?,
        automations_enabled: count(db, "SELECT COUNT(*) FROM automations WHERE enabled = 1").await?,
        layouts: count(db, "SELECT COUNT(*) FROM widget_layouts").await?,
        active_layout: active_layout.map(|r| r.0),
        widgets_active: count(
            db,
            "SELECT COUNT(*) FROM widgets WHERE layout_id = (SELECT id FROM widget_layouts WHERE is_active = 1 LIMIT 1)",
        )
        .await?,
        tasks_open: count(db, "SELECT COUNT(*) FROM tasks WHERE done = 0").await?,
        permissions_granted: count(db, "SELECT COUNT(*) FROM permissions WHERE granted = 1").await?,
        permissions_total: count(db, "SELECT COUNT(*) FROM permissions").await?,
        hotkeys_registered,
        autostart: crate::autostart::is_enabled(&app),
    })
}
