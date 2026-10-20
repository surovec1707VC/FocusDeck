# Модули Productivity — API

Четыре отдельных модуля в Module Registry (категория «Продуктивность»): `notes`, `tasks`, `snippets`, `timers`. Каждый отключается в Центре управления; пока модуль выключен, его страница скрыта, а команды возвращают ошибку `module_disabled`. Все данные хранятся только в локальной SQLite. Код — `src-tauri/src/productivity/`, обёртки — `src/lib/api.ts`.

## Схема БД

`src-tauri/src/db/migrations/0004_productivity.sql`:

| Таблица | Изменения |
|---|---|
| `notes` | + `tags_json` (JSON-массив), `project`, `layout_id` → `widget_layouts` (`ON DELETE SET NULL`), `deleted_at` (корзина) |
| `tasks` | + `priority` 0–3, `status` (`todo`, `in_progress`, `done`, `cancelled`), `category` («Работа/Отчёты»), `recurrence` (`none`, `daily`, `weekdays`, `weekly`, `monthly`), `notes`, `completed_at`; старые `done = 1` → `status = 'done'` |
| `snippets` | + `tags_json`, `use_count` |
| `timers` | новая: `kind` (`countdown`, `pomodoro`), интервалы, `phase`, `round`, `running`, `ends_at`, `remaining_ms`, `phase_started_at` |
| `timer_sessions` | новая: история интервалов (`kind`, `planned_ms`, `actual_ms`, `completed`, `started_at`, `ended_at`) |
| `stopwatches` | новая: `elapsed_ms`, `started_at`, `laps_json` |
| `stopwatch_results` | новая: сохранённые результаты (`total_ms`, `laps_json`, `note`) |

Время таймеров и секундомеров хранится как метки времени, поэтому они идут и при закрытом окне, и после перезапуска.

## Notes (`notes`)

| Команда | Что делает |
|---|---|
| `notes_list(filter?)` | `filter = { search?, tag?, project?, layout_id?, trash? }`; закреплённые первыми. Попутно очищает корзину от заметок старше `trash_days` |
| `notes_facets()` | теги с количеством, проекты, число заметок в корзине |
| `notes_create(input)`, `notes_update(id, input)` | `input = { title?, body?, tags?, pinned?, project?, layout_id? }`; `layout_id: null` отвязывает от макета |
| `notes_trash(id)` / `notes_restore(id)` | в корзину и обратно. Заметку виджета в корзину не положить — удаляется вместе с виджетом |
| `notes_delete_forever(id)` | только для заметок в корзине |
| `notes_empty_trash()` | очистить корзину |
| `notes_export(id, "md" \| "txt")` | диалог «Сохранить как»; `.md` — исходный Markdown с заголовком и тегами, `.txt` — без разметки |

Markdown отображается своим компонентом `src/components/Markdown.tsx` (заголовки, списки, задачи `- [ ]`, цитаты, код, ссылки http/https/mailto). HTML не вставляется, поэтому внедрить разметку через заметку нельзя.

Настройки модуля: `trash_days` (30), `preview` (вкл.).

## Tasks (`tasks`)

| Команда | Что делает |
|---|---|
| `tasks_query(filter?)` | `filter = { status?: "open" \| "all" \| статус, priority? (минимум), category? (с подкатегориями), due?: "today" \| "overdue" \| "week" \| "none", search? }` |
| `tasks_create(input)`, `tasks_update(id, input)` | `input = { title?, status?, priority?, due_at?: "ГГГГ-ММ-ДД" \| null, category?, recurrence?, notes? }` |
| `tasks_quick_add(line)` | создать задачу из строки |
| `tasks_quick_parse(line)` | предпросмотр разбора (без сохранения) |
| `tasks_remove(id)` | |
| `tasks_stats()` | выполнено и создано сегодня, открытые, в работе, просроченные, на сегодня, открытые по приоритетам, выполненные за 7 дней, список категорий |

**Быстрое добавление:** `Отчёт завтра !2 #Работа/Отчёты *еженедельно`
- `!1`–`!3` или `!`, `!!`, `!!!` — приоритет;
- `#категория/подкатегория`;
- `сегодня`, `завтра`, `послезавтра`, `@пн`…`@вс`, `@2026-10-25`, `@25.10` — срок;
- `*ежедневно`, `*будни`, `*еженедельно`, `*ежемесячно` — повтор (без срока ставится «сегодня»).

**Повторяющиеся задачи:** при переходе в `done` создаётся следующая задача со сроком по правилу повтора (в прошлом не бывает), а повтор переносится на неё. Выполненная остаётся в истории.

«Выполнено сегодня» считается по местной дате (`completed_at` хранится в UTC). Виджет «Список дел» работает с теми же задачами; ввод в виджете тоже понимает синтаксис быстрого добавления.

Настройки модуля: `hide_completed`, `default_priority`, `week_starts`.

## Snippets (`snippets`)

| Команда | Что делает |
|---|---|
| `snippets_list(search?, tag?)` | поиск по названию, тексту, сокращению и тегам; сортировка по частоте использования |
| `snippets_save(id?, input)` | `input = { title, body, shortcut?, tags }` — создать или изменить |
| `snippets_delete(id)` | |
| `snippets_render(id, values?)` | текст с подставленными переменными |
| `snippets_copy(id, values?)` | в буфер обмена; увеличивает `use_count` |
| `snippets_paste(id, values?)` | копирует, скрывает окно и нажимает Ctrl+V в программе, где вы работали. Нужно разрешение `system.paste` («Вставка текста», по умолчанию выключено) |
| `snippets_export()` / `snippets_import()` | JSON через диалоги файлов |

Переменные: `{{имя}}` (спрашиваются перед использованием), встроенные `{{date}}`, `{{time}}`, `{{datetime}}`, `{{weekday}}`.

Формат файла:

```json
{ "format": "focusdeck.snippets", "version": 1, "exported_at": "…",
  "snippets": [ { "title": "Подпись", "body": "С уважением, {{имя}}", "shortcut": "sig", "tags": ["почта"] } ] }
```

При импорте совпадения (то же название и текст) пропускаются и считаются в `duplicates`, некорректные записи попадают в `skipped`.

Настройки модуля: `paste_delay_ms` (250).

## Timers (`timers`)

| Команда | Что делает |
|---|---|
| `timers_state()` | `{ timers, stopwatches, stats: { pomodoros_today, focus_ms_today, sessions_today }, now_ms }` |
| `timers_save(id?, input)` | `input = { name, kind, duration_ms?, work_ms?, short_break_ms?, long_break_ms?, rounds? }`; изменение сбрасывает таймер |
| `timers_control(id, "start" \| "pause" \| "reset" \| "skip")` | `skip` — следующая фаза Pomodoro |
| `timers_delete(id)` | |
| `timer_sessions(limit?)`, `timer_sessions_clear()` | история сессий |
| `stopwatches_create(name)`, `stopwatches_rename`, `stopwatches_delete` | независимые секундомеры |
| `stopwatches_control(id, "start" \| "pause" \| "reset" \| "lap")` | |
| `stopwatches_save_result(id, note?)`, `stopwatch_results(limit?)`, `stopwatch_results_delete(id)` | сохранённые результаты |

**Pomodoro:** работа → короткий перерыв → … → после `rounds` рабочих интервалов длинный перерыв → работа (раунд 1).

**Фоновая задача модуля** (запускается и останавливается вместе с ним) раз в секунду:
- завершает истёкшие таймеры;
- пишет сессию в историю;
- шлёт уведомление (если включено);
- при `auto_start_next` сразу запускает следующую фазу;
- отправляет событие `focusdeck://timers-changed`.

Раз в час удаляет историю старше `history_days`. Сброс, пропуск и удаление записывают незавершённую сессию с фактическим временем.

Настройки модуля: `notify` (вкл.), `auto_start_next` (выкл.), `history_days` (90).

## Тесты

`cargo test --lib` — 55 тестов, из них для productivity:
- заметки: CRUD, поиск, теги, корзина, экспорт;
- быстрое добавление и повторы;
- задачи: фильтры, статусы, статистика;
- сниппеты: переменные, импорт и экспорт JSON;
- Pomodoro и сессии;
- секундомеры.
