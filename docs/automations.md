# Automations — API

Автоматизация — это сценарий «триггер → условия → действия». Код: `src-tauri/src/automation/`; редактор: `src/pages/Automations.tsx` (+ `automation-fields.ts`); окно подтверждения: `src/components/AutomationConfirm.tsx`.

JSON-схема (draft 2020-12): [automation.schema.json](automation.schema.json). Её же отдаёт команда `automations_schema()`.

## Модель

| Поле | Описание |
|---|---|
| `id` | |
| `name` | название |
| `description` | описание |
| `trigger` | один триггер |
| `conditions[]` | условия; выполняться должны все |
| `actions[]` | действия выполняются по порядку; после первой ошибки остальные пропускаются |
| `enabled` | включена ли автоматизация |
| `cooldown_secs` | минимальная пауза между автоматическими запусками (ручной запуск её не учитывает) |
| `module_id` | модуль-владелец |
| `permissions` | только чтение: разрешения, которые нужны действиям |
| `dangerous` | только чтение: опасные действия |
| логи | `automations_logs`: статус, источник запуска, длительность, результат каждого условия и действия |

**Модули.** Автоматизация не запускается, если выключен модуль «Автоматизации» или её `module_id`; такой запуск пишется в журнал как `skipped`. Действия своих модулей (`switch_layout` → «Макеты», `start_timer` → «Таймеры») при выключенном модуле завершаются ошибкой `module_disabled`. Выключенный модуль «Автоматизации» останавливает наблюдатель и снимает горячие клавиши сценариев.

## Триггеры

| type | Когда срабатывает |
|---|---|
| `app_start` | запуск FocusDeck |
| `system_start` | старт системы: FocusDeck запущен не позже `within_minutes` после загрузки Windows (обычно через автозапуск) |
| `process_started` / `process_stopped` | запуск или закрытие программы (`process`: `obs64.exe`; проверка каждые 2 с) |
| `window_focus` | активное окно сменилось на подходящее (`process` и/или `title_contains`) |
| `hotkey` | глобальная горячая клавиша |
| `time` | `time` (ЧЧ:ММ) в дни `days` (1 = пн … 7 = вс; пусто — каждый день) |
| `interval` | каждые `minutes` минут |
| `display_change` | подключение (`up`) или отключение (`down`) монитора |
| `network_change` | интернет появился (`up`) или пропал (`down`); проверка каждые 5 с |
| `manual` | только кнопка «Запустить» |
| `ui_button` | кнопка с подписью `label` на странице «Автоматизации» |

Наблюдатель — фоновая задача модуля «Автоматизации». Он проверяет состояние раз в секунду и опрашивает только то, что нужно включённым сценариям. Первое наблюдение не считается изменением.

## Условия

| type | Параметры |
|---|---|
| `process_running` | `process`, `running` |
| `active_app` | `process` |
| `weekday` | `days` |
| `time_range` | `from`, `to` (может переходить через полночь) |
| `module_enabled` | `module_id`, `enabled` |
| `active_layout` | `layout_id` |
| `internet` | `online` |

Если условие не выполнено, запуск записывается в журнал как `skipped`. У частых триггеров (окно, процессы, интервал) такие пропуски в журнал не пишутся, чтобы не засорять его.

## Действия

| type | Подтверждение | Разрешение | Что делает |
|---|---|---|---|
| `open_app` | | `system.launch_apps` | `.exe`, `.lnk`, `.url`; пути с `%ПЕРЕМЕННЫМИ%`; `args` — только для `.exe` |
| `close_app` | **да** | `system.close_apps` | `taskkill /IM` (мягко) или `/F` (`force`); «не запущен» не считается ошибкой |
| `open_url` | | | только `http`, `https`, `mailto` |
| `open_folder` | | | |
| `open_file` | **да**, для скриптов и программ (`.bat`, `.ps1`, `.exe` …) | `system.launch_apps` для них | |
| `run_command` | **да** | `system.run_commands` | `cmd /C`, таймаут — настройка `command_timeout_secs` |
| `switch_layout` | | | `layout_id` или первый макет профиля `profile` |
| `set_module` | | | критические модули выключить нельзя, модуль «Автоматизации» менять нельзя |
| `notify` | | `app.notifications` | |
| `copy_text` | | | |
| `paste_text` | | `system.paste` | Ctrl+V в предыдущей программе |
| `screenshot` | | | все мониторы в BMP, по умолчанию в `Изображения\FocusDeck` |
| `start_timer` | | | таймер модуля «Таймеры» (создаётся, если его нет) |
| `focus_mode` | | | Focus Mode на `minutes` (0 — до выключения) |
| `delete_files` | **да** | `files.delete` | файлы `pattern` из `folder` старше `older_than_days` — в **Корзину**, не навсегда |
| `log`, `show_window`, `hide_window` | | | |

Изменение системных настроек Windows автоматизации не поддерживают вообще.

**Focus Mode** скрывает всплывающие уведомления, кроме таймеров; сами уведомления сохраняются в истории. Режим не сохраняется после перезапуска. Его можно включить кнопкой на странице или командой `focus_mode_set`.

## Подтверждение опасных действий

- **Запуск из интерфейса** (кнопка, «Запустить», тест): без `confirm: true` команда возвращает `confirmation_required`. UI показывает список опасных действий и повторяет запуск после согласия.
- **Автоматический запуск** (время, процесс, окно…): запрос попадает в очередь, окно FocusDeck показывается, приходит уведомление. Диалог `AutomationConfirm` выводит план и ждёт ответа. Без ответа запрос истекает через 5 минут. Отказ записывается в журнал.
- Подтверждение не заменяет разрешения: без нужного разрешения запуск завершается ошибкой «нет разрешений», и ни одно действие не выполняется.

## Команды

| Команда | Что делает |
|---|---|
| `automations_list()` | список автоматизаций |
| `automations_create(input)` | создать |
| `automations_update(id, input)` | изменить |
| `automations_set_enabled(id, enabled)` | включить или выключить |
| `automations_delete(id)` | удалить вместе с журналом |
| `automations_reorder(ids)` | новый порядок (drag-and-drop) |
| `automations_run(id, confirm?, source?)` | запуск `manual` или `ui_button` → `RunReport` |
| `automations_test(input, id?, execute?, confirm?)` | проверка состояния редактора (в том числе несохранённого): модули, условия, разрешения, план. С `execute: true` действия выполняются (только для сохранённой) |
| `automations_logs(id?, limit?)` | журнал запусков |
| `automations_pending()` | ожидающие подтверждения запуски |
| `automations_confirm(token, approve)` | ответ на запрос подтверждения |
| `automations_templates()` | список готовых сценариев |
| `automations_install_template(key)` | установить сценарий |
| `automations_schema()` | JSON-схема |
| `automations_pick(kind)` | выбор пути в редакторе |
| `focus_mode_get()` / `focus_mode_set(enabled, minutes?)` | Focus Mode |

```ts
interface RunReport {
  status: "ok" | "error" | "skipped" | "pending" | "dry_run";
  message: string;
  conditions: { ok: boolean; text: string }[];
  plan: string[];                       // описание действий
  actions: { action: string; ok: boolean; message: string }[];
  missing_permissions: string[];
  dangers: string[];
}
```

События:
- `focusdeck://automation-run` — после каждого запуска;
- `focusdeck://automation-confirm` — нужно подтверждение;
- `focusdeck://focus-mode` — изменился Focus Mode.

## Готовые сценарии

Полный JSON — в `src-tauri/src/automation/templates.rs`. Сценарии «Start Gaming» и «Morning Setup» устанавливаются выключенными, чтобы не сработать неожиданно.

| key | Триггер | Действия |
|---|---|---|
| `start_work` | кнопка «Начать работу» | макет «Работа», включить «Уведомления» и «Задачи», выключить Focus Mode, уведомление |
| `start_coding` | кнопка «Код» | макет «Программирование», VS Code (`%LOCALAPPDATA%\Programs\Microsoft VS Code\Code.exe`), таймер 50 мин |
| `start_streaming` | кнопка «Стрим», условие «есть интернет» | макет «Стрим», OBS, Focus Mode, уведомление |
| `start_gaming` | запуск `steam.exe` | макет «Игры», Focus Mode |
| `focus_session` | кнопка «Фокус 25» | макет «Фокус», Focus Mode 25 мин, Pomodoro 25 мин |
| `end_work` | кнопка «Конец дня» | выключить Focus Mode, скриншот, уведомление, скрыть окно |
| `morning_setup` | 09:00 пн–пт, условие «есть интернет» | макет «Работа», календарь в браузере, уведомление |
| `cleanup_desktop` | ручной запуск | `*.tmp` с рабочего стола старше 7 дней — в Корзину (с подтверждением), уведомление |

Пример (Morning Setup):

```json
{
  "name": "Morning Setup",
  "enabled": true,
  "trigger": { "type": "time", "time": "09:00", "days": [1, 2, 3, 4, 5] },
  "conditions": [ { "type": "internet", "online": true } ],
  "actions": [
    { "type": "switch_layout", "profile": "work" },
    { "type": "open_url", "url": "https://calendar.google.com" },
    { "type": "notify", "title": "Доброе утро", "body": "Календарь открыт." }
  ],
  "cooldown_secs": 3600
}
```

## База данных

Миграция `0006_automations_v2.sql`:
- `automations`: добавлены `description`, `conditions_json`, `cooldown_secs`, `template`, `position`;
- `automation_logs`: добавлены `source`, `duration_ms`, `details_json`.

Автоматизации версии 1 (`app_start`, `interval`, `hotkey`, `notify`, `log`, `show_window`, `hide_window`) читаются без изменений.
