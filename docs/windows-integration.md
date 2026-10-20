# Windows Integration, Permissions Center, Safe Mode, Action Log

## Архитектура

```
UI (WindowsIntegration.tsx, Permissions.tsx)
   │  invoke(...)  ← useGuarded: «confirmation_required» → диалог → повтор с confirm=true
   ▼
windows_integration::win_*  ──►  safety::authorize(action, target, confirm)
                                    1. модуль включён?           → module_disabled
                                    2. разрешение выдано?        → permission_denied (+ permission_logs, action_log: denied)
                                    3. Safe Mode: в белом списке? → blocked (+ action_log: blocked)
                                    4. нужно подтверждение?      → confirmation_required
                                 ──►  действие:
                                       win.rs   Win32 (окна, SendInput)
                                       audio.rs Core Audio через COM (крейт windows)
                                       ps.rs    PowerShell / powercfg / netsh / WMI / WinRT / UI Automation
                                       tauri-plugin-opener — запуск приложений
                                 ──►  safety::finish → action_log (результат + undo_json) → событие focusdeck://action-log
```

- **Без `tauri-plugin-shell`.** Процессы запускаются из Rust (`std`/`tokio::process`, скрытое окно), а фронтенду доступ к shell не выдаётся. Так страница не может выполнить произвольную команду: в Rust зашиты только конкретные скрипты.
- **PowerShell** запускается с `-NoProfile -NonInteractive -EncodedCommand` и таймаутом. Пользовательские значения подставляются только как экранированные строковые литералы (`ps_str`), никогда как код.
- **UI Automation** только читает структуру окна (до 150 элементов, 2 уровня вложенности) и работает лишь с отдельным разрешением `ui.automation`.
- **Виртуальные рабочие столы.** Публичного API нет, поэтому состояние читается из реестра Explorer, а переключение — это нажатие Win+Ctrl+←/→/D/F4 (`SendInput`, разрешение `input.keyboard`).
- **Модуль `windows_integration`** по умолчанию выключен. Если его выключить, все команды `win_*` возвращают `module_disabled`, страница скрывается. Permissions Center, Safe Mode и Action Log работают всегда: это часть модуля «Безопасность».

## Команды Windows Integration

| Команда | Действие (каталог) | Риск | Разрешение | Отмена |
|---|---|---|---|---|
| `win_windows()`, `win_active_window()` | windows.list | чтение | windows.manage | |
| `win_window(hwnd, focus\|minimize\|maximize\|restore\|close, confirm?)` | windows.* | изменение; close — опасное | windows.manage | min/max/restore → прежнее состояние |
| `win_apps(query?, refresh?)` | apps.search | чтение | — | |
| `win_launch(app_id, confirm?)` | apps.launch | изменение | system.launch_apps | |
| `win_desktops()` | desktops.read | чтение | — | |
| `win_desktop(next\|previous\|new\|close, confirm?)` | desktops.* | изменение; close — опасное | input.keyboard | next ↔ previous |
| `win_volume()`, `win_set_volume(level?, muted?, confirm?)` | volume.read / volume.set | чтение / изменение | — / system.settings | прежняя громкость |
| `win_brightness()`, `win_set_brightness(level, confirm?)` | brightness.* | | — / system.settings | прежняя яркость |
| `win_power_plans()`, `win_set_power_plan(guid, confirm?)` | power.read / power.set | чтение / **опасное** | — / system.settings | прежняя схема |
| `win_wifi()`, `win_bluetooth()` | wifi.read / bluetooth.read | чтение | — | |
| `win_screenshot(copy?)` | screen.capture | изменение | screen.capture | |
| `win_clipboard_read()`, `win_clipboard_write(text, confirm?)` | clipboard.* | чтение / изменение | clipboard.read / clipboard.write | прежний текст |
| `win_notify(title, body?, native?)` | notify.app / notify.native | изменение | app.notifications | |
| `win_uia_inspect(hwnd)` | uia.inspect | чтение | ui.automation | |

Откуда берутся данные:
- приложения — `Get-StartApps` (обычные и из Магазина), запуск через `shell:AppsFolder\<AppID>`, список кэшируется на 5 минут;
- яркость — WMI `WmiMonitorBrightness`; работает только для встроенных экранов, у внешних мониторов яркость недоступна;
- Wi-Fi — `Get-NetAdapter` плюс SSID и уровень сигнала из `netsh wlan show interfaces`;
- Bluetooth — PnP-устройства и служба `bthserv`;
- схемы питания — `powercfg /list` и `/setactive`; разбор не зависит от языка Windows;
- уведомление Windows — WinRT toast от имени PowerShell;
- скриншот — BMP в папке `Изображения\FocusDeck`.

## Permissions Center

`permissions_center()` возвращает по каждому разрешению:
- описание и статус;
- категорию;
- `used_by` — модули, которые его используют: из каталога модулей и из действий модулей;
- `uses` — сколько раз использовано;
- `last_used_at` — когда использовано последний раз;
- `denials` — сколько было отказов.

Включение и отключение — `permissions_set`.

`permissions_access_log(id)` — журнал доступа: выдачи, отзывы и отказы (`permission_logs`) вместе с использованиями, блокировками и ошибками из Action Log.

| Категория | Разрешения |
|---|---|
| Доступ к файлам | fs.read, files.move, files.delete |
| Буфер обмена | clipboard.read, clipboard.write |
| Управление окнами | windows.manage, ui.automation |
| Ввод с клавиатуры | input.keyboard, system.paste, system.global_hotkeys |
| Ввод мышью | input.mouse (пока не используется) |
| Скриншоты | screen.capture (Windows Integration, действие автоматизаций «Скриншот») |
| Запуск процессов | system.launch_apps, system.run_commands, system.close_apps, system.autostart |
| Сетевые запросы | network.weather |
| Уведомления | app.notifications |
| Системные настройки | system.settings |

Новые разрешения по умолчанию выключены. Исключения — `files.move` (иначе Desktop Organizer перестал бы работать) и `clipboard.write`.

## Safe Mode

| Команда | Что делает |
|---|---|
| `safe_mode_get()` | текущее состояние |
| `safe_mode_set({ enabled, whitelist })` | включить, выключить, изменить белый список; нужен модуль «Безопасность» |
| `safety_actions()` | каталог действий: id, название, модуль, разрешение, риск (`read`, `change`, `dangerous`), можно ли отменить |

- **Выключен:** опасные действия всё равно подтверждаются.
- **Включён:**
  - действие не из белого списка → `blocked`, запись в журнал;
  - каждое изменение подтверждается пользователем;
  - белый список по умолчанию — только чтение.
- **Автоматизации, Desktop Organizer и быстрые действия Clipboard Hub** проверяют белый список: `automations.run`, `organizer.move`, `clipboard.quick_action`. Подтверждение опасных действий у них своё.

## Action Log

Таблица `action_log` (миграция `0008_windows_integration.sql`):

| Колонка | Содержимое |
|---|---|
| `created_at` | время |
| `module_id` | модуль |
| `action` | действие |
| `target` | цель |
| `result` | `ok`, `error`, `denied`, `blocked`, `undone` |
| `message` | пояснение |
| `permission` | использованное разрешение |
| `undo_json` | данные для отмены |
| `undone_at` | когда отменено |

Хранится до 5000 последних записей.

| Команда | Что делает |
|---|---|
| `action_log_list(module?, limit?)` | список записей (`can_undo` — можно ли отменить сейчас) |
| `action_log_undo(id)` | отмена: громкость, яркость, схема питания, состояние окна, буфер обмена, переключение рабочего стола, перемещение файлов Desktop Organizer |

В журнал пишут:
- Windows Integration — каждое действие;
- автоматизации — каждый запуск;
- Desktop Organizer — каждая операция, с отменой;
- Clipboard Hub — каждое быстрое действие.

## Примеры интеграций

**1. Новое действие в своём модуле**

```rust
// 1) описание в safety::ACTIONS
a("teams.mute", "Выключить микрофон в Teams", "integrations", Some(PERM_KEYBOARD), Risk::Change, true),

// 2) команда
#[tauri::command]
pub async fn teams_mute(app: AppHandle, confirm: Option<bool>) -> AppResult<()> {
    let state = app.state::<AppState>();
    crate::productivity::require(&state, "integrations")?;               // модуль
    let def = safety::authorize(&state.db, "teams.mute", "Teams", confirm.unwrap_or(false)).await?;
    let result = press_ctrl_shift_m().await;                              // само действие
    safety::finish(&app, def, "Teams", &result, |_| "микрофон выключен".into(),
                   |_| Some(json!({ "toggle": true }))).await;           // журнал + отмена
    result
}

// 3) отмена: ветка "teams.mute" в windows_integration::undo
```

**2. Вызов из интерфейса с подтверждением**

```tsx
const { run, dialog } = useGuarded(setError);
<button onClick={() => run((confirm) => api.winSetPowerPlan(guid, confirm), setPlans)}>…</button>
{dialog}
```

**3. Автоматизация «Вечерний режим»**

Триггер — время 22:00. Действия:
- `focus_mode`;
- `screenshot` (нужно разрешение `screen.capture`);
- `notify`.

Сам запуск автоматизации проверяет Safe Mode (`automations.run`) и попадает в Action Log.

**4. Аудит: кто трогал буфер обмена**

`permissions_access_log("clipboard.write")` → «использовано · windows_integration · clipboard.write: текст…».
