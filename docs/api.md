# API (IPC-команды)

Фронтенд обращается к Rust только через команды Tauri: `invoke(name, args)`, обёртки — в `src/lib/api.ts`.

**Соглашения:**
- Аргументы передаются в camelCase; Tauri приводит их к snake_case параметров Rust.
- Результат — JSON. Ошибка — `{ kind, message }`, где `message` на русском. Виды ошибок: `not_found`, `invalid`, `permission_denied`, `confirmation_required`, `module_disabled`, `blocked` (Safe Mode), `offline`, `database`, `json`, `internal`.
- Команда выключенного модуля отвечает `module_disabled`.
- Опасные действия сначала отвечают `confirmation_required` и выполняются только при повторном вызове с `confirm: true` — так делает `useGuarded` во фронтенде.
- События (backend → UI): `focusdeck://modules-changed`, `focusdeck://layout-changed`, `focusdeck://action-log`, `focusdeck://notify` и события виджетов; полный список — `EVENTS` в `src/lib/api.ts`.

Всего команд: **224**.

| Модуль Rust | Команды | Подробно |
|---|---|---|
| `info` | `app_info`, `dashboard_stats` | — |
| `session` | `session_status`, `session_ack`, `backup_create`, `backup_restore`, `app_restart` | — |
| `transfer` | `data_export`, `data_import` | — |
| `updates` | `updates_status`, `updates_check`, `updates_install`, `updates_rollback` | — |
| `settings` | `settings_get`, `settings_update` | — |
| `permissions` | `permissions_list`, `permissions_set`, `permissions_logs`, `permissions_center`, `permissions_access_log` | [windows-integration.md](windows-integration.md) |
| `safety` | `safety_actions`, `safe_mode_get`, `safe_mode_set`, `action_log_list`, `action_log_undo` | [windows-integration.md](windows-integration.md) |
| `windows_integration` | `win_windows`, `win_active_window`, `win_window`, `win_apps`, `win_launch`, `win_desktops`, `win_desktop`, `win_volume`, `win_set_volume`, `win_brightness`, `win_set_brightness`, `win_power_plans`, `win_set_power_plan`, `win_wifi`, `win_bluetooth`, `win_screenshot`, `win_clipboard_read`, `win_clipboard_write`, `win_notify`, `win_uia_inspect` | [windows-integration.md](windows-integration.md) |
| `modules` | `list_modules`, `get_module`, `enable_module`, `disable_module`, `update_module_settings`, `reset_module_settings`, `get_module_status`, `get_module_logs`, `get_module_permissions`, `export_config`, `import_config`, `export_config_to_file`, `read_config_file` | [api.md](api.md#команды) |
| `workspace` | `layouts_list`, `layouts_create`, `layouts_rename`, `layouts_duplicate`, `layouts_activate`, `layouts_delete`, `layouts_set_hotkey`, `tasks_list`, `tasks_add`, `tasks_toggle`, `tasks_delete` | [widgets.md](widgets.md) |
| `widgets` | `widget_catalog`, `widgets_list`, `widgets_add`, `widgets_update`, `widgets_save_grid`, `widgets_remove`, `widget_data`, `widget_action`, `widget_pick_app` | [widgets.md](widgets.md) |
| `productivity::notes` | `notes_list`, `notes_facets`, `notes_create`, `notes_update`, `notes_trash`, `notes_restore`, `notes_delete_forever`, `notes_empty_trash`, `notes_export` | [productivity.md](productivity.md) |
| `productivity::tasks` | `tasks_query`, `tasks_create`, `tasks_quick_add`, `tasks_quick_parse`, `tasks_update`, `tasks_remove`, `tasks_stats` | [productivity.md](productivity.md) |
| `productivity::snippets` | `snippets_list`, `snippets_save`, `snippets_delete`, `snippets_render`, `snippets_copy`, `snippets_paste`, `snippets_export`, `snippets_import` | [productivity.md](productivity.md) |
| `productivity::timers` | `timers_state`, `timers_save`, `timers_control`, `timers_delete`, `timer_sessions`, `timer_sessions_clear`, `stopwatches_create`, `stopwatches_rename`, `stopwatches_control`, `stopwatches_delete`, `stopwatches_save_result`, `stopwatch_results`, `stopwatch_results_delete` | [productivity.md](productivity.md) |
| `clipboard_hub` | `clipboard_list`, `clipboard_image`, `clipboard_details`, `clipboard_copy`, `clipboard_paste`, `clipboard_pin`, `clipboard_delete`, `clipboard_clear`, `clipboard_status`, `clipboard_set_private`, `quick_actions_list`, `quick_actions_save`, `quick_actions_delete`, `quick_actions_reorder`, `quick_actions_run`, `quick_actions_pick` | [clipboard-hub.md](clipboard-hub.md) |
| `organizer` | `organizer_preview`, `organizer_apply`, `organizer_run_now`, `organizer_pending`, `organizer_dismiss_pending`, `organizer_operations`, `organizer_moves`, `organizer_undo_last`, `organizer_undo_operation`, `organizer_restore`, `organizer_rules`, `organizer_save_rule`, `organizer_delete_rule`, `organizer_reorder_rules`, `organizer_reset_rules`, `organizer_exclude`, `organizer_paths`, `organizer_open_folder` | [desktop-organizer.md](desktop-organizer.md) |
| `automation` | `automations_list`, `automations_create`, `automations_update`, `automations_set_enabled`, `automations_delete`, `automations_reorder`, `automations_run`, `automations_test`, `automations_logs`, `automations_pending`, `automations_confirm`, `automations_templates`, `automations_install_template`, `automations_schema`, `automations_pick` | [automations.md](automations.md) |
| `focus` | `focus_mode_get`, `focus_mode_set` | — |
| `integrations` | `integrations_list`, `integrations_get`, `integrations_set_enabled`, `integrations_set_offline`, `integrations_update_settings`, `integrations_reset_settings`, `integrations_set_permission`, `integrations_set_secret`, `integrations_clear_secret`, `integrations_check`, `integrations_logs` | [integrations.md](integrations.md) |
| `devtools` | `devtools_pick_folder` | [developer-tools.md](developer-tools.md) |
| `devtools::git` | `git_repos`, `git_open_repo`, `git_forget_repo`, `git_status`, `git_log`, `git_diff`, `git_stage`, `git_commit`, `git_pull`, `git_push` | [developer-tools.md](developer-tools.md) |
| `devtools::projects` | `projects_list`, `projects_add`, `projects_scan`, `projects_update`, `projects_redetect`, `projects_remove`, `projects_open`, `projects_run` | [developer-tools.md](developer-tools.md) |
| `devtools::terminal` | `terminal_shells`, `terminal_home`, `terminal_open`, `terminal_run`, `terminal_cancel`, `terminal_history`, `terminal_history_clear`, `terminal_favorites`, `terminal_favorite_save`, `terminal_favorite_delete` | [developer-tools.md](developer-tools.md) |
| `devtools::api` | `api_send`, `api_to_curl`, `api_workspace`, `api_collection_save`, `api_collection_delete`, `api_request_save`, `api_request_delete`, `api_environments`, `api_environment_save`, `api_environment_delete`, `api_environment_activate`, `api_history`, `api_history_clear` | [developer-tools.md](developer-tools.md) |
| `devtools::regex` | `regex_list`, `regex_save`, `regex_delete` | [developer-tools.md](developer-tools.md) |
| `devtools::jsonyaml` | `devjson_process`, `devjson_query` | [developer-tools.md](developer-tools.md) |

Команды без отдельного документа:
- `info` — данные для «Обзора» и «О программе»;
- `session` — статус сеанса, резервная копия и восстановление, перезапуск;
- `transfer` — экспорт и импорт данных;
- `updates` — обновления (см. [release.md](release.md));
- `focus` — Focus Mode;
- `notifications` — история уведомлений.

| Команда | Аргументы | Результат |
|---|---|---|
| `session_status` | — | `{ previous_crash, crash_message, restored, restore_pending }` |
| `session_ack` | — | — |
| `backup_create` | — | путь или `null` (отмена); копия через `VACUUM INTO` |
| `backup_restore` | `confirm` | путь или `null`; копия проверяется и применяется при следующем запуске |
| `app_restart` | — | перезапуск |
| `data_export` | `sections[]`, `includeConfig` | путь или `null` |
| `data_import` | `confirm` | `{ imported, skipped, config_applied, warnings }` или `null` |
| `updates_status` | — | `{ current, configured, available, previous, releases_url }` без сети |
| `updates_check` | — | то же, с `available`; нужен модуль и `network.updates` |
| `updates_install` | `confirm` | копия базы, затем установка и перезапуск |
| `updates_rollback` | `confirm` | `{ version, backup, updated_at }`; копия ставится на восстановление |

Разделы `data_export`: `layouts` (макеты и виджеты), `notes`, `tasks`, `calendar`, `snippets`, `automations`. Формат файла — `{ format: "focusdeck-data", version: 1, app_version, exported_at, schema, config?, tables: { <таблица>: [строки] } }`. При импорте записи с тем же `id` заменяются, остальные добавляются; файл более новой схемы отклоняется.

---

## Module Registry

Все команды вызываются из фронтенда через `invoke` (обёртки — `src/lib/api.ts`). Ошибки приходят как `{ kind, message }`; `message` — готовый текст на русском.

| `kind` | Когда |
|---|---|
| `not_found` | неизвестный модуль |
| `invalid` | настройки не прошли проверку схемы, неверный файл конфигурации |
| `confirmation_required` | отключение критического модуля без `confirm: true` |
| `module_disabled` | действие требует модуля, который выключен |
| `permission_denied`, `database`, `json`, `internal` | прочие |

### Модель модуля

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

#### Схема настроек
Подмножество JSON Schema:

```json
{ "type": "object", "properties": {
    "scheduler_interval_secs": { "type": "integer", "title": "Период проверки расписания (с)", "default": 30, "minimum": 10, "maximum": 600 },
    "level": { "type": "string", "title": "Уровень", "default": "info", "enum": ["error", "warn", "info", "debug", "trace"] }
} }
```

Поддерживаются `type` (`boolean`, `integer`, `number`, `string`), `default`, `minimum`, `maximum`, `enum`. Неизвестные ключи отклоняются. Сохранённые ключи, которых больше нет в схеме, отбрасываются при чтении.

### Команды

#### `list_modules() → ModuleInfo[]`
Все модули в порядке каталога.

#### `get_module(id) → ModuleInfo`

#### `enable_module(id) → { module: ModuleInfo, warnings: string[] }`
Включает модуль и применяет эффекты (горячие клавиши, фоновые задачи, трей, автозапуск). `warnings` — проблемы применения, которые не отменили включение (например, сочетание занято другой программой).

#### `disable_module(id, confirm?: boolean) → { module, warnings }`
Отключает модуль, его настройки сохраняются. Для критического модуля без `confirm: true` возвращает `confirmation_required`; попытка пишется в журнал как `disable_blocked`.

#### `update_module_settings(id, settings) → ModuleInfo`
Проверяет `settings` по схеме и сохраняет поверх значений по умолчанию. Частичный объект допустим: отсутствующие ключи получают значение по умолчанию. Требует включённого модуля «Реестр модулей».

#### `reset_module_settings(id) → ModuleInfo`
Возвращает значения по умолчанию. Требует «Реестр модулей».

#### `get_module_status(id) → ModuleStatus`
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

#### `get_module_logs(id, limit = 100) → ModuleLog[]`
Новые записи первыми.

```ts
interface ModuleLog { id: number; module_id: string; level: "info" | "warn" | "error";
  action: string; message: string; source: "user" | "import" | "system"; created_at: string }
```

Действия (`action`): `enable`, `disable`, `disable_blocked`, `settings_update` (с перечнем изменённых ключей), `settings_reset`, `health_check`, `apply` (предупреждения при применении); у `module_registry` дополнительно `config_export` и `config_import`. Хранится не больше `module_registry.log_retention` записей на модуль (по умолчанию 1000).

#### `get_module_permissions(id) → { id, name, granted }[]`

### Экспорт и импорт конфигурации

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

### События

| Событие | Когда |
|---|---|
| `focusdeck://modules-changed` | любое изменение модулей (вкл/выкл, настройки, импорт, изменение разрешений) |
| `focusdeck://settings-changed` | настройки приложения изменены не со страницы «Настройки» (импорт) |

### Что происходит при отключении модуля

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

### Критические модули

`module_registry`, `settings`, `database`, `security`. Отключаются только после подтверждения (в UI — диалог).

| Модуль | Если отключён |
|---|---|
| Реестр модулей | нельзя менять и сбрасывать настройки модулей, страница «Модули» скрыта. Включать и отключать модули в Центре управления можно всегда |
| Настройки | страница скрыта, `settings_update` → `module_disabled`; текущие настройки действуют |
| База данных | экспорт и импорт конфигурации недоступны; нет сжатия WAL при выходе |
| Безопасность | страница «Разрешения» скрыта, менять разрешения нельзя; выданные разрешения продолжают проверяться |

### SQL

`src-tauri/src/db/migrations/0002_module_registry.sql`:
- `modules`: `is_core` → `is_critical`; новые колонки `version`, `requires_restart`, `settings_json`.
- новая таблица `module_logs(id, module_id, level, action, message, source, created_at)`.
- `automations.module_id` (по умолчанию `automations`).

### Каталог модулей

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
