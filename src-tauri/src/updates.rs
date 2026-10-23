//! Updates through tauri-plugin-updater, under the «Обновления» module and the
//! `network.updates` permission (both off by default — no network otherwise).
//!
//! - The endpoint and the minisign public key come from the build environment
//!   (`FOCUSDECK_UPDATE_ENDPOINT`, `FOCUSDECK_UPDATER_PUBKEY`). A build without
//!   them reports «обновления не настроены» and never connects anywhere.
//! - Every downloaded package is verified against that public key by the plugin.
//! - Before installing, the database is copied to `backups/pre-update-<version>.sqlite`
//!   and the previous version is remembered: that is the rollback point.
//! - Rollback: the backup is staged as a restore (`session`) and the user
//!   reinstalls the previous version from the releases page. Migrations only go
//!   forward, so the old version needs the old database.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

use crate::error::{AppError, AppResult};
use crate::modules::catalog::UPDATES;
use crate::modules::registry;
use crate::permissions::{self, PERM_NETWORK_UPDATES};
use crate::state::AppState;

pub const ENDPOINT: Option<&str> = option_env!("FOCUSDECK_UPDATE_ENDPOINT");
pub const PUBKEY: Option<&str> = option_env!("FOCUSDECK_UPDATER_PUBKEY");
pub const RELEASES_URL: Option<&str> = option_env!("FOCUSDECK_RELEASES_URL");
const PREVIOUS_KEY: &str = "updates.previous";

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub configured: bool,
    pub available: Option<Available>,
    pub previous: Option<Previous>,
    pub releases_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Available {
    pub version: String,
    pub date: Option<String>,
    pub notes: Option<String>,
}

/// What a rollback goes back to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Previous {
    pub version: String,
    pub backup: String,
    pub updated_at: String,
}

pub fn configured() -> bool {
    ENDPOINT.is_some_and(|e| e.starts_with("https://")) && PUBKEY.is_some_and(|k| !k.trim().is_empty())
}

/// Semantic version comparison (`1.10.0` > `1.9.3`); pre-release tags sort before the release.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    fn parse(v: &str) -> (Vec<u64>, bool) {
        let v = v.trim().trim_start_matches('v');
        let (core, pre) = v.split_once('-').map_or((v, false), |(c, _)| (c, true));
        (core.split('.').map(|p| p.parse().unwrap_or(0)).collect(), pre)
    }
    let (a, a_pre) = parse(candidate);
    let (b, b_pre) = parse(current);
    match a.cmp(&b) {
        std::cmp::Ordering::Equal => !a_pre && b_pre,
        o => o.is_gt(),
    }
}

async fn previous(pool: &SqlitePool) -> Option<Previous> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value_json FROM settings WHERE key = ?").bind(PREVIOUS_KEY).fetch_optional(pool).await.ok()?;
    row.and_then(|r| serde_json::from_str(&r.0).ok())
}

async fn require(app: &AppHandle) -> AppResult<()> {
    crate::productivity::require(&app.state::<AppState>(), UPDATES)?;
    permissions::require(&app.state::<AppState>().db, PERM_NETWORK_UPDATES, "updates").await?;
    if !configured() {
        return Err(AppError::Invalid("обновления не настроены в этой сборке (нет адреса или ключа подписи)".into()));
    }
    Ok(())
}

fn updater(app: &AppHandle) -> AppResult<tauri_plugin_updater::Updater> {
    let endpoint = ENDPOINT.unwrap_or_default().parse().map_err(|e| AppError::Invalid(format!("адрес обновлений: {e}")))?;
    app.updater_builder()
        .endpoints(vec![endpoint])
        .map_err(anyhow::Error::from)?
        .pubkey(PUBKEY.unwrap_or_default())
        .build()
        .map_err(|e| AppError::Other(anyhow::anyhow!("updater: {e}")))
}

async fn info(app: &AppHandle, available: Option<Available>) -> UpdateInfo {
    UpdateInfo {
        current: app.package_info().version.to_string(),
        configured: configured(),
        available,
        previous: previous(&app.state::<AppState>().db).await,
        releases_url: RELEASES_URL.map(String::from),
    }
}

/// Module state, version and rollback point; never touches the network.
#[tauri::command]
pub async fn updates_status(app: AppHandle) -> AppResult<UpdateInfo> {
    crate::productivity::require(&app.state::<AppState>(), UPDATES)?;
    Ok(info(&app, None).await)
}

#[tauri::command]
pub async fn updates_check(app: AppHandle) -> AppResult<UpdateInfo> {
    require(&app).await?;
    let found = updater(&app)?.check().await.map_err(|e| AppError::Other(anyhow::anyhow!("проверка обновлений: {e}")))?;
    let current = app.package_info().version.to_string();
    let available = found.filter(|u| is_newer(&u.version, &current)).map(|u| Available { version: u.version.clone(), date: u.date.map(|d| d.to_string()), notes: u.body.clone() });
    registry::log(&app.state::<AppState>().db, UPDATES, "info", "check", &available.as_ref().map_or("обновлений нет".into(), |a| format!("доступна версия {}", a.version)), "user").await?;
    Ok(info(&app, available).await)
}

/// Backs up the database, downloads, verifies and installs the update, then restarts.
#[tauri::command]
pub async fn updates_install(app: AppHandle, confirm: bool) -> AppResult<()> {
    require(&app).await?;
    let Some(update) = updater(&app)?.check().await.map_err(|e| AppError::Other(anyhow::anyhow!("проверка обновлений: {e}")))? else {
        return Err(AppError::NotFound("обновление".into()));
    };
    let current = app.package_info().version.to_string();
    if !confirm {
        return Err(AppError::ConfirmationRequired(format!(
            "Установить FocusDeck {} вместо {current}? Перед установкой база данных будет скопирована — к ней можно вернуться откатом. Приложение перезапустится.",
            update.version
        )));
    }
    let state = app.state::<AppState>();
    let dir = crate::paths::data_dir(&app)?.join("backups");
    std::fs::create_dir_all(&dir).map_err(anyhow::Error::from)?;
    let backup = dir.join(format!("pre-update-{current}.sqlite"));
    let tmp = backup.with_extension("sqlite.tmp");
    let _ = std::fs::remove_file(&tmp);
    sqlx::query("VACUUM INTO ?").bind(tmp.display().to_string()).execute(&state.db).await?;
    std::fs::rename(&tmp, &backup).map_err(anyhow::Error::from)?;
    let prev = Previous { version: current.clone(), backup: backup.display().to_string(), updated_at: chrono::Local::now().to_rfc3339() };
    sqlx::query("INSERT INTO settings (key, value_json) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = datetime('now')")
        .bind(PREVIOUS_KEY)
        .bind(serde_json::to_string(&prev)?)
        .execute(&state.db)
        .await?;
    registry::log(&state.db, UPDATES, "info", "install", &format!("{current} → {}, копия базы: {}", update.version, prev.backup), "user").await?;
    update
        .download_and_install(|_, _| {}, || tracing::info!("update downloaded"))
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("установка обновления: {e}")))?;
    crate::session::app_restart(app);
    Ok(())
}

/// Stages the pre-update database copy for restore; the previous version is then installed by hand.
#[tauri::command]
pub async fn updates_rollback(app: AppHandle, confirm: bool) -> AppResult<Previous> {
    crate::productivity::require(&app.state::<AppState>(), UPDATES)?;
    let state = app.state::<AppState>();
    let Some(prev) = previous(&state.db).await else {
        return Err(AppError::NotFound("точка отката (обновлений через FocusDeck ещё не было)".into()));
    };
    if !confirm {
        return Err(AppError::ConfirmationRequired(format!(
            "Вернуть данные к состоянию перед обновлением с версии {}? Изменения после обновления будут потеряны (текущая база сохранится рядом). Затем установите версию {} со страницы релизов.",
            prev.version, prev.version
        )));
    }
    crate::session::stage_restore(&app, std::path::Path::new(&prev.backup)).await?;
    registry::log(&state.db, UPDATES, "warn", "rollback", &format!("подготовлен откат данных к версии {}", prev.version), "user").await?;
    Ok(prev)
}

/// Optional check at start (module setting `auto_check`); a found update becomes a notification.
pub async fn startup_check(app: AppHandle) {
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    let db = app.state::<AppState>().db.clone();
    let auto = registry::settings(&db, UPDATES).await.ok().and_then(|s| s.get("auto_check").and_then(Value::as_bool)).unwrap_or(false);
    if !auto || require(&app).await.is_err() {
        return;
    }
    match updates_check(app.clone()).await {
        Ok(UpdateInfo { available: Some(a), .. }) => {
            let _ = crate::notifications::push(&app, "info", "Доступно обновление", &format!("FocusDeck {} — откройте «Обновления»", a.version), "updates").await;
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("update check: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_order() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("1.10.0", "1.9.3"));
        assert!(is_newer("v1.0.0", "1.0.0-beta.2"));
        assert!(!is_newer("1.0.0-beta.2", "1.0.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn unconfigured_build_has_no_endpoint() {
        // Test builds carry no update keys: nothing can reach the network.
        if ENDPOINT.is_none() || PUBKEY.is_none() {
            assert!(!configured());
        }
    }
}
