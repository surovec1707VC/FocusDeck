# Плагины и расширения

В FocusDeck нет загрузки стороннего кода во время работы: всё, что выполняется, собрано в exe и проверено тестами. Расширяют приложение на этапе сборки, через четыре точки:

| Что | Где | Как подключается |
|---|---|---|
| Плагины Tauri | `src-tauri/src/plugins/mod.rs` | регистрация в `Builder` |
| Коннекторы интеграций | `src-tauri/src/integrations/manifests/*.json` | JSON-манифест + строка в `SOURCES` |
| Виджеты | `widgets/catalog.rs` + `src/components/widgets/registry.ts` | запись в каталоге + компонент |
| Триггеры и действия автоматизаций | `automation/types.rs`, `automation/engine.rs` | вариант enum + валидация + исполнение |

## Плагины Tauri
| Плагин | Зачем | Доступ фронтенда |
|---|---|---|
| `single-instance` | второй запуск показывает уже открытое окно | — |
| `window-state` | позиция и размер окна | — |
| `global-shortcut` | глобальные горячие клавиши (только из Rust) | — |
| `autostart` | автозапуск с флагом `--minimized` | — |
| `opener` | открыть ссылку в браузере, показать файл в Проводнике | `opener:default` |
| `dialog` | окна «Открыть» и «Сохранить» (вызываются из Rust) | — |
| `updater` | обновления; адрес и ключ задаются при сборке, см. [release.md](release.md) | — |

**Правила для нового плагина:**
1. Только если задачу нельзя решить командой на Rust.
2. Права для WebView — минимальные, в `capabilities/*.json`, с комментарием, зачем они нужны.
3. Если плагин работает с сетью или системой — он скрыт за модулем и разрешением, которые по умолчанию выключены.

## Коннекторы интеграций (основной способ расширения)
Коннектор — это JSON-манифест: что умеет интеграция, какие разрешения ей нужны, какие у неё настройки и как проверить её состояние. Формат — [integration.schema.json](integration.schema.json), описание — [integrations.md](integrations.md).

```json
{
  "id": "my_tool",
  "name": "Мой инструмент",
  "description": "Что делает",
  "icon": "terminal",
  "version": "0.1.0",
  "kind": "local",
  "auth": { "type": "none" },
  "permissions": [{ "id": "launch", "name": "Запуск", "description": "…", "risk": "change" }],
  "actions": [{ "id": "open", "name": "Открыть", "description": "…", "permissions": ["launch"] }],
  "events": [],
  "settings": { "type": "object", "properties": {
    "path": { "type": "string", "title": "Путь", "default": "" }
  } },
  "health": { "kind": "executable", "target": "mytool", "setting": "path" },
  "enabled_by_default": false,
  "docs_url": "https://example.com/docs"
}
```

**Шаги:**
1. Положите файл в `src-tauri/src/integrations/manifests/` и добавьте `include_str!` в `SOURCES` (`manifest.rs`).
2. Добавьте иконку в `ICONS` (`src/pages/Integrations.tsx`) или используйте одну из существующих.
3. `cargo test` проверит манифест:
   - уникальность id;
   - ссылки actions → permissions;
   - значения по умолчанию;
   - отсутствие секретов в `settings`;
   - `https` у `docs_url`;
   - наличие `auth` у облачного коннектора.

**Ограничения:**
- Облачный коннектор (`kind: cloud`) недоступен в Offline Mode и включается только с подтверждением.
- Секреты — только через `auth`: они попадают в Credential Manager.
- Выполнение `actions` появится на следующем этапе: каждое действие пройдёт через `safety::authorize` (разрешение коннектора, Safe Mode, подтверждение, Action Log).

## Виджеты
1. **Backend.** Запись в `widgets/catalog.rs`: вид, минимальный размер, схема настроек, модуль-владелец, интервал обновления. Данные — ветка в `widgets/data.rs` (`widget_data` и `widget_action`).
2. **Модуль.** Каждый виджет — свой модуль (`widget_<вид>` в `modules/catalog.rs`), чтобы его можно было выключить.
3. **Frontend.** Компонент тела в `src/components/widgets/*` и запись в `registry.ts`: иконка и компонент. Данные загружает `useWidgetData`: он кэширует, ставит опрос на паузу при скрытом окне и прореживает частые события.
4. Тест в `widgets/mod.rs`: виджет выключенного модуля скрывается.

## Триггеры и действия автоматизаций
1. Вариант в `Trigger`, `Condition` или `Action` (`automation/types.rs`) с `serde(tag = "type")` и валидацией в `validate()`.
2. Если действие рискованное: `permission()` возвращает разрешение, `dangerous()` — текст для подтверждения.
3. Исполнение — ветка в `engine.rs::execute`. Процессы запускаются только со списком аргументов, без shell, кроме явного действия «Выполнить команду».
4. Описание — в `describe()`, `docs/automation.schema.json` и `docs/automations.md`.
5. Тест на валидацию: и корректные, и вредные значения.
