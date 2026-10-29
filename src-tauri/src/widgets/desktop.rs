//! Widgets on the Windows desktop: a widget detached from the grid gets its
//! own frameless, transparent window (label `dw-<widget id>`). The window
//! renders the same widget body; its position and size are saved to
//! `desktop_widgets` (debounced) and restored on the next launch.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use super::{Widget, def, get, require_available};
use crate::error::{AppError, AppResult};
use crate::modules::{catalog::DESKTOP_WIDGETS, registry};
use crate::state::AppState;

/// Emitted when a widget is detached or returned, so the dashboard refreshes.
pub const CHANGED_EVENT: &str = "focusdeck://desktop-widgets-changed";
const PREFIX: &str = "dw-";
/// Pixels per grid cell for the first size of a detached widget.
const CELL_PX: i64 = 80;
const MIN_PX: u32 = 160;
const SAVE_DELAY: Duration = Duration::from_millis(600);

pub fn is_desktop_label(label: &str) -> bool {
    label.starts_with(PREFIX)
}

pub fn label(widget_id: &str) -> String {
    format!("{PREFIX}{widget_id}")
}

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct Placement {
    pub widget_id: String,
    pub x: Option<i64>,
    pub y: Option<i64>,
    pub width: i64,
    pub height: i64,
}

// --- storage ---

pub async fn placements(pool: &SqlitePool) -> AppResult<Vec<Placement>> {
    Ok(sqlx::query_as("SELECT widget_id, x, y, width, height FROM desktop_widgets ORDER BY created_at").fetch_all(pool).await?)
}

pub async fn placement(pool: &SqlitePool, widget_id: &str) -> AppResult<Option<Placement>> {
    Ok(sqlx::query_as("SELECT widget_id, x, y, width, height FROM desktop_widgets WHERE widget_id = ?")
        .bind(widget_id)
        .fetch_optional(pool)
        .await?)
}

/// Marks a widget as being on the desktop; the first size follows its grid size.
pub async fn detach(pool: &SqlitePool, w: &Widget) -> AppResult<Placement> {
    let (width, height) = ((w.w * CELL_PX).max(MIN_PX as i64), (w.h * CELL_PX).max(MIN_PX as i64));
    sqlx::query("INSERT INTO desktop_widgets (widget_id, width, height) VALUES (?, ?, ?) ON CONFLICT(widget_id) DO NOTHING")
        .bind(&w.id)
        .bind(width)
        .bind(height)
        .execute(pool)
        .await?;
    Ok(placement(pool, &w.id).await?.expect("just inserted"))
}

pub async fn attach(pool: &SqlitePool, widget_id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM desktop_widgets WHERE widget_id = ?").bind(widget_id).execute(pool).await?;
    Ok(())
}

pub async fn save_geometry(pool: &SqlitePool, widget_id: &str, x: i64, y: i64, width: i64, height: i64) -> AppResult<()> {
    sqlx::query("UPDATE desktop_widgets SET x = ?, y = ?, width = ?, height = ?, updated_at = datetime('now') WHERE widget_id = ?")
        .bind(x)
        .bind(y)
        .bind(width.max(MIN_PX as i64))
        .bind(height.max(MIN_PX as i64))
        .bind(widget_id)
        .execute(pool)
        .await?;
    Ok(())
}

// --- windows ---

async fn always_on_top(pool: &SqlitePool) -> bool {
    registry::settings(pool, DESKTOP_WIDGETS).await.ok().and_then(|s| s.get("always_on_top").and_then(Value::as_bool)).unwrap_or(false)
}

/// A saved position is used only if its top-left area is on a connected monitor.
fn on_screen(app: &AppHandle, x: i64, y: i64) -> bool {
    app.available_monitors().unwrap_or_default().iter().any(|m| {
        let (p, s) = (m.position(), m.size());
        let (mx, my) = (p.x as i64, p.y as i64);
        x + 40 >= mx && x + 40 < mx + s.width as i64 && y + 20 >= my && y + 20 < my + s.height as i64
    })
}

/// Opens (or focuses) the window of a desktop widget.
pub async fn open(app: &AppHandle, w: &Widget, p: &Placement) -> AppResult<WebviewWindow> {
    let label = label(&w.id);
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.show();
        return Ok(win);
    }
    let on_top = always_on_top(&app.state::<AppState>().db).await;
    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title(&w.title)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(true)
        .skip_taskbar(true)
        .always_on_top(on_top)
        .always_on_bottom(!on_top)
        .min_inner_size(MIN_PX as f64, MIN_PX as f64)
        .visible(false);
    let win = builder.build().map_err(anyhow::Error::from)?;
    let _ = win.set_size(PhysicalSize::new(p.width.max(MIN_PX as i64) as u32, p.height.max(MIN_PX as i64) as u32));
    match (p.x, p.y) {
        (Some(x), Some(y)) if on_screen(app, x, y) => {
            let _ = win.set_position(PhysicalPosition::new(x as i32, y as i32));
        }
        _ => {
            let _ = win.center();
        }
    }
    let _ = win.show();
    track(app, &win, &w.id);
    Ok(win)
}

/// Saves the geometry after the user stops moving/resizing (one write per gesture).
fn track(app: &AppHandle, win: &WebviewWindow, widget_id: &str) {
    static GENERATION: Mutex<Option<HashMap<String, u64>>> = Mutex::new(None);
    let (app, id, handle) = (app.clone(), widget_id.to_string(), win.clone());
    win.on_window_event(move |e| {
        if !matches!(e, WindowEvent::Moved(_) | WindowEvent::Resized(_)) {
            return;
        }
        let generation = {
            let mut map = GENERATION.lock().unwrap();
            let n = map.get_or_insert_with(HashMap::new).entry(id.clone()).or_default();
            *n += 1;
            *n
        };
        let (app, id, handle) = (app.clone(), id.clone(), handle.clone());
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(SAVE_DELAY).await;
            if GENERATION.lock().unwrap().as_ref().and_then(|m| m.get(&id)) != Some(&generation) {
                return;
            }
            // Minimized windows report a bogus position.
            if handle.is_minimized().unwrap_or(false) {
                return;
            }
            let (Ok(pos), Ok(size)) = (handle.outer_position(), handle.inner_size()) else { return };
            // Smaller than the minimum only in the compact mode of the music widget.
            if size.width < MIN_PX || size.height < MIN_PX {
                return;
            }
            let db = &app.state::<AppState>().db;
            if let Err(e) = save_geometry(db, &id, pos.x as i64, pos.y as i64, size.width as i64, size.height as i64).await {
                tracing::warn!("desktop widget geometry: {e}");
            }
        });
    });
}

pub fn close(app: &AppHandle, widget_id: &str) {
    if let Some(win) = app.get_webview_window(&label(widget_id)) {
        let _ = win.destroy();
    }
}

pub fn close_all(app: &AppHandle) {
    for (label, win) in app.webview_windows() {
        if is_desktop_label(&label) {
            let _ = win.destroy();
        }
    }
}

/// Opens every detached widget whose modules are on (startup, module turned on).
/// Windows of widgets that became unavailable are closed.
pub async fn sync(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    if !state.module_enabled(DESKTOP_WIDGETS) {
        close_all(app);
        return Ok(());
    }
    for p in placements(&state.db).await? {
        let Ok(w) = get(&state.db, &p.widget_id).await else { continue };
        let available = def(&w.kind).is_ok_and(|d| require_available(&state, d).is_ok());
        if available {
            if let Err(e) = open(app, &w, &p).await {
                tracing::warn!(widget = %w.id, "desktop widget window: {e}");
            }
        } else {
            close(app, &w.id);
        }
    }
    Ok(())
}

/// At startup, honouring the «open at start» setting.
pub async fn restore(app: &AppHandle) {
    let db = &app.state::<AppState>().db;
    let restore = registry::settings(db, DESKTOP_WIDGETS).await.ok().and_then(|s| s.get("restore_on_start").and_then(Value::as_bool)).unwrap_or(true);
    if restore {
        if let Err(e) = sync(app).await {
            tracing::warn!("desktop widgets: {e}");
        }
    }
}

/// New «always on top» setting for open windows.
pub async fn apply_settings(app: &AppHandle) {
    let on_top = always_on_top(&app.state::<AppState>().db).await;
    for (label, win) in app.webview_windows() {
        if is_desktop_label(&label) {
            let _ = win.set_always_on_bottom(!on_top);
            let _ = win.set_always_on_top(on_top);
        }
    }
}

fn changed(app: &AppHandle) {
    let _ = app.emit(CHANGED_EVENT, ());
}

// --- commands ---

/// Widgets currently on the desktop.
#[tauri::command]
pub async fn desktop_widgets_list(state: State<'_, AppState>) -> AppResult<Vec<Placement>> {
    placements(&state.db).await
}

/// One widget for its desktop window (any layout).
#[tauri::command]
pub async fn desktop_widget_get(state: State<'_, AppState>, id: String) -> AppResult<Widget> {
    get(&state.db, &id).await
}

/// Detaches a widget from the grid into its own desktop window.
#[tauri::command]
pub async fn desktop_widget_detach(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<Placement> {
    crate::productivity::require(&state, DESKTOP_WIDGETS)?;
    let w = get(&state.db, &id).await?;
    require_available(&state, def(&w.kind)?)?;
    let p = detach(&state.db, &w).await?;
    if let Err(e) = open(&app, &w, &p).await {
        attach(&state.db, &id).await?;
        return Err(e);
    }
    changed(&app);
    Ok(p)
}

/// Returns a desktop widget to the app grid and closes its window.
#[tauri::command]
pub async fn desktop_widget_attach(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    if placement(&state.db, &id).await?.is_none() {
        return Err(AppError::NotFound("виджет на рабочем столе".into()));
    }
    attach(&state.db, &id).await?;
    close(&app, &id);
    changed(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::{insert, remove};

    #[tokio::test]
    async fn detach_save_attach() {
        let pool = registry::test_pool().await;
        crate::workspace::seed(&pool).await.unwrap();
        let layout = crate::workspace::active_layout_id(&pool).await.unwrap();
        let w = insert(&pool, &layout, "clock", None, None).await.unwrap();
        assert!(!w.desktop);

        let p = detach(&pool, &w).await.unwrap();
        assert_eq!((p.x, p.width, p.height), (None, w.w * CELL_PX, MIN_PX as i64));
        assert!(get(&pool, &w.id).await.unwrap().desktop);
        // Detaching twice keeps the saved geometry.
        save_geometry(&pool, &w.id, 10, 20, 400, 50).await.unwrap();
        detach(&pool, &w).await.unwrap();
        let p = placement(&pool, &w.id).await.unwrap().unwrap();
        assert_eq!((p.x, p.y, p.width, p.height), (Some(10), Some(20), 400, MIN_PX as i64));

        attach(&pool, &w.id).await.unwrap();
        assert!(!get(&pool, &w.id).await.unwrap().desktop);

        // Deleting the widget removes its placement.
        detach(&pool, &w).await.unwrap();
        remove(&pool, &w.id).await.unwrap();
        assert!(placements(&pool).await.unwrap().is_empty());
        assert!(is_desktop_label(&label("x")) && !is_desktop_label("main"));
    }
}
