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
pub fn search_logs(
    state: State<'_, AppState>,
    input: LogSearchInput,
) -> Result<LogSearchResponse, String> {
    let connection = open_connection(&state.storage_path)?;
    let response = logs::search_logs(&connection, input)?;

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
