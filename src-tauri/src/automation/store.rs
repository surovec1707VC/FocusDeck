//! Persistence for automations and their run log.

use serde::Serialize;
use sqlx::SqlitePool;

use super::types::{Automation, AutomationInput, requirements};
use crate::db::new_id;
use crate::error::{AppError, AppResult};

#[derive(sqlx::FromRow)]
struct Row {
    id: String,
    name: String,
    description: String,
    enabled: bool,
    module_id: String,
    trigger_json: String,
    conditions_json: String,
    actions_json: String,
    cooldown_secs: i64,
    template: Option<String>,
    last_run_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl TryFrom<Row> for Automation {
    type Error = AppError;
    fn try_from(r: Row) -> AppResult<Self> {
        let actions: Vec<super::types::Action> = serde_json::from_str(&r.actions_json)?;
        let (permissions, dangerous) = requirements(&actions);
        Ok(Self {
            trigger: serde_json::from_str(&r.trigger_json)?,
            conditions: serde_json::from_str(&r.conditions_json)?,
            actions,
            permissions,
            dangerous,
            id: r.id,
            name: r.name,
            description: r.description,
            enabled: r.enabled,
            module_id: r.module_id,
            cooldown_secs: r.cooldown_secs.max(0) as u32,
            template: r.template,
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
    /// ok | error | skipped
    pub status: String,
    pub message: String,
    /// Trigger kind, `manual`, `test`, `ui_button` or `confirmed`.
    pub source: String,
    pub duration_ms: i64,
    /// `[{ action, ok, message }]` per executed action.
    pub details_json: String,
    pub created_at: String,
}


pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Automation>> {
    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, name, description, enabled, module_id, trigger_json, conditions_json, actions_json, cooldown_secs, template, last_run_at, created_at, updated_at
         FROM automations ORDER BY position, created_at, rowid",
    )
    .fetch_all(pool)
    .await?;
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
    sqlx::query_as::<_, Row>(
        "SELECT id, name, description, enabled, module_id, trigger_json, conditions_json, actions_json, cooldown_secs, template, last_run_at, created_at, updated_at
         FROM automations WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("автоматизация".into()))?
    .try_into()
}

pub async fn create(pool: &SqlitePool, input: &AutomationInput) -> AppResult<Automation> {
    create_from(pool, input, None).await
}

pub async fn create_from(pool: &SqlitePool, input: &AutomationInput, template: Option<&str>) -> AppResult<Automation> {
    input.validate()?;
    let id = new_id();
    sqlx::query(
        "INSERT INTO automations (id, name, description, enabled, module_id, trigger_json, conditions_json, actions_json, cooldown_secs, template, position)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM automations))",
    )
    .bind(&id)
    .bind(input.name.trim())
    .bind(input.description.trim())
    .bind(input.enabled)
    .bind(input.module_id())
    .bind(serde_json::to_string(&input.trigger)?)
    .bind(serde_json::to_string(&input.conditions)?)
    .bind(serde_json::to_string(&input.actions)?)
    .bind(input.cooldown_secs)
    .bind(template)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn update(pool: &SqlitePool, id: &str, input: &AutomationInput) -> AppResult<Automation> {
    input.validate()?;
    let n = sqlx::query(
        "UPDATE automations SET name = ?, description = ?, enabled = ?, module_id = ?, trigger_json = ?, conditions_json = ?,
            actions_json = ?, cooldown_secs = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(input.name.trim())
    .bind(input.description.trim())
    .bind(input.enabled)
    .bind(input.module_id())
    .bind(serde_json::to_string(&input.trigger)?)
    .bind(serde_json::to_string(&input.conditions)?)
    .bind(serde_json::to_string(&input.actions)?)
    .bind(input.cooldown_secs)
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
    sqlx::query("UPDATE automations SET enabled = ?, updated_at = datetime('now') WHERE id = ?").bind(enabled).bind(id).execute(pool).await?;
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

pub struct LogEntry<'a> {
    pub status: &'a str,
    pub message: &'a str,
    pub source: &'a str,
    pub duration_ms: i64,
    pub details: &'a serde_json::Value,
}

/// Appends a run record, keeping the newest `keep` entries per automation.
pub async fn log_entry(pool: &SqlitePool, id: &str, e: LogEntry<'_>, keep: i64) -> AppResult<()> {
    sqlx::query("INSERT INTO automation_logs (automation_id, status, message, source, duration_ms, details_json) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(id)
        .bind(e.status)
        .bind(e.message)
        .bind(e.source)
        .bind(e.duration_ms)
        .bind(e.details.to_string())
        .execute(pool)
        .await?;
    sqlx::query(
        "DELETE FROM automation_logs WHERE automation_id = ? AND id NOT IN
           (SELECT id FROM automation_logs WHERE automation_id = ? ORDER BY id DESC LIMIT ?)",
    )
    .bind(id)
    .bind(id)
    .bind(keep.max(1))
    .execute(pool)
    .await?;
    Ok(())
}

/// Short form used by hotkeys and older code.
pub async fn log(pool: &SqlitePool, id: &str, status: &str, message: &str, keep: i64) -> AppResult<()> {
    log_entry(pool, id, LogEntry { status, message, source: "system", duration_ms: 0, details: &serde_json::json!([]) }, keep).await
}

pub async fn logs(pool: &SqlitePool, id: Option<&str>, limit: i64) -> AppResult<Vec<AutomationLog>> {
    let limit = limit.clamp(1, 500);
    Ok(sqlx::query_as::<_, AutomationLog>(
        "SELECT id, automation_id, status, message, source, duration_ms, details_json, created_at FROM automation_logs
         WHERE (?1 IS NULL OR automation_id = ?1) ORDER BY id DESC LIMIT ?2",
    )
    .bind(id)
    .bind(limit)
    .fetch_all(pool)
    .await?)
}

/// Moves an automation to a new index in the list.
pub async fn reorder(pool: &SqlitePool, ids: &[String]) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    for (pos, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE automations SET position = ? WHERE id = ?").bind(pos as i64).bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::types::{Action, Condition, Trigger};

    fn input() -> AutomationInput {
        AutomationInput {
            name: "Перерыв".into(),
            description: "Напоминание".into(),
            enabled: true,
            module_id: None,
            trigger: Trigger::Interval { minutes: 50 },
            conditions: vec![Condition::Weekday { days: vec![1, 2, 3, 4, 5] }],
            actions: vec![Action::Notify { title: "Пора отдохнуть".into(), body: "".into() }, Action::RunCommand { command: "echo hi".into() }],
            cooldown_secs: 60,
        }
    }

    #[tokio::test]
    async fn crud_and_logs() {
        let pool = crate::db::test_pool().await;
        let a = create(&pool, &input()).await.unwrap();
        assert_eq!(a.trigger, Trigger::Interval { minutes: 50 });
        assert_eq!(a.conditions.len(), 1);
        assert_eq!(a.cooldown_secs, 60);
        assert_eq!(a.dangerous, ["команда терминала"]);
        assert_eq!(a.permissions.len(), 2);

        let mut changed = input();
        changed.name = "Длинный перерыв".into();
        assert_eq!(update(&pool, &a.id, &changed).await.unwrap().name, "Длинный перерыв");
        assert!(!set_enabled(&pool, &a.id, false).await.unwrap().enabled);

        let b = create(&pool, &input()).await.unwrap();
        reorder(&pool, &[b.id.clone(), a.id.clone()]).await.unwrap();
        assert_eq!(list(&pool).await.unwrap()[0].id, b.id);

        log(&pool, &a.id, "ok", "done", 500).await.unwrap();
        assert_eq!(logs(&pool, Some(&a.id), 10).await.unwrap().len(), 1);
        assert_eq!(logs(&pool, None, 10).await.unwrap().len(), 1);

        delete(&pool, &a.id).await.unwrap();
        assert_eq!(list(&pool).await.unwrap().len(), 1);
        assert!(logs(&pool, Some(&a.id), 10).await.unwrap().is_empty(), "logs cascade on delete");
    }

    #[tokio::test]
    async fn unreadable_rows_are_skipped() {
        let pool = crate::db::test_pool().await;
        sqlx::query("INSERT INTO automations (id, name, trigger_json, actions_json) VALUES ('x', 'old', '{\"type\":\"gone\"}', '[]')").execute(&pool).await.unwrap();
        assert!(list(&pool).await.unwrap().is_empty());
    }
}
