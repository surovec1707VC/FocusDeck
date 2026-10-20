//! Integrations: connectors described by manifests (`manifests/*.json`), a
//! registry of their state in SQLite, secrets in Windows Credential Manager,
//! stub health checks and a per-connector log.
//!
//! Stage 1 runs no connector actions and calls no external API. Cloud
//! connectors are paused while Offline Mode (on by default) is on; enabling
//! one needs Offline Mode off and an explicit confirmation. Everything is
//! behind the «Интеграции» module.

pub mod health;
pub mod manifest;
pub mod secrets;
pub mod store;

use serde::Serialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::modules::catalog::INTEGRATIONS;
use crate::modules::registry;
use crate::state::AppState;
use manifest::{Kind, Manifest};

#[derive(Debug, Clone, Serialize)]
pub struct ConnectorInfo {
    pub manifest: &'static Manifest,
    pub enabled: bool,
    pub granted: Vec<String>,
    pub has_secret: bool,
    pub settings: Value,
    pub health_status: String,
    pub health_message: String,
    pub health_checked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Overview {
    pub offline_mode: bool,
    pub connectors: Vec<ConnectorInfo>,
}

fn require(app: &AppHandle) -> AppResult<()> {
    crate::productivity::require(&app.state::<AppState>(), INTEGRATIONS)
}

fn db(app: &AppHandle) -> SqlitePool {
    app.state::<AppState>().db.clone()
}

pub async fn offline_mode(pool: &SqlitePool) -> bool {
    registry::settings(pool, INTEGRATIONS).await.ok().and_then(|s| s.get("offline_mode").and_then(Value::as_bool)).unwrap_or(true)
}

async fn log_limit(pool: &SqlitePool) -> i64 {
    registry::settings(pool, INTEGRATIONS).await.ok().and_then(|s| s.get("log_limit").and_then(Value::as_i64)).unwrap_or(store::DEFAULT_LOG_LIMIT)
}

async fn log(pool: &SqlitePool, id: &str, level: &str, action: &str, message: &str) -> AppResult<()> {
    store::log(pool, id, level, action, message, log_limit(pool).await).await
}

async fn info(pool: &SqlitePool, m: &'static Manifest) -> AppResult<ConnectorInfo> {
    let row = store::row(pool, &m.id).await?;
    Ok(ConnectorInfo {
        manifest: m,
        enabled: row.enabled,
        granted: store::granted(&row),
        has_secret: m.auth.kind != manifest::AuthType::None && secrets::exists(&m.id),
        settings: store::settings(pool, m).await?,
        health_status: row.health_status,
        health_message: row.health_message,
        health_checked_at: row.health_checked_at,
    })
}

/// Runs the health check, stores and logs the result.
async fn run_health(pool: &SqlitePool, m: &'static Manifest) -> AppResult<ConnectorInfo> {
    let c = info(pool, m).await?;
    let result = health::check(health::Input { manifest: m, settings: &c.settings, enabled: c.enabled, offline: offline_mode(pool).await, has_secret: c.has_secret }).await;
    store::set_health(pool, &m.id, result.status.as_str(), &result.message).await?;
    let level = match result.status {
        health::Status::Error => "error",
        health::Status::Warning | health::Status::NotConfigured => "warn",
        _ => "info",
    };
    log(pool, &m.id, level, "health", &format!("проверка: {}", result.message)).await?;
    info(pool, m).await
}

pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool, confirm: bool) -> AppResult<()> {
    let m = manifest::get(id)?;
    if enabled && m.kind == Kind::Cloud {
        if offline_mode(pool).await {
            return Err(AppError::Offline(format!("Включён Offline Mode — облачную интеграцию «{}» нельзя включить", m.name)));
        }
        if !confirm {
            return Err(AppError::ConfirmationRequired(format!(
                "«{}» — облачная интеграция: после включения FocusDeck сможет обращаться к {} с вашим токеном. Включить?",
                m.name, m.name
            )));
        }
    }
    store::set_enabled(pool, id, enabled).await?;
    log(pool, id, "info", if enabled { "enable" } else { "disable" }, if enabled { "интеграция включена" } else { "интеграция выключена" }).await
}

/// Turning Offline Mode on pauses every enabled cloud connector.
pub async fn set_offline(pool: &SqlitePool, on: bool) -> AppResult<()> {
    // registry::update_settings replaces the whole object: keep the other keys.
    let mut current = registry::settings(pool, INTEGRATIONS).await?;
    current["offline_mode"] = json!(on);
    registry::update_settings(pool, INTEGRATIONS, &current, "integrations").await?;
    for m in manifest::all().iter().filter(|m| m.kind == Kind::Cloud) {
        if store::row(pool, &m.id).await?.enabled {
            if on {
                store::set_health(pool, &m.id, health::Status::Offline.as_str(), "Offline Mode: облачные интеграции приостановлены").await?;
            }
            log(pool, &m.id, "info", "offline", if on { "Offline Mode включён: интеграция приостановлена" } else { "Offline Mode выключен" }).await?;
        }
    }
    Ok(())
}

pub async fn seed(pool: &SqlitePool) -> AppResult<()> {
    store::seed(pool).await
}

// --- commands ---

#[tauri::command]
pub async fn integrations_list(app: AppHandle) -> AppResult<Overview> {
    require(&app)?;
    let pool = db(&app);
    let mut connectors = Vec::new();
    for m in manifest::all() {
        connectors.push(info(&pool, m).await?);
    }
    Ok(Overview { offline_mode: offline_mode(&pool).await, connectors })
}

#[tauri::command]
pub async fn integrations_get(app: AppHandle, id: String) -> AppResult<ConnectorInfo> {
    require(&app)?;
    info(&db(&app), manifest::get(&id)?).await
}

#[tauri::command]
pub async fn integrations_set_enabled(app: AppHandle, id: String, enabled: bool, confirm: Option<bool>) -> AppResult<ConnectorInfo> {
    require(&app)?;
    let pool = db(&app);
    set_enabled(&pool, &id, enabled, confirm.unwrap_or(false)).await?;
    run_health(&pool, manifest::get(&id)?).await
}

#[tauri::command]
pub async fn integrations_set_offline(app: AppHandle, enabled: bool) -> AppResult<bool> {
    require(&app)?;
    let pool = db(&app);
    set_offline(&pool, enabled).await?;
    Ok(offline_mode(&pool).await)
}

#[tauri::command]
pub async fn integrations_update_settings(app: AppHandle, id: String, settings: Value) -> AppResult<ConnectorInfo> {
    require(&app)?;
    let pool = db(&app);
    let m = manifest::get(&id)?;
    store::update_settings(&pool, m, &settings).await?;
    let keys: Vec<&str> = settings.as_object().map(|o| o.keys().map(String::as_str).collect()).unwrap_or_default();
    log(&pool, &id, "info", "settings", &format!("настройки изменены: {}", keys.join(", "))).await?;
    run_health(&pool, m).await
}

#[tauri::command]
pub async fn integrations_reset_settings(app: AppHandle, id: String) -> AppResult<ConnectorInfo> {
    require(&app)?;
    let pool = db(&app);
    let m = manifest::get(&id)?;
    store::reset_settings(&pool, m).await?;
    log(&pool, &id, "info", "settings", "настройки сброшены").await?;
    run_health(&pool, m).await
}

#[tauri::command]
pub async fn integrations_set_permission(app: AppHandle, id: String, permission: String, granted: bool) -> AppResult<ConnectorInfo> {
    require(&app)?;
    let pool = db(&app);
    let m = manifest::get(&id)?;
    store::set_permission(&pool, m, &permission, granted).await?;
    let name = m.permission(&permission).map(|p| p.name.as_str()).unwrap_or(&permission);
    log(&pool, &id, "info", "permission", &format!("{} разрешение «{name}»", if granted { "выдано" } else { "отозвано" })).await?;
    info(&pool, m).await
}

/// Saves the secret to Credential Manager; it never comes back to the UI.
#[tauri::command]
pub async fn integrations_set_secret(app: AppHandle, id: String, secret: String) -> AppResult<ConnectorInfo> {
    require(&app)?;
    let pool = db(&app);
    let m = manifest::get(&id)?;
    if m.auth.kind == manifest::AuthType::None {
        return Err(AppError::Invalid(format!("интеграции «{}» секрет не нужен", m.name)));
    }
    secrets::save(&id, &secret)?;
    log(&pool, &id, "info", "secret", "секрет сохранён в Windows Credential Manager").await?;
    run_health(&pool, m).await
}

#[tauri::command]
pub async fn integrations_clear_secret(app: AppHandle, id: String) -> AppResult<ConnectorInfo> {
    require(&app)?;
    let pool = db(&app);
    let m = manifest::get(&id)?;
    secrets::remove(&id)?;
    log(&pool, &id, "info", "secret", "секрет удалён из Credential Manager").await?;
    run_health(&pool, m).await
}

#[tauri::command]
pub async fn integrations_check(app: AppHandle, id: Option<String>) -> AppResult<Vec<ConnectorInfo>> {
    require(&app)?;
    let pool = db(&app);
    let targets: Vec<&'static Manifest> = match id {
        Some(id) => vec![manifest::get(&id)?],
        None => manifest::all().iter().collect(),
    };
    let mut out = Vec::new();
    for m in targets {
        out.push(run_health(&pool, m).await?);
    }
    Ok(out)
}

#[tauri::command]
pub async fn integrations_logs(app: AppHandle, id: Option<String>, limit: Option<i64>) -> AppResult<Vec<store::LogEntry>> {
    require(&app)?;
    if let Some(id) = &id {
        manifest::get(id)?;
    }
    store::logs(&db(&app), id.as_deref(), limit.unwrap_or(200)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn offline_mode_guards_cloud_connectors() {
        let pool = crate::db::test_pool().await;
        registry::seed(&pool).await.unwrap();
        seed(&pool).await.unwrap();
        assert!(offline_mode(&pool).await, "Offline Mode is on by default");

        // Local connectors: no confirmation, Offline Mode doesn't matter.
        set_enabled(&pool, "vscode", true, false).await.unwrap();
        assert!(store::row(&pool, "vscode").await.unwrap().enabled);

        // Cloud: blocked offline, then needs confirmation.
        assert!(matches!(set_enabled(&pool, "github", true, true).await, Err(AppError::Offline(_))));
        set_offline(&pool, false).await.unwrap();
        assert!(!offline_mode(&pool).await);
        assert!(matches!(set_enabled(&pool, "github", true, false).await, Err(AppError::ConfirmationRequired(_))));
        set_enabled(&pool, "github", true, true).await.unwrap();

        // Back offline: the enabled cloud connector is paused, not turned off.
        set_offline(&pool, true).await.unwrap();
        let row = store::row(&pool, "github").await.unwrap();
        assert!(row.enabled);
        assert_eq!(row.health_status, "offline");
        // Turning off always works.
        set_enabled(&pool, "github", false, false).await.unwrap();
        let actions: Vec<String> = store::logs(&pool, Some("github"), 10).await.unwrap().into_iter().map(|l| l.action).collect();
        assert_eq!(actions, ["disable", "offline", "enable"]);
    }
}
