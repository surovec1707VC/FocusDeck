//! FocusDeck — local desktop workspace. No AI, telemetry, analytics or
//! cloud sync: all data stays in a local SQLite database.

mod automation;
mod autostart;
mod background;
mod clipboard_hub;
mod db;
mod error;
mod hotkeys;
mod info;
mod logging;
mod modules;
mod notifications;
mod permissions;
mod plugins;
mod productivity;
mod settings;
mod state;
mod tray;
mod widgets;
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
        clipboard_hub::apply_persistence(&pool).await?;
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
    if state.module_enabled(modules::catalog::CLIPBOARD) {
        if let Err(e) = tauri::async_runtime::block_on(clipboard_hub::apply_persistence(&state.db)) {
            tracing::warn!("clipboard history cleanup: {e}");
        }
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
            workspace::layouts_duplicate,
            workspace::layouts_activate,
            workspace::layouts_delete,
            workspace::layouts_set_hotkey,
            widgets::widget_catalog,
            widgets::widgets_list,
            widgets::widgets_add,
            widgets::widgets_update,
            widgets::widgets_save_grid,
            widgets::widgets_remove,
            widgets::widget_data,
            widgets::widget_action,
            widgets::widget_pick_app,
            productivity::notes::notes_list,
            productivity::notes::notes_facets,
            productivity::notes::notes_create,
            productivity::notes::notes_update,
            productivity::notes::notes_trash,
            productivity::notes::notes_restore,
            productivity::notes::notes_delete_forever,
            productivity::notes::notes_empty_trash,
            productivity::notes::notes_export,
            productivity::tasks::tasks_query,
            productivity::tasks::tasks_create,
            productivity::tasks::tasks_quick_add,
            productivity::tasks::tasks_quick_parse,
            productivity::tasks::tasks_update,
            productivity::tasks::tasks_remove,
            productivity::tasks::tasks_stats,
            productivity::snippets::snippets_list,
            productivity::snippets::snippets_save,
            productivity::snippets::snippets_delete,
            productivity::snippets::snippets_render,
            productivity::snippets::snippets_copy,
            productivity::snippets::snippets_paste,
            productivity::snippets::snippets_export,
            productivity::snippets::snippets_import,
            productivity::timers::timers_state,
            productivity::timers::timers_save,
            productivity::timers::timers_control,
            productivity::timers::timers_delete,
            productivity::timers::timer_sessions,
            productivity::timers::timer_sessions_clear,
            productivity::timers::stopwatches_create,
            productivity::timers::stopwatches_rename,
            productivity::timers::stopwatches_control,
            productivity::timers::stopwatches_delete,
            productivity::timers::stopwatches_save_result,
            productivity::timers::stopwatch_results,
            productivity::timers::stopwatch_results_delete,
            clipboard_hub::clipboard_list,
            clipboard_hub::clipboard_image,
            clipboard_hub::clipboard_details,
            clipboard_hub::clipboard_copy,
            clipboard_hub::clipboard_paste,
            clipboard_hub::clipboard_pin,
            clipboard_hub::clipboard_delete,
            clipboard_hub::clipboard_clear,
            clipboard_hub::clipboard_status,
            clipboard_hub::clipboard_set_private,
            clipboard_hub::quick_actions_list,
            clipboard_hub::quick_actions_save,
            clipboard_hub::quick_actions_delete,
            clipboard_hub::quick_actions_reorder,
            clipboard_hub::quick_actions_run,
            clipboard_hub::quick_actions_pick,
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
