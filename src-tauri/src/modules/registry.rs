use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use sqlx::SqlitePool;

use super::settings;
use super::status::{LogBuffer, LogLevel, ModuleHealth, ModuleLogEntry, ModuleStatus};
use super::types::{Module, ModuleInfo};
use crate::db::models::ModuleRow;

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("module `{0}` not found")]
    NotFound(String),
    #[error("invalid settings: {0}")]
    InvalidSettings(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type RegistryResult<T> = Result<T, RegistryError>;

/// Owns all modules, persists their enabled flag and settings in the
/// `modules` table and tracks runtime status + logs in memory.
pub struct ModuleRegistry {
    pool: SqlitePool,
    modules: Vec<Arc<dyn Module>>,
    status: Mutex<HashMap<String, ModuleStatus>>,
    logs: Mutex<HashMap<String, LogBuffer>>,
}

impl ModuleRegistry {
    pub fn new(pool: SqlitePool, modules: Vec<Arc<dyn Module>>) -> Self {
        Self {
            pool,
            modules,
            status: Mutex::new(HashMap::new()),
            logs: Mutex::new(HashMap::new()),
        }
    }

    /// Inserts rows for modules seen for the first time, refreshes static
    /// metadata for known ones, then initializes every enabled module.
    pub async fn sync(&self) -> RegistryResult<()> {
        for module in &self.modules {
            let m = module.manifest();
            let defaults = settings::defaults(&m.settings_schema);
            sqlx::query(
                "INSERT INTO modules (id, name, description, category, enabled, settings_json, requires_restart, permissions_json)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    description = excluded.description,
                    category = excluded.category,
                    requires_restart = excluded.requires_restart,
                    permissions_json = excluded.permissions_json",
            )
            .bind(m.id)
            .bind(m.name)
            .bind(m.description)
            .bind(m.category.as_str())
            .bind(m.enabled_by_default)
            .bind(serde_json::to_string(&defaults)?)
            .bind(m.requires_restart)
            .bind(serde_json::to_string(&m.permissions)?)
            .execute(&self.pool)
            .await?;
        }

        for module in &self.modules {
            let id = module.manifest().id;
            let row = self.row(id).await?;
            if row.enabled {
                let settings = self.effective_settings(module.as_ref(), &row)?;
                self.start(module.as_ref(), &settings);
            } else {
                self.set_status(id, ModuleStatus::Stopped);
            }
        }
        Ok(())
    }

    pub async fn list(&self) -> RegistryResult<Vec<ModuleInfo>> {
        let mut out = Vec::with_capacity(self.modules.len());
        for module in &self.modules {
            out.push(self.info(module.as_ref()).await?);
        }
        Ok(out)
    }

    pub async fn get(&self, id: &str) -> RegistryResult<ModuleInfo> {
        let module = self.find(id)?;
        self.info(module.as_ref()).await
    }

    pub async fn get_settings(&self, id: &str) -> RegistryResult<Value> {
        let module = self.find(id)?;
        let row = self.row(id).await?;
        self.effective_settings(module.as_ref(), &row)
    }

    pub async fn enable(&self, id: &str) -> RegistryResult<ModuleInfo> {
        let module = self.find(id)?;
        self.set_enabled(id, true).await?;
        let settings = self.get_settings(id).await?;
        self.start(module.as_ref(), &settings);
        self.info(module.as_ref()).await
    }

    pub async fn disable(&self, id: &str) -> RegistryResult<ModuleInfo> {
        let module = self.find(id)?;
        self.set_enabled(id, false).await?;
        match module.shutdown() {
            Ok(()) => {
                self.set_status(id, ModuleStatus::Stopped);
                self.log(id, LogLevel::Info, "disabled");
            }
            Err(e) => {
                self.set_status(id, ModuleStatus::Error(e.to_string()));
                self.log(id, LogLevel::Error, format!("shutdown failed: {e}"));
            }
        }
        self.info(module.as_ref()).await
    }

    pub async fn update_settings(&self, id: &str, new_settings: Value) -> RegistryResult<ModuleInfo> {
        let module = self.find(id)?;
        let schema = &module.manifest().settings_schema;
        settings::validate(schema, &new_settings).map_err(RegistryError::InvalidSettings)?;
        let merged = settings::merge_with_defaults(schema, &new_settings);
        self.save_settings(id, &merged).await?;
        self.log(id, LogLevel::Info, "settings updated");
        self.notify_settings_changed(module.as_ref(), &merged).await?;
        self.info(module.as_ref()).await
    }

    pub async fn reset_settings(&self, id: &str) -> RegistryResult<ModuleInfo> {
        let module = self.find(id)?;
        let defaults = settings::defaults(&module.manifest().settings_schema);
        self.save_settings(id, &defaults).await?;
        self.log(id, LogLevel::Info, "settings reset to defaults");
        self.notify_settings_changed(module.as_ref(), &defaults).await?;
        self.info(module.as_ref()).await
    }

    pub fn logs(&self, id: &str) -> RegistryResult<Vec<ModuleLogEntry>> {
        self.find(id)?;
        let logs = self.logs.lock().unwrap();
        Ok(logs.get(id).map(LogBuffer::entries).unwrap_or_default())
    }

    /// Current status; runs the module's health check when it is running.
    pub fn status(&self, id: &str) -> RegistryResult<ModuleStatus> {
        let module = self.find(id)?;
        let current = self.current_status(id);
        if current != ModuleStatus::Running {
            return Ok(current);
        }
        let status = match module.health_check() {
            ModuleHealth::Healthy => ModuleStatus::Running,
            ModuleHealth::Degraded(msg) => {
                self.log(id, LogLevel::Warn, format!("degraded: {msg}"));
                ModuleStatus::Running
            }
            ModuleHealth::Unhealthy(msg) => {
                self.log(id, LogLevel::Error, format!("unhealthy: {msg}"));
                ModuleStatus::Error(msg)
            }
        };
        self.set_status(id, status.clone());
        Ok(status)
    }

    /// Shuts down every running module (called on app exit).
    pub fn shutdown_all(&self) {
        for module in &self.modules {
            let id = module.manifest().id;
            if self.current_status(id) == ModuleStatus::Running {
                if let Err(e) = module.shutdown() {
                    tauri_plugin_log::log::warn!("module {id} shutdown failed: {e}");
                }
            }
        }
    }

    // --- internals ---

    fn find(&self, id: &str) -> RegistryResult<Arc<dyn Module>> {
        self.modules
            .iter()
            .find(|m| m.manifest().id == id)
            .cloned()
            .ok_or_else(|| RegistryError::NotFound(id.to_string()))
    }

    async fn row(&self, id: &str) -> RegistryResult<ModuleRow> {
        sqlx::query_as::<_, ModuleRow>("SELECT * FROM modules WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| RegistryError::NotFound(id.to_string()))
    }

    fn effective_settings(&self, module: &dyn Module, row: &ModuleRow) -> RegistryResult<Value> {
        let stored: Value = serde_json::from_str(&row.settings_json)?;
        Ok(settings::merge_with_defaults(&module.manifest().settings_schema, &stored))
    }

    async fn info(&self, module: &dyn Module) -> RegistryResult<ModuleInfo> {
        let m = module.manifest();
        let row = self.row(m.id).await?;
        let settings = self.effective_settings(module, &row)?;
        Ok(ModuleInfo {
            id: m.id.to_string(),
            name: m.name.to_string(),
            description: m.description.to_string(),
            category: m.category,
            enabled: row.enabled,
            requires_restart: m.requires_restart,
            permissions: m.permissions.iter().map(|p| p.to_string()).collect(),
            settings_schema: m.settings_schema.clone(),
            settings,
            status: self.current_status(m.id),
        })
    }

    fn start(&self, module: &dyn Module, settings: &Value) {
        let id = module.manifest().id;
        match module.init(settings) {
            Ok(()) => {
                self.set_status(id, ModuleStatus::Running);
                self.log(id, LogLevel::Info, "started");
            }
            Err(e) => {
                self.set_status(id, ModuleStatus::Error(e.to_string()));
                self.log(id, LogLevel::Error, format!("init failed: {e}"));
            }
        }
    }

    async fn notify_settings_changed(&self, module: &dyn Module, settings: &Value) -> RegistryResult<()> {
        let id = module.manifest().id;
        if self.current_status(id) != ModuleStatus::Running {
            return Ok(());
        }
        if module.manifest().requires_restart {
            self.log(id, LogLevel::Warn, "restart required to apply settings");
        } else if let Err(e) = module.on_settings_changed(settings) {
            self.log(id, LogLevel::Error, format!("failed to apply settings: {e}"));
        }
        Ok(())
    }

    async fn set_enabled(&self, id: &str, enabled: bool) -> RegistryResult<()> {
        sqlx::query("UPDATE modules SET enabled = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(enabled)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn save_settings(&self, id: &str, value: &Value) -> RegistryResult<()> {
        sqlx::query("UPDATE modules SET settings_json = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(serde_json::to_string(value)?)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    fn current_status(&self, id: &str) -> ModuleStatus {
        self.status
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .unwrap_or(ModuleStatus::Stopped)
    }

    fn set_status(&self, id: &str, status: ModuleStatus) {
        self.status.lock().unwrap().insert(id.to_string(), status);
    }

    fn log(&self, id: &str, level: LogLevel, message: impl Into<String>) {
        self.logs
            .lock()
            .unwrap()
            .entry(id.to_string())
            .or_default()
            .push(level, message);
    }
}
