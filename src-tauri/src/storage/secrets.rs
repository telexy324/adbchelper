use std::fs;
use std::path::{Path, PathBuf};

use keyring::Entry;
use thiserror::Error;

const SERVICE_NAME: &str = "adbchelper";

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("failed to access secure secret storage: {0}")]
    Access(String),
    #[error("failed to persist fallback secret file: {0}")]
    Io(#[from] std::io::Error),
    #[error("no stored secret found")]
    Missing,
}

fn entry_for_profile(profile_id: &str) -> Result<Entry, keyring::Error> {
    Entry::new(SERVICE_NAME, &format!("connection-profile:{profile_id}"))
}

fn fallback_secret_path(app_data_dir: &Path, profile_id: &str) -> PathBuf {
    app_data_dir.join("secrets").join(format!("{profile_id}.secret"))
}

fn write_fallback_secret(app_data_dir: &Path, profile_id: &str, secret: &str) -> Result<(), std::io::Error> {
    let path = fallback_secret_path(app_data_dir, profile_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, secret)
}

fn read_fallback_secret(app_data_dir: &Path, profile_id: &str) -> Result<String, SecretError> {
    let path = fallback_secret_path(app_data_dir, profile_id);
    let value = fs::read_to_string(path).map_err(SecretError::Io)?;
    if value.trim().is_empty() {
        return Err(SecretError::Missing);
    }
    Ok(value)
}

pub fn set_profile_secret(app_data_dir: Option<&Path>, profile_id: &str, secret: &str) -> Result<(), SecretError> {
    let mut keyring_error = None;

    match entry_for_profile(profile_id) {
        Ok(entry) => {
            if let Err(error) = entry.set_password(secret) {
                keyring_error = Some(error.to_string());
            }
        }
        Err(error) => {
            keyring_error = Some(error.to_string());
        }
    }

    if let Some(app_data_dir) = app_data_dir {
        write_fallback_secret(app_data_dir, profile_id, secret)?;
    }

    if let Some(error) = keyring_error {
        return Err(SecretError::Access(format!(
            "system keychain unavailable{}: {error}",
            if app_data_dir.is_some() {
                ", stored secret in app data fallback instead"
            } else {
                ""
            }
        )));
    }

    Ok(())
}

pub fn get_profile_secret(app_data_dir: Option<&Path>, profile_id: &str) -> Result<String, SecretError> {
    if let Ok(entry) = entry_for_profile(profile_id) {
        if let Ok(secret) = entry.get_password() {
            if !secret.trim().is_empty() {
                return Ok(secret);
            }
        }
    }

    if let Some(app_data_dir) = app_data_dir {
        return read_fallback_secret(app_data_dir, profile_id);
    }

    Err(SecretError::Missing)
}

pub fn has_profile_secret(app_data_dir: Option<&Path>, profile_id: &str) -> bool {
    get_profile_secret(app_data_dir, profile_id).is_ok()
}

pub fn delete_profile_secret(app_data_dir: Option<&Path>, profile_id: &str) -> Result<(), SecretError> {
    if let Ok(entry) = entry_for_profile(profile_id) {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => {
                let _ = error;
            }
        }
    }

    if let Some(app_data_dir) = app_data_dir {
        let path = fallback_secret_path(app_data_dir, profile_id);
        if path.exists() {
            fs::remove_file(path)?;
        }
    }

    Ok(())
}
