# Integrations

Модуль `integrations` («Интеграции», категория «Интеграции», по умолчанию выключен) — система коннекторов. Каждый коннектор описан манифестом и включается отдельно. Страница — `/integrations`.

**Этап 1** (текущий): манифесты, реестр, UI, настройки, разрешения, секреты, проверки состояния (заглушки) и журнал. Коннекторы **не выполняют действий и не обращаются к внешним API**.

## Архитектура

```
UI  src/pages/Integrations.tsx — каталог (поиск, фильтры), Offline Mode, карточка коннектора:
    Обзор · Настройки · Доступ (секрет + разрешения) · Действия и события · Журнал
     │ invoke(...)           useGuarded: confirmation_required → диалог → повтор с confirm=true
     ▼
src-tauri/src/integrations/
  manifests/*.json  13 манифестов (include_str! — вшиты в exe)
  manifest.rs       типы, all() (разбор и проверка один раз), validate(), get()
  store.rs          состояние в SQLite: connectors, connector_settings, connector_logs
  secrets.rs        Windows Credential Manager (CredWriteW / CredReadW / CredDeleteW)
  health.rs         проверки без сети: секрет, программа, папка, локальный порт
  mod.rs            команды, Offline Mode, правила включения
```

Поток данных:
- Манифест — статичное описание: что умеет коннектор и что ему нужно.
- Строка `connectors` — выбор пользователя: включён ли, какие разрешения выданы, последняя проверка.
- `connector_settings` хранит настройки (ключ → JSON) поверх значений по умолчанию из манифеста.
- Секрет лежит только в Credential Manager.

Новый коннектор:
1. JSON-файл в `manifests/`.
2. Строка в `SOURCES` (`manifest.rs`).
3. Иконка в `ICONS` (`Integrations.tsx`).

Строка в БД создаётся при старте (`store::seed`). Ошибка в манифесте роняет тест `manifests_load_and_validate`.

## Формат манифеста

Полная схема — [integration.schema.json](integration.schema.json).

| Поле | Значение |
|---|---|
| `id`, `name`, `description`, `icon`, `version` | идентичность; `icon` — ключ иконки в UI |
| `kind` | `local` (только этот компьютер) или `cloud` (внешний сервис) |
| `auth` | `{ type: none \| token \| api_key \| oauth2 \| password, label, hint, optional }` |
| `permissions` | `[{ id, name, description, risk: read \| change \| dangerous }]` — разрешения коннектора |
| `actions` | `[{ id, name, description, permissions: [id…] }]` — что коннектор умеет; ссылки на разрешения проверяются |
| `events` | `[{ id, name, description }]` — события для будущих триггеров автоматизаций |
| `settings` | схема настроек того же формата, что у модулей; у каждого ключа есть `default`; секретов здесь быть не может |
| `health` | `{ kind: none \| token \| executable \| path \| local_port, target?, setting? }` |
| `enabled_by_default` | у всех 13 — `false` |
| `docs_url` | ссылка на документацию API, только `https://` |

Пример (сокращён):

```json
{
  "id": "obs", "name": "OBS Studio", "icon": "obs", "version": "0.1.0", "kind": "local",
  "auth": { "type": "password", "label": "Пароль obs-websocket", "optional": true },
  "permissions": [{ "id": "scenes.switch", "name": "Переключение сцен", "description": "…", "risk": "change" }],
  "actions": [{ "id": "scene.set", "name": "Переключить сцену", "description": "…", "permissions": ["scenes.switch"] }],
  "events": [{ "id": "scene.changed", "name": "Сцена изменилась", "description": "…" }],
  "settings": { "type": "object", "properties": { "port": { "type": "integer", "default": 4455, "minimum": 1, "maximum": 65535 } } },
  "health": { "kind": "local_port", "setting": "port" },
  "enabled_by_default": false,
  "docs_url": "https://github.com/obsproject/obs-websocket/blob/master/docs/generated/protocol.md"
}
```

## Коннекторы

| id | Название | Вид | Auth | Проверка |
|---|---|---|---|---|
| `github` | GitHub | cloud | token | секрет сохранён |
| `gitlab` | GitLab | cloud | token | секрет сохранён |
| `vscode` | VS Code | local | — | `code` в PATH или путь из настроек |
| `windows_terminal` | Windows Terminal | local | — | `wt` в PATH |
| `obs` | OBS Studio | local | password (необяз.) | порт obs-websocket на 127.0.0.1 |
| `twitch` | Twitch | cloud | oauth2 | секрет сохранён |
| `discord` | Discord | cloud | api_key (webhook / бот) | секрет сохранён |
| `telegram` | Telegram | cloud | api_key (токен бота) | секрет сохранён |
| `notion` | Notion | cloud | token | секрет сохранён |
| `google_calendar` | Google Calendar | cloud | oauth2 | секрет сохранён |
| `spotify` | Spotify | cloud | oauth2 | секрет сохранён |
| `browser` | Браузер | local | — | всегда готов |
| `local_files` | Локальные файлы | local | — | папка из настроек существует |

У OAuth-коннекторов пока нет входа через браузер, но токен можно сохранить вручную. OAuth (PKCE + loopback) появится на следующем этапе.

## Безопасность

- **Секреты.** Хранятся в Windows Credential Manager как generic credential `FocusDeck/integration/<id>` (persist: этот пользователь, этот компьютер).
  - В SQLite, настройках, журналах и ответах команд их нет: UI получает только `has_secret`.
  - Значение принимается один раз и обрезается по краям; управляющие символы и строки длиннее 4096 символов отклоняются.
  - Удаление секрета требует подтверждения в UI.
- **Offline Mode.** Настройка модуля `offline_mode`, по умолчанию **включена**.
  - Облачный коннектор нельзя включить: ошибка вида `offline`.
  - Уже включённые облачные коннекторы приостанавливаются (статус `offline`), но не выключаются.
  - Локальные коннекторы работают как обычно.
  - Остальное приложение от облака не зависит.
- **Включение облачного коннектора** требует выключенного Offline Mode и явного подтверждения (`confirmation_required` → диалог → `confirm: true`). Локальные коннекторы включаются без подтверждения. Выключить можно всегда.
- **Разрешения коннектора** по умолчанию все выключены. Каждое изменение пишется в журнал. Действие станет доступно, только когда выданы все его разрешения; в UI это видно уже сейчас.
- **Проверки состояния** не ходят в сеть. Единственное сетевое действие — TCP-подключение к `127.0.0.1:<port>` для OBS (таймаут 800 мс).

### Статусы
| Статус | Когда |
|---|---|
| `ok` | всё нужное на месте |
| `warning` | например, OBS не запущен |
| `error` | программа или папка не найдена |
| `not_configured` | нет секрета или настройки |
| `offline` | облачный коннектор при Offline Mode |
| `disabled` | коннектор выключен |
| `unknown` | ещё не проверялся |

Проверка запускается:
- при включении коннектора;
- при изменении настроек или секрета;
- кнопками «Проверить» и «Проверить все».

## База данных — миграция `0010_integrations.sql`

- `connectors` (из 0001), новые поля:
  - `granted_json` — выданные разрешения;
  - `health_status`, `health_message`, `health_checked_at` — последняя проверка.
- `connector_settings` (из 0001) — настройки, ключ → JSON.
- `connector_logs` — журнал:
  - поля: `connector_id`, `level` (info / warn / error), `action` (enable, disable, settings, permission, secret, health, offline), `message`, `created_at`;
  - на каждый коннектор хранится не больше `log_limit` записей (настройка модуля, по умолчанию 500).

## Команды

Все возвращают `module_disabled`, пока модуль выключен.

| Команда | Аргументы | Результат |
|---|---|---|
| `integrations_list` | — | `{ offline_mode, connectors: ConnectorInfo[] }` |
| `integrations_get` | `id` | `ConnectorInfo` |
| `integrations_set_enabled` | `id, enabled, confirm?` | `ConnectorInfo` (после проверки); ошибки `offline`, `confirmation_required` |
| `integrations_set_offline` | `enabled` | `bool` |
| `integrations_update_settings` | `id, settings` (частично) | `ConnectorInfo` |
| `integrations_reset_settings` | `id` | `ConnectorInfo` |
| `integrations_set_permission` | `id, permission, granted` | `ConnectorInfo` |
| `integrations_set_secret` | `id, secret` | `ConnectorInfo` (секрет не возвращается) |
| `integrations_clear_secret` | `id` | `ConnectorInfo` |
| `integrations_check` | `id?` | `ConnectorInfo[]` (все, если `id` не задан) |
| `integrations_logs` | `id?, limit?` | `ConnectorLog[]`, новые первыми |

`ConnectorInfo = { manifest, enabled, granted, has_secret, settings, health_status, health_message, health_checked_at }`.

## Следующие этапы

1. Выполнение действий через `safety::authorize`: разрешение коннектора, Safe Mode, подтверждение, Action Log. Сначала локальные коннекторы: VS Code, Windows Terminal, браузер, файлы, OBS.
2. OAuth 2 (PKCE, loopback-редирект) для Twitch, Google Calendar и Spotify.
3. События коннекторов как триггеры автоматизаций.
