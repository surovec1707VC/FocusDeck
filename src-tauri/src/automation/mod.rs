//! Automations: «trigger → actions» rules that only act inside FocusDeck.

pub mod engine;
pub mod store;
pub mod types;

use tauri::{AppHandle, State};

use crate::error::AppResult;
use crate::state::AppState;
use store::AutomationLog;
use types::{Automation, AutomationInput, Trigger};

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
pub async fn automations_create(
    app: AppHandle,
    state: State<'_, AppState>,
    input: AutomationInput,
) -> AppResult<Automation> {
    let a = store::create(&state.db, &input).await?;
    resync_if_hotkey(&app, &a.trigger).await?;
    Ok(a)
}

#[tauri::command]
pub async fn automations_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    input: AutomationInput,
) -> AppResult<Automation> {
    let before = store::get(&state.db, &id).await?;
    let a = store::update(&state.db, &id, &input).await?;
    resync_if_hotkey(&app, &before.trigger).await?;
    resync_if_hotkey(&app, &a.trigger).await?;
    Ok(a)
}

#[tauri::command]
pub async fn automations_set_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> AppResult<Automation> {
    let a = store::set_enabled(&state.db, &id, enabled).await?;
    resync_if_hotkey(&app, &a.trigger).await?;
    Ok(a)
}

#[tauri::command]
pub async fn automations_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    let a = store::get(&state.db, &id).await?;
    store::delete(&state.db, &id).await?;
    resync_if_hotkey(&app, &a.trigger).await
}

#[tauri::command]
pub async fn automations_run_now(app: AppHandle, id: String) -> AppResult<()> {
    engine::run_by_id(&app, &id).await
}

#[tauri::command]
pub async fn automations_logs(
    state: State<'_, AppState>,
    id: Option<String>,
    limit: Option<i64>,
) -> AppResult<Vec<AutomationLog>> {
    store::logs(&state.db, id.as_deref(), limit.unwrap_or(50)).await
}
