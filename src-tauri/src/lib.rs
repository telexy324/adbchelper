mod commands;
mod llm;
mod models;
mod orchestrator;
mod storage;

use std::path::PathBuf;

use storage::db::{initialize_database, DatabaseStatus};
use tauri::Manager;

pub struct AppState {
    pub database_ready: bool,
    pub storage_path: String,
}

fn resolve_app_data_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("adbchelper"))
}

fn bootstrap_storage(app: &tauri::AppHandle) -> DatabaseStatus {
    let base_dir = resolve_app_data_dir(app);

    match initialize_database(&base_dir) {
        Ok(status) => status,
        Err(_) => DatabaseStatus {
            storage_path: base_dir.join("adbchelper.db"),
            database_ready: false,
        },
    }
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let database_status = bootstrap_storage(&app.handle());
            app.manage(AppState {
                database_ready: database_status.database_ready,
                storage_path: database_status.storage_path.display().to_string(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_app_health,
            commands::app::list_environments,
            commands::chat::list_chat_sessions,
            commands::chat::list_chat_messages,
            commands::chat::list_tool_catalog,
            commands::chat::send_chat_message,
            commands::logs::search_logs,
            commands::nacos::compare_nacos_config,
            commands::ssh::run_ssh_diagnostics,
            commands::settings::save_environment,
            commands::settings::list_connection_profiles,
            commands::settings::validate_connection_profile,
            commands::settings::save_connection_profile,
            commands::settings::clear_connection_profile_secret
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
