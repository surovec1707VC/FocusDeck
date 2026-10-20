# Developer Tools

Шесть инструментов, каждый — отдельный модуль реестра (категория «Инструменты разработчика»). Все они включаются и отключаются в Центре управления. В интерфейсе это одна страница «Разработка» (`/dev/:tool`) с вкладками, но вкладка видна, только пока включён её модуль. Пункт меню скрывается, если отключены все шесть.

| Модуль | Инструмент | По умолчанию | Разрешения |
|---|---|---|---|
| `dev_git` | Git | выкл. | `dev.git` — для stage/commit/push/pull |
| `dev_projects` | Project Launcher | выкл. | `system.launch_apps` (открыть), `system.run_commands` (команды проекта) |
| `dev_terminal` | Terminal Quick Launcher | выкл. | `system.launch_apps` (окно), `system.run_commands` (вкладки) |
| `dev_api` | API Tester | выкл. | `network.api_tester` |
| `dev_regex` | Regex Tester | вкл. | — |
| `dev_json` | JSON/YAML Tools | вкл. | — |

Если модуль отключён, все его команды возвращают `module_disabled`, вкладка и (для Git) виджет скрываются. Данные — проекты, история, коллекции — сохраняются.

## Архитектура

```
UI  src/pages/DevTools.tsx  →  pages/dev/{Git,Projects,Terminal,Api,Regex,Json}Tool.tsx
     │ invoke(...)               useGuarded: confirmation_required → диалог → повтор с confirm=true
     ▼
src-tauri/src/devtools/
  mod.rs        require(module), guarded(): authorize → действие → Action Log;
                run_program() — процесс со списком аргументов, скрытое окно, таймаут, kill_tree();
                Decoder — UTF-8 с переносом разрезанных символов, иначе OEM-кодировка;
                spawn_console() — отдельное окно консоли
  git.rs        git с аргументами (без shell), GIT_TERMINAL_PROMPT=0, разбор porcelain v1 -z
  projects.rs   определение типа и команд по файлам, хранение, запуск через terminal::open_window
  terminal.rs   оболочки, окна, команды во вкладках с потоковым выводом, cd, история, избранное
  api.rs        reqwest, {{переменные}}, история, curl (bash / cmd / PowerShell)
  regex.rs      сохранённые выражения (проверка совпадений — в UI, Web Worker)
  jsonyaml.rs   serde_json (с сохранением порядка ключей) + serde_norway (YAML), свой JSONPath
```

**Безопасность**
- Фронтенду shell не выдаётся. Все процессы запускает Rust с явными аргументами.
- Пути внутри репозитория проверяются: только относительные, без `..` и без имён, начинающихся с `-`. Хеши коммитов — только hex.
- Действия с последствиями проходят `safety::authorize`: разрешение → Safe Mode → подтверждение. Каждое попадает в Action Log.

| Действие | Риск | Разрешение |
|---|---|---|
| `git.stage`, `git.commit` | изменение | `dev.git` |
| `git.pull`, `git.push` | **опасное** — подтверждается всегда | `dev.git` |
| `projects.open` | изменение | `system.launch_apps` |
| `projects.run` | изменение | `system.run_commands` |
| `terminal.open` | изменение | `system.launch_apps` |
| `terminal.run` | изменение | `system.run_commands` |
| `api.send` | изменение (в журнал — адрес без query) | `network.api_tester` |

В Safe Mode все эти действия заблокированы, пока их не добавят в белый список. Добавленные в список выполняются только после подтверждения.

## Схема БД (миграция `0009_developer_tools.sql`)

| Таблица | Колонки | Назначение |
|---|---|---|
| `dev_git_repos` | path PK, name, last_opened_at | открытые репозитории; текущий — открытый последним |
| `dev_projects` | id, name, path UNIQUE, kind, commands_json, favorite, position, created_at, last_opened_at | Project Launcher |
| `terminal_history` | id, shell, cwd, command, exit_code, duration_ms, created_at | история команд (лимит в настройках) |
| `terminal_favorites` | id, name, shell, command, cwd, position, created_at | избранные команды |
| `api_collections` | id, name, position, created_at | коллекции |
| `api_requests` | id, collection_id → api_collections ON DELETE CASCADE, name, method, url, headers_json, query_json, body, body_type, position, created_at, updated_at | сохранённые запросы |
| `api_environments` | id, name, vars_json, active, created_at | окружения с переменными |
| `api_history` | id, method, url, status, duration_ms, size, error, request_json, created_at | история запросов (лимит в настройках) |
| `regex_saved` | id, name, pattern, flags, sample, replacement, created_at, updated_at | сохранённые выражения |

Формат элементов `headers_json`, `query_json` и `vars_json` — `[{ "name", "value", "enabled" }]`.

Формат элементов `commands_json` — `[{ "name", "command", "custom" }]`. При повторном определении типа проекта сохраняются только команды с `custom: true`.

## Команды

**Git**
- `git_repos()`, `git_open_repo(path)` — открытие делает репозиторий текущим;
- `git_forget_repo(path)` — убрать из списка;
- `git_status(repo)` — ветка, upstream, ahead/behind, staged, unstaged, untracked, конфликты;
- `git_log(repo, limit?)`;
- `git_diff(repo, { path?, staged?, untracked?, commit? })` — до 1 МБ;
- `git_stage(repo, paths[], stage, confirm?)` — пустой список означает все файлы. Убирает из индекса `git restore --staged`, а в репозитории без коммитов — `git rm --cached`;
- `git_commit(repo, message, confirm?)` — только если в индексе что-то есть;
- `git_pull(repo, confirm?)` — `git pull --ff-only`;
- `git_push(repo, confirm?)` — обычный `git push`. Если у ветки нет upstream, выполняется `push --set-upstream origin <ветка>`. `--force` не используется никогда.

**Project Launcher**
- `projects_list()`, `projects_add(path)`;
- `projects_scan(dir)` — показывает проекты в папке и на уровень ниже, сам ничего не добавляет;
- `projects_update(id, { name?, kind?, commands?, favorite? })`, `projects_redetect(id)`, `projects_remove(id)` — папку не трогает;
- `projects_open(id, editor|terminal|explorer, shell?, confirm?)`;
- `projects_run(id, index, shell?, confirm?)` — команда запускается в новом окне терминала, окно остаётся открытым.

Как определяется тип проекта:

| Файл в папке | Тип | Команды |
|---|---|---|
| `package.json` | Node | скрипты, менеджер пакетов по lock-файлу (npm, pnpm, yarn, bun) |
| `package.json` + `src-tauri` | Tauri | скрипты + `tauri dev` / `tauri build` |
| `Cargo.toml` | Rust | |
| `pyproject.toml`, `requirements.txt` | Python | |
| `*.sln`, `*.csproj` | .NET | |
| `go.mod` | Go | |
| `pom.xml`, `build.gradle` | Java | |
| `composer.json` | PHP | |
| `Gemfile` | Ruby | |

**Terminal Quick Launcher**
- `terminal_shells()` — PowerShell, CMD, Git Bash (ищется в Program Files, LOCALAPPDATA и рядом с git из PATH), WSL, свой shell из настроек;
- `terminal_open(shell, cwd, command?, confirm?)` — отдельное окно; Git Bash без команды открывается в mintty;
- `terminal_run(run_id, shell, cwd, command, confirm?)` — выполняет команду во вкладке:
  - вывод приходит событиями `focusdeck://terminal-output` `{ run_id, stream, text }`;
  - результат — `{ code, timed_out, cancelled, duration_ms, cwd }`;
  - `cd`, `cd /d`, `cd..`, `Set-Location`, а также `/c/…` и `/mnt/c/…` обрабатывает FocusDeck: каждая вкладка помнит свою папку;
- `terminal_cancel(run_id)` — завершает команду вместе со всеми дочерними процессами (`taskkill /T /F`); то же при таймауте;
- `terminal_history(query?, limit?)`, `terminal_history_clear()`;
- `terminal_favorites()`, `terminal_favorite_save(id?, input)`, `terminal_favorite_delete(id)`.

Вкладки не дают интерактивного ввода (stdin закрыт). Для ssh, REPL и редакторов нужен терминал в окне. Набор вкладок — оболочка и папка — запоминается в браузерном хранилище, вывод не сохраняется.

**API Tester**
- `api_send(request, confirm?)`:
  - `{{переменные}}` берутся из активного окружения;
  - разрешены только http/https;
  - таймаут, редиректы и лимит размера ответа задаются в настройках модуля;
  - в ответе: статус, заголовки, тело (или пометка, что данные двоичные), размер, время и `missing_vars` — переменные без значения;
- `api_to_curl(request, bash|cmd|powershell)`;
- `api_workspace()` — коллекции с запросами и запросы без коллекции;
- `api_collection_save/delete`, `api_request_save/delete`;
- `api_environments()`, `api_environment_save(id?, name, vars)`, `api_environment_delete`, `api_environment_activate(id|null)` — первое созданное окружение сразу становится активным;
- `api_history(limit?)`, `api_history_clear()`. Запросы, остановленные до отправки (нет разрешения, Safe Mode), в историю не попадают.

**Regex Tester**
- `regex_list()`, `regex_save(id?, input)`, `regex_delete(id)`; флаги проверяются: `dgimsuvy`, без повторов, `u` и `v` нельзя вместе.
- Совпадения считаются в `src/lib/regex-worker.ts` движком JavaScript:
  - группы с позициями (флаг `d`), именованные группы, замена;
  - если проверка дольше таймаута (по умолчанию 1 с), worker завершается — так катастрофический бэктрекинг не вешает окно.
- 12 встроенных шаблонов: email, URL, IPv4, даты, телефон, UUID, HEX-цвет, semver, conventional commits, пробелы в концах строк, пути Windows.

**JSON/YAML Tools**
- `devjson_process(text, op, from?)`:
  - `op`: `format`, `minify`, `validate`, `to_json`, `to_yaml`;
  - формат ввода определяется автоматически;
  - ошибка возвращается как `{ message, line, column }`, плюс статистика: объекты, массивы, значения, глубина;
  - порядок ключей сохраняется; сортировка и отступ задаются в настройках модуля;
- `devjson_query(text, path, from?)` — JSONPath:
  - поддерживаются `$`, `.key`, `['key']`, `[0]`, `[-1]`, `[*]`, `..key`, `[a,b]`, `[start:end:step]`, `[?(@.x == 1)]`, `[?(@.x)]`;
  - сравнения: `== != < <= > >=`;
  - результат — нормализованные пути с найденными значениями.

## Виджет Git

Тип виджета `git` принадлежит модулю `dev_git`. Он показывает текущий репозиторий: ветку, число изменений, ahead/behind и три последних коммита. Обновляется каждые 30 с и открывает страницу Git.

## Примеры

**Добавить свой тип проекта** — новая ветка в `projects::detect`:

```rust
if f("deno.json") {
    return ("deno".into(), vec![cmd("dev", "deno task dev"), cmd("test", "deno test")]);
}
// и "deno" в KINDS, подпись в i18n dev.projects.kinds
```

**Вызов с подтверждением из UI** (push всегда спрашивает):

```tsx
const { run, dialog } = useGuarded(setError);
<button onClick={() => run((c) => api.gitPush(repo, c), (r) => setStatus(r.status))}>Push</button>
{dialog}
```

**Переменные API Tester**

Окружение `dev`: `base = http://localhost:8080`, `token = test-token`.

Запрос `GET {{base}}/items` с заголовком `Authorization: Bearer {{token}}`, экспорт в bash:

```
curl \
  'http://localhost:8080/items' \
  -H 'Authorization: Bearer test-token'
```
