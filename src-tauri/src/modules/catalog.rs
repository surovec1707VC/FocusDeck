//! Every FocusDeck feature as a module.

use serde_json::{Value, json};

use super::types::{Category, ModuleDef};
use crate::permissions::{
    PERM_AUTOSTART, PERM_CLIPBOARD_READ, PERM_FS_READ, PERM_GLOBAL_HOTKEYS, PERM_NOTIFICATIONS,
};

// Critical modules (confirmation required to turn off).
pub const MODULE_REGISTRY: &str = "module_registry";
pub const SETTINGS: &str = "settings";
pub const DATABASE: &str = "database";
pub const SECURITY: &str = "security";

pub const DASHBOARD: &str = "dashboard";
pub const WIDGETS: &str = "widgets";
pub const WIDGET_CLOCK: &str = "widget_clock";
pub const WIDGET_STATUS: &str = "widget_status";
pub const LAYOUTS: &str = "layouts";
pub const AUTOMATIONS: &str = "automations";
pub const TASKS: &str = "tasks";
pub const NOTES: &str = "notes";
pub const SNIPPETS: &str = "snippets";
pub const LOGGING: &str = "logging";
pub const CLIPBOARD: &str = "clipboard_history";
pub const FILES: &str = "files";
pub const HOTKEYS: &str = "hotkeys";
pub const TRAY: &str = "tray";
pub const AUTOSTART: &str = "autostart";
pub const NOTIFICATIONS: &str = "notifications";
pub const INTEGRATIONS: &str = "integrations";

/// Features that exist only as storage/placeholder so far.
pub const IN_DEVELOPMENT: &[&str] = &[SNIPPETS, CLIPBOARD, FILES, INTEGRATIONS];

fn no_settings() -> Value {
    json!({ "type": "object", "properties": {} })
}

pub const BUILTIN: &[ModuleDef] = &[
    // --- critical ---
    ModuleDef {
        id: MODULE_REGISTRY,
        name: "Реестр модулей",
        description: "Включение, отключение и настройка модулей.",
        details: "Хранит состояние и настройки всех модулей и ведёт журнал действий с ними. \
                  Если отключить, редактирование настроек модулей и страница «Модули» станут недоступны. \
                  Включать и отключать модули в Центре управления можно всегда — так модуль легко вернуть.",
        category: Category::DeveloperTools,
        version: "1.0.0",
        enabled_by_default: true,
        critical: true,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "log_retention": { "type": "integer", "title": "Записей журнала на модуль", "default": 1000, "minimum": 100, "maximum": 10000 }
        }}),
    },
    ModuleDef {
        id: SETTINGS,
        name: "Настройки",
        description: "Тема, окно, автозапуск и горячие клавиши приложения.",
        details: "Страница «Настройки». Если модуль отключён, текущие настройки продолжают действовать, \
                  но изменить их нельзя.",
        category: Category::Appearance,
        version: "1.0.0",
        enabled_by_default: true,
        critical: true,
        requires_restart: false,
        permissions: &[],
        settings_schema: no_settings,
    },
    ModuleDef {
        id: DATABASE,
        name: "База данных",
        description: "Обслуживание локальной базы SQLite, экспорт и импорт конфигурации.",
        details: "Сама база работает всегда. Модуль отвечает за обслуживание и за экспорт/импорт \
                  конфигурации в JSON. Если отключить — экспорт и импорт недоступны.",
        category: Category::Performance,
        version: "1.0.0",
        enabled_by_default: true,
        critical: true,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "checkpoint_on_exit": { "type": "boolean", "title": "Сжимать журнал WAL при выходе", "default": true }
        }}),
    },
    ModuleDef {
        id: SECURITY,
        name: "Безопасность",
        description: "Разрешения функций и журнал их изменений.",
        details: "Страница «Разрешения». Если модуль отключён, выданные разрешения продолжают \
                  проверяться, но изменить их нельзя.",
        category: Category::PrivacySecurity,
        version: "1.0.0",
        enabled_by_default: true,
        critical: true,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "log_denials": { "type": "boolean", "title": "Записывать отказы в журнал разрешений", "default": true }
        }}),
    },
    // --- workspace ---
    ModuleDef {
        id: DASHBOARD,
        name: "Обзор",
        description: "Главная страница со сводкой состояния.",
        details: "Карточки с количеством модулей, автоматизаций, задач и последние запуски автоматизаций.",
        category: Category::Layouts,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "show_recent_runs": { "type": "boolean", "title": "Показывать последние запуски", "default": true }
        }}),
    },
    ModuleDef {
        id: WIDGETS,
        name: "Виджеты",
        description: "Страница виджетов активного макета.",
        details: "Если отключить, страница «Виджеты» и все виджеты скрываются. Сами виджеты и их \
                  содержимое сохраняются.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "show_titles": { "type": "boolean", "title": "Показывать заголовки виджетов", "default": true }
        }}),
    },
    ModuleDef {
        id: WIDGET_CLOCK,
        name: "Виджет «Часы»",
        description: "Текущее время и дата.",
        details: "Если отключить, виджеты «Часы» скрываются во всех макетах.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "show_seconds": { "type": "boolean", "title": "Показывать секунды", "default": true },
            "show_date": { "type": "boolean", "title": "Показывать дату", "default": true }
        }}),
    },
    ModuleDef {
        id: WIDGET_STATUS,
        name: "Виджет «Состояние»",
        description: "Краткая сводка: модули, автоматизации, горячие клавиши.",
        details: "Если отключить, виджеты «Состояние» скрываются во всех макетах.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: no_settings,
    },
    ModuleDef {
        id: LAYOUTS,
        name: "Макеты",
        description: "Наборы виджетов и переключение между ними.",
        details: "Если отключить, страница «Макеты» скрывается; активный макет остаётся прежним.",
        category: Category::Layouts,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: no_settings,
    },
    ModuleDef {
        id: AUTOMATIONS,
        name: "Автоматизации",
        description: "Правила «триггер → действие» и фоновый планировщик.",
        details: "Если отключить: планировщик останавливается, автоматизации не выполняются, их \
                  горячие клавиши снимаются. Сами правила сохраняются.",
        category: Category::Automations,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "scheduler_interval_secs": { "type": "integer", "title": "Период проверки расписания (с)", "default": 30, "minimum": 10, "maximum": 600 },
            "max_log_entries": { "type": "integer", "title": "Записей журнала на автоматизацию", "default": 500, "minimum": 50, "maximum": 5000 }
        }}),
    },
    ModuleDef {
        id: TASKS,
        name: "Задачи",
        description: "Быстрый список задач (виджет «Задачи»).",
        details: "Если отключить, виджеты «Задачи» скрываются. Задачи сохраняются.",
        category: Category::Productivity,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "hide_completed": { "type": "boolean", "title": "Скрывать выполненные", "default": false }
        }}),
    },
    ModuleDef {
        id: NOTES,
        name: "Заметки",
        description: "Текстовые заметки (виджет «Заметка»).",
        details: "Если отключить, виджеты «Заметка» скрываются. Текст сохраняется.",
        category: Category::Productivity,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: no_settings,
    },
    ModuleDef {
        id: SNIPPETS,
        name: "Сниппеты",
        description: "Шаблоны текста для быстрой вставки.",
        details: "В разработке: хранилище готово, интерфейса пока нет.",
        category: Category::Productivity,
        version: "0.1.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: no_settings,
    },
    ModuleDef {
        id: LOGGING,
        name: "Журнал приложения",
        description: "Уровень подробности и запись журнала в файл.",
        details: "Журнал пишется только локально. Изменения применяются после перезапуска. \
                  Если отключить, журнал не пишется в файл (только в консоль разработчика).",
        category: Category::DeveloperTools,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: true,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "level": { "type": "string", "title": "Уровень", "default": "info", "enum": ["error", "warn", "info", "debug", "trace"] }
        }}),
    },
    ModuleDef {
        id: CLIPBOARD,
        name: "История буфера обмена",
        description: "Локальная история скопированного текста.",
        details: "В разработке. Потребуется разрешение «Чтение буфера обмена».",
        category: Category::Clipboard,
        version: "0.1.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_CLIPBOARD_READ],
        settings_schema: || json!({ "type": "object", "properties": {
            "max_items": { "type": "integer", "title": "Хранить записей", "default": 200, "minimum": 10, "maximum": 5000 }
        }}),
    },
    ModuleDef {
        id: FILES,
        name: "Файлы",
        description: "Быстрый доступ к избранным папкам и файлам.",
        details: "В разработке. Потребуется разрешение «Чтение файлов».",
        category: Category::Files,
        version: "0.1.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_FS_READ],
        settings_schema: no_settings,
    },
    // --- Windows integration ---
    ModuleDef {
        id: HOTKEYS,
        name: "Глобальные горячие клавиши",
        description: "Сочетания клавиш, работающие во всех программах.",
        details: "Показ/скрытие окна и запуск автоматизаций по сочетанию. Если отключить, все \
                  глобальные сочетания FocusDeck снимаются.",
        category: Category::WindowsIntegration,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_GLOBAL_HOTKEYS],
        settings_schema: no_settings,
    },
    ModuleDef {
        id: TRAY,
        name: "Значок в трее",
        description: "Значок в области уведомлений и сворачивание в трей.",
        details: "Если отключить, значок скрывается, а закрытие окна завершает программу \
                  (иначе вернуть окно было бы нечем).",
        category: Category::WindowsIntegration,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: no_settings,
    },
    ModuleDef {
        id: AUTOSTART,
        name: "Автозапуск",
        description: "Запуск FocusDeck вместе с Windows.",
        details: "Включается в «Настройках». Если отключить модуль, запись автозапуска удаляется, \
                  а выбор в настройках сохраняется и применится при повторном включении.",
        category: Category::WindowsIntegration,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_AUTOSTART],
        settings_schema: no_settings,
    },
    ModuleDef {
        id: NOTIFICATIONS,
        name: "Уведомления",
        description: "Всплывающие уведомления внутри окна.",
        details: "Используются автоматизациями. Если отключить, действие «Показать уведомление» \
                  пропускается.",
        category: Category::Notifications,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_NOTIFICATIONS],
        settings_schema: || json!({ "type": "object", "properties": {
            "duration_ms": { "type": "integer", "title": "Показывать (мс)", "default": 6000, "minimum": 2000, "maximum": 30000 },
            "max_visible": { "type": "integer", "title": "Одновременно на экране", "default": 4, "minimum": 1, "maximum": 10 }
        }}),
    },
    ModuleDef {
        id: INTEGRATIONS,
        name: "Локальные интеграции",
        description: "Подключения к программам на этом компьютере.",
        details: "В разработке. Только локальные интеграции, без облачных сервисов.",
        category: Category::Integrations,
        version: "0.1.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: no_settings,
    },
];

pub fn find(id: &str) -> Option<&'static ModuleDef> {
    BUILTIN.iter().find(|m| m.id == id)
}

/// Module that owns a widget kind; the widget is hidden while it is off.
pub fn widget_owner(kind: &str) -> Option<&'static str> {
    match kind {
        "clock" => Some(WIDGET_CLOCK),
        "status" => Some(WIDGET_STATUS),
        "tasks" => Some(TASKS),
        "note" => Some(NOTES),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_is_consistent() {
        let ids: HashSet<_> = BUILTIN.iter().map(|m| m.id).collect();
        assert_eq!(ids.len(), BUILTIN.len(), "ids are unique");
        for m in BUILTIN {
            let schema = (m.settings_schema)();
            assert_eq!(schema["type"], "object", "{}", m.id);
            assert!(super::super::schema::validate(&schema, &super::super::schema::defaults(&schema)).is_ok(), "{}", m.id);
        }
        let critical: HashSet<_> = BUILTIN.iter().filter(|m| m.critical).map(|m| m.id).collect();
        assert_eq!(critical, HashSet::from([MODULE_REGISTRY, SETTINGS, DATABASE, SECURITY]));
        // All 13 categories are used.
        let cats: HashSet<_> = BUILTIN.iter().map(|m| m.category).collect();
        assert_eq!(cats.len(), 13);
        for kind in ["clock", "status", "tasks", "note"] {
            assert!(ids.contains(widget_owner(kind).unwrap()));
        }
    }
}
