//! Chat sessions and their messages, persisted in `ai_sessions` / `ai_messages`.

use sqlx::SqlitePool;

use super::providers::{ChatMessage, Role};
use crate::db::models::{AiMessageRow, AiSessionRow};

pub async fn create(
    pool: &SqlitePool,
    title: &str,
    profile_id: &str,
    provider: &str,
    model: &str,
) -> sqlx::Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO ai_sessions (id, title, profile_id, provider, model) VALUES (?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(title)
        .bind(profile_id)
        .bind(provider)
        .bind(model)
        .execute(pool)
        .await?;
    Ok(id)
}

pub async fn list(pool: &SqlitePool) -> sqlx::Result<Vec<AiSessionRow>> {
    sqlx::query_as::<_, AiSessionRow>("SELECT * FROM ai_sessions ORDER BY updated_at DESC LIMIT 200")
        .fetch_all(pool)
        .await
}

pub async fn messages(pool: &SqlitePool, session_id: &str) -> sqlx::Result<Vec<AiMessageRow>> {
    sqlx::query_as::<_, AiMessageRow>(
        "SELECT * FROM ai_messages WHERE session_id = ? ORDER BY created_at, rowid",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
}

pub async fn add_message(pool: &SqlitePool, session_id: &str, role: Role, content: &str) -> sqlx::Result<()> {
    let role = match role {
        Role::User => "user",
        Role::Assistant => "assistant",
    };
    sqlx::query("INSERT INTO ai_messages (id, session_id, role, content) VALUES (?, ?, ?, ?)")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(session_id)
        .bind(role)
        .bind(content)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE ai_sessions SET updated_at = datetime('now') WHERE id = ?")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, session_id: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM ai_sessions WHERE id = ?")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Converts stored rows into provider messages (system/tool rows are skipped).
pub fn to_chat_messages(rows: &[AiMessageRow]) -> Vec<ChatMessage> {
    rows.iter()
        .filter_map(|r| {
            let role = match r.role.as_str() {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                _ => return None,
            };
            Some(ChatMessage { role, content: r.content.clone() })
        })
        .collect()
}
