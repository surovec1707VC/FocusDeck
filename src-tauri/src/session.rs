//! Session safety: crash detection and database backup/restore.
//!
//! - `session.lock` is created at start and removed on a clean exit. If it is
//!   still there at the next start, the previous session crashed (or was
//!   killed). A panic hook also writes `last_crash.txt` with the message.
//! - Backup: `VACUUM INTO` a file the user picked (a consistent copy).
//! - Restore: the chosen file is checked and staged as `focusdeck.restore.sqlite`;
//!   on the next start it replaces the database. The old database is kept
//!   next to it as `focusdeck.before-restore-<time>.sqlite`, never deleted.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Connection};
use tauri::{AppHandle, Manager};

use crate::db::DB_FILE;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

const LOCK: &str = "session.lock";
const CRASH: &str = "last_crash.txt";
const RESTORE: &str = "focusdeck.restore.sqlite";

#[derive(Debug, Clone, Default, Serialize)]
pub struct SessionStatus {
    /// The previous session did not exit cleanly.
    pub previous_crash: bool,
    /// Panic message of the previous session, if any.
    pub crash_message: Option<String>,
    /// A backup was restored at this start.
    pub restored: bool,
    /// A restore waits for the next start.
    pub restore_pending: bool,
}

static STATUS: OnceLock<std::sync::Mutex<SessionStatus>> = OnceLock::new();

fn status() -> &'static std::sync::Mutex<SessionStatus> {
    STATUS.get_or_init(Default::default)
}

/// Before the database opens: apply a staged restore, detect a crash, take the lock.
pub fn start(dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let restored = apply_restore(dir)?;
    let lock = dir.join(LOCK);
    let crash_file = dir.join(CRASH);
    let previous_crash = lock.exists();
    let crash_message = if previous_crash { std::fs::read_to_string(&crash_file).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) } else { None };
    let _ = std::fs::remove_file(&crash_file);
    std::fs::write(&lock, std::process::id().to_string())?;
    *status().lock().unwrap() = SessionStatus { previous_crash, crash_message, restored, restore_pending: false };

    let crash_path = crash_file.clone();
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = std::fs::write(&crash_path, format!("{info}"));
        default_hook(info);
    }));
    if previous_crash {
        tracing::warn!("previous session did not exit cleanly");
    }
    Ok(())
}

/// Clean exit: the lock goes away.
pub fn finish(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(LOCK));
}

fn stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Swaps the staged file in. The current database (with its -wal/-shm) is renamed, not deleted.
fn apply_restore(dir: &Path) -> anyhow::Result<bool> {
    let staged = dir.join(RESTORE);
    if !staged.exists() {
        return Ok(false);
    }
    let db = dir.join(DB_FILE);
    let keep = format!("focusdeck.before-restore-{}.sqlite", stamp());
    for suffix in ["", "-wal", "-shm"] {
        let from = PathBuf::from(format!("{}{suffix}", db.display()));
        if from.exists() {
            std::fs::rename(&from, dir.join(format!("{keep}{suffix}")))?;
        }
    }
    std::fs::rename(&staged, &db)?;
    tracing::info!("database restored from backup; previous kept as {keep}");
    Ok(true)
}

/// Opens a file read-only and checks it is a FocusDeck database.
pub async fn check_backup(path: &Path) -> AppResult<()> {
    let opts = SqliteConnectOptions::new().filename(path).read_only(true);
    let mut conn = opts.connect().await.map_err(|e| AppError::Invalid(format!("это не база SQLite: {e}")))?;
    let (ok,): (String,) = sqlx::query_as("PRAGMA quick_check").fetch_one(&mut conn).await.map_err(|e| AppError::Invalid(format!("файл повреждён: {e}")))?;
    if ok != "ok" {
        return Err(AppError::Invalid(format!("файл повреждён: {ok}")));
    }
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('_sqlx_migrations', 'settings', 'modules')")
        .fetch_one(&mut conn)
        .await?;
    conn.close().await.ok();
    if n != 3 {
        return Err(AppError::Invalid("это не резервная копия FocusDeck".into()));
    }
    Ok(())
}

fn data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(crate::paths::data_dir(app)?)
}

#[tauri::command]
pub fn session_status() -> SessionStatus {
    status().lock().unwrap().clone()
}

/// The user has seen the crash notice.
#[tauri::command]
pub fn session_ack() {
    let mut s = status().lock().unwrap();
    s.previous_crash = false;
    s.crash_message = None;
    s.restored = false;
}

/// Saves a consistent copy of the database; returns the path or None if cancelled.
#[tauri::command]
pub async fn backup_create(app: AppHandle) -> AppResult<Option<String>> {
    let name = format!("focusdeck-backup-{}.sqlite", stamp());
    let Some(path) = crate::productivity::pick_save_path(&app, "Резервная копия FocusDeck", name, ("SQLite", &["sqlite", "db"])).await? else {
        return Ok(None);
    };
    // VACUUM INTO refuses an existing file: write next to it, then replace (the user chose to overwrite in the dialog).
    let tmp = path.with_extension("sqlite.tmp");
    let _ = std::fs::remove_file(&tmp);
    sqlx::query("VACUUM INTO ?").bind(tmp.display().to_string()).execute(&app.state::<AppState>().db).await?;
    std::fs::rename(&tmp, &path).map_err(anyhow::Error::from)?;
    tracing::info!("backup saved to {}", path.display());
    Ok(Some(path.display().to_string()))
}

/// Picks a backup, checks it and stages it for the next start (needs `confirm`).
#[tauri::command]
pub async fn backup_restore(app: AppHandle, confirm: bool) -> AppResult<Option<String>> {
    if !confirm {
        return Err(AppError::ConfirmationRequired(
            "Все текущие данные FocusDeck будут заменены данными из резервной копии. Текущая база сохранится рядом с новой. Продолжить?".into(),
        ));
    }
    let handle = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        use tauri_plugin_dialog::DialogExt;
        handle.dialog().file().set_title("Восстановить из резервной копии").add_filter("SQLite", &["sqlite", "db"]).blocking_pick_file()
    })
    .await
    .map_err(anyhow::Error::from)?;
    let Some(picked) = picked else { return Ok(None) };
    let path = picked.into_path().map_err(anyhow::Error::from)?;
    stage_restore(&app, &path).await?;
    Ok(Some(path.display().to_string()))
}

/// Checks a backup and stages it to replace the database at the next start.
pub async fn stage_restore(app: &AppHandle, path: &Path) -> AppResult<()> {
    check_backup(path).await?;
    std::fs::copy(path, data_dir(app)?.join(RESTORE)).map_err(anyhow::Error::from)?;
    status().lock().unwrap().restore_pending = true;
    tracing::info!("restore staged from {}", path.display());
    Ok(())
}

/// Restarts FocusDeck (used to apply a staged restore).
#[tauri::command]
pub fn app_restart(app: AppHandle) {
    if let Ok(dir) = data_dir(&app) {
        finish(&dir);
    }
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("focusdeck-test-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[tokio::test]
    async fn crash_detection_backup_and_restore() {
        let dir = temp_dir("session");
        start(&dir).unwrap();
        assert!(!session_status().previous_crash);
        // No finish(): the next start sees a crash.
        start(&dir).unwrap();
        assert!(session_status().previous_crash);
        finish(&dir);
        start(&dir).unwrap();
        assert!(!session_status().previous_crash);
        finish(&dir);

        // A real database passes the check, a random file does not.
        let pool = crate::db::connect(&dir).await.unwrap();
        let backup = dir.join("b.sqlite");
        sqlx::query("VACUUM INTO ?").bind(backup.display().to_string()).execute(&pool).await.unwrap();
        pool.close().await;
        check_backup(&backup).await.unwrap();
        let junk = dir.join("junk.sqlite");
        std::fs::write(&junk, b"not a database").unwrap();
        assert!(check_backup(&junk).await.is_err());

        // Staged restore swaps files and keeps the old database.
        std::fs::copy(&backup, dir.join(RESTORE)).unwrap();
        assert!(apply_restore(&dir).unwrap());
        assert!(!dir.join(RESTORE).exists());
        assert!(dir.join(DB_FILE).exists());
        let kept = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).any(|e| e.file_name().to_string_lossy().starts_with("focusdeck.before-restore-"));
        assert!(kept);
        assert!(!apply_restore(&dir).unwrap());
    }
}
