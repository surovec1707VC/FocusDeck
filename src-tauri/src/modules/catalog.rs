//! Every FocusDeck feature as a module.

use serde_json::{Value, json};

use super::types::{Category, ModuleDef};
use crate::permissions::{
    PERM_AUTOSTART, PERM_CLIPBOARD_READ, PERM_FS_READ, PERM_GLOBAL_HOTKEYS, PERM_LAUNCH_APPS,
    PERM_NETWORK_WEATHER, PERM_NOTIFICATIONS, PERM_PASTE, PERM_WINDOWS, PERM_MOVE_FILES,
    PERM_NETWORK_API, PERM_NETWORK_UPDATES, PERM_RUN_COMMANDS, PERM_MEDIA,
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
pub const ORGANIZER: &str = "desktop_organizer";
pub const WINDOWS_INTEGRATION: &str = "windows_integration";
pub const LOGGING: &str = "logging";
pub const CLIPBOARD: &str = "clipboard_history";
pub const FILES: &str = "files";
pub const HOTKEYS: &str = "hotkeys";
pub const TRAY: &str = "tray";
pub const AUTOSTART: &str = "autostart";
pub const NOTIFICATIONS: &str = "notifications";
pub const INTEGRATIONS: &str = "integrations";
pub const UPDATES: &str = "updates";

// Developer Tools: one module per tool.
pub const DEV_GIT: &str = "dev_git";
pub const DEV_PROJECTS: &str = "dev_projects";
pub const DEV_TERMINAL: &str = "dev_terminal";
pub const DEV_API: &str = "dev_api";
pub const DEV_REGEX: &str = "dev_regex";
pub const DEV_JSON: &str = "dev_json";

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
pub const WIDGET_MEDIA: &str = "widget_media";
pub const DESKTOP_WIDGETS: &str = "desktop_widgets";

/// Features that exist only as storage/placeholder so far.
pub const IN_DEVELOPMENT: &[&str] = &[FILES];

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
        id: DESKTOP_WIDGETS,
        name: "Виджеты на рабочем столе",
        description: "Любой виджет можно открепить от сетки в отдельное окно на рабочем столе Windows.",
        details: "Каждый открепленный виджет — отдельное окно без рамки и фона: его можно двигать и менять размер, положение и размер запоминаются между запусками. Вернуть виджет в приложение можно кнопкой в его окне или в меню «Скрытые» на обзоре. Если отключить, окна закрываются, а виджеты остаются откреплёнными и появятся снова, когда модуль включат.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "always_on_top": { "type": "boolean", "title": "Поверх других окон (иначе — под окнами, как на рабочем столе)", "default": false },
            "restore_on_start": { "type": "boolean", "title": "Открывать виджеты при запуске FocusDeck", "default": true }
        }}),
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
        description: "Сценарии «триггер → условия → действия» и наблюдатель событий Windows.",
        details: "Фоновая задача раз в секунду следит за активным окном, процессами, мониторами, \
                  сетью, временем и интервалами. Опасные действия (закрытие программ, команды, \
                  удаление файлов, запуск скриптов) выполняются только после подтверждения. Если \
                  отключить: наблюдатель останавливается, автоматизации не выполняются, их горячие \
                  клавиши снимаются. Сами сценарии сохраняются.",
        category: Category::Automations,
        version: "2.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "scheduler_interval_secs": { "type": "integer", "title": "Период проверки расписания (с)", "default": 30, "minimum": 10, "maximum": 600 },
            "max_log_entries": { "type": "integer", "title": "Записей журнала на автоматизацию", "default": 500, "minimum": 50, "maximum": 5000 },
            "command_timeout_secs": { "type": "integer", "title": "Таймаут команды терминала (с)", "default": 60, "minimum": 5, "maximum": 600 }
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
        name: "Clipboard Hub",
        description: "История буфера обмена (текст, ссылки, файлы, изображения, HTML, RTF) и быстрые действия.",
        details: "Фоновая задача раз в секунду проверяет, изменился ли буфер, и сохраняет его в локальную базу. Нужно разрешение «Чтение буфера обмена». Не записываются: скрытое содержимое менеджеров паролей, копирование из исключённых программ и приватных окон браузера, всё во время Private Mode. Если отключить модуль, запись прекращается, страница, виджет и горячие клавиши модуля отключаются; история и быстрые действия сохраняются.",
        category: Category::Clipboard,
        version: "2.0.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_CLIPBOARD_READ],
        settings_schema: || json!({ "type": "object", "properties": {
            "max_items": { "type": "integer", "title": "Максимум записей", "default": 500, "minimum": 10, "maximum": 10000 },
            "retention_days": { "type": "integer", "title": "Срок хранения, дней (0 — без ограничения)", "default": 30, "minimum": 0, "maximum": 3650 },
            "persist_history": { "type": "boolean", "title": "Сохранять историю после перезапуска", "default": true },
            "save_images": { "type": "boolean", "title": "Сохранять изображения", "default": true },
            "max_image_mb": { "type": "integer", "title": "Максимальный размер изображения, МБ", "default": 10, "minimum": 1, "maximum": 50 },
            "save_rich": { "type": "boolean", "title": "Сохранять форматирование (HTML, RTF)", "default": true },
            "max_length": { "type": "integer", "title": "Максимальная длина текста (символов)", "default": 100000, "minimum": 100, "maximum": 1000000 },
            "excluded_apps": { "type": "string", "title": "Исключённые программы (exe через запятую)", "default": "keepass.exe, keepassxc.exe, 1password.exe, bitwarden.exe" },
            "exclude_private_windows": { "type": "boolean", "title": "Не записывать из приватных окон браузера", "default": true },
            "skip_sensitive": { "type": "boolean", "title": "Не сохранять скрытое содержимое (пароли)", "default": true },
            "open_hotkey": { "type": "string", "title": "Горячая клавиша открытия (пусто — нет)", "default": "Ctrl+Alt+V" },
            "private_minutes": { "type": "integer", "title": "Private Mode по умолчанию, минут (0 — до выключения)", "default": 15, "minimum": 0, "maximum": 1440 },
            "command_timeout_secs": { "type": "integer", "title": "Таймаут команды быстрого действия, с", "default": 60, "minimum": 5, "maximum": 600 }
        }}),
    },
    ModuleDef {
        id: ORGANIZER,
        name: "Desktop Organizer",
        description: "Сортировка файлов рабочего стола по папкам с предпросмотром и отменой.",
        details: "Раскладывает скриншоты, изображения, документы, архивы, установщики, видео, музыку и \
                  код по папкам по правилам. Файлы только перемещаются, никогда не удаляются; каждое \
                  перемещение записывается в журнал и может быть отменено. Ярлыки, скрытые и системные \
                  файлы не трогаются. Если отключить модуль, расписание и проверка лимита \
                  останавливаются, страница скрывается; журнал и правила сохраняются.",
        category: Category::Files,
        version: "1.0.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_MOVE_FILES],
        settings_schema: || json!({ "type": "object", "properties": {
            "target_root": { "type": "string", "title": "Корневая папка для сортировки ({root})", "default": "%USERPROFILE%\\Documents\\Desktop Organizer" },
            "exclude_patterns": { "type": "string", "title": "Исключения (маски через запятую)", "default": "*.lnk, *.url, desktop.ini, ~$*, *.crdownload, *.part, *.tmp" },
            "min_age_minutes": { "type": "integer", "title": "Не трогать файлы моложе (мин)", "default": 5, "minimum": 0, "maximum": 1440 },
            "auto_mode": { "type": "string", "title": "Автоматические запуски", "default": "preview", "enum": ["preview", "move"] },
            "schedule_enabled": { "type": "boolean", "title": "По расписанию", "default": false },
            "schedule_time": { "type": "string", "title": "Время (ЧЧ:ММ)", "default": "18:00" },
            "schedule_weekdays_only": { "type": "boolean", "title": "Только по будням", "default": true },
            "run_on_login": { "type": "boolean", "title": "При входе в систему", "default": false },
            "limit_enabled": { "type": "boolean", "title": "При превышении лимита файлов", "default": false },
            "file_limit": { "type": "integer", "title": "Лимит файлов на рабочем столе", "default": 40, "minimum": 5, "maximum": 1000 },
            "notify": { "type": "boolean", "title": "Уведомления", "default": true }
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
        id: WINDOWS_INTEGRATION,
        name: "Windows Integration",
        description: "Окна, приложения, рабочие столы, звук, яркость, сеть, питание, скриншоты.",
        details: "Каждое действие проверяет своё разрешение и Safe Mode и записывается в журнал действий; \
                  изменения (громкость, яркость, схема питания, состояние окна, буфер обмена) можно \
                  отменить. Используются Windows API, PowerShell и стандартные утилиты; UI Automation — \
                  только с отдельным разрешением. Если отключить модуль, все его команды недоступны, \
                  страница скрыта.",
        category: Category::WindowsIntegration,
        version: "1.0.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_WINDOWS],
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
        name: "Интеграции",
        description: "Коннекторы к программам и сервисам: каталог, настройки, разрешения, журнал.",
        details: "13 коннекторов (GitHub, GitLab, VS Code, Windows Terminal, OBS, Twitch, Discord, Telegram, Notion, \
                  Google Calendar, Spotify, браузер, локальные файлы), каждый описан манифестом и выключен по умолчанию. \
                  Токены хранятся только в Windows Credential Manager. Offline Mode (включён по умолчанию) \
                  приостанавливает все облачные коннекторы; облачный коннектор включается только с подтверждением. \
                  На этом этапе коннекторы не выполняют действий и не обращаются к внешним API. \
                  Если отключить, страница скрывается, настройки и секреты сохраняются.",
        category: Category::Integrations,
        version: "1.0.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "offline_mode": { "type": "boolean", "title": "Offline Mode: приостановить облачные интеграции", "default": true },
            "log_limit": { "type": "integer", "title": "Записей журнала на интеграцию", "default": 500, "minimum": 50, "maximum": 10000 }
        }}),
    },
    ModuleDef {
        id: UPDATES,
        name: "Обновления",
        description: "Проверка новой версии, список изменений, установка и откат.",
        details: "Обращается к серверу релизов только при выданном разрешении «Сеть: проверка обновлений». \
                  Пакет обновления проверяется по цифровой подписи. Перед установкой база данных копируется — \
                  это точка отката. В релизной сборке проверка при запуске включена; найденное обновление показывается окном, \
                  установка всегда требует подтверждения. Если отключить, проверки не выполняются.",
        category: Category::PrivacySecurity,
        version: "1.0.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_NETWORK_UPDATES],
        settings_schema: || json!({ "type": "object", "properties": {
            "auto_check": { "type": "boolean", "title": "Проверять при запуске", "default": crate::updates::configured() }
        }}),
    },
    // --- Developer Tools ---
    ModuleDef {
        id: DEV_GIT,
        name: "Git",
        description: "Ветка, статус, staged/unstaged, коммиты, diff; commit, push и pull.",
        details: "Работает с установленным git. Чтение (статус, история, diff) разрешений не требует; \
                  stage, commit, push и pull — только с разрешением «Git: изменение репозиториев», каждое \
                  действие записывается в журнал. Push и pull выполняются только после подтверждения, \
                  pull — только fast-forward, push — никогда не принудительный. Есть виджет для «Обзора». \
                  Если отключить, страница и виджет скрываются.",
        category: Category::DeveloperTools,
        version: "1.0.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "log_limit": { "type": "integer", "title": "Коммитов в истории", "default": 30, "minimum": 5, "maximum": 200 },
            "git_path": { "type": "string", "title": "Путь к git.exe (пусто — из PATH)", "default": "" },
            "timeout_secs": { "type": "integer", "title": "Таймаут push/pull, с", "default": 120, "minimum": 10, "maximum": 900 }
        }}),
    },
    ModuleDef {
        id: DEV_PROJECTS,
        name: "Project Launcher",
        description: "Список проектов: тип, команды запуска, VS Code, терминал, Git.",
        details: "Тип проекта и команды определяются по файлам (package.json, Cargo.toml, pyproject.toml, \
                  *.sln, go.mod …). Открытие в VS Code, терминале и проводнике требует разрешения «Запуск \
                  программ», запуск команд проекта — «Выполнение команд». Все запуски записываются в журнал \
                  действий. Если отключить, страница скрывается, список проектов сохраняется.",
        category: Category::DeveloperTools,
        version: "1.0.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_LAUNCH_APPS],
        settings_schema: || json!({ "type": "object", "properties": {
            "editor_command": { "type": "string", "title": "Команда редактора", "default": "code" },
            "default_shell": { "type": "string", "title": "Терминал по умолчанию", "default": "powershell", "enum": ["powershell", "cmd", "git_bash", "wsl", "custom"] }
        }}),
    },
    ModuleDef {
        id: DEV_TERMINAL,
        name: "Terminal Quick Launcher",
        description: "PowerShell, CMD, Git Bash, WSL и свой shell: вкладки, история, избранное.",
        details: "Открывает терминал в отдельном окне (разрешение «Запуск программ») или выполняет команды \
                  во вкладках внутри FocusDeck (разрешение «Выполнение команд», каждая команда — в журнале \
                  действий). Вкладки выполняют команды без интерактивного ввода: для программ, которые \
                  ждут ввода, откройте терминал в окне. Если отключить, страница скрывается, история и \
                  избранное сохраняются.",
        category: Category::DeveloperTools,
        version: "1.0.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_RUN_COMMANDS],
        settings_schema: || json!({ "type": "object", "properties": {
            "custom_shell_path": { "type": "string", "title": "Свой shell: путь к exe", "default": "" },
            "custom_shell_args": { "type": "string", "title": "Свой shell: аргументы перед командой", "default": "-c" },
            "command_timeout_secs": { "type": "integer", "title": "Таймаут команды во вкладке, с", "default": 120, "minimum": 5, "maximum": 3600 },
            "history_limit": { "type": "integer", "title": "Хранить команд в истории", "default": 1000, "minimum": 50, "maximum": 20000 },
            "save_history": { "type": "boolean", "title": "Сохранять историю команд", "default": true }
        }}),
    },
    ModuleDef {
        id: DEV_API,
        name: "API Tester",
        description: "REST-запросы, коллекции, переменные окружения, история, экспорт в curl.",
        details: "Запросы уходят только по вашей команде и только при выданном разрешении «Сеть: API Tester». \
                  Переменные окружения ({{имя}}) хранятся локально в базе открытым текстом — не держите \
                  в них боевые секреты. Каждый запрос попадает в историю и журнал действий. Если отключить, \
                  страница скрывается, коллекции сохраняются.",
        category: Category::DeveloperTools,
        version: "1.0.0",
        enabled_by_default: false,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_NETWORK_API],
        settings_schema: || json!({ "type": "object", "properties": {
            "timeout_secs": { "type": "integer", "title": "Таймаут запроса, с", "default": 30, "minimum": 1, "maximum": 600 },
            "follow_redirects": { "type": "boolean", "title": "Следовать редиректам", "default": true },
            "max_response_kb": { "type": "integer", "title": "Показывать ответ до, КБ", "default": 2048, "minimum": 16, "maximum": 51200 },
            "history_limit": { "type": "integer", "title": "Хранить запросов в истории", "default": 500, "minimum": 10, "maximum": 10000 }
        }}),
    },
    ModuleDef {
        id: DEV_REGEX,
        name: "Regex Tester",
        description: "Live-проверка регулярных выражений с подсветкой групп и шаблонами.",
        details: "Выражение проверяется движком JavaScript (как в браузере и Node.js) в отдельном потоке: \
                  зависшее выражение останавливается по таймауту. Сохранённые выражения хранятся локально. \
                  Если отключить, страница скрывается.",
        category: Category::DeveloperTools,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "timeout_ms": { "type": "integer", "title": "Таймаут проверки, мс", "default": 1000, "minimum": 100, "maximum": 10000 },
            "max_matches": { "type": "integer", "title": "Показывать совпадений", "default": 1000, "minimum": 10, "maximum": 100000 }
        }}),
    },
    ModuleDef {
        id: DEV_JSON,
        name: "JSON/YAML Tools",
        description: "Форматирование, валидация, JSON ⇄ YAML, JSONPath, ошибки со строкой и столбцом.",
        details: "Всё выполняется локально, данные никуда не сохраняются. Если отключить, страница скрывается.",
        category: Category::DeveloperTools,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[],
        settings_schema: || json!({ "type": "object", "properties": {
            "indent": { "type": "integer", "title": "Отступ при форматировании", "default": 2, "minimum": 1, "maximum": 8 },
            "sort_keys": { "type": "boolean", "title": "Сортировать ключи", "default": false }
        }}),
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
    ModuleDef {
        id: WIDGET_MEDIA,
        name: "Виджет «Музыка»",
        description: "Что сейчас играет в Windows: трек, исполнитель, обложка и кнопки управления.",
        details: "Данные берутся из системных медиа-элементов Windows (Global System Media Transport Controls): Spotify, браузеры, Яндекс Музыка, «Медиаплеер» и другие программы, которые их поддерживают. Нужно разрешение «Медиа: что сейчас играет». Ничего не сохраняется и не отправляется в сеть. Если отключить, виджет скрывается во всех макетах и фоновое отслеживание останавливается.",
        category: Category::Widgets,
        version: "1.0.0",
        enabled_by_default: true,
        critical: false,
        requires_restart: false,
        permissions: &[PERM_MEDIA],
        settings_schema: || json!({ "type": "object", "properties": {
            "show_cover": { "type": "boolean", "title": "Показывать обложку", "default": true },
            "show_equalizer": { "type": "boolean", "title": "Показывать эквалайзер", "default": true },
            "show_source": { "type": "boolean", "title": "Показывать программу-источник", "default": true },
            "auto_compact": { "type": "boolean", "title": "На рабочем столе: сворачивать в мини-плеер поверх окон, когда фокус уходит в другое окно", "default": true }
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
