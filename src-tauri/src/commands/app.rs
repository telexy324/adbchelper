use tauri::State;

use crate::models::app_health::AppHealth;
use crate::models::environment::EnvironmentProfile;
use crate::storage::db;
use crate::AppState;

#[tauri::command]
pub fn get_app_health(state: State<'_, AppState>) -> AppHealth {
    AppHealth {
        app_name: "ADBCHelper".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        database_ready: state.database_ready,
        storage_path: state.storage_path.clone(),
    }
}

#[tauri::command]
pub fn list_environments(state: State<'_, AppState>) -> Result<Vec<EnvironmentProfile>, String> {
    let connection =
        rusqlite::Connection::open(&state.storage_path).map_err(|error| error.to_string())?;
    db::list_environments(&connection).map_err(|error| error.to_string())
}
