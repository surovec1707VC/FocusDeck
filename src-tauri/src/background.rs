//! Background tasks owned by modules. A module's task runs only while the
//! module is enabled; disabling the module aborts it.

use tauri::{AppHandle, Manager};

use crate::modules::catalog::{AUTOMATIONS, CLIPBOARD, TIMERS};
use crate::state::AppState;

fn spawn_for(app: &AppHandle, module_id: &str) -> Option<tauri::async_runtime::JoinHandle<()>> {
    match module_id {
        AUTOMATIONS => Some(tauri::async_runtime::spawn(crate::automation::engine::scheduler_loop(app.clone()))),
        TIMERS => Some(tauri::async_runtime::spawn(crate::productivity::timers::tick_loop(app.clone()))),
        CLIPBOARD => Some(tauri::async_runtime::spawn(crate::widgets::clipboard::poll_loop(app.clone()))),
        _ => None,
    }
}

/// Modules that own a background task.
pub const OWNERS: &[&str] = &[AUTOMATIONS, CLIPBOARD, TIMERS];

/// Starts or stops a module's task to match its enabled state.
pub fn reconcile(app: &AppHandle, module_id: &str) {
    if !OWNERS.contains(&module_id) {
        return;
    }
    let state = app.state::<AppState>();
    let enabled = state.module_enabled(module_id);
    let mut tasks = state.background.lock().unwrap();
    match (enabled, tasks.contains_key(module_id)) {
        (true, false) => {
            if let Some(handle) = spawn_for(app, module_id) {
                tasks.insert(module_id.to_string(), handle);
                tracing::info!(module = module_id, "background task started");
            }
        }
        (false, true) => {
            if let Some(handle) = tasks.remove(module_id) {
                handle.abort();
                tracing::info!(module = module_id, "background task stopped");
            }
        }
        _ => {}
    }
}

/// Restarts a running task (e.g. after its settings changed).
pub fn restart(app: &AppHandle, module_id: &str) {
    let state = app.state::<AppState>();
    if let Some(handle) = state.background.lock().unwrap().remove(module_id) {
        handle.abort();
    }
    reconcile(app, module_id);
}

pub fn running(app: &AppHandle, module_id: &str) -> bool {
    app.state::<AppState>().background.lock().unwrap().contains_key(module_id)
}

pub fn reconcile_all(app: &AppHandle) {
    for id in OWNERS {
        reconcile(app, id);
    }
}
