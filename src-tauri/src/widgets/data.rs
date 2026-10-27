//! Data providers and actions of every widget kind. Providers are async and
//! slow work (Windows API, network) runs off the UI thread, so the UI never
//! waits on it. Data that can fail (weather) is cached for offline use.

use chrono::{Datelike, Local, NaiveDate, NaiveTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use super::{Widget, clipboard, def, set_config, system, valid_app_path, valid_link, weather};
use crate::db::new_id;
use crate::error::{AppError, AppResult};
use crate::modules::{catalog::*, registry};
use crate::permissions::{self, PERM_CLIPBOARD_READ, PERM_LAUNCH_APPS, PERM_MEDIA};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct WidgetData {
    pub widget_id: String,
    pub kind: String,
    pub data: Value,
    /// When the data was produced (RFC 3339); older than now for cached data.
    pub fetched_at: Option<String>,
    pub from_cache: bool,
    /// Why fresh data isn't available (offline, no permission); `data` may still hold cached data.
    pub error: Option<String>,
    /// How often the UI should refresh; 0 = only after actions.
    pub refresh_secs: u32,
}

pub struct Fetched {
    pub data: Value,
    pub fetched_at: Option<String>,
    pub from_cache: bool,
    pub error: Option<String>,
}

impl Fetched {
    fn now(data: Value) -> Self {
        Self { data, fetched_at: Some(Utc::now().to_rfc3339()), from_cache: false, error: None }
    }
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

pub async fn load(app: &AppHandle, w: &Widget, params: &Value, force: bool) -> AppResult<WidgetData> {
    let def = def(&w.kind)?;
    let db = &app.state::<AppState>().db;
    let settings = registry::settings(db, def.module).await?;
    let refresh_secs = def.refresh_secs(&settings);
    let limit = |key: &str| settings.get(key).and_then(Value::as_i64).unwrap_or(20);

    let fetched = match w.kind.as_str() {
        "clock" | "date" => {
            let now = Local::now();
            Fetched::now(json!({
                "now": now.to_rfc3339(),
                "epoch_ms": now.timestamp_millis(),
                "utc_offset_minutes": now.offset().local_minus_utc() / 60,
                "iso_week": now.iso_week().week(),
                "weekday": now.weekday().number_from_monday(),
                "day_of_year": now.ordinal(),
            }))
        }
        "weather" => weather::get(db, &settings, refresh_secs, force).await?,
        "notes" => Fetched::now(json!({ "note": note(app, w).await? })),
        "todo" => {
            let tasks = crate::workspace::tasks(db).await?;
            let hide_done = registry::settings(db, TASKS).await?.get("hide_completed").and_then(Value::as_bool).unwrap_or(false);
            let done = tasks.iter().filter(|t| t.done).count();
            Fetched::now(json!({ "tasks": tasks, "open": tasks.len() - done, "done": done, "hide_completed": hide_done }))
        }
        "quick_links" => Fetched::now(json!({ "links": w.config.get("links").cloned().unwrap_or(json!([])) })),
        "app_launcher" => {
            let apps: Vec<Value> = w
                .config
                .get("apps")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|mut a| {
                    let exists = a.get("path").and_then(Value::as_str).is_some_and(|p| std::path::Path::new(p).exists());
                    a["exists"] = json!(exists);
                    a
                })
                .collect();
            let granted = permissions::is_granted(db, PERM_LAUNCH_APPS).await?;
            Fetched::now(json!({ "apps": apps, "permission": granted }))
        }
        "git" => Fetched::now(crate::devtools::git::summary(app).await?),
        "system_info" => {
            let info = tauri::async_runtime::spawn_blocking(system::snapshot).await.map_err(anyhow::Error::from)?;
            Fetched::now(serde_json::to_value(info)?)
        }
        "clipboard" => {
            let permission = permissions::is_granted(db, PERM_CLIPBOARD_READ).await?;
            let recording = permission && crate::background::running(app, CLIPBOARD);
            let items = clipboard::list(db, limit("max_visible")).await?;
            Fetched::now(json!({ "items": items, "permission": permission, "recording": recording }))
        }
        "notifications" => {
            let unread_only = settings.get("unread_only").and_then(Value::as_bool).unwrap_or(false);
            let items = crate::notifications::list(db, limit("max_visible"), unread_only).await?;
            let unread = crate::notifications::unread(db).await?;
            Fetched::now(json!({ "items": items, "unread": unread }))
        }
        "calendar" => Fetched::now(calendar(db, params).await?),
        "countdown" => Fetched::now(countdown_state(app, w, &settings).await?),
        "stopwatch" => Fetched::now(json!({
            "elapsed_ms": w.config.get("elapsed_ms").cloned().unwrap_or(json!(0)),
            "started_at": w.config.get("started_at").cloned().unwrap_or(Value::Null),
            "laps": w.config.get("laps").cloned().unwrap_or(json!([])),
            "now_ms": now_ms(),
        })),
        "status" => Fetched::now(serde_json::to_value(crate::info::stats(app).await?)?),
        "media" => {
            if !permissions::is_granted(db, PERM_MEDIA).await? {
                Fetched::now(json!({ "permission": false }))
            } else {
                match crate::media::now_playing(settings.get("show_cover").and_then(Value::as_bool).unwrap_or(true)).await {
                    Ok(now) => Fetched::now(json!({ "permission": true, "now": now })),
                    Err(e) => Fetched { error: Some(e.to_string()), ..Fetched::now(json!({ "permission": true, "now": null })) },
                }
            }
        }
        other => return Err(AppError::Invalid(format!("неизвестный тип виджета {other}"))),
    };
    Ok(WidgetData {
        widget_id: w.id.clone(),
        kind: w.kind.clone(),
        data: fetched.data,
        fetched_at: fetched.fetched_at,
        from_cache: fetched.from_cache,
        error: fetched.error,
        refresh_secs,
    })
}

// --- helpers for payloads ---

fn field<'a>(p: &'a Value, key: &str) -> AppResult<&'a Value> {
    p.get(key).ok_or_else(|| AppError::Invalid(format!("нет поля «{key}»")))
}

fn text<'a>(p: &'a Value, key: &str) -> AppResult<&'a str> {
    field(p, key)?.as_str().ok_or_else(|| AppError::Invalid(format!("поле «{key}» должно быть строкой")))
}

fn int(p: &Value, key: &str) -> AppResult<i64> {
    field(p, key)?.as_i64().ok_or_else(|| AppError::Invalid(format!("поле «{key}» должно быть числом")))
}

fn unknown(kind: &str, action: &str) -> AppError {
    AppError::Invalid(format!("виджет «{kind}» не поддерживает действие «{action}»"))
}

// --- notes ---

#[derive(Debug, Serialize, sqlx::FromRow)]
struct Note {
    id: String,
    title: String,
    body: String,
    updated_at: String,
}

/// The widget's note; recreated if it went missing.
async fn note(app: &AppHandle, w: &Widget) -> AppResult<Note> {
    let db = &app.state::<AppState>().db;
    let id = w.config.get("note_id").and_then(Value::as_str).map(String::from);
    if let Some(id) = &id {
        if let Some(n) = sqlx::query_as::<_, Note>("SELECT id, title, body, updated_at FROM notes WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await?
        {
            return Ok(n);
        }
    }
    let id = id.unwrap_or_else(new_id);
    sqlx::query("INSERT INTO notes (id, title) VALUES (?, ?)").bind(&id).bind(&w.title).execute(db).await?;
    let mut config = w.config.clone();
    config["note_id"] = json!(id);
    set_config(db, &w.id, &config).await?;
    Ok(sqlx::query_as::<_, Note>("SELECT id, title, body, updated_at FROM notes WHERE id = ?").bind(&id).fetch_one(db).await?)
}

// --- calendar ---

#[derive(Debug, Serialize, sqlx::FromRow)]
struct Event {
    id: String,
    title: String,
    date: String,
    time: Option<String>,
    note: String,
}

async fn calendar(db: &sqlx::SqlitePool, params: &Value) -> AppResult<Value> {
    let today = Local::now().date_naive();
    let first = match params.get("month").and_then(Value::as_str) {
        Some(m) => NaiveDate::parse_from_str(&format!("{m}-01"), "%Y-%m-%d")
            .map_err(|_| AppError::Invalid(format!("месяц «{m}» должен быть в формате ГГГГ-ММ")))?,
        None => today.with_day(1).unwrap(),
    };
    let next = first.checked_add_months(chrono::Months::new(1)).unwrap();
    let events = sqlx::query_as::<_, Event>(
        "SELECT id, title, date, time, note FROM calendar_events WHERE date >= ? AND date < ? ORDER BY date, time",
    )
    .bind(first.to_string())
    .bind(next.to_string())
    .fetch_all(db)
    .await?;
    let upcoming = sqlx::query_as::<_, Event>(
        "SELECT id, title, date, time, note FROM calendar_events WHERE date >= ? ORDER BY date, time LIMIT 5",
    )
    .bind(today.to_string())
    .fetch_all(db)
    .await?;
    Ok(json!({ "month": first.format("%Y-%m").to_string(), "today": today.to_string(), "events": events, "upcoming": upcoming }))
}

async fn calendar_add(db: &sqlx::SqlitePool, p: &Value) -> AppResult<()> {
    let title = text(p, "title")?.trim();
    if title.is_empty() || title.chars().count() > 100 {
        return Err(AppError::Invalid("название события — от 1 до 100 символов".into()));
    }
    let date = NaiveDate::parse_from_str(text(p, "date")?, "%Y-%m-%d").map_err(|_| AppError::Invalid("дата должна быть в формате ГГГГ-ММ-ДД".into()))?;
    let time = match p.get("time").and_then(Value::as_str).filter(|t| !t.is_empty()) {
        Some(t) => Some(
            NaiveTime::parse_from_str(t, "%H:%M")
                .map_err(|_| AppError::Invalid("время должно быть в формате ЧЧ:ММ".into()))?
                .format("%H:%M")
                .to_string(),
        ),
        None => None,
    };
    let note = p.get("note").and_then(Value::as_str).unwrap_or("");
    if note.chars().count() > 500 {
        return Err(AppError::Invalid("описание — не больше 500 символов".into()));
    }
    sqlx::query("INSERT INTO calendar_events (id, title, date, time, note) VALUES (?, ?, ?, ?, ?)")
        .bind(new_id())
        .bind(title)
        .bind(date.to_string())
        .bind(time)
        .bind(note)
        .execute(db)
        .await?;
    Ok(())
}

// --- countdown and stopwatch: state lives in the widget config ---

const MAX_COUNTDOWN_MS: i64 = 100 * 3600 * 1000 - 1000;

fn i(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(Value::as_i64).unwrap_or(0)
}

/// Current countdown state; a timer that ran out is finished here (once),
/// with a notification if the module setting asks for it.
async fn countdown_state(app: &AppHandle, w: &Widget, settings: &Value) -> AppResult<Value> {
    let mut c = w.config.clone();
    let now = now_ms();
    if c.get("running").and_then(Value::as_bool) == Some(true) && i(&c, "ends_at") <= now {
        c["running"] = json!(false);
        c["remaining_ms"] = json!(0);
        c["finished"] = json!(true);
        set_config(&app.state::<AppState>().db, &w.id, &c).await?;
        if settings.get("notify_on_finish").and_then(Value::as_bool).unwrap_or(true) {
            let label = c.get("label").and_then(Value::as_str).filter(|l| !l.is_empty()).unwrap_or(&w.title).to_string();
            crate::notifications::push(app, "info", "Таймер завершён", &label, "Виджет «Таймер»").await?;
        }
    }
    c["now_ms"] = json!(now);
    Ok(c)
}

fn countdown_act(c: &mut Value, action: &str, p: &Value) -> AppResult<()> {
    let now = now_ms();
    let running = c.get("running").and_then(Value::as_bool) == Some(true);
    match action {
        "start" if !running => {
            let mut remaining = i(c, "remaining_ms");
            if remaining <= 0 || c.get("finished").and_then(Value::as_bool) == Some(true) {
                remaining = i(c, "duration_ms");
            }
            c["ends_at"] = json!(now + remaining);
            c["running"] = json!(true);
            c["finished"] = json!(false);
        }
        "pause" if running => {
            c["remaining_ms"] = json!((i(c, "ends_at") - now).max(0));
            c["running"] = json!(false);
        }
        "start" | "pause" => {}
        "reset" => {
            c["running"] = json!(false);
            c["finished"] = json!(false);
            c["remaining_ms"] = json!(i(c, "duration_ms"));
        }
        "set" => {
            let duration = int(p, "duration_ms")?;
            if !(1000..=MAX_COUNTDOWN_MS).contains(&duration) {
                return Err(AppError::Invalid("длительность — от 1 секунды до 99:59:59".into()));
            }
            let label = p.get("label").and_then(Value::as_str).unwrap_or("").trim();
            if label.chars().count() > 60 {
                return Err(AppError::Invalid("подпись — не больше 60 символов".into()));
            }
            c["duration_ms"] = json!(duration);
            c["label"] = json!(label);
            c["running"] = json!(false);
            c["finished"] = json!(false);
            c["remaining_ms"] = json!(duration);
        }
        other => return Err(unknown("countdown", other)),
    }
    Ok(())
}

fn stopwatch_act(c: &mut Value, action: &str) -> AppResult<()> {
    let now = now_ms();
    let started = c.get("started_at").and_then(Value::as_i64);
    let total = i(c, "elapsed_ms") + started.map(|s| now - s).unwrap_or(0);
    match action {
        "start" if started.is_none() => c["started_at"] = json!(now),
        "pause" if started.is_some() => {
            c["elapsed_ms"] = json!(total);
            c["started_at"] = Value::Null;
        }
        "start" | "pause" => {}
        "reset" => {
            c["elapsed_ms"] = json!(0);
            c["started_at"] = Value::Null;
            c["laps"] = json!([]);
        }
        "lap" if started.is_some() => {
            let laps = c["laps"].as_array_mut();
            match laps {
                Some(l) if l.len() < 100 => l.push(json!(total)),
                Some(_) => return Err(AppError::Invalid("не больше 100 кругов".into())),
                None => c["laps"] = json!([total]),
            }
        }
        "lap" => {}
        other => return Err(unknown("stopwatch", other)),
    }
    Ok(())
}

// --- actions ---

/// Runs a widget action. Actions by kind:
/// - notes: `save { body }`
/// - todo: `add { title }`, `toggle { id }`, `delete { id }`, `clear_done`
/// - quick_links: `open { index }`; app_launcher: `launch { index }`
/// - clipboard: `copy { id }`, `pin { id, pinned }`, `delete { id }`, `clear`
/// - notifications: `read_all`, `delete { id }`, `clear`
/// - calendar: `add { title, date, time?, note? }`, `delete { id }`
/// - countdown: `start`, `pause`, `reset`, `set { duration_ms, label? }`
/// - stopwatch: `start`, `pause`, `reset`, `lap`
pub async fn act(app: &AppHandle, w: &Widget, action: &str, p: &Value) -> AppResult<()> {
    let db = &app.state::<AppState>().db;
    match (w.kind.as_str(), action) {
        ("notes", "save") => {
            let body = text(p, "body")?;
            if body.chars().count() > 100_000 {
                return Err(AppError::Invalid("заметка — не больше 100 000 символов".into()));
            }
            let note = note(app, w).await?;
            sqlx::query("UPDATE notes SET body = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(body)
                .bind(&note.id)
                .execute(db)
                .await?;
        }
        ("media", control) => {
            crate::media::control(app, crate::media::Control::parse(control)?).await?;
        }
        ("todo", "add") => crate::workspace::add_task(db, text(p, "title")?).await?,
        ("todo", "toggle") => crate::workspace::toggle_task(db, text(p, "id")?).await?,
        ("todo", "delete") => crate::workspace::delete_task(db, text(p, "id")?).await?,
        ("todo", "clear_done") => {
            sqlx::query("DELETE FROM tasks WHERE done = 1").execute(db).await?;
        }
        ("quick_links", "open") => {
            let link = w.config["links"].get(int(p, "index")? as usize).ok_or_else(|| AppError::NotFound("ссылка".into()))?;
            let url = link.get("url").and_then(Value::as_str).unwrap_or("");
            if !valid_link(url) {
                return Err(AppError::Invalid("адрес ссылки не подходит".into()));
            }
            app.opener().open_url(url, None::<&str>).map_err(anyhow::Error::from)?;
        }
        ("app_launcher", "launch") => {
            permissions::require(db, PERM_LAUNCH_APPS, "widget:app_launcher").await?;
            let entry = w.config["apps"].get(int(p, "index")? as usize).ok_or_else(|| AppError::NotFound("программа".into()))?;
            let path = entry.get("path").and_then(Value::as_str).unwrap_or("");
            valid_app_path(path)?;
            if !std::path::Path::new(path).exists() {
                return Err(AppError::NotFound(format!("файл {path}")));
            }
            tracing::info!(path, "launching app from widget");
            app.opener().open_path(path, None::<&str>).map_err(anyhow::Error::from)?;
        }
        ("clipboard", "copy") => clipboard::copy_entry(db, int(p, "id")?).await?,
        ("clipboard", "pin") => {
            let pinned = field(p, "pinned")?.as_bool().unwrap_or(true);
            clipboard::set_pinned(db, int(p, "id")?, pinned).await?;
        }
        ("clipboard", "delete") => clipboard::delete(db, int(p, "id")?).await?,
        ("clipboard", "clear") => clipboard::clear(db).await?,
        ("notifications", "read_all") => crate::notifications::mark_all_read(db).await?,
        ("notifications", "delete") => crate::notifications::delete(db, int(p, "id")?).await?,
        ("notifications", "clear") => crate::notifications::clear(db).await?,
        ("calendar", "add") => calendar_add(db, p).await?,
        ("calendar", "delete") => {
            sqlx::query("DELETE FROM calendar_events WHERE id = ?").bind(text(p, "id")?).execute(db).await?;
        }
        ("countdown", a) => {
            let mut c = w.config.clone();
            countdown_act(&mut c, a, p)?;
            set_config(db, &w.id, &c).await?;
        }
        ("stopwatch", a) => {
            let mut c = w.config.clone();
            stopwatch_act(&mut c, a)?;
            set_config(db, &w.id, &c).await?;
        }
        (kind, a) => return Err(unknown(kind, a)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown_flow() {
        let mut c = super::super::initial_config("countdown");
        countdown_act(&mut c, "set", &json!({ "duration_ms": 60_000, "label": "Чай" })).unwrap();
        assert_eq!(c["remaining_ms"], 60_000);
        countdown_act(&mut c, "start", &Value::Null).unwrap();
        assert_eq!(c["running"], true);
        let ends = i(&c, "ends_at");
        assert!(ends - now_ms() <= 60_000);
        countdown_act(&mut c, "pause", &Value::Null).unwrap();
        assert_eq!(c["running"], false);
        assert!(i(&c, "remaining_ms") > 0 && i(&c, "remaining_ms") <= 60_000);
        countdown_act(&mut c, "reset", &Value::Null).unwrap();
        assert_eq!(c["remaining_ms"], 60_000);
        assert!(countdown_act(&mut c, "set", &json!({ "duration_ms": 10 })).is_err());
        assert!(countdown_act(&mut c, "explode", &Value::Null).is_err());
    }

    #[test]
    fn stopwatch_flow() {
        let mut c = super::super::initial_config("stopwatch");
        stopwatch_act(&mut c, "lap").unwrap(); // not running: ignored
        assert_eq!(c["laps"], json!([]));
        stopwatch_act(&mut c, "start").unwrap();
        c["started_at"] = json!(now_ms() - 1500);
        stopwatch_act(&mut c, "lap").unwrap();
        stopwatch_act(&mut c, "pause").unwrap();
        assert!(i(&c, "elapsed_ms") >= 1500);
        assert!(c["started_at"].is_null());
        assert_eq!(c["laps"].as_array().unwrap().len(), 1);
        stopwatch_act(&mut c, "reset").unwrap();
        assert_eq!(c["elapsed_ms"], 0);
    }

    #[tokio::test]
    async fn calendar_events() {
        let pool = crate::db::test_pool().await;
        calendar_add(&pool, &json!({ "title": "Встреча", "date": "2026-10-21", "time": "9:05" })).await.unwrap();
        calendar_add(&pool, &json!({ "title": "Отпуск", "date": "2026-11-02" })).await.unwrap();
        assert!(calendar_add(&pool, &json!({ "title": "x", "date": "21.10.2026" })).await.is_err());
        let oct = calendar(&pool, &json!({ "month": "2026-10" })).await.unwrap();
        assert_eq!(oct["events"].as_array().unwrap().len(), 1);
        assert_eq!(oct["events"][0]["time"], "09:05");
        assert!(calendar(&pool, &json!({ "month": "октябрь" })).await.is_err());
    }
}
