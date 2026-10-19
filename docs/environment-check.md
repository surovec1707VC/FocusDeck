# Environment check — 2026-10-19

| Проверка | Результат |
|---|---|
| ОС | Windows 11 Pro, 10.0.26200 |
| Node.js | v24.22.0 ✅ |
| npm | 11.19.1 ✅ |
| Rust (`rustc`) | ❌ не найден (нет `%USERPROFILE%\.cargo\bin`) |
| Cargo | ❌ не найден |
| Tauri CLI | ❌ не найден (будет установлен локально через npm) |
| Git | ❌ не найден |
| PowerShell | 5.1.26100 ✅ |
| Ollama | ❌ не найден |
| Visual Studio Installer | ✅ есть (наличие workload "Desktop development with C++" не проверено) |
| WebView2 Runtime | ✅ есть |
| winget | ✅ есть |
| crates.io (index.crates.io) | ✅ HTTP 200 |
| npm registry | ✅ HTTP 200 |

## Состояние репозитория

Папка `E:\ProjectFocusDeck` пустая, git-репозиторий не инициализирован.

## Блокеры

- Без Rust/Cargo невозможны разделы 1–4 (cargo add, cargo check, `tauri dev`).
- Без Git невозможны коммиты (правило 8).
- Ollama опционален — нужен только для локальной модели.
