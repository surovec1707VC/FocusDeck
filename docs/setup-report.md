# Setup report — FocusDeck

Дата: 2026-10-20 · Windows 11 Pro 10.0.26200 · ветка `main`, push не выполнялся.

## Финальная проверка
| Проверка | Результат |
|---|---|
| `npm install` | ✅ 0 уязвимостей |
| `npx tsc --noEmit` | ✅ |
| `npx vite build` | ✅ |
| `cargo check` | ✅ без предупреждений |
| `cargo test --lib` | ✅ 24 / 24 |
| `npm run tauri dev` | ✅ окно открывается, миграции применяются, все страницы загружаются |

Вручную в приложении проверено: Control Center, AI Assistant (без ключа), Skills (14 скиллов, тестовый запуск пишет лог), Connectors (тест Git прошёл), Voice (сохранение настроек, push-to-talk `Ctrl+Alt+M` доходит до UI).

## Окружение (установлено в этой сессии)
- Git 2.55.0, Rustup 1.29.1 / rustc 1.99.1 — winget.
- VS 2022 Build Tools (MSVC 14.44) — `D:\BuildTools` (на C: не хватало места).
- Windows SDK 10.0.26100 — winget.
- Tauri CLI 2.12.3 — локально в проекте (devDependency).

История проблем (все решены):
1. Build Tools на C: — ERROR_DISK_FULL (0x80070070).
2. Build Tools в `D:\BuildTools` — код 1603: MSVC установился, Windows SDK нет (LNK1181 `kernel32.lib`).
3. `setup.exe modify --add …Windows11SDK.26100` — код 87; SDK поставлен отдельным пакетом winget.

## Установленные Tauri-плагины
shell, fs, http, clipboard-manager, global-shortcut, notification, dialog, window-state, autostart, store, log, opener. updater пропущен по ТЗ. Подробно: [plugins.md](plugins.md).

## Установленные Rust-крейты
tokio, serde, serde_json, sqlx (sqlite, migrate), reqwest 0.13, eventsource-stream, futures-util, async-trait, tracing, tracing-subscriber, thiserror, anyhow, uuid, chrono, regex, git2, keyring 4, notify, sysinfo, windows, enigo, arboard. `reqwest-eventsource` заменён на `eventsource-stream` (тянул reqwest 0.12). Подробно: [rust-dependencies.md](rust-dependencies.md).

## Созданные модули
| Подсистема | Код | Документация |
|---|---|---|
| SQLite (23 таблицы, миграции при старте) | `src-tauri/src/db` | [modules.md](modules.md) |
| Module Registry + Control Center (9 модулей) | `src-tauri/src/modules`, `src/pages/ControlCenter.tsx` | [modules.md](modules.md) |
| AI Router: Ollama / OpenAI-compatible / Anthropic, профили, сессии, стриминг | `src-tauri/src/ai` | [ai-router.md](ai-router.md) |
| SecretStore (Windows Credential Manager) | `src-tauri/src/secrets.rs` | [ai-router.md](ai-router.md) |
| Skills (loader / validator / executor, только планы) | `src-tauri/src/ai/skills*`, `skills/` | [skills.md](skills.md) |
| Connectors (манифесты, ключи, health check) | `src-tauri/src/connectors`, `connectors/` | [connectors.md](connectors.md) |
| Voice (настройки, push-to-talk, VAD, intents) | `src-tauri/src/voice` | [voice-setup.md](voice-setup.md) |

## Созданные skills (14)
project-context-builder, prompt-improver, task-planner, code-reviewer, error-explainer, test-generator, refactor-planner, automation-designer, workflow-recovery, daily-summary, security-guard, voice-intent-router, widget-advisor, knowledge-recall.

## Созданные connectors (16)
ollama, openai-compatible, anthropic, git, github, vscode, windows-terminal, powershell, browser, clipboard, obs, twitch, discord, telegram, notion, google-calendar.

## Нерешённые проблемы и ограничения
1. **Реальные AI-запросы не проверены**: нет API-ключей, Ollama не установлен. Стриминг всех трёх провайдеров покрыт тестами на mock-сервере.
2. **Инструменты скиллов не исполняются**: executor возвращает проверенный план, но исполнителя с подтверждением пользователя ещё нет.
3. **Действия коннекторов не реализованы** — только манифесты, настройки, ключи и health check. OAuth-входа нет, токен вставляется вручную.
4. **Выключение коннекторов ollama / anthropic / openai-compatible не блокирует AI Router.**
5. **Голос**: нет захвата микрофона (cpal/WASAPI) и вызова STT/TTS; есть настройки, хоткей, VAD и маршрутизация intents.
6. **`Ctrl+Alt+Space` занят в системе** (похоже, быстрый вызов Claude Desktop) — default push-to-talk изменён на `Ctrl+Alt+M`.
7. В Cargo.lock две версии reqwest (0.12 от tauri-plugin-http и 0.13 у нас): работает, но увеличивает сборку.
8. Свободного места на C: мало — следите за этим при установке моделей (рекомендуется `D:\FocusDeck\voice\`).

## Команды для ручной установки (по желанию)
```powershell
winget install Ollama.Ollama          # локальные модели
ollama pull llama3.2                  # модель профиля local_private_assistant (~2 GB)
```
Голосовые движки (whisper.cpp, faster-whisper, Piper) ставятся вручную — см. [voice-setup.md](voice-setup.md).

## Следующие шаги
1. Добавить API-ключ Anthropic и проверить чат и тестовые запуски скиллов на реальной модели.
2. Исполнитель инструментов скиллов: подтверждение пользователя → вызов → запись в `permission_logs`.
3. Связать включение коннекторов и модулей с AI Router.
4. Захват микрофона (cpal) → VAD → whisper.cpp → `voice-intent-router`.
5. Реализовать первые действия коннекторов: git (status / diff / log), clipboard, browser.
6. Настроить `cargo tauri build` и подпись инсталлятора (updater — позже).
