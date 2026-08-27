use std::{collections::BTreeMap, sync::Mutex};

use crate::AppError;

const KEYRING_SERVICE: &str = "oxvif";

/// Secure password storage used by the application layer.
pub trait CredentialStore: Send + Sync {
    fn set(&self, reference: &str, password: &str) -> Result<(), AppError>;
    fn get(&self, reference: &str) -> Result<Option<String>, AppError>;
    fn delete(&self, reference: &str) -> Result<(), AppError>;
}

/// Platform credential store selected by the production CLI.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemCredentialStore;

#[cfg(windows)]
impl CredentialStore for SystemCredentialStore {
    fn set(&self, reference: &str, password: &str) -> Result<(), AppError> {
        entry(reference)?.set_password(password).map_err(|error| {
            AppError::credential_unavailable(format!(
                "Failed to store credential `{reference}`: {error}"
            ))
        })
    }

    fn get(&self, reference: &str) -> Result<Option<String>, AppError> {
        match entry(reference)?.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(AppError::credential_unavailable(format!(
                "Failed to load credential `{reference}`: {error}"
            ))),
        }
    }

    fn delete(&self, reference: &str) -> Result<(), AppError> {
        match entry(reference)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(AppError::credential_unavailable(format!(
                "Failed to delete credential `{reference}`: {error}"
            ))),
        }
    }
}

#[cfg(windows)]
fn entry(reference: &str) -> Result<keyring::Entry, AppError> {
    keyring::Entry::new(KEYRING_SERVICE, reference).map_err(|error| {
        AppError::credential_unavailable(format!(
            "Failed to access credential `{reference}`: {error}"
        ))
    })
}

#[cfg(not(windows))]
impl CredentialStore for SystemCredentialStore {
    fn set(&self, _reference: &str, _password: &str) -> Result<(), AppError> {
        Err(unsupported_platform())
    }

    fn get(&self, _reference: &str) -> Result<Option<String>, AppError> {
        Err(unsupported_platform())
    }

    fn delete(&self, _reference: &str) -> Result<(), AppError> {
        Err(unsupported_platform())
    }
}

#[cfg(not(windows))]
fn unsupported_platform() -> AppError {
    AppError::credential_unavailable("No native credential backend is enabled for this platform.")
}

/// Deterministic credential store for application and adapter tests.
#[derive(Debug, Default)]
pub struct MemoryCredentialStore {
    entries: Mutex<BTreeMap<String, String>>,
}

impl CredentialStore for MemoryCredentialStore {
    fn set(&self, reference: &str, password: &str) -> Result<(), AppError> {
        self.entries
            .lock()
            .map_err(|_| AppError::internal("Memory credential store lock was poisoned."))?
            .insert(reference.to_owned(), password.to_owned());
        Ok(())
    }

    fn get(&self, reference: &str) -> Result<Option<String>, AppError> {
        Ok(self
            .entries
            .lock()
            .map_err(|_| AppError::internal("Memory credential store lock was poisoned."))?
            .get(reference)
            .cloned())
    }

    fn delete(&self, reference: &str) -> Result<(), AppError> {
        self.entries
            .lock()
            .map_err(|_| AppError::internal("Memory credential store lock was poisoned."))?
            .remove(reference);
        Ok(())
    }
}

pub fn credential_reference(device_id: &str) -> String {
    format!("device/{device_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips_and_deletes_secret() {
        let store = MemoryCredentialStore::default();
        store.set("device/camera", "secret").expect("set");
        assert_eq!(
            store.get("device/camera").expect("get").as_deref(),
            Some("secret")
        );
        store.delete("device/camera").expect("delete");
        assert_eq!(store.get("device/camera").expect("get"), None);
    }
}
