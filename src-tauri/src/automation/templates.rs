//! Ready-made scenarios. Installing one creates a normal automation that the
//! user can then edit. Paths use `%VARIABLES%` so they fit most computers;
//! a program that isn't installed makes that action fail with a clear error.

use serde::Serialize;
use serde_json::{Value, json};

use super::types::AutomationInput;

#[derive(Debug, Clone, Serialize)]
pub struct Template {
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// lucide icon name for the UI.
    pub icon: &'static str,
    pub input: Value,
}

fn t(key: &'static str, name: &'static str, description: &'static str, icon: &'static str, input: Value) -> Template {
    Template { key, name, description, icon, input }
}

pub fn all() -> Vec<Template> {
    vec![
        t("start_work", "Start Work", "Макет «Работа», включить уведомления и задачи, напомнить о плане на день.", "briefcase", json!({
            "name": "Start Work", "description": "Начало рабочего дня", "enabled": true,
            "trigger": { "type": "ui_button", "label": "Начать работу" },
            "conditions": [],
            "actions": [
                { "type": "switch_layout", "profile": "work" },
                { "type": "set_module", "module_id": "notifications", "enabled": true },
                { "type": "set_module", "module_id": "tasks", "enabled": true },
                { "type": "focus_mode", "enabled": false },
                { "type": "notify", "title": "Рабочий день начат", "body": "Проверьте задачи на сегодня." }
            ],
            "cooldown_secs": 60
        })),
        t("start_coding", "Start Coding", "Макет «Программирование», VS Code и таймер на 50 минут.", "code-2", json!({
            "name": "Start Coding", "description": "Сессия программирования", "enabled": true,
            "trigger": { "type": "ui_button", "label": "Код" },
            "conditions": [],
            "actions": [
                { "type": "switch_layout", "profile": "coding" },
                { "type": "open_app", "path": "%LOCALAPPDATA%\\Programs\\Microsoft VS Code\\Code.exe" },
                { "type": "start_timer", "name": "Код", "kind": "countdown", "minutes": 50 }
            ],
            "cooldown_secs": 60
        })),
        t("start_streaming", "Start Streaming", "Макет «Стрим», OBS и Focus Mode, чтобы уведомления не попали в эфир.", "radio", json!({
            "name": "Start Streaming", "description": "Подготовка к стриму", "enabled": true,
            "trigger": { "type": "ui_button", "label": "Стрим" },
            "conditions": [ { "type": "internet", "online": true } ],
            "actions": [
                { "type": "switch_layout", "profile": "streaming" },
                { "type": "open_app", "path": "%ProgramFiles%\\obs-studio\\bin\\64bit\\obs64.exe" },
                { "type": "focus_mode", "enabled": true, "minutes": 0 },
                { "type": "notify", "title": "Стрим", "body": "Focus Mode включён: всплывающие уведомления скрыты." }
            ],
            "cooldown_secs": 60
        })),
        t("start_gaming", "Start Gaming", "Когда запускается Steam: макет «Игры» и Focus Mode.", "gamepad-2", json!({
            "name": "Start Gaming", "description": "Игровой режим при запуске Steam", "enabled": false,
            "trigger": { "type": "process_started", "process": "steam.exe" },
            "conditions": [],
            "actions": [
                { "type": "switch_layout", "profile": "gaming" },
                { "type": "focus_mode", "enabled": true, "minutes": 0 }
            ],
            "cooldown_secs": 300
        })),
        t("focus_session", "Focus Session", "Pomodoro 25 минут, макет «Фокус» и Focus Mode на это время.", "crosshair", json!({
            "name": "Focus Session", "description": "25 минут без отвлечений", "enabled": true,
            "trigger": { "type": "ui_button", "label": "Фокус 25" },
            "conditions": [],
            "actions": [
                { "type": "switch_layout", "profile": "focus" },
                { "type": "focus_mode", "enabled": true, "minutes": 25 },
                { "type": "start_timer", "name": "Фокус", "kind": "pomodoro", "minutes": 25 }
            ],
            "cooldown_secs": 0
        })),
        t("end_work", "End Work Session", "Выключить Focus Mode, снимок экрана на память, уведомление и скрыть окно.", "log-out", json!({
            "name": "End Work Session", "description": "Завершение рабочего дня", "enabled": true,
            "trigger": { "type": "ui_button", "label": "Конец дня" },
            "conditions": [],
            "actions": [
                { "type": "focus_mode", "enabled": false },
                { "type": "screenshot" },
                { "type": "notify", "title": "Рабочий день завершён", "body": "Хорошего вечера!" },
                { "type": "hide_window" }
            ],
            "cooldown_secs": 60
        })),
        t("morning_setup", "Morning Setup", "По будням в 09:00: макет «Работа», календарь в браузере, напоминание.", "sunrise", json!({
            "name": "Morning Setup", "description": "Утренняя подготовка по будням", "enabled": false,
            "trigger": { "type": "time", "time": "09:00", "days": [1, 2, 3, 4, 5] },
            "conditions": [ { "type": "internet", "online": true } ],
            "actions": [
                { "type": "switch_layout", "profile": "work" },
                { "type": "open_url", "url": "https://calendar.google.com" },
                { "type": "notify", "title": "Доброе утро", "body": "Календарь открыт, макет «Работа» включён." }
            ],
            "cooldown_secs": 3600
        })),
        t("cleanup_desktop", "Cleanup Desktop", "Переместить в Корзину временные файлы (*.tmp) с рабочего стола старше 7 дней. Требует подтверждения.", "trash-2", json!({
            "name": "Cleanup Desktop", "description": "Временные файлы с рабочего стола в Корзину", "enabled": true,
            "trigger": { "type": "manual" },
            "conditions": [],
            "actions": [
                { "type": "delete_files", "folder": "%USERPROFILE%\\Desktop", "pattern": "*.tmp", "older_than_days": 7 },
                { "type": "notify", "title": "Рабочий стол очищен", "body": "Файлы в Корзине — их можно восстановить." }
            ],
            "cooldown_secs": 0
        })),
    ]
}

pub fn find(key: &str) -> Option<(Template, AutomationInput)> {
    let t = all().into_iter().find(|t| t.key == key)?;
    let input = serde_json::from_value(t.input.clone()).ok()?;
    Some((t, input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_templates_are_valid() {
        let list = all();
        assert_eq!(list.len(), 8);
        for t in &list {
            let (_, input) = find(t.key).unwrap_or_else(|| panic!("{} does not parse", t.key));
            input.validate().unwrap_or_else(|e| panic!("{}: {e}", t.key));
        }
        let (_, cleanup) = find("cleanup_desktop").unwrap();
        assert!(!super::super::types::requirements(&cleanup.actions).1.is_empty(), "cleanup is dangerous");
    }
}
