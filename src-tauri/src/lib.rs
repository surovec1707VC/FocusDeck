//! FocusDeck — local desktop workspace. No AI, telemetry, analytics or
//! cloud sync: all data stays in a local SQLite database.

mod automation;
mod autostart;
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

use tauri::{App, Manager, RunEvent, WindowEvent};

use state::AppState;

fn setup(app: &mut App) -> anyhow::Result<()> {
    let handle = app.handle().clone();
    let guard = logging::init(&app.path().app_log_dir()?)?;
    app.manage(guard);
    tracing::info!(version = %app.package_info().version, "FocusDeck starting");

    let pool = tauri::async_runtime::block_on(async {
        let pool = db::connect(&app.path().app_data_dir()?).await?;
        permissions::seed(&pool).await?;
        modules::registry::seed(&pool).await?;
        workspace::seed(&pool).await?;
        anyhow::Ok(pool)
    })?;
    let settings = tauri::async_runtime::block_on(settings::load(&pool))?;
    let start_minimized =
        settings.start_minimized || std::env::args().any(|a| a == autostart::MINIMIZED_FLAG);

    autostart::reconcile(&handle, settings.autostart);
    app.manage(AppState::new(pool, settings));

    tray::create(&handle)?;
    if let Err(e) = tauri::async_runtime::block_on(hotkeys::sync(&handle)) {
        // A taken shortcut must not prevent startup; the user can change it in Settings.
        tracing::warn!("{e}");
    }
    automation::engine::start(handle.clone());

    if start_minimized {
        tracing::info!("started minimized to tray");
    } else {
        window::show(&handle);
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    plugins::register(tauri::Builder::default())
        .setup(|app| setup(app).map_err(|e| e.into()))
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                if app.state::<AppState>().settings().close_to_tray {
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
            modules::modules_list,
            modules::modules_set_enabled,
            modules::module_settings_get,
            modules::module_settings_set,
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
        .run(|_app, event| {
            if let RunEvent::Exit = event {
                tracing::info!("FocusDeck exiting");
            }
        });
}
