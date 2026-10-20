//! Productivity modules: notes, tasks, snippets, timers (with stopwatches).
//! Each is its own module in the registry; commands fail with
//! `module_disabled` while the module is off. All data is in SQLite.

pub mod notes;
pub mod quick_add;
pub mod snippets;
pub mod tasks;
pub mod timers;

use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::error::{AppError, AppResult};
use crate::modules::registry;
use crate::state::AppState;

/// `module_disabled` unless the module is on.
pub fn require(state: &AppState, module: &str) -> AppResult<()> {
    if state.module_enabled(module) {
        Ok(())
    } else {
        Err(AppError::ModuleDisabled(registry::def(module).map(|d| d.name).unwrap_or(module).into()))
    }
}

pub const MAX_TAGS: usize = 20;

/// Trims, drops `#`, lowercases, dedupes; each tag 1–30 characters.
pub fn normalize_tags(tags: &[String]) -> AppResult<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t = t.trim().trim_start_matches('#').trim().to_lowercase();
        if t.is_empty() {
            continue;
        }
        if t.chars().count() > 30 {
            return Err(AppError::Invalid(format!("тег «{t}» длиннее 30 символов")));
        }
        if !out.contains(&t) {
            out.push(t);
        }
    }
    if out.len() > MAX_TAGS {
        return Err(AppError::Invalid(format!("не больше {MAX_TAGS} тегов")));
    }
    Ok(out)
}

pub fn parse_tags(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap_or_default()
}

/// Characters Windows doesn't allow in file names.
pub fn safe_file_name(name: &str, fallback: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c })
        .collect::<String>()
        .trim()
        .trim_end_matches('.')
        .chars()
        .take(80)
        .collect();
    if cleaned.is_empty() { fallback.to_string() } else { cleaned }
}

/// «Save as» dialog; `None` when cancelled. Runs off the async runtime.
pub async fn pick_save_path(app: &AppHandle, title: &str, file_name: String, filter: (&'static str, &'static [&'static str])) -> AppResult<Option<PathBuf>> {
    let app = app.clone();
    let title = title.to_string();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file().set_title(title).set_file_name(file_name).add_filter(filter.0, filter.1).blocking_save_file()
    })
    .await
    .map_err(anyhow::Error::from)?;
    picked.map(|p| p.into_path().map_err(|e| AppError::Other(e.into()))).transpose()
}

/// «Open» dialog; returns the file content (up to `max_bytes`).
pub async fn pick_open_text(app: &AppHandle, title: &str, filter: (&'static str, &'static [&'static str]), max_bytes: u64) -> AppResult<Option<String>> {
    let app = app.clone();
    let title = title.to_string();
    let picked = tauri::async_runtime::spawn_blocking(move || app.dialog().file().set_title(title).add_filter(filter.0, filter.1).blocking_pick_file())
        .await
        .map_err(anyhow::Error::from)?;
    let Some(picked) = picked else { return Ok(None) };
    let path = picked.into_path().map_err(anyhow::Error::from)?;
    let size = std::fs::metadata(&path).map_err(anyhow::Error::from)?.len();
    if size > max_bytes {
        return Err(AppError::Invalid(format!("файл больше {} КБ", max_bytes / 1024)));
    }
    Ok(Some(std::fs::read_to_string(&path).map_err(|e| AppError::Invalid(format!("не удалось прочитать файл: {e}")))?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_and_names() {
        let tags = normalize_tags(&["#Работа".into(), " работа ".into(), "".into(), "Идеи".into()]).unwrap();
        assert_eq!(tags, ["работа", "идеи"]);
        assert!(normalize_tags(&["x".repeat(31)]).is_err());
        assert_eq!(safe_file_name("План: Q4/2026?", "note"), "План_ Q4_2026_");
        assert_eq!(safe_file_name("  ..", "note"), "note");
    }
}
