//! Automations: trigger → conditions → actions. See `docs/automations.md`
//! and the JSON schema `docs/automation.schema.json`.

pub mod engine;
pub mod store;
pub mod system;
pub mod templates;
pub mod types;

use serde_json::Value;
use tauri::{AppHandle, State};

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use engine::{PendingRun, RunOptions, RunReport};
use store::AutomationLog;
use types::{Automation, AutomationInput, Trigger};

pub const SCHEMA: &str = include_str!("../../../docs/automation.schema.json");

/// Re-registers hotkeys when a hotkey-triggered automation changes.
async fn resync_if_hotkey(app: &AppHandle, trigger: &Trigger) -> AppResult<()> {
    if matches!(trigger, Trigger::Hotkey { .. }) {
        crate::hotkeys::sync(app).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn automations_list(state: State<'_, AppState>) -> AppResult<Vec<Automation>> {
    store::list(&state.db).await
}

#[tauri::command]
pub async fn automations_create(app: AppHandle, state: State<'_, AppState>, input: AutomationInput) -> AppResult<Automation> {
    let a = store::create(&state.db, &input).await?;
    resync_if_hotkey(&app, &a.trigger).await?;
    Ok(a)
}

#[tauri::command]
pub async fn automations_update(app: AppHandle, state: State<'_, AppState>, id: String, input: AutomationInput) -> AppResult<Automation> {
    let before = store::get(&state.db, &id).await?;
    let a = store::update(&state.db, &id, &input).await?;
    resync_if_hotkey(&app, &before.trigger).await?;
    resync_if_hotkey(&app, &a.trigger).await?;
    Ok(a)
}

#[tauri::command]
pub async fn automations_set_enabled(app: AppHandle, state: State<'_, AppState>, id: String, enabled: bool) -> AppResult<Automation> {
    let a = store::set_enabled(&state.db, &id, enabled).await?;
    resync_if_hotkey(&app, &a.trigger).await?;
    Ok(a)
}

#[tauri::command]
pub async fn automations_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    let a = store::get(&state.db, &id).await?;
    store::delete(&state.db, &id).await?;
    resync_if_hotkey(&app, &a.trigger).await
}

/// New order of the list (drag and drop).
#[tauri::command]
pub async fn automations_reorder(state: State<'_, AppState>, ids: Vec<String>) -> AppResult<Vec<Automation>> {
    store::reorder(&state.db, &ids).await?;
    store::list(&state.db).await
}

/// Runs now from the UI. `source`: `manual` (default) or `ui_button`.
/// Dangerous actions need `confirm: true` (`confirmation_required` otherwise).
#[tauri::command]
pub async fn automations_run(app: AppHandle, state: State<'_, AppState>, id: String, confirm: Option<bool>, source: Option<String>) -> AppResult<RunReport> {
    let a = store::get(&state.db, &id).await?;
    let source = match source.as_deref() {
        Some("ui_button") => "ui_button",
        _ => "manual",
    };
    let opts = RunOptions { ignore_cooldown: source == "manual", confirmed: confirm.unwrap_or(false), dry_run: false };
    engine::run(&app, &a, source, opts).await
}

/// Test of the editor's current (maybe unsaved) state: modules, conditions,
/// permissions and the plan. With `execute: true` the actions run too.
#[tauri::command]
pub async fn automations_test(app: AppHandle, state: State<'_, AppState>, id: Option<String>, input: AutomationInput, execute: Option<bool>, confirm: Option<bool>) -> AppResult<RunReport> {
    input.validate()?;
    let (permissions, dangerous) = types::requirements(&input.actions);
    let saved = match &id {
        Some(id) => Some(store::get(&state.db, id).await?),
        None => None,
    };
    let a = Automation {
        id: id.clone().unwrap_or_else(|| "test".into()),
        name: input.name.clone(),
        description: input.description.clone(),
        enabled: true,
        module_id: input.module_id().to_string(),
        trigger: input.trigger.clone(),
        conditions: input.conditions.clone(),
        actions: input.actions.clone(),
        cooldown_secs: input.cooldown_secs,
        template: None,
        permissions,
        dangerous,
        last_run_at: saved.as_ref().and_then(|s| s.last_run_at.clone()),
        created_at: String::new(),
        updated_at: String::new(),
    };
    let execute = execute.unwrap_or(false);
    if execute && id.is_none() {
        return Err(AppError::Invalid("сохраните автоматизацию перед тестовым запуском".into()));
    }
    let opts = RunOptions { ignore_cooldown: true, confirmed: confirm.unwrap_or(false), dry_run: !execute };
    engine::run(&app, &a, "test", opts).await
}

#[tauri::command]
pub async fn automations_logs(state: State<'_, AppState>, id: Option<String>, limit: Option<i64>) -> AppResult<Vec<AutomationLog>> {
    store::logs(&state.db, id.as_deref(), limit.unwrap_or(50)).await
}

/// Dangerous automatic runs waiting for the user.
#[tauri::command]
pub fn automations_pending() -> Vec<PendingRun> {
    engine::pending()
}

#[tauri::command]
pub async fn automations_confirm(app: AppHandle, token: String, approve: bool) -> AppResult<Option<RunReport>> {
    engine::resolve_pending(&app, &token, approve).await
}

#[tauri::command]
pub fn automations_templates() -> Vec<templates::Template> {
    templates::all()
}

#[tauri::command]
pub async fn automations_install_template(app: AppHandle, state: State<'_, AppState>, key: String) -> AppResult<Automation> {
    let (_, input) = templates::find(&key).ok_or_else(|| AppError::NotFound(format!("сценарий {key}")))?;
    let a = store::create_from(&state.db, &input, Some(&key)).await?;
    resync_if_hotkey(&app, &a.trigger).await?;
    Ok(a)
}

/// File/folder dialog for the editor; nothing is opened or run.
#[tauri::command]
pub async fn automations_pick(app: AppHandle, kind: String) -> AppResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let d = app.dialog().file();
        match kind.as_str() {
            "app" => d.set_title("Выберите программу").add_filter("Программы и ярлыки", crate::widgets::APP_EXTENSIONS).blocking_pick_file(),
            "folder" => d.set_title("Выберите папку").blocking_pick_folder(),
            _ => d.set_title("Выберите файл").blocking_pick_file(),
        }
    })
    .await
    .map_err(anyhow::Error::from)?;
    picked.map(|p| p.into_path().map(|p| p.display().to_string()).map_err(|e| AppError::Other(e.into()))).transpose()
}

/// JSON Schema of an automation (draft 2020-12).
#[tauri::command]
pub fn automations_schema() -> AppResult<Value> {
    Ok(serde_json::from_str(SCHEMA)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_is_valid_json_and_lists_every_kind() {
        let schema: Value = serde_json::from_str(SCHEMA).unwrap();
        let text = schema.to_string();
        for kind in [
            "app_start", "system_start", "process_started", "process_stopped", "window_focus", "hotkey", "time", "interval",
            "display_change", "network_change", "manual", "ui_button", "process_running", "active_app", "weekday", "time_range",
            "module_enabled", "active_layout", "internet", "open_app", "close_app", "open_url", "open_folder", "open_file",
            "run_command", "switch_layout", "set_module", "notify", "copy_text", "paste_text", "screenshot", "start_timer",
            "focus_mode", "delete_files",
        ] {
            assert!(text.contains(&format!("\"{kind}\"")), "{kind} missing in schema");
        }
        // Templates validate against the Rust model (the schema mirrors it).
        for t in templates::all() {
            serde_json::from_value::<AutomationInput>(t.input).unwrap();
        }
    }
}
