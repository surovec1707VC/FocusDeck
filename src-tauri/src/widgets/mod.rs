//! Widget system: widget instances on layouts, grid positions and state
//! flags, data providers and widget actions. All widget data reaches the UI
//! through the commands at the bottom of this file.

pub mod catalog;
pub mod clipboard;
pub mod data;
pub mod desktop;
pub mod grid;
pub mod system;
pub mod weather;

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::SqlitePool;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::db::new_id;
use crate::error::{AppError, AppResult};
use crate::modules::{catalog as modules, registry, types::ModulePermission};
use crate::state::AppState;
use catalog::{DataSource, WidgetCategory, WidgetDef};
use grid::Rect;

#[derive(sqlx::FromRow)]
struct WidgetRow {
    id: String,
    layout_id: String,
    kind: String,
    title: String,
    position: i64,
    config_json: String,
    x: i64,
    y: i64,
    w: i64,
    h: i64,
    pinned: bool,
    collapsed: bool,
    hidden: bool,
    desktop: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Widget {
    pub id: String,
    pub layout_id: String,
    pub kind: String,
    pub title: String,
    /// Order of creation; the grid position is `x`, `y`.
    pub position: i64,
    pub config: Value,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    /// Locked in place: not moved by dragging, resizing or compaction.
    pub pinned: bool,
    /// Only the title bar is shown.
    pub collapsed: bool,
    /// Not shown on the dashboard; can be shown again from the hidden list.
    pub hidden: bool,
    /// Detached into its own window on the Windows desktop (see `desktop`).
    pub desktop: bool,
}

impl Widget {
    pub fn rect(&self) -> Rect {
        Rect { x: self.x, y: self.y, w: self.w, h: self.h }
    }
}

impl TryFrom<WidgetRow> for Widget {
    type Error = AppError;
    fn try_from(r: WidgetRow) -> AppResult<Self> {
        Ok(Self {
            config: serde_json::from_str(&r.config_json)?,
            id: r.id,
            layout_id: r.layout_id,
            kind: r.kind,
            title: r.title,
            position: r.position,
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
            pinned: r.pinned,
            collapsed: r.collapsed,
            hidden: r.hidden,
            desktop: r.desktop,
        })
    }
}

macro_rules! select_widgets {
    ($tail:literal) => {
        concat!(
            "SELECT id, layout_id, kind, title, position, config_json, x, y, w, h, pinned, collapsed, hidden,
                EXISTS(SELECT 1 FROM desktop_widgets d WHERE d.widget_id = widgets.id) AS desktop FROM widgets ",
            $tail
        )
    };
}

#[derive(Debug, Default, Deserialize)]
pub struct WidgetPatch {
    pub title: Option<String>,
    /// Merged into the current config; state keys (timer state, note id) are ignored.
    pub config: Option<Value>,
    pub pinned: Option<bool>,
    pub collapsed: Option<bool>,
    pub hidden: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct GridItem {
    pub id: String,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

/// Config keys managed by the backend (widget state), not editable through a patch.
const STATE_KEYS: &[&str] = &[
    "note_id", "duration_ms", "label", "running", "ends_at", "remaining_ms", "finished", "started_at", "elapsed_ms", "laps",
];
const MAX_CONFIG_BYTES: usize = 64 * 1024;
const MAX_ENTRIES: usize = 30;

pub fn def(kind: &str) -> AppResult<&'static WidgetDef> {
    catalog::find(kind).ok_or_else(|| AppError::Invalid(format!("неизвестный тип виджета {kind}")))
}

pub fn check_title(title: &str) -> AppResult<String> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 60 {
        return Err(AppError::Invalid("название должно быть от 1 до 60 символов".into()));
    }
    Ok(title.to_string())
}

/// Grid columns from the Dashboard module settings.
pub async fn columns(pool: &SqlitePool) -> i64 {
    registry::settings(pool, modules::DASHBOARD)
        .await
        .ok()
        .and_then(|s| s.get("columns").and_then(Value::as_i64))
        .unwrap_or(12)
        .clamp(4, grid::MAX_COLUMNS)
}

/// Errors with `ModuleDisabled` if a module the widget needs is off.
pub fn require_available(state: &AppState, def: &WidgetDef) -> AppResult<()> {
    for m in def.modules() {
        if !state.module_enabled(m) {
            let name = registry::def(m).map(|d| d.name).unwrap_or(m);
            return Err(AppError::ModuleDisabled(name.into()));
        }
    }
    Ok(())
}

pub fn initial_config(kind: &str) -> Value {
    match kind {
        "quick_links" => json!({ "links": [] }),
        "app_launcher" => json!({ "apps": [] }),
        "countdown" => json!({ "duration_ms": 300_000, "label": "", "running": false, "remaining_ms": 300_000, "finished": false }),
        "stopwatch" => json!({ "elapsed_ms": 0, "started_at": null, "laps": [] }),
        _ => json!({}),
    }
}

/// Only web and mail links: nothing that could run a program.
pub fn valid_link(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    url.len() <= 2048
        && !url.chars().any(|c| c.is_control() || c.is_whitespace())
        && (lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("mailto:"))
        && lower.len() > "http://".len()
}

pub const APP_EXTENSIONS: &[&str] = &["exe", "lnk", "url", "appref-ms"];

pub fn valid_app_path(path: &str) -> AppResult<()> {
    let p = Path::new(path);
    let ext = p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    if !p.is_absolute() || !APP_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::Invalid(format!(
            "можно добавить только программу или ярлык ({}) с полным путём",
            APP_EXTENSIONS.iter().map(|e| format!(".{e}")).collect::<Vec<_>>().join(", ")
        )));
    }
    Ok(())
}

fn validate_config(kind: &str, config: &Value) -> AppResult<()> {
    let list = |key: &str| -> AppResult<Vec<Value>> {
        let items = config.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
        if items.len() > MAX_ENTRIES {
            return Err(AppError::Invalid(format!("не больше {MAX_ENTRIES} элементов")));
        }
        Ok(items)
    };
    let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).unwrap_or("").to_string();
    match kind {
        "quick_links" => {
            for l in list("links")? {
                check_title(&text(&l, "title"))?;
                if !valid_link(&text(&l, "url")) {
                    return Err(AppError::Invalid(format!(
                        "адрес «{}» не подходит: нужны http://, https:// или mailto:",
                        text(&l, "url")
                    )));
                }
            }
        }
        "app_launcher" => {
            for a in list("apps")? {
                check_title(&text(&a, "name"))?;
                valid_app_path(&text(&a, "path"))?;
            }
        }
        _ => {}
    }
    if let Some(a) = config.get("appearance").filter(|a| !a.is_null()) {
        validate_appearance(a)?;
    }
    if config.to_string().len() > MAX_CONFIG_BYTES {
        return Err(AppError::Invalid("настройки виджета слишком большие".into()));
    }
    Ok(())
}

pub const APPEARANCE_SHAPES: &[&str] = &["rounded", "square", "soft", "pill", "cut"];
pub const APPEARANCE_TEXTURES: &[&str] = &["none", "glass", "gradient", "dots", "grid", "lines", "noise"];

/// Look of a widget card: `{ shape, color, texture, opacity }`, every key optional.
fn validate_appearance(a: &Value) -> AppResult<()> {
    let bad = || AppError::Invalid("неверные параметры оформления виджета".into());
    let Value::Object(map) = a else { return Err(bad()) };
    let one_of = |key: &str, allowed: &[&str]| match map.get(key) {
        None | Some(Value::Null) => Ok(()),
        Some(Value::String(v)) if allowed.contains(&v.as_str()) => Ok(()),
        _ => Err(bad()),
    };
    one_of("shape", APPEARANCE_SHAPES)?;
    one_of("texture", APPEARANCE_TEXTURES)?;
    match map.get("color") {
        None | Some(Value::Null) => {}
        Some(Value::String(c)) if c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit()) => {}
        _ => return Err(bad()),
    }
    match map.get("opacity") {
        None | Some(Value::Null) => {}
        Some(v) if v.as_f64().is_some_and(|o| (0.2..=1.0).contains(&o)) => {}
        _ => return Err(bad()),
    }
    Ok(())
}

fn merge_config(kind: &str, current: &Value, patch: &Value) -> AppResult<Value> {
    let Value::Object(patch) = patch else {
        return Err(AppError::Invalid("настройки виджета должны быть объектом".into()));
    };
    let mut out: Map<String, Value> = current.as_object().cloned().unwrap_or_default();
    for (k, v) in patch {
        if !STATE_KEYS.contains(&k.as_str()) {
            out.insert(k.clone(), v.clone());
        }
    }
    let out = Value::Object(out);
    validate_config(kind, &out)?;
    Ok(out)
}

// --- storage ---

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Widget> {
    sqlx::query_as::<_, WidgetRow>(select_widgets!("WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("виджет".into()))?
        .try_into()
}

/// All widgets of a layout. Widgets without a position (new or migrated) are placed first.
pub async fn all(pool: &SqlitePool, layout_id: &str) -> AppResult<Vec<Widget>> {
    let mut list: Vec<Widget> = sqlx::query_as::<_, WidgetRow>(select_widgets!("WHERE layout_id = ? ORDER BY position, rowid"))
        .bind(layout_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(Widget::try_from)
        .collect::<AppResult<_>>()?;
    if list.iter().any(|w| w.x < 0 || w.y < 0) {
        let cols = columns(pool).await;
        let mut occupied: Vec<Rect> = list.iter().filter(|w| w.x >= 0 && w.y >= 0 && !w.hidden).map(Widget::rect).collect();
        for w in list.iter_mut().filter(|w| w.x < 0 || w.y < 0) {
            w.w = w.w.clamp(1, cols);
            (w.x, w.y) = grid::find_free(&occupied, w.w, w.h, cols);
            occupied.push(w.rect());
            sqlx::query("UPDATE widgets SET x = ?, y = ?, w = ? WHERE id = ?")
                .bind(w.x)
                .bind(w.y)
                .bind(w.w)
                .bind(&w.id)
                .execute(pool)
                .await?;
        }
    }
    Ok(list)
}

/// Widgets the user can see: those whose modules are all on. The others stay in the database.
pub async fn visible(pool: &SqlitePool, layout_id: &str) -> AppResult<Vec<Widget>> {
    let enabled = registry::enabled_set(pool).await?;
    Ok(all(pool, layout_id)
        .await?
        .into_iter()
        .filter(|w| catalog::find(&w.kind).is_some_and(|d| d.missing_modules(&enabled).is_empty()))
        .collect())
}

/// Adds a widget at `at`, or at the first free spot.
pub async fn insert(pool: &SqlitePool, layout_id: &str, kind: &str, title: Option<&str>, at: Option<Rect>) -> AppResult<Widget> {
    let def = def(kind)?;
    let title = check_title(title.unwrap_or(def.title))?;
    let rect = match at {
        Some(r) => r,
        None => {
            let cols = columns(pool).await;
            let occupied: Vec<Rect> = all(pool, layout_id).await?.iter().filter(|w| !w.hidden).map(Widget::rect).collect();
            let (w, h) = (def.default_size.0.min(cols), def.default_size.1);
            let (x, y) = grid::find_free(&occupied, w, h, cols);
            Rect { x, y, w, h }
        }
    };
    let id = new_id();
    let mut config = initial_config(kind);
    if kind == "notes" {
        sqlx::query("INSERT INTO notes (id, title) VALUES (?, ?)").bind(&id).bind(&title).execute(pool).await?;
        config["note_id"] = json!(id);
    }
    sqlx::query(
        "INSERT INTO widgets (id, layout_id, kind, title, config_json, x, y, w, h, position)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM widgets WHERE layout_id = ?))",
    )
    .bind(&id)
    .bind(layout_id)
    .bind(kind)
    .bind(&title)
    .bind(config.to_string())
    .bind(rect.x)
    .bind(rect.y)
    .bind(rect.w)
    .bind(rect.h)
    .bind(layout_id)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn set_config(pool: &SqlitePool, id: &str, config: &Value) -> AppResult<()> {
    sqlx::query("UPDATE widgets SET config_json = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(config.to_string())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update(pool: &SqlitePool, id: &str, patch: WidgetPatch) -> AppResult<Widget> {
    let current = get(pool, id).await?;
    let title = patch.title.as_deref().map(check_title).transpose()?;
    let config = patch.config.as_ref().map(|c| merge_config(&current.kind, &current.config, c)).transpose()?;
    sqlx::query(
        "UPDATE widgets SET title = COALESCE(?, title), config_json = COALESCE(?, config_json),
            pinned = COALESCE(?, pinned), collapsed = COALESCE(?, collapsed), hidden = COALESCE(?, hidden),
            updated_at = datetime('now') WHERE id = ?",
    )
    .bind(title)
    .bind(config.map(|c| c.to_string()))
    .bind(patch.pinned)
    .bind(patch.collapsed)
    .bind(patch.hidden)
    .bind(id)
    .execute(pool)
    .await?;

    // Shown again: move it if its old spot is taken now.
    if patch.hidden == Some(false) && current.hidden {
        let others: Vec<Rect> = all(pool, &current.layout_id).await?.iter().filter(|w| w.id != id && !w.hidden).map(Widget::rect).collect();
        if others.iter().any(|o| o.overlaps(&current.rect())) {
            let cols = columns(pool).await;
            let (x, y) = grid::find_free(&others, current.w.min(cols), current.h, cols);
            sqlx::query("UPDATE widgets SET x = ?, y = ? WHERE id = ?").bind(x).bind(y).bind(id).execute(pool).await?;
        }
    }
    get(pool, id).await
}

/// Saves positions and sizes after a drag or resize (one transaction).
pub async fn save_grid(pool: &SqlitePool, layout_id: &str, items: &[GridItem]) -> AppResult<()> {
    let kinds: Vec<(String, String)> = sqlx::query_as("SELECT id, kind FROM widgets WHERE layout_id = ?")
        .bind(layout_id)
        .fetch_all(pool)
        .await?;
    let mut tx = pool.begin().await?;
    for item in items {
        let kind = kinds
            .iter()
            .find(|(id, _)| *id == item.id)
            .map(|(_, k)| k.as_str())
            .ok_or_else(|| AppError::NotFound("виджет в этом макете".into()))?;
        let rect = Rect { x: item.x, y: item.y, w: item.w, h: item.h };
        grid::validate(&rect, def(kind)?.min_size)?;
        sqlx::query("UPDATE widgets SET x = ?, y = ?, w = ?, h = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(rect.x)
            .bind(rect.y)
            .bind(rect.w)
            .bind(rect.h)
            .bind(&item.id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Deletes a widget together with its own data (the note of a notes widget).
pub async fn remove(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let w = get(pool, id).await?;
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM widgets WHERE id = ?").bind(id).execute(&mut *tx).await?;
    if let Some(note) = w.config.get("note_id").and_then(Value::as_str) {
        sqlx::query("DELETE FROM notes WHERE id = ?").bind(note).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Copies every widget of a layout into another one (notes are copied too).
pub async fn copy_all(pool: &SqlitePool, from: &str, to: &str) -> AppResult<()> {
    for w in all(pool, from).await? {
        let id = new_id();
        let mut config = w.config.clone();
        if let Some(note) = w.config.get("note_id").and_then(Value::as_str) {
            sqlx::query("INSERT INTO notes (id, title, body) SELECT ?, title, body FROM notes WHERE id = ?")
                .bind(&id)
                .bind(note)
                .execute(pool)
                .await?;
            config["note_id"] = json!(id);
        }
        sqlx::query(
            "INSERT INTO widgets (id, layout_id, kind, title, config_json, x, y, w, h, pinned, collapsed, hidden, position)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(to)
        .bind(&w.kind)
        .bind(&w.title)
        .bind(config.to_string())
        .bind(w.x)
        .bind(w.y)
        .bind(w.w)
        .bind(w.h)
        .bind(w.pinned)
        .bind(w.collapsed)
        .bind(w.hidden)
        .bind(w.position)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn layout_or_active(pool: &SqlitePool, layout_id: Option<String>) -> AppResult<String> {
    match layout_id {
        Some(id) => Ok(id),
        None => crate::workspace::active_layout_id(pool).await,
    }
}

// --- commands ---

#[derive(Debug, Serialize)]
pub struct Size {
    pub w: i64,
    pub h: i64,
}

/// A widget kind with the state of its module, for the Widgets page.
#[derive(Debug, Serialize)]
pub struct CatalogEntry {
    pub kind: &'static str,
    pub title: &'static str,
    pub module_id: &'static str,
    pub name: String,
    pub description: String,
    pub details: String,
    pub category: WidgetCategory,
    pub data_source: DataSource,
    pub default_size: Size,
    pub min_size: Size,
    pub refresh_secs: u32,
    /// The widget's own module is on.
    pub enabled: bool,
    /// All modules it needs are on (it can be shown and added).
    pub available: bool,
    /// Names of required modules that are off.
    pub missing_modules: Vec<String>,
    pub permissions: Vec<ModulePermission>,
    pub missing_permissions: Vec<String>,
    pub settings: Value,
    pub settings_schema: Value,
    /// Instances on the active layout.
    pub instances: i64,
}

#[tauri::command]
pub async fn widget_catalog(state: State<'_, AppState>) -> AppResult<Vec<CatalogEntry>> {
    let db = &state.db;
    let enabled = registry::enabled_set(db).await?;
    let pending = state.pending_restart();
    let active = crate::workspace::active_layout_id(db).await?;
    let counts: Vec<(String, i64)> = sqlx::query_as("SELECT kind, COUNT(*) FROM widgets WHERE layout_id = ? GROUP BY kind")
        .bind(&active)
        .fetch_all(db)
        .await?;
    let mut out = Vec::with_capacity(catalog::WIDGETS.len());
    for d in catalog::WIDGETS {
        let info = registry::get(db, d.module, &pending).await?;
        let missing = d.missing_modules(&enabled);
        out.push(CatalogEntry {
            kind: d.kind,
            title: d.title,
            module_id: d.module,
            refresh_secs: d.refresh_secs(&info.settings),
            name: info.name,
            description: info.description,
            details: info.details,
            category: d.category,
            data_source: d.data_source,
            default_size: Size { w: d.default_size.0, h: d.default_size.1 },
            min_size: Size { w: d.min_size.0, h: d.min_size.1 },
            enabled: info.enabled,
            available: missing.is_empty(),
            missing_modules: missing.iter().map(|m| registry::def(m).map(|d| d.name).unwrap_or(m).to_string()).collect(),
            permissions: info.permissions,
            missing_permissions: info.missing_permissions,
            settings: info.settings,
            settings_schema: info.settings_schema,
            instances: counts.iter().find(|(k, _)| k == d.kind).map(|c| c.1).unwrap_or(0),
        });
    }
    Ok(out)
}

/// Visible widgets of a layout (the active one when `layout_id` is omitted).
#[tauri::command]
pub async fn widgets_list(state: State<'_, AppState>, layout_id: Option<String>) -> AppResult<Vec<Widget>> {
    let layout = layout_or_active(&state.db, layout_id).await?;
    visible(&state.db, &layout).await
}

#[tauri::command]
pub async fn widgets_add(
    state: State<'_, AppState>,
    kind: String,
    layout_id: Option<String>,
    title: Option<String>,
) -> AppResult<Widget> {
    require_available(&state, def(&kind)?)?;
    let layout = layout_or_active(&state.db, layout_id).await?;
    insert(&state.db, &layout, &kind, title.as_deref(), None).await
}

#[tauri::command]
pub async fn widgets_update(state: State<'_, AppState>, id: String, patch: WidgetPatch) -> AppResult<Widget> {
    update(&state.db, &id, patch).await
}

#[tauri::command]
pub async fn widgets_save_grid(state: State<'_, AppState>, layout_id: String, items: Vec<GridItem>) -> AppResult<()> {
    save_grid(&state.db, &layout_id, &items).await
}

#[tauri::command]
pub async fn widgets_remove(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    remove(&state.db, &id).await?;
    desktop::close(&app, &id);
    Ok(())
}

/// Data of one widget. `params` are kind-specific (calendar: `{ month: "YYYY-MM" }`);
/// `force` skips caches (e.g. the refresh button of the weather widget).
#[tauri::command]
pub async fn widget_data(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    params: Option<Value>,
    force: Option<bool>,
) -> AppResult<data::WidgetData> {
    let w = get(&state.db, &id).await?;
    require_available(&state, def(&w.kind)?)?;
    data::load(&app, &w, &params.unwrap_or(Value::Null), force.unwrap_or(false)).await
}

/// Runs a widget action (see `data::act`) and returns fresh data.
#[tauri::command]
pub async fn widget_action(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    action: String,
    payload: Option<Value>,
    params: Option<Value>,
) -> AppResult<data::WidgetData> {
    let w = get(&state.db, &id).await?;
    require_available(&state, def(&w.kind)?)?;
    let payload = payload.unwrap_or(Value::Null);
    data::act(&app, &w, &action, &payload).await?;
    let w = get(&state.db, &id).await?;
    data::load(&app, &w, &params.unwrap_or(Value::Null), false).await
}

#[derive(Debug, Serialize)]
pub struct PickedApp {
    pub name: String,
    pub path: String,
}

/// File dialog for the app launcher widget. Nothing is started here.
#[tauri::command]
pub async fn widget_pick_app(app: AppHandle) -> AppResult<Option<PickedApp>> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Выберите программу или ярлык")
            .add_filter("Программы и ярлыки", APP_EXTENSIONS)
            .blocking_pick_file()
    })
    .await
    .map_err(anyhow::Error::from)?;
    let Some(picked) = picked else { return Ok(None) };
    let path = picked.into_path().map_err(anyhow::Error::from)?;
    let path_str = path.display().to_string();
    valid_app_path(&path_str)?;
    let name: String = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Программа").chars().take(60).collect();
    Ok(Some(PickedApp { name, path: path_str }))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (SqlitePool, String) {
        let pool = registry::test_pool().await;
        crate::workspace::seed(&pool).await.unwrap();
        let layout = crate::workspace::active_layout_id(&pool).await.unwrap();
        (pool, layout)
    }

    #[tokio::test]
    async fn new_widgets_take_free_spots() {
        let (pool, layout) = setup().await;
        let before = all(&pool, &layout).await.unwrap();
        let w = insert(&pool, &layout, "stopwatch", None, None).await.unwrap();
        assert_eq!(w.title, "Секундомер");
        assert!(before.iter().all(|o| !o.rect().overlaps(&w.rect())));
        assert_eq!(w.config["laps"], json!([]));
    }

    #[tokio::test]
    async fn notes_widget_owns_its_note() {
        let (pool, layout) = setup().await;
        let w = insert(&pool, &layout, "notes", Some("Идеи"), None).await.unwrap();
        let note = w.config["note_id"].as_str().unwrap().to_string();
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notes WHERE id = ?").bind(&note).fetch_one(&pool).await.unwrap();
        assert_eq!(n, 1);
        remove(&pool, &w.id).await.unwrap();
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notes WHERE id = ?").bind(&note).fetch_one(&pool).await.unwrap();
        assert_eq!(n, 0);
    }

    #[tokio::test]
    async fn config_patch_is_validated_and_keeps_state() {
        let (pool, layout) = setup().await;
        let w = insert(&pool, &layout, "quick_links", None, None).await.unwrap();
        let bad = WidgetPatch { config: Some(json!({ "links": [{ "title": "x", "url": "file:///c:/windows" }] })), ..Default::default() };
        assert!(matches!(update(&pool, &w.id, bad).await, Err(AppError::Invalid(_))));
        let good = WidgetPatch { config: Some(json!({ "links": [{ "title": "Почта", "url": "https://mail.example.com" }] })), ..Default::default() };
        assert_eq!(update(&pool, &w.id, good).await.unwrap().config["links"][0]["title"], "Почта");

        let n = insert(&pool, &layout, "notes", None, None).await.unwrap();
        let look = WidgetPatch { config: Some(json!({ "appearance": { "shape": "pill", "color": "#12ab34", "texture": "dots", "opacity": 0.8 } })), ..Default::default() };
        assert_eq!(update(&pool, &w.id, look).await.unwrap().config["appearance"]["shape"], "pill");
        for a in [json!({ "shape": "star" }), json!({ "color": "red" }), json!({ "opacity": 0.05 }), json!("x")] {
            let p = WidgetPatch { config: Some(json!({ "appearance": a })), ..Default::default() };
            assert!(matches!(update(&pool, &w.id, p).await, Err(AppError::Invalid(_))));
        }
        let hijack = WidgetPatch { config: Some(json!({ "note_id": "other" })), ..Default::default() };
        assert_eq!(update(&pool, &n.id, hijack).await.unwrap().config["note_id"], n.config["note_id"]);
    }

    #[tokio::test]
    async fn grid_flags_and_saving() {
        let (pool, layout) = setup().await;
        let w = all(&pool, &layout).await.unwrap().remove(0);
        save_grid(&pool, &layout, &[GridItem { id: w.id.clone(), x: 5, y: 7, w: 4, h: 3 }]).await.unwrap();
        let moved = get(&pool, &w.id).await.unwrap();
        assert_eq!((moved.x, moved.y, moved.w, moved.h), (5, 7, 4, 3));
        assert!(save_grid(&pool, &layout, &[GridItem { id: w.id.clone(), x: 22, y: 0, w: 4, h: 3 }]).await.is_err());

        let patch = WidgetPatch { pinned: Some(true), collapsed: Some(true), hidden: Some(true), ..Default::default() };
        let flagged = update(&pool, &w.id, patch).await.unwrap();
        assert!(flagged.pinned && flagged.collapsed && flagged.hidden);
    }

    #[tokio::test]
    async fn disabled_modules_hide_widgets() {
        let (pool, layout) = setup().await;
        let kinds = |v: Vec<Widget>| v.into_iter().map(|w| w.kind).collect::<Vec<_>>();
        assert_eq!(kinds(visible(&pool, &layout).await.unwrap()), ["clock", "todo", "status"]);

        registry::set_enabled(&pool, modules::WIDGET_CLOCK, false, false, "t").await.unwrap();
        assert_eq!(kinds(visible(&pool, &layout).await.unwrap()), ["todo", "status"]);
        // The storage module of the todo widget counts too.
        registry::set_enabled(&pool, modules::TASKS, false, false, "t").await.unwrap();
        assert_eq!(kinds(visible(&pool, &layout).await.unwrap()), ["status"]);
        assert_eq!(all(&pool, &layout).await.unwrap().len(), 3, "hidden widgets are kept");

        registry::set_enabled(&pool, modules::WIDGETS, false, false, "t").await.unwrap();
        assert!(visible(&pool, &layout).await.unwrap().is_empty());
    }

    #[test]
    fn links_and_paths() {
        assert!(valid_link("https://example.com/a?b=c"));
        assert!(valid_link("mailto:me@example.com"));
        assert!(!valid_link("javascript:alert(1)"));
        assert!(!valid_link("file:///C:/Windows/notepad.exe"));
        assert!(!valid_link("https://exa mple.com"));
        assert!(valid_app_path(r"C:\Program Files\App\app.exe").is_ok());
        assert!(valid_app_path(r"C:\scripts\run.bat").is_err());
        assert!(valid_app_path("app.exe").is_err());
    }
}
