use rusqlite::Connection;
use tauri::State;

use crate::models::investigation::{
    InvestigationEvidence, InvestigationSaveResponse, InvestigationSummary, SaveInvestigationInput,
};
use crate::storage::db;
use crate::AppState;

fn open_connection(storage_path: &str) -> Result<Connection, String> {
    Connection::open(storage_path).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_investigations(state: State<'_, AppState>) -> Result<Vec<InvestigationSummary>, String> {
    let connection = open_connection(&state.storage_path)?;
    db::list_investigations(&connection).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_investigation_evidence(
    state: State<'_, AppState>,
    investigation_id: String,
) -> Result<Vec<InvestigationEvidence>, String> {
    let connection = open_connection(&state.storage_path)?;
    db::list_investigation_evidence(&connection, &investigation_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_investigation_evidence(
    state: State<'_, AppState>,
    input: SaveInvestigationInput,
) -> Result<InvestigationSaveResponse, String> {
    let connection = open_connection(&state.storage_path)?;

    let investigation = match input.investigation_id.as_deref() {
        Some(investigation_id) => db::get_investigation(&connection, investigation_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Selected investigation no longer exists.".to_string())?,
        None => db::create_investigation(
            &connection,
            &input.environment_id,
            input
                .title
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("Investigation"),
        )
        .map_err(|error| error.to_string())?,
    };

    let evidence = db::add_investigation_evidence(
        &connection,
        &investigation.id,
        &input.evidence_type,
        &input.evidence_title,
        &input.summary,
        &input.content_json,
    )
    .map_err(|error| error.to_string())?;

    db::touch_investigation(&connection, &investigation.id).map_err(|error| error.to_string())?;
    let investigation = db::get_investigation(&connection, &investigation.id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Investigation missing after save.".to_string())?;

    db::insert_audit_log(
        &connection,
        None,
        Some(&investigation.environment_id),
        "user",
        "investigation_evidence_save",
        Some(&input.evidence_type),
        Some(&investigation.id),
        Some(&input.evidence_title),
        "completed",
    )
    .map_err(|error| error.to_string())?;

    Ok(InvestigationSaveResponse {
        investigation,
        evidence,
    })
}
