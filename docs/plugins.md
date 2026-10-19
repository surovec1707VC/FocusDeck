# Tauri plugins

Установлены через `npx tauri add <name>` (Rust crate + npm-пакет `@tauri-apps/plugin-<name>`).
Регистрация: `src-tauri/src/plugins/mod.rs` (`plugins::register`, вызывается из `src-tauri/src/lib.rs`).
Permissions: `src-tauri/capabilities/default.json` и `desktop.json` (только для desktop).

| Плагин | Версия | Назначение | Permissions | Статус |
|---|---|---|---|---|
| shell | 2.5.0 | запуск процессов/sidecar | `shell:default` | installed |
| fs | 2.7.1 | файловая система | `fs:default` | installed |
| http | 2.8.3 | HTTP-клиент из фронтенда | `http:default` + scope: localhost:11434, api.openai.com | installed |
| clipboard-manager | 2.5.0 | буфер обмена | `clipboard-manager:default`, `allow-read-text`, `allow-write-text` | installed |
| global-shortcut | 2.4.1 | глобальные хоткеи (push-to-talk и т.п.) | `global-shortcut:default` (desktop.json) | installed |
| notification | 2.7.0 | системные уведомления | `notification:default` | installed |
| dialog | 2.8.3 | диалоги открытия/сохранения | `dialog:default` | installed |
| window-state | 2.5.0 | сохранение позиции/размера окна | `window-state:default` (desktop.json) | installed |
| autostart | 2.7.0 | автозапуск с Windows | `autostart:default` (desktop.json) | installed |
| store | 2.5.0 | key-value настройки | `store:default` | installed |
| log | 2.10.1 | логирование (уровень Info) | `log:default` | installed |
| opener | 2.7.2 | открытие URL/файлов | `opener:default` | installed |
| updater | — | — | — | skipped (по ТЗ) |

Примечание: `default` для fs/shell очень ограничен (fs — только app-директории, shell — только open). Расширять по мере появления модулей.
