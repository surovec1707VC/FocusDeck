# Система виджетов — API

Виджет — независимый блок на обзоре. Каждый тип виджета — отдельный модуль в Module Registry: включается и отключается в Центре управления или на странице «Виджеты», имеет собственные настройки, разрешения и журнал. Все данные виджеты получают только через Tauri-команды (обёртки — `src/lib/api.ts`).

## Модель

| Поле | Откуда |
|---|---|
| id, название, описание | модуль виджета (`modules/catalog.rs`) |
| категория, размер по умолчанию и минимальный, источник данных, интервал обновления | `widgets/catalog.rs` (`WidgetDef`) |
| настройки, enabled/disabled, permissions | модуль виджета (настройки общие для всех экземпляров типа) |
| настройки экземпляра (ссылки, программы, длительность таймера) | `widgets.config_json` |
| положение и размер, закреплён, свёрнут, скрыт | `widgets.x, y, w, h, pinned, collapsed, hidden` |

### Каталог

| kind | Модуль | Нужны ещё | Категория | Данные | Обновление | Разрешение |
|---|---|---|---|---|---|---|
| clock | widget_clock | | time | local | 60 с (синхронизация) | |
| date | widget_date | | time | local | 5 мин | |
| calendar | widget_calendar | | time | database | 5 мин | |
| countdown | widget_countdown | | time | config | по действию | |
| stopwatch | widget_stopwatch | | time | config | по действию | |
| notes | widget_notes | notes | productivity | database | по действию | |
| todo | widget_todo | tasks | productivity | database | 60 с | |
| quick_links | widget_quick_links | | productivity | config | по действию | |
| app_launcher | widget_app_launcher | | productivity | config | по действию | system.launch_apps |
| system_info | widget_system_info | | system | Windows API | настройка `refresh_secs` (3 с) | |
| clipboard | widget_clipboard | clipboard_history | system | clipboard | 5 с + событие | clipboard.read (у модуля истории) |
| notifications | widget_notifications | notifications | information | database | 30 с + событие | |
| weather | widget_weather | | information | network | настройка `refresh_minutes` (30 мин) | network.weather |
| status | widget_status | | information | database | 30 с | |

Виджет виден, только если включены модуль «Виджеты», модуль самого виджета и модули из колонки «Нужны ещё». Выключенные виджеты остаются в базе вместе с позицией и данными.

## Команды

| Команда | Результат |
|---|---|
| `widget_catalog()` | `WidgetCatalogEntry[]`: определение + состояние модуля, настройки, схема, недостающие модули и разрешения, число экземпляров на активном макете |
| `widgets_list(layoutId?)` | видимые виджеты макета (по умолчанию активного); новые без позиции размещаются в первом свободном месте |
| `widgets_add(kind, layoutId?, title?)` | `Widget`; размер по умолчанию, первое свободное место. `module_disabled`, если нужный модуль выключен |
| `widgets_update(id, patch)` | `patch = { title?, config?, pinned?, collapsed?, hidden? }`; `config` сливается с текущим, служебные ключи (состояние таймера, `note_id`) игнорируются. При показе скрытого виджета на занятом месте он переносится |
| `widgets_save_grid(layoutId, items)` | сохраняет `{ id, x, y, w, h }[]` одной транзакцией; проверяются границы (до 24 колонок, высота до 24) и минимальный размер |
| `widgets_remove(id)` | удаляет виджет и его собственные данные (заметку) |
| `widget_data(id, params?, force?)` | `WidgetData` (см. ниже) |
| `widget_action(id, action, payload?, params?)` | выполняет действие и возвращает свежие `WidgetData` |
| `widget_pick_app()` | диалог выбора `.exe/.lnk/.url/.appref-ms`; ничего не запускает |

```ts
interface WidgetData<T> {
  widget_id: string; kind: WidgetKind;
  data: T;                 // для погоды без сети — последние сохранённые данные
  fetched_at: string|null; from_cache: boolean;
  error: string|null;      // «нет связи…», «нет разрешения…»
  refresh_secs: number;    // 0 — только после действий
}
```

### Действия

| kind | action и payload |
|---|---|
| notes | `save { body }` |
| todo | `add { title }`, `toggle { id }`, `delete { id }`, `clear_done` |
| quick_links | `open { index }` — только http, https, mailto |
| app_launcher | `launch { index }` — нужно разрешение «Запуск программ», файл должен существовать |
| clipboard | `copy { id }`, `pin { id, pinned }`, `delete { id }`, `clear` (кроме закреплённых) |
| notifications | `read_all`, `delete { id }`, `clear` |
| calendar | `add { title, date: "ГГГГ-ММ-ДД", time?: "ЧЧ:ММ", note? }`, `delete { id }`; `params: { month: "ГГГГ-ММ" }` |
| countdown | `start`, `pause`, `reset`, `set { duration_ms, label? }`. Истёкший таймер завершается при следующем чтении данных (один раз) и шлёт уведомление |
| stopwatch | `start`, `pause`, `reset`, `lap` (до 100 кругов) |

Состояние таймера и секундомера — метки времени в базе, поэтому они продолжают идти после перезапуска.

## Кэш, offline и производительность

- Погода: `widget_cache` хранит последний прогноз; свежий кэш отдаётся без сети, при ошибке сети или без разрешения возвращаются кэшированные данные и `error`. Запросы — к open-meteo.com (геокодинг + прогноз, без ключей); это единственное сетевое обращение FocusDeck.
- Системные данные читаются в фоновом потоке (`spawn_blocking`) и кэшируются на ~1 с, чтобы несколько виджетов не опрашивали Windows одновременно.
- Во фронтенде (`useWidgetData`) последние данные каждого виджета хранятся в памяти: при повторном открытии они показываются сразу, при ошибке обновления не пропадают. Опрос приостанавливается, пока окно скрыто.
- История буфера — фоновая задача модуля `clipboard_history`: раз в секунду сравнивает номер изменения буфера, сохраняет только текст, скопированный во время работы, пропускает содержимое, помеченное менеджерами паролей как скрытое.

## Макеты (профили)

Профили: `work`, `coding`, `streaming`, `gaming`, `focus`, `custom`. При первом запуске после миграции создаются готовые макеты для первых пяти; удалённые не пересоздаются. Новый макет с профилем получает набор виджетов профиля, `custom` — пустой.

| Команда | Что делает |
|---|---|
| `layouts_list()` | `Layout[]` с `profile`, `hotkey`, `widget_count` |
| `layouts_create(name, profile?)` | новый макет (неактивный) |
| `layouts_rename(id, name)` | |
| `layouts_duplicate(id)` | копия со всеми виджетами, позициями и копиями заметок; без горячей клавиши |
| `layouts_activate(id)` | ручное переключение |
| `layouts_delete(id)` | вместе с виджетами и их заметками; последний макет удалить нельзя |
| `layouts_set_hotkey(id, hotkey \| null)` | глобальное сочетание переключения; проверяется на дубли (окно, другие макеты, автоматизации) и на занятость другой программой |

Сочетания макетов работают, когда включены настройка «Глобальные горячие клавиши», модули «Глобальные горячие клавиши» и «Макеты» и выдано разрешение. Переключение по сочетанию шлёт событие `focusdeck://layout-changed` (payload — `Layout`).

## События

| Событие | Когда |
|---|---|
| `focusdeck://layout-changed` | макет переключён горячей клавишей |
| `focusdeck://clipboard-changed` | в историю буфера добавлена запись |
| `focusdeck://notify` | новое уведомление (виджет «Уведомления» обновляется) |
| `focusdeck://modules-changed` | виджеты перечитывают настройки и состав |

## Сетка (UI)

`src/components/widgets/WidgetGrid.tsx` + `src/lib/grid.ts`: 12 колонок по умолчанию (настройки модуля «Обзор»: `columns`, `row_height`, `compact`). Перетаскивание — за ручку в заголовке, размер — за правый нижний угол; с клавиатуры: стрелки двигают, Shift+стрелки меняют размер. Закреплённые виджеты не двигаются и не меняют размер, остальные сдвигаются вниз (или подтягиваются вверх при `compact`). Свёрнутый виджет занимает одну строку, его высота сохраняется. На ширине меньше 640 px виджеты показываются списком.

## SQL

`src-tauri/src/db/migrations/0003_widget_system.sql`:
- `widgets`: новые `x, y, w, h, pinned, collapsed, hidden`; колонка `size` удалена (переведена в `w × h`); типы `tasks → todo`, `note → notes`, текст заметок перенесён в `notes`.
- `widget_layouts`: `profile`, `hotkey`, `position`.
- новые таблицы `widget_cache`, `calendar_events`, `notifications`; индекс истории буфера.
