//! Voice pipeline: capture → VAD → STT → intent → (TTS). This section only
//! provides structure, settings, provider status, a push-to-talk hotkey and
//! local intent routing. No models are downloaded and no Python is used.

pub mod capture;
pub mod intent_router;
pub mod stt;
pub mod tts;
pub mod vad;

use std::str::FromStr;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use stt::{ProviderStatus, SttProvider};
use tts::TtsProvider;

const SETTINGS_KEY: &str = "voice.settings";
pub const PTT_EVENT: &str = "voice://push-to-talk";
pub const SAMPLE_RATES: [u32; 6] = [8_000, 16_000, 22_050, 32_000, 44_100, 48_000];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct VoiceSettings {
    pub voice_enabled: bool,
    /// Device id from the WebView `mediaDevices` list, or "default".
    pub microphone: String,
    pub sample_rate: u32,
    /// BCP-47 language like "ru", "en-US", or "auto".
    pub language: String,
    /// Empty = wake word disabled (push-to-talk only).
    pub wake_word: String,
    pub push_to_talk: String,
    pub stt_provider: SttProvider,
    pub tts_provider: TtsProvider,
    pub auto_send_to_ai: bool,
    pub whisper_cpp_binary: String,
    pub whisper_cpp_model: String,
    pub faster_whisper_url: String,
    pub piper_binary: String,
    pub piper_voice: String,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            voice_enabled: false,
            microphone: "default".into(),
            sample_rate: 16_000,
            language: "ru".into(),
            wake_word: String::new(),
            push_to_talk: "Ctrl+Alt+M".into(),
            stt_provider: SttProvider::WhisperCpp,
            tts_provider: TtsProvider::WindowsTts,
            auto_send_to_ai: false,
            whisper_cpp_binary: String::new(),
            whisper_cpp_model: String::new(),
            faster_whisper_url: "http://127.0.0.1:8000".into(),
            piper_binary: String::new(),
            piper_voice: String::new(),
        }
    }
}

impl VoiceSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !SAMPLE_RATES.contains(&self.sample_rate) {
            return Err(format!("sample rate must be one of {SAMPLE_RATES:?}"));
        }
        let lang_ok = self.language == "auto"
            || (2..=8).contains(&self.language.len())
                && self.language.chars().all(|c| c.is_ascii_alphabetic() || c == '-');
        if !lang_ok {
            return Err("language must be a code like \"ru\", \"en-US\" or \"auto\"".into());
        }
        if self.wake_word.chars().count() > 40 {
            return Err("wake word is too long".into());
        }
        if self.microphone.trim().is_empty() {
            return Err("microphone must not be empty (use \"default\")".into());
        }
        if !self.push_to_talk.trim().is_empty() {
            Shortcut::from_str(&self.push_to_talk)
                .map_err(|e| format!("invalid push-to-talk hotkey `{}`: {e}", self.push_to_talk))?;
        }
        if !(self.faster_whisper_url.starts_with("http://127.0.0.1") || self.faster_whisper_url.starts_with("http://localhost")) {
            return Err("faster-whisper URL must point to localhost".into());
        }
        Ok(())
    }
}

pub struct VoiceState {
    settings: Mutex<VoiceSettings>,
    /// Hotkey currently registered with the OS, if any.
    registered: Mutex<Option<Shortcut>>,
}

impl VoiceState {
    pub async fn load(pool: &SqlitePool) -> anyhow::Result<Self> {
        let stored: Option<(String,)> = sqlx::query_as("SELECT value_json FROM settings WHERE key = ?")
            .bind(SETTINGS_KEY)
            .fetch_optional(pool)
            .await?;
        let settings = stored
            .and_then(|(json,)| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        Ok(Self { settings: Mutex::new(settings), registered: Mutex::new(None) })
    }

    pub fn settings(&self) -> VoiceSettings {
        self.settings.lock().unwrap().clone()
    }

    /// Applies new settings: the hotkey is registered first and the settings
    /// are saved only if that works; otherwise the previous state is restored.
    async fn replace(&self, app: &AppHandle, pool: &SqlitePool, settings: VoiceSettings) -> anyhow::Result<()> {
        let previous = std::mem::replace(&mut *self.settings.lock().unwrap(), settings.clone());
        if let Err(e) = self.apply_hotkey(app) {
            *self.settings.lock().unwrap() = previous;
            let _ = self.apply_hotkey(app);
            return Err(e);
        }
        self.save(pool, settings).await
    }

    async fn save(&self, pool: &SqlitePool, settings: VoiceSettings) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO settings (key, value_json) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = datetime('now')",
        )
        .bind(SETTINGS_KEY)
        .bind(serde_json::to_string(&settings)?)
        .execute(pool)
        .await?;
        *self.settings.lock().unwrap() = settings;
        Ok(())
    }

    /// Registers the push-to-talk hotkey when voice is enabled (and unregisters
    /// the previous one). Press/release are emitted to the UI as `PTT_EVENT`.
    pub fn apply_hotkey(&self, app: &AppHandle) -> anyhow::Result<()> {
        let settings = self.settings();
        let gs = app.global_shortcut();
        let mut registered = self.registered.lock().unwrap();
        if let Some(old) = registered.take() {
            let _ = gs.unregister(old);
        }
        if !settings.voice_enabled || settings.push_to_talk.trim().is_empty() {
            return Ok(());
        }
        let shortcut = Shortcut::from_str(&settings.push_to_talk)?;
        gs.on_shortcut(shortcut, |app, _shortcut, event| {
            let state = match event.state() {
                ShortcutState::Pressed => "pressed",
                ShortcutState::Released => "released",
            };
            let _ = app.emit(PTT_EVENT, state);
        })
        .map_err(|e| anyhow::anyhow!("cannot register `{}` (taken by another app?): {e}", settings.push_to_talk))?;
        *registered = Some(shortcut);
        Ok(())
    }
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
pub fn voice_get_settings(voice: State<'_, VoiceState>) -> VoiceSettings {
    voice.settings()
}

#[tauri::command]
pub async fn voice_update_settings(
    app: AppHandle,
    voice: State<'_, VoiceState>,
    pool: State<'_, SqlitePool>,
    settings: VoiceSettings,
) -> CmdResult<VoiceSettings> {
    settings.validate()?;
    voice.replace(&app, &pool, settings).await.map_err(err)?;
    Ok(voice.settings())
}

#[tauri::command]
pub async fn voice_reset_settings(
    app: AppHandle,
    voice: State<'_, VoiceState>,
    pool: State<'_, SqlitePool>,
) -> CmdResult<VoiceSettings> {
    voice.replace(&app, &pool, VoiceSettings::default()).await.map_err(err)?;
    Ok(voice.settings())
}

/// Readiness of every STT/TTS provider (file checks only).
#[tauri::command]
pub fn voice_provider_status(voice: State<'_, VoiceState>) -> Vec<ProviderStatus> {
    let s = voice.settings();
    stt::ALL
        .into_iter()
        .map(|p| stt::status(p, &s))
        .chain(tts::ALL.into_iter().map(|p| tts::status(p, &s)))
        .collect()
}

/// Routes a (typed or transcribed) command locally; nothing is executed.
#[tauri::command]
pub fn voice_route_intent(voice: State<'_, VoiceState>, transcript: String) -> intent_router::Intent {
    intent_router::route_local(&transcript, voice.settings().auto_send_to_ai)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_disabled() {
        let s = VoiceSettings::default();
        assert!(s.validate().is_ok());
        assert!(!s.voice_enabled);
    }

    #[test]
    fn validation() {
        let bad = |f: fn(&mut VoiceSettings)| {
            let mut s = VoiceSettings::default();
            f(&mut s);
            s.validate().is_err()
        };
        assert!(bad(|s| s.sample_rate = 12_345));
        assert!(bad(|s| s.language = "русский".into()));
        assert!(bad(|s| s.push_to_talk = "Ctrl+Nope+Banana".into()));
        assert!(bad(|s| s.faster_whisper_url = "https://example.com".into()));
        assert!(bad(|s| s.microphone = " ".into()));
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: VoiceSettings = serde_json::from_str(r#"{ "language": "en" }"#).unwrap();
        assert_eq!(s.language, "en");
        assert_eq!(s.sample_rate, 16_000);
    }
}
