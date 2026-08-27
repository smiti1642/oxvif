use std::{env, sync::Arc, time::Duration};

use oxvif::{DeviceInfo, OnvifClient};
use tokio::time::{Instant, timeout};

use crate::{
    AppError, CommandData, CommandRequest, CommandSuccess, CredentialStore, DeviceMetadata,
    LiveDeviceInfo, RegistryStore, ResultMeta, SystemCredentialStore, Warning,
    credential_profile_reference, credential_reference, describe, normalize_target,
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
                let id = self.registry.resolve_device_selector(&request.id)?;
                let device = self.registry.get(&id)?;
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
            CommandRequest::DeviceCredentialUseProfile(request) => {
                let device = self
                    .registry
                    .assign_credential_profile(&request.device_id, &request.profile_id)?;
                Outcome::device("credential profile assigned", device)
            }
            CommandRequest::CredentialProfileSet(request) => {
                let id = request.id;
                let reference = credential_profile_reference(&id);
                let previous = match self.registry.get_stored_credential_profile(&id) {
                    Ok(profile) => Some(profile),
                    Err(error) if error.code == crate::ErrorCode::ResourceNotFound => None,
                    Err(error) => return Err(error),
                };
                let previous_secret = match &previous {
                    Some(profile) => self.credentials.get(profile.credential_ref())?,
                    None => None,
                };
                self.credentials
                    .set(&reference, request.password.expose_secret())?;
                let profile =
                    match self
                        .registry
                        .set_credential_profile(&id, &request.username, &reference)
                    {
                        Ok(profile) => profile,
                        Err(error) => {
                            if let Some(secret) = previous_secret {
                                let _ = self.credentials.set(&reference, &secret);
                            } else {
                                let _ = self.credentials.delete(&reference);
                            }
                            return Err(error);
                        }
                    };
                Outcome::data(CommandData::CredentialProfileRecord {
                    action: if previous.is_some() { "updated" } else { "set" }.to_owned(),
                    profile,
                })
            }
            CommandRequest::CredentialProfileList => {
                Outcome::data(CommandData::CredentialProfileList {
                    profiles: self.registry.list_credential_profiles()?,
                })
            }
            CommandRequest::CredentialProfileShow(request) => {
                Outcome::data(CommandData::CredentialProfileRecord {
                    action: "shown".to_owned(),
                    profile: self.registry.get_credential_profile(&request.id)?,
                })
            }
            CommandRequest::CredentialProfileDelete(request) => {
                let profile = self.registry.remove_credential_profile(&request.id)?;
                if let Err(error) = self.credentials.delete(profile.credential_ref()) {
                    let _ = self.registry.set_credential_profile(
                        &request.id,
                        profile.username(),
                        profile.credential_ref(),
                    );
                    return Err(error);
                }
                Outcome::data(CommandData::ResourceRemoved {
                    resource: "credential_profile".to_owned(),
                    id: request.id,
                })
            }
            CommandRequest::GroupCreate(request) => Outcome::data(CommandData::GroupRecord {
                action: "created".to_owned(),
                group: self.registry.create_group(request.group)?,
            }),
            CommandRequest::GroupList => Outcome::data(CommandData::GroupList {
                groups: self.registry.list_groups()?,
            }),
            CommandRequest::GroupShow(request) => Outcome::data(CommandData::GroupRecord {
                action: "shown".to_owned(),
                group: self.registry.get_group(&request.id)?,
            }),
            CommandRequest::GroupDelete(request) => Outcome::data(CommandData::GroupRecord {
                action: "deleted".to_owned(),
                group: self.registry.delete_group(&request.id)?,
            }),
            CommandRequest::GroupMemberAdd(request) => Outcome::data(CommandData::GroupRecord {
                action: "member added".to_owned(),
                group: self.registry.add_group_member(
                    &request.group_id,
                    &request.device_id,
                    &request.alias,
                )?,
            }),
            CommandRequest::GroupMemberRemove(request) => Outcome::data(CommandData::GroupRecord {
                action: "member removed".to_owned(),
                group: self
                    .registry
                    .remove_group_member(&request.group_id, &request.alias)?,
            }),
            CommandRequest::ViewCreate(request) => Outcome::data(CommandData::ViewRecord {
                action: "created".to_owned(),
                view: self.registry.create_view(request.view)?,
            }),
            CommandRequest::ViewList => Outcome::data(CommandData::ViewList {
                views: self.registry.list_views()?,
            }),
            CommandRequest::ViewShow(request) => Outcome::data(CommandData::ViewRecord {
                action: "shown".to_owned(),
                view: self.registry.get_view(&request.id)?,
            }),
            CommandRequest::ViewEvaluate(request) => {
                let view = self.registry.get_view(&request.id)?;
                let devices = self.registry.evaluate_view(&request.id)?;
                Outcome::data(CommandData::ViewEvaluation { view, devices })
            }
            CommandRequest::ViewDelete(request) => Outcome::data(CommandData::ViewRecord {
                action: "deleted".to_owned(),
                view: self.registry.delete_view(&request.id)?,
            }),
            CommandRequest::DiscoverScan(request) => {
                let discovered = oxvif::discovery::probe(options.timeout).await;
                let devices = discovered
                    .into_iter()
                    .map(|device| crate::DiscoveryRecord {
                        endpoint: device.endpoint,
                        types: device.types,
                        scopes: device.scopes,
                        xaddrs: device.xaddrs,
                        manufacturer: None,
                        model: None,
                        firmware_version: None,
                        serial_number: None,
                    })
                    .collect();
                Outcome::data(CommandData::DiscoverySnapshotRecord {
                    action: "saved".to_owned(),
                    snapshot: self
                        .registry
                        .save_discovery_snapshot(&request.snapshot_id, devices)?,
                })
            }
            CommandRequest::DiscoverySnapshotList => {
                Outcome::data(CommandData::DiscoverySnapshotList {
                    snapshots: self.registry.list_discovery_snapshots()?,
                })
            }
            CommandRequest::DiscoverySnapshotShow(request) => {
                Outcome::data(CommandData::DiscoverySnapshotRecord {
                    action: "shown".to_owned(),
                    snapshot: self
                        .registry
                        .get_discovery_snapshot(&request.id, &request.filters)?,
                })
            }
            CommandRequest::DiscoverySnapshotRemove(request) => {
                self.registry.remove_discovery_snapshot(&request.id)?;
                Outcome::data(CommandData::ResourceRemoved {
                    resource: "discovery_snapshot".to_owned(),
                    id: request.id,
                })
            }
            CommandRequest::Use(request) => {
                let id = self.registry.resolve_device_selector(&request.id)?;
                let device = self.registry.set_current(&id)?;
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
                let id = self.registry.resolve_device_selector(&request.id)?;
                let previous = self.registry.get(&id)?;
                let resolved = self.resolve_saved(&id)?;
                let information = self.fetch_device_info(&resolved, options).await?;
                let mut warnings = Vec::new();
                if let Some(warning) = identity_change_warning(
                    &id,
                    previous.serial_number.as_deref(),
                    &information.serial_number,
                ) {
                    warnings.push(warning);
                }
                let device = self.registry.update_metadata(
                    &id,
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
        let canonical_id = self.registry.resolve_device_selector(id)?;
        let stored = self.registry.get_stored(&canonical_id)?;
        let profile = stored
            .credential_profile()
            .map(|profile_id| self.registry.get_stored_credential_profile(profile_id))
            .transpose()?;
        let credential_ref = profile
            .as_ref()
            .map(|profile| profile.credential_ref())
            .or_else(|| stored.credential_ref());
        let password = match credential_ref {
            Some(reference) => self.credentials.get(reference)?.ok_or_else(|| {
                AppError::credential_unavailable(format!(
                    "Credential `{reference}` is referenced by `{canonical_id}` but does not exist."
                ))
            })?,
            None => env::var("OXVIF_PASSWORD").unwrap_or_default(),
        };
        let username = profile
            .as_ref()
            .map(|profile| profile.username().to_owned())
            .or_else(|| stored.username().map(str::to_owned))
            .or_else(|| env::var("OXVIF_USERNAME").ok());
        let password = if credential_ref.is_some() || env::var_os("OXVIF_PASSWORD").is_some() {
            Some(password)
        } else {
            None
        };
        if password.is_some() && username.is_none() {
            return Err(AppError::credential_unavailable(format!(
                "Device `{canonical_id}` has a password but no username."
            )));
        }
        Ok(ResolvedTarget {
            device_id: Some(canonical_id),
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

    #[tokio::test]
    async fn reusable_credential_profile_is_shared_and_cannot_be_deleted_while_in_use() {
        let directory = tempfile::tempdir().expect("temp directory");
        let registry = RegistryStore::at(directory.path());
        let credentials = Arc::new(MemoryCredentialStore::default());
        let application = Application::with_stores(registry.clone(), credentials.clone());
        let options = ExecutionOptions::default();
        for id in ["camera-a", "camera-b"] {
            application
                .execute(
                    CommandRequest::DeviceAdd(DeviceAddRequest {
                        device: NewDevice {
                            id: id.to_owned(),
                            name: None,
                            target: "192.0.2.20".to_owned(),
                            tags: Vec::new(),
                        },
                    }),
                    &options,
                )
                .await
                .expect("device should add");
        }
        application
            .execute(
                CommandRequest::CredentialProfileSet(crate::CredentialProfileSetRequest {
                    id: "factory-admin".to_owned(),
                    username: "admin".to_owned(),
                    password: SecretString::new("shared-secret").expect("secret should construct"),
                }),
                &options,
            )
            .await
            .expect("profile should set");
        for id in ["camera-a", "camera-b"] {
            application
                .execute(
                    CommandRequest::DeviceCredentialUseProfile(
                        crate::DeviceCredentialProfileRequest {
                            device_id: id.to_owned(),
                            profile_id: "factory-admin".to_owned(),
                        },
                    ),
                    &options,
                )
                .await
                .expect("profile should assign");
        }

        assert_eq!(
            credentials
                .get("profile/factory-admin")
                .expect("credential should load")
                .as_deref(),
            Some("shared-secret")
        );
        assert_eq!(
            registry
                .get("camera-a")
                .expect("device should load")
                .credential_profile
                .as_deref(),
            Some("factory-admin")
        );
        let error = application
            .execute(
                CommandRequest::CredentialProfileDelete(crate::ResourceIdRequest {
                    id: "factory-admin".to_owned(),
                }),
                &options,
            )
            .await
            .expect_err("in-use profile must not delete");
        assert_eq!(error.code, crate::ErrorCode::ResourceInUse);
        let registry_contents = std::fs::read_to_string(directory.path().join("devices.toml"))
            .expect("registry should exist");
        assert!(!registry_contents.contains("shared-secret"));
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
