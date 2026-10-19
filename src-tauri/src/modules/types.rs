use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Module categories (labels are localized in the frontend).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Appearance,
    Widgets,
    Layouts,
    Automations,
    Productivity,
    DeveloperTools,
    Clipboard,
    Files,
    WindowsIntegration,
    Notifications,
    PrivacySecurity,
    Integrations,
    Performance,
}

/// Static description of a built-in module.
pub struct ModuleDef {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Longer text for the Control Center detail panel.
    pub details: &'static str,
    pub category: Category,
    pub version: &'static str,
    pub enabled_by_default: bool,
    /// Turning it off needs explicit confirmation.
    pub critical: bool,
    /// Changes take effect after restarting FocusDeck.
    pub requires_restart: bool,
    pub permissions: &'static [&'static str],
    /// JSON-Schema subset: `{ "type": "object", "properties": { key: { type, title, default, … } } }`.
    pub settings_schema: fn() -> Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModulePermission {
    pub id: String,
    pub name: String,
    pub granted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub details: String,
    pub category: Category,
    pub version: String,
    pub enabled: bool,
    pub critical: bool,
    pub requires_restart: bool,
    /// Enabled state or settings changed since start; a restart is needed.
    pub pending_restart: bool,
    pub permissions: Vec<ModulePermission>,
    pub missing_permissions: Vec<String>,
    pub settings: Value,
    pub settings_schema: Value,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    /// Enabled and working.
    Ok,
    /// Enabled but limited (e.g. a permission is missing, feature in development).
    Degraded,
    /// Enabled but failing.
    Error,
    /// Turned off by the user.
    Disabled,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleStatus {
    pub module_id: String,
    pub enabled: bool,
    pub state: HealthState,
    pub message: String,
    pub pending_restart: bool,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ModuleLog {
    pub id: i64,
    pub module_id: String,
    pub level: String,
    pub action: String,
    pub message: String,
    pub source: String,
    pub created_at: String,
}
