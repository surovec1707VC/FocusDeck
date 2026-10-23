//! Automation model: trigger → conditions → actions. The JSON shape is
//! described by `docs/automation.schema.json` (served by `automations_schema`).

use std::str::FromStr;

use chrono::NaiveTime;
use serde::{Deserialize, Serialize};
use tauri_plugin_global_shortcut::Shortcut;

use crate::error::{AppError, AppResult};
use crate::permissions::{PERM_CLOSE_APPS, PERM_DELETE_FILES, PERM_LAUNCH_APPS, PERM_NOTIFICATIONS, PERM_PASTE, PERM_RUN_COMMANDS, PERM_SCREEN};

pub const MAX_INTERVAL_MINUTES: u32 = 24 * 60;
pub const MAX_ACTIONS: usize = 20;
pub const MAX_CONDITIONS: usize = 10;
pub const MAX_COOLDOWN_SECS: u32 = 7 * 24 * 3600;

/// What starts an automation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    /// FocusDeck started.
    AppStart,
    /// FocusDeck started within `within_minutes` after Windows started (autostart).
    SystemStart {
        #[serde(default = "default_boot_window")]
        within_minutes: u32,
    },
    /// A program started (process name, e.g. `obs64.exe`).
    ProcessStarted { process: String },
    /// A program exited.
    ProcessStopped { process: String },
    /// The active window changed to a matching program and/or title.
    WindowFocus {
        #[serde(default)]
        process: String,
        #[serde(default)]
        title_contains: String,
    },
    /// A global shortcut.
    Hotkey { shortcut: String },
    /// Every day at `time` (HH:MM) on the given ISO weekdays (1 = Mon … 7 = Sun; empty = every day).
    Time {
        time: String,
        #[serde(default)]
        days: Vec<u8>,
    },
    /// Every N minutes while FocusDeck runs.
    Interval { minutes: u32 },
    /// A monitor was connected or disconnected.
    DisplayChange {
        #[serde(default)]
        event: ChangeKind,
    },
    /// The computer went online or offline.
    NetworkChange {
        #[serde(default)]
        event: ChangeKind,
    },
    /// Only by «Запустить» in the editor or another automation.
    Manual,
    /// A button on the Automations page and the dashboard.
    UiButton {
        #[serde(default)]
        label: String,
    },
}

/// For display/network triggers: `connected`/`online` = `up`, `disconnected`/`offline` = `down`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    #[default]
    Any,
    Up,
    Down,
}

fn default_boot_window() -> u32 {
    10
}

impl Trigger {
    /// Fires often by itself: condition skips aren't logged.
    pub fn is_frequent(&self) -> bool {
        matches!(self, Self::WindowFocus { .. } | Self::ProcessStarted { .. } | Self::ProcessStopped { .. } | Self::Interval { .. })
    }
}

/// Checks before the actions run; all must hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    ProcessRunning {
        process: String,
        #[serde(default = "yes")]
        running: bool,
    },
    ActiveApp { process: String },
    /// ISO weekdays, 1 = Monday.
    Weekday { days: Vec<u8> },
    /// Local time between `from` and `to` (HH:MM); may cross midnight.
    TimeRange { from: String, to: String },
    ModuleEnabled {
        module_id: String,
        #[serde(default = "yes")]
        enabled: bool,
    },
    ActiveLayout { layout_id: String },
    Internet {
        #[serde(default = "yes")]
        online: bool,
    },
}

fn yes() -> bool {
    true
}

/// What an automation does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// `.exe`, `.lnk`, `.url`; `%VARS%` are expanded.
    OpenApp {
        path: String,
        #[serde(default)]
        args: String,
    },
    /// Asks the program to close (`taskkill`); `force` kills it. Dangerous.
    CloseApp {
        process: String,
        #[serde(default)]
        force: bool,
    },
    OpenUrl { url: String },
    OpenFolder { path: String },
    /// Opening scripts or programs this way is dangerous.
    OpenFile { path: String },
    /// `cmd /C`. Dangerous.
    RunCommand { command: String },
    /// By id, or the first layout with this profile.
    SwitchLayout {
        #[serde(default)]
        layout_id: String,
        #[serde(default)]
        profile: String,
    },
    SetModule { module_id: String, enabled: bool },
    Notify {
        title: String,
        #[serde(default)]
        body: String,
    },
    CopyText { text: String },
    PasteText { text: String },
    /// Saves the whole screen as BMP into `folder` (default: Pictures\FocusDeck).
    Screenshot {
        #[serde(default)]
        folder: String,
    },
    /// Starts a timer of the Timers module, created if it doesn't exist.
    StartTimer {
        name: String,
        #[serde(default = "countdown")]
        kind: String,
        #[serde(default = "default_minutes")]
        minutes: u32,
    },
    /// Focus Mode: toasts are muted (still saved); `minutes = 0` → until turned off.
    FocusMode {
        enabled: bool,
        #[serde(default)]
        minutes: u32,
    },
    /// Moves matching files to the Recycle Bin. Dangerous.
    DeleteFiles {
        folder: String,
        #[serde(default = "all_files")]
        pattern: String,
        #[serde(default)]
        older_than_days: u32,
    },
    /// A line in the application log.
    Log { message: String },
    ShowWindow,
    HideWindow,
}

fn countdown() -> String {
    "countdown".into()
}
fn default_minutes() -> u32 {
    25
}
fn all_files() -> String {
    "*".into()
}

/// Files that start programs when opened.
pub const SCRIPT_EXTENSIONS: &[&str] = &["exe", "bat", "cmd", "ps1", "vbs", "js", "wsf", "msi", "com", "scr", "lnk", "hta"];

impl Action {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::OpenApp { .. } => "open_app",
            Self::CloseApp { .. } => "close_app",
            Self::OpenUrl { .. } => "open_url",
            Self::OpenFolder { .. } => "open_folder",
            Self::OpenFile { .. } => "open_file",
            Self::RunCommand { .. } => "run_command",
            Self::SwitchLayout { .. } => "switch_layout",
            Self::SetModule { .. } => "set_module",
            Self::Notify { .. } => "notify",
            Self::CopyText { .. } => "copy_text",
            Self::PasteText { .. } => "paste_text",
            Self::Screenshot { .. } => "screenshot",
            Self::StartTimer { .. } => "start_timer",
            Self::FocusMode { .. } => "focus_mode",
            Self::DeleteFiles { .. } => "delete_files",
            Self::Log { .. } => "log",
            Self::ShowWindow => "show_window",
            Self::HideWindow => "hide_window",
        }
    }

    /// Why the action needs the user's confirmation before every run, if it does.
    pub fn danger(&self) -> Option<&'static str> {
        match self {
            Self::CloseApp { .. } => Some("закрытие программы"),
            Self::RunCommand { .. } => Some("команда терминала"),
            Self::DeleteFiles { .. } => Some("удаление файлов"),
            Self::OpenFile { path } => {
                let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
                SCRIPT_EXTENSIONS.contains(&ext.as_str()).then_some("запуск скрипта или программы")
            }
            _ => None,
        }
    }

    pub fn permission(&self) -> Option<&'static str> {
        match self {
            Self::OpenApp { .. } => Some(PERM_LAUNCH_APPS),
            Self::OpenFile { .. } if self.danger().is_some() => Some(PERM_LAUNCH_APPS),
            Self::CloseApp { .. } => Some(PERM_CLOSE_APPS),
            Self::RunCommand { .. } => Some(PERM_RUN_COMMANDS),
            Self::PasteText { .. } => Some(PERM_PASTE),
            Self::DeleteFiles { .. } => Some(PERM_DELETE_FILES),
            Self::Notify { .. } => Some(PERM_NOTIFICATIONS),
            Self::Screenshot { .. } => Some(PERM_SCREEN),
            _ => None,
        }
    }

    /// Module the action belongs to (it fails while the module is off).
    pub fn module(&self) -> Option<&'static str> {
        use crate::modules::catalog::*;
        match self {
            Self::SwitchLayout { .. } => Some(LAYOUTS),
            Self::StartTimer { .. } => Some(TIMERS),
            Self::Notify { .. } => Some(NOTIFICATIONS),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Automation {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    /// Owning module; the automation doesn't run while it is off.
    pub module_id: String,
    pub trigger: Trigger,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    /// Minimum seconds between automatic runs.
    pub cooldown_secs: u32,
    /// Template key it was created from.
    pub template: Option<String>,
    /// Permissions the actions need (derived).
    pub permissions: Vec<String>,
    /// Dangerous actions: every run must be confirmed.
    pub dangerous: Vec<String>,
    pub last_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub enabled: bool,
    /// Owning module (default: `automations`).
    #[serde(default)]
    pub module_id: Option<String>,
    pub trigger: Trigger,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    #[serde(default)]
    pub cooldown_secs: u32,
}

fn len_between(value: &str, min: usize, max: usize, what: &str) -> AppResult<()> {
    let n = value.trim().chars().count();
    if n < min || n > max {
        return Err(AppError::Invalid(format!("{what}: от {min} до {max} символов")));
    }
    Ok(())
}

pub fn parse_hhmm(s: &str, what: &str) -> AppResult<NaiveTime> {
    NaiveTime::parse_from_str(s.trim(), "%H:%M").map_err(|_| AppError::Invalid(format!("{what}: время в формате ЧЧ:ММ")))
}

fn check_days(days: &[u8]) -> AppResult<()> {
    if days.iter().any(|d| !(1..=7).contains(d)) {
        return Err(AppError::Invalid("дни недели: от 1 (пн) до 7 (вс)".into()));
    }
    Ok(())
}

fn check_process(p: &str) -> AppResult<()> {
    len_between(p, 1, 100, "имя процесса")?;
    // Letters, digits and a few safe characters only: names reach taskkill and process lists.
    if !p.chars().all(|c| c.is_alphanumeric() || " ._-+()".contains(c)) {
        return Err(AppError::Invalid("имя процесса без пути, например obs64.exe".into()));
    }
    Ok(())
}

/// Paths may start with `%VAR%` (expanded at run time) or be absolute.
fn check_path(p: &str, what: &str) -> AppResult<()> {
    len_between(p, 1, 1000, what)?;
    let t = p.trim();
    if !(t.starts_with('%') || std::path::Path::new(t).is_absolute()) {
        return Err(AppError::Invalid(format!("{what}: нужен полный путь или путь с %ПЕРЕМЕННОЙ%")));
    }
    Ok(())
}

impl AutomationInput {
    pub fn module_id(&self) -> &str {
        self.module_id.as_deref().unwrap_or(crate::modules::catalog::AUTOMATIONS)
    }

    pub fn validate(&self) -> AppResult<()> {
        len_between(&self.name, 1, 60, "название")?;
        len_between(&self.description, 0, 500, "описание")?;
        if crate::modules::catalog::find(self.module_id()).is_none() {
            return Err(AppError::Invalid(format!("неизвестный модуль «{}»", self.module_id())));
        }
        if self.cooldown_secs > MAX_COOLDOWN_SECS {
            return Err(AppError::Invalid("пауза между запусками — не больше 7 дней".into()));
        }
        match &self.trigger {
            Trigger::Interval { minutes } if *minutes == 0 || *minutes > MAX_INTERVAL_MINUTES => {
                return Err(AppError::Invalid(format!("интервал: от 1 до {MAX_INTERVAL_MINUTES} минут")));
            }
            Trigger::Hotkey { shortcut } => {
                Shortcut::from_str(shortcut.trim()).map_err(|e| AppError::Invalid(format!("сочетание «{shortcut}» не распознано: {e}")))?;
            }
            Trigger::Time { time, days } => {
                parse_hhmm(time, "время запуска")?;
                check_days(days)?;
            }
            Trigger::ProcessStarted { process } | Trigger::ProcessStopped { process } => check_process(process)?,
            Trigger::WindowFocus { process, title_contains } => {
                if process.trim().is_empty() && title_contains.trim().is_empty() {
                    return Err(AppError::Invalid("укажите программу или часть заголовка окна".into()));
                }
                if !process.trim().is_empty() {
                    check_process(process)?;
                }
                len_between(title_contains, 0, 200, "заголовок окна")?;
            }
            Trigger::SystemStart { within_minutes } if !(1..=120).contains(within_minutes) => {
                return Err(AppError::Invalid("старт системы: от 1 до 120 минут после загрузки".into()));
            }
            Trigger::UiButton { label } => len_between(label, 0, 30, "подпись кнопки")?,
            _ => {}
        }
        if self.conditions.len() > MAX_CONDITIONS {
            return Err(AppError::Invalid(format!("не больше {MAX_CONDITIONS} условий")));
        }
        for c in &self.conditions {
            match c {
                Condition::ProcessRunning { process, .. } | Condition::ActiveApp { process } => check_process(process)?,
                Condition::Weekday { days } => {
                    check_days(days)?;
                    if days.is_empty() {
                        return Err(AppError::Invalid("выберите хотя бы один день недели".into()));
                    }
                }
                Condition::TimeRange { from, to } => {
                    parse_hhmm(from, "начало интервала")?;
                    parse_hhmm(to, "конец интервала")?;
                }
                Condition::ModuleEnabled { module_id, .. } if crate::modules::catalog::find(module_id).is_none() => {
                    return Err(AppError::Invalid(format!("неизвестный модуль «{module_id}»")));
                }
                Condition::ActiveLayout { layout_id } => len_between(layout_id, 1, 64, "макет")?,
                _ => {}
            }
        }
        if self.actions.is_empty() || self.actions.len() > MAX_ACTIONS {
            return Err(AppError::Invalid(format!("нужно от 1 до {MAX_ACTIONS} действий")));
        }
        for a in &self.actions {
            match a {
                Action::OpenApp { path, args } => {
                    check_path(path, "программа")?;
                    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
                    if !["exe", "lnk", "url", "appref-ms"].contains(&ext.as_str()) {
                        return Err(AppError::Invalid("программа: .exe, .lnk, .url или .appref-ms".into()));
                    }
                    len_between(args, 0, 1000, "аргументы")?;
                }
                Action::CloseApp { process, .. } => check_process(process)?,
                Action::OpenUrl { url } if !crate::widgets::valid_link(url) => {
                    return Err(AppError::Invalid("ссылка: только http://, https:// или mailto:".into()));
                }
                Action::OpenFolder { path } | Action::OpenFile { path } => check_path(path, "путь")?,
                Action::RunCommand { command } => len_between(command, 1, 2000, "команда")?,
                Action::SwitchLayout { layout_id, profile } => {
                    if layout_id.trim().is_empty() && !crate::workspace::PROFILES.contains(&profile.as_str()) {
                        return Err(AppError::Invalid("выберите макет или профиль макета".into()));
                    }
                }
                Action::SetModule { module_id, enabled } => match crate::modules::catalog::find(module_id) {
                    None => return Err(AppError::Invalid(format!("неизвестный модуль «{module_id}»"))),
                    Some(m) if m.critical && !enabled => {
                        return Err(AppError::Invalid(format!("критический модуль «{}» автоматизация отключить не может", m.name)));
                    }
                    Some(m) if m.id == crate::modules::catalog::AUTOMATIONS => {
                        return Err(AppError::Invalid("автоматизация не может менять сам модуль «Автоматизации»".into()));
                    }
                    _ => {}
                },
                Action::Notify { title, body } => {
                    len_between(title, 1, 80, "заголовок уведомления")?;
                    len_between(body, 0, 500, "текст уведомления")?;
                }
                Action::CopyText { text } | Action::PasteText { text } => len_between(text, 1, 50_000, "текст")?,
                Action::Screenshot { folder } if !folder.trim().is_empty() => check_path(folder, "папка")?,
                Action::StartTimer { name, kind, minutes } => {
                    len_between(name, 1, 60, "название таймера")?;
                    if !["countdown", "pomodoro"].contains(&kind.as_str()) || !(1..=600).contains(minutes) {
                        return Err(AppError::Invalid("таймер: countdown или pomodoro, от 1 до 600 минут".into()));
                    }
                }
                Action::FocusMode { minutes, .. } if *minutes > 24 * 60 => {
                    return Err(AppError::Invalid("Focus Mode — не дольше 24 часов".into()));
                }
                Action::DeleteFiles { folder, pattern, .. } => {
                    check_path(folder, "папка")?;
                    len_between(pattern, 1, 100, "маска")?;
                    if pattern.contains(['\\', '/']) || pattern.contains("..") {
                        return Err(AppError::Invalid("маска — только имя файла, например *.tmp".into()));
                    }
                }
                Action::Log { message } => len_between(message, 1, 500, "сообщение для журнала")?,
                _ => {}
            }
        }
        Ok(())
    }
}

/// Permissions and dangers of a list of actions (deduplicated).
pub fn requirements(actions: &[Action]) -> (Vec<String>, Vec<String>) {
    let mut perms: Vec<String> = Vec::new();
    let mut dangers: Vec<String> = Vec::new();
    for a in actions {
        if let Some(p) = a.permission().filter(|p| !perms.iter().any(|x| x == p)) {
            perms.push(p.into());
        }
        if let Some(d) = a.danger().filter(|d| !dangers.iter().any(|x| x == d)) {
            dangers.push(d.into());
        }
    }
    (perms, dangers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn input(trigger: Trigger, actions: Vec<Action>) -> AutomationInput {
        AutomationInput { name: "Тест".into(), description: String::new(), enabled: true, module_id: None, trigger, conditions: vec![], actions, cooldown_secs: 0 }
    }

    fn log() -> Vec<Action> {
        vec![Action::Log { message: "ok".into() }]
    }

    #[test]
    fn json_shape_is_backward_compatible() {
        let t: Trigger = serde_json::from_value(json!({ "type": "interval", "minutes": 15 })).unwrap();
        assert_eq!(t, Trigger::Interval { minutes: 15 });
        let a: Action = serde_json::from_value(json!({ "type": "show_window" })).unwrap();
        assert_eq!(a, Action::ShowWindow);
        let n: Action = serde_json::from_value(json!({ "type": "notify", "title": "x" })).unwrap();
        assert_eq!(n, Action::Notify { title: "x".into(), body: String::new() });
        let c: Condition = serde_json::from_value(json!({ "type": "internet" })).unwrap();
        assert_eq!(c, Condition::Internet { online: true });
        assert!(serde_json::from_value::<Action>(json!({ "type": "format_disk" })).is_err());
    }

    /// Security: names that could break out of a command line are refused at save time.
    #[test]
    fn process_names_cannot_inject() {
        let close = |p: &str| input(Trigger::AppStart, vec![Action::CloseApp { process: p.into(), force: false }]).validate();
        assert!(close("obs64.exe").is_ok());
        assert!(close("Code - Insiders.exe").is_ok());
        for bad in ["x.exe & calc", "a|b", "a\"b", "%COMSPEC%", "a>b", "a^b", "..\\x.exe", "x;y", "a`b", "$(x)"] {
            assert!(close(bad).is_err(), "{bad} must be rejected");
        }
        let started = input(Trigger::ProcessStarted { process: "a&b".into() }, log()).validate();
        assert!(started.is_err());
    }

    #[test]
    fn validation() {
        assert!(input(Trigger::AppStart, log()).validate().is_ok());
        assert!(input(Trigger::Interval { minutes: 0 }, log()).validate().is_err());
        assert!(input(Trigger::Hotkey { shortcut: "Ctrl+Nope".into() }, log()).validate().is_err());
        assert!(input(Trigger::Time { time: "25:00".into(), days: vec![] }, log()).validate().is_err());
        assert!(input(Trigger::Time { time: "09:00".into(), days: vec![1, 8] }, log()).validate().is_err());
        assert!(input(Trigger::ProcessStarted { process: r"C:\obs.exe".into() }, log()).validate().is_err());
        assert!(input(Trigger::WindowFocus { process: String::new(), title_contains: String::new() }, log()).validate().is_err());
        assert!(input(Trigger::AppStart, vec![]).validate().is_err());

        let act = |a: Action| input(Trigger::Manual, vec![a]).validate();
        assert!(act(Action::OpenApp { path: "%LOCALAPPDATA%\\Programs\\app.exe".into(), args: String::new() }).is_ok());
        assert!(act(Action::OpenApp { path: "C:\\run.bat".into(), args: String::new() }).is_err());
        assert!(act(Action::SetModule { module_id: "security".into(), enabled: false }).is_err(), "critical");
        assert!(act(Action::SetModule { module_id: "automations".into(), enabled: false }).is_err());
        assert!(act(Action::SetModule { module_id: "tray".into(), enabled: false }).is_ok());
        assert!(act(Action::DeleteFiles { folder: "C:\\Temp".into(), pattern: "..\\*".into(), older_than_days: 0 }).is_err());
        assert!(act(Action::SwitchLayout { layout_id: String::new(), profile: "party".into() }).is_err());
    }

    #[test]
    fn dangers_and_permissions() {
        let actions = vec![
            Action::RunCommand { command: "dir".into() },
            Action::OpenFile { path: "C:\\x\\setup.BAT".into() },
            Action::OpenFile { path: "C:\\x\\doc.pdf".into() },
            Action::CloseApp { process: "obs64.exe".into(), force: false },
            Action::Notify { title: "a".into(), body: String::new() },
        ];
        let (perms, dangers) = requirements(&actions);
        assert_eq!(dangers, ["команда терминала", "запуск скрипта или программы", "закрытие программы"]);
        assert_eq!(perms, [PERM_RUN_COMMANDS, PERM_LAUNCH_APPS, PERM_CLOSE_APPS, PERM_NOTIFICATIONS]);
    }
}
