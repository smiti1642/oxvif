use std::{collections::BTreeMap, sync::Mutex};

use crate::{AppError, SecretString};

const KEYRING_SERVICE: &str = "oxvif";

/// Secure password storage used by the application layer.
pub trait CredentialStore: Send + Sync {
    /// Create or replace one secret. Concurrent sequences must be serialized by
    /// the caller; the last successful native operation determines the value.
    fn set(&self, reference: &str, password: &str) -> Result<(), AppError>;
    /// Return an owned, zeroizing secret or `None` when the entry is absent.
    fn get(&self, reference: &str) -> Result<Option<SecretString>, AppError>;
    /// Delete one secret. Deleting an absent entry is successful.
    fn delete(&self, reference: &str) -> Result<(), AppError>;
}

/// Platform credential store selected by the production CLI.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemCredentialStore;

#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
impl CredentialStore for SystemCredentialStore {
    fn set(&self, reference: &str, password: &str) -> Result<(), AppError> {
        entry(reference)?
            .set_password(password)
            .map_err(|_| AppError::credential_backend_unavailable("store"))
    }

    fn get(&self, reference: &str) -> Result<Option<SecretString>, AppError> {
        match entry(reference)?.get_password() {
            Ok(password) => SecretString::new(password)
                .map(Some)
                .map_err(|_| AppError::credential_backend_unavailable("load")),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(AppError::credential_backend_unavailable("load")),
        }
    }

    fn delete(&self, reference: &str) -> Result<(), AppError> {
        match entry(reference)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(AppError::credential_backend_unavailable("delete")),
        }
    }
}

#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
fn entry(reference: &str) -> Result<keyring::Entry, AppError> {
    keyring::Entry::new(KEYRING_SERVICE, reference)
        .map_err(|_| AppError::credential_backend_unavailable("access"))
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
impl CredentialStore for SystemCredentialStore {
    fn set(&self, _reference: &str, _password: &str) -> Result<(), AppError> {
        Err(unsupported_platform())
    }

    fn get(&self, _reference: &str) -> Result<Option<SecretString>, AppError> {
        Err(unsupported_platform())
    }

    fn delete(&self, _reference: &str) -> Result<(), AppError> {
        Err(unsupported_platform())
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn unsupported_platform() -> AppError {
    AppError::credential_backend_unavailable("access")
}

/// Deterministic credential store for application and adapter tests.
#[derive(Debug, Default)]
pub struct MemoryCredentialStore {
    entries: Mutex<BTreeMap<String, SecretString>>,
}

impl CredentialStore for MemoryCredentialStore {
    fn set(&self, reference: &str, password: &str) -> Result<(), AppError> {
        self.entries
            .lock()
            .map_err(|_| AppError::internal("Memory credential store lock was poisoned."))?
            .insert(reference.to_owned(), SecretString::new(password)?);
        Ok(())
    }

    fn get(&self, reference: &str) -> Result<Option<SecretString>, AppError> {
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

pub fn credential_profile_reference(profile_id: &str) -> String {
    format!("profile/{profile_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_store_contract(store: &dyn CredentialStore, reference: &str) {
        store.delete(reference).expect("pre-test cleanup");
        assert_eq!(store.get(reference).expect("initial get"), None);
        store.set(reference, "first-secret").expect("initial set");
        assert_eq!(
            store
                .get(reference)
                .expect("get after set")
                .as_ref()
                .map(SecretString::expose_secret),
            Some("first-secret")
        );
        store
            .set(reference, "replacement-secret")
            .expect("replacement set");
        assert_eq!(
            store
                .get(reference)
                .expect("get replacement")
                .as_ref()
                .map(SecretString::expose_secret),
            Some("replacement-secret")
        );
        store.delete(reference).expect("delete");
        assert_eq!(store.get(reference).expect("get after delete"), None);
        store.delete(reference).expect("idempotent delete");
    }

    #[test]
    fn memory_store_round_trips_and_deletes_secret() {
        let store = MemoryCredentialStore::default();
        assert_store_contract(&store, "device/camera");
    }

    #[test]
    fn native_backend_error_contract_does_not_echo_sensitive_context() {
        let error = AppError::credential_backend_unavailable("load");
        let rendered = format!("{error:?}");
        assert_eq!(error.code, crate::ErrorCode::CredentialUnavailable);
        assert!(!rendered.contains("sensitive-account"));
        assert!(!rendered.contains("sensitive-secret"));
        assert!(rendered.contains("plaintext fallback"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires DBUS_SESSION_BUS_ADDRESS to be absent"]
    fn system_store_unavailable_contract() {
        assert!(std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none());
        let error = SystemCredentialStore
            .get("contract-test/sensitive-account")
            .expect_err("a missing D-Bus session must be reported");
        let rendered = format!("{error:?}");
        assert_eq!(error.code, crate::ErrorCode::CredentialUnavailable);
        assert!(!rendered.contains("sensitive-account"));
        assert!(rendered.contains("Secret Service"));
        assert!(rendered.contains("plaintext fallback"));
    }

    #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
    #[test]
    #[ignore = "requires an isolated native credential backend session"]
    fn system_store_contract() {
        let store = SystemCredentialStore;
        let reference = format!(
            "contract-test/{}/{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should follow the Unix epoch")
                .as_nanos()
        );
        struct Cleanup<'a> {
            store: &'a dyn CredentialStore,
            reference: &'a str,
        }
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = self.store.delete(self.reference);
            }
        }
        let _cleanup = Cleanup {
            store: &store,
            reference: &reference,
        };
        assert_store_contract(&store, &reference);
    }
}
