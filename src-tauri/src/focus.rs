//! Focus Mode: while on, toasts are muted (notifications are still saved to
//! the history; timer notifications still show). Turned on by automations
//! or from the UI, for N minutes or until turned off. Not kept across restarts.

use std::sync::Mutex;

use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::error::AppResult;

pub const EVENT: &str = "focusdeck://focus-mode";

/// `None` = off, `Some(None)` = until turned off, `Some(Some(t))` = until epoch ms `t`.
static FOCUS: Mutex<Option<Option<i64>>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize)]
pub struct FocusState {
    pub active: bool,
    pub until: Option<i64>,
}

pub fn state() -> FocusState {
    let mut f = FOCUS.lock().unwrap();
    if let Some(Some(until)) = *f {
        if until <= Utc::now().timestamp_millis() {
            *f = None;
        }
    }
    FocusState { active: f.is_some(), until: f.flatten() }
}

pub fn set(app: &AppHandle, enabled: bool, minutes: u32) -> FocusState {
    *FOCUS.lock().unwrap() = enabled.then(|| (minutes > 0).then(|| Utc::now().timestamp_millis() + minutes as i64 * 60_000));
    let s = state();
    tracing::info!(active = s.active, "focus mode");
    let _ = app.emit(EVENT, &s);
    s
}

/// Toasts from these sources are shown even in Focus Mode.
pub fn allows_toast(source: &str) -> bool {
    !state().active || source == "Таймеры"
}

#[tauri::command]
pub fn focus_mode_get() -> FocusState {
    state()
}

#[tauri::command]
pub fn focus_mode_set(app: AppHandle, enabled: bool, minutes: Option<u32>) -> AppResult<FocusState> {
    Ok(set(&app, enabled, minutes.unwrap_or(0).min(24 * 60)))
}
