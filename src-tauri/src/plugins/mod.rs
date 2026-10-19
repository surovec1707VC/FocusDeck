//! Registration of Tauri plugins. Only local, offline plugins are used.

use tauri::{Builder, Wry};
use tauri_plugin_autostart::MacosLauncher;

use crate::autostart::MINIMIZED_FLAG;
use crate::window::RESTORED_STATE;

pub fn register(builder: Builder<Wry>) -> Builder<Wry> {
    builder
        // Must be first: a second launch focuses the running instance instead.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            crate::window::show(app)
        }))
        // Position/size/maximized are restored; visibility and decorations are ours.
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(RESTORED_STATE)
                .build(),
        )
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| crate::hotkeys::handle(app, shortcut, event))
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![MINIMIZED_FLAG]),
        ))
        .plugin(tauri_plugin_opener::init())
        // File dialogs for configuration export/import.
        .plugin(tauri_plugin_dialog::init())
}
