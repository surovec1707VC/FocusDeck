# FocusDeck

Локальное рабочее пространство для Windows 10/11: виджеты, макеты, автоматизации, глобальные горячие клавиши и работа из трея.

**Без ИИ, телеметрии, аналитики и облачной синхронизации.** Все данные хранятся локально в SQLite.

## Быстрый старт

### Требования
- Windows 10/11, WebView2 Runtime (в Windows 11 уже установлен)
- Node.js 20+ и npm
- Rust stable (`rustup`) и Visual Studio Build Tools с компонентом «Desktop development with C++» и Windows SDK

### Запуск
```powershell
npm install
npm run tauri dev
```
Первая сборка Rust занимает несколько минут. Окно откроется без системной рамки, значок появится в трее.

### Проверки
```powershell
npx tsc --noEmit
cd src-tauri; cargo test --lib
```

### Сборка установщика
```powershell
npm run tauri build
```

## Возможности каркаса
- Окно без рамки, прозрачное, со своим заголовком; сворачивание в трей, запуск свёрнутым, запоминание позиции и размера.
- Значок в трее: клик показывает или скрывает окно; меню «Показать / Свернуть в трей / Настройки / Выход».
- Глобальная горячая клавиша `Ctrl+Alt+D` показывает или скрывает окно. Есть автоматизации по хоткею.
- Опциональный автозапуск с Windows (запуск свёрнутым в трей).
- Тёмная тема по умолчанию, светлая и системная; русский интерфейс; адаптивная вёрстка.
- Страницы: Обзор, Виджеты, Макеты, Автоматизации, Центр управления, Модули, Настройки, Разрешения, О программе.
- Каждая функция — модуль (21 модуль, 13 категорий): включение/отключение, настройки в JSON, права, версия, журнал, health check; экспорт и импорт конфигурации в JSON. API — [docs/api.md](docs/api.md).

## Структура проекта
```
src/                        frontend (React + TypeScript + Vite + Tailwind CSS 4)
  main.tsx, App.tsx         точка входа и оболочка окна
  index.css                 Tailwind, темы, общие стили
  i18n/                     русская локализация
  lib/                      API к Rust, тема, настройки, хуки
  components/               заголовок окна, меню, тосты, UI, виджеты
  pages/                    9 страниц (в т.ч. Центр управления)
src-tauri/                  backend (Rust)
  tauri.conf.json           окно: frameless, transparent, скрыто до setup
  capabilities/             права фронтенда (только управление окном и opener)
  src/
    lib.rs                  запуск приложения
    error.rs  logging.rs    ошибки (thiserror/anyhow) и tracing
    state.rs                общее состояние
    db/                     SQLite и миграции (db/migrations/0001_init.sql, 0002_module_registry.sql)
    modules/                Module Registry: каталог, настройки, эффекты, health check, экспорт/импорт
    background.rs           фоновые задачи модулей
    automation/             автоматизации: типы, хранилище, движок
    plugins/                регистрация Tauri-плагинов
    settings/  permissions/  workspace/
    hotkeys.rs  tray.rs  window.rs  autostart.rs  info.rs
docs/                       документация
```

Подробнее — [docs/architecture.md](docs/architecture.md).

## Где хранятся данные
- База: `%APPDATA%\com.focusdeck.app\focusdeck.sqlite`
- Журналы: `%LOCALAPPDATA%\com.focusdeck.app\logs\`; уровень меняется переменной `FOCUSDECK_LOG`, например `debug`.
