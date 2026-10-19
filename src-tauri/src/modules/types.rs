use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::status::ModuleHealth;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleCategory {
    Core,
    Ai,
    Productivity,
    Automation,
    Voice,
    Integrations,
    System,
}

impl ModuleCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Ai => "ai",
            Self::Productivity => "productivity",
            Self::Automation => "automation",
            Self::Voice => "voice",
            Self::Integrations => "integrations",
            Self::System => "system",
        }
    }
}

/// Static description of a module. `settings_schema` is a JSON-Schema-like
/// object: `{ "properties": { "<key>": { "type": ..., "default": ..., "title": ... } } }`.
#[derive(Debug, Clone, Serialize)]
pub struct ModuleManifest {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub category: ModuleCategory,
    pub enabled_by_default: bool,
    pub requires_restart: bool,
    pub permissions: Vec<&'static str>,
    pub settings_schema: Value,
}

/// Lifecycle hooks a module implements. Enable/disable state and settings
/// persistence are handled by the registry; modules only react to them.
pub trait Module: Send + Sync {
    fn manifest(&self) -> &ModuleManifest;

    fn init(&self, _settings: &Value) -> anyhow::Result<()> {
        Ok(())
    }

    fn shutdown(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn health_check(&self) -> ModuleHealth {
        ModuleHealth::Healthy
    }

    /// Called after settings were validated and saved.
    fn on_settings_changed(&self, _settings: &Value) -> anyhow::Result<()> {
        Ok(())
    }
}

/// What the frontend sees for a module.
#[derive(Debug, Clone, Serialize)]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: ModuleCategory,
    pub enabled: bool,
    pub requires_restart: bool,
    pub permissions: Vec<String>,
    pub settings_schema: Value,
    pub settings: Value,
    pub status: super::status::ModuleStatus,
}
