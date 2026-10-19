//! Registration of official Tauri plugins.
//!
//! Each plugin has its own wrapper so it can be configured (or later gated
//! by the Module Registry) in one place. `register` wires them all in.

use tauri::{plugin::Plugin, Builder, Runtime};

pub fn register<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    let builder = builder
        .plugin(log())
        .plugin(store())
        .plugin(shell())
        .plugin(fs())
        .plugin(http())
        .plugin(clipboard_manager())
        .plugin(notification())
        .plugin(dialog())
        .plugin(opener());

    #[cfg(desktop)]
    let builder = builder
        .plugin(window_state())
        .plugin(global_shortcut())
        .plugin(autostart());

    builder
}

pub fn log<R: Runtime>() -> impl Plugin<R> {
    tauri_plugin_log::Builder::new()
        .level(tauri_plugin_log::log::LevelFilter::Info)
        .build()
}

pub fn store<R: Runtime>() -> impl Plugin<R> {
    tauri_plugin_store::Builder::new().build()
}

pub fn shell<R: Runtime>() -> impl Plugin<R> {
    tauri_plugin_shell::init()
}

pub fn fs<R: Runtime>() -> impl Plugin<R> {
    tauri_plugin_fs::init()
}

pub fn http<R: Runtime>() -> impl Plugin<R> {
    tauri_plugin_http::init()
}

pub fn clipboard_manager<R: Runtime>() -> impl Plugin<R> {
    tauri_plugin_clipboard_manager::init()
}

pub fn notification<R: Runtime>() -> impl Plugin<R> {
    tauri_plugin_notification::init()
}

pub fn dialog<R: Runtime>() -> impl Plugin<R> {
    tauri_plugin_dialog::init()
}

pub fn opener<R: Runtime>() -> impl Plugin<R> {
    tauri_plugin_opener::init()
}

#[cfg(desktop)]
pub fn window_state<R: Runtime>() -> impl Plugin<R> {
    tauri_plugin_window_state::Builder::new().build()
}

#[cfg(desktop)]
pub fn global_shortcut<R: Runtime>() -> impl Plugin<R> {
    tauri_plugin_global_shortcut::Builder::new().build()
}

#[cfg(desktop)]
pub fn autostart<R: Runtime>() -> impl Plugin<R> {
    tauri_plugin_autostart::Builder::new().build()
}
