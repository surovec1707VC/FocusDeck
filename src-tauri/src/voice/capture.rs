//! Microphone capture. Not implemented yet: the backend will use WASAPI
//! (e.g. via the `cpal` crate) and feed 16-bit mono frames into the VAD.
//! Device selection currently happens in the UI through the WebView's
//! `mediaDevices` API; the chosen device id is stored in voice settings.
// Used once the capture backend lands.
#![allow(dead_code)]

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CaptureConfig {
    pub device_id: String,
    pub sample_rate: u32,
    /// Frame length fed to the VAD.
    pub frame_ms: u32,
}

impl CaptureConfig {
    pub fn samples_per_frame(&self) -> usize {
        (self.sample_rate as usize * self.frame_ms as usize) / 1000
    }
}

/// A source of PCM frames. Implementations push frames to `on_frame` until stopped.
pub trait AudioCapture: Send {
    fn start(&mut self, config: &CaptureConfig, on_frame: Box<dyn FnMut(&[i16]) + Send>) -> anyhow::Result<()>;
    fn stop(&mut self) -> anyhow::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_size() {
        let c = CaptureConfig { device_id: "default".into(), sample_rate: 16_000, frame_ms: 30 };
        assert_eq!(c.samples_per_frame(), 480);
    }
}
