# Rust dependencies (`src-tauri/Cargo.toml`)

Все крейты добавлены через `cargo add`, `cargo check` проходит (rustc 1.99.1, MSVC).

| Crate | Версия | Назначение | Статус | Риски |
|---|---|---|---|---|
| tokio (full) | 1.53.3 | async runtime | installed | full увеличивает время сборки; позже можно сузить features |
| serde (derive) | 1.0.x | сериализация | installed | — |
| serde_json | 1.0.x | JSON | installed | — |
| sqlx (runtime-tokio, sqlite, chrono, uuid, json, migrate, macros) | 0.9.0 | SQLite + миграции | installed | макросы `query!` требуют `DATABASE_URL` или `.sqlx` offline-кеш — используем `query`/`query_as` без макросов |
| reqwest (json, stream) | 0.13.5 | HTTP к AI-провайдерам | installed | — |
| eventsource-stream | 0.2.3 | SSE-парсинг поверх `reqwest::Response::bytes_stream()` (reqwest 0.13) | installed | — |
| futures-util | 0.3.34 | `StreamExt` для стримов | installed | — |
| ~~reqwest-eventsource~~ | — | — | removed | тянул reqwest 0.12. Замечание: reqwest 0.12 всё равно остаётся в дереве через `tauri-plugin-http` — это нормально, типы не пересекаются |
| tracing | 0.1.44 | логирование | installed | — |
| tracing-subscriber (env-filter) | 0.3.23 | вывод логов | installed | может конфликтовать с `tauri-plugin-log` (оба ставят глобальный logger) — инициализировать только один |
| thiserror | 2.0.21 | типизированные ошибки | installed | — |
| anyhow | 1.0.104 | ошибки в прикладном коде | installed | — |
| uuid (v4, serde) | 1.28.0 | идентификаторы | installed | — |
| chrono (serde) | 0.4.45 | время | installed | — |
| regex | 1.13.1 | парсинг | installed | — |
| git2 | 0.21.0 | Git-коннектор | installed | собирает libgit2 из C-исходников (долгая первая сборка, нужен MSVC) |
| keyring | 4.2.0 | SecretStore (Windows Credential Manager) | installed | v4 — новый API относительно v3; бэкенд Windows включён по умолчанию |
| notify | 8.2.0 | слежение за файлами | installed | — |
| sysinfo | 0.39.6 | системные метрики | installed | API часто меняется между минорными версиями |
| windows | 0.62.2 | WinAPI | installed | без features почти пуст — добавлять `Win32_*` features по мере надобности |
| enigo | 0.6.1 | эмуляция ввода | installed | действия от имени пользователя — только через permissions + подтверждение |
| arboard | 3.7.0 | буфер обмена из Rust | installed | дублирует `tauri-plugin-clipboard-manager`; оставлен для фоновой истории буфера |
