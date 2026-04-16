use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use rusqlite::Connection;
use tauri::State;
use uuid::Uuid;
use serde_json::Value;

use crate::hardening::run_command_with_timeout;
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
            match secrets::set_profile_secret(Some(Path::new(&state.app_data_dir)), &profile_id, secret_value) {
                Ok(()) => {}
                Err(error) => {
                    if error.to_string().contains("stored secret in app data fallback instead") {
                        // Keep the save successful when fallback storage works.
                    } else {
                        return Err(error.to_string());
                    }
                }
            }
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
    secrets::delete_profile_secret(Some(Path::new(&state.app_data_dir)), &profile_id)
        .map_err(|error| error.to_string())?;
    db::update_connection_profile_secret_state(&connection, &profile_id, false)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_connection_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<(), String> {
    let connection = open_connection(&state.storage_path)?;
    secrets::delete_profile_secret(Some(Path::new(&state.app_data_dir)), &profile_id)
        .map_err(|error| error.to_string())?;
    db::delete_connection_profile(&connection, &profile_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn trust_ssh_host_key(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<String, String> {
    let connection = open_connection(&state.storage_path)?;
    let profile = db::get_connection_profile(&connection, &profile_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "SSH profile not found.".to_string())?;

    if profile.profile_type != "ssh" {
        return Err("Trust host key is only available for SSH profiles.".to_string());
    }

    let ssh_config = TrustedSshProfile::from_profile(&profile, Path::new(&state.app_data_dir))?;
    if trust_with_keyscan(&ssh_config).is_err() {
        trust_with_accept_new(&ssh_config)?;
    }

    Ok(format!(
        "Trusted SSH host key for {} using {}.",
        ssh_config.render_target(),
        ssh_config.known_hosts_path.display()
    ))
}

#[derive(Debug)]
struct TrustedSshProfile {
    host: String,
    port: Option<u16>,
    username: Option<String>,
    known_hosts_path: PathBuf,
}

impl TrustedSshProfile {
    fn from_profile(profile: &ConnectionProfile, app_data_dir: &Path) -> Result<Self, String> {
        let config_json = if profile.config_json.trim().is_empty() {
            "{}"
        } else {
            profile.config_json.as_str()
        };
        let config = serde_json::from_str::<Value>(config_json)
            .map_err(|error| format!("Invalid SSH profile JSON: {error}"))?;
        let endpoint = config
            .get("host")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| profile.endpoint.trim());
        if endpoint.is_empty() {
            return Err("SSH profile is missing a host or endpoint.".to_string());
        }

        let (host, endpoint_port) = parse_host_and_port(endpoint)?;
        let port = config
            .get("port")
            .and_then(Value::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .or(endpoint_port);
        let known_hosts_path = resolve_known_hosts_path(
            config
                .get("knownHostsPath")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty()),
            app_data_dir,
        )?;

        Ok(Self {
            host,
            port,
            username: profile.username.clone().filter(|value| !value.trim().is_empty()),
            known_hosts_path,
        })
    }

    fn render_target(&self) -> String {
        match &self.username {
            Some(username) => format!("{username}@{}", self.host),
            None => self.host.clone(),
        }
    }
}

fn trust_with_keyscan(profile: &TrustedSshProfile) -> Result<(), String> {
    let parent = profile
        .known_hosts_path
        .parent()
        .ok_or_else(|| "Known hosts path is invalid.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("Failed to prepare known_hosts directory: {error}"))?;

    let mut command = Command::new("ssh-keyscan");
    if let Some(port) = profile.port {
        command.arg("-p").arg(port.to_string());
    }
    command.arg(&profile.host);

    let output = run_command_with_timeout(&mut command, Duration::from_secs(6), "ssh-keyscan")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "ssh-keyscan failed without stderr output.".to_string()
        } else {
            format!("ssh-keyscan failed: {stderr}")
        });
    }

    let scan_output = String::from_utf8_lossy(&output.stdout).to_string();
    if scan_output.trim().is_empty() {
        return Err("ssh-keyscan returned no host key entries.".to_string());
    }

    append_unique_known_hosts_entries(&profile.known_hosts_path, &scan_output)
}

fn trust_with_accept_new(profile: &TrustedSshProfile) -> Result<(), String> {
    let parent = profile
        .known_hosts_path
        .parent()
        .ok_or_else(|| "Known hosts path is invalid.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("Failed to prepare known_hosts directory: {error}"))?;
    let before = fs::read_to_string(&profile.known_hosts_path).unwrap_or_default();

    let mut command = Command::new("ssh");
    command.arg("-o").arg("BatchMode=yes");
    command.arg("-o").arg("PreferredAuthentications=none");
    command.arg("-o").arg("PubkeyAuthentication=no");
    command.arg("-o").arg("PasswordAuthentication=no");
    command.arg("-o").arg("NumberOfPasswordPrompts=0");
    command.arg("-o").arg("StrictHostKeyChecking=accept-new");
    command
        .arg("-o")
        .arg(format!("UserKnownHostsFile={}", profile.known_hosts_path.display()));
    command.arg("-o").arg("ConnectTimeout=5");
    if let Some(port) = profile.port {
        command.arg("-p").arg(port.to_string());
    }
    command.arg(profile.render_target());
    command.arg("exit");

    let _ = run_command_with_timeout(&mut command, Duration::from_secs(8), "ssh host key trust probe");
    let after = fs::read_to_string(&profile.known_hosts_path).unwrap_or_default();
    if after.len() > before.len() {
        Ok(())
    } else {
        Err(format!(
            "Unable to trust the SSH host key automatically for {}. Make sure either `ssh-keyscan` is available or your system SSH client supports `StrictHostKeyChecking=accept-new`.",
            profile.render_target()
        ))
    }
}

fn append_unique_known_hosts_entries(path: &Path, new_entries: &str) -> Result<(), String> {
    let mut existing = fs::read_to_string(path).unwrap_or_default();
    let mut changed = false;

    for line in new_entries.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if !existing.lines().any(|existing_line| existing_line.trim() == line) {
            if !existing.ends_with('\n') && !existing.is_empty() {
                existing.push('\n');
            }
            existing.push_str(line);
            existing.push('\n');
            changed = true;
        }
    }

    if changed || !path.exists() {
        fs::write(path, existing).map_err(|error| format!("Failed to update known_hosts file: {error}"))?;
    }

    Ok(())
}

fn resolve_known_hosts_path(configured: Option<&str>, app_data_dir: &Path) -> Result<PathBuf, String> {
    if let Some(path) = configured {
        return Ok(expand_home_path(path, app_data_dir));
    }

    if let Some(home_dir) = home_dir() {
        return Ok(home_dir.join(".ssh").join("known_hosts"));
    }

    Ok(app_data_dir.join("ssh").join("known_hosts"))
}

fn expand_home_path(value: &str, app_data_dir: &Path) -> PathBuf {
    if let Some(stripped) = value.strip_prefix("~/") {
        if let Some(home_dir) = home_dir() {
            return home_dir.join(stripped);
        }
    }
    if let Some(stripped) = value.strip_prefix("$HOME/") {
        if let Some(home_dir) = home_dir() {
            return home_dir.join(stripped);
        }
    }
    let candidate = PathBuf::from(value);
    if candidate.is_absolute() {
        candidate
    } else {
        app_data_dir.join(candidate)
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

fn parse_host_and_port(endpoint: &str) -> Result<(String, Option<u16>), String> {
    if endpoint.contains(' ') {
        return Err("SSH host cannot contain spaces.".to_string());
    }

    if let Some((host, port)) = endpoint.rsplit_once(':') {
        if host.contains(':') {
            return Ok((endpoint.to_string(), None));
        }
        if let Ok(port) = port.parse::<u16>() {
            return Ok((host.to_string(), Some(port)));
        }
    }

    Ok((endpoint.to_string(), None))
}
