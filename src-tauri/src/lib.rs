//! FocusDeck — local desktop workspace. No AI, telemetry, analytics or
//! cloud sync: all data stays in a local SQLite database.

mod automation;
mod autostart;
mod background;
mod db;
mod error;
mod hotkeys;
mod info;
mod logging;
mod modules;
mod permissions;
mod plugins;
mod settings;
mod state;
mod tray;
mod window;
mod workspace;

use serde_json::Value;
use tauri::{App, AppHandle, Manager, RunEvent, WindowEvent};

use modules::catalog::{AUTOMATIONS, AUTOSTART, DATABASE, LOGGING, TRAY};
use modules::registry;
use state::AppState;

fn setup(app: &mut App) -> anyhow::Result<()> {
    let handle = app.handle().clone();

    // Database first: the logging module's settings decide how logs are written.
    let pool = tauri::async_runtime::block_on(async {
        let pool = db::connect(&app.path().app_data_dir()?).await?;
        permissions::seed(&pool).await?;
        registry::seed(&pool).await?;
        workspace::seed(&pool).await?;
        anyhow::Ok(pool)
    })?;
    let (log_level, log_to_file) = tauri::async_runtime::block_on(async {
        let level = registry::settings(&pool, LOGGING)
            .await
            .ok()
            .and_then(|s| s.get("level").and_then(Value::as_str).map(String::from))
            .unwrap_or_else(|| "info".into());
        let to_file = registry::is_enabled(&pool, LOGGING).await.unwrap_or(true);
        (level, to_file)
    });
    app.manage(logging::init(&app.path().app_log_dir()?, &log_level, log_to_file)?);
    tracing::info!(version = %app.package_info().version, "FocusDeck starting");

    let settings = tauri::async_runtime::block_on(settings::load(&pool))?;
    let enabled = tauri::async_runtime::block_on(registry::enabled_set(&pool))?;
    let start_minimized = settings.start_minimized || std::env::args().any(|a| a == autostart::MINIMIZED_FLAG);

    // The OS autostart entry follows the setting only while the module is on.
    autostart::reconcile(&handle, settings.autostart && enabled.contains(AUTOSTART));
    let tray_on = enabled.contains(TRAY);
    let automations_on = enabled.contains(AUTOMATIONS);
    app.manage(AppState::new(pool, settings, enabled));

    tray::create(&handle)?;
    tray::set_visible(&handle, tray_on);
    if let Err(e) = tauri::async_runtime::block_on(hotkeys::sync(&handle)) {
        // A taken shortcut must not prevent startup; the user can change it in Settings.
        tracing::warn!("{e}");
    }
    background::reconcile_all(&handle);
    if automations_on {
        let h = handle.clone();
        tauri::async_runtime::spawn(async move { automation::engine::run_startup_automations(&h).await });
    }

    // Without the tray there is no way back to a hidden window.
    if start_minimized && tray_on {
        tracing::info!("started minimized to tray");
    } else {
        window::show(&handle);
    }
    Ok(())
}

/// Optional WAL checkpoint on exit (Database module setting).
fn on_exit(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.module_enabled(DATABASE) {
        tauri::async_runtime::block_on(async {
            let checkpoint = registry::settings(&state.db, DATABASE)
                .await
                .ok()
                .and_then(|s| s.get("checkpoint_on_exit").and_then(Value::as_bool))
                .unwrap_or(true);
            if checkpoint {
                if let Err(e) = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").execute(&state.db).await {
                    tracing::warn!("wal checkpoint: {e}");
                }
            }
        });
    }
    tracing::info!("FocusDeck exiting");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    plugins::register(tauri::Builder::default())
        .setup(|app| setup(app).map_err(|e| e.into()))
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let state = app.state::<AppState>();
                if state.settings().close_to_tray && state.module_enabled(TRAY) {
                    api.prevent_close();
                    window::hide(app);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            info::app_info,
            info::dashboard_stats,
            settings::settings_get,
            settings::settings_update,
            permissions::permissions_list,
            permissions::permissions_set,
            permissions::permissions_logs,
            modules::list_modules,
            modules::get_module,
            modules::enable_module,
            modules::disable_module,
            modules::update_module_settings,
            modules::reset_module_settings,
            modules::get_module_status,
            modules::get_module_logs,
            modules::get_module_permissions,
            modules::export_config,
            modules::import_config,
            modules::export_config_to_file,
            modules::read_config_file,
            workspace::layouts_list,
            workspace::layouts_create,
            workspace::layouts_rename,
            workspace::layouts_activate,
            workspace::layouts_delete,
            workspace::widgets_list,
            workspace::widgets_add,
            workspace::widgets_update,
            workspace::widgets_move,
            workspace::widgets_remove,
            workspace::tasks_list,
            workspace::tasks_add,
            workspace::tasks_toggle,
            workspace::tasks_delete,
            automation::automations_list,
            automation::automations_create,
            automation::automations_update,
            automation::automations_set_enabled,
            automation::automations_delete,
            automation::automations_run_now,
            automation::automations_logs,
        ])
        .build(tauri::generate_context!())
        .expect("error while building FocusDeck")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                on_exit(app);
            }
        });
}
