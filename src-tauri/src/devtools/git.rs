//! Git tool: repositories, branch, status (staged/unstaged/untracked/
//! conflicts), history, diffs; stage, commit, push and pull.
//!
//! Uses the installed `git` with explicit arguments (never a shell), with
//! `GIT_TERMINAL_PROMPT=0` so it can't hang waiting for a password. Reading
//! needs no permission; changes need `dev.git` and are logged. Push and pull
//! are dangerous actions (always confirmed); pull is fast-forward only and
//! push is never forced.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

use super::{guarded, require, run_program, setting_str, setting_u64, which, ProcOutput};
use crate::error::{AppError, AppResult};
use crate::modules::catalog::DEV_GIT;
use crate::state::AppState;

const READ_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_DIFF: usize = 1024 * 1024;

async fn program(app: &AppHandle) -> AppResult<PathBuf> {
    let custom = setting_str(app, DEV_GIT, "git_path").await;
    if !custom.is_empty() {
        let p = PathBuf::from(&custom);
        return if p.is_file() { Ok(p) } else { Err(AppError::NotFound(format!("git: {custom}"))) };
    }
    which("git").ok_or_else(|| AppError::NotFound("git не найден в PATH. Установите Git или укажите путь в настройках модуля.".into()))
}

/// Runs git in `repo`; a non-zero exit becomes an error with git's message.
async fn git(app: &AppHandle, repo: &Path, args: &[&str], timeout: Duration) -> AppResult<String> {
    let out = git_raw(app, repo, args, timeout).await?;
    if out.timed_out {
        return Err(AppError::Other(anyhow::anyhow!("git не ответил за {} с", timeout.as_secs())));
    }
    if out.code != Some(0) {
        return Err(AppError::Other(anyhow::anyhow!(git_error(&out))));
    }
    Ok(out.stdout)
}

async fn git_raw(app: &AppHandle, repo: &Path, args: &[&str], timeout: Duration) -> AppResult<ProcOutput> {
    let mut full: Vec<String> = ["-c", "core.quotepath=false", "-c", "color.ui=false", "-c", "core.pager=cat"].iter().map(|s| s.to_string()).collect();
    full.extend(args.iter().map(|s| s.to_string()));
    run_program(&program(app).await?, &full, Some(repo), &[("GIT_TERMINAL_PROMPT", "0"), ("GIT_OPTIONAL_LOCKS", "0"), ("GCM_INTERACTIVE", "never")], timeout).await
}

fn git_error(out: &ProcOutput) -> String {
    let text = if out.stderr.trim().is_empty() { &out.stdout } else { &out.stderr };
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("hint:")).take(6).collect();
    if lines.is_empty() { format!("git завершился с кодом {:?}", out.code) } else { lines.join("\n") }
}

// --- status parsing ---

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FileChange {
    pub path: String,
    /// Old path of a rename or copy.
    pub orig_path: Option<String>,
    /// Status letter: M A D R C T U ?.
    pub status: char,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Status {
    pub root: String,
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// HEAD isn't on a branch.
    pub detached: bool,
    /// The repository has no commits yet.
    pub no_commits: bool,
    pub staged: Vec<FileChange>,
    pub unstaged: Vec<FileChange>,
    pub untracked: Vec<FileChange>,
    pub conflicted: Vec<FileChange>,
    pub clean: bool,
}

/// Parses `git status --porcelain=v1 -b -z`.
pub fn parse_status(out: &str) -> Status {
    let mut s = Status::default();
    let mut parts = out.split('\0').filter(|p| !p.is_empty());
    while let Some(entry) = parts.next() {
        if let Some(head) = entry.strip_prefix("## ") {
            parse_branch(head, &mut s);
            continue;
        }
        if entry.len() < 4 {
            continue;
        }
        let (x, y) = (entry.as_bytes()[0] as char, entry.as_bytes()[1] as char);
        let path = entry[3..].to_string();
        let orig_path = if matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C') { parts.next().map(String::from) } else { None };
        let xy = format!("{x}{y}");
        if ["DD", "AU", "UD", "UA", "DU", "AA", "UU"].contains(&xy.as_str()) {
            s.conflicted.push(FileChange { path, orig_path, status: 'U' });
            continue;
        }
        if xy == "??" {
            s.untracked.push(FileChange { path, orig_path, status: '?' });
            continue;
        }
        if x != ' ' && x != '!' {
            s.staged.push(FileChange { path: path.clone(), orig_path: orig_path.clone(), status: x });
        }
        if y != ' ' && y != '!' {
            s.unstaged.push(FileChange { path, orig_path, status: y });
        }
    }
    s.clean = s.staged.is_empty() && s.unstaged.is_empty() && s.untracked.is_empty() && s.conflicted.is_empty();
    s
}

fn parse_branch(head: &str, s: &mut Status) {
    if let Some(rest) = head.strip_prefix("No commits yet on ").or_else(|| head.strip_prefix("Initial commit on ")) {
        s.no_commits = true;
        s.branch = Some(rest.trim().to_string());
        return;
    }
    if head.starts_with("HEAD (no branch)") {
        s.detached = true;
        return;
    }
    let (names, track) = match head.find(" [") {
        Some(i) => (&head[..i], Some(head[i + 2..].trim_end_matches(']'))),
        None => (head, None),
    };
    match names.split_once("...") {
        Some((b, u)) => {
            s.branch = Some(b.to_string());
            s.upstream = Some(u.to_string());
        }
        None => s.branch = Some(names.to_string()),
    }
    for part in track.unwrap_or("").split(", ") {
        if let Some(n) = part.strip_prefix("ahead ") {
            s.ahead = n.parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            s.behind = n.parse().unwrap_or(0);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Commit {
    pub hash: String,
    pub short: String,
    pub author: String,
    pub email: String,
    pub date: String,
    /// Branch and tag names pointing here (`%D`).
    pub refs: String,
    pub subject: String,
}

const LOG_FORMAT: &str = "--pretty=format:%H%x1f%h%x1f%an%x1f%ae%x1f%aI%x1f%D%x1f%s%x1e";

pub fn parse_log(out: &str) -> Vec<Commit> {
    out.split('\x1e')
        .map(|r| r.trim_start_matches(['\n', '\r']))
        .filter(|r| !r.is_empty())
        .filter_map(|r| {
            let f: Vec<&str> = r.split('\x1f').collect();
            (f.len() >= 7).then(|| Commit {
                hash: f[0].into(),
                short: f[1].into(),
                author: f[2].into(),
                email: f[3].into(),
                date: f[4].into(),
                refs: f[5].into(),
                subject: f[6..].join("\x1f"),
            })
        })
        .collect()
}

fn valid_hash(h: &str) -> bool {
    (4..=64).contains(&h.len()) && h.chars().all(|c| c.is_ascii_hexdigit())
}

/// A path inside the repository: relative, no `..`, no option-looking names.
fn valid_rel_path(p: &str) -> bool {
    let path = Path::new(p);
    !p.is_empty() && !path.is_absolute() && !p.starts_with('-') && !path.components().any(|c| matches!(c, std::path::Component::ParentDir))
}

fn truncate(mut s: String, max: usize) -> (String, bool) {
    if s.len() <= max {
        return (s, false);
    }
    let mut cut = max;
    while !s.is_char_boundary(cut) {
        cut -= 1;
    }
    s.truncate(cut);
    (s, true)
}

// --- repositories ---

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Repo {
    pub path: String,
    pub name: String,
    pub last_opened_at: String,
}

pub async fn repos(pool: &SqlitePool) -> AppResult<Vec<Repo>> {
    Ok(sqlx::query_as::<_, Repo>("SELECT * FROM dev_git_repos ORDER BY last_opened_at DESC, rowid DESC").fetch_all(pool).await?)
}

/// The current repository: the one opened last.
pub async fn current(pool: &SqlitePool) -> AppResult<Option<PathBuf>> {
    Ok(repos(pool).await?.into_iter().next().map(|r| PathBuf::from(r.path)))
}

async fn toplevel(app: &AppHandle, dir: &Path) -> AppResult<PathBuf> {
    let out = git_raw(app, dir, &["rev-parse", "--show-toplevel"], READ_TIMEOUT).await?;
    if out.code != Some(0) {
        return Err(AppError::Invalid(format!("{} — не Git-репозиторий", dir.display())));
    }
    Ok(PathBuf::from(out.stdout.trim().replace('/', std::path::MAIN_SEPARATOR_STR)))
}

async fn repo_path(app: &AppHandle, repo: &str) -> AppResult<PathBuf> {
    require(app, DEV_GIT)?;
    super::existing_dir(repo)
}

async fn status_of(app: &AppHandle, repo: &Path) -> AppResult<Status> {
    let out = git(app, repo, &["status", "--porcelain=v1", "-b", "-z", "--untracked-files=all"], READ_TIMEOUT).await?;
    let mut s = parse_status(&out);
    s.root = repo.display().to_string();
    Ok(s)
}

async fn log_of(app: &AppHandle, repo: &Path, limit: u64) -> AppResult<Vec<Commit>> {
    let n = format!("-n{limit}");
    Ok(parse_log(&git(app, repo, &["log", &n, LOG_FORMAT], READ_TIMEOUT).await?))
}

/// Data for the dashboard widget.
pub async fn summary(app: &AppHandle) -> AppResult<Value> {
    let db = &app.state::<AppState>().db;
    let Some(repo) = current(db).await? else { return Ok(json!({ "repo": null })) };
    if !repo.is_dir() {
        return Ok(json!({ "repo": repo, "error": "папка не найдена" }));
    }
    let status = status_of(app, &repo).await?;
    let commits = if status.no_commits { vec![] } else { log_of(app, &repo, 3).await.unwrap_or_default() };
    let name = repo.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(json!({ "repo": repo, "name": name, "status": status, "commits": commits }))
}

// --- commands ---

#[tauri::command]
pub async fn git_repos(app: AppHandle) -> AppResult<Vec<Repo>> {
    require(&app, DEV_GIT)?;
    repos(&app.state::<AppState>().db).await
}

/// Opens (or re-opens) a repository and makes it current.
#[tauri::command]
pub async fn git_open_repo(app: AppHandle, path: String) -> AppResult<Repo> {
    let dir = repo_path(&app, &path).await?;
    let root = toplevel(&app, &dir).await?;
    let root_s = root.display().to_string();
    let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| root_s.clone());
    let db = &app.state::<AppState>().db;
    sqlx::query(
        "INSERT INTO dev_git_repos (path, name) VALUES (?, ?)
         ON CONFLICT(path) DO UPDATE SET name = excluded.name, last_opened_at = datetime('now')",
    )
    .bind(&root_s)
    .bind(&name)
    .execute(db)
    .await?;
    Ok(sqlx::query_as::<_, Repo>("SELECT * FROM dev_git_repos WHERE path = ?").bind(&root_s).fetch_one(db).await?)
}

/// Removes a repository from the list (the folder is not touched).
#[tauri::command]
pub async fn git_forget_repo(app: AppHandle, path: String) -> AppResult<()> {
    require(&app, DEV_GIT)?;
    sqlx::query("DELETE FROM dev_git_repos WHERE path = ?").bind(path).execute(&app.state::<AppState>().db).await?;
    Ok(())
}

#[tauri::command]
pub async fn git_status(app: AppHandle, repo: String) -> AppResult<Status> {
    let repo = repo_path(&app, &repo).await?;
    status_of(&app, &repo).await
}

#[tauri::command]
pub async fn git_log(app: AppHandle, repo: String, limit: Option<u64>) -> AppResult<Vec<Commit>> {
    let repo = repo_path(&app, &repo).await?;
    let limit = match limit {
        Some(l) => l.clamp(1, 500),
        None => setting_u64(&app, DEV_GIT, "log_limit", 30).await,
    };
    let out = git_raw(&app, &repo, &["log", &format!("-n{limit}"), LOG_FORMAT], READ_TIMEOUT).await?;
    // An empty repository has no log: that's not an error.
    if out.code != Some(0) {
        let status = status_of(&app, &repo).await?;
        return if status.no_commits { Ok(vec![]) } else { Err(AppError::Other(anyhow::anyhow!(git_error(&out)))) };
    }
    Ok(parse_log(&out.stdout))
}

#[derive(Debug, Clone, Serialize)]
pub struct Diff {
    pub text: String,
    pub truncated: bool,
}

/// Diff of a file (staged or not), of the whole working tree, or of a commit.
#[tauri::command]
pub async fn git_diff(app: AppHandle, repo: String, path: Option<String>, staged: Option<bool>, untracked: Option<bool>, commit: Option<String>) -> AppResult<Diff> {
    let repo = repo_path(&app, &repo).await?;
    let text = if let Some(c) = commit {
        if !valid_hash(&c) {
            return Err(AppError::Invalid("некорректный хеш коммита".into()));
        }
        git(&app, &repo, &["show", "--no-color", "--stat", "--patch", "--format=fuller", &c], READ_TIMEOUT).await?
    } else {
        if let Some(p) = &path {
            if !valid_rel_path(p) {
                return Err(AppError::Invalid("некорректный путь".into()));
            }
        }
        match (path.as_deref(), untracked.unwrap_or(false)) {
            // A new file isn't known to git yet: show it as added.
            (Some(p), true) => new_file_diff(&repo, p)?,
            (p, _) => {
                let mut args = vec!["diff", "--no-color"];
                if staged.unwrap_or(false) {
                    args.push("--cached");
                }
                args.push("--");
                if let Some(p) = p {
                    args.push(p);
                }
                git(&app, &repo, &args, READ_TIMEOUT).await?
            }
        }
    };
    let (text, truncated) = truncate(text, MAX_DIFF);
    Ok(Diff { text, truncated })
}

fn new_file_diff(repo: &Path, rel: &str) -> AppResult<String> {
    let full = repo.join(rel);
    let bytes = std::fs::read(&full).map_err(|e| AppError::NotFound(format!("{rel}: {e}")))?;
    let mut out = format!("diff --git a/{rel} b/{rel}\nnew file\n--- /dev/null\n+++ b/{rel}\n");
    if bytes.contains(&0) {
        out += &format!("Двоичный файл, {} байт\n", bytes.len());
        return Ok(out);
    }
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_DIFF)]);
    let lines: Vec<&str> = text.lines().collect();
    out += &format!("@@ -0,0 +1,{} @@\n", lines.len());
    for l in lines {
        out.push('+');
        out += l;
        out.push('\n');
    }
    Ok(out)
}

/// Adds files to the index (`stage = true`) or takes them out. No paths = all.
#[tauri::command]
pub async fn git_stage(app: AppHandle, repo: String, paths: Vec<String>, stage: bool, confirm: Option<bool>) -> AppResult<Status> {
    let repo = repo_path(&app, &repo).await?;
    if let Some(bad) = paths.iter().find(|p| !valid_rel_path(p)) {
        return Err(AppError::Invalid(format!("некорректный путь {bad}")));
    }
    let target = format!("{}: {}", repo.display(), if paths.is_empty() { "все файлы".to_string() } else { paths.join(", ") });
    let action = async {
        let mut args: Vec<&str> = if stage {
            vec!["add", "-A", "--"]
        } else if status_of(&app, &repo).await?.no_commits {
            vec!["rm", "--cached", "-r", "-q", "--"]
        } else {
            vec!["restore", "--staged", "--"]
        };
        if paths.is_empty() {
            args.push(".");
        } else {
            args.extend(paths.iter().map(String::as_str));
        }
        git(&app, &repo, &args, READ_TIMEOUT).await?;
        status_of(&app, &repo).await
    };
    let verb = if stage { "добавлено в индекс" } else { "убрано из индекса" };
    guarded(&app, DEV_GIT, "git.stage", &target, confirm.unwrap_or(false), action, |_| verb.to_string()).await
}

#[tauri::command]
pub async fn git_commit(app: AppHandle, repo: String, message: String, confirm: Option<bool>) -> AppResult<Commit> {
    let repo = repo_path(&app, &repo).await?;
    let message = message.trim().to_string();
    if message.is_empty() {
        return Err(AppError::Invalid("введите сообщение коммита".into()));
    }
    if message.len() > 20_000 {
        return Err(AppError::Invalid("сообщение коммита слишком длинное".into()));
    }
    let subject = message.lines().next().unwrap_or("").to_string();
    let target = format!("{}: {subject}", repo.display());
    let action = async {
        if status_of(&app, &repo).await?.staged.is_empty() {
            return Err(AppError::Invalid("нет файлов в индексе: сначала добавьте изменения".into()));
        }
        git(&app, &repo, &["commit", "-m", &message], READ_TIMEOUT).await?;
        log_of(&app, &repo, 1).await?.into_iter().next().ok_or_else(|| AppError::Other(anyhow::anyhow!("коммит не найден")))
    };
    guarded(&app, DEV_GIT, "git.commit", &target, confirm.unwrap_or(false), action, |c| format!("{} {}", c.short, c.subject)).await
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncResult {
    pub output: String,
    pub status: Status,
}

/// `git pull --ff-only`. Always needs confirmation.
#[tauri::command]
pub async fn git_pull(app: AppHandle, repo: String, confirm: Option<bool>) -> AppResult<SyncResult> {
    let repo = repo_path(&app, &repo).await?;
    let timeout = Duration::from_secs(setting_u64(&app, DEV_GIT, "timeout_secs", 120).await);
    let target = repo.display().to_string();
    let action = async {
        let out = git(&app, &repo, &["pull", "--ff-only", "--no-rebase"], timeout).await?;
        Ok(SyncResult { output: out, status: status_of(&app, &repo).await? })
    };
    guarded(&app, DEV_GIT, "git.pull", &target, confirm.unwrap_or(false), action, |r| first_line(&r.output)).await
}

/// `git push` (never forced). Without an upstream, pushes to `origin` and sets it.
#[tauri::command]
pub async fn git_push(app: AppHandle, repo: String, confirm: Option<bool>) -> AppResult<SyncResult> {
    let repo = repo_path(&app, &repo).await?;
    let timeout = Duration::from_secs(setting_u64(&app, DEV_GIT, "timeout_secs", 120).await);
    let status = status_of(&app, &repo).await?;
    let target = format!("{} ({})", repo.display(), status.branch.clone().unwrap_or_else(|| "HEAD".into()));
    let action = async {
        let args: Vec<String> = match (&status.upstream, &status.branch) {
            (Some(_), _) => vec!["push".into()],
            (None, Some(branch)) if !status.detached => {
                let remotes = git(&app, &repo, &["remote"], READ_TIMEOUT).await?;
                if !remotes.lines().any(|r| r.trim() == "origin") {
                    return Err(AppError::Invalid("у ветки нет upstream и нет удалённого репозитория origin".into()));
                }
                vec!["push".into(), "--set-upstream".into(), "origin".into(), branch.clone()]
            }
            _ => return Err(AppError::Invalid("HEAD не на ветке: push невозможен".into())),
        };
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = git_raw(&app, &repo, &refs, timeout).await?;
        if out.timed_out {
            return Err(AppError::Other(anyhow::anyhow!("git push не ответил за {} с", timeout.as_secs())));
        }
        if out.code != Some(0) {
            return Err(AppError::Other(anyhow::anyhow!(git_error(&out))));
        }
        // git push reports progress on stderr.
        let output = format!("{}{}", out.stdout, out.stderr).trim().to_string();
        Ok(SyncResult { output, status: status_of(&app, &repo).await? })
    };
    guarded(&app, DEV_GIT, "git.push", &target, confirm.unwrap_or(false), action, |r| first_line(&r.output)).await
}

fn first_line(s: &str) -> String {
    s.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("готово").chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status() {
        let out = "## main...origin/main [ahead 2, behind 1]\0M  staged.rs\0 M worktree.rs\0MM both.rs\0R  new.rs\0old.rs\0?? new dir/файл.txt\0UU conflict.rs\0";
        let s = parse_status(out);
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!(s.upstream.as_deref(), Some("origin/main"));
        assert_eq!((s.ahead, s.behind), (2, 1));
        assert_eq!(s.staged.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["staged.rs", "both.rs", "new.rs"]);
        assert_eq!(s.staged[2].orig_path.as_deref(), Some("old.rs"));
        assert_eq!(s.unstaged.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["worktree.rs", "both.rs"]);
        assert_eq!(s.untracked[0].path, "new dir/файл.txt");
        assert_eq!(s.conflicted[0].path, "conflict.rs");
        assert!(!s.clean);

        let empty = parse_status("## No commits yet on master\0");
        assert!(empty.no_commits && empty.clean);
        assert_eq!(empty.branch.as_deref(), Some("master"));
        assert!(parse_status("## HEAD (no branch)\0").detached);
        let local = parse_status("## feature\0");
        assert_eq!((local.branch.as_deref(), local.upstream), (Some("feature"), None));
    }

    #[test]
    fn parses_log() {
        let out = "abc123\x1fabc\x1fИван\x1fi@x.ru\x1f2026-10-20T10:00:00+03:00\x1fHEAD -> main\x1fПервый коммит\x1e\nfff\x1ff\x1fA\x1fa@b\x1f2026-10-19T10:00:00+03:00\x1f\x1fsecond\x1e";
        let log = parse_log(out);
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].subject, "Первый коммит");
        assert_eq!(log[0].refs, "HEAD -> main");
        assert_eq!(log[1].hash, "fff");
    }

    #[test]
    fn validates_paths_and_hashes() {
        assert!(valid_rel_path("src/main.rs"));
        assert!(!valid_rel_path("../outside"));
        assert!(!valid_rel_path("--force"));
        assert!(!valid_rel_path(r"C:\Windows"));
        assert!(valid_hash("cb84815"));
        assert!(!valid_hash("HEAD~1"));
        let (t, cut) = truncate("абв".into(), 3);
        assert_eq!((t.as_str(), cut), ("а", true));
    }

    #[test]
    fn new_file_diff_marks_lines_added() {
        let dir = std::env::temp_dir().join(format!("fd-git-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        let d = new_file_diff(&dir, "a.txt").unwrap();
        assert!(d.contains("@@ -0,0 +1,2 @@\n+one\n+two\n"));
        std::fs::remove_file(dir.join("a.txt")).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
}
