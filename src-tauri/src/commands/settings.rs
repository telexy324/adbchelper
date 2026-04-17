use std::fs;
use std::path::{Path, PathBuf};
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
use crate::models::ssh::SshKeyPairResult;
use crate::orchestrator::ssh_tools::{resolve_ssh, resolve_ssh_keygen, resolve_ssh_keyscan, SshToolLocator};
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
    if trust_with_keyscan(&state, &ssh_config).is_err() {
        trust_with_accept_new(&state, &ssh_config)?;
    }

    Ok(format!(
        "Trusted SSH host key for {} using {}.",
        ssh_config.render_target(),
        ssh_config.known_hosts_path.display()
    ))
}

#[tauri::command]
pub fn prepare_ssh_rsa_keypair(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<SshKeyPairResult, String> {
    let connection = open_connection(&state.storage_path)?;
    let profile = db::get_connection_profile(&connection, &profile_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "SSH profile not found.".to_string())?;

    if profile.profile_type != "ssh" {
        return Err("RSA key preparation is only available for SSH profiles.".to_string());
    }

    let keys_dir = Path::new(&state.app_data_dir).join("ssh_keys");
    fs::create_dir_all(&keys_dir).map_err(|error| format!("Failed to prepare SSH key directory: {error}"))?;
    let private_key_path = keys_dir.join(format!("{profile_id}_rsa"));
    let public_key_path = PathBuf::from(format!("{}.pub", private_key_path.display()));
    let created = if private_key_path.exists() && public_key_path.exists() {
        false
    } else {
        generate_rsa_keypair(&state, &profile, &private_key_path, &profile.name)?;
        true
    };

    let public_key = fs::read_to_string(&public_key_path)
        .map_err(|error| format!("Failed to read generated SSH public key: {error}"))?;
    let updated_profile = save_profile_with_rsa_keypair(
        &connection,
        &profile,
        &private_key_path,
        &public_key_path,
    )?;

    Ok(SshKeyPairResult {
        profile: updated_profile,
        private_key_path: private_key_path.display().to_string(),
        public_key_path: public_key_path.display().to_string(),
        public_key: public_key.trim().to_string(),
        created,
    })
}

#[derive(Debug)]
struct TrustedSshProfile {
    host: String,
    port: Option<u16>,
    username: Option<String>,
    ssh_path: Option<String>,
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
        let ssh_path = config
            .get("sshPath")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
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
            ssh_path,
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

fn trust_with_keyscan(state: &AppState, profile: &TrustedSshProfile) -> Result<(), String> {
    let parent = profile
        .known_hosts_path
        .parent()
        .ok_or_else(|| "Known hosts path is invalid.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("Failed to prepare known_hosts directory: {error}"))?;

    let keyscan = resolve_ssh_keyscan(
        SshToolLocator {
            resource_dir: Some(Path::new(&state.resource_dir)),
            executable_dir: Some(Path::new(&state.executable_dir)),
        },
        profile.ssh_path.as_deref(),
    )?;
    let mut command = keyscan.command();
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

fn trust_with_accept_new(state: &AppState, profile: &TrustedSshProfile) -> Result<(), String> {
    let parent = profile
        .known_hosts_path
        .parent()
        .ok_or_else(|| "Known hosts path is invalid.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("Failed to prepare known_hosts directory: {error}"))?;
    let before = fs::read_to_string(&profile.known_hosts_path).unwrap_or_default();

    let ssh = resolve_ssh(
        SshToolLocator {
            resource_dir: Some(Path::new(&state.resource_dir)),
            executable_dir: Some(Path::new(&state.executable_dir)),
        },
        profile.ssh_path.as_deref(),
    )?;
    let mut command = ssh.command();
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

fn generate_rsa_keypair(
    state: &AppState,
    profile: &ConnectionProfile,
    private_key_path: &Path,
    profile_name: &str,
) -> Result<(), String> {
    let config = serde_json::from_str::<Value>(&profile.config_json)
        .unwrap_or_else(|_| Value::Object(Default::default()));
    let keygen = resolve_ssh_keygen(
        SshToolLocator {
            resource_dir: Some(Path::new(&state.resource_dir)),
            executable_dir: Some(Path::new(&state.executable_dir)),
        },
        config.get("sshPath").and_then(Value::as_str),
    )?;
    let mut command = keygen.command();
    command.arg("-q");
    command.arg("-t").arg("rsa");
    command.arg("-b").arg("4096");
    command.arg("-m").arg("PEM");
    command.arg("-N").arg("");
    command.arg("-f").arg(private_key_path);
    command.arg("-C").arg(format!("adbchelper:{profile_name}"));

    let output = run_command_with_timeout(&mut command, Duration::from_secs(10), "ssh-keygen")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "ssh-keygen failed without stderr output.".to_string()
        } else {
            format!("ssh-keygen failed: {stderr}")
        });
    }

    Ok(())
}

fn save_profile_with_rsa_keypair(
    connection: &Connection,
    profile: &ConnectionProfile,
    private_key_path: &Path,
    public_key_path: &Path,
) -> Result<ConnectionProfile, String> {
    let mut config = if profile.config_json.trim().is_empty() {
        Value::Object(Default::default())
    } else {
        serde_json::from_str::<Value>(&profile.config_json)
            .unwrap_or_else(|_| Value::Object(Default::default()))
    };

    let object = config
        .as_object_mut()
        .ok_or_else(|| "SSH profile config must be a JSON object.".to_string())?;
    object.insert("authMode".to_string(), Value::String("rsa".to_string()));
    object.insert(
        "privateKeyPath".to_string(),
        Value::String(private_key_path.display().to_string()),
    );
    object.insert(
        "publicKeyPath".to_string(),
        Value::String(public_key_path.display().to_string()),
    );

    let input = UpsertConnectionProfileInput {
        id: Some(profile.id.clone()),
        environment_id: profile.environment_id.clone(),
        profile_type: profile.profile_type.clone(),
        name: profile.name.clone(),
        endpoint: profile.endpoint.clone(),
        username: profile.username.clone(),
        default_scope: profile.default_scope.clone(),
        notes: profile.notes.clone(),
        config_json: Some(serde_json::to_string(&config).map_err(|error| error.to_string())?),
        secret_value: None,
    };

    db::upsert_connection_profile(connection, &input, &profile.id, profile.has_secret)
        .map_err(|error| error.to_string())
}
