//! Runs automations. Triggers come from FocusDeck start, global hotkeys, the
//! UI, and a watcher (background task of the Automations module) that once a
//! second checks the active window, processes, monitors, network, the clock
//! and intervals. Every run: modules → cooldown → conditions → confirmation of
//! dangerous actions → permissions → actions → log.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::{DateTime, Datelike, Local, NaiveDateTime, NaiveTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

use super::store::{self, LogEntry};
use super::system::{self, expand_env, normalize_process};
use super::types::{Action, Automation, ChangeKind, Condition, Trigger, parse_hhmm};
use crate::error::{AppError, AppResult};
use crate::modules::catalog::{AUTOMATIONS, NOTIFICATIONS};
use crate::modules::registry;
use crate::permissions;
use crate::state::AppState;

pub const CONFIRM_EVENT: &str = "focusdeck://automation-confirm";
pub const RUN_EVENT: &str = "focusdeck://automation-run";
const CONFIRM_TTL: Duration = Duration::from_secs(300);

/// Parses RFC 3339 or SQLite's `datetime('now')` format (UTC).
pub fn parse_ts(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|d| d.with_timezone(&Utc))
        .ok()
        .or_else(|| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").ok().map(|n| n.and_utc()))
}

/// An interval automation is due `minutes` after its last run (or creation).
pub fn is_due(a: &Automation, now: DateTime<Utc>) -> bool {
    let Trigger::Interval { minutes } = a.trigger else { return false };
    let since = a.last_run_at.as_deref().or(Some(a.created_at.as_str())).and_then(parse_ts);
    match since {
        Some(t) => now - t >= chrono::Duration::minutes(minutes as i64),
        None => true,
    }
}

/// Still within the cooldown after the last run?
pub fn in_cooldown(a: &Automation, now: DateTime<Utc>) -> bool {
    a.cooldown_secs > 0
        && a.last_run_at.as_deref().and_then(parse_ts).is_some_and(|t| now - t < chrono::Duration::seconds(a.cooldown_secs as i64))
}

/// Why an automation must not run now (its module or Automations is off).
pub async fn blocked_reason(db: &sqlx::SqlitePool, a: &Automation) -> AppResult<Option<String>> {
    for module in [AUTOMATIONS, a.module_id.as_str()] {
        if !registry::is_enabled(db, module).await? {
            let name = registry::def(module).map(|m| m.name).unwrap_or(module);
            return Ok(Some(format!("модуль «{name}» отключён")));
        }
    }
    Ok(None)
}

// --- conditions ---

/// System state, read lazily once per run.
#[derive(Default)]
struct Probe {
    processes: Option<HashSet<String>>,
    foreground: Option<(String, String)>,
    online: Option<bool>,
}

impl Probe {
    async fn processes(&mut self) -> AppResult<&HashSet<String>> {
        if self.processes.is_none() {
            self.processes = Some(tauri::async_runtime::spawn_blocking(system::processes).await.map_err(anyhow::Error::from)?);
        }
        Ok(self.processes.as_ref().unwrap())
    }
    async fn foreground(&mut self) -> AppResult<&(String, String)> {
        if self.foreground.is_none() {
            self.foreground = Some(tauri::async_runtime::spawn_blocking(system::foreground).await.map_err(anyhow::Error::from)?);
        }
        Ok(self.foreground.as_ref().unwrap())
    }
    async fn online(&mut self) -> AppResult<bool> {
        if self.online.is_none() {
            self.online = Some(tauri::async_runtime::spawn_blocking(system::online).await.map_err(anyhow::Error::from)?);
        }
        Ok(self.online.unwrap())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub ok: bool,
    pub text: String,
}

const WEEKDAYS: [&str; 7] = ["пн", "вт", "ср", "чт", "пт", "сб", "вс"];

fn days_text(days: &[u8]) -> String {
    days.iter().filter_map(|d| WEEKDAYS.get(*d as usize - 1)).copied().collect::<Vec<_>>().join(", ")
}

/// Local time inside [from, to); a range like 22:00–06:00 crosses midnight.
pub fn in_time_range(now: NaiveTime, from: NaiveTime, to: NaiveTime) -> bool {
    if from <= to { now >= from && now < to } else { now >= from || now < to }
}

async fn check_condition(app: &AppHandle, c: &Condition, probe: &mut Probe) -> AppResult<CheckResult> {
    let db = &app.state::<AppState>().db;
    let now = Local::now();
    let (ok, text) = match c {
        Condition::ProcessRunning { process, running } => {
            let is = probe.processes().await?.contains(&normalize_process(process));
            (is == *running, format!("{process} {}", if *running { "запущен" } else { "не запущен" }))
        }
        Condition::ActiveApp { process } => {
            let (exe, _) = probe.foreground().await?;
            (exe.to_lowercase() == normalize_process(process), format!("активна программа {process} (сейчас: {exe})"))
        }
        Condition::Weekday { days } => (days.contains(&(now.weekday().number_from_monday() as u8)), format!("день недели: {}", days_text(days))),
        Condition::TimeRange { from, to } => {
            let ok = in_time_range(now.time(), parse_hhmm(from, "от")?, parse_hhmm(to, "до")?);
            (ok, format!("время с {from} до {to}"))
        }
        Condition::ModuleEnabled { module_id, enabled } => {
            let is = registry::is_enabled(db, module_id).await?;
            let name = registry::def(module_id).map(|m| m.name).unwrap_or(module_id);
            (is == *enabled, format!("модуль «{name}» {}", if *enabled { "включён" } else { "выключен" }))
        }
        Condition::ActiveLayout { layout_id } => {
            let active = crate::workspace::active_layout_id(db).await?;
            let name = crate::workspace::get(db, layout_id).await.map(|l| l.name).unwrap_or_else(|_| layout_id.clone());
            (active == *layout_id, format!("активен макет «{name}»"))
        }
        Condition::Internet { online } => {
            let is = probe.online().await?;
            (is == *online, if *online { "есть интернет".into() } else { "нет интернета".into() })
        }
    };
    Ok(CheckResult { ok, text })
}

// --- actions ---

#[derive(Debug, Clone, Serialize)]
pub struct ActionResult {
    pub action: String,
    pub ok: bool,
    pub message: String,
}

fn command_timeout(settings: &Value) -> Duration {
    Duration::from_secs(settings.get("command_timeout_secs").and_then(Value::as_u64).unwrap_or(60))
}

fn existing(path: &str) -> AppResult<String> {
    let p = expand_env(path.trim());
    if !std::path::Path::new(&p).exists() {
        return Err(AppError::NotFound(p));
    }
    Ok(p)
}

/// Runs one action; returns a short description of what happened.
async fn execute(app: &AppHandle, a: &Automation, action: &Action) -> AppResult<String> {
    let state = app.state::<AppState>();
    let db = &state.db;
    let source = format!("automation:{}", a.id);
    if let Some(m) = action.module() {
        if !state.module_enabled(m) {
            if m == NOTIFICATIONS {
                return Ok("пропущено: уведомления выключены".into());
            }
            let name = registry::def(m).map(|d| d.name).unwrap_or(m);
            return Err(AppError::ModuleDisabled(name.into()));
        }
    }
    if let Some(p) = action.permission() {
        permissions::require(db, p, &source).await?;
    }
    Ok(match action {
        Action::OpenApp { path, args } => {
            let p = existing(path)?;
            if args.trim().is_empty() || !p.to_lowercase().ends_with(".exe") {
                app.opener().open_path(&p, None::<&str>).map_err(anyhow::Error::from)?;
            } else {
                let mut cmd = std::process::Command::new(&p);
                #[cfg(windows)]
                std::os::windows::process::CommandExt::raw_arg(&mut cmd, args.trim());
                cmd.spawn().map_err(|e| AppError::Other(anyhow::anyhow!("не удалось запустить {p}: {e}")))?;
            }
            format!("запущено: {p}")
        }
        Action::CloseApp { process, force } => {
            let name = normalize_process(process);
            let mut cmd = format!("taskkill /IM \"{name}\"");
            if *force {
                cmd += " /F";
            }
            let out = crate::clipboard_hub::actions::run_command(&cmd, Duration::from_secs(20)).await?;
            match out.code {
                Some(0) => format!("закрыто: {name}"),
                Some(128) => format!("{name} не запущен"),
                _ => return Err(AppError::Other(anyhow::anyhow!("не удалось закрыть {name}: {}", out.stderr.trim()))),
            }
        }
        Action::OpenUrl { url } => {
            app.opener().open_url(url, None::<&str>).map_err(anyhow::Error::from)?;
            format!("открыто: {url}")
        }
        Action::OpenFolder { path } | Action::OpenFile { path } => {
            let p = existing(path)?;
            app.opener().open_path(&p, None::<&str>).map_err(anyhow::Error::from)?;
            format!("открыто: {p}")
        }
        Action::RunCommand { command } => {
            let settings = registry::settings(db, AUTOMATIONS).await?;
            let out = crate::clipboard_hub::actions::run_command(command, command_timeout(&settings)).await?;
            if out.timed_out {
                return Err(AppError::Other(anyhow::anyhow!("команда не завершилась вовремя и остановлена")));
            }
            let text: String = format!("{}{}", out.stdout, out.stderr).trim().chars().take(500).collect();
            if out.code != Some(0) {
                return Err(AppError::Other(anyhow::anyhow!("код завершения {:?}: {text}", out.code)));
            }
            if text.is_empty() { "команда выполнена".into() } else { text }
        }
        Action::SwitchLayout { layout_id, profile } => {
            let layouts = crate::workspace::layouts(db).await?;
            let target = layouts
                .iter()
                .find(|l| !layout_id.is_empty() && l.id == *layout_id)
                .or_else(|| layouts.iter().find(|l| !profile.is_empty() && l.profile == *profile))
                .ok_or_else(|| AppError::NotFound("макет".into()))?;
            crate::workspace::activate_from_hotkey(app, &target.id).await?;
            format!("макет «{}»", target.name)
        }
        Action::SetModule { module_id, enabled } => {
            registry::set_enabled(db, module_id, *enabled, false, "automation").await?;
            crate::modules::effects::apply_enabled(app, module_id).await?;
            let name = registry::def(module_id).map(|d| d.name).unwrap_or(module_id);
            format!("модуль «{name}» {}", if *enabled { "включён" } else { "выключен" })
        }
        Action::Notify { title, body } => {
            crate::notifications::push(app, "info", title, body, &a.name).await?;
            format!("уведомление «{title}»")
        }
        Action::CopyText { text } => {
            crate::clipboard_hub::copy_text(text).await?;
            "текст скопирован".into()
        }
        Action::PasteText { text } => {
            crate::clipboard_hub::copy_text(text).await?;
            crate::clipboard_hub::paste_into_previous(app).await?;
            "текст вставлен".into()
        }
        Action::Screenshot { folder } => {
            let dir = if folder.trim().is_empty() {
                app.path().picture_dir().map_err(anyhow::Error::from)?.join("FocusDeck")
            } else {
                expand_env(folder.trim()).into()
            };
            std::fs::create_dir_all(&dir).map_err(anyhow::Error::from)?;
            let bmp = tauri::async_runtime::spawn_blocking(system::screenshot_bmp)
                .await
                .map_err(anyhow::Error::from)?
                .map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;
            let path = dir.join(format!("screenshot-{}.bmp", Local::now().format("%Y%m%d-%H%M%S")));
            std::fs::write(&path, bmp).map_err(anyhow::Error::from)?;
            format!("снимок: {}", path.display())
        }
        Action::StartTimer { name, kind, minutes } => {
            use crate::productivity::timers;
            let ms = *minutes as i64 * 60_000;
            let existing = timers::timers(db).await?.into_iter().find(|t| t.name == *name && t.kind == *kind);
            let t = match existing {
                Some(t) => t,
                None => {
                    let input = timers::TimerInput {
                        name: name.clone(),
                        kind: kind.clone(),
                        duration_ms: ms,
                        work_ms: ms,
                        short_break_ms: 5 * 60_000,
                        long_break_ms: 15 * 60_000,
                        rounds: 4,
                    };
                    timers::save_timer(db, None, input).await?
                }
            };
            timers::control(db, &t.id, "start").await?;
            let _ = app.emit(timers::EVENT, ());
            format!("таймер «{name}» запущен")
        }
        Action::FocusMode { enabled, minutes } => {
            crate::focus::set(app, *enabled, *minutes);
            match (enabled, minutes) {
                (false, _) => "Focus Mode выключен".into(),
                (true, 0) => "Focus Mode включён".into(),
                (true, m) => format!("Focus Mode на {m} мин"),
            }
        }
        Action::DeleteFiles { folder, pattern, older_than_days } => {
            let dir = existing(folder)?;
            let (pattern, days) = (pattern.clone(), *older_than_days);
            let n = tauri::async_runtime::spawn_blocking(move || -> Result<usize, String> {
                let files = system::matching_files(std::path::Path::new(&dir), &pattern, days).map_err(|e| e.to_string())?;
                system::recycle(&files)?;
                Ok(files.len())
            })
            .await
            .map_err(anyhow::Error::from)?
            .map_err(|e| AppError::Other(anyhow::anyhow!("не удалось удалить файлы: {e}")))?;
            format!("в Корзину перемещено файлов: {n}")
        }
        Action::Log { message } => {
            tracing::info!(automation = %a.name, "{message}");
            "записано в журнал".into()
        }
        Action::ShowWindow => {
            crate::window::show(app);
            "окно показано".into()
        }
        Action::HideWindow => {
            crate::window::hide(app);
            "окно скрыто".into()
        }
    })
}

/// Human-readable plan line for an action (dry run, confirmation dialog).
pub fn describe(action: &Action) -> String {
    match action {
        Action::OpenApp { path, .. } => format!("открыть программу {path}"),
        Action::CloseApp { process, force } => format!("{} {process}", if *force { "принудительно закрыть" } else { "закрыть" }),
        Action::OpenUrl { url } => format!("открыть {url}"),
        Action::OpenFolder { path } => format!("открыть папку {path}"),
        Action::OpenFile { path } => format!("открыть файл {path}"),
        Action::RunCommand { command } => format!("выполнить команду: {command}"),
        Action::SwitchLayout { layout_id, profile } => format!("переключить макет {}", if layout_id.is_empty() { profile } else { layout_id }),
        Action::SetModule { module_id, enabled } => format!("{} модуль {module_id}", if *enabled { "включить" } else { "выключить" }),
        Action::Notify { title, .. } => format!("уведомление «{title}»"),
        Action::CopyText { .. } => "скопировать текст".into(),
        Action::PasteText { .. } => "вставить текст".into(),
        Action::Screenshot { .. } => "снимок экрана".into(),
        Action::StartTimer { name, .. } => format!("запустить таймер «{name}»"),
        Action::FocusMode { enabled, .. } => format!("{} Focus Mode", if *enabled { "включить" } else { "выключить" }),
        Action::DeleteFiles { folder, pattern, older_than_days } => {
            format!("переместить в Корзину {pattern} из {folder}{}", if *older_than_days > 0 { format!(" старше {older_than_days} дн.") } else { String::new() })
        }
        Action::Log { message } => format!("журнал: {message}"),
        Action::ShowWindow => "показать окно".into(),
        Action::HideWindow => "скрыть окно".into(),
    }
}

// --- runs ---

#[derive(Debug, Clone, Copy, Default)]
pub struct RunOptions {
    /// Run even within the cooldown (manual runs).
    pub ignore_cooldown: bool,
    /// The user confirmed the dangerous actions of this run.
    pub confirmed: bool,
    /// Only check modules, conditions and permissions; don't execute.
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunReport {
    /// ok | error | skipped | pending | dry_run
    pub status: String,
    pub message: String,
    pub conditions: Vec<CheckResult>,
    pub plan: Vec<String>,
    pub actions: Vec<ActionResult>,
    pub missing_permissions: Vec<String>,
    pub dangers: Vec<String>,
}

impl RunReport {
    fn new(a: &Automation) -> Self {
        Self {
            status: String::new(),
            message: String::new(),
            conditions: vec![],
            plan: a.actions.iter().map(describe).collect(),
            actions: vec![],
            missing_permissions: vec![],
            dangers: a.dangerous.clone(),
        }
    }
}

/// A dangerous automatic run waiting for the user.
#[derive(Debug, Clone, Serialize)]
pub struct PendingRun {
    pub token: String,
    pub automation_id: String,
    pub name: String,
    pub source: String,
    pub dangers: Vec<String>,
    pub plan: Vec<String>,
    #[serde(skip)]
    pub created: Option<Instant>,
}

static PENDING: Mutex<Vec<PendingRun>> = Mutex::new(Vec::new());

pub fn pending() -> Vec<PendingRun> {
    let mut p = PENDING.lock().unwrap();
    p.retain(|r| r.created.is_some_and(|c| c.elapsed() < CONFIRM_TTL));
    p.clone()
}

pub fn take_pending(token: &str) -> Option<PendingRun> {
    pending();
    let mut p = PENDING.lock().unwrap();
    let i = p.iter().position(|r| r.token == token)?;
    Some(p.remove(i))
}

fn is_user_source(source: &str) -> bool {
    matches!(source, "manual" | "test" | "ui_button")
}

async fn write_log(app: &AppHandle, a: &Automation, source: &str, report: &RunReport, started: Instant) {
    let db = &app.state::<AppState>().db;
    let keep = registry::settings(db, AUTOMATIONS).await.ok().and_then(|s| s.get("max_log_entries").and_then(Value::as_i64)).unwrap_or(500);
    let details = json!({ "conditions": report.conditions, "actions": report.actions });
    let status = if report.status == "pending" { "skipped" } else { report.status.as_str() };
    let entry = LogEntry { status, message: &report.message, source, duration_ms: started.elapsed().as_millis() as i64, details: &details };
    if let Err(e) = store::log_entry(db, &a.id, entry, keep).await {
        tracing::error!("cannot write automation log: {e}");
    }
    let _ = app.emit(RUN_EVENT, json!({ "automation_id": a.id, "status": report.status }));
}

/// The whole run pipeline. `source` is the trigger kind or `manual` / `test` /
/// `ui_button` / `hotkey` / `confirmed`.
pub async fn run(app: &AppHandle, a: &Automation, source: &str, opts: RunOptions) -> AppResult<RunReport> {
    let state = app.state::<AppState>();
    let db = &state.db;
    let started = Instant::now();
    let mut report = RunReport::new(a);

    if let Some(reason) = blocked_reason(db, a).await? {
        report.status = "skipped".into();
        report.message = reason;
        if !opts.dry_run {
            write_log(app, a, source, &report, started).await;
        }
        return Ok(report);
    }
    if !opts.ignore_cooldown && !opts.dry_run && in_cooldown(a, Utc::now()) {
        report.status = "skipped".into();
        report.message = format!("пауза между запусками ({} с)", a.cooldown_secs);
        return Ok(report);
    }

    let mut probe = Probe::default();
    for c in &a.conditions {
        report.conditions.push(check_condition(app, c, &mut probe).await?);
    }
    for p in &a.permissions {
        if !permissions::is_granted(db, p).await? {
            report.missing_permissions.push(p.clone());
        }
    }
    if opts.dry_run {
        report.status = "dry_run".into();
        let failed = report.conditions.iter().filter(|c| !c.ok).count();
        report.message = if failed > 0 { format!("не выполнено условий: {failed}") } else { "все условия выполнены".into() };
        return Ok(report);
    }
    if let Some(c) = report.conditions.iter().find(|c| !c.ok) {
        report.status = "skipped".into();
        report.message = format!("условие не выполнено: {}", c.text);
        if !(a.trigger.is_frequent() && !is_user_source(source)) {
            write_log(app, a, source, &report, started).await;
        }
        return Ok(report);
    }

    if !a.dangerous.is_empty() && !opts.confirmed {
        let what = a.dangerous.join(", ");
        if is_user_source(source) {
            return Err(AppError::ConfirmationRequired(format!("Автоматизация «{}» выполнит опасные действия: {what}. Продолжить?", a.name)));
        }
        let run = PendingRun {
            token: crate::db::new_id(),
            automation_id: a.id.clone(),
            name: a.name.clone(),
            source: source.into(),
            dangers: a.dangerous.clone(),
            plan: report.plan.clone(),
            created: Some(Instant::now()),
        };
        PENDING.lock().unwrap().push(run.clone());
        crate::window::show(app);
        let _ = app.emit(CONFIRM_EVENT, &run);
        let _ = crate::notifications::push(app, "warn", "Нужно подтверждение", &format!("«{}»: {what}", a.name), "Автоматизации").await;
        report.status = "pending".into();
        report.message = format!("ожидает подтверждения: {what}");
        write_log(app, a, source, &report, started).await;
        return Ok(report);
    }

    if !report.missing_permissions.is_empty() {
        report.status = "error".into();
        report.message = format!("нет разрешений: {}", report.missing_permissions.join(", "));
        write_log(app, a, source, &report, started).await;
        return Ok(report);
    }

    let _ = store::mark_run(db, &a.id, &Utc::now().to_rfc3339()).await;
    report.status = "ok".into();
    for action in &a.actions {
        match execute(app, a, action).await {
            Ok(message) => report.actions.push(ActionResult { action: action.kind().into(), ok: true, message }),
            Err(e) => {
                report.actions.push(ActionResult { action: action.kind().into(), ok: false, message: e.to_string() });
                report.status = "error".into();
                break;
            }
        }
    }
    report.message = match report.status.as_str() {
        "ok" => format!("выполнено действий: {}", report.actions.len()),
        _ => format!("ошибка в действии {}: {}", report.actions.len(), report.actions.last().map(|r| r.message.as_str()).unwrap_or("")),
    };
    tracing::info!(automation = %a.id, source, status = %report.status, "{}", report.message);
    write_log(app, a, source, &report, started).await;
    Ok(report)
}

/// Fires an automation from a trigger without blocking the caller.
fn fire(app: &AppHandle, a: Automation, source: &'static str) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run(&app, &a, source, RunOptions::default()).await {
            tracing::warn!(automation = %a.id, "{e}");
        }
    });
}

pub async fn run_by_id(app: &AppHandle, id: &str) -> AppResult<()> {
    let a = store::get(&app.state::<AppState>().db, id).await?;
    fire(app, a, "hotkey");
    Ok(())
}

/// FocusDeck start: `app_start`, and `system_start` when Windows booted recently.
pub async fn run_startup_automations(app: &AppHandle) {
    let state = app.state::<AppState>();
    let uptime = tauri::async_runtime::spawn_blocking(system::uptime_secs).await.unwrap_or(u64::MAX);
    match store::list(&state.db).await {
        Ok(list) => {
            for a in list.into_iter().filter(|a| a.enabled) {
                match a.trigger {
                    Trigger::AppStart => fire(app, a, "app_start"),
                    Trigger::SystemStart { within_minutes } if uptime < within_minutes as u64 * 60 => fire(app, a, "system_start"),
                    _ => {}
                }
            }
        }
        Err(e) => tracing::error!("cannot load automations: {e}"),
    }
}

fn change_matches(kind: ChangeKind, up: bool) -> bool {
    match kind {
        ChangeKind::Any => true,
        ChangeKind::Up => up,
        ChangeKind::Down => !up,
    }
}

fn window_matches(t: &Trigger, exe: &str, title: &str) -> bool {
    let Trigger::WindowFocus { process, title_contains } = t else { return false };
    (process.trim().is_empty() || exe.to_lowercase() == normalize_process(process))
        && (title_contains.trim().is_empty() || title.to_lowercase().contains(&title_contains.trim().to_lowercase()))
}

/// The watcher: a background task of the Automations module.
pub async fn scheduler_loop(app: AppHandle) {
    let state = app.state::<AppState>();
    let period = registry::settings(&state.db, AUTOMATIONS).await.ok().and_then(|s| s.get("scheduler_interval_secs").and_then(Value::as_u64)).unwrap_or(30);
    tracing::info!(period, "automation watcher started");
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    let mut tick: u64 = 0;
    let mut fg: Option<(String, String)> = None;
    let mut procs: Option<HashSet<String>> = None;
    let mut monitors: Option<i32> = None;
    let mut online: Option<bool> = None;
    let mut last_minute = String::new();
    let mut list: Vec<Automation> = Vec::new();
    loop {
        ticker.tick().await;
        tick += 1;
        if tick % 2 == 1 {
            match store::list(&state.db).await {
                Ok(l) => list = l.into_iter().filter(|a| a.enabled).collect(),
                Err(e) => tracing::warn!("automations: {e}"),
            }
        }
        if list.is_empty() {
            continue;
        }
        let has = |f: fn(&Trigger) -> bool| list.iter().any(|a| f(&a.trigger));

        // Active window.
        if has(|t| matches!(t, Trigger::WindowFocus { .. })) {
            if let Ok(now) = tauri::async_runtime::spawn_blocking(system::foreground).await {
                if fg.as_ref() != Some(&now) {
                    for a in list.iter().filter(|a| window_matches(&a.trigger, &now.0, &now.1)) {
                        let was = fg.as_ref().is_some_and(|(e, t)| window_matches(&a.trigger, e, t));
                        if !was {
                            fire(&app, a.clone(), "window_focus");
                        }
                    }
                    fg = Some(now);
                }
            }
        }

        // Processes started / stopped.
        if tick % 2 == 0 && has(|t| matches!(t, Trigger::ProcessStarted { .. } | Trigger::ProcessStopped { .. })) {
            if let Ok(now) = tauri::async_runtime::spawn_blocking(system::processes).await {
                if let Some(prev) = &procs {
                    for a in &list {
                        match &a.trigger {
                            Trigger::ProcessStarted { process } if now.contains(&normalize_process(process)) && !prev.contains(&normalize_process(process)) => {
                                fire(&app, a.clone(), "process_started")
                            }
                            Trigger::ProcessStopped { process } if !now.contains(&normalize_process(process)) && prev.contains(&normalize_process(process)) => {
                                fire(&app, a.clone(), "process_stopped")
                            }
                            _ => {}
                        }
                    }
                }
                procs = Some(now);
            }
        } else if !has(|t| matches!(t, Trigger::ProcessStarted { .. } | Trigger::ProcessStopped { .. })) {
            procs = None;
        }

        // Monitors.
        if tick % 2 == 0 && has(|t| matches!(t, Trigger::DisplayChange { .. })) {
            let now = tauri::async_runtime::spawn_blocking(system::monitors).await.unwrap_or(1);
            if let Some(prev) = monitors.filter(|p| *p != now) {
                for a in &list {
                    if let Trigger::DisplayChange { event } = a.trigger {
                        if change_matches(event, now > prev) {
                            fire(&app, a.clone(), "display_change");
                        }
                    }
                }
            }
            monitors = Some(now);
        }

        // Network.
        if tick % 5 == 0 && has(|t| matches!(t, Trigger::NetworkChange { .. })) {
            let now = tauri::async_runtime::spawn_blocking(system::online).await.unwrap_or(true);
            if let Some(prev) = online.filter(|p| *p != now) {
                for a in &list {
                    if let Trigger::NetworkChange { event } = a.trigger {
                        if change_matches(event, now && !prev) {
                            fire(&app, a.clone(), "network_change");
                        }
                    }
                }
            }
            online = Some(now);
        }

        // Clock: once per matching minute.
        let local = Local::now();
        let minute = local.format("%H:%M").to_string();
        if minute != last_minute {
            if !last_minute.is_empty() {
                let wd = local.weekday().number_from_monday() as u8;
                for a in &list {
                    if let Trigger::Time { time, days } = &a.trigger {
                        if parse_hhmm(time, "").is_ok_and(|t| t.format("%H:%M").to_string() == minute) && (days.is_empty() || days.contains(&wd)) {
                            fire(&app, a.clone(), "time");
                        }
                    }
                }
            }
            last_minute = minute;
        }

        // Intervals.
        if tick % period == 0 {
            let now = Utc::now();
            for a in list.iter().filter(|a| is_due(a, now)) {
                fire(&app, a.clone(), "interval");
            }
        }
    }
}

/// Runs a confirmed pending run (or records the refusal).
pub async fn resolve_pending(app: &AppHandle, token: &str, approve: bool) -> AppResult<Option<RunReport>> {
    let p = take_pending(token).ok_or_else(|| AppError::NotFound("запрос подтверждения (истёк или уже обработан)".into()))?;
    let a = store::get(&app.state::<AppState>().db, &p.automation_id).await?;
    if !approve {
        let mut report = RunReport::new(&a);
        report.status = "skipped".into();
        report.message = "отклонено пользователем".into();
        write_log(app, &a, &p.source, &report, Instant::now()).await;
        return Ok(None);
    }
    let opts = RunOptions { ignore_cooldown: true, confirmed: true, dry_run: false };
    Ok(Some(run(app, &a, "confirmed", opts).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::types::AutomationInput;

    fn automation(trigger: Trigger, created: &str, last: Option<&str>) -> Automation {
        Automation {
            id: "a".into(),
            name: "a".into(),
            description: String::new(),
            enabled: true,
            module_id: AUTOMATIONS.into(),
            trigger,
            conditions: vec![],
            actions: vec![],
            cooldown_secs: 0,
            template: None,
            permissions: vec![],
            dangerous: vec![],
            last_run_at: last.map(Into::into),
            created_at: created.into(),
            updated_at: created.into(),
        }
    }

    #[test]
    fn parses_both_timestamp_formats() {
        assert!(parse_ts("2026-10-20 08:00:00").is_some());
        assert!(parse_ts("2026-10-20T08:00:00+00:00").is_some());
        assert!(parse_ts("garbage").is_none());
    }

    #[test]
    fn interval_and_cooldown() {
        let now = parse_ts("2026-10-20 10:00:00").unwrap();
        let every_30 = Trigger::Interval { minutes: 30 };
        assert!(!is_due(&automation(every_30.clone(), "2026-10-20 09:45:00", None), now));
        assert!(is_due(&automation(every_30.clone(), "2026-10-20 09:00:00", None), now));
        assert!(!is_due(&automation(every_30.clone(), "2026-10-20 08:00:00", Some("2026-10-20T09:40:00+00:00")), now));
        assert!(!is_due(&automation(Trigger::AppStart, "2026-10-20 00:00:00", None), now));

        let mut a = automation(Trigger::Manual, "2026-10-20 00:00:00", Some("2026-10-20T09:59:30+00:00"));
        assert!(!in_cooldown(&a, now));
        a.cooldown_secs = 60;
        assert!(in_cooldown(&a, now));
        a.cooldown_secs = 10;
        assert!(!in_cooldown(&a, now));
    }

    #[test]
    fn time_ranges_and_matching() {
        let t = |s: &str| NaiveTime::parse_from_str(s, "%H:%M").unwrap();
        assert!(in_time_range(t("10:00"), t("09:00"), t("18:00")));
        assert!(!in_time_range(t("18:00"), t("09:00"), t("18:00")));
        assert!(in_time_range(t("23:30"), t("22:00"), t("06:00")));
        assert!(in_time_range(t("05:59"), t("22:00"), t("06:00")));
        assert!(!in_time_range(t("12:00"), t("22:00"), t("06:00")));

        let w = Trigger::WindowFocus { process: "Code".into(), title_contains: "focusdeck".into() };
        assert!(window_matches(&w, "Code.exe", "main.rs — FocusDeck"));
        assert!(!window_matches(&w, "notepad.exe", "FocusDeck"));
        assert!(change_matches(ChangeKind::Down, false) && !change_matches(ChangeKind::Up, false));
    }

    #[tokio::test]
    async fn disabled_modules_block_runs() {
        let pool = registry::test_pool().await;
        let input = AutomationInput {
            name: "Задачи".into(),
            description: String::new(),
            enabled: true,
            module_id: Some("tasks".into()),
            trigger: Trigger::AppStart,
            conditions: vec![],
            actions: vec![Action::Log { message: "x".into() }],
            cooldown_secs: 0,
        };
        let a = store::create(&pool, &input).await.unwrap();
        assert_eq!(blocked_reason(&pool, &a).await.unwrap(), None);
        registry::set_enabled(&pool, "tasks", false, false, "test").await.unwrap();
        assert_eq!(blocked_reason(&pool, &a).await.unwrap().unwrap(), "модуль «Задачи» отключён");
        registry::set_enabled(&pool, "tasks", true, false, "test").await.unwrap();
        registry::set_enabled(&pool, AUTOMATIONS, false, false, "test").await.unwrap();
        assert!(blocked_reason(&pool, &a).await.unwrap().unwrap().contains("Автоматизации"));
    }

    #[test]
    fn pending_runs_expire() {
        PENDING.lock().unwrap().push(PendingRun {
            token: "t1".into(),
            automation_id: "a".into(),
            name: "n".into(),
            source: "time".into(),
            dangers: vec![],
            plan: vec![],
            created: Some(Instant::now()),
        });
        let stale = PendingRun { token: "old".into(), created: None, ..pending()[0].clone() };
        PENDING.lock().unwrap().push(stale);
        assert_eq!(pending().len(), 1, "invalid/expired entries are dropped");
        assert!(take_pending("t1").is_some());
        assert!(take_pending("t1").is_none());
    }
}
