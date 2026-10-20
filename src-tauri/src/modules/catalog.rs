//! Every FocusDeck feature as a module.

use serde_json::{Value, json};

use super::types::{Category, ModuleDef};
use crate::permissions::{
    PERM_AUTOSTART, PERM_CLIPBOARD_READ, PERM_FS_READ, PERM_GLOBAL_HOTKEYS, PERM_LAUNCH_APPS,
    PERM_NETWORK_WEATHER, PERM_NOTIFICATIONS, PERM_PASTE,
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
pub const TIMERS: &str = "timers";
pub const LOGGING: &str = "logging";
pub const CLIPBOARD: &str = "clipboard_history";
pub const FILES: &str = "files";
pub const HOTKEYS: &str = "hotkeys";
pub const TRAY: &str = "tray";
pub const AUTOSTART: &str = "autostart";
pub const NOTIFICATIONS: &str = "notifications";
pub const INTEGRATIONS: &str = "integrations";

// Widget modules: one per widget kind (see `crate::widgets::catalog`).
pub const WIDGET_DATE: &str = "widget_date";
pub const WIDGET_WEATHER: &str = "widget_weather";
pub const WIDGET_NOTES: &str = "widget_notes";
pub const WIDGET_TODO: &str = "widget_todo";
pub const WIDGET_QUICK_LINKS: &str = "widget_quick_links";
pub const WIDGET_APP_LAUNCHER: &str = "widget_app_launcher";
pub const WIDGET_SYSTEM_INFO: &str = "widget_system_info";
pub const WIDGET_CLIPBOARD: &str = "widget_clipboard";
pub const WIDGET_NOTIFICATIONS: &str = "widget_notifications";
pub const WIDGET_CALENDAR: &str = "widget_calendar";
pub const WIDGET_COUNTDOWN: &str = "widget_countdown";
pub const WIDGET_STOPWATCH: &str = "widget_stopwatch";

/// Features that exist only as storage/placeholder so far.
pub const IN_DEVELOPMENT: &[&str] = &[FILES, INTEGRATIONS];

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
        description: "Главная страница: сетка виджетов активного макета.",
        details: "Виджеты можно перетаскивать, менять их размер, закреплять, сворачивать и скрывать. \
                  Позиции сохраняются отдельно для каждого макета.",
        category: Category::Layouts,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "columns": { "type": "integer", "title": "Колонок в сетке", "default": 12, "minimum": 4, "maximum": 24 },
            "row_height": { "type": "integer", "title": "Высота строки (px)", "default": 72, "minimum": 40, "maximum": 160 },
            "compact": { "type": "boolean", "title": "Подтягивать виджеты вверх", "default": true }
        }}),
    },
    ModuleDef {
        id: WIDGETS,
        name: "Виджеты",
        description: "Каталог виджетов и сетка виджетов на обзоре.",
        details: "Если отключить, страница «Виджеты» и все виджеты на обзоре скрываются. Сами \
                  виджеты, их позиции и содержимое сохраняются.",
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
            "hour12": { "type": "boolean", "title": "12-часовой формат", "default": false },
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
        description: "Задачи с приоритетами, сроками, статусами, категориями и повторами.",
        details: "Страница «Задачи», быстрое добавление и статистика. Если отключить, страница и \
                  виджеты «Список дел» скрываются, повторяющиеся задачи не создаются. Задачи сохраняются.",
        category: Category::Productivity,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "hide_completed": { "type": "boolean", "title": "Скрывать выполненные", "default": false },
            "default_priority": { "type": "integer", "title": "Приоритет по умолчанию (0–3)", "default": 0, "minimum": 0, "maximum": 3 },
            "week_starts": { "type": "string", "title": "Первый день недели", "default": "monday", "enum": ["monday", "sunday"] }
        }}),
    },
    ModuleDef {
        id: NOTES,
        name: "Заметки",
        description: "Заметки в Markdown: теги, поиск, закрепление, корзина, экспорт.",
        details: "Страница «Заметки». Заметки можно привязать к проекту или макету. Удалённые \
                  попадают в корзину и очищаются через заданное число дней. Если отключить, страница \
                  и виджеты «Заметки» скрываются; заметки сохраняются.",
        category: Category::Productivity,
        version: "1.1.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "trash_days": { "type": "integer", "title": "Хранить в корзине (дней)", "default": 30, "minimum": 1, "maximum": 365 },
            "preview": { "type": "boolean", "title": "Показывать предпросмотр Markdown", "default": true }
        }}),
    },
    ModuleDef {
        id: SNIPPETS,
        name: "Сниппеты",
        description: "Шаблоны текста с переменными: копирование и вставка.",
        details: "Страница «Сниппеты»: теги, поиск, переменные {{имя}} и встроенные {{date}}, {{time}}, \
                  {{datetime}}, импорт и экспорт JSON. «Вставить» копирует текст, прячет окно и нажимает \
                  Ctrl+V в программе, где вы работали, — для этого нужно разрешение «Вставка текста». \
                  Если отключить, страница скрывается; сниппеты сохраняются.",
        category: Category::Productivity,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_PASTE],
        settings_schema: || json!({ "type": "object", "properties": {
            "paste_delay_ms": { "type": "integer", "title": "Задержка перед вставкой (мс)", "default": 250, "minimum": 50, "maximum": 2000 }
        }}),
    },
    ModuleDef {
        id: TIMERS,
        name: "Таймеры",
        description: "Таймеры, Pomodoro и секундомеры с историей.",
        details: "Страница «Таймеры». Фоновая задача раз в секунду завершает истёкшие таймеры, пишет \
                  сессию в историю и показывает уведомление, даже если окно скрыто. Если отключить, \
                  фоновая задача останавливается и страница скрывается; таймеры и история сохраняются.",
        category: Category::Productivity,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "notify": { "type": "boolean", "title": "Уведомлять по завершении", "default": true },
            "auto_start_next": { "type": "boolean", "title": "Pomodoro: сразу начинать следующий интервал", "default": false },
            "history_days": { "type": "integer", "title": "Хранить историю (дней)", "default": 90, "minimum": 7, "maximum": 3650 }
        }}),
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
        details: "Фоновая задача раз в секунду проверяет, изменился ли буфер, и сохраняет текст в \
                  локальную базу. Нужно разрешение «Чтение буфера обмена». Содержимое, которое \
                  менеджеры паролей помечают как скрытое, не сохраняется. Если отключить, запись \
                  прекращается, история сохраняется.",
        category: Category::Clipboard,
        version: "1.0.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_CLIPBOARD_READ],
        settings_schema: || json!({ "type": "object", "properties": {
            "max_items": { "type": "integer", "title": "Хранить записей", "default": 200, "minimum": 10, "maximum": 5000 },
            "max_length": { "type": "integer", "title": "Максимальная длина записи (символов)", "default": 20000, "minimum": 100, "maximum": 100000 },
            "skip_sensitive": { "type": "boolean", "title": "Не сохранять скрытое содержимое (пароли)", "default": true }
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
    // --- widgets (one module per widget kind) ---
    ModuleDef {
        id: WIDGET_DATE,
        name: "Виджет «Дата»",
        description: "Сегодняшняя дата, день недели и номер недели.",
        details: "Если отключить, виджет скрывается во всех макетах; его данные сохраняются.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "show_weekday": { "type": "boolean", "title": "Показывать день недели", "default": true },
            "show_week_number": { "type": "boolean", "title": "Показывать номер недели", "default": true }
        }}),
    },
    ModuleDef {
        id: WIDGET_WEATHER,
        name: "Виджет «Погода»",
        description: "Текущая погода и прогноз на 3 дня.",
        details: "Погода запрашивается у open-meteo.com только при выданном разрешении «Сеть: прогноз погоды» и кэшируется: без сети показывается последний полученный прогноз. Если отключить, виджет скрывается во всех макетах; его данные сохраняются.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_NETWORK_WEATHER],
        settings_schema: || json!({ "type": "object", "properties": {
            "city": { "type": "string", "title": "Город", "default": "Москва" },
            "units": { "type": "string", "title": "Единицы", "default": "celsius", "enum": ["celsius", "fahrenheit"] },
            "refresh_minutes": { "type": "integer", "title": "Обновлять раз в (мин)", "default": 30, "minimum": 10, "maximum": 360 }
        }}),
    },
    ModuleDef {
        id: WIDGET_NOTES,
        name: "Виджет «Заметки»",
        description: "Текстовая заметка с автосохранением.",
        details: "Требует модуль «Заметки». Если отключить, виджет скрывается во всех макетах; его данные сохраняются.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "monospace": { "type": "boolean", "title": "Моноширинный шрифт", "default": false },
            "autosave_ms": { "type": "integer", "title": "Автосохранение через (мс)", "default": 800, "minimum": 300, "maximum": 5000 }
        }}),
    },
    ModuleDef {
        id: WIDGET_TODO,
        name: "Виджет «Список дел»",
        description: "Задачи с отметкой о выполнении.",
        details: "Требует модуль «Задачи». Если отключить, виджет скрывается во всех макетах; его данные сохраняются.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "show_counter": { "type": "boolean", "title": "Показывать счётчик", "default": true }
        }}),
    },
    ModuleDef {
        id: WIDGET_QUICK_LINKS,
        name: "Виджет «Быстрые ссылки»",
        description: "Ссылки на сайты, открываются в браузере.",
        details: "Разрешены только адреса http, https и mailto. Если отключить, виджет скрывается во всех макетах; его данные сохраняются.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "show_urls": { "type": "boolean", "title": "Показывать адреса", "default": false }
        }}),
    },
    ModuleDef {
        id: WIDGET_APP_LAUNCHER,
        name: "Виджет «Запуск программ»",
        description: "Кнопки запуска выбранных программ и ярлыков.",
        details: "Нужно разрешение «Запуск программ». Запускаются только файлы, которые вы добавили сами (.exe, .lnk, .url). Если отключить, виджет скрывается во всех макетах; его данные сохраняются.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_LAUNCH_APPS],
        settings_schema: || json!({ "type": "object", "properties": {
            "confirm_launch": { "type": "boolean", "title": "Спрашивать перед запуском", "default": false }
        }}),
    },
    ModuleDef {
        id: WIDGET_SYSTEM_INFO,
        name: "Виджет «Система»",
        description: "Загрузка процессора, память, диски и батарея.",
        details: "Данные читаются через Windows API в фоновом потоке. Если отключить, виджет скрывается во всех макетах; его данные сохраняются.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "refresh_secs": { "type": "integer", "title": "Обновлять раз в (с)", "default": 3, "minimum": 1, "maximum": 60 },
            "show_cpu": { "type": "boolean", "title": "Процессор", "default": true },
            "show_ram": { "type": "boolean", "title": "Память", "default": true },
            "show_disk": { "type": "boolean", "title": "Диски", "default": true },
            "show_battery": { "type": "boolean", "title": "Батарея", "default": true }
        }}),
    },
    ModuleDef {
        id: WIDGET_CLIPBOARD,
        name: "Виджет «Буфер обмена»",
        description: "Последние скопированные тексты: скопировать снова, закрепить, удалить.",
        details: "Показывает историю модуля «История буфера обмена». Если отключить, виджет скрывается во всех макетах; его данные сохраняются.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "max_visible": { "type": "integer", "title": "Показывать записей", "default": 20, "minimum": 5, "maximum": 200 }
        }}),
    },
    ModuleDef {
        id: WIDGET_NOTIFICATIONS,
        name: "Виджет «Уведомления»",
        description: "История уведомлений FocusDeck.",
        details: "Показывает уведомления модуля «Уведомления». Если отключить, виджет скрывается во всех макетах; его данные сохраняются.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "max_visible": { "type": "integer", "title": "Показывать записей", "default": 20, "minimum": 5, "maximum": 200 },
            "unread_only": { "type": "boolean", "title": "Только непрочитанные", "default": false }
        }}),
    },
    ModuleDef {
        id: WIDGET_CALENDAR,
        name: "Виджет «Календарь»",
        description: "Месяц с локальными событиями.",
        details: "Если отключить, виджет скрывается во всех макетах; его данные сохраняются.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "week_starts": { "type": "string", "title": "Первый день недели", "default": "monday", "enum": ["monday", "sunday"] },
            "show_events": { "type": "boolean", "title": "Показывать события", "default": true }
        }}),
    },
    ModuleDef {
        id: WIDGET_COUNTDOWN,
        name: "Виджет «Таймер»",
        description: "Обратный отсчёт с уведомлением по окончании.",
        details: "Состояние хранится в базе: таймер идёт и после перезапуска. Если отключить, виджет скрывается во всех макетах; его данные сохраняются.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "notify_on_finish": { "type": "boolean", "title": "Уведомлять по окончании", "default": true }
        }}),
    },
    ModuleDef {
        id: WIDGET_STOPWATCH,
        name: "Виджет «Секундомер»",
        description: "Секундомер с кругами.",
        details: "Состояние хранится в базе: секундомер идёт и после перезапуска. Если отключить, виджет скрывается во всех макетах; его данные сохраняются.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "show_ms": { "type": "boolean", "title": "Показывать сотые", "default": true }
        }}),
    },
];

pub fn find(id: &str) -> Option<&'static ModuleDef> {
    BUILTIN.iter().find(|m| m.id == id)
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
        for w in crate::widgets::catalog::WIDGETS {
            assert!(ids.contains(w.module), "{}", w.kind);
            assert!(w.requires.iter().all(|r| ids.contains(r)), "{}", w.kind);
        }
    }
}
