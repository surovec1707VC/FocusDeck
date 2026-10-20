//! Timers (countdown and Pomodoro) with session history, and independent
//! stopwatches with saved results. State is stored as timestamps; a
//! background task of the Timers module finishes expired timers, writes the
//! session and sends a notification even while the window is hidden.

use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, State};

use super::require;
use crate::db::new_id;
use crate::error::{AppError, AppResult};
use crate::modules::{catalog::TIMERS, registry};
use crate::state::AppState;

/// Emitted when a timer finishes or moves to the next Pomodoro phase.
pub const EVENT: &str = "focusdeck://timers-changed";
const MAX_MS: i64 = 100 * 3_600_000 - 1000;

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

fn ms_to_sql(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Timer {
    pub id: String,
    pub name: String,
    /// countdown | pomodoro
    pub kind: String,
    pub duration_ms: i64,
    pub work_ms: i64,
    pub short_break_ms: i64,
    pub long_break_ms: i64,
    pub rounds: i64,
    /// work | short_break | long_break (Pomodoro)
    pub phase: String,
    pub round: i64,
    pub running: bool,
    pub ends_at: Option<i64>,
    pub remaining_ms: Option<i64>,
    pub phase_started_at: Option<i64>,
}

impl Timer {
    /// Length of the current interval.
    pub fn interval_ms(&self) -> i64 {
        match (self.kind.as_str(), self.phase.as_str()) {
            ("countdown", _) => self.duration_ms,
            (_, "short_break") => self.short_break_ms,
            (_, "long_break") => self.long_break_ms,
            _ => self.work_ms,
        }
    }

    fn remaining(&self, now: i64) -> i64 {
        if self.running {
            (self.ends_at.unwrap_or(now) - now).max(0)
        } else {
            self.remaining_ms.unwrap_or_else(|| self.interval_ms())
        }
    }

    fn session_kind(&self) -> &str {
        if self.kind == "countdown" { "countdown" } else { &self.phase }
    }
}

#[derive(Debug, Deserialize)]
pub struct TimerInput {
    pub name: String,
    pub kind: String,
    #[serde(default = "d_duration")]
    pub duration_ms: i64,
    #[serde(default = "d_work")]
    pub work_ms: i64,
    #[serde(default = "d_short")]
    pub short_break_ms: i64,
    #[serde(default = "d_long")]
    pub long_break_ms: i64,
    #[serde(default = "d_rounds")]
    pub rounds: i64,
}
fn d_duration() -> i64 {
    300_000
}
fn d_work() -> i64 {
    1_500_000
}
fn d_short() -> i64 {
    300_000
}
fn d_long() -> i64 {
    900_000
}
fn d_rounds() -> i64 {
    4
}

fn check(i: &TimerInput) -> AppResult<()> {
    let n = i.name.trim().chars().count();
    if n == 0 || n > 60 {
        return Err(AppError::Invalid("название таймера — от 1 до 60 символов".into()));
    }
    if !["countdown", "pomodoro"].contains(&i.kind.as_str()) {
        return Err(AppError::Invalid("тип таймера: countdown или pomodoro".into()));
    }
    for (v, what) in [(i.duration_ms, "длительность"), (i.work_ms, "работа"), (i.short_break_ms, "короткий перерыв"), (i.long_break_ms, "длинный перерыв")] {
        if !(1000..=MAX_MS).contains(&v) {
            return Err(AppError::Invalid(format!("{what}: от 1 секунды до 99:59:59")));
        }
    }
    if !(1..=12).contains(&i.rounds) {
        return Err(AppError::Invalid("раундов до длинного перерыва — от 1 до 12".into()));
    }
    Ok(())
}

pub async fn timers(pool: &SqlitePool) -> AppResult<Vec<Timer>> {
    Ok(sqlx::query_as::<_, Timer>(
        "SELECT id, name, kind, duration_ms, work_ms, short_break_ms, long_break_ms, rounds, phase, round, running, ends_at, remaining_ms, phase_started_at
         FROM timers ORDER BY created_at, rowid",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn timer(pool: &SqlitePool, id: &str) -> AppResult<Timer> {
    timers(pool).await?.into_iter().find(|t| t.id == id).ok_or_else(|| AppError::NotFound("таймер".into()))
}

/// Creates a timer, or changes its intervals (this resets it).
pub async fn save_timer(pool: &SqlitePool, id: Option<&str>, i: TimerInput) -> AppResult<Timer> {
    check(&i)?;
    let id = match id {
        Some(id) => {
            timer(pool, id).await?;
            sqlx::query(
                "UPDATE timers SET name = ?, kind = ?, duration_ms = ?, work_ms = ?, short_break_ms = ?, long_break_ms = ?, rounds = ?,
                    phase = 'work', round = 1, running = 0, ends_at = NULL, remaining_ms = NULL, phase_started_at = NULL,
                    updated_at = datetime('now') WHERE id = ?",
            )
            .bind(i.name.trim())
            .bind(&i.kind)
            .bind(i.duration_ms)
            .bind(i.work_ms)
            .bind(i.short_break_ms)
            .bind(i.long_break_ms)
            .bind(i.rounds)
            .bind(id)
            .execute(pool)
            .await?;
            id.to_string()
        }
        None => {
            let id = new_id();
            sqlx::query("INSERT INTO timers (id, name, kind, duration_ms, work_ms, short_break_ms, long_break_ms, rounds) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(&id)
                .bind(i.name.trim())
                .bind(&i.kind)
                .bind(i.duration_ms)
                .bind(i.work_ms)
                .bind(i.short_break_ms)
                .bind(i.long_break_ms)
                .bind(i.rounds)
                .execute(pool)
                .await?;
            id
        }
    };
    timer(pool, &id).await
}

async fn store(pool: &SqlitePool, t: &Timer) -> AppResult<()> {
    sqlx::query(
        "UPDATE timers SET phase = ?, round = ?, running = ?, ends_at = ?, remaining_ms = ?, phase_started_at = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&t.phase)
    .bind(t.round)
    .bind(t.running)
    .bind(t.ends_at)
    .bind(t.remaining_ms)
    .bind(t.phase_started_at)
    .bind(&t.id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Writes the current interval to the history (nothing if it never started).
async fn log_session(pool: &SqlitePool, t: &Timer, now: i64, completed: bool) -> AppResult<()> {
    let Some(started) = t.phase_started_at else { return Ok(()) };
    let planned = t.interval_ms();
    let actual = if completed { planned } else { planned - t.remaining(now) };
    if actual <= 0 {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO timer_sessions (timer_id, timer_name, kind, planned_ms, actual_ms, completed, started_at, ended_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&t.id)
    .bind(&t.name)
    .bind(t.session_kind())
    .bind(planned)
    .bind(actual)
    .bind(completed)
    .bind(ms_to_sql(started))
    .bind(ms_to_sql(now))
    .execute(pool)
    .await?;
    Ok(())
}

/// Next Pomodoro phase: work → short break (long after `rounds`) → work.
fn advance(t: &mut Timer) {
    if t.phase == "work" {
        t.phase = if t.round >= t.rounds { "long_break" } else { "short_break" }.into();
    } else {
        t.round = if t.phase == "long_break" { 1 } else { t.round + 1 };
        t.phase = "work".into();
    }
    t.running = false;
    t.ends_at = None;
    t.remaining_ms = None;
    t.phase_started_at = None;
}

fn start(t: &mut Timer, now: i64) {
    if t.running {
        return;
    }
    let remaining = t.remaining(now);
    if t.remaining_ms.is_none() {
        t.phase_started_at = Some(now);
    }
    t.running = true;
    t.ends_at = Some(now + remaining);
    t.remaining_ms = Some(remaining);
}

/// start | pause | reset | skip (Pomodoro: next phase).
pub async fn control(pool: &SqlitePool, id: &str, action: &str) -> AppResult<Timer> {
    let mut t = timer(pool, id).await?;
    let now = now_ms();
    match action {
        "start" => start(&mut t, now),
        "pause" if t.running => {
            t.remaining_ms = Some(t.remaining(now));
            t.running = false;
            t.ends_at = None;
        }
        "pause" => {}
        "reset" => {
            log_session(pool, &t, now, false).await?;
            t.running = false;
            t.ends_at = None;
            t.remaining_ms = None;
            t.phase_started_at = None;
            if t.kind == "pomodoro" {
                t.phase = "work".into();
                t.round = 1;
            }
        }
        "skip" if t.kind == "pomodoro" => {
            log_session(pool, &t, now, false).await?;
            advance(&mut t);
        }
        other => return Err(AppError::Invalid(format!("действие «{other}» не поддерживается"))),
    }
    store(pool, &t).await?;
    Ok(t)
}

/// Finishes expired timers. Returns (timer, finished phase) for notifications.
pub async fn finish_expired(pool: &SqlitePool, auto_start_next: bool) -> AppResult<Vec<(Timer, String)>> {
    let now = now_ms();
    let mut done = Vec::new();
    for mut t in timers(pool).await?.into_iter().filter(|t| t.running && t.ends_at.is_some_and(|e| e <= now)) {
        log_session(pool, &t, now, true).await?;
        let finished = t.session_kind().to_string();
        if t.kind == "pomodoro" {
            advance(&mut t);
            if auto_start_next {
                start(&mut t, now);
            }
        } else {
            t.running = false;
            t.ends_at = None;
            t.remaining_ms = None;
            t.phase_started_at = None;
        }
        store(pool, &t).await?;
        done.push((t, finished));
    }
    Ok(done)
}

pub async fn delete_timer(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let t = timer(pool, id).await?;
    log_session(pool, &t, now_ms(), false).await?;
    sqlx::query("DELETE FROM timers WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Session {
    pub id: i64,
    pub timer_id: Option<String>,
    pub timer_name: String,
    pub kind: String,
    pub planned_ms: i64,
    pub actual_ms: i64,
    pub completed: bool,
    pub started_at: String,
    pub ended_at: String,
}

pub async fn sessions(pool: &SqlitePool, limit: i64) -> AppResult<Vec<Session>> {
    Ok(sqlx::query_as::<_, Session>(
        "SELECT id, timer_id, timer_name, kind, planned_ms, actual_ms, completed, started_at, ended_at FROM timer_sessions ORDER BY id DESC LIMIT ?",
    )
    .bind(limit.clamp(1, 1000))
    .fetch_all(pool)
    .await?)
}

#[derive(Debug, Serialize)]
pub struct SessionStats {
    /// Completed Pomodoro work intervals today.
    pub pomodoros_today: i64,
    /// Time in work intervals and countdowns today.
    pub focus_ms_today: i64,
    pub sessions_today: i64,
}

pub async fn session_stats(pool: &SqlitePool) -> AppResult<SessionStats> {
    let start = chrono::Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
    let start = start.and_local_timezone(chrono::Local).earliest().unwrap().with_timezone(&Utc).format("%Y-%m-%d %H:%M:%S").to_string();
    let (pomodoros, focus, total): (i64, Option<i64>, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(kind = 'work' AND completed = 1), 0),
                SUM(CASE WHEN kind IN ('work', 'countdown') THEN actual_ms END),
                COUNT(*)
         FROM timer_sessions WHERE ended_at >= ?",
    )
    .bind(start)
    .fetch_one(pool)
    .await?;
    Ok(SessionStats { pomodoros_today: pomodoros, focus_ms_today: focus.unwrap_or(0), sessions_today: total })
}

pub async fn prune_history(pool: &SqlitePool, days: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM timer_sessions WHERE ended_at < datetime('now', ?)").bind(format!("-{days} days")).execute(pool).await?;
    Ok(())
}

/// Background task of the Timers module.
pub async fn tick_loop(app: AppHandle) {
    let mut ticks = 0u64;
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        if let Err(e) = tick(&app, ticks % 3600 == 0).await {
            tracing::warn!("timers: {e}");
        }
        ticks += 1;
    }
}

async fn tick(app: &AppHandle, prune: bool) -> AppResult<()> {
    let db = &app.state::<AppState>().db;
    let settings = registry::settings(db, TIMERS).await?;
    let flag = |k: &str, d: bool| settings.get(k).and_then(Value::as_bool).unwrap_or(d);
    if prune {
        prune_history(db, settings["history_days"].as_i64().unwrap_or(90)).await?;
    }
    let finished = finish_expired(db, flag("auto_start_next", false)).await?;
    if finished.is_empty() {
        return Ok(());
    }
    for (t, phase) in &finished {
        if flag("notify", true) {
            let (title, body) = match phase.as_str() {
                "work" => ("Помидор завершён", format!("{}: пора отдохнуть", t.name)),
                "short_break" | "long_break" => ("Перерыв окончен", format!("{}: пора работать", t.name)),
                _ => ("Таймер завершён", t.name.clone()),
            };
            crate::notifications::push(app, "info", title, &body, "Таймеры").await?;
        }
    }
    let _ = app.emit(EVENT, ());
    Ok(())
}

// --- stopwatches ---

#[derive(sqlx::FromRow)]
struct StopwatchRow {
    id: String,
    name: String,
    elapsed_ms: i64,
    started_at: Option<i64>,
    laps_json: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Stopwatch {
    pub id: String,
    pub name: String,
    /// Accumulated time before the current run.
    pub elapsed_ms: i64,
    /// Epoch ms of the current run; `None` while paused.
    pub started_at: Option<i64>,
    /// Total time at each lap.
    pub laps: Vec<i64>,
}

impl Stopwatch {
    pub fn total(&self, now: i64) -> i64 {
        self.elapsed_ms + self.started_at.map(|s| now - s).unwrap_or(0)
    }
}

fn to_stopwatch(r: StopwatchRow) -> Stopwatch {
    Stopwatch { laps: serde_json::from_str(&r.laps_json).unwrap_or_default(), id: r.id, name: r.name, elapsed_ms: r.elapsed_ms, started_at: r.started_at }
}

pub async fn stopwatches(pool: &SqlitePool) -> AppResult<Vec<Stopwatch>> {
    Ok(sqlx::query_as::<_, StopwatchRow>("SELECT id, name, elapsed_ms, started_at, laps_json FROM stopwatches ORDER BY position, created_at")
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(to_stopwatch)
        .collect())
}

pub async fn stopwatch(pool: &SqlitePool, id: &str) -> AppResult<Stopwatch> {
    stopwatches(pool).await?.into_iter().find(|s| s.id == id).ok_or_else(|| AppError::NotFound("секундомер".into()))
}

fn check_name(name: &str) -> AppResult<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 60 {
        return Err(AppError::Invalid("название — от 1 до 60 символов".into()));
    }
    Ok(n.to_string())
}

pub async fn create_stopwatch(pool: &SqlitePool, name: &str) -> AppResult<Stopwatch> {
    let id = new_id();
    sqlx::query("INSERT INTO stopwatches (id, name, position) VALUES (?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM stopwatches))")
        .bind(&id)
        .bind(check_name(name)?)
        .execute(pool)
        .await?;
    stopwatch(pool, &id).await
}

pub async fn rename_stopwatch(pool: &SqlitePool, id: &str, name: &str) -> AppResult<Stopwatch> {
    sqlx::query("UPDATE stopwatches SET name = ? WHERE id = ?").bind(check_name(name)?).bind(id).execute(pool).await?;
    stopwatch(pool, id).await
}

/// start | pause | reset | lap
pub async fn control_stopwatch(pool: &SqlitePool, id: &str, action: &str) -> AppResult<Stopwatch> {
    let mut s = stopwatch(pool, id).await?;
    let now = now_ms();
    match action {
        "start" => {
            s.started_at.get_or_insert(now);
        }
        "pause" => {
            s.elapsed_ms = s.total(now);
            s.started_at = None;
        }
        "reset" => {
            s.elapsed_ms = 0;
            s.started_at = None;
            s.laps.clear();
        }
        "lap" if s.started_at.is_some() => {
            if s.laps.len() >= 200 {
                return Err(AppError::Invalid("не больше 200 кругов".into()));
            }
            s.laps.push(s.total(now));
        }
        "lap" => {}
        other => return Err(AppError::Invalid(format!("действие «{other}» не поддерживается"))),
    }
    sqlx::query("UPDATE stopwatches SET elapsed_ms = ?, started_at = ?, laps_json = ? WHERE id = ?")
        .bind(s.elapsed_ms)
        .bind(s.started_at)
        .bind(serde_json::to_string(&s.laps)?)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(s)
}

pub async fn delete_stopwatch(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM stopwatches WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct StopwatchResult {
    pub id: i64,
    pub stopwatch_id: Option<String>,
    pub name: String,
    pub total_ms: i64,
    pub laps_json: String,
    pub note: String,
    pub saved_at: String,
}

/// Saves the current time and laps (the stopwatch keeps running).
pub async fn save_result(pool: &SqlitePool, id: &str, note: &str) -> AppResult<()> {
    let s = stopwatch(pool, id).await?;
    let total = s.total(now_ms());
    if total <= 0 {
        return Err(AppError::Invalid("секундомер ещё не запускался".into()));
    }
    if note.chars().count() > 200 {
        return Err(AppError::Invalid("заметка — не больше 200 символов".into()));
    }
    sqlx::query("INSERT INTO stopwatch_results (stopwatch_id, name, total_ms, laps_json, note) VALUES (?, ?, ?, ?, ?)")
        .bind(&s.id)
        .bind(&s.name)
        .bind(total)
        .bind(serde_json::to_string(&s.laps)?)
        .bind(note.trim())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn results(pool: &SqlitePool, limit: i64) -> AppResult<Vec<StopwatchResult>> {
    Ok(sqlx::query_as::<_, StopwatchResult>(
        "SELECT id, stopwatch_id, name, total_ms, laps_json, note, saved_at FROM stopwatch_results ORDER BY id DESC LIMIT ?",
    )
    .bind(limit.clamp(1, 1000))
    .fetch_all(pool)
    .await?)
}

// --- commands ---

#[derive(Debug, Serialize)]
pub struct TimersState {
    pub timers: Vec<Timer>,
    pub stopwatches: Vec<Stopwatch>,
    pub stats: SessionStats,
    /// Backend clock, to compute remaining time without drift.
    pub now_ms: i64,
}

async fn state_of(pool: &SqlitePool) -> AppResult<TimersState> {
    Ok(TimersState { timers: timers(pool).await?, stopwatches: stopwatches(pool).await?, stats: session_stats(pool).await?, now_ms: now_ms() })
}

#[tauri::command]
pub async fn timers_state(state: State<'_, AppState>) -> AppResult<TimersState> {
    require(&state, TIMERS)?;
    state_of(&state.db).await
}

#[tauri::command]
pub async fn timers_save(state: State<'_, AppState>, id: Option<String>, input: TimerInput) -> AppResult<TimersState> {
    require(&state, TIMERS)?;
    save_timer(&state.db, id.as_deref(), input).await?;
    state_of(&state.db).await
}

#[tauri::command]
pub async fn timers_control(state: State<'_, AppState>, id: String, action: String) -> AppResult<TimersState> {
    require(&state, TIMERS)?;
    control(&state.db, &id, &action).await?;
    state_of(&state.db).await
}

#[tauri::command]
pub async fn timers_delete(state: State<'_, AppState>, id: String) -> AppResult<TimersState> {
    require(&state, TIMERS)?;
    delete_timer(&state.db, &id).await?;
    state_of(&state.db).await
}

#[tauri::command]
pub async fn timer_sessions(state: State<'_, AppState>, limit: Option<i64>) -> AppResult<Vec<Session>> {
    require(&state, TIMERS)?;
    sessions(&state.db, limit.unwrap_or(100)).await
}

#[tauri::command]
pub async fn timer_sessions_clear(state: State<'_, AppState>) -> AppResult<()> {
    require(&state, TIMERS)?;
    sqlx::query("DELETE FROM timer_sessions").execute(&state.db).await?;
    Ok(())
}

#[tauri::command]
pub async fn stopwatches_create(state: State<'_, AppState>, name: String) -> AppResult<TimersState> {
    require(&state, TIMERS)?;
    create_stopwatch(&state.db, &name).await?;
    state_of(&state.db).await
}

#[tauri::command]
pub async fn stopwatches_rename(state: State<'_, AppState>, id: String, name: String) -> AppResult<TimersState> {
    require(&state, TIMERS)?;
    rename_stopwatch(&state.db, &id, &name).await?;
    state_of(&state.db).await
}

#[tauri::command]
pub async fn stopwatches_control(state: State<'_, AppState>, id: String, action: String) -> AppResult<TimersState> {
    require(&state, TIMERS)?;
    control_stopwatch(&state.db, &id, &action).await?;
    state_of(&state.db).await
}

#[tauri::command]
pub async fn stopwatches_delete(state: State<'_, AppState>, id: String) -> AppResult<TimersState> {
    require(&state, TIMERS)?;
    delete_stopwatch(&state.db, &id).await?;
    state_of(&state.db).await
}

#[tauri::command]
pub async fn stopwatches_save_result(state: State<'_, AppState>, id: String, note: Option<String>) -> AppResult<Vec<StopwatchResult>> {
    require(&state, TIMERS)?;
    save_result(&state.db, &id, note.as_deref().unwrap_or("")).await?;
    results(&state.db, 100).await
}

#[tauri::command]
pub async fn stopwatch_results(state: State<'_, AppState>, limit: Option<i64>) -> AppResult<Vec<StopwatchResult>> {
    require(&state, TIMERS)?;
    results(&state.db, limit.unwrap_or(100)).await
}

#[tauri::command]
pub async fn stopwatch_results_delete(state: State<'_, AppState>, id: i64) -> AppResult<Vec<StopwatchResult>> {
    require(&state, TIMERS)?;
    sqlx::query("DELETE FROM stopwatch_results WHERE id = ?").bind(id).execute(&state.db).await?;
    results(&state.db, 100).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pomodoro() -> TimerInput {
        TimerInput { name: "Фокус".into(), kind: "pomodoro".into(), duration_ms: 60_000, work_ms: 60_000, short_break_ms: 10_000, long_break_ms: 30_000, rounds: 2 }
    }

    async fn expire(pool: &SqlitePool, id: &str) {
        sqlx::query("UPDATE timers SET ends_at = ? WHERE id = ?").bind(now_ms() - 1).bind(id).execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn pomodoro_cycle_and_history() {
        let pool = crate::db::test_pool().await;
        let t = save_timer(&pool, None, pomodoro()).await.unwrap();
        assert!(save_timer(&pool, None, TimerInput { rounds: 0, ..pomodoro() }).await.is_err());

        let phases = [("work", 1), ("short_break", 1), ("work", 2), ("long_break", 2), ("work", 1)];
        for w in phases.windows(2) {
            let cur = control(&pool, &t.id, "start").await.unwrap();
            assert_eq!((cur.phase.as_str(), cur.round), w[0]);
            expire(&pool, &t.id).await;
            let done = finish_expired(&pool, false).await.unwrap();
            assert_eq!(done.len(), 1);
            let next = timer(&pool, &t.id).await.unwrap();
            assert_eq!((next.phase.as_str(), next.round), w[1]);
            assert!(!next.running);
        }
        let stats = session_stats(&pool).await.unwrap();
        assert_eq!((stats.pomodoros_today, stats.sessions_today), (2, 4));
        assert_eq!(stats.focus_ms_today, 120_000);

        // Pause keeps the remaining time; reset logs an unfinished session.
        control(&pool, &t.id, "start").await.unwrap();
        let paused = control(&pool, &t.id, "pause").await.unwrap();
        assert!(!paused.running && paused.remaining_ms.unwrap() <= 60_000);
        control(&pool, &t.id, "reset").await.unwrap();
        let last = &sessions(&pool, 1).await.unwrap()[0];
        assert!(!last.completed || last.actual_ms > 0);
    }

    #[tokio::test]
    async fn countdown_and_auto_start() {
        let pool = crate::db::test_pool().await;
        let c = save_timer(&pool, None, TimerInput { name: "Чай".into(), kind: "countdown".into(), ..pomodoro() }).await.unwrap();
        control(&pool, &c.id, "start").await.unwrap();
        expire(&pool, &c.id).await;
        assert_eq!(finish_expired(&pool, true).await.unwrap()[0].1, "countdown");
        let c = timer(&pool, &c.id).await.unwrap();
        assert!(!c.running && c.remaining_ms.is_none(), "countdown never auto-restarts");
        assert!(control(&pool, &c.id, "skip").await.is_err());

        let p = save_timer(&pool, None, pomodoro()).await.unwrap();
        control(&pool, &p.id, "start").await.unwrap();
        expire(&pool, &p.id).await;
        finish_expired(&pool, true).await.unwrap();
        let p = timer(&pool, &p.id).await.unwrap();
        assert!(p.running && p.phase == "short_break");
    }

    #[tokio::test]
    async fn independent_stopwatches_and_results() {
        let pool = crate::db::test_pool().await;
        let a = create_stopwatch(&pool, "Задача А").await.unwrap();
        let b = create_stopwatch(&pool, "Задача Б").await.unwrap();
        assert!(save_result(&pool, &a.id, "").await.is_err());
        control_stopwatch(&pool, &a.id, "start").await.unwrap();
        sqlx::query("UPDATE stopwatches SET started_at = ? WHERE id = ?").bind(now_ms() - 2000).bind(&a.id).execute(&pool).await.unwrap();
        control_stopwatch(&pool, &a.id, "lap").await.unwrap();
        let a2 = control_stopwatch(&pool, &a.id, "pause").await.unwrap();
        assert!(a2.elapsed_ms >= 2000 && a2.laps.len() == 1);
        assert_eq!(stopwatch(&pool, &b.id).await.unwrap().elapsed_ms, 0, "independent");
        save_result(&pool, &a.id, "первый подход").await.unwrap();
        let r = &results(&pool, 10).await.unwrap()[0];
        assert_eq!((r.name.as_str(), r.note.as_str()), ("Задача А", "первый подход"));
        assert!(control_stopwatch(&pool, &a.id, "reset").await.unwrap().laps.is_empty());
    }
}
