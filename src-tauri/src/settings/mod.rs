//! Application settings, stored one key per row in the `settings` table.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_global_shortcut::Shortcut;

use crate::error::{AppError, AppResult};
use crate::permissions::{self, PERM_AUTOSTART, PERM_GLOBAL_HOTKEYS};
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub theme: Theme,
    pub language: String,
    /// Closing the window hides it to the tray instead of quitting.
    pub close_to_tray: bool,
    /// Start hidden in the tray (also implied by the `--minimized` flag used by autostart).
    pub start_minimized: bool,
    pub autostart: bool,
    pub global_hotkeys_enabled: bool,
    /// Global shortcut that shows/hides the window.
    pub toggle_window_hotkey: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            language: "ru".into(),
            close_to_tray: true,
            start_minimized: false,
            autostart: false,
            global_hotkeys_enabled: true,
            toggle_window_hotkey: "Ctrl+Alt+D".into(),
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> AppResult<()> {
        if self.language != "ru" {
            return Err(AppError::Invalid("поддерживается только русский язык".into()));
        }
        let hotkey = self.toggle_window_hotkey.trim();
        if !hotkey.is_empty() {
            Shortcut::from_str(hotkey)
                .map_err(|e| AppError::Invalid(format!("сочетание «{hotkey}» не распознано: {e}")))?;
        }
        Ok(())
    }
}

pub async fn load(pool: &SqlitePool) -> AppResult<AppSettings> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value_json FROM settings").fetch_all(pool).await?;
    let mut map = Map::new();
    for (key, json) in rows {
        // A corrupt value falls back to its default instead of breaking startup.
        if let Ok(v) = serde_json::from_str::<Value>(&json) {
            map.insert(key, v);
        }
    }
    Ok(serde_json::from_value(Value::Object(map)).unwrap_or_default())
}

pub async fn save(pool: &SqlitePool, settings: &AppSettings) -> AppResult<()> {
    let Value::Object(map) = serde_json::to_value(settings)? else { unreachable!() };
    let mut tx = pool.begin().await?;
    for (key, value) in map {
        sqlx::query(
            "INSERT INTO settings (key, value_json) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = datetime('now')",
        )
        .bind(key)
        .bind(value.to_string())
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> AppSettings {
    state.settings()
}

/// Validates and saves settings, then applies side effects (autostart, hotkeys).
#[tauri::command]
pub async fn settings_update(app: AppHandle, state: State<'_, AppState>, settings: AppSettings) -> AppResult<AppSettings> {
    settings.validate()?;
    let old = state.settings();

    // Turning a feature on in Settings is the user's consent for its permission.
    if settings.autostart && !old.autostart {
        permissions::ensure_granted(&state.db, PERM_AUTOSTART, "settings").await?;
    }
    if settings.global_hotkeys_enabled && !old.global_hotkeys_enabled {
        permissions::ensure_granted(&state.db, PERM_GLOBAL_HOTKEYS, "settings").await?;
    }

    if settings.autostart != old.autostart {
        crate::autostart::set(&app, settings.autostart)?;
    }

    *state.settings.write().unwrap() = settings.clone();
    // A hotkey taken by another program must not be saved: roll back.
    if let Err(e) = crate::hotkeys::sync(&app).await {
        tracing::warn!("hotkey registration failed, reverting settings: {e}");
        if settings.autostart != old.autostart {
            let _ = crate::autostart::set(&app, old.autostart);
        }
        *state.settings.write().unwrap() = old;
        let _ = crate::hotkeys::sync(&app).await;
        return Err(e);
    }
    save(&state.db, &settings).await?;
    tracing::info!("settings updated");
    Ok(app.state::<AppState>().settings())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn round_trip_and_defaults() {
        let pool = crate::db::test_pool().await;
        assert_eq!(load(&pool).await.unwrap(), AppSettings::default());

        let mut s = AppSettings::default();
        s.theme = Theme::Light;
        s.close_to_tray = false;
        save(&pool, &s).await.unwrap();
        assert_eq!(load(&pool).await.unwrap(), s);
    }

    #[tokio::test]
    async fn unknown_and_corrupt_values_fall_back() {
        let pool = crate::db::test_pool().await;
        sqlx::query("INSERT INTO settings (key, value_json) VALUES ('theme', 'not json'), ('legacy', '1')")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(load(&pool).await.unwrap().theme, Theme::Dark);
    }

    #[test]
    fn validates_hotkey_and_language() {
        let mut s = AppSettings::default();
        assert!(s.validate().is_ok());
        s.toggle_window_hotkey = "Ctrl+Banana+Q".into();
        assert!(s.validate().is_err());
        let mut s = AppSettings::default();
        s.language = "en".into();
        assert!(s.validate().is_err());
    }
}
