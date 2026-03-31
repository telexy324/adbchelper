use rusqlite::Connection;
use tauri::State;

use crate::models::connection_profile::ConnectionProfile;
use crate::models::nacos::{CompareNacosConfigInput, CompareNacosConfigResponse};
use crate::orchestrator::nacos;
use crate::storage::db;
use crate::AppState;

fn open_connection(storage_path: &str) -> Result<Connection, String> {
    Connection::open(storage_path).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn compare_nacos_config(
    state: State<'_, AppState>,
    input: CompareNacosConfigInput,
) -> Result<CompareNacosConfigResponse, String> {
    let connection = open_connection(&state.storage_path)?;
    let profiles = db::list_connection_profiles(&connection).map_err(|error| error.to_string())?;
    let source_profile = resolve_nacos_profile(&profiles, &input.source_environment_id)?;
    let target_profile = resolve_nacos_profile(&profiles, &input.target_environment_id)?;
    drop(connection);

    let response = nacos::compare_config(source_profile, target_profile, input).await?;
    let connection = open_connection(&state.storage_path)?;

    db::insert_audit_log(
        &connection,
        None,
        Some(&response.target_environment_id),
        "user",
        "nacos_compare",
        Some("compare_nacos_config"),
        Some(&response.data_id),
        Some(&format!(
            "source={}, target={}, group={}, namespace={}",
            response.source_environment_id,
            response.target_environment_id,
            response.group,
            response.namespace_id.clone().unwrap_or_else(|| "default".to_string())
        )),
        "completed",
    )
    .map_err(|error| error.to_string())?;

    Ok(response)
}

fn resolve_nacos_profile(
    profiles: &[ConnectionProfile],
    environment_id: &str,
) -> Result<ConnectionProfile, String> {
    profiles
        .iter()
        .find(|profile| profile.environment_id == environment_id && profile.profile_type == "nacos")
        .cloned()
        .ok_or_else(|| format!("No Nacos profile found for environment {environment_id}. Add one in Settings first."))
}
