use std::{env, sync::Arc, time::Duration};

use oxvif::{DeviceInfo, OnvifClient};
use tokio::time::{Instant, timeout};

use crate::{
    AppError, CommandData, CommandRequest, CommandSuccess, CredentialStore, DeviceMetadata,
    LiveDeviceInfo, RegistryStore, ResultMeta, SystemCredentialStore, Warning,
    credential_reference, describe, normalize_target,
};

/// Invocation policy shared by CLI and future non-CLI adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionOptions {
    pub non_interactive: bool,
    pub timeout: Duration,
    pub retries: u32,
    pub verbosity: u8,
    pub quiet: bool,
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            non_interactive: false,
            timeout: Duration::from_secs(10),
            retries: 0,
            verbosity: 0,
            quiet: false,
        }
    }
}

/// Executes typed oxvif commands without depending on terminal parsing.
pub struct Application {
    registry: RegistryStore,
    credentials: Arc<dyn CredentialStore>,
}

impl Application {
    pub fn system() -> Result<Self, AppError> {
        Ok(Self {
            registry: RegistryStore::system()?,
            credentials: Arc::new(SystemCredentialStore),
        })
    }

    pub fn with_stores(registry: RegistryStore, credentials: Arc<dyn CredentialStore>) -> Self {
        Self {
            registry,
            credentials,
        }
    }

    pub fn registry(&self) -> &RegistryStore {
        &self.registry
    }

    pub async fn execute(
        &self,
        request: CommandRequest,
        options: &ExecutionOptions,
    ) -> Result<CommandSuccess, AppError> {
        let started = Instant::now();
        let command_name = request.name();
        let outcome = match request {
            CommandRequest::Describe(request) => Outcome::data(describe::execute(request)?),
            CommandRequest::DeviceAdd(request) => {
                let device = self.registry.add(request.device)?;
                Outcome::device("added", device)
            }
            CommandRequest::DeviceList => {
                let (devices, current_device) = self.registry.list()?;
                Outcome::data(CommandData::DeviceList {
                    devices,
                    current_device,
                })
            }
            CommandRequest::DeviceShow(request) => {
                let device = self.registry.get(&request.id)?;
                Outcome::device("shown", device)
            }
            CommandRequest::DeviceUpdate(request) => {
                let device = self.registry.update(&request.id, request.update)?;
                Outcome::device("updated", device)
            }
            CommandRequest::DeviceRename(request) => {
                let device = self.registry.rename(&request.id, &request.name)?;
                Outcome::device("renamed", device)
            }
            CommandRequest::DeviceRemove(request) => self.remove_device(&request.id)?,
            CommandRequest::DeviceCredentialSet(request) => {
                let id = request.id;
                self.registry.get(&id)?;
                let reference = credential_reference(&id);
                self.credentials
                    .set(&reference, request.password.expose_secret())?;
                let device = match self
                    .registry
                    .set_credentials(&id, &request.username, &reference)
                {
                    Ok(device) => device,
                    Err(error) => {
                        let _ = self.credentials.delete(&reference);
                        return Err(error);
                    }
                };
                Outcome {
                    data: CommandData::CredentialUpdated {
                        action: "set".to_owned(),
                        device: device.clone(),
                    },
                    warnings: Vec::new(),
                    device_id: Some(device.id),
                    target: Some(device.target),
                }
            }
            CommandRequest::DeviceCredentialDelete(request) => {
                let stored = self.registry.get_stored(&request.id)?;
                if let Some(reference) = stored.credential_ref() {
                    self.credentials.delete(reference)?;
                }
                let device = self.registry.clear_credentials(&request.id)?;
                Outcome {
                    data: CommandData::CredentialUpdated {
                        action: "deleted".to_owned(),
                        device: device.clone(),
                    },
                    warnings: Vec::new(),
                    device_id: Some(device.id),
                    target: Some(device.target),
                }
            }
            CommandRequest::Use(request) => {
                let device = self.registry.set_current(&request.id)?;
                Outcome::device("selected", device)
            }
            CommandRequest::Current => {
                let device = self.registry.current()?;
                Outcome {
                    device_id: device.as_ref().map(|device| device.id.clone()),
                    target: device.as_ref().map(|device| device.target.clone()),
                    data: CommandData::CurrentDevice { device },
                    warnings: Vec::new(),
                }
            }
            CommandRequest::DeviceTest(request) => {
                let resolved = self.resolve_target(request.selector)?;
                let information = self.fetch_device_info(&resolved, options).await?;
                Outcome {
                    data: CommandData::DeviceTest {
                        device_id: resolved.device_id.clone(),
                        target: resolved.target.clone(),
                        authenticated: resolved.password.is_some(),
                        information,
                    },
                    warnings: Vec::new(),
                    device_id: resolved.device_id,
                    target: Some(resolved.target),
                }
            }
            CommandRequest::DeviceInfo(request) => {
                let resolved = self.resolve_target(request.selector)?;
                let information = self.fetch_device_info(&resolved, options).await?;
                Outcome {
                    data: CommandData::DeviceInformation {
                        device_id: resolved.device_id.clone(),
                        target: resolved.target.clone(),
                        information,
                    },
                    warnings: Vec::new(),
                    device_id: resolved.device_id,
                    target: Some(resolved.target),
                }
            }
            CommandRequest::DeviceRefresh(request) => {
                let previous = self.registry.get(&request.id)?;
                let resolved = self.resolve_saved(&request.id)?;
                let information = self.fetch_device_info(&resolved, options).await?;
                let mut warnings = Vec::new();
                if let Some(warning) = identity_change_warning(
                    &request.id,
                    previous.serial_number.as_deref(),
                    &information.serial_number,
                ) {
                    warnings.push(warning);
                }
                let device = self.registry.update_metadata(
                    &request.id,
                    DeviceMetadata {
                        manufacturer: information.manufacturer,
                        model: information.model,
                        firmware_version: information.firmware_version,
                        serial_number: information.serial_number,
                    },
                )?;
                let mut outcome = Outcome::device("refreshed", device);
                outcome.warnings = warnings;
                outcome
            }
        };

        Ok(CommandSuccess {
            data: outcome.data,
            warnings: outcome.warnings,
            meta: ResultMeta {
                command: Some(command_name.to_owned()),
                device_id: outcome.device_id,
                target: outcome.target,
                elapsed_ms: elapsed_millis(started),
            },
        })
    }

    fn remove_device(&self, id: &str) -> Result<Outcome, AppError> {
        let stored = self.registry.get_stored(id)?;
        if let Some(reference) = stored.credential_ref() {
            self.credentials.delete(reference)?;
        }
        self.registry.remove(id)?;
        Ok(Outcome {
            data: CommandData::DeviceRemoved { id: id.to_owned() },
            warnings: Vec::new(),
            device_id: Some(id.to_owned()),
            target: Some(stored.target().to_owned()),
        })
    }

    fn resolve_target(&self, selector: crate::TargetSelector) -> Result<ResolvedTarget, AppError> {
        if selector.device.is_some() && selector.target.is_some() {
            return Err(AppError::invalid_argument(
                "--device and --target are mutually exclusive.",
            ));
        }
        if let Some(target) = selector.target {
            return self.resolve_direct(&target);
        }
        if let Some(id) = selector.device {
            return self.resolve_saved(&id);
        }
        if let Ok(id) = env::var("OXVIF_DEVICE")
            && !id.trim().is_empty()
        {
            return self.resolve_saved(id.trim());
        }
        if let Some(device) = self.registry.current()? {
            return self.resolve_saved(&device.id);
        }
        Err(AppError::missing_target())
    }

    fn resolve_saved(&self, id: &str) -> Result<ResolvedTarget, AppError> {
        let stored = self.registry.get_stored(id)?;
        let password = match stored.credential_ref() {
            Some(reference) => self.credentials.get(reference)?.ok_or_else(|| {
                AppError::credential_unavailable(format!(
                    "Credential `{reference}` is referenced by `{id}` but does not exist."
                ))
            })?,
            None => env::var("OXVIF_PASSWORD").unwrap_or_default(),
        };
        let username = stored
            .username()
            .map(str::to_owned)
            .or_else(|| env::var("OXVIF_USERNAME").ok());
        let password =
            if stored.credential_ref().is_some() || env::var_os("OXVIF_PASSWORD").is_some() {
                Some(password)
            } else {
                None
            };
        if password.is_some() && username.is_none() {
            return Err(AppError::credential_unavailable(format!(
                "Device `{id}` has a password but no username."
            )));
        }
        Ok(ResolvedTarget {
            device_id: Some(id.to_owned()),
            target: stored.target().to_owned(),
            username,
            password,
        })
    }

    fn resolve_direct(&self, target: &str) -> Result<ResolvedTarget, AppError> {
        let username = env::var("OXVIF_USERNAME").ok();
        let password = env::var("OXVIF_PASSWORD").ok();
        if password.is_some() && username.is_none() {
            return Err(AppError::credential_unavailable(
                "OXVIF_PASSWORD is set but OXVIF_USERNAME is missing.",
            ));
        }
        Ok(ResolvedTarget {
            device_id: None,
            target: normalize_target(target)?,
            username,
            password,
        })
    }

    async fn fetch_device_info(
        &self,
        resolved: &ResolvedTarget,
        options: &ExecutionOptions,
    ) -> Result<LiveDeviceInfo, AppError> {
        let attempts = options.retries.saturating_add(1);
        let mut last_error = None;
        for _ in 0..attempts {
            let mut client = OnvifClient::new(&resolved.target);
            if let (Some(username), Some(password)) = (&resolved.username, &resolved.password) {
                client = client.with_credentials(username, password);
            }
            match timeout(options.timeout, client.get_device_info()).await {
                Ok(Ok(information)) => return Ok(live_information(information)),
                Ok(Err(error)) => last_error = Some(error.to_string()),
                Err(_) => {
                    last_error = Some(format!(
                        "Request exceeded the {} ms timeout.",
                        options.timeout.as_millis()
                    ));
                }
            }
        }
        Err(AppError::device_connection_failed(
            last_error.unwrap_or_else(|| "Device request failed without an error.".to_owned()),
        ))
    }
}

struct ResolvedTarget {
    device_id: Option<String>,
    target: String,
    username: Option<String>,
    password: Option<String>,
}

struct Outcome {
    data: CommandData,
    warnings: Vec<Warning>,
    device_id: Option<String>,
    target: Option<String>,
}

impl Outcome {
    fn data(data: CommandData) -> Self {
        Self {
            data,
            warnings: Vec::new(),
            device_id: None,
            target: None,
        }
    }

    fn device(action: &str, device: crate::DeviceView) -> Self {
        Self {
            data: CommandData::DeviceRecord {
                action: action.to_owned(),
                device: device.clone(),
            },
            warnings: Vec::new(),
            device_id: Some(device.id),
            target: Some(device.target),
        }
    }
}

fn live_information(information: DeviceInfo) -> LiveDeviceInfo {
    LiveDeviceInfo {
        manufacturer: information.manufacturer,
        model: information.model,
        firmware_version: information.firmware_version,
        serial_number: information.serial_number,
        hardware_id: information.hardware_id,
    }
}

fn identity_change_warning(
    id: &str,
    previous_serial: Option<&str>,
    current_serial: &str,
) -> Option<Warning> {
    previous_serial
        .filter(|previous| *previous != current_serial)
        .map(|previous| Warning {
            code: "DEVICE_IDENTITY_CHANGED".to_owned(),
            message: format!(
                "Saved device `{id}` previously reported serial `{previous}` but now reports `{current_serial}`; the endpoint may identify a replacement device."
            ),
        })
}

fn elapsed_millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DeviceAddRequest, DeviceCredentialSetRequest, DeviceIdRequest, MemoryCredentialStore,
        NewDevice, SecretString,
    };

    #[tokio::test]
    async fn credential_is_stored_outside_registry_and_removed_with_device() {
        let directory = tempfile::tempdir().expect("temp directory");
        let registry = RegistryStore::at(directory.path());
        let credentials = Arc::new(MemoryCredentialStore::default());
        let application = Application::with_stores(registry.clone(), credentials.clone());
        let options = ExecutionOptions::default();

        application
            .execute(
                CommandRequest::DeviceAdd(DeviceAddRequest {
                    device: NewDevice {
                        id: "camera".to_owned(),
                        name: None,
                        target: "192.168.1.20".to_owned(),
                        tags: Vec::new(),
                    },
                }),
                &options,
            )
            .await
            .expect("device add should succeed");
        application
            .execute(
                CommandRequest::DeviceCredentialSet(DeviceCredentialSetRequest {
                    id: "camera".to_owned(),
                    username: "admin".to_owned(),
                    password: SecretString::new("top-secret").expect("secret should construct"),
                }),
                &options,
            )
            .await
            .expect("credential set should succeed");

        assert_eq!(
            credentials
                .get("device/camera")
                .expect("credential should load")
                .as_deref(),
            Some("top-secret")
        );
        let registry_contents = std::fs::read_to_string(directory.path().join("devices.toml"))
            .expect("registry should exist");
        assert!(!registry_contents.contains("top-secret"));
        assert!(
            registry
                .get("camera")
                .expect("device should load")
                .has_credentials
        );

        application
            .execute(
                CommandRequest::DeviceRemove(DeviceIdRequest {
                    id: "camera".to_owned(),
                }),
                &options,
            )
            .await
            .expect("device remove should succeed");
        assert_eq!(
            credentials
                .get("device/camera")
                .expect("credential lookup should work"),
            None
        );
    }

    #[test]
    fn serial_change_warns_without_flagging_first_refresh() {
        assert!(identity_change_warning("camera", None, "new").is_none());
        assert!(identity_change_warning("camera", Some("same"), "same").is_none());
        let warning = identity_change_warning("camera", Some("old"), "new")
            .expect("changed serial should warn");
        assert_eq!(warning.code, "DEVICE_IDENTITY_CHANGED");
        assert!(warning.message.contains("replacement device"));
    }
}
