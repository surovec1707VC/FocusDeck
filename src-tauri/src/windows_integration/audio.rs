//! Master volume of the default output device (Core Audio, COM).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Volume {
    /// 0.0 … 1.0
    pub level: f32,
    pub muted: bool,
}

#[cfg(windows)]
mod imp {
    use super::Volume;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender};
    use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};

    fn endpoint() -> windows::core::Result<IAudioEndpointVolume> {
        // SAFETY: COM is initialized for this (blocking) thread; S_FALSE / changed mode are fine.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
        }
    }

    pub fn get() -> Result<Volume, String> {
        let ep = endpoint().map_err(|e| format!("нет устройства вывода звука: {e}"))?;
        // SAFETY: valid COM interface.
        unsafe {
            let level = ep.GetMasterVolumeLevelScalar().map_err(|e| e.to_string())?;
            let muted = ep.GetMute().map_err(|e| e.to_string())?.as_bool();
            Ok(Volume { level, muted })
        }
    }

    pub fn set(level: Option<f32>, muted: Option<bool>) -> Result<Volume, String> {
        let ep = endpoint().map_err(|e| format!("нет устройства вывода звука: {e}"))?;
        // SAFETY: valid COM interface; no event context.
        unsafe {
            if let Some(l) = level {
                ep.SetMasterVolumeLevelScalar(l.clamp(0.0, 1.0), std::ptr::null()).map_err(|e| e.to_string())?;
            }
            if let Some(m) = muted {
                ep.SetMute(m, std::ptr::null()).map_err(|e| e.to_string())?;
            }
        }
        get()
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Volume;
    pub fn get() -> Result<Volume, String> {
        Err("только в Windows".into())
    }
    pub fn set(_: Option<f32>, _: Option<bool>) -> Result<Volume, String> {
        Err("только в Windows".into())
    }
}

pub use imp::{get, set};
