mod ai;
mod db;
mod modules;
mod plugins;
mod secrets;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    plugins::register(tauri::Builder::default())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let pool = tauri::async_runtime::block_on(db::connect(&data_dir))?;

            let registry = modules::ModuleRegistry::new(pool.clone(), modules::builtin_modules());
            tauri::async_runtime::block_on(registry.sync())?;

            let router = tauri::async_runtime::block_on(ai::AiRouter::load(&pool, secrets::SecretStore))?;

            app.manage(pool);
            app.manage(registry);
            app.manage(router);
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
            ai::ai_list_profiles,
            ai::ai_get_config,
            ai::ai_set_config,
            ai::ai_list_models,
            ai::ai_test_connection,
            ai::ai_set_api_key,
            ai::ai_has_api_key,
            ai::ai_delete_api_key,
            ai::ai_list_sessions,
            ai::ai_get_messages,
            ai::ai_delete_session,
            ai::ai_chat_stream,
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
