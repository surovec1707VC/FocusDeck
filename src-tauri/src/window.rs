//! Main window helpers: show / hide to tray / toggle.

use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

pub const MAIN: &str = "main";

/// Window state persisted between runs. Visibility is driven by the tray and
/// start-minimized setting; decorations stay off (custom title bar).
pub const RESTORED_STATE: StateFlags = StateFlags::all()
    .difference(StateFlags::VISIBLE)
    .difference(StateFlags::DECORATIONS);

fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(MAIN)
}

pub fn show(app: &AppHandle) {
    if let Some(w) = main_window(app) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn hide(app: &AppHandle) {
    // Persist position/size now: a hidden window may be killed with the session.
    let _ = app.save_window_state(RESTORED_STATE);
    if let Some(w) = main_window(app) {
        let _ = w.hide();
    }
}

pub fn toggle(app: &AppHandle) {
    let visible = main_window(app)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    let minimized = main_window(app)
        .and_then(|w| w.is_minimized().ok())
        .unwrap_or(false);
    if visible && !minimized {
        hide(app);
    } else {
        show(app);
    }
}
