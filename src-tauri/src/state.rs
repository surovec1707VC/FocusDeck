use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, RwLock};

use sqlx::SqlitePool;
use tauri::async_runtime::JoinHandle;

use crate::hotkeys::HotkeyTarget;
use crate::settings::AppSettings;

/// Shared state managed by Tauri (`State<'_, AppState>`).
pub struct AppState {
    pub db: SqlitePool,
    /// Cached copy of the `settings` table; updated on every save.
    pub settings: RwLock<AppSettings>,
    /// Registered global shortcuts by shortcut id.
    pub hotkeys: Mutex<HashMap<u32, HotkeyTarget>>,
    /// Ids of enabled modules (cache of `modules.enabled` for sync code paths).
    pub enabled_modules: RwLock<HashSet<String>>,
    /// `requires_restart` modules changed since start.
    pub pending_restart: Mutex<HashSet<String>>,
    /// Background tasks by owning module id.
    pub background: Mutex<HashMap<String, JoinHandle<()>>>,
}

impl AppState {
    pub fn new(db: SqlitePool, settings: AppSettings, enabled_modules: HashSet<String>) -> Self {
        Self {
            db,
            settings: RwLock::new(settings),
            hotkeys: Mutex::new(HashMap::new()),
            enabled_modules: RwLock::new(enabled_modules),
            pending_restart: Mutex::new(HashSet::new()),
            background: Mutex::new(HashMap::new()),
        }
    }

    pub fn settings(&self) -> AppSettings {
        self.settings.read().unwrap().clone()
    }

    pub fn module_enabled(&self, id: &str) -> bool {
        self.enabled_modules.read().unwrap().contains(id)
    }

    pub fn pending_restart(&self) -> HashSet<String> {
        self.pending_restart.lock().unwrap().clone()
    }
}
