use keyring::Entry;
use thiserror::Error;

const SERVICE_NAME: &str = "adbchelper";

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("failed to access system keychain: {0}")]
    Access(#[from] keyring::Error),
}

fn entry_for_profile(profile_id: &str) -> Result<Entry, keyring::Error> {
    Entry::new(SERVICE_NAME, &format!("connection-profile:{profile_id}"))
}

pub fn set_profile_secret(profile_id: &str, secret: &str) -> Result<(), SecretError> {
    let entry = entry_for_profile(profile_id)?;
    entry.set_password(secret)?;
    Ok(())
}

pub fn get_profile_secret(profile_id: &str) -> Result<String, SecretError> {
    let entry = entry_for_profile(profile_id)?;
    entry.get_password().map_err(SecretError::Access)
}

pub fn delete_profile_secret(profile_id: &str) -> Result<(), SecretError> {
    let entry = entry_for_profile(profile_id)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(SecretError::Access(error)),
    }
}
