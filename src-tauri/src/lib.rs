mod commands;
mod hardening;
mod llm;
mod models;
mod orchestrator;
mod storage;

use std::path::PathBuf;

use storage::app_log::{append_log, log_path};
use storage::db::{initialize_database, DatabaseStatus};
use tauri::Manager;

pub struct AppState {
    pub database_ready: bool,
    pub storage_path: String,
    pub app_data_dir: String,
    pub log_path: String,
    pub resource_dir: String,
    pub executable_dir: String,
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
            let app_data_dir = resolve_app_data_dir(&app.handle());
            let database_status = bootstrap_storage(&app.handle());
            let log_file_path = log_path(&app_data_dir);
            let resource_dir = app
                .path()
                .resource_dir()
                .unwrap_or_else(|_| app_data_dir.clone());
            let executable_dir = std::env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(|parent| parent.to_path_buf()))
                .unwrap_or_else(|| app_data_dir.clone());
            let _ = append_log(
                &app_data_dir,
                "INFO",
                "startup",
                &format!(
                    "ADBCHelper booting. database_ready={}, storage_path={}",
                    database_status.database_ready,
                    database_status.storage_path.display()
                ),
            );
            app.manage(AppState {
                database_ready: database_status.database_ready,
                storage_path: database_status.storage_path.display().to_string(),
                app_data_dir: app_data_dir.display().to_string(),
                log_path: log_file_path.display().to_string(),
                resource_dir: resource_dir.display().to_string(),
                executable_dir: executable_dir.display().to_string(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::approvals::list_approval_requests,
            commands::approvals::create_approval_request,
            commands::approvals::approve_request,
            commands::approvals::execute_approval_request,
            commands::app::get_app_health,
            commands::app::list_environments,
            commands::app::toggle_devtools,
            commands::chat::list_chat_sessions,
            commands::chat::list_chat_messages,
            commands::chat::list_tool_catalog,
            commands::chat::attach_tool_evidence,
            commands::investigations::list_investigations,
            commands::investigations::list_investigation_evidence,
            commands::investigations::get_investigation_detail,
            commands::investigations::generate_investigation_report,
            commands::investigations::save_investigation_evidence,
            commands::chat::send_chat_message,
            commands::kubernetes::list_kubernetes_events,
            commands::logs::search_logs,
            commands::nacos::compare_nacos_config,
            commands::redis::analyze_redis,
            commands::ssh::run_ssh_diagnostics,
            commands::settings::save_environment,
            commands::settings::list_connection_profiles,
            commands::settings::validate_connection_profile,
            commands::settings::save_connection_profile,
            commands::settings::clear_connection_profile_secret,
            commands::settings::delete_connection_profile,
            commands::settings::trust_ssh_host_key
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
