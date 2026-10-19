//! Export and import of the configuration (app settings, modules,
//! permissions) as JSON. User data (layouts, tasks, notes, automations) is
//! not part of the configuration.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;

use super::catalog::{BUILTIN, SECURITY, SETTINGS};
use super::registry;
use super::schema;
use crate::error::{AppError, AppResult};
use crate::permissions;
use crate::settings::{self, AppSettings};

pub const FORMAT: &str = "focusdeck.config";
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    pub id: String,
    pub enabled: bool,
    #[serde(default)]
    pub settings: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    pub id: String,
    pub granted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub exported_at: String,
    #[serde(default)]
    pub app_version: String,
    pub settings: Option<AppSettings>,
    #[serde(default)]
    pub modules: Vec<ModuleConfig>,
    #[serde(default)]
    pub permissions: Vec<PermissionConfig>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportReport {
    pub settings_applied: bool,
    pub modules_applied: usize,
    pub permissions_applied: usize,
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
    /// Modules whose enabled state changed (side effects must be applied).
    #[serde(skip)]
    pub changed_modules: Vec<String>,
    /// Modules whose settings changed.
    #[serde(skip)]
    pub changed_settings: Vec<String>,
}

pub async fn export(pool: &SqlitePool, app_settings: &AppSettings, app_version: &str) -> AppResult<ConfigFile> {
    let mut modules = Vec::new();
    for m in BUILTIN {
        modules.push(ModuleConfig {
            id: m.id.into(),
            enabled: registry::is_enabled(pool, m.id).await?,
            settings: registry::settings(pool, m.id).await?,
        });
    }
    let permissions = permissions::list(pool)
        .await?
        .into_iter()
        .map(|p| PermissionConfig { id: p.id, granted: p.granted })
        .collect();
    Ok(ConfigFile {
        format: FORMAT.into(),
        version: VERSION,
        exported_at: chrono::Utc::now().to_rfc3339(),
        app_version: app_version.into(),
        settings: Some(app_settings.clone()),
        modules,
        permissions,
    })
}

pub fn parse(json: &str) -> AppResult<ConfigFile> {
    let file: ConfigFile =
        serde_json::from_str(json).map_err(|e| AppError::Invalid(format!("файл не похож на конфигурацию FocusDeck: {e}")))?;
    if file.format != FORMAT {
        return Err(AppError::Invalid(format!("неизвестный формат «{}»", file.format)));
    }
    if file.version > VERSION {
        return Err(AppError::Invalid(format!(
            "конфигурация версии {} создана более новой версией FocusDeck",
            file.version
        )));
    }
    Ok(file)
}

/// Critical modules the file would turn off.
pub fn critical_disables(file: &ConfigFile) -> Vec<&'static str> {
    file.modules
        .iter()
        .filter(|m| !m.enabled)
        .filter_map(|m| BUILTIN.iter().find(|d| d.id == m.id && d.critical).map(|d| d.name))
        .collect()
}

/// Applies the file to the database. Invalid parts are skipped and reported;
/// app settings are returned for the caller to apply (hotkeys, autostart).
/// Turning off critical modules needs `confirm`.
pub async fn apply(pool: &SqlitePool, file: &ConfigFile, confirm: bool) -> AppResult<(ImportReport, Option<AppSettings>)> {
    let critical = critical_disables(file);
    if !critical.is_empty() && !confirm {
        return Err(AppError::ConfirmationRequired(format!(
            "Конфигурация отключает критические модули: {}. Подтвердите импорт.",
            critical.join(", ")
        )));
    }
    let mut report = ImportReport::default();

    let new_settings = match &file.settings {
        Some(_) if !registry::is_enabled(pool, SETTINGS).await? => {
            report.warnings.push("модуль «Настройки» отключён — настройки приложения не импортированы".into());
            None
        }
        Some(s) => match s.validate() {
            Ok(()) => {
                report.settings_applied = true;
                Some(s.clone())
            }
            Err(e) => {
                report.skipped.push(format!("настройки приложения: {e}"));
                None
            }
        },
        None => None,
    };

    for mc in &file.modules {
        let Some(def) = BUILTIN.iter().find(|d| d.id == mc.id) else {
            report.skipped.push(format!("неизвестный модуль «{}»", mc.id));
            continue;
        };
        let schema_v = (def.settings_schema)();
        let settings_value = if mc.settings.is_null() { Value::Object(Default::default()) } else { mc.settings.clone() };
        if let Err(e) = schema::validate(&schema_v, &settings_value) {
            report.skipped.push(format!("настройки «{}»: {e}", def.name));
        } else if registry::settings(pool, def.id).await? != schema::merge(&schema_v, &settings_value) {
            registry::update_settings(pool, def.id, &settings_value, "import").await?;
            report.changed_settings.push(def.id.into());
        }
        if registry::set_enabled(pool, def.id, mc.enabled, confirm, "import").await? {
            report.changed_modules.push(def.id.into());
        }
        report.modules_applied += 1;
    }

    if !file.permissions.is_empty() {
        if registry::is_enabled(pool, SECURITY).await? {
            let known: Vec<String> = permissions::list(pool).await?.into_iter().map(|p| p.id).collect();
            for p in &file.permissions {
                if known.contains(&p.id) {
                    permissions::set(pool, &p.id, p.granted, "import").await?;
                    report.permissions_applied += 1;
                } else {
                    report.skipped.push(format!("неизвестное разрешение «{}»", p.id));
                }
            }
        } else {
            report.warnings.push("модуль «Безопасность» отключён — разрешения не импортированы".into());
        }
    }

    if let Some(s) = &new_settings {
        settings::save(pool, s).await?;
    }
    Ok((report, new_settings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::catalog::{AUTOMATIONS, WIDGET_CLOCK};
    use serde_json::json;

    #[tokio::test]
    async fn export_import_round_trip() {
        let pool = registry::test_pool().await;
        registry::update_settings(&pool, WIDGET_CLOCK, &json!({ "show_seconds": false }), "t").await.unwrap();
        registry::set_enabled(&pool, AUTOMATIONS, false, false, "t").await.unwrap();
        let mut s = AppSettings::default();
        s.close_to_tray = false;
        let exported = export(&pool, &s, "0.1.0").await.unwrap();
        let json = serde_json::to_string(&exported).unwrap();

        // Fresh database, then import.
        let other = registry::test_pool().await;
        let (report, settings) = apply(&other, &parse(&json).unwrap(), false).await.unwrap();
        assert!(report.skipped.is_empty(), "{:?}", report.skipped);
        assert_eq!(report.modules_applied, BUILTIN.len());
        assert_eq!(report.changed_modules, [AUTOMATIONS]);
        assert_eq!(report.changed_settings, [WIDGET_CLOCK]);
        assert!(!settings.unwrap().close_to_tray);
        assert!(!registry::is_enabled(&other, AUTOMATIONS).await.unwrap());
        assert_eq!(registry::settings(&other, WIDGET_CLOCK).await.unwrap()["show_seconds"], false);
    }

    #[tokio::test]
    async fn rejects_and_skips_bad_input() {
        assert!(parse("{}").is_err());
        assert!(parse(r#"{"format":"other","version":1}"#).is_err());
        assert!(parse(r#"{"format":"focusdeck.config","version":99}"#).is_err());

        let pool = registry::test_pool().await;
        let file = parse(
            r#"{"format":"focusdeck.config","version":1,
                "modules":[{"id":"nope","enabled":true},
                           {"id":"automations","enabled":true,"settings":{"scheduler_interval_secs":1}}],
                "permissions":[{"id":"unknown.perm","granted":true}]}"#,
        )
        .unwrap();
        let (report, _) = apply(&pool, &file, false).await.unwrap();
        assert_eq!(report.skipped.len(), 3, "{:?}", report.skipped);
        assert_eq!(registry::settings(&pool, AUTOMATIONS).await.unwrap()["scheduler_interval_secs"], 30);
    }

    #[tokio::test]
    async fn critical_disable_needs_confirmation() {
        let pool = registry::test_pool().await;
        let file = parse(r#"{"format":"focusdeck.config","version":1,"modules":[{"id":"database","enabled":false}]}"#).unwrap();
        assert!(matches!(apply(&pool, &file, false).await, Err(AppError::ConfirmationRequired(_))));
        assert!(registry::is_enabled(&pool, "database").await.unwrap());
        apply(&pool, &file, true).await.unwrap();
        assert!(!registry::is_enabled(&pool, "database").await.unwrap());
    }
}
