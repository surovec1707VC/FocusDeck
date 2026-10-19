# AI Router

## Backend (`src-tauri/src/ai`)
| Файл | Что внутри |
|---|---|
| `providers/mod.rs` | трейт `AiProvider` (`stream_chat`, `chat`, `list_models`, `test_connection`), общие типы, `AiError`, NDJSON-парсер, тесты стриминга на mock-сервере |
| `providers/ollama.rs` | `http://localhost:11434`, `/api/chat` (NDJSON), `/api/tags`, `/api/version`; ключ не нужен |
| `providers/openai_compatible.rs` | `{base_url}/chat/completions` (SSE через `eventsource-stream`), `/models`; ключ необязателен (Bearer) |
| `providers/anthropic.rs` | Messages API по raw HTTP (официального Rust SDK нет), SSE, `/v1/models` |
| `router.rs` | `AiRouter`: собирает провайдер, берёт ключ из SecretStore, хранит конфиг в `settings` (`ai.router`), выбирает провайдера и модель (`resolve_target`) |
| `profiles.rs` | 6 профилей: fast_assistant, coding_assistant, prompt_improver, error_explainer, task_planner, local_private_assistant |
| `session.rs` | сессии и сообщения в `ai_sessions` / `ai_messages` |
| `skills.rs` | заготовка под раздел 6 |
| `mod.rs` | Tauri-команды |

`src-tauri/src/secrets.rs` — `SecretStore` на `keyring` (Windows Credential Manager, сервис `com.focusdeck.app`). Ключи не попадают в файлы, SQLite или фронтенд: UI только сохраняет, удаляет и проверяет наличие ключа.

### Anthropic
- Модель по умолчанию — `claude-opus-5-5`; глубина рассуждений задаётся через `output_config.effort` в профиле (low / medium / high).
- Включён серверный fallback при отказе модели: заголовок `anthropic-beta: server-side-fallback-2026-07-01` + `fallbacks: "default"`. Если модель отказывает по соображениям безопасности, API автоматически повторяет запрос на запасной модели. Чтобы отключить, уберите это поле и заголовок в `providers/anthropic.rs`.
- `stop_reason: "refusal"` и `"max_tokens"` показываются в UI отдельным уведомлением.

### Несколько OpenAI-совместимых сервисов и провайдер по умолчанию
- В конфиге хранится список `openai_endpoints` (`id`, `name`, `base_url`, `default_model`) и `active_openai_endpoint`: он используется всякий раз, когда выбран провайдер `openai_compatible`.
- У каждого endpoint свой ключ в keyring: `ai.openai_compatible.<id>.api_key`. Endpoint `default` сохраняет старый ключ `ai.openai_compatible.api_key`, поэтому прежние настройки и ключ переносятся автоматически. При удалении endpoint его ключ удаляется.
- `default_provider` (`null` / `anthropic` / `openai_compatible` / `ollama`) переопределяет провайдера всех профилей, кроме `local_private_assistant` (`pinned_provider: true`, всегда Ollama).
- Выбор модели: явный выбор в UI → модель профиля (если провайдер совпадает с профилем) → `default_model` активного endpoint / `ollama_default_model` / `claude-opus-5-5`. Если модели нет, выдаётся понятная ошибка.
- В UI есть пресеты base URL: Gemini, Groq, OpenRouter, Mistral, OpenAI, LM Studio. Модели не предзаданы — они выбираются из живого списка (`Load models`).
- Ограничение: сессия чата хранит только тип провайдера, поэтому при продолжении старого чата используется текущий активный endpoint.
- Команда `ai_resolve_target(profile_id)` — какой провайдер и какая модель будут использованы сейчас.

### Стриминг
`ai_chat_stream` принимает `tauri::ipc::Channel<StreamEvent>` и отправляет события `started` → `delta`… → `done` | `error`. Ответ пользователя и ассистента сохраняются в БД (частичный ответ тоже, если поток оборвался).

### Команды
`ai_list_profiles`, `ai_get_config`, `ai_set_config`, `ai_list_models`, `ai_test_connection`, `ai_set_api_key`, `ai_has_api_key`, `ai_delete_api_key`, `ai_list_sessions`, `ai_get_messages`, `ai_delete_session`, `ai_chat_stream`.

## Frontend
- `src/pages/AiAssistant.tsx` — страница чата (стартовая страница приложения).
- `src/components/ai/` — `ChatWindow`, `ModelSelector` (провайдер + модель с подсказками из `list_models`), `ProfileSelector`, `MessageInput` (Enter — отправить, Shift+Enter — перенос), `SessionHistory`, `ProviderSettings` (URL-ы, API-ключи, тест подключения).
- `src/lib/ai.ts` — типы и обёртки `invoke`/`Channel`.

## Проверено
- `cargo test`: тесты стриминга Ollama (NDJSON), OpenAI-compatible (SSE) и Anthropic (SSE, включая пропуск thinking-дельт) на локальном mock-сервере.
- Приложение запускается, страница отображается.
- **Не проверено:** реальные запросы к провайдерам — нет API-ключей и не установлен Ollama.
