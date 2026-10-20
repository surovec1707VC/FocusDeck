//! Workspace: layouts (profiles, presets, hotkeys) and the task list.
//! Widgets on a layout live in `crate::widgets`.

use std::str::FromStr;

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::Shortcut;

use crate::db::new_id;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::widgets::{self, grid::Rect};

/// Emitted when the active layout changes outside the UI (hotkey).
pub const LAYOUT_EVENT: &str = "focusdeck://layout-changed";

pub const PROFILES: &[&str] = &["work", "coding", "streaming", "gaming", "focus", "custom"];

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Layout {
    pub id: String,
    pub name: String,
    /// work | coding | streaming | gaming | focus | custom
    pub profile: String,
    /// Global shortcut that switches to this layout.
    pub hotkey: Option<String>,
    pub is_active: bool,
    pub position: i64,
    pub widget_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub done: bool,
    pub position: i64,
    pub created_at: String,
}

type Preset = (&'static str, &'static str, &'static [(&'static str, i64, i64, i64, i64)]);

/// Ready-made layouts by profile: (profile, name, [(kind, x, y, w, h)]) on a 12-column grid.
const PRESETS: &[Preset] = &[
    ("work", "Работа", &[
        ("clock", 0, 0, 3, 2), ("date", 3, 0, 3, 2), ("calendar", 6, 0, 6, 5),
        ("todo", 0, 2, 6, 5), ("notes", 6, 5, 6, 4), ("notifications", 0, 7, 6, 4),
    ]),
    ("coding", "Программирование", &[
        ("system_info", 0, 0, 4, 4), ("stopwatch", 4, 0, 4, 3), ("clipboard", 8, 0, 4, 6),
        ("todo", 4, 3, 4, 5), ("quick_links", 0, 4, 4, 3), ("notes", 0, 7, 4, 4),
    ]),
    ("streaming", "Стрим", &[
        ("clock", 0, 0, 3, 2), ("countdown", 3, 0, 3, 3), ("system_info", 6, 0, 6, 4),
        ("notifications", 0, 3, 6, 5), ("app_launcher", 6, 4, 3, 3), ("quick_links", 9, 4, 3, 3),
    ]),
    ("gaming", "Игры", &[
        ("system_info", 0, 0, 6, 4), ("clock", 6, 0, 3, 2), ("stopwatch", 9, 0, 3, 3),
        ("app_launcher", 6, 2, 3, 4), ("weather", 0, 4, 6, 3),
    ]),
    ("focus", "Фокус", &[
        ("clock", 0, 0, 4, 3), ("countdown", 4, 0, 4, 3), ("todo", 8, 0, 4, 6), ("notes", 0, 3, 8, 4),
    ]),
    ("custom", "Свой", &[]),
];

const PRESETS_SEEDED: &str = "layouts.presets_seeded";

pub fn check_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(AppError::Invalid("название должно быть от 1 до 60 символов".into()));
    }
    Ok(name.to_string())
}

fn check_profile(profile: &str) -> AppResult<()> {
    if PROFILES.contains(&profile) {
        Ok(())
    } else {
        Err(AppError::Invalid(format!("неизвестный профиль макета {profile}")))
    }
}

/// Creates a layout from its profile preset (empty for `custom`).
pub async fn create(pool: &SqlitePool, name: &str, profile: &str) -> AppResult<String> {
    check_profile(profile)?;
    let name = check_name(name)?;
    let id = new_id();
    sqlx::query(
        "INSERT INTO widget_layouts (id, name, profile, is_active, position)
         VALUES (?, ?, ?, 0, (SELECT COALESCE(MAX(position), -1) + 1 FROM widget_layouts))",
    )
    .bind(&id)
    .bind(&name)
    .bind(profile)
    .execute(pool)
    .await?;
    let (_, _, widgets_of) = PRESETS.iter().find(|p| p.0 == profile).unwrap();
    for (kind, x, y, w, h) in widgets_of.iter() {
        widgets::insert(pool, &id, kind, None, Some(Rect { x: *x, y: *y, w: *w, h: *h })).await?;
    }
    Ok(id)
}

/// First start: a «Основной» layout. Once: one layout per profile preset.
pub async fn seed(pool: &SqlitePool) -> AppResult<()> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM widget_layouts").fetch_one(pool).await?;
    if count == 0 {
        let id = new_id();
        sqlx::query("INSERT INTO widget_layouts (id, name, profile, is_active) VALUES (?, 'Основной', 'custom', 1)")
            .bind(&id)
            .execute(pool)
            .await?;
        for (kind, x, y, w, h) in [("clock", 0, 0, 3, 2), ("todo", 3, 0, 6, 4), ("status", 9, 0, 3, 3)] {
            widgets::insert(pool, &id, kind, None, Some(Rect { x, y, w, h })).await?;
        }
    }
    let seeded: Option<(String,)> = sqlx::query_as("SELECT value_json FROM settings WHERE key = ?")
        .bind(PRESETS_SEEDED)
        .fetch_optional(pool)
        .await?;
    if seeded.is_none() {
        for (profile, name, _) in PRESETS.iter().filter(|p| p.0 != "custom") {
            let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM widget_layouts WHERE profile = ?")
                .bind(profile)
                .fetch_optional(pool)
                .await?;
            if exists.is_none() {
                create(pool, name, profile).await?;
            }
        }
        sqlx::query("INSERT INTO settings (key, value_json) VALUES (?, 'true')").bind(PRESETS_SEEDED).execute(pool).await?;
    }
    Ok(())
}

pub async fn layouts(pool: &SqlitePool) -> AppResult<Vec<Layout>> {
    Ok(sqlx::query_as::<_, Layout>(
        "SELECT l.id, l.name, l.profile, l.hotkey, l.is_active, l.position,
            (SELECT COUNT(*) FROM widgets w WHERE w.layout_id = l.id) AS widget_count, l.created_at, l.updated_at
         FROM widget_layouts l ORDER BY l.position, l.created_at, l.rowid",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Layout> {
    layouts(pool).await?.into_iter().find(|l| l.id == id).ok_or_else(|| AppError::NotFound("макет".into()))
}

pub async fn active_layout_id(pool: &SqlitePool) -> AppResult<String> {
    let row: Option<(String,)> = sqlx::query_as("SELECT id FROM widget_layouts WHERE is_active = 1 LIMIT 1")
        .fetch_optional(pool)
        .await?;
    row.map(|r| r.0).ok_or_else(|| AppError::NotFound("активный макет".into()))
}

pub async fn activate(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    let found = sqlx::query("UPDATE widget_layouts SET is_active = 1, updated_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if found == 0 {
        return Err(AppError::NotFound("макет".into()));
    }
    sqlx::query("UPDATE widget_layouts SET is_active = 0 WHERE id != ?").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn rename(pool: &SqlitePool, id: &str, name: &str) -> AppResult<()> {
    let n = sqlx::query("UPDATE widget_layouts SET name = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(check_name(name)?)
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound("макет".into()));
    }
    Ok(())
}

/// Copies a layout with all its widgets (without the hotkey: it must stay unique).
pub async fn duplicate(pool: &SqlitePool, id: &str) -> AppResult<String> {
    let src = get(pool, id).await?;
    let mut name: String = format!("{} (копия)", src.name);
    if name.chars().count() > 60 {
        name = format!("{}… (копия)", src.name.chars().take(48).collect::<String>());
    }
    let new = new_id();
    sqlx::query(
        "INSERT INTO widget_layouts (id, name, profile, is_active, position)
         VALUES (?, ?, ?, 0, (SELECT COALESCE(MAX(position), -1) + 1 FROM widget_layouts))",
    )
    .bind(&new)
    .bind(&name)
    .bind(&src.profile)
    .execute(pool)
    .await?;
    widgets::copy_all(pool, id, &new).await?;
    Ok(new)
}

pub async fn delete_layout(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let all = layouts(pool).await?;
    if all.len() <= 1 {
        return Err(AppError::Invalid("нельзя удалить последний макет".into()));
    }
    let target = all.iter().find(|l| l.id == id).ok_or_else(|| AppError::NotFound("макет".into()))?;
    let was_active = target.is_active;
    // Notes belong to their widgets: delete them with the layout.
    sqlx::query(
        "DELETE FROM notes WHERE id IN (SELECT json_extract(config_json, '$.note_id') FROM widgets
         WHERE layout_id = ? AND kind = 'notes')",
    )
    .bind(id)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM widget_layouts WHERE id = ?").bind(id).execute(pool).await?;
    if was_active {
        let next = all.iter().find(|l| l.id != id).unwrap();
        activate(pool, &next.id).await?;
    }
    Ok(())
}

/// Same physical keys? (`Ctrl+Alt+1` == `alt+ctrl+1`)
fn same_shortcut(a: &str, b: &str) -> bool {
    match (Shortcut::from_str(a.trim()), Shortcut::from_str(b.trim())) {
        (Ok(a), Ok(b)) => a.id() == b.id(),
        _ => false,
    }
}

/// Validates a layout hotkey: parseable and not used anywhere else in FocusDeck.
pub async fn check_hotkey(pool: &SqlitePool, state: &AppState, layout_id: &str, hotkey: &str) -> AppResult<()> {
    Shortcut::from_str(hotkey).map_err(|e| AppError::Invalid(format!("сочетание «{hotkey}» не распознано: {e}")))?;
    if same_shortcut(hotkey, &state.settings().toggle_window_hotkey) {
        return Err(AppError::Invalid(format!("«{hotkey}» уже показывает и скрывает окно")));
    }
    for l in layouts(pool).await?.iter().filter(|l| l.id != layout_id) {
        if l.hotkey.as_deref().is_some_and(|h| same_shortcut(h, hotkey)) {
            return Err(AppError::Invalid(format!("«{hotkey}» уже назначено макету «{}»", l.name)));
        }
    }
    for a in crate::automation::store::list(pool).await? {
        if let crate::automation::types::Trigger::Hotkey { shortcut } = &a.trigger {
            if same_shortcut(shortcut, hotkey) {
                return Err(AppError::Invalid(format!("«{hotkey}» уже используется автоматизацией «{}»", a.name)));
            }
        }
    }
    Ok(())
}

async fn store_hotkey(pool: &SqlitePool, id: &str, hotkey: Option<&str>) -> AppResult<()> {
    sqlx::query("UPDATE widget_layouts SET hotkey = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(hotkey)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Switches layouts from a global hotkey and tells the UI.
pub async fn activate_from_hotkey(app: &AppHandle, id: &str) -> AppResult<()> {
    let db = &app.state::<AppState>().db;
    activate(db, id).await?;
    let layout = get(db, id).await?;
    tracing::info!(layout = %layout.name, "layout switched by hotkey");
    app.emit(LAYOUT_EVENT, &layout).map_err(anyhow::Error::from)?;
    Ok(())
}

// --- tasks ---

pub async fn tasks(pool: &SqlitePool) -> AppResult<Vec<Task>> {
    Ok(sqlx::query_as::<_, Task>("SELECT id, title, done, position, created_at FROM tasks ORDER BY done, position, created_at")
        .fetch_all(pool)
        .await?)
}

pub async fn add_task(pool: &SqlitePool, title: &str) -> AppResult<()> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 200 {
        return Err(AppError::Invalid("текст задачи должен быть от 1 до 200 символов".into()));
    }
    sqlx::query("INSERT INTO tasks (id, title, position) VALUES (?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM tasks))")
        .bind(new_id())
        .bind(title)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn toggle_task(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("UPDATE tasks SET done = NOT done, updated_at = datetime('now') WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn delete_task(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM tasks WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

// --- commands: layouts ---

#[tauri::command]
pub async fn layouts_list(state: State<'_, AppState>) -> AppResult<Vec<Layout>> {
    layouts(&state.db).await
}

/// `profile` picks the preset widgets (`custom` = empty).
#[tauri::command]
pub async fn layouts_create(state: State<'_, AppState>, name: String, profile: Option<String>) -> AppResult<Vec<Layout>> {
    create(&state.db, &name, profile.as_deref().unwrap_or("custom")).await?;
    layouts(&state.db).await
}

#[tauri::command]
pub async fn layouts_rename(state: State<'_, AppState>, id: String, name: String) -> AppResult<Vec<Layout>> {
    rename(&state.db, &id, &name).await?;
    layouts(&state.db).await
}

#[tauri::command]
pub async fn layouts_duplicate(state: State<'_, AppState>, id: String) -> AppResult<Vec<Layout>> {
    duplicate(&state.db, &id).await?;
    layouts(&state.db).await
}

#[tauri::command]
pub async fn layouts_activate(state: State<'_, AppState>, id: String) -> AppResult<Vec<Layout>> {
    activate(&state.db, &id).await?;
    layouts(&state.db).await
}

#[tauri::command]
pub async fn layouts_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<Vec<Layout>> {
    let had_hotkey = get(&state.db, &id).await?.hotkey.is_some();
    delete_layout(&state.db, &id).await?;
    if had_hotkey {
        crate::hotkeys::sync(&app).await?;
    }
    layouts(&state.db).await
}

/// Sets (or clears with `null` / "") the layout's global hotkey. A shortcut
/// taken by another program is not saved.
#[tauri::command]
pub async fn layouts_set_hotkey(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    hotkey: Option<String>,
) -> AppResult<Vec<Layout>> {
    let db = &state.db;
    let old = get(db, &id).await?.hotkey;
    let hotkey = hotkey.map(|h| h.trim().to_string()).filter(|h| !h.is_empty());
    if let Some(h) = &hotkey {
        check_hotkey(db, &state, &id, h).await?;
    }
    store_hotkey(db, &id, hotkey.as_deref()).await?;
    if let Err(e) = crate::hotkeys::sync(&app).await {
        tracing::warn!("{e}");
    }
    if let Some(h) = &hotkey {
        if crate::hotkeys::active(&app).await? && !crate::hotkeys::layout_registered(&app, &id) {
            store_hotkey(db, &id, old.as_deref()).await?;
            let _ = crate::hotkeys::sync(&app).await;
            return Err(AppError::Invalid(format!("сочетание «{h}» занято другой программой")));
        }
    }
    layouts(db).await
}

// --- commands: tasks ---

#[tauri::command]
pub async fn tasks_list(state: State<'_, AppState>) -> AppResult<Vec<Task>> {
    tasks(&state.db).await
}

#[tauri::command]
pub async fn tasks_add(state: State<'_, AppState>, title: String) -> AppResult<Vec<Task>> {
    add_task(&state.db, &title).await?;
    tasks(&state.db).await
}

#[tauri::command]
pub async fn tasks_toggle(state: State<'_, AppState>, id: String) -> AppResult<Vec<Task>> {
    toggle_task(&state.db, &id).await?;
    tasks(&state.db).await
}

#[tauri::command]
pub async fn tasks_delete(state: State<'_, AppState>, id: String) -> AppResult<Vec<Task>> {
    delete_task(&state.db, &id).await?;
    tasks(&state.db).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn seed_creates_default_and_profile_layouts_once() {
        let pool = crate::db::test_pool().await;
        seed(&pool).await.unwrap();
        let all = layouts(&pool).await.unwrap();
        let profiles: Vec<&str> = all.iter().map(|l| l.profile.as_str()).collect();
        assert_eq!(profiles, ["custom", "work", "coding", "streaming", "gaming", "focus"]);
        assert_eq!(all.iter().filter(|l| l.is_active).count(), 1);
        assert_eq!(all[0].name, "Основной");
        assert_eq!(all[1].widget_count, 6);

        // Deleted presets don't come back.
        delete_layout(&pool, &all[5].id).await.unwrap();
        seed(&pool).await.unwrap();
        assert_eq!(layouts(&pool).await.unwrap().len(), 5);
    }

    #[tokio::test]
    async fn layout_rules() {
        let pool = crate::db::test_pool().await;
        seed(&pool).await.unwrap();
        let first = active_layout_id(&pool).await.unwrap();
        let second = create(&pool, "Второй", "custom").await.unwrap();
        assert_eq!(get(&pool, &second).await.unwrap().widget_count, 0);
        assert!(create(&pool, "x", "party").await.is_err());

        activate(&pool, &second).await.unwrap();
        assert_eq!(active_layout_id(&pool).await.unwrap(), second);
        rename(&pool, &second, "Вечер").await.unwrap();
        assert_eq!(get(&pool, &second).await.unwrap().name, "Вечер");

        // Deleting the active layout activates another one.
        delete_layout(&pool, &second).await.unwrap();
        assert_ne!(active_layout_id(&pool).await.unwrap(), second);
        assert!(layouts(&pool).await.unwrap().iter().any(|l| l.id == first));
    }

    #[tokio::test]
    async fn duplicate_copies_widgets_and_notes() {
        let pool = crate::db::test_pool().await;
        seed(&pool).await.unwrap();
        let work = layouts(&pool).await.unwrap().into_iter().find(|l| l.profile == "work").unwrap();
        sqlx::query("UPDATE widget_layouts SET hotkey = 'Ctrl+Alt+W' WHERE id = ?").bind(&work.id).execute(&pool).await.unwrap();
        let copy = get(&pool, &duplicate(&pool, &work.id).await.unwrap()).await.unwrap();
        assert_eq!(copy.name, "Работа (копия)");
        assert_eq!(copy.widget_count, work.widget_count);
        assert!(copy.hotkey.is_none());

        let notes = |layout: String| {
            let pool = pool.clone();
            async move {
                widgets::all(&pool, &layout).await.unwrap().into_iter().find(|w| w.kind == "notes").unwrap().config["note_id"].clone()
            }
        };
        assert_ne!(notes(work.id.clone()).await, notes(copy.id.clone()).await, "the copy has its own note");

        let (before,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notes").fetch_one(&pool).await.unwrap();
        delete_layout(&pool, &copy.id).await.unwrap();
        let (after,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notes").fetch_one(&pool).await.unwrap();
        assert_eq!(after, before - 1);
    }

    #[test]
    fn shortcuts_compare_by_keys() {
        assert!(same_shortcut("Ctrl+Alt+1", "alt+ctrl+1"));
        assert!(!same_shortcut("Ctrl+Alt+1", "Ctrl+Alt+2"));
    }
}
