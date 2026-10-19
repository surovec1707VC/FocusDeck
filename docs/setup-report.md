# Setup report

## Выполнено
- Git 2.55.0 и Rustup 1.29.1 (stable rustc 1.99.1) установлены через winget.
- `git init -b main`.
- Каркас Tauri 2 + React + TypeScript (create-tauri-app 4.7.5, identifier `com.focusdeck.app`), `npm install` — ok, tauri-cli 2.12.3 локально.

## Нерешённые проблемы
1. **Нет MSVC-линкера (`link.exe`)** — `cargo check` падает. Visual Studio Installer есть, но workload C++ не установлен.
   Решение (требует подтверждения, ~2–6 ГБ):
   ```
   winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
   ```
2. **Не задан git user.name / user.email** — коммиты невозможны до настройки:
   ```
   git config user.name "Ваше имя"
   git config user.email "you@example.com"
   ```
3. Ollama не установлен (опционально): `winget install Ollama.Ollama`.
