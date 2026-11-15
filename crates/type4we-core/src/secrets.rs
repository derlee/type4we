//! Secret storage boundary.
//!
//! API keys never go into `config.json`. On Windows the app implements this
//! trait with Credential Manager. Tests use [`MemorySecretStore`].

use std::collections::HashMap;
use std::sync::Mutex;

/// Credential Manager target name. The account matches Type4Me's grouped
/// keychain account (`tf_asr_soniox`) so a later importer can use the same ids.
pub fn credential_target(account: &str, field: &str) -> String {
    format!("com.type4we.grouped:{account}:{field}")
}

pub const SONIOX_ACCOUNT: &str = "tf_asr_soniox";
pub const VOLCANO_ACCOUNT: &str = "tf_asr_volcano";
pub const API_KEY_FIELD: &str = "apiKey";
pub const APP_KEY_FIELD: &str = "appKey";
pub const ACCESS_KEY_FIELD: &str = "accessKey";

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("{0}")]
    Message(String),
}

pub trait SecretStore: Send + Sync {
    fn get(&self, account: &str, field: &str) -> Result<Option<String>, SecretError>;
    fn set(&self, account: &str, field: &str, value: &str) -> Result<(), SecretError>;
    fn delete(&self, account: &str, field: &str) -> Result<(), SecretError>;
}

#[derive(Debug, Default)]
pub struct MemorySecretStore {
    values: Mutex<HashMap<(String, String), String>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, account: &str, field: &str) -> Result<Option<String>, SecretError> {
        let guard = self
            .values
            .lock()
            .map_err(|err| SecretError::Message(err.to_string()))?;
        Ok(guard
            .get(&(account.to_string(), field.to_string()))
            .cloned())
    }

    fn set(&self, account: &str, field: &str, value: &str) -> Result<(), SecretError> {
        let mut guard = self
            .values
            .lock()
            .map_err(|err| SecretError::Message(err.to_string()))?;
        guard.insert((account.to_string(), field.to_string()), value.to_string());
        Ok(())
    }

    fn delete(&self, account: &str, field: &str) -> Result<(), SecretError> {
        let mut guard = self
            .values
            .lock()
            .map_err(|err| SecretError::Message(err.to_string()))?;
        guard.remove(&(account.to_string(), field.to_string()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_roundtrip_and_target_name() {
        let store = MemorySecretStore::new();
        assert!(store.get(SONIOX_ACCOUNT, API_KEY_FIELD).unwrap().is_none());
        store.set(SONIOX_ACCOUNT, API_KEY_FIELD, "secret").unwrap();
        assert_eq!(
            store.get(SONIOX_ACCOUNT, API_KEY_FIELD).unwrap().as_deref(),
            Some("secret")
        );
        store.delete(SONIOX_ACCOUNT, API_KEY_FIELD).unwrap();
        assert!(store.get(SONIOX_ACCOUNT, API_KEY_FIELD).unwrap().is_none());
        assert_eq!(
            credential_target(SONIOX_ACCOUNT, API_KEY_FIELD),
            "com.type4we.grouped:tf_asr_soniox:apiKey"
        );
    }
}
