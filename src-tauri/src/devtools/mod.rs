//! Developer Tools: Git, Project Launcher, Terminal Quick Launcher, API
//! Tester, Regex Tester, JSON/YAML Tools. Each tool is its own module in the
//! registry; its commands fail with `module_disabled` while it is off.
//!
//! Actions that change something (git stage/commit/push/pull, launching
//! editors and terminals, running commands, sending HTTP requests) go through
//! `safety::authorize` (permission → Safe Mode → confirmation) and are written
//! to the Action Log. Programs are started from Rust with explicit arguments;
//! the UI never gets a shell.

pub mod api;
pub mod git;
pub mod jsonyaml;
pub mod projects;
pub mod regex;
pub mod terminal;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::modules::registry;
use crate::safety::{self, ActionDef};
use crate::state::AppState;

pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
pub const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
/// Output kept from a program run inside FocusDeck.
pub const MAX_OUTPUT: usize = 1024 * 1024;

pub fn require(app: &AppHandle, module: &str) -> AppResult<()> {
    crate::productivity::require(&app.state::<AppState>(), module)
}

pub async fn setting_u64(app: &AppHandle, module: &str, key: &str, default: u64) -> u64 {
    let db = &app.state::<AppState>().db;
    registry::settings(db, module).await.ok().and_then(|s| s.get(key).and_then(Value::as_u64)).unwrap_or(default)
}

pub async fn setting_str(app: &AppHandle, module: &str, key: &str) -> String {
    let db = &app.state::<AppState>().db;
    registry::settings(db, module).await.ok().and_then(|s| s.get(key).and_then(Value::as_str).map(|v| v.trim().to_string())).unwrap_or_default()
}

/// Module on → authorize → run → Action Log.
pub async fn guarded<T, Fut>(app: &AppHandle, module: &str, action: &str, target: &str, confirm: bool, run: Fut, message: impl FnOnce(&T) -> String) -> AppResult<T>
where
    Fut: std::future::Future<Output = AppResult<T>>,
{
    require(app, module)?;
    let db = &app.state::<AppState>().db;
    let def: &'static ActionDef = safety::authorize(db, action, target, confirm).await?;
    let result = run.await;
    safety::finish(app, def, target, &result, message, |_| None).await;
    result
}

/// An existing absolute directory.
pub fn existing_dir(path: &str) -> AppResult<PathBuf> {
    let p = PathBuf::from(path.trim());
    if path.trim().is_empty() || !p.is_absolute() {
        return Err(AppError::Invalid("нужен полный путь к папке".into()));
    }
    if !p.is_dir() {
        return Err(AppError::NotFound(format!("папка {}", p.display())));
    }
    Ok(p)
}

/// Finds an executable in PATH (with PATHEXT-style extensions on Windows).
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: &[&str] = if cfg!(windows) { &["", ".exe", ".cmd", ".bat"] } else { &[""] };
    std::env::split_paths(&path).find_map(|dir| exts.iter().map(|e| dir.join(format!("{name}{e}"))).find(|p| p.is_file()))
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProcOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub duration_ms: i64,
}

/// Runs a program with explicit arguments in a hidden window and collects
/// its output (UTF-8 or the OEM code page). Killed with its child processes
/// after `timeout`.
pub async fn run_program(program: &Path, args: &[String], cwd: Option<&Path>, env: &[(&str, &str)], timeout: Duration) -> AppResult<ProcOutput> {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    let started = Instant::now();
    let child = cmd.spawn().map_err(|e| AppError::Other(anyhow::anyhow!("не удалось запустить {}: {e}", program.display())))?;
    let pid = child.id();
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => Ok(ProcOutput {
            code: out.status.code(),
            stdout: clip(&out.stdout),
            stderr: clip(&out.stderr),
            timed_out: false,
            duration_ms: started.elapsed().as_millis() as i64,
        }),
        Ok(Err(e)) => Err(AppError::Other(e.into())),
        Err(_) => {
            if let Some(pid) = pid {
                kill_tree(pid).await;
            }
            Ok(ProcOutput { timed_out: true, duration_ms: started.elapsed().as_millis() as i64, ..Default::default() })
        }
    }
}

fn clip(bytes: &[u8]) -> String {
    let s = crate::clipboard_hub::actions::decode(&bytes[..bytes.len().min(MAX_OUTPUT)]);
    if bytes.len() > MAX_OUTPUT { s + "\n… (вывод обрезан)" } else { s }
}

/// Ends a process and everything it started (`taskkill /T /F`).
pub async fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        let mut cmd = tokio::process::Command::new("taskkill");
        cmd.args(["/T", "/F", "/PID", &pid.to_string()]).creation_flags(CREATE_NO_WINDOW);
        cmd.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        let _ = cmd.status().await;
    }
    #[cfg(not(windows))]
    let _ = pid;
}

/// Streaming decoder for program output: UTF-8 (a character split between
/// chunks is carried over), falling back to the OEM code page.
#[derive(Default)]
pub struct Decoder {
    carry: Vec<u8>,
}

impl Decoder {
    pub fn push(&mut self, bytes: &[u8]) -> String {
        let mut buf = std::mem::take(&mut self.carry);
        buf.extend_from_slice(bytes);
        match std::str::from_utf8(&buf) {
            Ok(s) => s.to_string(),
            Err(e) if e.error_len().is_none() => {
                let valid = e.valid_up_to();
                self.carry = buf[valid..].to_vec();
                String::from_utf8_lossy(&buf[..valid]).into_owned()
            }
            Err(_) => crate::clipboard_hub::actions::decode(&buf),
        }
    }

    pub fn finish(&mut self) -> String {
        let rest = std::mem::take(&mut self.carry);
        if rest.is_empty() { String::new() } else { crate::clipboard_hub::actions::decode(&rest) }
    }
}

/// Spawns a program in its own console window (terminals, project commands).
/// `raw_tail` is appended unquoted (cmd.exe parses its own command line).
pub fn spawn_console(program: &Path, args: &[String], raw_tail: Option<&str>, cwd: &Path, env: &[(&str, &str)], new_console: bool) -> AppResult<()> {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args).current_dir(cwd);
    #[cfg(windows)]
    if let Some(raw) = raw_tail {
        use std::os::windows::process::CommandExt;
        cmd.raw_arg(raw);
    }
    #[cfg(not(windows))]
    if let Some(raw) = raw_tail {
        cmd.arg(raw);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(if new_console { CREATE_NEW_CONSOLE } else { CREATE_NO_WINDOW });
    }
    #[cfg(not(windows))]
    let _ = new_console;
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    cmd.spawn().map_err(|e| AppError::Other(anyhow::anyhow!("не удалось запустить {}: {e}", program.display())))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_carries_split_characters() {
        let bytes = "привет".as_bytes();
        let mut d = Decoder::default();
        let mut out = d.push(&bytes[..3]);
        out += &d.push(&bytes[3..]);
        out += &d.finish();
        assert_eq!(out, "привет");
    }

    #[test]
    fn existing_dir_checks() {
        assert!(existing_dir("relative").is_err());
        assert!(existing_dir("").is_err());
        let tmp = std::env::temp_dir();
        assert!(existing_dir(&tmp.to_string_lossy()).is_ok());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn program_output_and_timeout() {
        let cmd = which("cmd").unwrap();
        let out = run_program(&cmd, &["/D".into(), "/C".into(), "echo ok".into()], None, &[], Duration::from_secs(20)).await.unwrap();
        assert_eq!(out.stdout.trim(), "ok");
        assert_eq!(out.code, Some(0));
        let slow = run_program(&cmd, &["/D".into(), "/C".into(), "ping -n 5 127.0.0.1 >nul".into()], None, &[], Duration::from_millis(300)).await.unwrap();
        assert!(slow.timed_out);
    }
}
