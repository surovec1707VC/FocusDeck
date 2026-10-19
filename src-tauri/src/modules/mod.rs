//! App modules: features that can be switched on and off on the «Модули» page.

pub mod registry;
pub mod types;

use serde_json::Value;
use tauri::{AppHandle, State};

use crate::error::AppResult;
use crate::state::AppState;
use types::ModuleInfo;

#[tauri::command]
pub async fn modules_list(state: State<'_, AppState>) -> AppResult<Vec<ModuleInfo>> {
    registry::list(&state.db).await
}

#[tauri::command]
pub async fn modules_set_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> AppResult<Vec<ModuleInfo>> {
    registry::set_enabled(&state.db, &id, enabled).await?;
    // Hotkeys depend on the hotkeys and automations modules.
    if id == "hotkeys" || id == registry::MODULE_AUTOMATIONS {
        crate::hotkeys::sync(&app).await?;
    }
    registry::list(&state.db).await
}

#[tauri::command]
pub async fn module_settings_get(state: State<'_, AppState>, module_id: String, key: String) -> AppResult<Option<Value>> {
    registry::get_setting(&state.db, &module_id, &key).await
}

#[tauri::command]
pub async fn module_settings_set(state: State<'_, AppState>, module_id: String, key: String, value: Value) -> AppResult<()> {
    registry::set_setting(&state.db, &module_id, &key, &value).await
}
