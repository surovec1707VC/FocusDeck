# Module Registry — API

Все команды вызываются из фронтенда через `invoke` (обёртки — `src/lib/api.ts`). Ошибки приходят как `{ kind, message }`; `message` — готовый текст на русском.

| `kind` | Когда |
|---|---|
| `not_found` | неизвестный модуль |
| `invalid` | настройки не прошли проверку схемы, неверный файл конфигурации |
| `confirmation_required` | отключение критического модуля без `confirm: true` |
| `module_disabled` | действие требует модуля, который выключен |
| `permission_denied`, `database`, `json`, `internal` | прочие |

## Модель модуля

```ts
interface ModuleInfo {
  id: string;                    // "widget_clock"
  name: string;                  // "Виджет «Часы»"
  description: string;           // одна строка
  details: string;               // что происходит при отключении и т.п.
  category: ModuleCategory;      // см. ниже
  version: string;               // "1.0.0"
  enabled: boolean;
  critical: boolean;             // отключение — только с подтверждением
  requires_restart: boolean;     // изменения применяются после перезапуска
  pending_restart: boolean;      // с момента запуска были такие изменения
  permissions: { id, name, granted }[];
  missing_permissions: string[]; // id невыданных разрешений
  settings: object;              // действующие настройки (сохранённые поверх значений по умолчанию)
  settings_schema: object;       // схема настроек (см. ниже)
  updated_at: string;
}
```

Категории (`ModuleCategory`): `appearance`, `widgets`, `layouts`, `automations`, `productivity`, `developer_tools`, `clipboard`, `files`, `windows_integration`, `notifications`, `privacy_security`, `integrations`, `performance`. Русские названия — в `src/i18n/ru.ts` → `categories`.

### Схема настроек
Подмножество JSON Schema:

```json
{ "type": "object", "properties": {
    "scheduler_interval_secs": { "type": "integer", "title": "Период проверки расписания (с)", "default": 30, "minimum": 10, "maximum": 600 },
    "level": { "type": "string", "title": "Уровень", "default": "info", "enum": ["error", "warn", "info", "debug", "trace"] }
} }
```

Поддерживаются `type` (`boolean`, `integer`, `number`, `string`), `default`, `minimum`, `maximum`, `enum`. Неизвестные ключи отклоняются. Сохранённые ключи, которых больше нет в схеме, отбрасываются при чтении.

## Команды

### `list_modules() → ModuleInfo[]`
Все модули в порядке каталога.

### `get_module(id) → ModuleInfo`

### `enable_module(id) → { module: ModuleInfo, warnings: string[] }`
Включает модуль и применяет эффекты (горячие клавиши, фоновые задачи, трей, автозапуск). `warnings` — проблемы применения, которые не отменили включение (например, сочетание занято другой программой).

### `disable_module(id, confirm?: boolean) → { module, warnings }`
Отключает модуль, его настройки сохраняются. Для критического модуля без `confirm: true` возвращает `confirmation_required`; попытка пишется в журнал как `disable_blocked`.

### `update_module_settings(id, settings) → ModuleInfo`
Проверяет `settings` по схеме и сохраняет поверх значений по умолчанию. Частичный объект допустим: отсутствующие ключи получают значение по умолчанию. Требует включённого модуля «Реестр модулей».

### `reset_module_settings(id) → ModuleInfo`
Возвращает значения по умолчанию. Требует «Реестр модулей».

### `get_module_status(id) → ModuleStatus`
Запускает health check и пишет результат в журнал модуля.

```ts
interface ModuleStatus {
  module_id: string;
  enabled: boolean;
  state: "ok" | "degraded" | "error" | "disabled";
  message: string;          // «планировщик работает», «нет разрешений: …»
  pending_restart: boolean;
  checked_at: string;       // RFC 3339
}
```

Что проверяется: `database` — `PRAGMA quick_check`; `automations` — запущен ли планировщик; `hotkeys` — зарегистрировано ли сочетание окна; `tray` — существует ли значок; `autostart` — совпадает ли запись Windows с настройкой; `logging` — есть ли папка журнала. Модули без выданных разрешений — `degraded`; модули «в разработке» — `degraded`; выключенные — `disabled`.

### `get_module_logs(id, limit = 100) → ModuleLog[]`
Новые записи первыми.

```ts
interface ModuleLog { id: number; module_id: string; level: "info" | "warn" | "error";
  action: string; message: string; source: "user" | "import" | "system"; created_at: string }
```

Действия (`action`): `enable`, `disable`, `disable_blocked`, `settings_update` (с перечнем изменённых ключей), `settings_reset`, `health_check`, `apply` (предупреждения при применении); у `module_registry` дополнительно `config_export` и `config_import`. Хранится не больше `module_registry.log_retention` записей на модуль (по умолчанию 1000).

### `get_module_permissions(id) → { id, name, granted }[]`

## Экспорт и импорт конфигурации

Конфигурация — это настройки приложения, состояние и настройки модулей, разрешения. Пользовательские данные (макеты, виджеты, задачи, автоматизации) в неё не входят. Все команды требуют включённого модуля «База данных».

| Команда | Что делает |
|---|---|
| `export_config() → string` | JSON-конфигурация |
| `import_config(json, confirm?) → ImportReport` | применяет JSON |
| `export_config_to_file() → string \| null` | диалог «Сохранить» + запись файла; `null`, если отменено |
| `read_config_file() → string \| null` | диалог «Открыть», возвращает содержимое (до 1 МБ) |

Формат файла:

```json
{
  "format": "focusdeck.config",
  "version": 1,
  "exported_at": "2026-10-20T02:10:00+00:00",
  "app_version": "0.1.0",
  "settings": { "theme": "dark", "language": "ru", "close_to_tray": true, "...": "..." },
  "modules": [ { "id": "widget_clock", "enabled": true, "settings": { "show_seconds": true, "show_date": true } } ],
  "permissions": [ { "id": "system.autostart", "granted": false } ]
}
```

Правила импорта:
- `format` должен совпадать, `version` — не новее поддерживаемой.
- Неизвестные модули и разрешения, а также настройки, не прошедшие проверку, пропускаются и попадают в `skipped`.
- Если файл отключает критические модули, без `confirm: true` возвращается `confirmation_required`.
- Если модуль «Настройки» выключен, настройки приложения не импортируются; если выключена «Безопасность» — не импортируются разрешения (оба случая — в `warnings`).
- После импорта применяются эффекты всех изменённых модулей, а UI получает события `focusdeck://modules-changed` и `focusdeck://settings-changed`.

```ts
interface ImportReport { settings_applied: boolean; modules_applied: number;
  permissions_applied: number; skipped: string[]; warnings: string[] }
```

## События

| Событие | Когда |
|---|---|
| `focusdeck://modules-changed` | любое изменение модулей (вкл/выкл, настройки, импорт, изменение разрешений) |
| `focusdeck://settings-changed` | настройки приложения изменены не со страницы «Настройки» (импорт) |

## Что происходит при отключении модуля

| Эффект | Как реализовано |
|---|---|
| виджеты скрываются | `widgets_list` не возвращает виджеты, чей модуль-владелец выключен (`clock` → `widget_clock`, `status` → `widget_status`, `tasks` → `tasks`, `note` → `notes`); при выключенном модуле «Виджеты» скрыты все. Сами виджеты остаются в БД |
| горячие клавиши отключаются | `hotkeys::sync` снимает все сочетания и регистрирует только сочетания включённых модулей (`hotkeys` — окно; `automations` и модуль автоматизации — её сочетание) |
| фоновые задачи останавливаются | `background::reconcile` прерывает задачу модуля (планировщик автоматизаций) и запускает её снова при включении |
| автоматизации не выполняются | перед запуском проверяются модуль `automations` и `module_id` автоматизации; пропуск пишется в журнал запусков как `skipped`. Действие «уведомление» пропускается, если выключен модуль `notifications` |
| настройки сохраняются | отключение меняет только флаг `enabled`, `settings_json` не трогается |

Особые случаи:
- **Трей**: если модуль «Значок в трее» выключен, значок скрывается, а закрытие окна завершает программу.
- **Автозапуск**: при выключенном модуле запись в Windows удаляется, а выбор в настройках сохраняется и применяется при повторном включении.
- **Журнал приложения**: уровень и запись в файл применяются после перезапуска (`requires_restart`).

## Критические модули

`module_registry`, `settings`, `database`, `security`. Отключаются только после подтверждения (в UI — диалог).

| Модуль | Если отключён |
|---|---|
| Реестр модулей | нельзя менять и сбрасывать настройки модулей, страница «Модули» скрыта. Включать и отключать модули в Центре управления можно всегда |
| Настройки | страница скрыта, `settings_update` → `module_disabled`; текущие настройки действуют |
| База данных | экспорт и импорт конфигурации недоступны; нет сжатия WAL при выходе |
| Безопасность | страница «Разрешения» скрыта, менять разрешения нельзя; выданные разрешения продолжают проверяться |

## SQL

`src-tauri/src/db/migrations/0002_module_registry.sql`:
- `modules`: `is_core` → `is_critical`; новые колонки `version`, `requires_restart`, `settings_json`.
- новая таблица `module_logs(id, module_id, level, action, message, source, created_at)`.
- `automations.module_id` (по умолчанию `automations`).

## Каталог модулей

| id | Категория | Крит. | Перезапуск | Разрешения | Настройки |
|---|---|---|---|---|---|
| module_registry | developer_tools | ✔ | | | log_retention |
| settings | appearance | ✔ | | | — |
| database | performance | ✔ | | | checkpoint_on_exit |
| security | privacy_security | ✔ | | | log_denials |
| dashboard | layouts | | | | show_recent_runs |
| widgets | widgets | | | | show_titles |
| widget_clock | widgets | | | | show_seconds, show_date |
| widget_status | widgets | | | | — |
| layouts | layouts | | | | — |
| automations | automations | | | | scheduler_interval_secs, max_log_entries |
| tasks | productivity | | | | hide_completed |
| notes | productivity | | | | — |
| snippets | productivity | | | | — (в разработке) |
| logging | developer_tools | | ✔ | | level |
| clipboard_history | clipboard | | | clipboard.read | max_items (в разработке) |
| files | files | | | fs.read | — (в разработке) |
| hotkeys | windows_integration | | | system.global_hotkeys | — |
| tray | windows_integration | | | | — |
| autostart | windows_integration | | | system.autostart | — |
| notifications | notifications | | | app.notifications | duration_ms, max_visible |
| integrations | integrations | | | | — (в разработке) |

Добавить модуль: запись в `src-tauri/src/modules/catalog.rs` (`BUILTIN`) → при следующем запуске `registry::seed` создаст строку с настройками по умолчанию. Эффекты включения и отключения — в `modules/effects.rs`, проверка состояния — в `modules/health.rs`.
