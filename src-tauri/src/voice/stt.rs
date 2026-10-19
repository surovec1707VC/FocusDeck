//! Speech-to-text providers. Only configuration and availability checks
//! exist; nothing is downloaded or installed automatically
//! (see docs/voice-setup.md for manual setup).

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::VoiceSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SttProvider {
    /// whisper.cpp CLI (`whisper-cli.exe`) with a local ggml model.
    WhisperCpp,
    /// A local faster-whisper HTTP server (OpenAI-compatible `/v1/audio/transcriptions`).
    FasterWhisper,
    /// Windows built-in speech recognition (Windows.Media.SpeechRecognition).
    WindowsSpeech,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderStatus {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub ready: bool,
    pub detail: String,
}

fn file_status(label: &str, path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        Err(format!("{label} is not set"))
    } else if !Path::new(path).is_file() {
        Err(format!("{label} not found: {path}"))
    } else {
        Ok(())
    }
}

/// Checks configuration only — no process is started and no network is used.
pub fn status(provider: SttProvider, s: &VoiceSettings) -> ProviderStatus {
    let (name, result) = match provider {
        SttProvider::WhisperCpp => (
            "whisper.cpp",
            file_status("whisper.cpp binary", &s.whisper_cpp_binary)
                .and_then(|_| file_status("whisper model", &s.whisper_cpp_model))
                .map(|_| "binary and model found".to_string()),
        ),
        SttProvider::FasterWhisper => (
            "faster-whisper",
            if s.faster_whisper_url.starts_with("http://127.0.0.1") || s.faster_whisper_url.starts_with("http://localhost") {
                Ok(format!("server URL set to {} (start the server manually)", s.faster_whisper_url))
            } else {
                Err("server URL must point to localhost".into())
            },
        ),
        SttProvider::WindowsSpeech => (
            "Windows Speech",
            Err("integration not implemented yet; install the speech language pack to prepare".into()),
        ),
    };
    let (ready, detail) = match result {
        Ok(d) => (true, d),
        Err(d) => (false, d),
    };
    ProviderStatus {
        id: serde_json::to_value(provider).unwrap().as_str().unwrap().to_string(),
        name: name.into(),
        kind: "stt",
        ready,
        detail,
    }
}

pub const ALL: [SttProvider; 3] = [SttProvider::WhisperCpp, SttProvider::FasterWhisper, SttProvider::WindowsSpeech];
