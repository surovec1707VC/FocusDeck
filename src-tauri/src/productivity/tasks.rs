//! Tasks: priorities, due dates, statuses, categories ("Работа/Отчёты"),
//! filters, repeating tasks, quick add and daily statistics.

use chrono::{Duration, Local, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;

use super::quick_add::{self, next_due};
use super::require;
use crate::db::new_id;
use crate::error::{AppError, AppResult};
use crate::modules::{catalog::TASKS, registry};
use crate::state::AppState;

pub const STATUSES: &[&str] = &["todo", "in_progress", "done", "cancelled"];
pub const RECURRENCES: &[&str] = &["none", "daily", "weekdays", "weekly", "monthly"];

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub done: bool,
    pub status: String,
    /// 0 = none, 1 = low, 2 = medium, 3 = high.
    pub priority: i64,
    /// YYYY-MM-DD.
    pub due_at: Option<String>,
    pub category: String,
    pub recurrence: String,
    pub notes: String,
    pub position: i64,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct TaskFilter {
    /// "open" (todo + in_progress, default), "all", or one status.
    pub status: Option<String>,
    /// Minimum priority.
    pub priority: Option<i64>,
    /// Category and its subcategories.
    pub category: Option<String>,
    /// "today" | "overdue" | "week" | "none".
    pub due: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct TaskInput {
    pub title: Option<String>,
    pub status: Option<String>,
    pub priority: Option<i64>,
    /// `Some(None)` clears the date.
    #[serde(default, deserialize_with = "double_option")]
    pub due_at: Option<Option<String>>,
    pub category: Option<String>,
    pub recurrence: Option<String>,
    pub notes: Option<String>,
}

fn double_option<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    Ok(Some(Option::<String>::deserialize(d)?))
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

fn check(input: &TaskInput) -> AppResult<()> {
    if let Some(t) = &input.title {
        let n = t.trim().chars().count();
        if n == 0 || n > 200 {
            return Err(AppError::Invalid("текст задачи должен быть от 1 до 200 символов".into()));
        }
    }
    if input.status.as_ref().is_some_and(|s| !STATUSES.contains(&s.as_str())) {
        return Err(AppError::Invalid("неизвестный статус задачи".into()));
    }
    if input.priority.is_some_and(|p| !(0..=3).contains(&p)) {
        return Err(AppError::Invalid("приоритет — от 0 до 3".into()));
    }
    if let Some(Some(d)) = &input.due_at {
        NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| AppError::Invalid("срок должен быть в формате ГГГГ-ММ-ДД".into()))?;
    }
    if input.recurrence.as_ref().is_some_and(|r| !RECURRENCES.contains(&r.as_str())) {
        return Err(AppError::Invalid("неизвестный повтор".into()));
    }
    if input.category.as_ref().is_some_and(|c| c.chars().count() > 80) {
        return Err(AppError::Invalid("категория — не больше 80 символов".into()));
    }
    if input.notes.as_ref().is_some_and(|n| n.chars().count() > 5000) {
        return Err(AppError::Invalid("описание — не больше 5000 символов".into()));
    }
    Ok(())
}

/// "Работа / Отчёты/" → "Работа/Отчёты".
fn clean_category(c: &str) -> String {
    c.split('/').map(str::trim).filter(|p| !p.is_empty()).collect::<Vec<_>>().join("/")
}

pub async fn all(pool: &SqlitePool) -> AppResult<Vec<Task>> {
    Ok(sqlx::query_as::<_, Task>(
        "SELECT id, title, done, status, priority, due_at, category, recurrence, notes, position, created_at, completed_at FROM tasks
         ORDER BY done, priority DESC, due_at IS NULL, due_at, position, created_at",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Task> {
    sqlx::query_as::<_, Task>(
        "SELECT id, title, done, status, priority, due_at, category, recurrence, notes, position, created_at, completed_at FROM tasks WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("задача".into()))
}

pub async fn list(pool: &SqlitePool, f: &TaskFilter) -> AppResult<Vec<Task>> {
    let today = today();
    let week_end = today + Duration::days(7);
    let search = f.search.as_deref().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());
    let date = |t: &Task| t.due_at.as_deref().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
    Ok(all(pool)
        .await?
        .into_iter()
        .filter(|t| match f.status.as_deref().unwrap_or("open") {
            "open" => t.status == "todo" || t.status == "in_progress",
            "all" => true,
            s => t.status == s,
        })
        .filter(|t| f.priority.is_none_or(|p| t.priority >= p))
        .filter(|t| f.category.as_ref().is_none_or(|c| t.category == *c || t.category.starts_with(&format!("{c}/"))))
        .filter(|t| match f.due.as_deref() {
            Some("today") => date(t) == Some(today),
            Some("overdue") => date(t).is_some_and(|d| d < today) && !t.done,
            Some("week") => date(t).is_some_and(|d| d >= today && d <= week_end),
            Some("none") => t.due_at.is_none(),
            _ => true,
        })
        .filter(|t| search.as_ref().is_none_or(|s| t.title.to_lowercase().contains(s) || t.notes.to_lowercase().contains(s)))
        .collect())
}

pub async fn create(pool: &SqlitePool, input: TaskInput) -> AppResult<Task> {
    if input.title.is_none() {
        return Err(AppError::Invalid("нужен текст задачи".into()));
    }
    check(&input)?;
    let default_priority = registry::settings(pool, TASKS).await.ok().and_then(|s| s["default_priority"].as_i64()).unwrap_or(0);
    let id = new_id();
    let status = input.status.unwrap_or_else(|| "todo".into());
    sqlx::query(
        "INSERT INTO tasks (id, title, status, done, priority, due_at, category, recurrence, notes, position, completed_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM tasks), CASE WHEN ? THEN datetime('now') END)",
    )
    .bind(&id)
    .bind(input.title.unwrap().trim())
    .bind(&status)
    .bind(status == "done")
    .bind(input.priority.unwrap_or(default_priority))
    .bind(input.due_at.flatten())
    .bind(clean_category(input.category.as_deref().unwrap_or("")))
    .bind(input.recurrence.as_deref().unwrap_or("none"))
    .bind(input.notes.as_deref().unwrap_or(""))
    .bind(status == "done")
    .execute(pool)
    .await?;
    get(pool, &id).await
}

/// Creates a task from a quick-add line (see `quick_add`).
pub async fn quick_add(pool: &SqlitePool, line: &str) -> AppResult<Task> {
    let q = quick_add::parse(line, today());
    create(
        pool,
        TaskInput {
            title: Some(q.title),
            priority: q.priority,
            due_at: Some(q.due.map(|d| d.to_string())),
            category: q.category,
            recurrence: q.recurrence.map(String::from),
            ..Default::default()
        },
    )
    .await
}

/// Updates fields; finishing a repeating task creates its next occurrence.
pub async fn update(pool: &SqlitePool, id: &str, input: TaskInput) -> AppResult<Task> {
    check(&input)?;
    let cur = get(pool, id).await?;
    let status = input.status.clone().unwrap_or(cur.status.clone());
    let finished = status == "done" && cur.status != "done";
    let due = match input.due_at.clone() {
        Some(d) => d,
        None => cur.due_at.clone(),
    };
    let recurrence = input.recurrence.clone().unwrap_or(cur.recurrence.clone());
    sqlx::query(
        "UPDATE tasks SET title = ?, status = ?, done = ?, priority = ?, due_at = ?, category = ?, recurrence = ?, notes = ?,
            completed_at = CASE WHEN ? = 'done' THEN COALESCE(completed_at, datetime('now')) ELSE NULL END,
            updated_at = datetime('now') WHERE id = ?",
    )
    .bind(input.title.as_deref().map(str::trim).unwrap_or(&cur.title))
    .bind(&status)
    .bind(status == "done")
    .bind(input.priority.unwrap_or(cur.priority))
    .bind(&due)
    .bind(input.category.as_deref().map(clean_category).unwrap_or(cur.category.clone()))
    .bind(&recurrence)
    .bind(input.notes.as_deref().unwrap_or(&cur.notes))
    .bind(&status)
    .bind(id)
    .execute(pool)
    .await?;

    if finished && recurrence != "none" {
        let from = due.as_deref().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).unwrap_or_else(today);
        // Skip dates in the past: the next one is after today.
        let mut next = next_due(from, &recurrence);
        while let Some(n) = next.filter(|n| *n <= today()) {
            next = next_due(n, &recurrence);
        }
        if let Some(next) = next {
            let t = get(pool, id).await?;
            // The repeat moves to the new task; the finished one stays as history.
            sqlx::query("UPDATE tasks SET recurrence = 'none' WHERE id = ?").bind(id).execute(pool).await?;
            create(
                pool,
                TaskInput {
                    title: Some(t.title),
                    priority: Some(t.priority),
                    due_at: Some(Some(next.to_string())),
                    category: Some(t.category),
                    recurrence: Some(recurrence),
                    notes: Some(t.notes),
                    ..Default::default()
                },
            )
            .await?;
        }
    }
    get(pool, id).await
}

/// Done ↔ todo (checkbox in lists and the widget).
pub async fn toggle(pool: &SqlitePool, id: &str) -> AppResult<Task> {
    let cur = get(pool, id).await?;
    let status = if cur.status == "done" { "todo" } else { "done" };
    update(pool, id, TaskInput { status: Some(status.into()), ..Default::default() }).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM tasks WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct DayCount {
    pub date: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct TaskStats {
    pub completed_today: i64,
    pub created_today: i64,
    pub open: i64,
    pub in_progress: i64,
    pub overdue: i64,
    pub due_today: i64,
    /// Open tasks by priority 0..=3.
    pub by_priority: [i64; 4],
    /// Completed per local day, oldest first, last 7 days.
    pub last_7_days: Vec<DayCount>,
    pub categories: Vec<String>,
}

/// UTC timestamp (SQLite format) of local midnight of `day`.
fn utc_start(day: NaiveDate) -> String {
    let local = Local.from_local_datetime(&day.and_hms_opt(0, 0, 0).unwrap()).earliest().unwrap();
    local.with_timezone(&Utc).format("%Y-%m-%d %H:%M:%S").to_string()
}

pub async fn stats(pool: &SqlitePool) -> AppResult<TaskStats> {
    let today = today();
    let tasks = all(pool).await?;
    let in_range = |ts: &Option<String>, day: NaiveDate| {
        let (a, b) = (utc_start(day), utc_start(day + Duration::days(1)));
        ts.as_deref().is_some_and(|t| t >= a.as_str() && t < b.as_str())
    };
    let open: Vec<&Task> = tasks.iter().filter(|t| t.status == "todo" || t.status == "in_progress").collect();
    let due = |t: &&Task| t.due_at.as_deref().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
    let mut by_priority = [0; 4];
    for t in &open {
        by_priority[t.priority.clamp(0, 3) as usize] += 1;
    }
    let mut categories: Vec<String> = tasks.iter().map(|t| t.category.clone()).filter(|c| !c.is_empty()).collect();
    categories.sort();
    categories.dedup();
    Ok(TaskStats {
        completed_today: tasks.iter().filter(|t| t.status == "done" && in_range(&t.completed_at, today)).count() as i64,
        created_today: tasks.iter().filter(|t| in_range(&Some(t.created_at.clone()), today)).count() as i64,
        open: open.len() as i64,
        in_progress: open.iter().filter(|t| t.status == "in_progress").count() as i64,
        overdue: open.iter().filter(|t| due(t).is_some_and(|d| d < today)).count() as i64,
        due_today: open.iter().filter(|t| due(t) == Some(today)).count() as i64,
        by_priority,
        last_7_days: (0..7)
            .rev()
            .map(|i| {
                let day = today - Duration::days(i);
                DayCount {
                    date: day.to_string(),
                    count: tasks.iter().filter(|t| t.status == "done" && in_range(&t.completed_at, day)).count() as i64,
                }
            })
            .collect(),
        categories,
    })
}

// --- commands ---

#[tauri::command]
pub async fn tasks_query(state: State<'_, AppState>, filter: Option<TaskFilter>) -> AppResult<Vec<Task>> {
    require(&state, TASKS)?;
    list(&state.db, &filter.unwrap_or_default()).await
}

#[tauri::command]
pub async fn tasks_create(state: State<'_, AppState>, input: TaskInput) -> AppResult<Task> {
    require(&state, TASKS)?;
    create(&state.db, input).await
}

#[tauri::command]
pub async fn tasks_quick_add(state: State<'_, AppState>, line: String) -> AppResult<Task> {
    require(&state, TASKS)?;
    quick_add(&state.db, &line).await
}

/// Preview of what quick add will create (shown while typing).
#[tauri::command]
pub fn tasks_quick_parse(line: String) -> quick_add::QuickTask {
    quick_add::parse(&line, today())
}

#[tauri::command]
pub async fn tasks_update(state: State<'_, AppState>, id: String, input: TaskInput) -> AppResult<Task> {
    require(&state, TASKS)?;
    update(&state.db, &id, input).await
}

#[tauri::command]
pub async fn tasks_remove(state: State<'_, AppState>, id: String) -> AppResult<()> {
    require(&state, TASKS)?;
    delete(&state.db, &id).await
}

#[tauri::command]
pub async fn tasks_stats(state: State<'_, AppState>) -> AppResult<TaskStats> {
    require(&state, TASKS)?;
    stats(&state.db).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(title: &str) -> TaskInput {
        TaskInput { title: Some(title.into()), ..Default::default() }
    }

    #[tokio::test]
    async fn statuses_filters_and_stats() {
        let pool = crate::db::test_pool().await;
        let a = create(&pool, TaskInput { priority: Some(3), category: Some(" Работа / Отчёты/".into()), ..input("Отчёт") }).await.unwrap();
        assert_eq!(a.category, "Работа/Отчёты");
        let b = create(&pool, TaskInput { due_at: Some(Some("2000-01-01".into())), ..input("Старая") }).await.unwrap();
        create(&pool, input("Третья")).await.unwrap();
        assert!(create(&pool, TaskInput { priority: Some(5), ..input("x") }).await.is_err());

        let f = |f: TaskFilter| {
            let pool = pool.clone();
            async move { list(&pool, &f).await.unwrap().into_iter().map(|t| t.title).collect::<Vec<_>>() }
        };
        assert_eq!(f(TaskFilter { category: Some("Работа".into()), ..Default::default() }).await, ["Отчёт"]);
        assert_eq!(f(TaskFilter { due: Some("overdue".into()), ..Default::default() }).await, ["Старая"]);
        assert_eq!(f(TaskFilter { priority: Some(2), ..Default::default() }).await, ["Отчёт"]);

        update(&pool, &a.id, TaskInput { status: Some("in_progress".into()), ..Default::default() }).await.unwrap();
        let done = toggle(&pool, &b.id).await.unwrap();
        assert!(done.done && done.completed_at.is_some());
        assert_eq!(f(TaskFilter::default()).await.len(), 2, "open = todo + in progress");

        let s = stats(&pool).await.unwrap();
        assert_eq!((s.completed_today, s.open, s.in_progress, s.overdue), (1, 2, 1, 0));
        assert_eq!(s.last_7_days.last().unwrap().count, 1);
        assert_eq!(s.by_priority[3], 1);

        let undone = toggle(&pool, &b.id).await.unwrap();
        assert!(!undone.done && undone.completed_at.is_none());
    }

    #[tokio::test]
    async fn repeating_task_creates_next() {
        let pool = crate::db::test_pool().await;
        let t = quick_add(&pool, "Зарядка *ежедневно !1").await.unwrap();
        assert_eq!(t.due_at, Some(today().to_string()));
        toggle(&pool, &t.id).await.unwrap();
        let open = list(&pool, &TaskFilter::default()).await.unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].due_at, Some((today() + Duration::days(1)).to_string()));
        assert_eq!(open[0].recurrence, "daily");
        assert_eq!(get(&pool, &t.id).await.unwrap().recurrence, "none");
    }
}
