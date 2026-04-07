use rusqlite::Connection;
use tauri::State;

use crate::models::redis::{AnalyzeRedisInput, AnalyzeRedisResponse};
use crate::orchestrator::redis;
use crate::storage::db;
use crate::AppState;

fn open_connection(storage_path: &str) -> Result<Connection, String> {
    Connection::open(storage_path).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn analyze_redis(
    state: State<'_, AppState>,
    input: AnalyzeRedisInput,
) -> Result<AnalyzeRedisResponse, String> {
    let connection = open_connection(&state.storage_path)?;
    let response = redis::analyze_redis(&connection, input)?;

    db::insert_audit_log(
        &connection,
        None,
        Some(&response.environment_id),
        "user",
        "redis_analysis",
        Some("analyze_redis_instance"),
        Some(&response.instance_name),
        Some(&response.executed_plan),
        "completed",
    )
    .map_err(|error| error.to_string())?;

    Ok(response)
}
