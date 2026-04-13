use rusqlite::Connection;
use tauri::State;

use crate::models::logs::{LogSearchInput, LogSearchResponse};
use crate::orchestrator::logs;
use crate::storage::db;
use crate::AppState;

fn open_connection(storage_path: &str) -> Result<Connection, String> {
    Connection::open(storage_path).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn search_logs(
    state: State<'_, AppState>,
    input: LogSearchInput,
) -> Result<LogSearchResponse, String> {
    let connection = open_connection(&state.storage_path)?;
    let elk_profile = db::list_connection_profiles(&connection)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|profile| profile.environment_id == input.environment_id && profile.profile_type == "elk")
        .filter(|profile| !profile.endpoint.trim().is_empty());
    drop(connection);

    let response = logs::search_logs(elk_profile, &state.app_data_dir, input).await?;

    let connection = open_connection(&state.storage_path)?;

    db::insert_audit_log(
        &connection,
        None,
        Some(&response.environment_id),
        "user",
        "log_search",
        Some("search_elk_logs"),
        None,
        Some(&response.executed_query),
        "completed",
    )
    .map_err(|error| error.to_string())?;

    Ok(response)
}
