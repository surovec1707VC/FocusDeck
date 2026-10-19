//! Module Registry: every FocusDeck feature is a module that can be turned
//! on and off, configured and health-checked. See `docs/api.md`.

pub mod catalog;
pub mod config;
pub mod effects;
pub mod health;
pub mod registry;
pub mod schema;
pub mod types;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use catalog::{DATABASE, MODULE_REGISTRY};
use config::ImportReport;
use types::{ModuleInfo, ModuleLog, ModulePermission, ModuleStatus};

const SOURCE_USER: &str = "user";

fn require_enabled(state: &AppState, id: &str, name: &str) -> AppResult<()> {
    if state.module_enabled(id) {
        Ok(())
    } else {
        Err(AppError::ModuleDisabled(name.into()))
    }
}

async fn info(state: &AppState, id: &str) -> AppResult<ModuleInfo> {
    registry::get(&state.db, id, &state.pending_restart()).await
}

#[tauri::command]
pub async fn list_modules(state: State<'_, AppState>) -> AppResult<Vec<ModuleInfo>> {
    registry::list(&state.db, &state.pending_restart()).await
}

#[tauri::command]
pub async fn get_module(state: State<'_, AppState>, id: String) -> AppResult<ModuleInfo> {
    info(&state, &id).await
}

#[derive(Serialize)]
pub struct ToggleResult {
    pub module: ModuleInfo,
    /// Problems applying side effects (the module state did change).
    pub warnings: Vec<String>,
}

#[tauri::command]
pub async fn enable_module(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<ToggleResult> {
    registry::set_enabled(&state.db, &id, true, false, SOURCE_USER).await?;
    let warnings = effects::apply_enabled(&app, &id).await?;
    Ok(ToggleResult { module: info(&state, &id).await?, warnings })
}

/// Disabling a critical module returns `confirmation_required` unless `confirm` is true.
#[tauri::command]
pub async fn disable_module(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    confirm: Option<bool>,
) -> AppResult<ToggleResult> {
    registry::set_enabled(&state.db, &id, false, confirm.unwrap_or(false), SOURCE_USER).await?;
    let warnings = effects::apply_enabled(&app, &id).await?;
    Ok(ToggleResult { module: info(&state, &id).await?, warnings })
}

#[tauri::command]
pub async fn update_module_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    settings: Value,
) -> AppResult<ModuleInfo> {
    require_enabled(&state, MODULE_REGISTRY, "Реестр модулей")?;
    registry::update_settings(&state.db, &id, &settings, SOURCE_USER).await?;
    effects::apply_settings(&app, &id).await?;
    info(&state, &id).await
}

#[tauri::command]
pub async fn reset_module_settings(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<ModuleInfo> {
    require_enabled(&state, MODULE_REGISTRY, "Реестр модулей")?;
    registry::reset_settings(&state.db, &id, SOURCE_USER).await?;
    effects::apply_settings(&app, &id).await?;
    info(&state, &id).await
}

/// Runs the module's health check and logs the result.
#[tauri::command]
pub async fn get_module_status(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<ModuleStatus> {
    let (health, message) = health::check(&app, &id).await?;
    let level = match health {
        types::HealthState::Error => "error",
        types::HealthState::Degraded => "warn",
        _ => "info",
    };
    registry::log(&state.db, &id, level, "health_check", &message, SOURCE_USER).await?;
    Ok(ModuleStatus {
        module_id: id.clone(),
        enabled: registry::is_enabled(&state.db, &id).await?,
        state: health,
        message,
        pending_restart: state.pending_restart().contains(&id),
        checked_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
pub async fn get_module_logs(state: State<'_, AppState>, id: String, limit: Option<i64>) -> AppResult<Vec<ModuleLog>> {
    registry::logs(&state.db, &id, limit.unwrap_or(100)).await
}

#[tauri::command]
pub async fn get_module_permissions(state: State<'_, AppState>, id: String) -> AppResult<Vec<ModulePermission>> {
    registry::permissions(&state.db, &id).await
}

// --- configuration export / import ---

#[tauri::command]
pub async fn export_config(app: AppHandle, state: State<'_, AppState>) -> AppResult<String> {
    require_enabled(&state, DATABASE, "База данных")?;
    let file = config::export(&state.db, &state.settings(), &app.package_info().version.to_string()).await?;
    registry::log(&state.db, MODULE_REGISTRY, "info", "config_export", "конфигурация экспортирована", SOURCE_USER).await?;
    Ok(serde_json::to_string_pretty(&file)?)
}

/// Applies a configuration JSON. Critical modules turned off by the file
/// require `confirm` (otherwise `confirmation_required`).
#[tauri::command]
pub async fn import_config(app: AppHandle, state: State<'_, AppState>, json: String, confirm: Option<bool>) -> AppResult<ImportReport> {
    require_enabled(&state, DATABASE, "База данных")?;
    let file = config::parse(&json)?;
    let (mut report, new_settings) = config::apply(&state.db, &file, confirm.unwrap_or(false)).await?;

    if let Some(s) = new_settings {
        let old = state.settings();
        *state.settings.write().unwrap() = s.clone();
        crate::settings::notify_changed(&app);
        if s.autostart != old.autostart && state.module_enabled(catalog::AUTOSTART) {
            if let Err(e) = crate::autostart::set(&app, s.autostart) {
                report.warnings.push(e.to_string());
            }
        }
    }
    effects::refresh_cache(&app).await?;
    for id in report.changed_modules.clone() {
        report.warnings.extend(effects::apply_enabled(&app, &id).await?);
    }
    for id in report.changed_settings.clone() {
        effects::apply_settings(&app, &id).await?;
    }
    if let Err(e) = crate::hotkeys::sync(&app).await {
        report.warnings.push(e.to_string());
    }
    registry::log(
        &state.db,
        MODULE_REGISTRY,
        if report.skipped.is_empty() { "info" } else { "warn" },
        "config_import",
        &format!(
            "модулей: {}, разрешений: {}, пропущено: {}",
            report.modules_applied,
            report.permissions_applied,
            report.skipped.len()
        ),
        SOURCE_USER,
    )
    .await?;
    effects::notify_changed(&app);
    Ok(report)
}

/// Asks where to save and writes the exported configuration. `None` = cancelled.
#[tauri::command]
pub async fn export_config_to_file(app: AppHandle, state: State<'_, AppState>) -> AppResult<Option<String>> {
    let json = export_config(app.clone(), state).await?;
    let picked = app
        .dialog()
        .file()
        .set_title("Экспорт конфигурации FocusDeck")
        .set_file_name("focusdeck-config.json")
        .add_filter("Конфигурация FocusDeck", &["json"])
        .blocking_save_file();
    let Some(picked) = picked else { return Ok(None) };
    let path = picked.into_path().map_err(anyhow::Error::from)?;
    std::fs::write(&path, json).map_err(anyhow::Error::from)?;
    tracing::info!(path = %path.display(), "configuration exported");
    Ok(Some(path.display().to_string()))
}

/// Asks for a configuration file and returns its contents. `None` = cancelled.
#[tauri::command]
pub async fn read_config_file(app: AppHandle) -> AppResult<Option<String>> {
    let state = app.state::<AppState>();
    require_enabled(&state, DATABASE, "База данных")?;
    let picked = app
        .dialog()
        .file()
        .set_title("Импорт конфигурации FocusDeck")
        .add_filter("Конфигурация FocusDeck", &["json"])
        .blocking_pick_file();
    let Some(picked) = picked else { return Ok(None) };
    let path = picked.into_path().map_err(anyhow::Error::from)?;
    let meta = std::fs::metadata(&path).map_err(anyhow::Error::from)?;
    if meta.len() > 1024 * 1024 {
        return Err(AppError::Invalid("файл больше 1 МБ — это не конфигурация FocusDeck".into()));
    }
    Ok(Some(std::fs::read_to_string(&path).map_err(anyhow::Error::from)?))
}
