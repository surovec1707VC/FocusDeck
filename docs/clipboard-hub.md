# Clipboard Hub — API

Модуль `clipboard_history` («Clipboard Hub», категория «Буфер обмена»). В него входят история буфера обмена, Private Mode, быстрая вставка и быстрые действия. Весь буфер хранится только в локальной SQLite. Код — `src-tauri/src/clipboard_hub/`, страница — `src/pages/ClipboardHub.tsx` (меню «Буфер обмена»).

## Полное отключение

Когда модуль выключен в Центре управления:
- фоновая задача записи останавливается;
- все команды ниже возвращают `module_disabled`;
- горячие клавиши модуля (открытие Hub, быстрые действия) снимаются;
- страница и виджет «Буфер обмена» скрываются.

История и быстрые действия при этом сохраняются. Для записи нужно ещё разрешение `clipboard.read` (по умолчанию выключено).

## Схема БД

`src-tauri/src/db/migrations/0005_clipboard_hub.sql`.

`clipboard_history` (дополнена):

| Колонка | Что хранит |
|---|---|
| `kind` | `text`, `link`, `files`, `image`, `html`, `rtf` |
| `content` | читаемый текст: сам текст, список файлов или «Изображение W×H»; по нему идёт поиск |
| `text`, `html` (CF_HTML как есть), `rtf` | все текстовые форматы, которые были в буфере |
| `image` | изображение в формате BMP |
| `image_width`, `image_height` | размеры изображения |
| `files_json` | пути к файлам (CF_HDROP) |
| `source_app` | exe программы-источника |
| `hash` | FNV-1a от типа, текста, файлов и изображения: повторное копирование не создаёт дубль |
| `size` | размер всех форматов в байтах |
| `pinned`, `created_at`, `last_used_at` | |

`quick_actions(id, name, kind, target, hotkey, position, last_run_at, created_at, updated_at)`. Типы действий: `open_app`, `open_url`, `open_folder`, `open_file`, `paste_text`, `copy_text`, `command`.

## Что записывается и что нет

Фоновая задача раз в секунду сравнивает номер изменения буфера Windows. Сохраняется только то, что скопировано во время работы FocusDeck. Запись пропускается, если:
- включён **Private Mode**: на N минут (`private_minutes`, по умолчанию 15) или до выключения; режим не переживает перезапуск;
- программа-источник есть в `excluded_apps` (по умолчанию KeePass, KeePassXC, 1Password, Bitwarden);
- активное окно — приватное окно браузера (InPrivate, Incognito/Инкогнито, Private Browsing, «Приватное окно»), если включено `exclude_private_windows`;
- содержимое помечено менеджером паролей как скрытое (`ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory = 0`, `Clipboard Viewer Ignore`), если включено `skip_sensitive`;
- изображение больше `max_image_mb` или выключено `save_images`;
- текст длиннее `max_length`.

Если такое же содержимое уже есть, запись поднимается наверх; если оно закреплено, дубль не создаётся. Лишние незакреплённые записи сверх `max_items` удаляются. Записи старше `retention_days` удаляются раз в час и при запуске. При `persist_history = false` незакреплённая история очищается при запуске и выходе.

HTML и RTF декодируются как UTF-8, а при ошибке — в кодировке ANSI (так их записывают некоторые программы). В интерфейсе HTML и RTF показываются только как текст и никогда не отображаются как разметка.

## Команды: история

| Команда | Что делает |
|---|---|
| `clipboard_list(filter?)` | `filter = { search?, kind?, pinned_only?, limit?, offset? }` → `{ items, total }`; закреплённые первыми. Поиск — по тексту и программе-источнику |
| `clipboard_details(id)` | `{ text, html_text, rtf, files }` для подробного просмотра |
| `clipboard_image(id)` | `data:image/bmp;base64,…`; загружается только при раскрытии записи |
| `clipboard_copy(id)` | возвращает в буфер все сохранённые форматы (текст, HTML, RTF, изображение, файлы) |
| `clipboard_paste(id)` | то же, затем прячет окно и нажимает Ctrl+V в предыдущей программе. Нужно разрешение `system.paste` |
| `clipboard_pin(id, pinned)` | закрепить или открепить |
| `clipboard_delete(id)` | удалить запись |
| `clipboard_clear(include_pinned?)` | очистить историю (по умолчанию закреплённые остаются) |
| `clipboard_status()` | `{ recording, permission, private_mode: { active, until }, stats: { total, pinned, bytes, by_kind } }` |
| `clipboard_set_private(enabled, minutes?)` | включить Private Mode (0 или пусто — до выключения) или выключить |

События: `focusdeck://clipboard-changed` (новая запись или очистка), `focusdeck://clipboard-private`.

Клавиатура на странице: ↑/↓ — выбор, Enter — копировать, Shift+Enter — вставить, Delete — удалить.

## Команды: быстрые действия

| Команда | Что делает |
|---|---|
| `quick_actions_list()` | список действий |
| `quick_actions_save(id?, input)` | `input = { name, kind, target, hotkey? }`; сочетание, которое не удалось зарегистрировать, не сохраняется (ошибка) |
| `quick_actions_delete(id)` | удалить действие |
| `quick_actions_reorder(id, delta)` | переместить выше или ниже |
| `quick_actions_run(id, confirm?)` | запустить; результат `{ output }` (только для команд) |
| `quick_actions_pick("app" \| "file" \| "folder")` | диалог выбора пути; ничего не запускает |

| Тип | Проверки при запуске |
|---|---|
| `open_app` | `.exe`, `.lnk`, `.url`, `.appref-ms`; файл существует; разрешение `system.launch_apps` |
| `open_url` | только `http`, `https`, `mailto` |
| `open_folder` / `open_file` | путь существует; исполняемые файлы (`.exe`, `.bat`, `.ps1`, `.lnk` …) требуют `system.launch_apps` |
| `paste_text` | разрешение `system.paste` |
| `copy_text` | — |
| `command` | без `confirm: true` возвращается `confirmation_required`: UI каждый раз показывает команду и спрашивает. Нужно разрешение `system.run_commands` (по умолчанию выключено). Выполняется `cmd /D /C` в скрытом окне. Вывод (до 64 КБ, UTF-8 или OEM-кодировка) возвращается в UI; по истечении `command_timeout_secs` процесс завершается. Горячую клавишу команде назначить нельзя |

## Горячие клавиши

- `open_hotkey` (по умолчанию `Ctrl+Alt+V`) показывает окно на странице Clipboard Hub.
- Горячие клавиши быстрых действий (кроме команд) выполняют действие; ошибка приходит уведомлением.
- Работают, когда включены настройка «Глобальные горячие клавиши», модуль «Глобальные горячие клавиши», разрешение и сам Clipboard Hub.

## Настройки модуля

| Ключ | По умолчанию |
|---|---|
| `max_items` | 500 |
| `retention_days` | 30 (0 — без ограничения) |
| `persist_history` | вкл. |
| `save_images` | вкл. |
| `max_image_mb` | 10 |
| `save_rich` (HTML, RTF) | вкл. |
| `max_length` | 100 000 |
| `excluded_apps` | keepass.exe, keepassxc.exe, 1password.exe, bitwarden.exe |
| `exclude_private_windows` | вкл. |
| `skip_sensitive` | вкл. |
| `open_hotkey` | Ctrl+Alt+V |
| `private_minutes` | 15 |
| `command_timeout_secs` | 60 |

## Разрешения

| id | По умолчанию | Для чего |
|---|---|---|
| `clipboard.read` | нет | запись истории |
| `system.paste` | нет | «Вставить» (Ctrl+V в другой программе) |
| `system.launch_apps` | нет | открыть программу |
| `system.run_commands` | нет | команды терминала |

## Тесты

`cargo test --lib clipboard` покрывает:
- DIB → BMP, текст из CF_HTML, распознавание приватных окон;
- определение типа, дедупликацию, фильтры, поиск, закрепление, очистку, возврат всех форматов;
- исключённые программы, Private Mode с таймером;
- проверку быстрых действий, порядок, вывод `cmd` в кириллице и таймаут.
