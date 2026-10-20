//! Connector state in SQLite: enabled flag, granted permissions, settings,
//! last health check and the per-connector log.

use serde::Serialize;
use serde_json::Value;
use sqlx::SqlitePool;

use super::manifest::{self, Manifest};
use crate::error::{AppError, AppResult};
use crate::modules::schema;

pub const DEFAULT_LOG_LIMIT: i64 = 500;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Row {
    pub enabled: bool,
    pub granted_json: String,
    pub health_status: String,
    pub health_message: String,
    pub health_checked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct LogEntry {
    pub id: i64,
    pub connector_id: String,
    pub level: String,
    pub action: String,
    pub message: String,
    pub created_at: String,
}

/// A row for every manifest; names and kinds follow the manifests.
pub async fn seed(pool: &SqlitePool) -> AppResult<()> {
    for m in manifest::all() {
        sqlx::query(
            "INSERT INTO connectors (id, name, kind, enabled) VALUES (?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, kind = excluded.kind",
        )
        .bind(&m.id)
        .bind(&m.name)
        .bind(m.kind.as_str())
        .bind(m.enabled_by_default)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn row(pool: &SqlitePool, id: &str) -> AppResult<Row> {
    sqlx::query_as::<_, Row>("SELECT enabled, granted_json, health_status, health_message, health_checked_at FROM connectors WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("интеграция {id}")))
}

pub fn granted(row: &Row) -> Vec<String> {
    serde_json::from_str(&row.granted_json).unwrap_or_default()
}

/// Stored values over the manifest defaults.
pub async fn settings(pool: &SqlitePool, m: &Manifest) -> AppResult<Value> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value_json FROM connector_settings WHERE connector_id = ?").bind(&m.id).fetch_all(pool).await?;
    let stored: serde_json::Map<String, Value> = rows.into_iter().filter_map(|(k, v)| serde_json::from_str(&v).ok().map(|v| (k, v))).collect();
    Ok(schema::merge(&m.settings, &Value::Object(stored)))
}

pub async fn update_settings(pool: &SqlitePool, m: &Manifest, new: &Value) -> AppResult<Value> {
    schema::validate(&m.settings, new).map_err(AppError::Invalid)?;
    // Partial update: keys not in `new` keep their current values.
    let mut merged = settings(pool, m).await?;
    if let (Some(target), Some(src)) = (merged.as_object_mut(), new.as_object()) {
        for (k, v) in src {
            target.insert(k.clone(), v.clone());
        }
    }
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM connector_settings WHERE connector_id = ?").bind(&m.id).execute(&mut *tx).await?;
    for (k, v) in merged.as_object().into_iter().flatten() {
        sqlx::query("INSERT INTO connector_settings (connector_id, key, value_json) VALUES (?, ?, ?)").bind(&m.id).bind(k).bind(v.to_string()).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(merged)
}

pub async fn reset_settings(pool: &SqlitePool, m: &Manifest) -> AppResult<Value> {
    sqlx::query("DELETE FROM connector_settings WHERE connector_id = ?").bind(&m.id).execute(pool).await?;
    Ok(schema::defaults(&m.settings))
}

pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> AppResult<()> {
    sqlx::query("UPDATE connectors SET enabled = ?, updated_at = datetime('now') WHERE id = ?").bind(enabled).bind(id).execute(pool).await?;
    Ok(())
}

/// Grants or revokes one manifest permission; returns the new set.
pub async fn set_permission(pool: &SqlitePool, m: &Manifest, permission: &str, grant: bool) -> AppResult<Vec<String>> {
    if m.permission(permission).is_none() {
        return Err(AppError::Invalid(format!("у интеграции {} нет разрешения «{permission}»", m.name)));
    }
    let mut set = granted(&row(pool, &m.id).await?);
    set.retain(|p| p != permission && m.permission(p).is_some());
    if grant {
        set.push(permission.to_string());
    }
    // Keep manifest order.
    set.sort_by_key(|p| m.permissions.iter().position(|x| &x.id == p));
    sqlx::query("UPDATE connectors SET granted_json = ?, updated_at = datetime('now') WHERE id = ?").bind(serde_json::to_string(&set)?).bind(&m.id).execute(pool).await?;
    Ok(set)
}

pub async fn set_health(pool: &SqlitePool, id: &str, status: &str, message: &str) -> AppResult<()> {
    sqlx::query("UPDATE connectors SET health_status = ?, health_message = ?, health_checked_at = datetime('now') WHERE id = ?")
        .bind(status)
        .bind(message)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn log(pool: &SqlitePool, id: &str, level: &str, action: &str, message: &str, limit: i64) -> AppResult<()> {
    sqlx::query("INSERT INTO connector_logs (connector_id, level, action, message) VALUES (?, ?, ?, ?)").bind(id).bind(level).bind(action).bind(message).execute(pool).await?;
    sqlx::query("DELETE FROM connector_logs WHERE connector_id = ? AND id NOT IN (SELECT id FROM connector_logs WHERE connector_id = ? ORDER BY id DESC LIMIT ?)")
        .bind(id)
        .bind(id)
        .bind(limit.max(10))
        .execute(pool)
        .await?;
    Ok(())
}

/// Newest first; all connectors when `id` is None.
pub async fn logs(pool: &SqlitePool, id: Option<&str>, limit: i64) -> AppResult<Vec<LogEntry>> {
    Ok(sqlx::query_as::<_, LogEntry>(
        "SELECT id, connector_id, level, action, message, created_at FROM connector_logs
         WHERE (?1 IS NULL OR connector_id = ?1) ORDER BY id DESC LIMIT ?2",
    )
    .bind(id)
    .bind(limit.clamp(1, 2000))
    .fetch_all(pool)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn seed_settings_permissions_logs() {
        let pool = crate::db::test_pool().await;
        seed(&pool).await.unwrap();
        seed(&pool).await.unwrap();
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM connectors").fetch_one(&pool).await.unwrap();
        assert_eq!(n, 13);

        let gh = manifest::get("github").unwrap();
        let r = row(&pool, "github").await.unwrap();
        assert!(!r.enabled);
        assert_eq!(r.health_status, "unknown");

        assert_eq!(settings(&pool, gh).await.unwrap()["poll_minutes"], json!(15));
        let s = update_settings(&pool, gh, &json!({ "poll_minutes": 30 })).await.unwrap();
        assert_eq!((s["poll_minutes"].clone(), s["api_url"].clone()), (json!(30), json!("https://api.github.com")));
        assert_eq!(settings(&pool, gh).await.unwrap()["poll_minutes"], json!(30));
        assert!(update_settings(&pool, gh, &json!({ "poll_minutes": 1 })).await.is_err());
        assert!(update_settings(&pool, gh, &json!({ "token": "x" })).await.is_err());
        assert_eq!(reset_settings(&pool, gh).await.unwrap()["poll_minutes"], json!(15));

        assert_eq!(set_permission(&pool, gh, "issues.read", true).await.unwrap(), ["issues.read"]);
        assert_eq!(set_permission(&pool, gh, "repos.read", true).await.unwrap(), ["repos.read", "issues.read"]);
        assert_eq!(set_permission(&pool, gh, "issues.read", false).await.unwrap(), ["repos.read"]);
        assert!(set_permission(&pool, gh, "admin", true).await.is_err());

        set_enabled(&pool, "github", true).await.unwrap();
        set_health(&pool, "github", "ok", "fine").await.unwrap();
        let r = row(&pool, "github").await.unwrap();
        assert!(r.enabled && r.health_status == "ok" && r.health_checked_at.is_some());

        for i in 0..15 {
            log(&pool, "github", "info", "health", &format!("#{i}"), 10).await.unwrap();
        }
        log(&pool, "obs", "info", "enable", "on", 10).await.unwrap();
        let gh_logs = logs(&pool, Some("github"), 100).await.unwrap();
        assert_eq!(gh_logs.len(), 10);
        assert_eq!(gh_logs[0].message, "#14");
        assert_eq!(logs(&pool, None, 100).await.unwrap().len(), 11);
    }
}
