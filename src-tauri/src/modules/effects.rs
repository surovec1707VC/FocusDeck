//! What happens outside the database when a module is turned on/off or its
//! settings change. Settings themselves are always kept.

use tauri::{AppHandle, Emitter, Manager};

use super::catalog::{AUTOMATIONS, AUTOSTART, HOTKEYS, TRAY};
use super::registry;
use crate::error::AppResult;
use crate::state::AppState;

/// Emitted after any module change so every page can refresh.
pub const CHANGED_EVENT: &str = "focusdeck://modules-changed";

pub fn notify_changed(app: &AppHandle) {
    let _ = app.emit(CHANGED_EVENT, ());
}

pub async fn refresh_cache(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let set = registry::enabled_set(&state.db).await?;
    *state.enabled_modules.write().unwrap() = set;
    Ok(())
}

/// Applies a module's enabled state. Returns warnings that didn't stop the change.
pub async fn apply_enabled(app: &AppHandle, id: &str) -> AppResult<Vec<String>> {
    refresh_cache(app).await?;
    let state = app.state::<AppState>();
    let enabled = state.module_enabled(id);
    let mut warnings = Vec::new();

    match id {
        // Hidden widgets need nothing here: widget lists are filtered by module.
        HOTKEYS => {
            if let Err(e) = crate::hotkeys::sync(app).await {
                warnings.push(e.to_string());
            }
        }
        AUTOMATIONS => {
            crate::background::reconcile(app, AUTOMATIONS);
            if let Err(e) = crate::hotkeys::sync(app).await {
                warnings.push(e.to_string());
            }
        }
        TRAY => crate::tray::set_visible(app, enabled),
        AUTOSTART => {
            // Disabling removes the OS entry; the setting is kept and re-applied later.
            let wanted = enabled && state.settings().autostart;
            if let Err(e) = crate::autostart::set(app, wanted) {
                warnings.push(e.to_string());
            }
        }
        _ => {
            // Any module can own automation hotkeys.
            if let Err(e) = crate::hotkeys::sync(app).await {
                warnings.push(e.to_string());
            }
        }
    }

    if registry::def(id)?.requires_restart {
        state.pending_restart.lock().unwrap().insert(id.to_string());
    }
    for w in &warnings {
        registry::log(&state.db, id, "warn", "apply", w, "system").await?;
    }
    notify_changed(app);
    Ok(warnings)
}

/// Applies changed settings of a module.
pub async fn apply_settings(app: &AppHandle, id: &str) -> AppResult<()> {
    let state = app.state::<AppState>();
    if id == AUTOMATIONS {
        // New scheduler period.
        crate::background::restart(app, AUTOMATIONS);
    }
    if registry::def(id)?.requires_restart {
        state.pending_restart.lock().unwrap().insert(id.to_string());
    }
    notify_changed(app);
    Ok(())
}
