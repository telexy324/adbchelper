use rusqlite::Connection;
use tauri::State;

use crate::models::kubernetes::{ListKubernetesEventsInput, ListKubernetesEventsResponse};
use crate::orchestrator::kubernetes;
use crate::storage::db;
use crate::AppState;

fn open_connection(storage_path: &str) -> Result<Connection, String> {
    Connection::open(storage_path).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_kubernetes_events(
    state: State<'_, AppState>,
    input: ListKubernetesEventsInput,
) -> Result<ListKubernetesEventsResponse, String> {
    let connection = open_connection(&state.storage_path)?;
    let response = kubernetes::list_events(&connection, input)?;

    db::insert_audit_log(
        &connection,
        None,
        Some(&response.environment_id),
        "user",
        "kubernetes_events",
        Some("list_k8s_events"),
        Some(&response.namespace),
        Some(&response.query_summary),
        "completed",
    )
    .map_err(|error| error.to_string())?;

    Ok(response)
}
