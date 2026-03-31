use rusqlite::Connection;
use tauri::State;

use crate::models::ssh::{SshDiagnosticsInput, SshDiagnosticsResponse};
use crate::orchestrator::ssh;
use crate::storage::db;
use crate::AppState;

fn open_connection(storage_path: &str) -> Result<Connection, String> {
    Connection::open(storage_path).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn run_ssh_diagnostics(
    state: State<'_, AppState>,
    input: SshDiagnosticsInput,
) -> Result<SshDiagnosticsResponse, String> {
    let connection = open_connection(&state.storage_path)?;
    let response = ssh::run_diagnostics(&connection, input)?;

    db::insert_audit_log(
        &connection,
        None,
        Some(&response.environment_id),
        "user",
        "ssh_diagnostics",
        Some("inspect_ssh_host"),
        Some(&response.target_host),
        Some(&response.executed_command),
        "completed",
    )
    .map_err(|error| error.to_string())?;

    Ok(response)
}
