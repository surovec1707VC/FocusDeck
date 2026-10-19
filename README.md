# FocusDeck

Desktop-приложение для Windows 10/11 на Tauri 2 + React + TypeScript: AI-ассистент, скиллы, коннекторы, голосовое управление и модульный Control Center.

## Quick Start

### Требования
- Windows 10/11, Node.js 20+ и npm
- Rust (stable, через rustup) и Visual Studio Build Tools с компонентом «Desktop development with C++» и Windows SDK
- WebView2 Runtime (в Windows 11 уже есть)

### Запуск
```powershell
npm install
npm run tauri dev
```
Первая сборка Rust занимает несколько минут. Проверки:
```powershell
npx tsc --noEmit
cd src-tauri; cargo test --lib
```
База данных: `%APPDATA%\com.focusdeck.app\focusdeck.db` (создаётся и мигрирует при старте).

### Установленные плагины Tauri
shell, fs, http, clipboard-manager, global-shortcut, notification, dialog, window-state, autostart, store, log, opener. Регистрация — `src-tauri/src/plugins/mod.rs`, права — `src-tauri/capabilities/`. Подробнее: [docs/plugins.md](docs/plugins.md).

### Как включить Ollama
1. Установите Ollama: `winget install Ollama.Ollama`.
2. Скачайте модель: `ollama pull llama3.2`.
3. В FocusDeck: **AI Assistant** → профиль **Local Private Assistant** (или провайдер *Ollama (local)*). Адрес сервера и «Test connection» — в блоке **Providers & API keys**, по умолчанию `http://localhost:11434`.

### Как добавить API key
**AI Assistant** → **Providers & API keys** → вставьте ключ Anthropic или OpenAI-compatible → **Save key** → **Test connection**. Ключи коннекторов (GitHub, Telegram, Notion…) задаются так же: **Connectors** → коннектор → **Settings**.

Ключи хранятся в Windows Credential Manager (сервис `com.focusdeck.app`), не в файлах и не в SQLite, и не возвращаются в интерфейс. Ключи Anthropic и OpenAI-compatible общие для AI Assistant и одноимённых коннекторов.

### Как включить голосовое управление
1. Установите движок распознавания вручную по [docs/voice-setup.md](docs/voice-setup.md) (рекомендуется whisper.cpp). FocusDeck ничего не скачивает сам.
2. **Voice** → укажите пути к `whisper-cli.exe` и модели → включите **Voice enabled** → **Save**.
3. Удерживайте **Ctrl+Alt+M** (push-to-talk). Сочетание меняется там же; занятое другой программой не сохранится.

Сейчас работают настройки, хоткей и маршрутизация команд (блок «Intent test»). Захват микрофона и распознавание — следующий этап.

### Как отключить любой модуль
- **Модули**: **Control Center** → переключатель у модуля. Состояние сохраняется в БД. Там же настройки, статус, логи и «Reset to defaults».
- **Скиллы**: **Skills** → переключатель. Выключенный скилл не запускается. Если выключить модуль *Skills*, не запустится ни один скилл.
- **Коннекторы**: **Connectors** → переключатель.
- **Голос**: **Voice** → снять **Voice enabled** (хоткей снимается сразу).

## Структура
| Путь | Что там |
|---|---|
| `src-tauri/src/db` | SQLite, миграции |
| `src-tauri/src/modules` | Module Registry |
| `src-tauri/src/ai` | AI Router, провайдеры, профили, сессии, skills |
| `src-tauri/src/connectors` | реестр коннекторов |
| `src-tauri/src/voice` | голосовой контур |
| `src-tauri/src/secrets.rs` | SecretStore (keyring) |
| `skills/`, `connectors/` | JSON-манифесты (входят в бандл) |
| `src/pages`, `src/components` | интерфейс |
| `docs/` | документация и [отчёт об установке](docs/setup-report.md) |

## Recommended IDE Setup
- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
