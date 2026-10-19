//! Runs automations: once at start, on a timer, or from a global hotkey.

use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::store;
use super::types::{Action, Automation, Trigger};
use crate::error::AppResult;
use crate::modules::registry::{self, MODULE_AUTOMATIONS};
use crate::permissions::{self, PERM_NOTIFICATIONS};
use crate::state::AppState;

pub const NOTIFY_EVENT: &str = "focusdeck://notify";
const TICK: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub source: String,
}

/// Parses RFC 3339 or SQLite's `datetime('now')` format (UTC).
pub fn parse_ts(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|d| d.with_timezone(&Utc))
        .ok()
        .or_else(|| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").ok().map(|n| n.and_utc()))
}

/// An interval automation is due `minutes` after its last run (or creation).
pub fn is_due(a: &Automation, now: DateTime<Utc>) -> bool {
    let Trigger::Interval { minutes } = a.trigger else { return false };
    let since = a.last_run_at.as_deref().or(Some(a.created_at.as_str())).and_then(parse_ts);
    match since {
        Some(t) => now - t >= chrono::Duration::minutes(minutes as i64),
        None => true,
    }
}

/// Starts app-start automations and the interval scheduler.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        match store::list(&state.db).await {
            Ok(list) => {
                for a in list.iter().filter(|a| a.enabled && a.trigger == Trigger::AppStart) {
                    run(&app, a).await;
                }
            }
            Err(e) => tracing::error!("cannot load automations: {e}"),
        }

        let mut ticker = tokio::time::interval(TICK);
        loop {
            ticker.tick().await;
            let Ok(list) = store::list(&state.db).await else { continue };
            let now = Utc::now();
            for a in list.iter().filter(|a| a.enabled && is_due(a, now)) {
                run(&app, a).await;
            }
        }
    });
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
        if !registry::is_enabled(db, MODULE_AUTOMATIONS).await? {
            return Ok(Some("модуль «Автоматизации» выключен".to_string()));
        }
        for action in &a.actions {
            execute(app, a, action).await?;
        }
        AppResult::Ok(None)
    }
    .await;

    let _ = store::mark_run(db, &a.id, &now).await;
    let (status, message) = match outcome {
        Ok(None) => ("ok", format!("выполнено действий: {}", a.actions.len())),
        Ok(Some(reason)) => ("skipped", reason),
        Err(e) => ("error", e.to_string()),
    };
    tracing::info!(automation = %a.id, status, "{message}");
    if let Err(e) = store::log(db, &a.id, status, &message).await {
        tracing::error!("cannot write automation log: {e}");
    }
}

async fn execute(app: &AppHandle, a: &Automation, action: &Action) -> AppResult<()> {
    match action {
        Action::Notify { title, body } => {
            permissions::require(&app.state::<AppState>().db, PERM_NOTIFICATIONS, &format!("automation:{}", a.id)).await?;
            app.emit(
                NOTIFY_EVENT,
                Notification { title: title.clone(), body: body.clone(), source: a.name.clone() },
            )
            .map_err(anyhow::Error::from)?;
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

    fn automation(trigger: Trigger, created: &str, last: Option<&str>) -> Automation {
        Automation {
            id: "a".into(),
            name: "a".into(),
            enabled: true,
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
}
