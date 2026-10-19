//! Connectors: integrations described by `connectors/*.json` manifests.
//! For now only manifests, settings, credentials and health checks exist;
//! actions are declared but not implemented.

pub mod manifest;
pub mod registry;
pub mod types;

use serde_json::Value;
use tauri::State;

use crate::ai::AiRouter;
use crate::modules::status::ModuleLogEntry;
pub use registry::ConnectorRegistry;
use manifest::LoadIssue;
use types::ConnectorInfo;

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
pub async fn connectors_list(registry: State<'_, ConnectorRegistry>) -> CmdResult<Vec<ConnectorInfo>> {
    registry.list().await.map_err(err)
}

#[tauri::command]
pub fn connectors_issues(registry: State<'_, ConnectorRegistry>) -> Vec<LoadIssue> {
    registry.issues()
}

#[tauri::command]
pub async fn connectors_reload(registry: State<'_, ConnectorRegistry>) -> CmdResult<Vec<ConnectorInfo>> {
    registry.reload().await.map_err(err)?;
    registry.list().await.map_err(err)
}

#[tauri::command]
pub async fn connectors_set_enabled(
    registry: State<'_, ConnectorRegistry>,
    id: String,
    enabled: bool,
) -> CmdResult<ConnectorInfo> {
    registry.set_enabled(&id, enabled).await.map_err(err)
}

#[tauri::command]
pub async fn connectors_update_settings(
    registry: State<'_, ConnectorRegistry>,
    id: String,
    settings: Value,
) -> CmdResult<ConnectorInfo> {
    registry.update_settings(&id, settings).await.map_err(err)
}

#[tauri::command]
pub async fn connectors_reset_settings(registry: State<'_, ConnectorRegistry>, id: String) -> CmdResult<ConnectorInfo> {
    registry.reset_settings(&id).await.map_err(err)
}

/// Stores a secret in the OS keyring. It is never returned to the frontend.
#[tauri::command]
pub async fn connectors_set_secret(
    registry: State<'_, ConnectorRegistry>,
    id: String,
    secret: String,
) -> CmdResult<ConnectorInfo> {
    registry.set_secret(&id, &secret).await.map_err(err)
}

#[tauri::command]
pub async fn connectors_delete_secret(registry: State<'_, ConnectorRegistry>, id: String) -> CmdResult<ConnectorInfo> {
    registry.delete_secret(&id).await.map_err(err)
}

#[tauri::command]
pub async fn connectors_test(
    registry: State<'_, ConnectorRegistry>,
    router: State<'_, AiRouter>,
    id: String,
) -> CmdResult<ConnectorInfo> {
    registry.test(&router, &id).await.map_err(err)
}

#[tauri::command]
pub fn connectors_logs(registry: State<'_, ConnectorRegistry>, id: String) -> CmdResult<Vec<ModuleLogEntry>> {
    registry.logs(&id).map_err(err)
}
