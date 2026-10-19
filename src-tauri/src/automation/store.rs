//! Persistence for automations and their run log.

use serde::Serialize;
use sqlx::SqlitePool;

use super::types::{Automation, AutomationInput};
use crate::db::new_id;
use crate::error::{AppError, AppResult};

#[derive(sqlx::FromRow)]
struct Row {
    id: String,
    name: String,
    enabled: bool,
    trigger_json: String,
    actions_json: String,
    last_run_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl TryFrom<Row> for Automation {
    type Error = AppError;
    fn try_from(r: Row) -> AppResult<Self> {
        Ok(Self {
            trigger: serde_json::from_str(&r.trigger_json)?,
            actions: serde_json::from_str(&r.actions_json)?,
            id: r.id,
            name: r.name,
            enabled: r.enabled,
            last_run_at: r.last_run_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AutomationLog {
    pub id: i64,
    pub automation_id: String,
    pub status: String,
    pub message: String,
    pub created_at: String,
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Automation>> {
    let rows = sqlx::query_as::<_, Row>("SELECT * FROM automations ORDER BY created_at, rowid").fetch_all(pool).await?;
    // A row that no longer parses (e.g. a removed action type) is skipped, not fatal.
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let id = r.id.clone();
            Automation::try_from(r).map_err(|e| tracing::warn!(automation = %id, "unreadable automation: {e}")).ok()
        })
        .collect())
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Automation> {
    sqlx::query_as::<_, Row>("SELECT * FROM automations WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("автоматизация".into()))?
        .try_into()
}

pub async fn create(pool: &SqlitePool, input: &AutomationInput) -> AppResult<Automation> {
    input.validate()?;
    let id = new_id();
    sqlx::query("INSERT INTO automations (id, name, enabled, trigger_json, actions_json) VALUES (?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(input.name.trim())
        .bind(input.enabled)
        .bind(serde_json::to_string(&input.trigger)?)
        .bind(serde_json::to_string(&input.actions)?)
        .execute(pool)
        .await?;
    get(pool, &id).await
}

pub async fn update(pool: &SqlitePool, id: &str, input: &AutomationInput) -> AppResult<Automation> {
    input.validate()?;
    let n = sqlx::query(
        "UPDATE automations SET name = ?, enabled = ?, trigger_json = ?, actions_json = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(input.name.trim())
    .bind(input.enabled)
    .bind(serde_json::to_string(&input.trigger)?)
    .bind(serde_json::to_string(&input.actions)?)
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound("автоматизация".into()));
    }
    get(pool, id).await
}

pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> AppResult<Automation> {
    sqlx::query("UPDATE automations SET enabled = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(enabled)
        .bind(id)
        .execute(pool)
        .await?;
    get(pool, id).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM automations WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn mark_run(pool: &SqlitePool, id: &str, at: &str) -> AppResult<()> {
    sqlx::query("UPDATE automations SET last_run_at = ? WHERE id = ?").bind(at).bind(id).execute(pool).await?;
    Ok(())
}

pub async fn log(pool: &SqlitePool, id: &str, status: &str, message: &str) -> AppResult<()> {
    sqlx::query("INSERT INTO automation_logs (automation_id, status, message) VALUES (?, ?, ?)")
        .bind(id)
        .bind(status)
        .bind(message)
        .execute(pool)
        .await?;
    // Keep the log bounded: the newest 500 entries per automation.
    sqlx::query(
        "DELETE FROM automation_logs WHERE automation_id = ? AND id NOT IN
           (SELECT id FROM automation_logs WHERE automation_id = ? ORDER BY id DESC LIMIT 500)",
    )
    .bind(id)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn logs(pool: &SqlitePool, id: Option<&str>, limit: i64) -> AppResult<Vec<AutomationLog>> {
    let limit = limit.clamp(1, 500);
    Ok(match id {
        Some(id) => {
            sqlx::query_as::<_, AutomationLog>("SELECT * FROM automation_logs WHERE automation_id = ? ORDER BY id DESC LIMIT ?")
                .bind(id)
                .bind(limit)
                .fetch_all(pool)
                .await?
        }
        None => {
            sqlx::query_as::<_, AutomationLog>("SELECT * FROM automation_logs ORDER BY id DESC LIMIT ?")
                .bind(limit)
                .fetch_all(pool)
                .await?
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::types::{Action, Trigger};

    fn input() -> AutomationInput {
        AutomationInput {
            name: "Перерыв".into(),
            enabled: true,
            trigger: Trigger::Interval { minutes: 50 },
            actions: vec![Action::Notify { title: "Пора отдохнуть".into(), body: "".into() }],
        }
    }

    #[tokio::test]
    async fn crud_and_logs() {
        let pool = crate::db::test_pool().await;
        let a = create(&pool, &input()).await.unwrap();
        assert_eq!(a.trigger, Trigger::Interval { minutes: 50 });
        assert_eq!(list(&pool).await.unwrap().len(), 1);

        let mut changed = input();
        changed.name = "Длинный перерыв".into();
        assert_eq!(update(&pool, &a.id, &changed).await.unwrap().name, "Длинный перерыв");
        assert!(!set_enabled(&pool, &a.id, false).await.unwrap().enabled);

        log(&pool, &a.id, "ok", "done").await.unwrap();
        assert_eq!(logs(&pool, Some(&a.id), 10).await.unwrap().len(), 1);

        delete(&pool, &a.id).await.unwrap();
        assert!(list(&pool).await.unwrap().is_empty());
        assert!(logs(&pool, None, 10).await.unwrap().is_empty(), "logs cascade on delete");
    }

    #[tokio::test]
    async fn unreadable_rows_are_skipped() {
        let pool = crate::db::test_pool().await;
        sqlx::query("INSERT INTO automations (id, name, trigger_json, actions_json) VALUES ('x', 'old', '{\"type\":\"gone\"}', '[]')")
            .execute(&pool)
            .await
            .unwrap();
        assert!(list(&pool).await.unwrap().is_empty());
    }
}
