use rusqlite::Connection;
use tauri::State;
use uuid::Uuid;

use crate::models::connection_profile::{
    ConnectionProfile, UpsertConnectionProfileInput, UpsertEnvironmentInput, ValidationResult,
};
use crate::models::environment::EnvironmentProfile;
use crate::storage::db;
use crate::storage::secrets;
use crate::AppState;

fn open_connection(storage_path: &str) -> Result<Connection, String> {
    Connection::open(storage_path).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_environment(
    state: State<'_, AppState>,
    input: UpsertEnvironmentInput,
) -> Result<EnvironmentProfile, String> {
    let connection = open_connection(&state.storage_path)?;
    db::upsert_environment(&connection, input).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_connection_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<ConnectionProfile>, String> {
    let connection = open_connection(&state.storage_path)?;
    db::list_connection_profiles(&connection).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn validate_connection_profile(
    input: UpsertConnectionProfileInput,
) -> Result<ValidationResult, String> {
    Ok(db::validate_connection_profile(&input))
}

#[tauri::command]
pub fn save_connection_profile(
    state: State<'_, AppState>,
    input: UpsertConnectionProfileInput,
) -> Result<ConnectionProfile, String> {
    let validation = db::validate_connection_profile(&input);
    if !validation.ok {
        return Err(validation.messages.join(" "));
    }

    let connection = open_connection(&state.storage_path)?;
    let profile_id = input.id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
    let existing_secret = db::get_connection_profile(&connection, &profile_id)
        .map_err(|error| error.to_string())?
        .map(|profile| profile.has_secret)
        .unwrap_or(false);
    let has_secret = input
        .secret_value
        .as_ref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
        || existing_secret;

    db::upsert_connection_profile(&connection, &input, &profile_id, has_secret)
        .map_err(|error| error.to_string())?;

    if let Some(secret_value) = &input.secret_value {
        if !secret_value.trim().is_empty() {
            secrets::set_profile_secret(&profile_id, secret_value).map_err(|error| error.to_string())?;
            db::update_connection_profile_secret_state(&connection, &profile_id, true)
                .map_err(|error| error.to_string())?;
        }
    }

    let profiles = db::list_connection_profiles(&connection).map_err(|error| error.to_string())?;
    profiles
        .into_iter()
        .find(|item| item.id == profile_id)
        .ok_or_else(|| "Saved profile could not be loaded.".to_string())
}

#[tauri::command]
pub fn clear_connection_profile_secret(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<(), String> {
    let connection = open_connection(&state.storage_path)?;
    secrets::delete_profile_secret(&profile_id).map_err(|error| error.to_string())?;
    db::update_connection_profile_secret_state(&connection, &profile_id, false)
        .map_err(|error| error.to_string())
}
