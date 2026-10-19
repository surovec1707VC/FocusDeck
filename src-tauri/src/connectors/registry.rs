use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use std::time::Duration;

use serde_json::Value;
use sqlx::SqlitePool;

use super::manifest::{self, LoadIssue};
use super::types::{ConnectorInfo, ConnectorManifest, ConnectorStatus, HealthCheck};
use crate::ai::AiRouter;
use crate::modules::settings;
use crate::modules::status::{LogBuffer, LogLevel, ModuleLogEntry};
use crate::secrets::SecretStore;

struct Loaded {
    manifest: ConnectorManifest,
}

pub struct ConnectorRegistry {
    dir: PathBuf,
    pool: SqlitePool,
    secrets: SecretStore,
    http: reqwest::Client,
    loaded: RwLock<(Vec<Loaded>, Vec<LoadIssue>)>,
    logs: Mutex<HashMap<String, LogBuffer>>,
}

impl ConnectorRegistry {
    pub fn new(dir: PathBuf, pool: SqlitePool, secrets: SecretStore) -> anyhow::Result<Self> {
        Ok(Self {
            dir,
            pool,
            secrets,
            http: reqwest::Client::builder().timeout(Duration::from_secs(5)).build()?,
            loaded: RwLock::new((Vec::new(), Vec::new())),
            logs: Mutex::new(HashMap::new()),
        })
    }

    /// Reads manifests from disk and syncs metadata into the `connectors` table.
    /// Enabled state and settings already in the database are kept.
    pub async fn reload(&self) -> anyhow::Result<()> {
        let (loaded, issues) = manifest::load_dir(&self.dir);
        for l in &loaded {
            let m = &l.manifest;
            sqlx::query(
                "INSERT INTO connectors (id, name, category, auth_type, enabled, settings_json)
                 VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    category = excluded.category,
                    auth_type = excluded.auth_type,
                    updated_at = datetime('now')",
            )
            .bind(&m.id)
            .bind(&m.name)
            .bind(&m.category)
            .bind(m.auth_type.as_str())
            .bind(m.enabled)
            .bind(serde_json::to_string(&settings::defaults(&m.settings_schema))?)
            .execute(&self.pool)
            .await?;
        }
        for issue in &issues {
            tauri_plugin_log::log::warn!("connector {} not loaded: {:?}", issue.file, issue.errors);
        }
        let loaded = loaded.into_iter().map(|l| Loaded { manifest: l.manifest }).collect();
        *self.loaded.write().unwrap() = (loaded, issues);
        Ok(())
    }

    pub fn issues(&self) -> Vec<LoadIssue> {
        self.loaded.read().unwrap().1.clone()
    }

    fn manifest(&self, id: &str) -> anyhow::Result<ConnectorManifest> {
        self.loaded
            .read()
            .unwrap()
            .0
            .iter()
            .find(|l| l.manifest.id == id)
            .map(|l| l.manifest.clone())
            .ok_or_else(|| anyhow::anyhow!("connector `{id}` not found"))
    }

    async fn info(&self, m: ConnectorManifest) -> anyhow::Result<ConnectorInfo> {
        let (enabled, settings_json, last_status): (bool, String, Option<String>) =
            sqlx::query_as("SELECT enabled, settings_json, last_status FROM connectors WHERE id = ?")
                .bind(&m.id)
                .fetch_one(&self.pool)
                .await?;
        let stored: Value = serde_json::from_str(&settings_json)?;
        let status = match last_status.as_deref() {
            None => ConnectorStatus::Unknown,
            Some("ok") => ConnectorStatus::Ok,
            Some(other) => ConnectorStatus::Error(other.trim_start_matches("error: ").to_string()),
        };
        let has_secret = m.auth_type.uses_secret() && self.secrets.has(&m.secret_key()).unwrap_or(false);
        Ok(ConnectorInfo {
            settings: settings::merge_with_defaults(&m.settings_schema, &stored),
            manifest: m,
            enabled,
            has_secret,
            status,
        })
    }

    pub async fn list(&self) -> anyhow::Result<Vec<ConnectorInfo>> {
        let manifests: Vec<ConnectorManifest> =
            self.loaded.read().unwrap().0.iter().map(|l| l.manifest.clone()).collect();
        let mut out = Vec::with_capacity(manifests.len());
        for m in manifests {
            out.push(self.info(m).await?);
        }
        Ok(out)
    }

    pub async fn get(&self, id: &str) -> anyhow::Result<ConnectorInfo> {
        self.info(self.manifest(id)?).await
    }

    pub async fn set_enabled(&self, id: &str, enabled: bool) -> anyhow::Result<ConnectorInfo> {
        self.manifest(id)?;
        sqlx::query("UPDATE connectors SET enabled = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(enabled)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.log(id, LogLevel::Info, if enabled { "enabled" } else { "disabled" });
        self.get(id).await
    }

    pub async fn update_settings(&self, id: &str, new_settings: Value) -> anyhow::Result<ConnectorInfo> {
        let m = self.manifest(id)?;
        settings::validate(&m.settings_schema, &new_settings).map_err(|e| anyhow::anyhow!(e))?;
        let merged = settings::merge_with_defaults(&m.settings_schema, &new_settings);
        self.save_settings(id, &merged).await?;
        self.log(id, LogLevel::Info, "settings updated");
        self.get(id).await
    }

    pub async fn reset_settings(&self, id: &str) -> anyhow::Result<ConnectorInfo> {
        let m = self.manifest(id)?;
        self.save_settings(id, &settings::defaults(&m.settings_schema)).await?;
        self.log(id, LogLevel::Info, "settings reset to defaults");
        self.get(id).await
    }

    async fn save_settings(&self, id: &str, value: &Value) -> anyhow::Result<()> {
        sqlx::query("UPDATE connectors SET settings_json = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(serde_json::to_string(value)?)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Stores the connector's API key/token in the OS keyring; only metadata goes to SQLite.
    pub async fn set_secret(&self, id: &str, value: &str) -> anyhow::Result<ConnectorInfo> {
        let m = self.manifest(id)?;
        anyhow::ensure!(m.auth_type.uses_secret(), "this connector does not use credentials");
        let value = value.trim();
        anyhow::ensure!(!value.is_empty(), "secret is empty");
        let key = m.secret_key();
        self.secrets.set(&key, value)?;
        sqlx::query(
            "INSERT INTO connector_credentials_metadata (connector_id, secret_key, label) VALUES (?, ?, ?)
             ON CONFLICT(connector_id, secret_key) DO UPDATE SET created_at = datetime('now')",
        )
        .bind(id)
        .bind(&key)
        .bind(m.auth_type.as_str())
        .execute(&self.pool)
        .await?;
        self.log(id, LogLevel::Info, "credentials saved");
        self.get(id).await
    }

    pub async fn delete_secret(&self, id: &str) -> anyhow::Result<ConnectorInfo> {
        let m = self.manifest(id)?;
        let key = m.secret_key();
        self.secrets.delete(&key)?;
        sqlx::query("DELETE FROM connector_credentials_metadata WHERE connector_id = ? AND secret_key = ?")
            .bind(id)
            .bind(&key)
            .execute(&self.pool)
            .await?;
        self.log(id, LogLevel::Info, "credentials removed");
        self.get(id).await
    }

    /// Runs the manifest's health check and stores the result as `last_status`.
    pub async fn test(&self, router: &AiRouter, id: &str) -> anyhow::Result<ConnectorInfo> {
        let info = self.get(id).await?;
        let result = self.run_health_check(router, &info).await;
        let (stored, level, message) = match &result {
            Ok(msg) => ("ok".to_string(), LogLevel::Info, format!("test passed: {msg}")),
            Err(e) => (format!("error: {e}"), LogLevel::Error, format!("test failed: {e}")),
        };
        sqlx::query("UPDATE connectors SET last_status = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(&stored)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.log(id, level, message);
        self.get(id).await
    }

    async fn run_health_check(&self, router: &AiRouter, info: &ConnectorInfo) -> anyhow::Result<String> {
        let m = &info.manifest;
        match &m.health_check {
            HealthCheck::None => Ok("no check needed".into()),
            HealthCheck::Credentials => {
                if info.has_secret {
                    Ok("credentials stored (the service itself was not contacted)".into())
                } else if m.auth_type == super::types::AuthType::Oauth {
                    anyhow::bail!("not connected — OAuth sign-in is not implemented yet; paste an access token")
                } else {
                    anyhow::bail!("no API key stored")
                }
            }
            HealthCheck::Executable { program } => find_executable(program)
                .map(|p| format!("found {}", p.display()))
                .ok_or_else(|| anyhow::anyhow!("`{program}` was not found on PATH")),
            HealthCheck::Http { url } => {
                let url = fill_placeholders(url, &info.settings);
                let resp = self.http.get(&url).send().await?;
                anyhow::ensure!(resp.status().is_success(), "{url} returned HTTP {}", resp.status());
                Ok(format!("{url} responded"))
            }
            HealthCheck::AiProvider { provider } => {
                router.provider(*provider)?.test_connection().await?;
                Ok("provider responded".into())
            }
        }
    }

    pub fn logs(&self, id: &str) -> anyhow::Result<Vec<ModuleLogEntry>> {
        self.manifest(id)?;
        Ok(self.logs.lock().unwrap().get(id).map(LogBuffer::entries).unwrap_or_default())
    }

    fn log(&self, id: &str, level: LogLevel, message: impl Into<String>) {
        self.logs.lock().unwrap().entry(id.to_string()).or_default().push(level, message);
    }
}

/// Replaces `{key}` with string/number settings values.
fn fill_placeholders(template: &str, settings: &Value) -> String {
    let mut out = template.to_string();
    if let Some(obj) = settings.as_object() {
        for (k, v) in obj {
            let value = match v {
                Value::String(s) => s.trim_end_matches('/').to_string(),
                Value::Number(n) => n.to_string(),
                _ => continue,
            };
            out = out.replace(&format!("{{{k}}}"), &value);
        }
    }
    out
}

/// Looks for `program` on PATH, trying PATHEXT extensions on Windows.
fn find_executable(program: &str) -> Option<PathBuf> {
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into())
            .split(';')
            .filter(|e| !e.is_empty())
            .map(|e| e.to_ascii_lowercase())
            .collect()
    } else {
        vec![String::new()]
    };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        exts.iter()
            .map(|ext| dir.join(format!("{program}{ext}")))
            .chain(std::iter::once(dir.join(program)))
            .find(|candidate| Path::new(candidate).is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fills_placeholders() {
        let s = json!({ "base_url": "http://localhost:11434/", "port": 4455, "flag": true });
        assert_eq!(fill_placeholders("{base_url}/api/version", &s), "http://localhost:11434/api/version");
        assert_eq!(fill_placeholders("ws://h:{port}", &s), "ws://h:4455");
    }

    #[test]
    fn finds_executables_on_path() {
        let program = if cfg!(windows) { "cmd" } else { "sh" };
        assert!(find_executable(program).is_some());
        assert!(find_executable("definitely-not-a-real-program-xyz").is_none());
    }
}
