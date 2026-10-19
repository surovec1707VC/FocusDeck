# Setup report

## Выполнено
- Git 2.55.0 и Rustup 1.29.1 (stable rustc 1.99.1) установлены через winget.
- `git init -b main`, git user задан локально в репозитории.
- Каркас Tauri 2 + React + TypeScript (create-tauri-app 4.7.5, identifier `com.focusdeck.app`), `npm install` — ok, tauri-cli 2.12.3 локально.
- VS 2022 Build Tools (MSVC 14.44) установлены в `D:\BuildTools`.
- Windows SDK 10.0.26100 установлен через winget (`Microsoft.WindowsSDK.10.0.26100`).
- `cargo check` в `src-tauri` — ✅ успешно.

## История проблем (решены)
1. Установка Build Tools на C: — ERROR_DISK_FULL (0x80070070).
2. Установка в `D:\BuildTools` — код 1603: MSVC встал, Windows SDK нет (LNK1181 `kernel32.lib`).
3. `setup.exe modify --add ...Windows11SDK.26100` — код 87; SDK поставлен отдельным пакетом winget.

## Нерешённые проблемы
- Ollama не установлен (опционально): `winget install Ollama.Ollama`.
