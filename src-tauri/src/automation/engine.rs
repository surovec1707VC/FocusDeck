//! Runs automations: once at start, on a timer (a background task of the
//! Automations module), or from a global hotkey.

use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;
use tauri::{AppHandle, Manager};

use super::store;
use super::types::{Action, Automation, Trigger};
use crate::error::AppResult;
use crate::modules::catalog::{AUTOMATIONS, NOTIFICATIONS};
use crate::modules::registry;
use crate::permissions::{self, PERM_NOTIFICATIONS};
use crate::state::AppState;

/// Parses RFC 3339 or SQLite's `datetime('now')` format (UTC).
pub fn parse_ts(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|d| d.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|n| n.and_utc())
        })
}

/// An interval automation is due `minutes` after its last run (or creation).
pub fn is_due(a: &Automation, now: DateTime<Utc>) -> bool {
    let Trigger::Interval { minutes } = a.trigger else {
        return false;
    };
    let since = a.last_run_at.as_deref().or(Some(a.created_at.as_str())).and_then(parse_ts);
    match since {
        Some(t) => now - t >= chrono::Duration::minutes(minutes as i64),
        None => true,
    }
}

/// Why an automation must not run now, if anything (checked before every run).
pub async fn blocked_reason(db: &sqlx::SqlitePool, a: &Automation) -> AppResult<Option<String>> {
    for module in [AUTOMATIONS, a.module_id.as_str()] {
        if !registry::is_enabled(db, module).await? {
            let name = registry::def(module).map(|m| m.name).unwrap_or(module);
            return Ok(Some(format!("модуль «{name}» отключён")));
        }
    }
    Ok(None)
}

/// Runs app-start automations once (at startup or when the module is enabled).
pub async fn run_startup_automations(app: &AppHandle) {
    let state = app.state::<AppState>();
    match store::list(&state.db).await {
        Ok(list) => {
            for a in list.iter().filter(|a| a.enabled && a.trigger == Trigger::AppStart) {
                run(app, a).await;
            }
        }
        Err(e) => tracing::error!("cannot load automations: {e}"),
    }
}

/// The interval scheduler; started and aborted by `background`.
pub async fn scheduler_loop(app: AppHandle) {
    let state = app.state::<AppState>();
    let period = registry::settings(&state.db, AUTOMATIONS)
        .await
        .ok()
        .and_then(|s| s.get("scheduler_interval_secs").and_then(Value::as_u64))
        .unwrap_or(30);
    tracing::info!(period, "automation scheduler started");
    let mut ticker = tokio::time::interval(Duration::from_secs(period));
    loop {
        ticker.tick().await;
        let Ok(list) = store::list(&state.db).await else {
            continue;
        };
        let now = Utc::now();
        for a in list.iter().filter(|a| a.enabled && is_due(a, now)) {
            run(&app, a).await;
        }
    }
}

pub async fn run_by_id(app: &AppHandle, id: &str) -> AppResult<()> {
    let a = store::get(&app.state::<AppState>().db, id).await?;
    run(app, &a).await;
    Ok(())
}

/// Executes all actions and records the outcome in `automation_logs`.
pub async fn run(app: &AppHandle, a: &Automation) {
    let state = app.state::<AppState>();
    let db = &state.db;
    let now = Utc::now().to_rfc3339();

    let outcome = async {
        if let Some(reason) = blocked_reason(db, a).await? {
            return Ok(Some(reason));
        }
        for action in &a.actions {
            execute(app, a, action).await?;
        }
        AppResult::Ok(None)
    }
    .await;

    let skipped = matches!(outcome, Ok(Some(_)));
    if !skipped {
        let _ = store::mark_run(db, &a.id, &now).await;
    }
    let (status, message) = match outcome {
        Ok(None) => ("ok", format!("выполнено действий: {}", a.actions.len())),
        Ok(Some(reason)) => ("skipped", reason),
        Err(e) => ("error", e.to_string()),
    };
    tracing::info!(automation = %a.id, status, "{message}");
    let keep = registry::settings(db, AUTOMATIONS)
        .await
        .ok()
        .and_then(|s| s.get("max_log_entries").and_then(Value::as_i64))
        .unwrap_or(500);
    if let Err(e) = store::log(db, &a.id, status, &message, keep).await {
        tracing::error!("cannot write automation log: {e}");
    }
}

async fn execute(app: &AppHandle, a: &Automation, action: &Action) -> AppResult<()> {
    let db = &app.state::<AppState>().db;
    match action {
        Action::Notify { title, body } => {
            if !registry::is_enabled(db, NOTIFICATIONS).await? {
                tracing::info!(automation = %a.id, "notification skipped: module off");
                return Ok(());
            }
            permissions::require(db, PERM_NOTIFICATIONS, &format!("automation:{}", a.id)).await?;
            crate::notifications::push(app, "info", title, body, &a.name).await?;
        }
        Action::Log { message } => tracing::info!(automation = %a.name, "{message}"),
        Action::ShowWindow => crate::window::show(app),
        Action::HideWindow => crate::window::hide(app),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::types::AutomationInput;

    fn automation(trigger: Trigger, created: &str, last: Option<&str>) -> Automation {
        Automation {
            id: "a".into(),
            name: "a".into(),
            enabled: true,
            module_id: AUTOMATIONS.into(),
            trigger,
            actions: vec![],
            last_run_at: last.map(Into::into),
            created_at: created.into(),
            updated_at: created.into(),
        }
    }

    #[test]
    fn parses_both_timestamp_formats() {
        assert!(parse_ts("2026-10-20 08:00:00").is_some());
        assert!(parse_ts("2026-10-20T08:00:00+00:00").is_some());
        assert!(parse_ts("garbage").is_none());
    }

    #[test]
    fn interval_due_logic() {
        let now = parse_ts("2026-10-20 10:00:00").unwrap();
        let every_30 = Trigger::Interval { minutes: 30 };
        assert!(!is_due(&automation(every_30.clone(), "2026-10-20 09:45:00", None), now));
        assert!(is_due(&automation(every_30.clone(), "2026-10-20 09:00:00", None), now));
        assert!(!is_due(&automation(every_30.clone(), "2026-10-20 08:00:00", Some("2026-10-20T09:40:00+00:00")), now));
        assert!(is_due(&automation(every_30, "2026-10-20 08:00:00", Some("2026-10-20T09:30:00+00:00")), now));
        assert!(!is_due(&automation(Trigger::AppStart, "2026-10-20 00:00:00", None), now));
    }

    #[tokio::test]
    async fn disabled_modules_block_runs() {
        let pool = registry::test_pool().await;
        let input = AutomationInput {
            name: "Задачи".into(),
            enabled: true,
            module_id: Some("tasks".into()),
            trigger: Trigger::AppStart,
            actions: vec![Action::Log { message: "x".into() }],
        };
        let a = store::create(&pool, &input).await.unwrap();
        assert_eq!(blocked_reason(&pool, &a).await.unwrap(), None);

        registry::set_enabled(&pool, "tasks", false, false, "test").await.unwrap();
        assert_eq!(blocked_reason(&pool, &a).await.unwrap().unwrap(), "модуль «Задачи» отключён");

        registry::set_enabled(&pool, "tasks", true, false, "test").await.unwrap();
        registry::set_enabled(&pool, AUTOMATIONS, false, false, "test").await.unwrap();
        assert!(blocked_reason(&pool, &a).await.unwrap().unwrap().contains("Автоматизации"));
    }
}
