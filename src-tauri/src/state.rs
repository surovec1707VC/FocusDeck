use std::collections::HashMap;
use std::sync::{Mutex, RwLock};

use sqlx::SqlitePool;

use crate::hotkeys::HotkeyTarget;
use crate::settings::AppSettings;

/// Shared state managed by Tauri (`State<'_, AppState>`).
pub struct AppState {
    pub db: SqlitePool,
    /// Cached copy of the `settings` table; updated on every save.
    pub settings: RwLock<AppSettings>,
    /// Registered global shortcuts by shortcut id.
    pub hotkeys: Mutex<HashMap<u32, HotkeyTarget>>,
}

impl AppState {
    pub fn new(db: SqlitePool, settings: AppSettings) -> Self {
        Self { db, settings: RwLock::new(settings), hotkeys: Mutex::new(HashMap::new()) }
    }

    pub fn settings(&self) -> AppSettings {
        self.settings.read().unwrap().clone()
    }
}
