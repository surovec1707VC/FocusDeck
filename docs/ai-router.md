# AI Router

## Backend (`src-tauri/src/ai`)
| Файл | Что внутри |
|---|---|
| `providers/mod.rs` | трейт `AiProvider` (`stream_chat`, `chat`, `list_models`, `test_connection`), общие типы, `AiError`, NDJSON-парсер, тесты стриминга на mock-сервере |
| `providers/ollama.rs` | `http://localhost:11434`, `/api/chat` (NDJSON), `/api/tags`, `/api/version`; ключ не нужен |
| `providers/openai_compatible.rs` | `{base_url}/chat/completions` (SSE через `eventsource-stream`), `/models`; ключ необязателен (Bearer) |
| `providers/anthropic.rs` | Messages API по raw HTTP (официального Rust SDK нет), SSE, `/v1/models` |
| `router.rs` | `AiRouter`: собирает провайдер, берёт ключ из SecretStore, хранит URL-ы в `settings` (`ai.router`) |
| `profiles.rs` | 6 профилей: fast_assistant, coding_assistant, prompt_improver, error_explainer, task_planner, local_private_assistant |
| `session.rs` | сессии и сообщения в `ai_sessions` / `ai_messages` |
| `skills.rs` | заготовка под раздел 6 |
| `mod.rs` | Tauri-команды |

`src-tauri/src/secrets.rs` — `SecretStore` на `keyring` (Windows Credential Manager, сервис `com.focusdeck.app`). Ключи не попадают в файлы, SQLite или фронтенд: UI только сохраняет, удаляет и проверяет наличие ключа.

### Anthropic
- Модель по умолчанию — `claude-opus-5-5`; глубина рассуждений задаётся через `output_config.effort` в профиле (low / medium / high).
- Включён серверный fallback при отказе модели: заголовок `anthropic-beta: server-side-fallback-2026-07-01` + `fallbacks: "default"`. Если модель отказывает по соображениям безопасности, API автоматически повторяет запрос на запасной модели. Чтобы отключить, уберите это поле и заголовок в `providers/anthropic.rs`.
- `stop_reason: "refusal"` и `"max_tokens"` показываются в UI отдельным уведомлением.

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
