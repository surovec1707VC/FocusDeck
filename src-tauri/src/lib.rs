mod ai;
mod db;
mod modules;
mod plugins;
mod secrets;

use std::path::PathBuf;

use tauri::Manager;

/// Bundled resource folder (e.g. `skills/`, copied by `bundle.resources`);
/// falls back to the repository folder when no copy exists.
fn resource_dir(app: &tauri::App, name: &str) -> PathBuf {
    app.path()
        .resource_dir()
        .map(|d| d.join(name))
        .ok()
        .filter(|d| d.is_dir())
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(name))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    plugins::register(tauri::Builder::default())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let pool = tauri::async_runtime::block_on(db::connect(&data_dir))?;

            let registry = modules::ModuleRegistry::new(pool.clone(), modules::builtin_modules());
            tauri::async_runtime::block_on(registry.sync())?;

            let router = tauri::async_runtime::block_on(ai::AiRouter::load(&pool, secrets::SecretStore))?;

            let skills = ai::skills::SkillRegistry::new(resource_dir(app, "skills"), pool.clone());
            tauri::async_runtime::block_on(skills.reload())?;

            app.manage(pool);
            app.manage(registry);
            app.manage(router);
            app.manage(skills);
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
            ai::skills::skills_list,
            ai::skills::skills_issues,
            ai::skills::skills_reload,
            ai::skills::skills_set_enabled,
            ai::skills::skills_test_run,
            ai::skills::skills_logs,
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
