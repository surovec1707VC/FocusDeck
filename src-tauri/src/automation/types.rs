use std::str::FromStr;

use serde::{Deserialize, Serialize};
use tauri_plugin_global_shortcut::Shortcut;

use crate::error::{AppError, AppResult};

pub const MAX_INTERVAL_MINUTES: u32 = 24 * 60;

/// What starts an automation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    /// Once, when FocusDeck starts.
    AppStart,
    /// Every N minutes while FocusDeck runs.
    Interval { minutes: u32 },
    /// A global shortcut (needs the hotkeys permission and module).
    Hotkey { shortcut: String },
}

/// What an automation does. Only local, in-app actions: no shell commands,
/// no network, no input simulation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Toast inside the FocusDeck window.
    Notify {
        title: String,
        body: String,
    },
    /// A line in the application log.
    Log {
        message: String,
    },
    ShowWindow,
    HideWindow,
}

#[derive(Debug, Clone, Serialize)]
pub struct Automation {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    /// Owning module; the automation doesn't run while it is off.
    pub module_id: String,
    pub trigger: Trigger,
    pub actions: Vec<Action>,
    pub last_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AutomationInput {
    pub name: String,
    pub enabled: bool,
    /// Owning module (default: `automations`).
    #[serde(default)]
    pub module_id: Option<String>,
    pub trigger: Trigger,
    pub actions: Vec<Action>,
}

fn len_between(value: &str, min: usize, max: usize, what: &str) -> AppResult<()> {
    let n = value.trim().chars().count();
    if n < min || n > max {
        return Err(AppError::Invalid(format!(
            "{what}: от {min} до {max} символов"
        )));
    }
    Ok(())
}

impl AutomationInput {
    pub fn module_id(&self) -> &str {
        self.module_id.as_deref().unwrap_or(crate::modules::catalog::AUTOMATIONS)
    }

    pub fn validate(&self) -> AppResult<()> {
        len_between(&self.name, 1, 60, "название")?;
        if crate::modules::catalog::find(self.module_id()).is_none() {
            return Err(AppError::Invalid(format!("неизвестный модуль «{}»", self.module_id())));
        }
        match &self.trigger {
            Trigger::AppStart => {}
            Trigger::Interval { minutes } => {
                if *minutes == 0 || *minutes > MAX_INTERVAL_MINUTES {
                    return Err(AppError::Invalid(format!(
                        "интервал: от 1 до {MAX_INTERVAL_MINUTES} минут"
                    )));
                }
            }
            Trigger::Hotkey { shortcut } => {
                Shortcut::from_str(shortcut.trim()).map_err(|e| {
                    AppError::Invalid(format!("сочетание «{shortcut}» не распознано: {e}"))
                })?;
            }
        }
        if self.actions.is_empty() || self.actions.len() > 10 {
            return Err(AppError::Invalid("нужно от 1 до 10 действий".into()));
        }
        for a in &self.actions {
            match a {
                Action::Notify { title, body } => {
                    len_between(title, 1, 80, "заголовок уведомления")?;
                    len_between(body, 0, 500, "текст уведомления")?;
                }
                Action::Log { message } => len_between(message, 1, 500, "сообщение для журнала")?,
                Action::ShowWindow | Action::HideWindow => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn input(trigger: Trigger, actions: Vec<Action>) -> AutomationInput {
        AutomationInput {
            name: "Тест".into(),
            enabled: true,
            module_id: None,
            trigger,
            actions,
        }
    }

    #[test]
    fn json_shape() {
        let t: Trigger =
            serde_json::from_value(json!({ "type": "interval", "minutes": 15 })).unwrap();
        assert_eq!(t, Trigger::Interval { minutes: 15 });
        let a: Action = serde_json::from_value(json!({ "type": "show_window" })).unwrap();
        assert_eq!(a, Action::ShowWindow);
        assert!(
            serde_json::from_value::<Action>(json!({ "type": "run_shell", "cmd": "x" })).is_err()
        );
    }

    #[test]
    fn validation() {
        let log = || {
            vec![Action::Log {
                message: "ok".into(),
            }]
        };
        assert!(input(Trigger::AppStart, log()).validate().is_ok());
        assert!(
            input(Trigger::Interval { minutes: 0 }, log())
                .validate()
                .is_err()
        );
        assert!(
            input(
                Trigger::Hotkey {
                    shortcut: "Ctrl+Alt+K".into()
                },
                log()
            )
            .validate()
            .is_ok()
        );
        assert!(
            input(
                Trigger::Hotkey {
                    shortcut: "Ctrl+Nope".into()
                },
                log()
            )
            .validate()
            .is_err()
        );
        assert!(input(Trigger::AppStart, vec![]).validate().is_err());
        assert!(
            input(
                Trigger::AppStart,
                vec![Action::Notify {
                    title: " ".into(),
                    body: "".into()
                }]
            )
            .validate()
            .is_err()
        );
    }
}
