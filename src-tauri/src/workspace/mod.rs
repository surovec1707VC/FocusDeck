//! Workspace: widget layouts, widgets and the quick task list.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::State;

use crate::db::new_id;
use crate::error::{AppError, AppResult};
use crate::modules::{catalog, registry};
use crate::state::AppState;

pub const WIDGET_KINDS: &[&str] = &["clock", "note", "tasks", "status"];

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Layout {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct WidgetRow {
    pub id: String,
    pub layout_id: String,
    pub kind: String,
    pub title: String,
    pub size: String,
    pub position: i64,
    pub config_json: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Widget {
    pub id: String,
    pub layout_id: String,
    pub kind: String,
    pub title: String,
    pub size: String,
    pub position: i64,
    pub config: Value,
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
            size: r.size,
            position: r.position,
        })
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub done: bool,
    pub position: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct WidgetPatch {
    pub title: Option<String>,
    pub size: Option<String>,
    pub config: Option<Value>,
}

fn check_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(AppError::Invalid(
            "название должно быть от 1 до 60 символов".into(),
        ));
    }
    Ok(name.to_string())
}

fn check_size(size: &str) -> AppResult<()> {
    if ["sm", "md", "lg"].contains(&size) {
        Ok(())
    } else {
        Err(AppError::Invalid(format!(
            "неизвестный размер виджета {size}"
        )))
    }
}

/// Creates the default layout with a clock widget on first start.
pub async fn seed(pool: &SqlitePool) -> AppResult<()> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM widget_layouts")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(());
    }
    let layout = new_id();
    sqlx::query("INSERT INTO widget_layouts (id, name, is_active) VALUES (?, 'Основной', 1)")
        .bind(&layout)
        .execute(pool)
        .await?;
    for (pos, (kind, title, size)) in [
        ("clock", "Часы", "sm"),
        ("tasks", "Задачи", "md"),
        ("status", "Состояние", "sm"),
    ]
    .into_iter()
    .enumerate()
    {
        sqlx::query("INSERT INTO widgets (id, layout_id, kind, title, size, position) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(new_id())
            .bind(&layout)
            .bind(kind)
            .bind(title)
            .bind(size)
            .bind(pos as i64)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn layouts(pool: &SqlitePool) -> AppResult<Vec<Layout>> {
    Ok(
        sqlx::query_as::<_, Layout>("SELECT * FROM widget_layouts ORDER BY created_at, rowid")
            .fetch_all(pool)
            .await?,
    )
}

pub async fn active_layout_id(pool: &SqlitePool) -> AppResult<String> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT id FROM widget_layouts WHERE is_active = 1 LIMIT 1")
            .fetch_optional(pool)
            .await?;
    row.map(|r| r.0)
        .ok_or_else(|| AppError::NotFound("активный макет".into()))
}

pub async fn activate(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    let found = sqlx::query(
        "UPDATE widget_layouts SET is_active = 1, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if found == 0 {
        return Err(AppError::NotFound("макет".into()));
    }
    sqlx::query("UPDATE widget_layouts SET is_active = 0 WHERE id != ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn delete_layout(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let all = layouts(pool).await?;
    if all.len() <= 1 {
        return Err(AppError::Invalid("нельзя удалить последний макет".into()));
    }
    let target = all
        .iter()
        .find(|l| l.id == id)
        .ok_or_else(|| AppError::NotFound("макет".into()))?;
    let was_active = target.is_active;
    sqlx::query("DELETE FROM widget_layouts WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if was_active {
        let next = all.iter().find(|l| l.id != id).unwrap();
        activate(pool, &next.id).await?;
    }
    Ok(())
}

pub async fn widgets(pool: &SqlitePool, layout_id: &str) -> AppResult<Vec<Widget>> {
    sqlx::query_as::<_, WidgetRow>(
        "SELECT id, layout_id, kind, title, size, position, config_json FROM widgets WHERE layout_id = ? ORDER BY position, rowid",
    )
    .bind(layout_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(Widget::try_from)
    .collect()
}

/// Widgets the user can see: none while the Widgets module is off, and
/// none whose owning module (e.g. «Часы») is off. Hidden widgets are kept.
pub async fn visible_widgets(pool: &SqlitePool, layout_id: &str) -> AppResult<Vec<Widget>> {
    if !registry::is_enabled(pool, catalog::WIDGETS).await? {
        return Ok(Vec::new());
    }
    let enabled = registry::enabled_set(pool).await?;
    Ok(widgets(pool, layout_id)
        .await?
        .into_iter()
        .filter(|w| catalog::widget_owner(&w.kind).is_none_or(|owner| enabled.contains(owner)))
        .collect())
}

/// Moves a widget one place up (-1) or down (+1) within its layout.
pub async fn move_widget(pool: &SqlitePool, id: &str, delta: i64) -> AppResult<()> {
    let (layout_id,): (String,) = sqlx::query_as("SELECT layout_id FROM widgets WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("виджет".into()))?;
    let mut list = widgets(pool, &layout_id).await?;
    let Some(i) = list.iter().position(|w| w.id == id) else {
        return Ok(());
    };
    let j = i as i64 + delta.signum();
    if j < 0 || j >= list.len() as i64 {
        return Ok(());
    }
    list.swap(i, j as usize);
    let mut tx = pool.begin().await?;
    for (pos, w) in list.iter().enumerate() {
        sqlx::query("UPDATE widgets SET position = ? WHERE id = ?")
            .bind(pos as i64)
            .bind(&w.id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

// --- commands: layouts ---

#[tauri::command]
pub async fn layouts_list(state: State<'_, AppState>) -> AppResult<Vec<Layout>> {
    layouts(&state.db).await
}

#[tauri::command]
pub async fn layouts_create(state: State<'_, AppState>, name: String) -> AppResult<Vec<Layout>> {
    sqlx::query("INSERT INTO widget_layouts (id, name, is_active) VALUES (?, ?, 0)")
        .bind(new_id())
        .bind(check_name(&name)?)
        .execute(&state.db)
        .await?;
    layouts(&state.db).await
}

#[tauri::command]
pub async fn layouts_rename(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> AppResult<Vec<Layout>> {
    let n = sqlx::query(
        "UPDATE widget_layouts SET name = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(check_name(&name)?)
    .bind(&id)
    .execute(&state.db)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound("макет".into()));
    }
    layouts(&state.db).await
}

#[tauri::command]
pub async fn layouts_activate(state: State<'_, AppState>, id: String) -> AppResult<Vec<Layout>> {
    activate(&state.db, &id).await?;
    layouts(&state.db).await
}

#[tauri::command]
pub async fn layouts_delete(state: State<'_, AppState>, id: String) -> AppResult<Vec<Layout>> {
    delete_layout(&state.db, &id).await?;
    layouts(&state.db).await
}

// --- commands: widgets ---

/// Widgets of a layout (the active one when `layout_id` is omitted).
#[tauri::command]
pub async fn widgets_list(
    state: State<'_, AppState>,
    layout_id: Option<String>,
) -> AppResult<Vec<Widget>> {
    let layout = match layout_id {
        Some(id) => id,
        None => active_layout_id(&state.db).await?,
    };
    visible_widgets(&state.db, &layout).await
}

#[tauri::command]
pub async fn widgets_add(
    state: State<'_, AppState>,
    kind: String,
    title: String,
    size: String,
) -> AppResult<Vec<Widget>> {
    if !WIDGET_KINDS.contains(&kind.as_str()) {
        return Err(AppError::Invalid(format!("неизвестный тип виджета {kind}")));
    }
    for module in [Some(catalog::WIDGETS), catalog::widget_owner(&kind)].into_iter().flatten() {
        if !state.module_enabled(module) {
            let name = registry::def(module).map(|m| m.name).unwrap_or(module);
            return Err(AppError::ModuleDisabled(name.into()));
        }
    }
    check_size(&size)?;
    let layout = active_layout_id(&state.db).await?;
    sqlx::query(
        "INSERT INTO widgets (id, layout_id, kind, title, size, position)
         VALUES (?, ?, ?, ?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM widgets WHERE layout_id = ?))",
    )
    .bind(new_id())
    .bind(&layout)
    .bind(&kind)
    .bind(check_name(&title)?)
    .bind(&size)
    .bind(&layout)
    .execute(&state.db)
    .await?;
    visible_widgets(&state.db, &layout).await
}

#[tauri::command]
pub async fn widgets_update(
    state: State<'_, AppState>,
    id: String,
    patch: WidgetPatch,
) -> AppResult<Widget> {
    if let Some(size) = &patch.size {
        check_size(size)?;
    }
    let title = patch.title.as_deref().map(check_name).transpose()?;
    let config = patch.config.as_ref().map(|c| c.to_string());
    let n = sqlx::query(
        "UPDATE widgets SET title = COALESCE(?, title), size = COALESCE(?, size),
            config_json = COALESCE(?, config_json), updated_at = datetime('now') WHERE id = ?",
    )
    .bind(title)
    .bind(patch.size)
    .bind(config)
    .bind(&id)
    .execute(&state.db)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound("виджет".into()));
    }
    let row = sqlx::query_as::<_, WidgetRow>(
        "SELECT id, layout_id, kind, title, size, position, config_json FROM widgets WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await?;
    row.try_into()
}

#[tauri::command]
pub async fn widgets_move(state: State<'_, AppState>, id: String, delta: i64) -> AppResult<()> {
    move_widget(&state.db, &id, delta).await
}

#[tauri::command]
pub async fn widgets_remove(state: State<'_, AppState>, id: String) -> AppResult<()> {
    sqlx::query("DELETE FROM widgets WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(())
}

// --- commands: tasks ---

#[tauri::command]
pub async fn tasks_list(state: State<'_, AppState>) -> AppResult<Vec<Task>> {
    Ok(sqlx::query_as::<_, Task>("SELECT id, title, done, position, created_at FROM tasks ORDER BY done, position, created_at")
        .fetch_all(&state.db)
        .await?)
}

#[tauri::command]
pub async fn tasks_add(state: State<'_, AppState>, title: String) -> AppResult<Vec<Task>> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 200 {
        return Err(AppError::Invalid(
            "текст задачи должен быть от 1 до 200 символов".into(),
        ));
    }
    sqlx::query("INSERT INTO tasks (id, title, position) VALUES (?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM tasks))")
        .bind(new_id())
        .bind(title)
        .execute(&state.db)
        .await?;
    tasks_list(state).await
}

#[tauri::command]
pub async fn tasks_toggle(state: State<'_, AppState>, id: String) -> AppResult<Vec<Task>> {
    sqlx::query("UPDATE tasks SET done = NOT done, updated_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    tasks_list(state).await
}

#[tauri::command]
pub async fn tasks_delete(state: State<'_, AppState>, id: String) -> AppResult<Vec<Task>> {
    sqlx::query("DELETE FROM tasks WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    tasks_list(state).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_layout_and_layout_rules() {
        let pool = crate::db::test_pool().await;
        seed(&pool).await.unwrap();
        seed(&pool).await.unwrap(); // idempotent
        let all = layouts(&pool).await.unwrap();
        assert_eq!(all.len(), 1);
        let first = active_layout_id(&pool).await.unwrap();
        assert_eq!(widgets(&pool, &first).await.unwrap().len(), 3);

        assert!(matches!(
            delete_layout(&pool, &first).await,
            Err(AppError::Invalid(_))
        ));

        sqlx::query("INSERT INTO widget_layouts (id, name) VALUES ('second', 'Второй')")
            .execute(&pool)
            .await
            .unwrap();
        activate(&pool, "second").await.unwrap();
        assert_eq!(active_layout_id(&pool).await.unwrap(), "second");

        // Deleting the active layout activates another one.
        delete_layout(&pool, "second").await.unwrap();
        assert_eq!(active_layout_id(&pool).await.unwrap(), first);
    }

    #[tokio::test]
    async fn moving_widgets() {
        let pool = crate::db::test_pool().await;
        seed(&pool).await.unwrap();
        let layout = active_layout_id(&pool).await.unwrap();
        let before: Vec<String> = widgets(&pool, &layout)
            .await
            .unwrap()
            .into_iter()
            .map(|w| w.kind)
            .collect();
        assert_eq!(before, ["clock", "tasks", "status"]);

        let tasks_id = widgets(&pool, &layout).await.unwrap()[1].id.clone();
        move_widget(&pool, &tasks_id, -1).await.unwrap();
        move_widget(&pool, &tasks_id, -1).await.unwrap(); // already first: no-op
        let after: Vec<String> = widgets(&pool, &layout)
            .await
            .unwrap()
            .into_iter()
            .map(|w| w.kind)
            .collect();
        assert_eq!(after, ["tasks", "clock", "status"]);
    }

    #[tokio::test]
    async fn disabled_modules_hide_widgets() {
        let pool = registry::test_pool().await;
        seed(&pool).await.unwrap();
        let layout = active_layout_id(&pool).await.unwrap();
        let kinds = |v: Vec<Widget>| v.into_iter().map(|w| w.kind).collect::<Vec<_>>();
        assert_eq!(kinds(visible_widgets(&pool, &layout).await.unwrap()), ["clock", "tasks", "status"]);

        registry::set_enabled(&pool, catalog::WIDGET_CLOCK, false, false, "t").await.unwrap();
        assert_eq!(kinds(visible_widgets(&pool, &layout).await.unwrap()), ["tasks", "status"]);
        assert_eq!(widgets(&pool, &layout).await.unwrap().len(), 3, "hidden widgets are kept");

        registry::set_enabled(&pool, catalog::WIDGETS, false, false, "t").await.unwrap();
        assert!(visible_widgets(&pool, &layout).await.unwrap().is_empty());
    }
}
