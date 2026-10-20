//! Terminal Quick Launcher: PowerShell, CMD, Git Bash, WSL and a custom shell.
//!
//! Two ways to use a shell:
//! - `terminal_open` starts it in its own console window (`terminal.open`,
//!   permission «Запуск программ»);
//! - `terminal_run` runs one command inside a FocusDeck tab without
//!   interactive input (`terminal.run`, permission «Выполнение команд»). The
//!   output is streamed as `focusdeck://terminal-output` events; the command
//!   can be stopped (its whole process tree is ended). `cd` is handled by
//!   FocusDeck, so each tab keeps its own folder.
//!
//! Commands are saved to the history (module setting) and can be kept as
//! favourites.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncReadExt;

use super::{guarded, kill_tree, require, setting_str, setting_u64, spawn_console, which, Decoder, CREATE_NO_WINDOW, MAX_OUTPUT};
use crate::db::new_id;
use crate::error::{AppError, AppResult};
use crate::modules::{catalog::DEV_TERMINAL, registry};
use crate::state::AppState;

pub const OUTPUT_EVENT: &str = "focusdeck://terminal-output";
pub const SHELLS: &[&str] = &["powershell", "cmd", "git_bash", "wsl", "custom"];

/// Running tab commands: run id → stop signal.
static RUNNING: LazyLock<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>> = LazyLock::new(Default::default);

#[derive(Debug, Clone, Serialize)]
pub struct Shell {
    pub id: &'static str,
    pub name: String,
    pub path: Option<String>,
    pub available: bool,
}

fn system32(exe: &str) -> PathBuf {
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    root.join("System32").join(exe)
}

/// Git for Windows install folder (has `bin\bash.exe`).
fn git_root() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)"].iter().filter_map(|v| std::env::var_os(v)).map(|p| PathBuf::from(p).join("Git")).collect();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join("Programs").join("Git"));
    }
    // git.exe in PATH lives in <root>\cmd or <root>\bin or <root>\mingw64\bin.
    if let Some(git) = which("git") {
        candidates.extend(git.ancestors().skip(2).take(3).map(Path::to_path_buf));
    }
    candidates.into_iter().find(|r| r.join("bin").join("bash.exe").is_file())
}

fn powershell() -> PathBuf {
    let p = system32(r"WindowsPowerShell\v1.0\powershell.exe");
    if p.is_file() { p } else { PathBuf::from("powershell.exe") }
}

pub async fn shells(app: &AppHandle) -> Vec<Shell> {
    let custom = setting_str(app, DEV_TERMINAL, "custom_shell_path").await;
    let git = git_root();
    let wsl = system32("wsl.exe");
    let ps = powershell();
    let cmd = system32("cmd.exe");
    let sh = |id, name: &str, path: Option<PathBuf>| {
        let available = path.as_ref().is_some_and(|p| p.is_file());
        Shell { id, name: name.into(), path: path.map(|p| p.display().to_string()), available }
    };
    vec![
        sh("powershell", "PowerShell", Some(ps)),
        sh("cmd", "CMD", Some(cmd)),
        sh("git_bash", "Git Bash", git.map(|g| g.join("bin").join("bash.exe"))),
        sh("wsl", "WSL", Some(wsl)),
        sh("custom", "Свой shell", (!custom.is_empty()).then(|| PathBuf::from(&custom))),
    ]
}

async fn shell_path(app: &AppHandle, id: &str) -> AppResult<PathBuf> {
    if !SHELLS.contains(&id) {
        return Err(AppError::Invalid(format!("неизвестный shell {id}")));
    }
    let s = shells(app).await.into_iter().find(|s| s.id == id).expect("known shell");
    match (s.available, s.path) {
        (true, Some(p)) => Ok(PathBuf::from(p)),
        _ if id == "custom" => Err(AppError::Invalid("укажите путь к своему shell в настройках модуля".into())),
        _ => Err(AppError::NotFound(format!("{} не установлен", s.name))),
    }
}

fn ps_encoded(script: &str) -> String {
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(utf16)
}

/// Opens the shell in its own console window in `cwd`; with a command, runs
/// it there and keeps the window open.
pub async fn open_window(app: &AppHandle, shell: &str, cwd: &Path, command: Option<&str>) -> AppResult<()> {
    let exe = shell_path(app, shell).await?;
    let command = command.map(str::trim).filter(|c| !c.is_empty());
    match shell {
        "powershell" => {
            let mut args = vec!["-NoLogo".to_string()];
            if let Some(c) = command {
                args.extend(["-NoExit".into(), "-EncodedCommand".into(), ps_encoded(c)]);
            }
            spawn_console(&exe, &args, None, cwd, &[], true)
        }
        "cmd" => {
            let tail = command.map(|c| format!("/K {c}"));
            spawn_console(&exe, &[], tail.as_deref(), cwd, &[], true)
        }
        "git_bash" => {
            let root = exe.parent().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default();
            match command {
                // mintty window, like the «Git Bash» shortcut.
                None if root.join("git-bash.exe").is_file() => spawn_console(&root.join("git-bash.exe"), &[format!("--cd={}", cwd.display())], None, cwd, &[], false),
                None => spawn_console(&exe, &["--login".into(), "-i".into()], None, cwd, &[("CHERE_INVOKING", "1")], true),
                Some(c) => spawn_console(&exe, &["--login".into(), "-i".into(), "-c".into(), format!("{c}; exec bash --login -i")], None, cwd, &[("CHERE_INVOKING", "1")], true),
            }
        }
        "wsl" => {
            let mut args = vec!["--cd".to_string(), cwd.display().to_string()];
            if let Some(c) = command {
                args.extend(["-e".into(), "sh".into(), "-lc".into(), format!("{c}; exec \"${{SHELL:-sh}}\" -l")]);
            }
            spawn_console(&exe, &args, None, cwd, &[], true)
        }
        _ => {
            let mut args: Vec<String> = Vec::new();
            if let Some(c) = command {
                args.extend(setting_str(app, DEV_TERMINAL, "custom_shell_args").await.split_whitespace().map(String::from));
                args.push(c.to_string());
            }
            spawn_console(&exe, &args, None, cwd, &[], true)
        }
    }
}

// --- cd inside tabs ---

/// `cd`, `cd X`, `cd /d X`, `cd..`, `Set-Location X` → the target (empty = home).
pub fn parse_cd(command: &str) -> Option<String> {
    let c = command.trim();
    let lower = c.to_lowercase();
    let rest = if lower == "cd" || lower == "set-location" {
        ""
    } else if lower.starts_with("cd..") || lower.starts_with("cd\\") {
        &c[2..]
    } else if lower.starts_with("cd ") {
        &c[3..]
    } else if lower.starts_with("set-location ") {
        &c[13..]
    } else {
        return None;
    };
    // Another command after cd is run by the shell, not here.
    if rest.contains("&&") || rest.contains(';') || rest.contains('|') || rest.contains('&') {
        return None;
    }
    let rest = rest.trim();
    let rest = rest.strip_prefix("/d ").or_else(|| rest.strip_prefix("/D ")).unwrap_or(rest).trim();
    Some(rest.trim_matches(['"', '\'']).to_string())
}

/// Lexically removes `.` and `..`.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if out.parent().is_some() {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Resolves a `cd` target against the tab folder; Unix paths of Git Bash
/// (`/c/…`) and WSL (`/mnt/c/…`) are mapped to Windows folders.
pub fn resolve_cd(cwd: &Path, target: &str, home: &Path) -> AppResult<PathBuf> {
    let t = target.trim();
    let path = if t.is_empty() || t == "~" {
        home.to_path_buf()
    } else if let Some(rest) = t.strip_prefix("~/").or_else(|| t.strip_prefix("~\\")) {
        home.join(rest)
    } else if let Some(win) = unix_to_windows(t) {
        win
    } else if t.starts_with('/') {
        return Err(AppError::Invalid("во вкладке можно перейти только в папку Windows".into()));
    } else {
        cwd.join(t)
    };
    let path = normalize(&path);
    if !path.is_dir() {
        return Err(AppError::NotFound(format!("папка {}", path.display())));
    }
    Ok(path)
}

fn unix_to_windows(t: &str) -> Option<PathBuf> {
    let rest = t.strip_prefix("/mnt/").or_else(|| t.strip_prefix('/'))?;
    let mut parts = rest.splitn(2, '/');
    let drive = parts.next()?;
    if drive.len() != 1 || !drive.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let tail = parts.next().unwrap_or("").replace('/', "\\");
    Some(PathBuf::from(format!("{}:\\{tail}", drive.to_ascii_uppercase())))
}

fn home() -> PathBuf {
    std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\"))
}

// --- running commands in tabs ---

#[derive(Debug, Clone, Serialize)]
pub struct TabRun {
    pub run_id: String,
    pub code: Option<i32>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub duration_ms: i64,
    /// The tab folder after the command (changes only with `cd`).
    pub cwd: String,
}

async fn build(app: &AppHandle, shell: &str, cwd: &Path, command: &str) -> AppResult<tokio::process::Command> {
    let exe = shell_path(app, shell).await?;
    let mut cmd = tokio::process::Command::new(&exe);
    cmd.current_dir(cwd);
    match shell {
        "powershell" => {
            let script = format!(
                "$ProgressPreference='SilentlyContinue';[Console]::OutputEncoding=[Text.Encoding]::UTF8;$OutputEncoding=[Text.Encoding]::UTF8\n{command}\nif ($LASTEXITCODE) {{ exit $LASTEXITCODE }} elseif (-not $?) {{ exit 1 }}"
            );
            cmd.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &ps_encoded(&script)]);
        }
        "cmd" => {
            #[cfg(windows)]
            cmd.raw_arg(format!("/D /S /C \"{command}\""));
            #[cfg(not(windows))]
            cmd.arg(command);
        }
        "git_bash" => {
            cmd.args(["--login", "-c", command]).env("CHERE_INVOKING", "1");
        }
        "wsl" => {
            cmd.args(["--cd", &cwd.display().to_string(), "-e", "sh", "-lc", command]).env("WSL_UTF8", "1");
        }
        _ => {
            cmd.args(setting_str(app, DEV_TERMINAL, "custom_shell_args").await.split_whitespace()).arg(command);
        }
    }
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    Ok(cmd)
}

fn emit(app: &AppHandle, run_id: &str, stream: &str, text: String) {
    if !text.is_empty() {
        let _ = app.emit(OUTPUT_EVENT, json!({ "run_id": run_id, "stream": stream, "text": text }));
    }
}

/// Streams one pipe as events; stops forwarding after `MAX_OUTPUT` bytes.
async fn pump(app: AppHandle, run_id: String, stream: &'static str, mut pipe: impl tokio::io::AsyncRead + Unpin) {
    let mut buf = vec![0u8; 8192];
    let mut dec = Decoder::default();
    let mut total = 0usize;
    loop {
        match pipe.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                total += n;
                if total <= MAX_OUTPUT {
                    emit(&app, &run_id, stream, dec.push(&buf[..n]));
                } else if total - n <= MAX_OUTPUT {
                    emit(&app, &run_id, "stderr", "\n… (вывод обрезан, команда продолжает работать)\n".into());
                }
            }
        }
    }
    emit(&app, &run_id, stream, dec.finish());
}

async fn execute(app: &AppHandle, run_id: &str, shell: &str, cwd: &Path, command: &str) -> AppResult<TabRun> {
    let started = Instant::now();
    let done = |code, timed_out, cancelled, cwd: &Path| TabRun {
        run_id: run_id.to_string(),
        code,
        timed_out,
        cancelled,
        duration_ms: started.elapsed().as_millis() as i64,
        cwd: cwd.display().to_string(),
    };
    if let Some(target) = parse_cd(command) {
        let new = resolve_cd(cwd, &target, &home())?;
        return Ok(done(Some(0), false, false, &new));
    }
    let timeout = Duration::from_secs(setting_u64(app, DEV_TERMINAL, "command_timeout_secs", 120).await);
    let mut child = build(app, shell, cwd, command).await?.spawn().map_err(|e| AppError::Other(anyhow::anyhow!("не удалось запустить shell: {e}")))?;
    let pid = child.id();
    let out = tokio::spawn(pump(app.clone(), run_id.into(), "stdout", child.stdout.take().expect("piped")));
    let err = tokio::spawn(pump(app.clone(), run_id.into(), "stderr", child.stderr.take().expect("piped")));
    let (tx, rx) = tokio::sync::oneshot::channel();
    RUNNING.lock().unwrap().insert(run_id.to_string(), tx);
    let outcome = tokio::select! {
        s = child.wait() => Ok(s.map_err(anyhow::Error::from)?.code()),
        _ = tokio::time::sleep(timeout) => Err(true),
        _ = rx => Err(false),
    };
    RUNNING.lock().unwrap().remove(run_id);
    let result = match outcome {
        Ok(code) => done(code, false, false, cwd),
        Err(timed_out) => {
            if let Some(pid) = pid {
                kill_tree(pid).await;
            }
            let _ = child.kill().await;
            done(None, timed_out, !timed_out, cwd)
        }
    };
    // All output events are sent before the command returns.
    let _ = tokio::time::timeout(Duration::from_secs(5), async {
        let _ = out.await;
        let _ = err.await;
    })
    .await;
    Ok(result)
}

async fn save_history(app: &AppHandle, shell: &str, cwd: &Path, command: &str, run: &TabRun) -> AppResult<()> {
    let db = &app.state::<AppState>().db;
    let settings = registry::settings(db, DEV_TERMINAL).await?;
    if !settings.get("save_history").and_then(|v| v.as_bool()).unwrap_or(true) {
        return Ok(());
    }
    let limit = settings.get("history_limit").and_then(|v| v.as_i64()).unwrap_or(1000);
    sqlx::query("INSERT INTO terminal_history (shell, cwd, command, exit_code, duration_ms) VALUES (?, ?, ?, ?, ?)")
        .bind(shell)
        .bind(cwd.display().to_string())
        .bind(command)
        .bind(run.code)
        .bind(run.duration_ms)
        .execute(db)
        .await?;
    sqlx::query("DELETE FROM terminal_history WHERE id <= (SELECT MAX(id) FROM terminal_history) - ?").bind(limit).execute(db).await?;
    Ok(())
}

// --- history and favourites ---

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct HistoryEntry {
    pub id: i64,
    pub shell: String,
    pub cwd: String,
    pub command: String,
    pub exit_code: Option<i64>,
    pub duration_ms: i64,
    pub created_at: String,
}

pub async fn history(pool: &SqlitePool, query: Option<&str>, limit: i64) -> AppResult<Vec<HistoryEntry>> {
    let like = query.map(str::trim).filter(|q| !q.is_empty()).map(|q| format!("%{}%", q.replace('%', "\\%").replace('_', "\\_")));
    Ok(sqlx::query_as::<_, HistoryEntry>("SELECT * FROM terminal_history WHERE (?1 IS NULL OR command LIKE ?1 ESCAPE '\\') ORDER BY id DESC LIMIT ?2")
        .bind(like)
        .bind(limit.clamp(1, 2000))
        .fetch_all(pool)
        .await?)
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Favorite {
    pub id: String,
    pub name: String,
    pub shell: String,
    pub command: String,
    pub cwd: Option<String>,
    pub position: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FavoriteInput {
    pub name: String,
    pub shell: String,
    pub command: String,
    #[serde(default)]
    pub cwd: Option<String>,
}

pub async fn favorites(pool: &SqlitePool) -> AppResult<Vec<Favorite>> {
    Ok(sqlx::query_as::<_, Favorite>("SELECT id, name, shell, command, cwd, position FROM terminal_favorites ORDER BY position, created_at").fetch_all(pool).await?)
}

pub async fn save_favorite(pool: &SqlitePool, id: Option<&str>, i: FavoriteInput) -> AppResult<Favorite> {
    let name = i.name.trim();
    let command = i.command.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::Invalid("название: от 1 до 80 символов".into()));
    }
    if command.is_empty() || command.len() > 8000 {
        return Err(AppError::Invalid("команда: от 1 до 8000 символов".into()));
    }
    if !SHELLS.contains(&i.shell.as_str()) {
        return Err(AppError::Invalid(format!("неизвестный shell {}", i.shell)));
    }
    let cwd = i.cwd.map(|c| c.trim().to_string()).filter(|c| !c.is_empty());
    let id = match id {
        Some(id) => {
            let n = sqlx::query("UPDATE terminal_favorites SET name = ?, shell = ?, command = ?, cwd = ? WHERE id = ?")
                .bind(name)
                .bind(&i.shell)
                .bind(command)
                .bind(&cwd)
                .bind(id)
                .execute(pool)
                .await?
                .rows_affected();
            if n == 0 {
                return Err(AppError::NotFound("избранная команда".into()));
            }
            id.to_string()
        }
        None => {
            let id = new_id();
            sqlx::query("INSERT INTO terminal_favorites (id, name, shell, command, cwd, position) VALUES (?, ?, ?, ?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM terminal_favorites))")
                .bind(&id)
                .bind(name)
                .bind(&i.shell)
                .bind(command)
                .bind(&cwd)
                .execute(pool)
                .await?;
            id
        }
    };
    Ok(sqlx::query_as::<_, Favorite>("SELECT id, name, shell, command, cwd, position FROM terminal_favorites WHERE id = ?").bind(id).fetch_one(pool).await?)
}

// --- commands ---

#[tauri::command]
pub async fn terminal_shells(app: AppHandle) -> AppResult<Vec<Shell>> {
    require(&app, DEV_TERMINAL)?;
    Ok(shells(&app).await)
}

#[tauri::command]
pub async fn terminal_home(app: AppHandle) -> AppResult<String> {
    require(&app, DEV_TERMINAL)?;
    Ok(home().display().to_string())
}

/// Opens a shell in its own window.
#[tauri::command]
pub async fn terminal_open(app: AppHandle, shell: String, cwd: String, command: Option<String>, confirm: Option<bool>) -> AppResult<()> {
    require(&app, DEV_TERMINAL)?;
    let dir = super::existing_dir(&cwd)?;
    let target = format!("{shell}: {}", dir.display());
    let action = open_window(&app, &shell, &dir, command.as_deref());
    guarded(&app, DEV_TERMINAL, "terminal.open", &target, confirm.unwrap_or(false), action, |_| "открыт".into()).await
}

/// Runs one command in a tab; output arrives as `focusdeck://terminal-output`.
#[tauri::command]
pub async fn terminal_run(app: AppHandle, run_id: String, shell: String, cwd: String, command: String, confirm: Option<bool>) -> AppResult<TabRun> {
    require(&app, DEV_TERMINAL)?;
    let dir = super::existing_dir(&cwd)?;
    let command = command.trim().to_string();
    if command.is_empty() || command.len() > 8000 {
        return Err(AppError::Invalid("команда: от 1 до 8000 символов".into()));
    }
    if run_id.is_empty() || run_id.len() > 64 {
        return Err(AppError::Invalid("некорректный идентификатор запуска".into()));
    }
    let target = format!("{shell} · {}: {command}", dir.display());
    let action = execute(&app, &run_id, &shell, &dir, &command);
    let run = guarded(&app, DEV_TERMINAL, "terminal.run", &target, confirm.unwrap_or(false), action, |r| match (r.cancelled, r.timed_out, r.code) {
        (true, ..) => "остановлена".into(),
        (_, true, _) => "превышен таймаут".into(),
        (.., Some(c)) => format!("код {c}"),
        _ => "завершена".into(),
    })
    .await?;
    if let Err(e) = save_history(&app, &shell, &dir, &command, &run).await {
        tracing::warn!("terminal history: {e}");
    }
    Ok(run)
}

/// Stops a running tab command with all its child processes.
#[tauri::command]
pub async fn terminal_cancel(run_id: String) -> AppResult<bool> {
    Ok(RUNNING.lock().unwrap().remove(&run_id).map(|tx| tx.send(()).is_ok()).unwrap_or(false))
}

#[tauri::command]
pub async fn terminal_history(app: AppHandle, query: Option<String>, limit: Option<i64>) -> AppResult<Vec<HistoryEntry>> {
    require(&app, DEV_TERMINAL)?;
    history(&app.state::<AppState>().db, query.as_deref(), limit.unwrap_or(200)).await
}

#[tauri::command]
pub async fn terminal_history_clear(app: AppHandle) -> AppResult<()> {
    require(&app, DEV_TERMINAL)?;
    sqlx::query("DELETE FROM terminal_history").execute(&app.state::<AppState>().db).await?;
    Ok(())
}

#[tauri::command]
pub async fn terminal_favorites(app: AppHandle) -> AppResult<Vec<Favorite>> {
    require(&app, DEV_TERMINAL)?;
    favorites(&app.state::<AppState>().db).await
}

#[tauri::command]
pub async fn terminal_favorite_save(app: AppHandle, id: Option<String>, input: FavoriteInput) -> AppResult<Favorite> {
    require(&app, DEV_TERMINAL)?;
    save_favorite(&app.state::<AppState>().db, id.as_deref(), input).await
}

#[tauri::command]
pub async fn terminal_favorite_delete(app: AppHandle, id: String) -> AppResult<()> {
    require(&app, DEV_TERMINAL)?;
    sqlx::query("DELETE FROM terminal_favorites WHERE id = ?").bind(id).execute(&app.state::<AppState>().db).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cd_is_recognised() {
        assert_eq!(parse_cd("cd src").as_deref(), Some("src"));
        assert_eq!(parse_cd("cd /d \"D:\\Work\"").as_deref(), Some("D:\\Work"));
        assert_eq!(parse_cd("cd..").as_deref(), Some(".."));
        assert_eq!(parse_cd("CD").as_deref(), Some(""));
        assert_eq!(parse_cd("Set-Location 'C:\\x'").as_deref(), Some("C:\\x"));
        assert_eq!(parse_cd("cd src && npm test"), None);
        assert_eq!(parse_cd("cdk deploy"), None);
        assert_eq!(parse_cd("echo cd"), None);
    }

    #[test]
    fn cd_resolves_folders() {
        let base = std::env::temp_dir().join(format!("fd-term-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(base.join("sub")).unwrap();
        assert_eq!(resolve_cd(&base, "sub", &base).unwrap(), normalize(&base.join("sub")));
        assert_eq!(resolve_cd(&base.join("sub"), "..", &base).unwrap(), normalize(&base));
        assert_eq!(resolve_cd(&base.join("sub"), "~", &base).unwrap(), base);
        assert!(resolve_cd(&base, "missing", &base).is_err());
        assert!(resolve_cd(&base, "/usr/bin", &base).is_err());
        std::fs::remove_dir(base.join("sub")).unwrap();
        std::fs::remove_dir(&base).unwrap();
        assert_eq!(unix_to_windows("/mnt/c/Users/x"), Some(PathBuf::from(r"C:\Users\x")));
        assert_eq!(unix_to_windows("/d/work"), Some(PathBuf::from(r"D:\work")));
        assert_eq!(unix_to_windows("/usr/bin"), None);
    }

    #[tokio::test]
    async fn favorites_and_history() {
        let pool = crate::db::test_pool().await;
        let input = |name: &str, shell: &str| FavoriteInput { name: name.into(), shell: shell.into(), command: "git status".into(), cwd: Some(" ".into()) };
        let a = save_favorite(&pool, None, input("Статус", "powershell")).await.unwrap();
        assert!(a.cwd.is_none());
        let b = save_favorite(&pool, None, input("Второй", "cmd")).await.unwrap();
        assert_eq!(b.position, 1);
        assert!(save_favorite(&pool, None, input("x", "zsh")).await.is_err());
        save_favorite(&pool, Some(&a.id), input("Переименован", "cmd")).await.unwrap();
        assert_eq!(favorites(&pool).await.unwrap()[0].name, "Переименован");
        for c in ["npm test", "git log", "npm run 50%_done"] {
            sqlx::query("INSERT INTO terminal_history (shell, cwd, command) VALUES ('cmd', 'C:\\', ?)").bind(c).execute(&pool).await.unwrap();
        }
        assert_eq!(history(&pool, Some("npm"), 10).await.unwrap().len(), 2);
        assert_eq!(history(&pool, Some("50%_"), 10).await.unwrap().len(), 1);
        assert_eq!(history(&pool, None, 10).await.unwrap()[0].command, "npm run 50%_done");
    }
}
