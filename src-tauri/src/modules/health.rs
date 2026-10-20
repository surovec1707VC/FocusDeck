//! Health checks: is an enabled module actually working?

use tauri::{AppHandle, Manager};

use super::catalog::{self, *};
use super::registry;
use super::types::HealthState;
use crate::error::AppResult;
use crate::hotkeys::HotkeyTarget;
use crate::state::AppState;

pub async fn check(app: &AppHandle, id: &str) -> AppResult<(HealthState, String)> {
    let state = app.state::<AppState>();
    let db = &state.db;
    registry::def(id)?;

    if !registry::is_enabled(db, id).await? {
        return Ok((HealthState::Disabled, "модуль отключён".into()));
    }
    let missing: Vec<String> = registry::permissions(db, id).await?.into_iter().filter(|p| !p.granted).map(|p| p.name).collect();
    if !missing.is_empty() {
        return Ok((HealthState::Degraded, format!("нет разрешений: {}", missing.join(", "))));
    }
    if catalog::IN_DEVELOPMENT.contains(&id) {
        return Ok((HealthState::Degraded, "функция в разработке".into()));
    }

    let ok = |m: String| Ok((HealthState::Ok, m));
    match id {
        DATABASE => {
            let (result,): (String,) = sqlx::query_as("PRAGMA quick_check").fetch_one(db).await?;
            if result == "ok" {
                ok("база данных в порядке".into())
            } else {
                Ok((HealthState::Error, format!("проверка базы: {result}")))
            }
        }
        MODULE_REGISTRY => {
            let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM modules").fetch_one(db).await?;
            ok(format!("модулей в реестре: {n}"))
        }
        SETTINGS => match state.settings().validate() {
            Ok(()) => ok("настройки корректны".into()),
            Err(e) => Ok((HealthState::Error, e.to_string())),
        },
        SECURITY => {
            let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM permissions WHERE granted = 1").fetch_one(db).await?;
            ok(format!("выдано разрешений: {n}"))
        }
        HOTKEYS => {
            if !state.settings().global_hotkeys_enabled {
                return Ok((HealthState::Degraded, "выключены в настройках".into()));
            }
            let map = state.hotkeys.lock().unwrap();
            let window = map.values().any(|t| *t == HotkeyTarget::ToggleWindow);
            if window || state.settings().toggle_window_hotkey.trim().is_empty() {
                ok(format!("зарегистрировано сочетаний: {}", map.len()))
            } else {
                Ok((HealthState::Error, "сочетание показа окна не зарегистрировано".into()))
            }
        }
        AUTOMATIONS => {
            if crate::background::running(app, AUTOMATIONS) {
                ok("планировщик работает".into())
            } else {
                Ok((HealthState::Error, "планировщик не запущен".into()))
            }
        }
        CLIPBOARD => {
            if crate::background::running(app, CLIPBOARD) {
                let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM clipboard_history").fetch_one(db).await?;
                ok(format!("запись идёт, записей: {n}"))
            } else {
                Ok((HealthState::Error, "фоновая задача не запущена".into()))
            }
        }
        TIMERS => {
            if crate::background::running(app, TIMERS) {
                let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM timers WHERE running = 1").fetch_one(db).await?;
                ok(format!("фоновая задача работает, запущено таймеров: {n}"))
            } else {
                Ok((HealthState::Error, "фоновая задача не запущена".into()))
            }
        }
        WIDGET_WEATHER => {
            let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM widget_cache WHERE key LIKE 'weather:%'").fetch_one(db).await?;
            ok(if n > 0 { "прогноз в кэше".into() } else { "прогноз ещё не загружался".into() })
        }
        WIDGET_SYSTEM_INFO => {
            let info = tauri::async_runtime::spawn_blocking(crate::widgets::system::snapshot).await.map_err(anyhow::Error::from)?;
            if info.ram_total > 0 {
                ok(format!("данные Windows доступны, ядер: {}", info.cores))
            } else {
                Ok((HealthState::Error, "не удалось прочитать данные Windows".into()))
            }
        }
        TRAY => {
            if app.tray_by_id(crate::tray::TRAY_ID).is_some() {
                ok("значок в трее".into())
            } else {
                Ok((HealthState::Error, "значок не создан".into()))
            }
        }
        AUTOSTART => {
            let wanted = state.settings().autostart;
            let actual = crate::autostart::is_enabled(app);
            match (wanted, actual) {
                (true, true) => ok("включён".into()),
                (false, false) => ok("выключен в настройках".into()),
                _ => Ok((HealthState::Degraded, "состояние Windows не совпадает с настройкой".into())),
            }
        }
        LOGGING => {
            let dir = app.path().app_log_dir().map_err(anyhow::Error::from)?;
            if dir.is_dir() {
                ok(format!("журнал: {}", dir.display()))
            } else {
                Ok((HealthState::Error, "папка журнала не найдена".into()))
            }
        }
        INTEGRATIONS => {
            let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM connectors WHERE enabled = 1").fetch_one(db).await?;
            let offline = crate::integrations::offline_mode(db).await;
            ok(format!("включено интеграций: {n}{}", if offline { ", Offline Mode" } else { "" }))
        }
        _ => ok("работает".into()),
    }
}
