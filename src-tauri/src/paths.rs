//! Where FocusDeck keeps its data.
//!
//! Installed (NSIS): `%APPDATA%\com.focusdeck.app` and its `logs`.
//! Portable: a file named `portable` next to `focusdeck.exe` switches
//! everything — database, logs, WebView2 profile — to `<exe dir>\data`,
//! so the folder can live on a USB stick and leaves nothing in the profile
//! (apart from Credential Manager secrets, which stay per user).

use std::path::PathBuf;
use std::sync::OnceLock;

use tauri::{AppHandle, Manager, Runtime};

pub const PORTABLE_MARKER: &str = "portable";

/// `<exe dir>\data` when the portable marker exists.
pub fn portable_dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
        exe_dir.join(PORTABLE_MARKER).is_file().then(|| exe_dir.join("data"))
    })
    .clone()
}

pub fn is_portable() -> bool {
    portable_dir().is_some()
}

pub fn data_dir<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<PathBuf> {
    match portable_dir() {
        Some(d) => Ok(d),
        None => Ok(app.path().app_data_dir()?),
    }
}

pub fn log_dir<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<PathBuf> {
    match portable_dir() {
        Some(d) => Ok(d.join("logs")),
        None => Ok(app.path().app_log_dir()?),
    }
}

/// Before any window exists: keep the WebView2 profile inside the portable folder.
pub fn prepare_webview() {
    if let Some(d) = portable_dir() {
        let profile = d.join("webview");
        let _ = std::fs::create_dir_all(&profile);
        // SAFETY: called at the very start of `run`, before other threads exist.
        unsafe { std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", profile) };
    }
}
