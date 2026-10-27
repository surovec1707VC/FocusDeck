//! Widget kinds. Every kind belongs to its own module in the Module Registry:
//! the name, description, settings and permissions come from there, so a
//! widget is turned off, configured and checked in the Control Center.

use std::collections::HashSet;

use serde::Serialize;
use serde_json::Value;

use crate::modules::catalog::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetCategory {
    Time,
    Productivity,
    System,
    Information,
}

/// Where a widget's data comes from (shown in the catalog).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    /// Computed on this computer (time, date).
    Local,
    /// The local SQLite database.
    Database,
    /// The widget's own saved configuration.
    Config,
    /// Windows API.
    System,
    /// The Windows clipboard (through the clipboard history module).
    Clipboard,
    /// The internet; cached for offline use.
    Network,
}

pub struct WidgetDef {
    pub kind: &'static str,
    /// Default title of a new instance.
    pub title: &'static str,
    /// Module that owns the widget.
    pub module: &'static str,
    /// Other modules the widget needs (e.g. the notes storage).
    pub requires: &'static [&'static str],
    pub category: WidgetCategory,
    pub data_source: DataSource,
    /// Grid cells (width, height); the grid has 12 columns by default.
    pub default_size: (i64, i64),
    pub min_size: (i64, i64),
    /// How often the UI refreshes the data; 0 = only after actions.
    /// Modules may override it with a `refresh_secs` / `refresh_minutes` setting.
    pub refresh_secs: u32,
}

const fn def(
    kind: &'static str,
    title: &'static str,
    module: &'static str,
    requires: &'static [&'static str],
    category: WidgetCategory,
    data_source: DataSource,
    default_size: (i64, i64),
    min_size: (i64, i64),
    refresh_secs: u32,
) -> WidgetDef {
    WidgetDef { kind, title, module, requires, category, data_source, default_size, min_size, refresh_secs }
}

use DataSource as D;
use WidgetCategory as C;

pub const WIDGETS: &[WidgetDef] = &[
    def("clock", "Часы", WIDGET_CLOCK, &[], C::Time, D::Local, (3, 2), (2, 2), 60),
    def("date", "Дата", WIDGET_DATE, &[], C::Time, D::Local, (3, 2), (2, 2), 300),
    def("calendar", "Календарь", WIDGET_CALENDAR, &[], C::Time, D::Database, (4, 5), (3, 4), 300),
    def("countdown", "Таймер", WIDGET_COUNTDOWN, &[], C::Time, D::Config, (3, 3), (2, 2), 0),
    def("stopwatch", "Секундомер", WIDGET_STOPWATCH, &[], C::Time, D::Config, (3, 3), (2, 2), 0),
    def("notes", "Заметки", WIDGET_NOTES, &[NOTES], C::Productivity, D::Database, (4, 4), (2, 2), 0),
    def("todo", "Список дел", WIDGET_TODO, &[TASKS], C::Productivity, D::Database, (4, 4), (3, 3), 60),
    def("quick_links", "Быстрые ссылки", WIDGET_QUICK_LINKS, &[], C::Productivity, D::Config, (3, 3), (2, 2), 0),
    def("app_launcher", "Запуск программ", WIDGET_APP_LAUNCHER, &[], C::Productivity, D::Config, (3, 3), (2, 2), 0),
    def("system_info", "Система", WIDGET_SYSTEM_INFO, &[], C::System, D::System, (4, 4), (3, 3), 3),
    def("clipboard", "Буфер обмена", WIDGET_CLIPBOARD, &[CLIPBOARD], C::System, D::Clipboard, (4, 5), (3, 3), 5),
    def("notifications", "Уведомления", WIDGET_NOTIFICATIONS, &[NOTIFICATIONS], C::Information, D::Database, (4, 4), (3, 3), 30),
    def("weather", "Погода", WIDGET_WEATHER, &[], C::Information, D::Network, (4, 3), (3, 3), 1800),
    def("status", "Состояние", WIDGET_STATUS, &[], C::Information, D::Database, (3, 3), (2, 2), 30),
    def("git", "Git", DEV_GIT, &[], C::Productivity, D::System, (4, 4), (3, 3), 30),
    def("media", "Музыка", WIDGET_MEDIA, &[], C::Information, D::System, (4, 3), (3, 2), 0),
];

pub fn find(kind: &str) -> Option<&'static WidgetDef> {
    WIDGETS.iter().find(|w| w.kind == kind)
}

impl WidgetDef {
    /// Modules that must be on for the widget to show: Widgets, its own, required ones.
    pub fn modules(&self) -> impl Iterator<Item = &'static str> + use<> {
        [WIDGETS_MODULE, self.module].into_iter().chain(self.requires.iter().copied())
    }

    /// Ids of required modules that are off.
    pub fn missing_modules(&self, enabled: &HashSet<String>) -> Vec<&'static str> {
        self.modules().filter(|m| !enabled.contains(*m)).collect()
    }

    /// Refresh period from the module settings, falling back to the default.
    pub fn refresh_secs(&self, settings: &Value) -> u32 {
        if let Some(s) = settings.get("refresh_secs").and_then(Value::as_u64) {
            return s as u32;
        }
        if let Some(m) = settings.get("refresh_minutes").and_then(Value::as_u64) {
            return (m * 60) as u32;
        }
        self.refresh_secs
    }
}

/// The «Виджеты» module (named to avoid a clash with the `WIDGETS` list).
const WIDGETS_MODULE: &str = crate::modules::catalog::WIDGETS;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_are_unique_and_sizes_valid() {
        let kinds: HashSet<_> = WIDGETS.iter().map(|w| w.kind).collect();
        assert_eq!(kinds.len(), WIDGETS.len());
        assert_eq!(WIDGETS.len(), 16);
        for w in WIDGETS {
            assert!(w.min_size.0 <= w.default_size.0 && w.min_size.1 <= w.default_size.1, "{}", w.kind);
            assert!(w.default_size.0 <= 12, "{}", w.kind);
        }
        let enabled: HashSet<String> = [WIDGETS_MODULE, WIDGET_CLOCK].iter().map(|s| s.to_string()).collect();
        assert!(find("clock").unwrap().missing_modules(&enabled).is_empty());
        assert_eq!(find("notes").unwrap().missing_modules(&enabled), [WIDGET_NOTES, NOTES]);
    }
}
