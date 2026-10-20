//! In-app notifications: saved to the history (widget «Уведомления») and
//! shown as a toast. Nothing is sent to Windows or outside the app.

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::AppResult;
use crate::modules::{catalog::NOTIFICATIONS, registry};
use crate::permissions::{self, PERM_NOTIFICATIONS};
use crate::state::AppState;

pub const NOTIFY_EVENT: &str = "focusdeck://notify";
const KEEP: i64 = 500;

/// Payload of the toast event.
#[derive(Debug, Clone, Serialize)]
pub struct Toast {
    pub title: String,
    pub body: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct NotificationItem {
    pub id: i64,
    pub level: String,
    pub title: String,
    pub body: String,
    pub source: String,
    pub read: bool,
    pub created_at: String,
}

/// Saves and shows a notification. Does nothing while the module is off;
/// without the permission it is only saved. Returns whether a toast was shown.
pub async fn push(app: &AppHandle, level: &str, title: &str, body: &str, source: &str) -> AppResult<bool> {
    let db = &app.state::<AppState>().db;
    if !registry::is_enabled(db, NOTIFICATIONS).await? {
        return Ok(false);
    }
    save(db, level, title, body, source).await?;
    if !permissions::is_granted(db, PERM_NOTIFICATIONS).await? {
        return Ok(false);
    }
    app.emit(NOTIFY_EVENT, Toast { title: title.into(), body: body.into(), source: source.into() })
        .map_err(anyhow::Error::from)?;
    Ok(true)
}

pub async fn save(pool: &SqlitePool, level: &str, title: &str, body: &str, source: &str) -> AppResult<()> {
    sqlx::query("INSERT INTO notifications (level, title, body, source) VALUES (?, ?, ?, ?)")
        .bind(level)
        .bind(title)
        .bind(body)
        .bind(source)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM notifications WHERE id NOT IN (SELECT id FROM notifications ORDER BY id DESC LIMIT ?)")
        .bind(KEEP)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list(pool: &SqlitePool, limit: i64, unread_only: bool) -> AppResult<Vec<NotificationItem>> {
    Ok(sqlx::query_as::<_, NotificationItem>(
        "SELECT id, level, title, body, source, read, created_at FROM notifications
         WHERE (? = 0 OR read = 0) ORDER BY id DESC LIMIT ?",
    )
    .bind(unread_only)
    .bind(limit.clamp(1, KEEP))
    .fetch_all(pool)
    .await?)
}

pub async fn unread(pool: &SqlitePool) -> AppResult<i64> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE read = 0").fetch_one(pool).await?;
    Ok(n)
}

pub async fn mark_all_read(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query("UPDATE notifications SET read = 1 WHERE read = 0").execute(pool).await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM notifications WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn clear(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query("DELETE FROM notifications").execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn history() {
        let pool = crate::db::test_pool().await;
        save(&pool, "info", "a", "", "t").await.unwrap();
        save(&pool, "warn", "b", "", "t").await.unwrap();
        assert_eq!(unread(&pool).await.unwrap(), 2);
        assert_eq!(list(&pool, 10, false).await.unwrap()[0].title, "b");
        mark_all_read(&pool).await.unwrap();
        assert!(list(&pool, 10, true).await.unwrap().is_empty());
        assert_eq!(list(&pool, 10, false).await.unwrap().len(), 2);
        clear(&pool).await.unwrap();
        assert!(list(&pool, 10, false).await.unwrap().is_empty());
    }
}
