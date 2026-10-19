use serde::Serialize;

/// Static description of a built-in module.
pub struct ModuleDef {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub category: &'static str,
    pub enabled_by_default: bool,
    /// Core modules can't be disabled.
    pub is_core: bool,
    /// Permissions the module needs to work.
    pub permissions: &'static [&'static str],
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub enabled: bool,
    pub is_core: bool,
    pub permissions: Vec<String>,
    /// Required permissions that are not granted yet.
    pub missing_permissions: Vec<String>,
}
