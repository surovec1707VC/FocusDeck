//! SQLite storage: connection pool and embedded migrations.

pub mod models;

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;

pub const DB_FILE: &str = "focusdeck.db";

/// Opens (creating if needed) the database at `dir/focusdeck.db` and applies
/// pending migrations from `src/db/migrations`.
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

    sqlx::migrate!("./src/db/migrations").run(&pool).await?;
    Ok(pool)
}
