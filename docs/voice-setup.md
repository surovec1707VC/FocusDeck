# Voice setup (ручная установка)

FocusDeck **ничего не скачивает и не устанавливает сам**: ни модели, ни Python-пакеты. Всё ниже выполняется вручную и только при желании. После установки укажите пути на странице **Voice** и нажмите **Save**. Блок «Provider status» покажет, найдены ли файлы.

> Состояние на сейчас: готовы настройки, хоткей push-to-talk, VAD и локальная маршрутизация команд. Захват микрофона и вызов STT/TTS ещё не подключены (см. «Что дальше»). Подготовить движки можно заранее.

Рекомендуемая папка для всего голосового: `D:\FocusDeck\voice\` (на диске C: мало места).

---

## 1. whisper.cpp — распознавание речи (рекомендуется)

Локально, без Python, работает на CPU.

1. Скачайте Windows-сборку со страницы релизов: <https://github.com/ggml-org/whisper.cpp/releases>. Нужен архив `whisper-bin-x64.zip` (или `whisper-blas-bin-x64.zip`, он быстрее на CPU).
2. Распакуйте, например в `D:\FocusDeck\voice\whisper\`. Нужный файл — `whisper-cli.exe` (в старых сборках `main.exe`).
3. Скачайте модель с <https://huggingface.co/ggerganov/whisper.cpp/tree/main>:

   | Модель | Размер | Комментарий |
   |---|---|---|
   | `ggml-base.bin` | ~142 MB | быстро, для коротких команд |
   | `ggml-small.bin` | ~466 MB | заметно точнее на русском |
   | `ggml-large-v3-turbo.bin` | ~1.6 GB | лучшее качество, медленно на CPU |

   Положите файл в `D:\FocusDeck\voice\models\`.
4. Проверка из PowerShell:
   ```powershell
   D:\FocusDeck\voice\whisper\whisper-cli.exe -m D:\FocusDeck\voice\models\ggml-small.bin -l ru -f some.wav
   ```
   (Нужен WAV 16 kHz mono.)
5. В FocusDeck: Voice → Speech-to-text → **whisper.cpp**, заполните пути к `whisper-cli.exe` и модели, нажмите **Save**.

## 2. faster-whisper — распознавание через локальный сервер (Python)

Подходит, если есть NVIDIA GPU или нужен Python. FocusDeck будет обращаться к **локальному** серверу с OpenAI-совместимым API (`/v1/audio/transcriptions`).

1. Установите Python 3.10+ с <https://www.python.org/downloads/>. Глобальные пакеты не нужны, работайте в venv.
2. Создайте окружение:
   ```powershell
   cd D:\FocusDeck\voice
   python -m venv fw-venv
   .\fw-venv\Scripts\Activate.ps1
   pip install faster-whisper
   ```
3. Запустите OpenAI-совместимый сервер поверх faster-whisper, например **Speaches** (бывший faster-whisper-server): <https://github.com/speaches-ai/speaches>. Инструкции по запуску (Docker или uv) есть в его README. Сервер должен слушать `http://127.0.0.1:8000`.
4. При первом запросе модель скачивается с Hugging Face (от сотен мегабайт до ~3 GB в зависимости от размера). Это делает сам сервер, а не FocusDeck.
5. В FocusDeck: Speech-to-text → **faster-whisper**, URL `http://127.0.0.1:8000`. Разрешены только localhost-адреса.

## 3. Windows Speech — встроенное распознавание

1. «Параметры» → «Время и язык» → «Речь».
2. В «Языки речи» добавьте русский (или нужный язык) и дождитесь установки пакета.
3. «Параметры» → «Конфиденциальность и защита» → «Микрофон»: доступ для классических приложений должен быть включён.

Интеграция в FocusDeck ещё не реализована (статус «not implemented yet»). Пакет можно поставить заранее.

## 4. Piper — синтез речи (локально)

1. Скачайте Windows-сборку: <https://github.com/rhasspy/piper/releases> (`piper_windows_amd64.zip`). Проект переехал в <https://github.com/OHF-Voice/piper1-gpl>, новые версии ищите там.
2. Распакуйте в `D:\FocusDeck\voice\piper\`.
3. Скачайте голос с <https://huggingface.co/rhasspy/piper-voices/tree/main>, например `ru/ru_RU/irina/medium/`. Нужны **оба** файла: `ru_RU-irina-medium.onnx` (~60 MB) и `ru_RU-irina-medium.onnx.json`, в одну папку.
4. Проверка:
   ```powershell
   echo "Привет, это FocusDeck" | D:\FocusDeck\voice\piper\piper.exe --model D:\FocusDeck\voice\piper\ru_RU-irina-medium.onnx --output_file test.wav
   ```
5. В FocusDeck: Text-to-speech → **Piper**, пути к `piper.exe` и `.onnx`.

## 5. Windows TTS — встроенные голоса

Скачивать ничего не нужно. Дополнительные голоса: «Параметры» → «Время и язык» → «Речь» → «Добавить голоса». Интеграция в FocusDeck ещё не реализована.

---

## Настройки на странице Voice

| Настройка | По умолчанию | Описание |
|---|---|---|
| Voice enabled | выкл | включает регистрацию хоткея push-to-talk |
| Microphone | System default | список берётся из WebView; для имён устройств нажмите «Show microphone names» и разрешите доступ |
| Sample rate | 16000 Hz | Whisper работает на 16 kHz |
| Language | `ru` | код языка или `auto` |
| Wake word | пусто | пока не используется; пусто — только push-to-talk |
| Push-to-talk hotkey | `Ctrl+Alt+M` | формат вида `Ctrl+Shift+V`. Если сочетание занято другой программой (например, `Ctrl+Alt+Space` занят быстрым вызовом Claude Desktop), будет ошибка и настройки не сохранятся |
| STT provider | whisper.cpp | |
| TTS provider | Windows TTS | |
| Auto-send to AI | выкл | нераспознанные локально команды отправлять ассистенту, а не скиллу `voice-intent-router` |

Настройки хранятся в SQLite (`settings`, ключ `voice.settings`).

## Что дальше (не сделано)
1. Захват микрофона в Rust (WASAPI через крейт `cpal`) → `EnergyVad` → WAV 16 kHz.
2. Вызов `whisper-cli.exe` или faster-whisper-сервера с этим WAV.
3. Маршрутизация текста: `voice_route_intent`, затем скилл `voice-intent-router` → подтверждение пользователя.
4. Озвучка ответа через Piper или Windows TTS (`Windows.Media.SpeechSynthesis` через крейт `windows`).
