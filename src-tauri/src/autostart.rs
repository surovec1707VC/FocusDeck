//! Optional start with Windows (registry Run key via tauri-plugin-autostart).
//! The app is launched with `--minimized`, so it starts in the tray.

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::error::AppResult;

pub const MINIMIZED_FLAG: &str = "--minimized";

pub fn set(app: &AppHandle, enabled: bool) -> AppResult<()> {
    let launcher = app.autolaunch();
    let result = if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    };
    result.map_err(|e| {
        anyhow::anyhow!(
            "не удалось {} автозапуск: {e}",
            if enabled {
                "включить"
            } else {
                "выключить"
            }
        )
    })?;
    tracing::info!(enabled, "autostart changed");
    Ok(())
}

pub fn is_enabled(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// Makes the OS state match the saved setting (e.g. after the entry was removed manually).
pub fn reconcile(app: &AppHandle, wanted: bool) {
    if is_enabled(app) != wanted {
        if let Err(e) = set(app, wanted) {
            tracing::warn!("{e}");
        }
    }
}
