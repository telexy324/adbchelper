use rusqlite::Connection;
use tauri::State;

use crate::models::tidb::{AnalyzeTidbInput, AnalyzeTidbResponse};
use crate::orchestrator::tidb;
use crate::storage::db;
use crate::AppState;

fn open_connection(storage_path: &str) -> Result<Connection, String> {
    Connection::open(storage_path).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn analyze_tidb(
    state: State<'_, AppState>,
    input: AnalyzeTidbInput,
) -> Result<AnalyzeTidbResponse, String> {
    let connection = open_connection(&state.storage_path)?;
    let response = tidb::analyze_tidb(&connection, &state.app_data_dir, input)?;

    db::insert_audit_log(
        &connection,
        None,
        Some(&response.environment_id),
        "user",
        "tidb_analysis",
        Some("analyze_tidb_slow_queries"),
        Some(&response.instance_name),
        Some(&response.executed_plan),
        "completed",
    )
    .map_err(|error| error.to_string())?;

    Ok(response)
}
