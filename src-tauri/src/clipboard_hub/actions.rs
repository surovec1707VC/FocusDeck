//! Quick actions: user-made buttons that open an app, URL, folder or file,
//! paste or copy text, or run a terminal command (always after explicit
//! confirmation and with the «Выполнение команд» permission).

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::db::new_id;
use crate::error::{AppError, AppResult};
use crate::modules::{catalog::CLIPBOARD, registry};
use crate::permissions::{self, PERM_LAUNCH_APPS, PERM_PASTE, PERM_RUN_COMMANDS};
use crate::state::AppState;
use crate::widgets::{valid_app_path, valid_link};

pub const KINDS: &[&str] = &["open_app", "open_url", "open_folder", "open_file", "paste_text", "copy_text", "command"];
/// Files that start programs: opening them needs the «Запуск программ» permission.
const EXECUTABLE: &[&str] = &["exe", "lnk", "bat", "cmd", "ps1", "vbs", "js", "msi", "com", "scr", "url", "appref-ms"];
const MAX_OUTPUT: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct QuickAction {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub target: String,
    pub hotkey: Option<String>,
    pub position: i64,
    pub last_run_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ActionInput {
    pub name: String,
    pub kind: String,
    pub target: String,
    #[serde(default)]
    pub hotkey: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct CommandOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct RunResult {
    /// Only for `command`.
    pub output: Option<CommandOutput>,
}

pub fn validate(i: &ActionInput) -> AppResult<()> {
    let n = i.name.trim().chars().count();
    if n == 0 || n > 60 {
        return Err(AppError::Invalid("название действия — от 1 до 60 символов".into()));
    }
    if !KINDS.contains(&i.kind.as_str()) {
        return Err(AppError::Invalid("неизвестный тип действия".into()));
    }
    let t = i.target.trim();
    match i.kind.as_str() {
        "open_app" => valid_app_path(t)?,
        "open_url" if !valid_link(t) => return Err(AppError::Invalid("адрес должен начинаться с http://, https:// или mailto:".into())),
        "open_folder" | "open_file" if !Path::new(t).is_absolute() => return Err(AppError::Invalid("укажите полный путь".into())),
        "paste_text" | "copy_text" if i.target.is_empty() || i.target.chars().count() > 50_000 => {
            return Err(AppError::Invalid("текст — от 1 до 50 000 символов".into()));
        }
        "command" if t.is_empty() || t.chars().count() > 2000 || t.contains('\0') => {
            return Err(AppError::Invalid("команда — от 1 до 2000 символов".into()));
        }
        _ => {}
    }
    if i.kind == "command" && i.hotkey.as_ref().is_some_and(|h| !h.trim().is_empty()) {
        return Err(AppError::Invalid("команде нельзя назначить горячую клавишу: её запуск всегда подтверждается в окне".into()));
    }
    Ok(())
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<QuickAction>> {
    Ok(sqlx::query_as::<_, QuickAction>("SELECT id, name, kind, target, hotkey, position, last_run_at FROM quick_actions ORDER BY position, created_at")
        .fetch_all(pool)
        .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<QuickAction> {
    list(pool).await?.into_iter().find(|a| a.id == id).ok_or_else(|| AppError::NotFound("быстрое действие".into()))
}

pub async fn save(pool: &SqlitePool, id: Option<&str>, i: ActionInput) -> AppResult<QuickAction> {
    validate(&i)?;
    let hotkey = i.hotkey.as_deref().map(str::trim).filter(|h| !h.is_empty());
    // Text is kept as typed; paths, URLs and commands are trimmed.
    let target = if i.kind.ends_with("_text") { i.target.clone() } else { i.target.trim().to_string() };
    let id = match id {
        Some(id) => {
            get(pool, id).await?;
            sqlx::query("UPDATE quick_actions SET name = ?, kind = ?, target = ?, hotkey = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(i.name.trim())
                .bind(&i.kind)
                .bind(&target)
                .bind(hotkey)
                .bind(id)
                .execute(pool)
                .await?;
            id.to_string()
        }
        None => {
            let id = new_id();
            sqlx::query(
                "INSERT INTO quick_actions (id, name, kind, target, hotkey, position)
                 VALUES (?, ?, ?, ?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM quick_actions))",
            )
            .bind(&id)
            .bind(i.name.trim())
            .bind(&i.kind)
            .bind(&target)
            .bind(hotkey)
            .execute(pool)
            .await?;
            id
        }
    };
    get(pool, &id).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM quick_actions WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

/// Moves an action one place up (-1) or down (+1).
pub async fn reorder(pool: &SqlitePool, id: &str, delta: i64) -> AppResult<()> {
    let mut all = list(pool).await?;
    let Some(i) = all.iter().position(|a| a.id == id) else { return Err(AppError::NotFound("быстрое действие".into())) };
    let j = i as i64 + delta.signum();
    if j < 0 || j >= all.len() as i64 {
        return Ok(());
    }
    all.swap(i, j as usize);
    let mut tx = pool.begin().await?;
    for (pos, a) in all.iter().enumerate() {
        sqlx::query("UPDATE quick_actions SET position = ? WHERE id = ?").bind(pos as i64).bind(&a.id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Runs an action. `command` needs `confirm: true` (asked in the UI every
/// time) and the «Выполнение команд» permission.
pub async fn run(app: &AppHandle, id: &str, confirm: bool) -> AppResult<RunResult> {
    let db = &app.state::<AppState>().db;
    let a = get(db, id).await?;
    let source = format!("quick_action:{}", a.id);
    let mut result = RunResult::default();
    match a.kind.as_str() {
        "open_app" => {
            permissions::require(db, PERM_LAUNCH_APPS, &source).await?;
            valid_app_path(&a.target)?;
            if !Path::new(&a.target).exists() {
                return Err(AppError::NotFound(format!("файл {}", a.target)));
            }
            app.opener().open_path(&a.target, None::<&str>).map_err(anyhow::Error::from)?;
        }
        "open_url" => {
            if !valid_link(&a.target) {
                return Err(AppError::Invalid("адрес не подходит".into()));
            }
            app.opener().open_url(&a.target, None::<&str>).map_err(anyhow::Error::from)?;
        }
        "open_folder" | "open_file" => {
            let p = Path::new(&a.target);
            let ok = if a.kind == "open_folder" { p.is_dir() } else { p.is_file() };
            if !ok {
                return Err(AppError::NotFound(a.target.clone()));
            }
            let ext = p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
            if a.kind == "open_file" && EXECUTABLE.contains(&ext.as_str()) {
                permissions::require(db, PERM_LAUNCH_APPS, &source).await?;
            }
            app.opener().open_path(&a.target, None::<&str>).map_err(anyhow::Error::from)?;
        }
        "copy_text" => super::copy_text(&a.target).await?,
        "paste_text" => {
            permissions::require(db, PERM_PASTE, &source).await?;
            super::copy_text(&a.target).await?;
            super::paste_into_previous(app).await?;
        }
        "command" => {
            if !confirm {
                return Err(AppError::ConfirmationRequired(format!("Выполнить команду «{}»?", a.target)));
            }
            permissions::require(db, PERM_RUN_COMMANDS, &source).await?;
            let timeout = registry::settings(db, CLIPBOARD).await?.get("command_timeout_secs").and_then(Value::as_u64).unwrap_or(60);
            tracing::info!(action = %a.name, "running quick action command");
            result.output = Some(run_command(&a.target, Duration::from_secs(timeout)).await?);
        }
        other => return Err(AppError::Invalid(format!("неизвестный тип действия {other}"))),
    }
    sqlx::query("UPDATE quick_actions SET last_run_at = datetime('now') WHERE id = ?").bind(id).execute(db).await?;
    Ok(result)
}

/// Console programs write UTF-8 or the OEM code page (cp866 on Russian Windows).
fn decode(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Globalization::{CP_OEMCP, MultiByteToWideChar};
        // SAFETY: sizes come from the input; the output buffer is sized by the first call.
        unsafe {
            let n = MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), bytes.len() as i32, std::ptr::null_mut(), 0);
            if n > 0 {
                let mut wide = vec![0u16; n as usize];
                MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), bytes.len() as i32, wide.as_mut_ptr(), n);
                return String::from_utf16_lossy(&wide);
            }
        }
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn truncate(bytes: &[u8]) -> String {
    let s = decode(&bytes[..bytes.len().min(MAX_OUTPUT)]);
    if bytes.len() > MAX_OUTPUT { s + "\n… (вывод обрезан)" } else { s }
}

/// `cmd /C` in a hidden window; output decoded from UTF-8 or the OEM code page; killed after `timeout`.
async fn run_command(command: &str, timeout: Duration) -> AppResult<CommandOutput> {
    let mut cmd = tokio::process::Command::new("cmd");
    cmd.arg("/D").arg("/C");
    // Raw: cmd parses its own command line, Rust's quoting would break it.
    #[cfg(windows)]
    cmd.raw_arg(command);
    #[cfg(not(windows))]
    cmd.arg(command);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    let child = cmd.spawn().map_err(|e| AppError::Other(anyhow::anyhow!("не удалось запустить cmd: {e}")))?;
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => Ok(CommandOutput { code: out.status.code(), stdout: truncate(&out.stdout), stderr: truncate(&out.stderr), timed_out: false }),
        Ok(Err(e)) => Err(AppError::Other(e.into())),
        // Dropping the future kills the process (kill_on_drop).
        Err(_) => Ok(CommandOutput { timed_out: true, ..Default::default() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(kind: &str, target: &str) -> ActionInput {
        ActionInput { name: "Действие".into(), kind: kind.into(), target: target.into(), hotkey: None }
    }

    #[test]
    fn validation() {
        assert!(validate(&input("open_url", "https://example.com")).is_ok());
        assert!(validate(&input("open_url", "file:///C:/")).is_err());
        assert!(validate(&input("open_app", r"C:\Windows\notepad.exe")).is_ok());
        assert!(validate(&input("open_app", r"C:\run.bat")).is_err());
        assert!(validate(&input("open_folder", "relative\\path")).is_err());
        assert!(validate(&input("copy_text", "")).is_err());
        assert!(validate(&input("launch_rocket", "x")).is_err());
        let with_hotkey = ActionInput { hotkey: Some("Ctrl+Alt+K".into()), ..input("command", "dir") };
        assert!(validate(&with_hotkey).is_err(), "commands can't have hotkeys");
    }

    #[tokio::test]
    async fn crud_and_order() {
        let pool = crate::db::test_pool().await;
        let a = save(&pool, None, input("copy_text", "  текст с пробелами ")).await.unwrap();
        assert_eq!(a.target, "  текст с пробелами ");
        let b = save(&pool, None, input("open_url", " https://example.com ")).await.unwrap();
        assert_eq!(b.target, "https://example.com");
        reorder(&pool, &b.id, -1).await.unwrap();
        assert_eq!(list(&pool).await.unwrap()[0].id, b.id);
        save(&pool, Some(&a.id), ActionInput { hotkey: Some(" ".into()), ..input("copy_text", "x") }).await.unwrap();
        assert!(get(&pool, &a.id).await.unwrap().hotkey.is_none());
        delete(&pool, &a.id).await.unwrap();
        assert_eq!(list(&pool).await.unwrap().len(), 1);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn command_output_and_timeout() {
        let out = run_command("echo привет", Duration::from_secs(20)).await.unwrap();
        assert_eq!(out.code, Some(0));
        assert_eq!(out.stdout.trim(), "привет");
        let slow = run_command("ping -n 5 127.0.0.1 >nul", Duration::from_millis(300)).await.unwrap();
        assert!(slow.timed_out);
    }
}
