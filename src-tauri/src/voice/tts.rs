//! Text-to-speech providers. Configuration and availability checks only;
//! voices are installed manually (see docs/voice-setup.md).

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::stt::ProviderStatus;
use super::VoiceSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsProvider {
    /// Piper (`piper.exe`) with a local `.onnx` voice.
    Piper,
    /// Windows built-in voices (Windows.Media.SpeechSynthesis).
    WindowsTts,
}

pub fn status(provider: TtsProvider, s: &VoiceSettings) -> ProviderStatus {
    let (name, ready, detail) = match provider {
        TtsProvider::Piper => {
            let missing = [("piper binary", &s.piper_binary), ("piper voice", &s.piper_voice)]
                .into_iter()
                .find(|(_, p)| p.trim().is_empty() || !Path::new(p.as_str()).is_file());
            match missing {
                None => ("Piper", true, "binary and voice found".to_string()),
                Some((label, p)) if p.trim().is_empty() => ("Piper", false, format!("{label} is not set")),
                Some((label, p)) => ("Piper", false, format!("{label} not found: {p}")),
            }
        }
        TtsProvider::WindowsTts => (
            "Windows TTS",
            false,
            "integration not implemented yet; built-in voices need no download".to_string(),
        ),
    };
    ProviderStatus {
        id: serde_json::to_value(provider).unwrap().as_str().unwrap().to_string(),
        name: name.into(),
        kind: "tts",
        ready,
        detail,
    }
}

pub const ALL: [TtsProvider; 2] = [TtsProvider::Piper, TtsProvider::WindowsTts];
