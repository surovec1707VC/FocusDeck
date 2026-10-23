//! Module state in SQLite: enabled flag, JSON settings, action log.
//! Pure database logic; side effects (hotkeys, tray, …) live in `effects`.

use std::collections::HashSet;

use serde_json::Value;
use sqlx::SqlitePool;

use super::catalog::{self, BUILTIN, MODULE_REGISTRY};
use super::schema;
use super::types::{ModuleDef, ModuleInfo, ModuleLog, ModulePermission};
use crate::error::{AppError, AppResult};
use crate::permissions;

pub fn def(id: &str) -> AppResult<&'static ModuleDef> {
    catalog::find(id).ok_or_else(|| AppError::NotFound(format!("модуль «{id}»")))
}

/// Adds new modules and refreshes static metadata. The user's enabled flag
/// and settings are never overwritten.
pub async fn seed(pool: &SqlitePool) -> AppResult<()> {
    for m in BUILTIN {
        sqlx::query(
            "INSERT INTO modules (id, name, description, category, enabled, is_critical, version, requires_restart, settings_json)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, description = excluded.description,
                category = excluded.category, is_critical = excluded.is_critical,
                version = excluded.version, requires_restart = excluded.requires_restart",
        )
        .bind(m.id)
        .bind(m.name)
        .bind(m.description)
        .bind(serde_json::to_value(m.category)?.as_str().unwrap_or_default().to_string())
        // A release build that knows its update server starts with updates on.
        .bind(m.enabled_by_default || (m.id == catalog::UPDATES && crate::updates::configured()))
        .bind(m.critical)
        .bind(m.version)
        .bind(m.requires_restart)
        .bind(schema::defaults(&(m.settings_schema)()).to_string())
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn is_enabled(pool: &SqlitePool, id: &str) -> AppResult<bool> {
    let row: Option<(bool,)> = sqlx::query_as("SELECT enabled FROM modules WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0).unwrap_or(false))
}

pub async fn enabled_set(pool: &SqlitePool) -> AppResult<HashSet<String>> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT id FROM modules WHERE enabled = 1").fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Effective settings: stored values over schema defaults.
pub async fn settings(pool: &SqlitePool, id: &str) -> AppResult<Value> {
    let m = def(id)?;
    let (json,): (String,) = sqlx::query_as("SELECT settings_json FROM modules WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("модуль «{id}»")))?;
    let stored: Value = serde_json::from_str(&json).unwrap_or(Value::Null);
    Ok(schema::merge(&(m.settings_schema)(), &stored))
}

pub async fn permissions(pool: &SqlitePool, id: &str) -> AppResult<Vec<ModulePermission>> {
    let m = def(id)?;
    let all = permissions::list(pool).await?;
    Ok(m.permissions
        .iter()
        .map(|p| {
            let found = all.iter().find(|x| x.id == *p);
            ModulePermission {
                id: p.to_string(),
                name: found.map(|x| x.name.clone()).unwrap_or_else(|| p.to_string()),
                granted: found.map(|x| x.granted).unwrap_or(false),
            }
        })
        .collect())
}

pub async fn get(pool: &SqlitePool, id: &str, pending_restart: &HashSet<String>) -> AppResult<ModuleInfo> {
    let m = def(id)?;
    let (enabled, updated_at): (bool, String) = sqlx::query_as("SELECT enabled, updated_at FROM modules WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    let perms = permissions(pool, id).await?;
    Ok(ModuleInfo {
        id: m.id.into(),
        name: m.name.into(),
        description: m.description.into(),
        details: m.details.into(),
        category: m.category,
        version: m.version.into(),
        enabled,
        critical: m.critical,
        requires_restart: m.requires_restart,
        pending_restart: pending_restart.contains(m.id),
        missing_permissions: perms.iter().filter(|p| !p.granted).map(|p| p.id.clone()).collect(),
        permissions: perms,
        settings: settings(pool, id).await?,
        settings_schema: (m.settings_schema)(),
        updated_at,
    })
}

pub async fn list(pool: &SqlitePool, pending_restart: &HashSet<String>) -> AppResult<Vec<ModuleInfo>> {
    let mut out = Vec::with_capacity(BUILTIN.len());
    for m in BUILTIN {
        out.push(get(pool, m.id, pending_restart).await?);
    }
    Ok(out)
}

/// Enables or disables a module. Disabling a critical module needs `confirm`.
/// Settings are kept either way. Returns whether the state changed.
pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool, confirm: bool, source: &str) -> AppResult<bool> {
    let m = def(id)?;
    if is_enabled(pool, id).await? == enabled {
        return Ok(false);
    }
    if !enabled && m.critical && !confirm {
        log(pool, id, "warn", "disable_blocked", "отключение критического модуля без подтверждения", source).await?;
        return Err(AppError::ConfirmationRequired(format!(
            "«{}» — критический модуль. Подтвердите отключение.",
            m.name
        )));
    }
    sqlx::query("UPDATE modules SET enabled = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(enabled)
        .bind(id)
        .execute(pool)
        .await?;
    let (level, action, message) = match (enabled, m.critical) {
        (true, _) => ("info", "enable", "модуль включён"),
        (false, true) => ("warn", "disable", "критический модуль отключён (подтверждено)"),
        (false, false) => ("info", "disable", "модуль отключён"),
    };
    log(pool, id, level, action, message, source).await?;
    tracing::info!(module = id, enabled, source, "module toggled");
    Ok(true)
}

/// Validates and stores settings (merged over defaults). Returns the effective settings.
pub async fn update_settings(pool: &SqlitePool, id: &str, new: &Value, source: &str) -> AppResult<Value> {
    let m = def(id)?;
    let schema = (m.settings_schema)();
    schema::validate(&schema, new).map_err(AppError::Invalid)?;
    let merged = schema::merge(&schema, new);
    let before = settings(pool, id).await?;
    store_settings(pool, id, &merged).await?;
    let changed: Vec<&String> = merged
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, v)| before.get(k.as_str()) != Some(v))
        .map(|(k, _)| k)
        .collect();
    let message = if changed.is_empty() {
        "без изменений".to_string()
    } else {
        format!("изменено: {}", changed.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", "))
    };
    log(pool, id, "info", "settings_update", &message, source).await?;
    Ok(merged)
}

pub async fn reset_settings(pool: &SqlitePool, id: &str, source: &str) -> AppResult<Value> {
    let m = def(id)?;
    let defaults = schema::defaults(&(m.settings_schema)());
    store_settings(pool, id, &defaults).await?;
    log(pool, id, "info", "settings_reset", "настройки сброшены к значениям по умолчанию", source).await?;
    Ok(defaults)
}

async fn store_settings(pool: &SqlitePool, id: &str, value: &Value) -> AppResult<()> {
    sqlx::query("UPDATE modules SET settings_json = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(value.to_string())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Writes a module action to `module_logs`, keeping the newest N entries
/// (N = `log_retention` of the Module Registry).
pub async fn log(pool: &SqlitePool, id: &str, level: &str, action: &str, message: &str, source: &str) -> AppResult<()> {
    sqlx::query("INSERT INTO module_logs (module_id, level, action, message, source) VALUES (?, ?, ?, ?, ?)")
        .bind(id)
        .bind(level)
        .bind(action)
        .bind(message)
        .bind(source)
        .execute(pool)
        .await?;
    let retention = if id == MODULE_REGISTRY {
        // Avoid recursion through settings(): read the value directly.
        1000
    } else {
        settings(pool, MODULE_REGISTRY).await?.get("log_retention").and_then(Value::as_i64).unwrap_or(1000)
    };
    sqlx::query(
        "DELETE FROM module_logs WHERE module_id = ? AND id NOT IN
           (SELECT id FROM module_logs WHERE module_id = ? ORDER BY id DESC LIMIT ?)",
    )
    .bind(id)
    .bind(id)
    .bind(retention)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn logs(pool: &SqlitePool, id: &str, limit: i64) -> AppResult<Vec<ModuleLog>> {
    def(id)?;
    Ok(sqlx::query_as::<_, ModuleLog>("SELECT * FROM module_logs WHERE module_id = ? ORDER BY id DESC LIMIT ?")
        .bind(id)
        .bind(limit.clamp(1, 1000))
        .fetch_all(pool)
        .await?)
}

#[cfg(test)]
pub(crate) async fn test_pool() -> SqlitePool {
    let pool = crate::db::test_pool().await;
    permissions::seed(&pool).await.unwrap();
    seed(&pool).await.unwrap();
    pool
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::catalog::{AUTOMATIONS, CLIPBOARD, SETTINGS, WIDGET_CLOCK};
    use serde_json::json;

    #[tokio::test]
    async fn seed_list_and_metadata() {
        let pool = test_pool().await;
        let all = list(&pool, &HashSet::new()).await.unwrap();
        assert_eq!(all.len(), BUILTIN.len());
        let clock = all.iter().find(|m| m.id == WIDGET_CLOCK).unwrap();
        assert_eq!(clock.settings, json!({ "show_seconds": true, "hour12": false, "show_date": true }));
        assert_eq!(clock.version, "1.0.0");
        let clip = all.iter().find(|m| m.id == CLIPBOARD).unwrap();
        assert!(!clip.enabled);
        assert_eq!(clip.missing_permissions, ["clipboard.read"]);
    }

    #[tokio::test]
    async fn disable_keeps_settings_and_reseed_keeps_choice() {
        let pool = test_pool().await;
        update_settings(&pool, AUTOMATIONS, &json!({ "scheduler_interval_secs": 60 }), "test").await.unwrap();
        assert!(set_enabled(&pool, AUTOMATIONS, false, false, "test").await.unwrap());
        assert!(!set_enabled(&pool, AUTOMATIONS, false, false, "test").await.unwrap(), "no-op");
        seed(&pool).await.unwrap();
        assert!(!is_enabled(&pool, AUTOMATIONS).await.unwrap());
        assert_eq!(settings(&pool, AUTOMATIONS).await.unwrap()["scheduler_interval_secs"], 60);
    }

    #[tokio::test]
    async fn critical_needs_confirmation() {
        let pool = test_pool().await;
        assert!(matches!(
            set_enabled(&pool, SETTINGS, false, false, "test").await,
            Err(AppError::ConfirmationRequired(_))
        ));
        assert!(is_enabled(&pool, SETTINGS).await.unwrap());
        assert!(set_enabled(&pool, SETTINGS, false, true, "test").await.unwrap());
        assert!(set_enabled(&pool, SETTINGS, true, false, "test").await.unwrap(), "enabling needs no confirmation");
        let actions: Vec<String> = logs(&pool, SETTINGS, 10).await.unwrap().into_iter().map(|l| l.action).collect();
        assert_eq!(actions, ["enable", "disable", "disable_blocked"]);
    }

    #[tokio::test]
    async fn settings_validation_reset_and_log() {
        let pool = test_pool().await;
        assert!(matches!(
            update_settings(&pool, AUTOMATIONS, &json!({ "scheduler_interval_secs": 1 }), "t").await,
            Err(AppError::Invalid(_))
        ));
        update_settings(&pool, AUTOMATIONS, &json!({ "scheduler_interval_secs": 120 }), "t").await.unwrap();
        assert_eq!(reset_settings(&pool, AUTOMATIONS, "t").await.unwrap()["scheduler_interval_secs"], 30);
        let last = &logs(&pool, AUTOMATIONS, 10).await.unwrap()[1];
        assert_eq!(last.action, "settings_update");
        assert!(last.message.contains("scheduler_interval_secs"));
    }

    #[tokio::test]
    async fn log_retention_prunes() {
        let pool = test_pool().await;
        update_settings(&pool, MODULE_REGISTRY, &json!({ "log_retention": 100 }), "t").await.unwrap();
        for i in 0..105 {
            log(&pool, AUTOMATIONS, "info", "x", &i.to_string(), "t").await.unwrap();
        }
        let kept = logs(&pool, AUTOMATIONS, 1000).await.unwrap();
        assert_eq!(kept.len(), 100);
        assert_eq!(kept[0].message, "104");
    }
}
