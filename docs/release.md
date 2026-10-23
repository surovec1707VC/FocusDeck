# Релиз

Как собрать установщик и portable-версию, подписать их и настроить автообновления.

## Требования
- Windows 10/11 x64, Node.js 20+, Rust stable (MSVC), Visual Studio Build Tools с компонентом «Разработка классических приложений на C++».
- WebView2 Runtime: в Windows 11 он встроен. Установщик ставит его сам (`webviewInstallMode: downloadBootstrapper`).
- При первой сборке NSIS Tauri скачивает в свой кэш (`%LOCALAPPDATA%\tauri`) инструменты NSIS. На машине без интернета положите их туда заранее.

## 1. Версия и список изменений
1. Поднимите версию в трёх местах: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`.
2. Перенесите пункты в `CHANGELOG.md` под новый заголовок `## [x.y.z] — дата`.
3. Проверьте:
```bash
npm run typecheck
```
```bash
npm run test:rust
```
4. Пройдите UI smoke-тест на запущенном приложении (см. [README](../README.md#тесты)).

## 2. Установщик NSIS
```bash
npm run release:nsis
```
Результат — `src-tauri/target/release/bundle/nsis/FocusDeck_<версия>_x64-setup.exe`.

Параметры (`tauri.conf.json → bundle.windows.nsis`):
- `installMode: currentUser` — установка в профиль, без прав администратора;
- языки установщика — русский и английский;
- папка в меню «Пуск» — `FocusDeck`.

Данные пользователя (`%APPDATA%\com.focusdeck.app`) при удалении программы остаются.

## 3. Portable-версия
```bash
npm run release:portable
```
Скрипт `scripts/build-portable.ps1` собирает `focusdeck.exe` без установщика и кладёт рядом файл-маркер `portable`. Результат — `release/FocusDeck-<версия>-portable.zip` и `.sha256`.

Если рядом с exe лежит `portable`, FocusDeck хранит базу, журналы и профиль WebView2 в папке `data` рядом с собой (`src-tauri/src/paths.rs`). Исключение — секреты интеграций: они остаются в Credential Manager текущего пользователя. Portable-версия не обновляется сама: на странице «Обновления» она показывает «не настроен».

## 4. Подпись кода (Authenticode)
Без подписи SmartScreen показывает «Неизвестный издатель». Нужен сертификат подписи кода:
- **OV** — дешевле; репутация SmartScreen набирается со временем;
- **EV** — предупреждение исчезает сразу. С 2023 года ключ хранится только на токене или в облачном HSM.

### Вариант A: сертификат в хранилище Windows
1. Импортируйте `.pfx` в «Личное» хранилище и узнайте отпечаток:
```bash
powershell -Command "Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Format-List Subject,Thumbprint"
```
2. Укажите его в `tauri.conf.json`:
```json
"bundle": { "windows": {
  "certificateThumbprint": "<ОТПЕЧАТОК>",
  "digestAlgorithm": "sha256",
  "timestampUrl": "http://timestamp.digicert.com"
} }
```
3. `npm run release:nsis` подпишет и `focusdeck.exe`, и установщик.

### Вариант B: облачный HSM (Azure Trusted Signing и т. п.)
Задайте `bundle.windows.signCommand`, например:
```json
"signCommand": "trusted-signing-cli -e https://weu.codesigning.azure.net -a <account> -c <profile> %1"
```
Tauri вызывает команду для каждого подписываемого файла.

### Проверка
```bash
powershell -Command "Get-AuthenticodeSignature src-tauri\target\release\bundle\nsis\*.exe | Format-List Status,SignerCertificate"
```
Portable-zip подписывайте так же: подпишите `focusdeck.exe` через `signtool sign /fd sha256 /tr <timestamp> /td sha256 /sha1 <ОТПЕЧАТОК>` до упаковки.

Не храните `.pfx` и пароль в репозитории — только в секретах CI.

## 5. Автообновления
FocusDeck использует `tauri-plugin-updater`, но адрес и ключ задаются **при сборке**, а не в `tauri.conf.json`. Поэтому сборки без них (dev, portable, форки) никуда не обращаются.

### Однократно: ключ подписи обновлений
```bash
npx tauri signer generate -w %USERPROFILE%\.tauri\focusdeck-updater.key
```
- Приватный ключ и пароль храните в менеджере паролей или секретах CI. Если ключ потерян, существующие установки больше не смогут обновиться.
- Публичный ключ (`.key.pub`) не секретен.

### Сборка релиза с обновлениями
1. В `tauri.conf.json` включите артефакты: `"bundle": { "createUpdaterArtifacts": true }`.
2. Задайте переменные окружения и соберите:
```bash
powershell -Command "$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $env:USERPROFILE\.tauri\focusdeck-updater.key -Raw; $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = '<пароль>'; $env:FOCUSDECK_UPDATER_PUBKEY = Get-Content $env:USERPROFILE\.tauri\focusdeck-updater.key.pub -Raw; $env:FOCUSDECK_UPDATE_ENDPOINT = 'https://github.com/<owner>/<repo>/releases/latest/download/latest.json'; $env:FOCUSDECK_RELEASES_URL = 'https://github.com/<owner>/<repo>/releases'; npm run release:nsis"
```
Рядом с установщиком появится `.sig`.

3. Опубликуйте в релизе установщик и `latest.json`:
```json
{
  "version": "0.2.0",
  "notes": "Markdown со списком изменений — показывается на странице «Обновления»",
  "pub_date": "2026-11-01T12:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<содержимое FocusDeck_0.2.0_x64-setup.exe.sig>",
      "url": "https://github.com/<owner>/<repo>/releases/download/v0.2.0/FocusDeck_0.2.0_x64-setup.exe"
    }
  }
}
```

### Как это работает у пользователя
1. Модуль «Обновления» и разрешение «Сеть: проверка обновлений» по умолчанию выключены.
2. «Проверить обновления» скачивает `latest.json` и сравнивает версии по SemVer.
3. Автопроверка при запуске — настройка модуля «Проверять при запуске» (по умолчанию выключена). Найденная версия превращается в уведомление; сама она ничего не устанавливает.
4. «Установить» после подтверждения:
   - копирует базу в `backups/pre-update-<текущая>.sqlite`;
   - запоминает точку отката;
   - скачивает пакет, проверяет подпись, устанавливает и перезапускает приложение.

### Откат
Миграции базы идут только вперёд, поэтому старой версии нужна старая база.
1. «Обновления → Подготовить откат данных» ставит копию перед обновлением на восстановление при следующем запуске. Текущая база сохраняется рядом.
2. Установите прежнюю версию со страницы релизов поверх текущей.

## 6. Чек-лист перед релизом
См. раздел «Чек-лист перед релизом» в [README](../README.md#чек-лист-перед-релизом).
