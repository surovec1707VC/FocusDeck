//! Project Launcher: a list of project folders with their type and launch
//! commands, detected from the files in the folder (package.json, Cargo.toml,
//! pyproject.toml, *.sln, go.mod …) and editable.
//!
//! Opening a project in the editor, a terminal or Explorer is `projects.open`
//! (permission «Запуск программ»); running one of its commands in a new
//! terminal window is `projects.run` (permission «Выполнение команд»).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use super::{guarded, require, setting_str, terminal};
use crate::db::new_id;
use crate::error::{AppError, AppResult};
use crate::modules::catalog::DEV_PROJECTS;
use crate::state::AppState;

pub const KINDS: &[&str] = &["node", "tauri", "rust", "python", "dotnet", "go", "java", "php", "ruby", "generic"];
const MAX_COMMANDS: usize = 40;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectCommand {
    pub name: String,
    pub command: String,
    /// Added by the user (kept when the project is detected again).
    #[serde(default)]
    pub custom: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub kind: String,
    pub commands: Vec<ProjectCommand>,
    pub favorite: bool,
    pub position: i64,
    pub last_opened_at: Option<String>,
    /// The folder still exists.
    pub exists: bool,
    /// The folder is a Git repository (has `.git`).
    pub is_git: bool,
}

#[derive(sqlx::FromRow)]
struct Row {
    id: String,
    name: String,
    path: String,
    kind: String,
    commands_json: String,
    favorite: bool,
    position: i64,
    last_opened_at: Option<String>,
}

impl From<Row> for Project {
    fn from(r: Row) -> Self {
        let p = Path::new(&r.path);
        Project {
            exists: p.is_dir(),
            is_git: p.join(".git").exists(),
            commands: serde_json::from_str(&r.commands_json).unwrap_or_default(),
            id: r.id,
            name: r.name,
            path: r.path,
            kind: r.kind,
            favorite: r.favorite,
            position: r.position,
            last_opened_at: r.last_opened_at,
        }
    }
}

// --- detection ---

fn cmd(name: &str, command: &str) -> ProjectCommand {
    ProjectCommand { name: name.into(), command: command.into(), custom: false }
}

fn has_ext(dir: &Path, ext: &str) -> bool {
    std::fs::read_dir(dir)
        .map(|rd| rd.flatten().any(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case(ext))))
        .unwrap_or(false)
}

/// Project type and its usual commands, from the files in `dir`.
pub fn detect(dir: &Path) -> (String, Vec<ProjectCommand>) {
    let f = |name: &str| dir.join(name).exists();
    if f("package.json") {
        let pm = if f("pnpm-lock.yaml") {
            "pnpm"
        } else if f("yarn.lock") {
            "yarn"
        } else if f("bun.lockb") || f("bun.lock") {
            "bun"
        } else {
            "npm"
        };
        let mut cmds = vec![cmd("Установить зависимости", &format!("{pm} install"))];
        let scripts = std::fs::read_to_string(dir.join("package.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| v.get("scripts").and_then(Value::as_object).cloned())
            .unwrap_or_default();
        let tauri = f("src-tauri/tauri.conf.json") || f("src-tauri/Cargo.toml");
        if tauri && scripts.contains_key("tauri") {
            cmds.push(cmd("tauri dev", &format!("{pm} run tauri dev")));
            cmds.push(cmd("tauri build", &format!("{pm} run tauri build")));
        }
        for name in scripts.keys().take(MAX_COMMANDS - cmds.len()) {
            let run = if pm == "npm" { format!("npm run {name}") } else { format!("{pm} {name}") };
            cmds.push(cmd(name, &run));
        }
        return (if tauri { "tauri" } else { "node" }.into(), cmds);
    }
    if f("Cargo.toml") {
        return ("rust".into(), vec![cmd("run", "cargo run"), cmd("build", "cargo build"), cmd("test", "cargo test"), cmd("check", "cargo check")]);
    }
    if f("pyproject.toml") || f("requirements.txt") || f("setup.py") || f("Pipfile") {
        let mut cmds = Vec::new();
        if f("requirements.txt") {
            cmds.push(cmd("Установить зависимости", "python -m pip install -r requirements.txt"));
        } else if f("pyproject.toml") {
            cmds.push(cmd("Установить пакет", "python -m pip install -e ."));
        }
        for main in ["main.py", "app.py", "manage.py"] {
            if f(main) {
                let c = if main == "manage.py" { "python manage.py runserver".to_string() } else { format!("python {main}") };
                cmds.push(cmd("run", &c));
            }
        }
        cmds.push(cmd("test", "python -m pytest"));
        return ("python".into(), cmds);
    }
    if has_ext(dir, "sln") || has_ext(dir, "csproj") || has_ext(dir, "fsproj") {
        return ("dotnet".into(), vec![cmd("run", "dotnet run"), cmd("build", "dotnet build"), cmd("test", "dotnet test")]);
    }
    if f("go.mod") {
        return ("go".into(), vec![cmd("run", "go run ."), cmd("build", "go build ./..."), cmd("test", "go test ./...")]);
    }
    if f("pom.xml") {
        return ("java".into(), vec![cmd("package", "mvn package"), cmd("test", "mvn test")]);
    }
    if f("build.gradle") || f("build.gradle.kts") {
        let g = if f("gradlew.bat") { ".\\gradlew.bat" } else { "gradle" };
        return ("java".into(), vec![cmd("build", &format!("{g} build")), cmd("test", &format!("{g} test"))]);
    }
    if f("composer.json") {
        return ("php".into(), vec![cmd("Установить зависимости", "composer install")]);
    }
    if f("Gemfile") {
        return ("ruby".into(), vec![cmd("Установить зависимости", "bundle install")]);
    }
    ("generic".into(), vec![])
}

fn is_project(dir: &Path) -> bool {
    detect(dir).0 != "generic" || dir.join(".git").exists()
}

fn validate_commands(cmds: &[ProjectCommand]) -> AppResult<()> {
    if cmds.len() > MAX_COMMANDS {
        return Err(AppError::Invalid(format!("не больше {MAX_COMMANDS} команд")));
    }
    for c in cmds {
        if c.name.trim().is_empty() || c.name.chars().count() > 60 {
            return Err(AppError::Invalid("название команды: от 1 до 60 символов".into()));
        }
        if c.command.trim().is_empty() || c.command.len() > 4000 {
            return Err(AppError::Invalid("команда: от 1 до 4000 символов".into()));
        }
    }
    Ok(())
}

// --- storage ---

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Project>> {
    Ok(sqlx::query_as::<_, Row>("SELECT * FROM dev_projects ORDER BY favorite DESC, position, name").fetch_all(pool).await?.into_iter().map(Project::from).collect())
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Project> {
    sqlx::query_as::<_, Row>("SELECT * FROM dev_projects WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .map(Project::from)
        .ok_or_else(|| AppError::NotFound("проект".into()))
}

pub async fn add(pool: &SqlitePool, path: &Path) -> AppResult<Project> {
    let path_s = path.display().to_string();
    if sqlx::query_as::<_, (String,)>("SELECT id FROM dev_projects WHERE path = ?").bind(&path_s).fetch_optional(pool).await?.is_some() {
        return Err(AppError::Invalid(format!("проект {path_s} уже в списке")));
    }
    let (kind, commands) = detect(path);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path_s.clone());
    let id = new_id();
    sqlx::query("INSERT INTO dev_projects (id, name, path, kind, commands_json, position) VALUES (?, ?, ?, ?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM dev_projects))")
        .bind(&id)
        .bind(&name)
        .bind(&path_s)
        .bind(&kind)
        .bind(serde_json::to_string(&commands)?)
        .execute(pool)
        .await?;
    get(pool, &id).await
}

#[derive(Debug, Deserialize)]
pub struct ProjectUpdate {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub commands: Option<Vec<ProjectCommand>>,
    pub favorite: Option<bool>,
}

pub async fn update(pool: &SqlitePool, id: &str, u: ProjectUpdate) -> AppResult<Project> {
    let mut p = get(pool, id).await?;
    if let Some(name) = u.name {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 100 {
            return Err(AppError::Invalid("название: от 1 до 100 символов".into()));
        }
        p.name = name.into();
    }
    if let Some(kind) = u.kind {
        if !KINDS.contains(&kind.as_str()) {
            return Err(AppError::Invalid(format!("неизвестный тип проекта {kind}")));
        }
        p.kind = kind;
    }
    if let Some(cmds) = u.commands {
        validate_commands(&cmds)?;
        p.commands = cmds.into_iter().map(|c| ProjectCommand { name: c.name.trim().into(), command: c.command.trim().into(), custom: c.custom }).collect();
    }
    if let Some(f) = u.favorite {
        p.favorite = f;
    }
    sqlx::query("UPDATE dev_projects SET name = ?, kind = ?, commands_json = ?, favorite = ? WHERE id = ?")
        .bind(&p.name)
        .bind(&p.kind)
        .bind(serde_json::to_string(&p.commands)?)
        .bind(p.favorite)
        .bind(id)
        .execute(pool)
        .await?;
    get(pool, id).await
}

/// Detects the type and commands again; the user's own commands stay.
pub async fn redetect(pool: &SqlitePool, id: &str) -> AppResult<Project> {
    let p = get(pool, id).await?;
    let (kind, mut commands) = detect(Path::new(&p.path));
    commands.extend(p.commands.into_iter().filter(|c| c.custom));
    commands.truncate(MAX_COMMANDS);
    update(pool, id, ProjectUpdate { name: None, kind: Some(kind), commands: Some(commands), favorite: None }).await
}

// --- commands ---

#[tauri::command]
pub async fn projects_list(app: AppHandle) -> AppResult<Vec<Project>> {
    require(&app, DEV_PROJECTS)?;
    list(&app.state::<AppState>().db).await
}

#[tauri::command]
pub async fn projects_add(app: AppHandle, path: String) -> AppResult<Project> {
    require(&app, DEV_PROJECTS)?;
    let dir = super::existing_dir(&path)?;
    add(&app.state::<AppState>().db, &dir).await
}

#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub added: bool,
}

/// Project folders in `dir` and one level below (not added automatically).
#[tauri::command]
pub async fn projects_scan(app: AppHandle, dir: String) -> AppResult<Vec<Candidate>> {
    require(&app, DEV_PROJECTS)?;
    let root = super::existing_dir(&dir)?;
    let known: Vec<String> = list(&app.state::<AppState>().db).await?.into_iter().map(|p| p.path).collect();
    tauri::async_runtime::spawn_blocking(move || {
        let mut dirs: Vec<PathBuf> = vec![root.clone()];
        if let Ok(rd) = std::fs::read_dir(&root) {
            let mut subs: Vec<PathBuf> = rd
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .map(|e| e.path())
                .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.') || n == "node_modules"))
                .collect();
            subs.sort();
            dirs.extend(subs.into_iter().take(500));
        }
        dirs.into_iter()
            .filter(|d| is_project(d))
            .map(|d| {
                let path = d.display().to_string();
                Candidate {
                    name: d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone()),
                    kind: detect(&d).0,
                    added: known.contains(&path),
                    path,
                }
            })
            .collect()
    })
    .await
    .map_err(|e| AppError::Other(e.into()))
}

#[tauri::command]
pub async fn projects_update(app: AppHandle, id: String, update: ProjectUpdate) -> AppResult<Project> {
    require(&app, DEV_PROJECTS)?;
    self::update(&app.state::<AppState>().db, &id, update).await
}

#[tauri::command]
pub async fn projects_redetect(app: AppHandle, id: String) -> AppResult<Project> {
    require(&app, DEV_PROJECTS)?;
    redetect(&app.state::<AppState>().db, &id).await
}

/// Removes a project from the list; the folder is not touched.
#[tauri::command]
pub async fn projects_remove(app: AppHandle, id: String) -> AppResult<()> {
    require(&app, DEV_PROJECTS)?;
    sqlx::query("DELETE FROM dev_projects WHERE id = ?").bind(id).execute(&app.state::<AppState>().db).await?;
    Ok(())
}

async fn opened(app: &AppHandle, id: &str) {
    let _ = sqlx::query("UPDATE dev_projects SET last_opened_at = datetime('now') WHERE id = ?").bind(id).execute(&app.state::<AppState>().db).await;
}

/// Opens the project in the editor (`editor`), a terminal window
/// (`terminal`, optional `shell`) or Explorer (`explorer`).
#[tauri::command]
pub async fn projects_open(app: AppHandle, id: String, target: String, shell: Option<String>, confirm: Option<bool>) -> AppResult<()> {
    require(&app, DEV_PROJECTS)?;
    let p = get(&app.state::<AppState>().db, &id).await?;
    let dir = super::existing_dir(&p.path)?;
    let label = format!("{} → {target}", p.name);
    let action = async {
        match target.as_str() {
            "editor" => {
                let editor = setting_str(&app, DEV_PROJECTS, "editor_command").await;
                let editor = if editor.is_empty() { "code".to_string() } else { editor };
                // `code` is a .cmd script, so it is started through cmd.exe.
                let cmd = super::which("cmd").unwrap_or_else(|| PathBuf::from("cmd.exe"));
                let tail = format!("/D /S /C \"{editor} \"{}\"\"", dir.display());
                super::spawn_console(&cmd, &[], Some(&tail), &dir, &[], false)
            }
            "terminal" => {
                let shell = match shell.as_deref() {
                    Some(s) => s.to_string(),
                    None => setting_str(&app, DEV_PROJECTS, "default_shell").await,
                };
                terminal::open_window(&app, if shell.is_empty() { "powershell" } else { &shell }, &dir, None).await
            }
            "explorer" => app.opener().open_path(dir.display().to_string(), None::<&str>).map_err(|e| AppError::Other(e.into())),
            other => Err(AppError::Invalid(format!("неизвестное действие {other}"))),
        }
    };
    guarded(&app, DEV_PROJECTS, "projects.open", &label, confirm.unwrap_or(false), action, |_| "открыт".into()).await?;
    opened(&app, &id).await;
    Ok(())
}

/// Runs one of the project's commands in a new terminal window.
#[tauri::command]
pub async fn projects_run(app: AppHandle, id: String, index: usize, shell: Option<String>, confirm: Option<bool>) -> AppResult<()> {
    require(&app, DEV_PROJECTS)?;
    let p = get(&app.state::<AppState>().db, &id).await?;
    let dir = super::existing_dir(&p.path)?;
    let c = p.commands.get(index).ok_or_else(|| AppError::NotFound("команда проекта".into()))?.clone();
    let shell = match shell {
        Some(s) => s,
        None => setting_str(&app, DEV_PROJECTS, "default_shell").await,
    };
    let shell = if shell.is_empty() { "powershell".to_string() } else { shell };
    let label = format!("{}: {}", p.name, c.command);
    let action = terminal::open_window(&app, &shell, &dir, Some(&c.command));
    guarded(&app, DEV_PROJECTS, "projects.run", &label, confirm.unwrap_or(false), action, |_| format!("запущено в {shell}")).await?;
    opened(&app, &id).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fd-proj-{}", uuid::Uuid::new_v4()));
        for (name, body) in files {
            let p = dir.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &Path) {
        // Test folders only: created above, removed file by file.
        for e in walk(dir) {
            std::fs::remove_file(e).unwrap();
        }
        let mut dirs: Vec<PathBuf> = vec![dir.to_path_buf()];
        let mut i = 0;
        while i < dirs.len() {
            if let Ok(rd) = std::fs::read_dir(&dirs[i]) {
                dirs.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
            }
            i += 1;
        }
        for d in dirs.iter().rev() {
            std::fs::remove_dir(d).unwrap();
        }
    }

    fn walk(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() { out.extend(walk(&p)) } else { out.push(p) }
        }
        out
    }

    #[test]
    fn detects_project_types() {
        let node = temp_dir(&[("package.json", r#"{"scripts":{"dev":"vite","tauri":"tauri"}}"#), ("pnpm-lock.yaml", ""), ("src-tauri/tauri.conf.json", "{}")]);
        let (kind, cmds) = detect(&node);
        assert_eq!(kind, "tauri");
        assert_eq!(cmds[0].command, "pnpm install");
        assert!(cmds.iter().any(|c| c.command == "pnpm run tauri dev"));
        assert!(cmds.iter().any(|c| c.command == "pnpm dev"));
        cleanup(&node);

        let rust = temp_dir(&[("Cargo.toml", "[package]")]);
        assert_eq!(detect(&rust).0, "rust");
        cleanup(&rust);
        let py = temp_dir(&[("requirements.txt", ""), ("main.py", "")]);
        let (kind, cmds) = detect(&py);
        assert_eq!(kind, "python");
        assert!(cmds.iter().any(|c| c.command == "python main.py"));
        cleanup(&py);
        let net = temp_dir(&[("App.sln", "")]);
        assert_eq!(detect(&net).0, "dotnet");
        cleanup(&net);
        let empty = temp_dir(&[]);
        assert_eq!(detect(&empty), ("generic".to_string(), vec![]));
        assert!(!is_project(&empty));
        cleanup(&empty);
    }

    #[tokio::test]
    async fn add_update_redetect() {
        let pool = crate::db::test_pool().await;
        let dir = temp_dir(&[("go.mod", "module x")]);
        let p = add(&pool, &dir).await.unwrap();
        assert_eq!(p.kind, "go");
        assert!(p.exists);
        assert!(add(&pool, &dir).await.is_err(), "no duplicates");
        let mut cmds = p.commands.clone();
        cmds.push(ProjectCommand { name: "lint".into(), command: "golangci-lint run".into(), custom: true });
        let u = update(&pool, &p.id, ProjectUpdate { name: Some("Сервис".into()), kind: None, commands: Some(cmds), favorite: Some(true) }).await.unwrap();
        assert_eq!((u.name.as_str(), u.favorite, u.commands.len()), ("Сервис", true, 4));
        assert!(update(&pool, &p.id, ProjectUpdate { name: None, kind: Some("cobol".into()), commands: None, favorite: None }).await.is_err());
        std::fs::write(dir.join("Cargo.toml"), "").unwrap();
        std::fs::remove_file(dir.join("go.mod")).unwrap();
        let r = redetect(&pool, &p.id).await.unwrap();
        assert_eq!(r.kind, "rust");
        assert!(r.commands.iter().any(|c| c.custom && c.name == "lint"), "custom commands are kept");
        cleanup(&dir);
    }
}
