mod db;
mod modules;
mod plugins;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    plugins::register(tauri::Builder::default())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let pool = tauri::async_runtime::block_on(db::connect(&data_dir))?;

            let registry = modules::ModuleRegistry::new(pool.clone(), modules::builtin_modules());
            tauri::async_runtime::block_on(registry.sync())?;

            app.manage(pool);
            app.manage(registry);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            modules::list_modules,
            modules::get_module,
            modules::enable_module,
            modules::disable_module,
            modules::update_module_settings,
            modules::reset_module_settings,
            modules::get_module_logs,
            modules::get_module_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(registry) = app.try_state::<modules::ModuleRegistry>() {
                    registry.shutdown_all();
                }
            }
        });
}
