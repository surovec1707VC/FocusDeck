# Storage & Module Registry

## SQLite (`src-tauri/src/db`)
- Файл БД: `%APPDATA%\com.focusdeck.app\focusdeck.db` (WAL, foreign keys on).
- `db::connect()` открывает пул и применяет миграции `sqlx::migrate!("./src/db/migrations")` при каждом старте.
- `0001_init.sql` — 23 таблицы: settings, modules, module_settings, widget_layouts, widgets, automations, automation_logs, ai_sessions, ai_messages, prompts, projects, project_context, tasks, agent_tasks, agent_logs, connectors, connector_credentials_metadata, skills, permissions, permission_logs, clipboard_history, snippets, notes.
- Секреты в БД не хранятся: `connector_credentials_metadata` содержит только ключ записи в keyring.
- Запросы — `sqlx::query`/`query_as` без compile-time макросов (не требует `DATABASE_URL`).

## Module Registry (`src-tauri/src/modules`)
| Файл | Что внутри |
|---|---|
| `types.rs` | `ModuleManifest` (id, name, description, category, enabled_by_default, requires_restart, permissions, settings_schema), трейт `Module` (`init`, `shutdown`, `health_check`, `on_settings_changed`), `ModuleInfo` для фронтенда |
| `registry.rs` | `ModuleRegistry`: `sync`, `list`, `get`, `get_settings`, `update_settings`, `reset_settings`, `enable`, `disable`, `logs`, `status`, `shutdown_all` |
| `settings.rs` | дефолты из схемы, merge, валидация типов/enum (+ unit-тесты) |
| `status.rs` | `ModuleStatus`, `ModuleHealth`, кольцевой буфер логов (200 записей на модуль, в памяти) |
| `mod.rs` | встроенные модули (пока заглушки) и Tauri-команды |

Включение/настройки хранятся в таблице `modules`; модуль сам их не сохраняет, а только реагирует через хуки.

Tauri commands: `list_modules`, `get_module`, `enable_module`, `disable_module`, `update_module_settings`, `reset_module_settings`, `get_module_logs`, `get_module_status`.

Встроенные модули: ai_router, skills, connectors, clipboard_history, snippets, notes, automations, voice, system_monitor.

### Как добавить модуль
1. Реализовать `Module` для своей структуры.
2. Добавить её в `builtin_modules()`; при следующем запуске `sync()` создаст строку в БД с дефолтами из `settings_schema`.

## Frontend
- `src/lib/modules.ts` — типы и обёртки `invoke`.
- `src/pages/ControlCenter.tsx` — поиск, фильтр по категории, toggle, описание, статус (кнопка «Check status» запускает health check), вкладки Settings / Permissions / Logs, reset.
- `src/pages/ModuleSettings.tsx` — форма, сгенерированная из `settings_schema`.
- `src/pages/Modules.tsx` — сводная таблица, «Open» открывает модуль в Control Center.
