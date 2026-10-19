# Connectors

Коннектор — JSON-манифест в `connectors/`. Пока это **заготовки**: есть манифесты, настройки, хранение ключей, проверка подключения и логи. Действия (`actions`) только описаны, их реализации ещё нет.

## Манифест
| Поле | Описание |
|---|---|
| `id` | kebab-case, совпадает с именем файла |
| `name`, `description`, `category` | для UI |
| `enabled` | значение по умолчанию; текущее хранится в таблице `connectors` |
| `auth_type` | `none` / `api_key` / `oauth` / `local` |
| `secret_ref` | *(необязательно)* запись в keyring вместо `connector.<id>.secret`, чтобы делить ключ с другой функцией |
| `permissions` | нужные права |
| `actions` | `{ id, description, mutating }`; `mutating: true` — нужно подтверждение пользователя |
| `events` | `{ id, description }` |
| `settings_schema` | та же схема, что у модулей (`properties` + `default`) |
| `health_check` | `http` (`{setting}` в URL), `executable` (программа есть в PATH, ничего не запускается), `credentials` (ключ сохранён), `ai_provider` (тест провайдера AI Router), `none` |
| `docs_url` | ссылка https |

Неизвестные поля запрещены.

## Коннекторы (15)
| id | auth | health check | по умолчанию |
|---|---|---|---|
| ollama | local | GET `{base_url}/api/version` | вкл |
| openai-compatible | none (ключи — у endpoint-ов в AI Assistant) | ai_provider (активный endpoint) | вкл |
| git | local | `git` в PATH | выкл |
| github | api_key (PAT) | credentials | выкл |
| vscode | local | `code` в PATH | выкл |
| windows-terminal | local | `wt` в PATH | выкл |
| powershell | local | `powershell` в PATH | выкл |
| browser | local | — | выкл |
| clipboard | local | — | выкл |
| obs | api_key (пароль obs-websocket) | credentials | выкл |
| twitch | oauth | credentials | выкл |
| discord | api_key (webhook URL) | credentials | выкл |
| telegram | api_key (bot token) | credentials | выкл |
| notion | api_key (integration token) | credentials | выкл |
| google-calendar | oauth | credentials | выкл |

Проверка `credentials` только убеждается, что ключ сохранён; сам сервис не вызывается. OAuth-входа ещё нет, вместо него можно вставить access token.

## Backend (`src-tauri/src/connectors`)
- `types.rs` — манифест, `AuthType`, `HealthCheck`, `ConnectorInfo`, статус.
- `manifest.rs` — загрузка `connectors/*.json` и валидация (id и имя файла, snake_case у action/event, https у `docs_url`, согласованность `auth_type` / `secret_ref` / `health_check`).
- `registry.rs` — `ConnectorRegistry`: синхронизация с таблицей `connectors`, вкл/выкл, настройки (переиспользует `modules::settings`), секреты в keyring + метаданные в `connector_credentials_metadata`, health check с сохранением `last_status`, логи в памяти.
- `mod.rs` — команды: `connectors_list`, `connectors_issues`, `connectors_reload`, `connectors_set_enabled`, `connectors_update_settings`, `connectors_reset_settings`, `connectors_set_secret`, `connectors_delete_secret`, `connectors_test`, `connectors_logs`.
- `connectors/*.json` входят в бандл через `bundle.resources`.

Ограничение: выключение коннекторов ollama / openai-compatible пока не блокирует AI Router. Связать их — отдельная задача.

## Frontend
- `src/pages/Connectors.tsx` — список, поиск, фильтр по категории, вкл/выкл, перезагрузка манифестов.
- `src/components/connectors/ConnectorCard.tsx` — карточка со статусом.
- `src/components/connectors/ConnectorSettings.tsx` — ключ или токен, настройки (общая форма `ModuleSettings`, теперь с полями-массивами), права, действия и события, тест подключения, логи, ссылка на документацию.

## Проверено
- `cargo test`: все 16 манифестов загружаются и проходят валидацию; тесты валидатора, подстановки `{setting}` и поиска программ в PATH.
- В приложении: список отображается; тест Git прошёл («found C:\Program Files\Git\cmd\git.exe») и записался в логи.
