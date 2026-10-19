//! SQLite connection pool and embedded migrations (`db/migrations`).

use std::path::Path;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

pub const DB_FILE: &str = "focusdeck.sqlite";

/// Opens (creating if needed) `dir/focusdeck.sqlite` and applies migrations.
pub async fn connect(dir: &Path) -> anyhow::Result<SqlitePool> {
    std::fs::create_dir_all(dir)?;
    // `filename` instead of a sqlite:// URL: Windows paths don't round-trip through URLs.
    let options = SqliteConnectOptions::new()
        .filename(dir.join(DB_FILE))
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    migrate(&pool).await?;
    Ok(pool)
}

pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./src/db/migrations").run(pool).await?;
    Ok(())
}

/// In-memory database with migrations applied, for tests.
#[cfg(test)]
pub async fn test_pool() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .in_memory(true)
        .foreign_keys(true);
    // One connection: every in-memory connection is a separate database.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    pool
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_create_all_tables() {
        let pool = test_pool().await;
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != '_sqlx_migrations' ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        let names: Vec<&str> = tables.iter().map(|t| t.0.as_str()).collect();
        assert_eq!(
            names,
            [
                "automation_logs",
                "automations",
                "clipboard_history",
                "connector_settings",
                "connectors",
                "module_logs",
                "module_settings",
                "modules",
                "notes",
                "permission_logs",
                "permissions",
                "settings",
                "snippets",
                "tasks",
                "widget_layouts",
                "widgets",
            ]
        );
    }
}
