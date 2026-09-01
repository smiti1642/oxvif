use std::{collections::BTreeMap, env, future::Future, net::Ipv4Addr, sync::Arc, time::Duration};

use oxvif::{
    DeviceInfo, OnvifClient, OnvifError, OnvifSession,
    health::{ErrorClass, HealthCheck, HealthReport},
    transport::{HttpTransport, Transport, TransportError},
};
use tokio::time::{Instant, sleep, timeout};

use crate::{
    AppError, CommandData, CommandRequest, CommandSuccess, CredentialStore, DeviceMetadata,
    LiveDeviceInfo, RegistryStore, ResultMeta, SecretString, SystemCredentialStore, Warning,
    credential_profile_reference, credential_reference, describe, normalize_target,
};

/// Client-side WS-Security timestamp synchronization policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ClockSyncPolicy {
    /// Synchronize only when credentials are present.
    #[default]
    Auto,
    /// Always read device time before the authenticated session handshake.
    Always,
    /// Never read device time as part of session setup.
    Never,
}

/// Invocation policy shared by CLI and future non-CLI adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionOptions {
    pub non_interactive: bool,
    pub timeout: Duration,
    pub retries: u32,
    pub clock_sync: ClockSyncPolicy,
    pub ca_certificates: Vec<Vec<u8>>,
    pub verbosity: u8,
    pub quiet: bool,
    pub jobs: usize,
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            non_interactive: false,
            timeout: Duration::from_secs(10),
            retries: 0,
            clock_sync: ClockSyncPolicy::Auto,
            ca_certificates: Vec::new(),
            verbosity: 0,
            quiet: false,
            jobs: 16,
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

    /// Validate setup inputs and secret-slot availability before a CLI prompts.
    pub fn preflight_setup(&self, device: &crate::NewDevice) -> Result<(), AppError> {
        self.registry.validate_new(device)?;
        let reference = credential_reference(&device.id);
        if self.credentials.get(&reference)?.is_some() {
            return Err(AppError::resource_exists(
                "native device credential",
                &device.id,
            ));
        }
        Ok(())
    }

    pub async fn execute(
        &self,
        request: CommandRequest,
        options: &ExecutionOptions,
    ) -> Result<CommandSuccess, AppError> {
        let started = Instant::now();
        let command_name = request.name();
        let outcome = match request {
            CommandRequest::AgentGuide => Outcome::data(CommandData::AgentGuide {
                guide: crate::agent::guide(),
            }),
            CommandRequest::AgentPrompt => Outcome::data(CommandData::AgentPrompt {
                prompt: crate::agent::prompt(),
            }),
            CommandRequest::Describe(request) => Outcome::data(describe::execute(request)?),
            CommandRequest::ConfigPath => Outcome::data(CommandData::ConfigStatus {
                config_dir: self.registry.config_dir().display().to_string(),
                registry_file: self.registry.registry_path().display().to_string(),
                snapshots_dir: self.registry.snapshots_dir().display().to_string(),
                validated: false,
                device_count: None,
                snapshot_count: None,
                orphaned_snapshot_files: Vec::new(),
            }),
            CommandRequest::ConfigValidate => {
                let (devices, _) = self.registry.list()?;
                let snapshots = self.registry.list_discovery_snapshots()?;
                for snapshot in &snapshots {
                    self.registry.get_discovery_snapshot(&snapshot.id, &[])?;
                }
                let orphaned_snapshot_files = self
                    .registry
                    .orphaned_snapshot_files()?
                    .into_iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>();
                let mut outcome = Outcome::data(CommandData::ConfigStatus {
                    config_dir: self.registry.config_dir().display().to_string(),
                    registry_file: self.registry.registry_path().display().to_string(),
                    snapshots_dir: self.registry.snapshots_dir().display().to_string(),
                    validated: true,
                    device_count: Some(devices.len()),
                    snapshot_count: Some(snapshots.len()),
                    orphaned_snapshot_files: orphaned_snapshot_files.clone(),
                });
                outcome.warnings = orphaned_snapshot_files
                    .into_iter()
                    .map(|path| Warning {
                        code: "ORPHANED_SNAPSHOT_FILE".to_owned(),
                        message: format!(
                            "Snapshot file `{path}` is not indexed; it was reported but not deleted."
                        ),
                    })
                    .collect();
                outcome
            }
            CommandRequest::DeviceSetup(request) => {
                self.preflight_setup(&request.device)?;
                let verified = request.verify;
                let set_current = request.set_current;
                let reference = credential_reference(&request.device.id);
                let target = normalize_target(&request.device.target)?;
                let information = if verified {
                    Some(
                        fetch_live_information(
                            &target,
                            Some(&request.username),
                            Some(request.password.expose_secret()),
                            options,
                        )
                        .await?,
                    )
                } else {
                    None
                };

                self.credentials
                    .set(&reference, request.password.expose_secret())?;
                let id = request.device.id.clone();
                if let Err(error) = self.registry.add(request.device) {
                    return Err(self.rollback_setup(&id, &reference, false, error));
                }
                let mut device =
                    match self
                        .registry
                        .set_credentials(&id, &request.username, &reference)
                    {
                        Ok(device) => device,
                        Err(error) => {
                            return Err(self.rollback_setup(&id, &reference, true, error));
                        }
                    };
                if let Some(information) = information {
                    device = match self.registry.update_metadata(
                        &id,
                        DeviceMetadata {
                            manufacturer: information.manufacturer,
                            model: information.model,
                            firmware_version: information.firmware_version,
                            serial_number: information.serial_number,
                        },
                    ) {
                        Ok(device) => device,
                        Err(error) => {
                            return Err(self.rollback_setup(&id, &reference, true, error));
                        }
                    };
                }
                if set_current {
                    device = match self.registry.set_current(&id) {
                        Ok(device) => device,
                        Err(error) => {
                            return Err(self.rollback_setup(&id, &reference, true, error));
                        }
                    };
                }
                Outcome {
                    data: CommandData::DeviceSetup {
                        device: device.clone(),
                        verified,
                        current: set_current,
                    },
                    warnings: Vec::new(),
                    device_id: Some(device.id),
                    selected_by: None,
                    target: Some(device.target),
                }
            }
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
                Outcome::device_selected("shown", device, request.id)
            }
            CommandRequest::DeviceUpdate(request) => {
                let id = self.registry.resolve_device_selector(&request.id)?;
                let device = self.registry.update(&id, request.update)?;
                Outcome::device_selected("updated", device, request.id)
            }
            CommandRequest::DeviceRename(request) => {
                let id = self.registry.resolve_device_selector(&request.id)?;
                let device = self.registry.rename(&id, &request.name)?;
                Outcome::device_selected("renamed", device, request.id)
            }
            CommandRequest::DeviceRemove(request) => self.remove_device(&request.id)?,
            CommandRequest::DeviceImport(request) => {
                if let Some(profile_id) = request.credential_profile.as_deref() {
                    let profile = self.registry.get_stored_credential_profile(profile_id)?;
                    if self.credentials.get(profile.credential_ref())?.is_none() {
                        return Err(AppError::credential_unavailable(format!(
                            "Credential profile `{profile_id}` has no native secret."
                        )));
                    }
                }
                match request.mode {
                    crate::ImportMode::Plan => {
                        let plan = self.registry.plan_discovery_import(&request)?;
                        Outcome::data(CommandData::DeviceImport {
                            applied: false,
                            plan,
                            devices: Vec::new(),
                        })
                    }
                    crate::ImportMode::Apply => {
                        let expected =
                            request.expected_fingerprint.as_deref().ok_or_else(|| {
                                AppError::invalid_argument(
                                    "`device import --apply` requires an expected plan fingerprint.",
                                )
                            })?;
                        let (plan, devices) =
                            self.registry.apply_discovery_import(&request, expected)?;
                        Outcome::data(CommandData::DeviceImport {
                            applied: true,
                            plan,
                            devices,
                        })
                    }
                }
            }
            CommandRequest::DeviceCredentialSet(request) => {
                let id = self.registry.resolve_device_selector(&request.id)?;
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
                    selected_by: Some(request.id),
                    target: Some(device.target),
                }
            }
            CommandRequest::DeviceCredentialDelete(request) => {
                let selected_by = request.id;
                let id = self.registry.resolve_device_selector(&selected_by)?;
                let stored = self.registry.get_stored(&id)?;
                if let Some(reference) = stored.credential_ref() {
                    self.credentials.delete(reference)?;
                }
                let device = self.registry.clear_credentials(&id)?;
                Outcome {
                    data: CommandData::CredentialUpdated {
                        action: "deleted".to_owned(),
                        device: device.clone(),
                    },
                    warnings: Vec::new(),
                    device_id: Some(device.id),
                    selected_by: Some(selected_by),
                    target: Some(device.target),
                }
            }
            CommandRequest::DeviceCredentialUseProfile(request) => {
                let id = self.registry.resolve_device_selector(&request.device_id)?;
                let device = self
                    .registry
                    .assign_credential_profile(&id, &request.profile_id)?;
                Outcome::device_selected("credential profile assigned", device, request.device_id)
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
                                let _ = self.credentials.set(&reference, secret.expose_secret());
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
                let (devices, explanation) = self.registry.evaluate_view_explained(&request.id)?;
                Outcome::data(CommandData::ViewEvaluation {
                    view,
                    devices,
                    explanation: request.explain.then_some(explanation),
                })
            }
            CommandRequest::ViewDelete(request) => Outcome::data(CommandData::ViewRecord {
                action: "deleted".to_owned(),
                view: self.registry.delete_view(&request.id)?,
            }),
            CommandRequest::DiscoverScan(request) => {
                let (devices, selected_interfaces, warnings) = scan_discovery_interfaces(
                    &request.interfaces,
                    options.timeout,
                    options.retries,
                )
                .await?;
                let saved_snapshot = request
                    .snapshot_id
                    .as_deref()
                    .map(|id| {
                        self.registry.save_discovery_snapshot_with_interfaces(
                            id,
                            devices.clone(),
                            selected_interfaces.clone(),
                        )
                    })
                    .transpose()?
                    .map(snapshot_summary);
                let registrations = self.discovery_registrations(&devices)?;
                let mut outcome = Outcome::data(CommandData::DiscoveryScan {
                    devices,
                    saved_snapshot,
                    interfaces: selected_interfaces,
                    registrations,
                });
                outcome.warnings = warnings;
                outcome
            }
            CommandRequest::DiscoveryRefresh(request) => {
                let (devices, selected_interfaces, warnings) = scan_discovery_interfaces(
                    &request.interfaces,
                    options.timeout,
                    options.retries,
                )
                .await?;
                let snapshot = self.registry.refresh_discovery_snapshot(
                    &request.id,
                    devices.clone(),
                    selected_interfaces.clone(),
                )?;
                let registrations = self.discovery_registrations(&devices)?;
                let mut outcome = Outcome::data(CommandData::DiscoveryScan {
                    devices,
                    saved_snapshot: Some(snapshot_summary(snapshot)),
                    interfaces: selected_interfaces,
                    registrations,
                });
                outcome.warnings = warnings;
                outcome
            }
            CommandRequest::DiscoveryEnrich(request) => {
                let profile = self
                    .registry
                    .get_stored_credential_profile(&request.credential_profile)?;
                let password =
                    self.credentials
                        .get(profile.credential_ref())?
                        .ok_or_else(|| {
                            AppError::credential_unavailable(format!(
                                "Credential profile `{}` has no native secret.",
                                request.credential_profile
                            ))
                        })?;
                let snapshot = self.registry.get_discovery_snapshot(&request.id, &[])?;
                if snapshot.devices.is_empty() {
                    return Err(AppError::discovery_failed(format!(
                        "Discovery snapshot `{}` is empty.",
                        request.id
                    )));
                }
                let selected = self
                    .registry
                    .get_discovery_snapshot(&request.id, &request.filters)?
                    .devices
                    .into_iter()
                    .map(|device| device.endpoint)
                    .collect::<std::collections::BTreeSet<_>>();
                if selected.is_empty() {
                    return Err(AppError::invalid_argument(
                        "No discovery records match the enrichment filters.",
                    ));
                }
                let attempted = selected.len();
                let mut devices = snapshot.devices;
                let mut candidates = Vec::new();
                let mut warnings = Vec::new();
                for (index, device) in devices.iter().enumerate() {
                    if !selected.contains(&device.endpoint) {
                        continue;
                    }
                    let target = device
                        .xaddrs
                        .iter()
                        .find_map(|target| normalize_target(target).ok());
                    if let Some(target) = target {
                        candidates.push((index, device.endpoint.clone(), target));
                    } else {
                        warnings.push(Warning {
                            code: "DISCOVERY_ENRICH_NO_TARGET".to_owned(),
                            message: format!(
                                "Discovery endpoint `{}` has no valid ONVIF XAddr.",
                                device.endpoint
                            ),
                        });
                    }
                }

                let mut queue = candidates.into_iter();
                let mut tasks = tokio::task::JoinSet::new();
                for _ in 0..request.jobs {
                    let Some((index, endpoint, target)) = queue.next() else {
                        break;
                    };
                    let username = profile.username().to_owned();
                    let password = password.clone();
                    let options = options.clone();
                    tasks.spawn(async move {
                        let result = fetch_live_information(
                            &target,
                            Some(&username),
                            Some(password.expose_secret()),
                            &options,
                        )
                        .await;
                        (index, endpoint, result)
                    });
                }

                let mut enriched = 0usize;
                while let Some(joined) = tasks.join_next().await {
                    match joined {
                        Ok((index, _, Ok(information))) => {
                            let device = &mut devices[index];
                            device.manufacturer = Some(information.manufacturer);
                            device.model = Some(information.model);
                            device.firmware_version = Some(information.firmware_version);
                            device.serial_number = Some(information.serial_number);
                            enriched += 1;
                        }
                        Ok((_, endpoint, Err(error))) => warnings.push(Warning {
                            code: "DISCOVERY_ENRICH_FAILED".to_owned(),
                            message: format!("Failed to enrich `{endpoint}`: {}", error.message),
                        }),
                        Err(error) => warnings.push(Warning {
                            code: "DISCOVERY_ENRICH_FAILED".to_owned(),
                            message: format!("An enrichment task failed: {error}"),
                        }),
                    }
                    if let Some((index, endpoint, target)) = queue.next() {
                        let username = profile.username().to_owned();
                        let password = password.clone();
                        let options = options.clone();
                        tasks.spawn(async move {
                            let result = fetch_live_information(
                                &target,
                                Some(&username),
                                Some(password.expose_secret()),
                                &options,
                            )
                            .await;
                            (index, endpoint, result)
                        });
                    }
                }
                if enriched == 0 {
                    return Err(AppError::discovery_failed(format!(
                        "Enrichment failed for every record in snapshot `{}`.",
                        request.id
                    )));
                }
                let failed = attempted - enriched;
                let snapshot = self
                    .registry
                    .replace_discovery_snapshot(&request.id, devices)?;
                let mut outcome = Outcome::data(CommandData::DiscoveryEnrichment {
                    snapshot: snapshot_summary(snapshot),
                    attempted,
                    enriched,
                    failed,
                });
                outcome.warnings = warnings;
                outcome
            }
            CommandRequest::DiscoverySnapshotList => {
                Outcome::data(CommandData::DiscoverySnapshotList {
                    snapshots: self.registry.list_discovery_snapshots()?,
                })
            }
            CommandRequest::DiscoverySnapshotShow(request) => {
                let snapshot = self
                    .registry
                    .get_discovery_snapshot(&request.id, &request.filters)?;
                let registrations = self.discovery_registrations(&snapshot.devices)?;
                Outcome::data(CommandData::DiscoverySnapshotRecord {
                    action: "shown".to_owned(),
                    snapshot,
                    registrations,
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
                Outcome::device_selected("selected", device, request.id)
            }
            CommandRequest::Current => {
                let device = self.registry.current()?;
                Outcome {
                    device_id: device.as_ref().map(|device| device.id.clone()),
                    selected_by: None,
                    target: device.as_ref().map(|device| device.target.clone()),
                    data: CommandData::CurrentDevice { device },
                    warnings: Vec::new(),
                }
            }
            CommandRequest::DeviceTest(request) => {
                if request.selector.group.is_some() || request.selector.view.is_some() {
                    self.device_diagnostic(
                        request.selector,
                        DiagnosticOperation::DeviceTest,
                        options,
                    )
                    .await?
                } else {
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
                        selected_by: resolved.selected_by,
                        target: Some(resolved.target),
                    }
                }
            }
            CommandRequest::DeviceInfo(request) => {
                if request.selector.group.is_some() || request.selector.view.is_some() {
                    self.device_diagnostic(
                        request.selector,
                        DiagnosticOperation::DeviceInformation,
                        options,
                    )
                    .await?
                } else {
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
                        selected_by: resolved.selected_by,
                        target: Some(resolved.target),
                    }
                }
            }
            CommandRequest::DeviceCapabilities(request) => {
                self.device_diagnostic(request.selector, DiagnosticOperation::Capabilities, options)
                    .await?
            }
            CommandRequest::DeviceServices(request) => {
                self.device_diagnostic(request.selector, DiagnosticOperation::Services, options)
                    .await?
            }
            CommandRequest::MediaProfiles(request) => {
                self.device_diagnostic(
                    request.selector,
                    DiagnosticOperation::MediaProfiles,
                    options,
                )
                .await?
            }
            CommandRequest::MediaStreamUri(request) => {
                self.device_diagnostic(
                    request.selector,
                    DiagnosticOperation::MediaStreamUri(request.profile),
                    options,
                )
                .await?
            }
            CommandRequest::MediaSnapshotUri(request) => {
                self.device_diagnostic(
                    request.selector,
                    DiagnosticOperation::MediaSnapshotUri(request.profile),
                    options,
                )
                .await?
            }
            CommandRequest::PtzStatus(request) => {
                self.device_diagnostic(
                    request.selector,
                    DiagnosticOperation::PtzStatus(request.profile),
                    options,
                )
                .await?
            }
            CommandRequest::PtzPresets(request) => {
                self.device_diagnostic(
                    request.selector,
                    DiagnosticOperation::PtzPresets(request.profile),
                    options,
                )
                .await?
            }
            CommandRequest::HealthCheck(request) => {
                self.device_diagnostic(request.selector, DiagnosticOperation::Health, options)
                    .await?
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
                outcome.selected_by = Some(request.id);
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
                selected_by: outcome.selected_by,
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
            selected_by: None,
            target: Some(stored.target().to_owned()),
        })
    }

    fn rollback_setup(
        &self,
        id: &str,
        reference: &str,
        remove_device: bool,
        mut original: AppError,
    ) -> AppError {
        let mut cleanup_failures = Vec::new();
        if remove_device && let Err(error) = self.registry.remove(id) {
            cleanup_failures.push(format!("registry cleanup failed: {}", error.message));
        }
        if let Err(error) = self.credentials.delete(reference) {
            cleanup_failures.push(format!("credential cleanup failed: {}", error.message));
        }
        if cleanup_failures.is_empty() {
            original.message.push_str(" Setup rollback completed.");
        } else {
            original.message.push_str(&format!(
                " Setup rollback was incomplete: {}",
                cleanup_failures.join("; ")
            ));
            original.suggested_action = Some(format!(
                "Inspect `oxvif device show {id}` and remove any local device or native credential left by setup."
            ));
        }
        original
    }

    fn discovery_registrations(
        &self,
        records: &[crate::DiscoveryRecord],
    ) -> Result<BTreeMap<String, String>, AppError> {
        let (devices, _) = self.registry.list()?;
        let mut registrations = BTreeMap::new();
        for record in records {
            let matching = devices.iter().find(|device| {
                device.device_uuid.as_deref().is_some_and(|uuid| {
                    record.endpoint.eq_ignore_ascii_case(uuid)
                        || record
                            .endpoint
                            .strip_prefix("urn:uuid:")
                            .is_some_and(|endpoint| endpoint.eq_ignore_ascii_case(uuid))
                }) || record.xaddrs.iter().any(|xaddr| {
                    normalize_target(xaddr).is_ok_and(|target| target == device.target)
                })
            });
            if let Some(device) = matching {
                registrations.insert(record.endpoint.clone(), device.id.clone());
            }
        }
        Ok(registrations)
    }

    fn resolve_target(&self, selector: crate::TargetSelector) -> Result<ResolvedTarget, AppError> {
        if selector.group.is_some() || selector.view.is_some() {
            return Err(AppError::invalid_argument(
                "Group/View selection requires a fleet-capable diagnostic command.",
            ));
        }
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
            Some(reference) => Some(self.credentials.get(reference)?.ok_or_else(|| {
                AppError::credential_unavailable(format!(
                    "Credential `{reference}` is referenced by `{canonical_id}` but does not exist."
                ))
            })?),
            None => env::var("OXVIF_PASSWORD")
                .ok()
                .map(SecretString::new)
                .transpose()?,
        };
        let username = profile
            .as_ref()
            .map(|profile| profile.username().to_owned())
            .or_else(|| stored.username().map(str::to_owned))
            .or_else(|| env::var("OXVIF_USERNAME").ok());
        if password.is_some() && username.is_none() {
            return Err(AppError::credential_unavailable(format!(
                "Device `{canonical_id}` has a password but no username."
            )));
        }
        Ok(ResolvedTarget {
            selected_by: Some(id.to_owned()),
            device_id: Some(canonical_id),
            target: stored.target().to_owned(),
            username,
            password,
        })
    }

    fn resolve_direct(&self, target: &str) -> Result<ResolvedTarget, AppError> {
        let username = env::var("OXVIF_USERNAME").ok();
        let password = env::var("OXVIF_PASSWORD")
            .ok()
            .map(SecretString::new)
            .transpose()?;
        if password.is_some() && username.is_none() {
            return Err(AppError::credential_unavailable(
                "OXVIF_PASSWORD is set but OXVIF_USERNAME is missing.",
            ));
        }
        Ok(ResolvedTarget {
            device_id: None,
            selected_by: None,
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
        fetch_live_information(
            &resolved.target,
            resolved.username.as_deref(),
            resolved.password(),
            options,
        )
        .await
    }

    async fn device_diagnostic(
        &self,
        selector: crate::TargetSelector,
        operation: DiagnosticOperation,
        options: &ExecutionOptions,
    ) -> Result<Outcome, AppError> {
        if selector.group.is_some() || selector.view.is_some() {
            return self.fleet_diagnostic(selector, operation, options).await;
        }
        let resolved = self.resolve_target(selector)?;
        let operation_name = operation.name().to_owned();
        let result = execute_diagnostic(&resolved, operation, options).await?;
        Ok(Outcome {
            data: CommandData::DeviceDiagnostic {
                operation: operation_name,
                device_id: resolved.device_id.clone(),
                target: resolved.target.clone(),
                result,
            },
            warnings: Vec::new(),
            device_id: resolved.device_id,
            selected_by: resolved.selected_by,
            target: Some(resolved.target),
        })
    }

    async fn fleet_diagnostic(
        &self,
        selector: crate::TargetSelector,
        operation: DiagnosticOperation,
        options: &ExecutionOptions,
    ) -> Result<Outcome, AppError> {
        if selector.device.is_some() || selector.target.is_some() {
            return Err(AppError::invalid_argument(
                "A fleet selector cannot be combined with --device or --target.",
            ));
        }
        let (selection_kind, selection_id, selected) = match (selector.group, selector.view) {
            (Some(_), Some(_)) => {
                return Err(AppError::invalid_argument(
                    "--group and --view are mutually exclusive.",
                ));
            }
            (Some(group_id), None) => {
                let group = self.registry.get_group(&group_id)?;
                let selected = group
                    .members
                    .into_iter()
                    .map(|member| (format!("{group_id}/{}", member.alias), member.device_id))
                    .collect::<Vec<_>>();
                ("group".to_owned(), group_id, selected)
            }
            (None, Some(view_id)) => {
                let selected = self
                    .registry
                    .evaluate_view(&view_id)?
                    .into_iter()
                    .map(|device| (format!("view:{view_id}/{}", device.id), device.id))
                    .collect::<Vec<_>>();
                ("view".to_owned(), view_id, selected)
            }
            (None, None) => return Err(AppError::missing_target()),
        };
        if selected.is_empty() {
            return Err(AppError::invalid_argument(format!(
                "Selected {selection_kind} `{selection_id}` contains no devices."
            )));
        }

        let operation_name = operation.name().to_owned();
        let mut items = Vec::with_capacity(selected.len());
        let mut queue = Vec::new();
        for (selected_by, device_id) in selected {
            let target = self
                .registry
                .get(&device_id)
                .map(|device| device.target)
                .unwrap_or_default();
            match self.resolve_saved(&device_id) {
                Ok(mut resolved) => {
                    resolved.selected_by = Some(selected_by.clone());
                    queue.push((selected_by, resolved));
                }
                Err(error) => items.push(crate::FleetDiagnosticItem {
                    device_id,
                    selected_by,
                    target,
                    ok: false,
                    result: None,
                    error: Some(fleet_item_error(error)),
                    elapsed_ms: 0,
                }),
            }
        }

        let mut queue = queue.into_iter();
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..options.jobs {
            let Some((selected_by, resolved)) = queue.next() else {
                break;
            };
            spawn_diagnostic_task(
                &mut tasks,
                selected_by,
                resolved,
                operation.clone(),
                options,
            );
        }
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok(item) => items.push(item),
                Err(error) => {
                    return Err(AppError::internal(format!("Fleet task failed: {error}")));
                }
            }
            if let Some((selected_by, resolved)) = queue.next() {
                spawn_diagnostic_task(
                    &mut tasks,
                    selected_by,
                    resolved,
                    operation.clone(),
                    options,
                );
            }
        }
        items.sort_by(|left, right| {
            left.device_id
                .cmp(&right.device_id)
                .then_with(|| left.selected_by.cmp(&right.selected_by))
        });
        let succeeded = items.iter().filter(|item| item.ok).count();
        let failed = items.len() - succeeded;
        if succeeded == 0 {
            return Err(AppError::fleet_failed(format!(
                "{operation_name} failed for all {failed} device(s) selected by {selection_kind} `{selection_id}`."
            )));
        }
        Ok(Outcome::data(CommandData::FleetDiagnostic {
            operation: operation_name,
            selection_kind,
            selection_id,
            total: items.len(),
            succeeded,
            failed,
            items,
        }))
    }
}

#[derive(Clone)]
enum DiagnosticOperation {
    DeviceTest,
    DeviceInformation,
    Capabilities,
    Services,
    MediaProfiles,
    MediaStreamUri(String),
    MediaSnapshotUri(String),
    PtzStatus(String),
    PtzPresets(String),
    Health,
}

impl DiagnosticOperation {
    fn name(&self) -> &'static str {
        match self {
            Self::DeviceTest => "device.test",
            Self::DeviceInformation => "device.info",
            Self::Capabilities => "device.capabilities",
            Self::Services => "device.services",
            Self::MediaProfiles => "media.profiles",
            Self::MediaStreamUri(_) => "media.stream-uri",
            Self::MediaSnapshotUri(_) => "media.snapshot-uri",
            Self::PtzStatus(_) => "ptz.status",
            Self::PtzPresets(_) => "ptz.presets",
            Self::Health => "health.check",
        }
    }
}

enum DiagnosticAttemptFailure {
    Onvif(OnvifError),
    Serialization(serde_json::Error),
}

impl DiagnosticAttemptFailure {
    fn is_retryable(&self) -> bool {
        match self {
            Self::Onvif(error) => is_retryable_onvif_error(error),
            Self::Serialization(_) => false,
        }
    }
}

impl std::fmt::Display for DiagnosticAttemptFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Onvif(error) => error.fmt(formatter),
            Self::Serialization(error) => error.fmt(formatter),
        }
    }
}

impl From<OnvifError> for DiagnosticAttemptFailure {
    fn from(value: OnvifError) -> Self {
        Self::Onvif(value)
    }
}

impl From<serde_json::Error> for DiagnosticAttemptFailure {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value)
    }
}

fn is_retryable_onvif_error(error: &OnvifError) -> bool {
    match error {
        OnvifError::Transport(TransportError::Http(error)) => {
            error.is_timeout() || error.is_connect() || error.is_body() || error.is_request()
        }
        OnvifError::Transport(TransportError::HttpStatus { status, .. }) => {
            matches!(*status, 408 | 425 | 429 | 502 | 503 | 504)
        }
        OnvifError::Soap(_) | OnvifError::InvalidArgument(_) => false,
    }
}

fn retry_delay(attempt: u32, discriminator: &str) -> Duration {
    let exponent = attempt.min(4);
    let base_ms = 100_u64.saturating_mul(1_u64 << exponent);
    let hash = discriminator.bytes().fold(0_u64, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(u64::from(byte))
    });
    let jitter_ms = hash.wrapping_add(u64::from(attempt)) % 51;
    Duration::from_millis((base_ms + jitter_ms).min(2_000))
}

fn spawn_diagnostic_task(
    tasks: &mut tokio::task::JoinSet<crate::FleetDiagnosticItem>,
    selected_by: String,
    resolved: ResolvedTarget,
    operation: DiagnosticOperation,
    options: &ExecutionOptions,
) {
    let options = options.clone();
    tasks.spawn(async move {
        let started = Instant::now();
        let device_id = resolved
            .device_id
            .clone()
            .unwrap_or_else(|| "(unknown)".to_owned());
        let target = resolved.target.clone();
        match execute_diagnostic(&resolved, operation, &options).await {
            Ok(result) => crate::FleetDiagnosticItem {
                device_id,
                selected_by,
                target,
                ok: true,
                result: Some(result),
                error: None,
                elapsed_ms: elapsed_millis(started),
            },
            Err(error) => crate::FleetDiagnosticItem {
                device_id,
                selected_by,
                target,
                ok: false,
                result: None,
                error: Some(fleet_item_error(error)),
                elapsed_ms: elapsed_millis(started),
            },
        }
    });
}

fn fleet_item_error(error: AppError) -> crate::FleetItemError {
    crate::FleetItemError {
        code: error.code.as_str().to_owned(),
        message: error.message,
        retryable: error.retryable,
    }
}

async fn execute_diagnostic(
    resolved: &ResolvedTarget,
    operation: DiagnosticOperation,
    options: &ExecutionOptions,
) -> Result<serde_json::Value, AppError> {
    if matches!(&operation, DiagnosticOperation::Health) {
        return execute_health_check(resolved, options).await;
    }
    let transport =
        build_http_transport(options, resolved.username.as_deref(), resolved.password())?;
    let attempts = options.retries.saturating_add(1);
    let mut last_error = None;
    let mut last_retryable = true;
    for attempt in 0..attempts {
        let future = async {
            let mut builder = OnvifSession::builder(&resolved.target);
            if let (Some(username), Some(password)) =
                (resolved.username.as_deref(), resolved.password())
            {
                builder = builder.with_credentials(username, password);
            }
            builder = builder.with_transport(transport.clone());
            if should_sync_clock(options.clock_sync, resolved.password.is_some()) {
                builder = builder.with_clock_sync();
            }
            let session = builder.build().await?;
            let mut value = match &operation {
                DiagnosticOperation::DeviceTest => {
                    let information = session.get_device_info().await?;
                    serde_json::json!({
                        "authenticated": resolved.password.is_some(),
                        "information": information,
                    })
                }
                DiagnosticOperation::DeviceInformation => {
                    serde_json::to_value(session.get_device_info().await?)?
                }
                DiagnosticOperation::Capabilities => serde_json::to_value(session.capabilities())?,
                DiagnosticOperation::Services => {
                    serde_json::to_value(session.get_services().await?)?
                }
                DiagnosticOperation::MediaProfiles => {
                    serde_json::to_value(session.get_profiles().await?)?
                }
                DiagnosticOperation::MediaStreamUri(profile) => {
                    serde_json::to_value(session.get_stream_uri(profile).await?)?
                }
                DiagnosticOperation::MediaSnapshotUri(profile) => {
                    serde_json::to_value(session.get_snapshot_uri(profile).await?)?
                }
                DiagnosticOperation::PtzStatus(profile) => {
                    serde_json::to_value(session.ptz_get_status(profile).await?)?
                }
                DiagnosticOperation::PtzPresets(profile) => {
                    serde_json::to_value(session.ptz_get_presets(profile).await?)?
                }
                DiagnosticOperation::Health => unreachable!("health is handled above"),
            };
            sanitize_uri_values(&mut value);
            Ok::<_, DiagnosticAttemptFailure>(value)
        };
        match timeout(options.timeout, future).await {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(error)) => {
                last_retryable = error.is_retryable();
                last_error = Some(error.to_string());
            }
            Err(_) => {
                last_retryable = true;
                last_error = Some(format!(
                    "Diagnostic exceeded the {} ms timeout.",
                    options.timeout.as_millis()
                ));
            }
        }
        if !last_retryable {
            break;
        }
        if attempt + 1 < attempts {
            sleep(retry_delay(attempt, &resolved.target)).await;
        }
    }
    Err(AppError::device_operation_failed(
        last_error.unwrap_or_else(|| "Device diagnostic failed without an error.".to_owned()),
        last_retryable,
    ))
}

async fn execute_health_check(
    resolved: &ResolvedTarget,
    options: &ExecutionOptions,
) -> Result<serde_json::Value, AppError> {
    let transport =
        build_http_transport(options, resolved.username.as_deref(), resolved.password())?;
    let attempts = options.retries.saturating_add(1);
    for attempt in 0..attempts {
        let mut check = HealthCheck::new(&resolved.target);
        if let (Some(username), Some(password)) =
            (resolved.username.as_deref(), resolved.password())
        {
            check = check.with_credentials(username, password);
        }
        check = check.with_transport(transport.clone());
        check = check.with_clock_sync(should_sync_clock(
            options.clock_sync,
            resolved.password.is_some(),
        ));
        match timeout(options.timeout, check.run()).await {
            Ok(report) if health_report_is_retryable(&report) && attempt + 1 < attempts => {
                sleep(retry_delay(attempt, &resolved.target)).await;
            }
            Ok(report) => return health_report_value(report),
            Err(_) if attempt + 1 < attempts => {
                sleep(retry_delay(attempt, &resolved.target)).await;
            }
            Err(_) => {
                return Err(AppError::device_connection_failed(format!(
                    "Health check exceeded the {} ms per-attempt timeout after {} attempt(s).",
                    options.timeout.as_millis(),
                    attempts
                )));
            }
        }
    }
    Err(AppError::internal(
        "Health retry policy completed without a result.",
    ))
}

fn health_report_is_retryable(report: &HealthReport) -> bool {
    let failures = report
        .checks
        .iter()
        .filter(|check| matches!(check.status, oxvif::health::CheckStatus::Fail(_)))
        .collect::<Vec<_>>();
    !failures.is_empty()
        && failures.iter().all(|check| {
            check
                .error
                .as_ref()
                .is_some_and(|error| error.class == ErrorClass::Http && !error.is_auth())
        })
}

fn health_report_value(report: HealthReport) -> Result<serde_json::Value, AppError> {
    let passed = report
        .checks
        .iter()
        .filter(|check| matches!(check.status, oxvif::health::CheckStatus::Pass))
        .count();
    let warned = report
        .checks
        .iter()
        .filter(|check| matches!(check.status, oxvif::health::CheckStatus::Warn(_)))
        .count();
    let failed = report
        .checks
        .iter()
        .filter(|check| matches!(check.status, oxvif::health::CheckStatus::Fail(_)))
        .count();
    let skipped = report.checks.len() - passed - warned - failed;
    let mut value = serde_json::json!({
        "healthy": report.ok(),
        "summary": {
            "passed": passed,
            "warned": warned,
            "failed": failed,
            "skipped": skipped,
        },
        "report": report,
    });
    sanitize_uri_values(&mut value);
    Ok(value)
}

fn sanitize_uri_values(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                sanitize_uri_values(item);
            }
        }
        serde_json::Value::Object(fields) => {
            for value in fields.values_mut() {
                sanitize_uri_values(value);
            }
        }
        serde_json::Value::String(text) => {
            if let Ok(mut uri) = url::Url::parse(text)
                && (!uri.username().is_empty() || uri.password().is_some())
            {
                let _ = uri.set_username("");
                let _ = uri.set_password(None);
                *text = uri.to_string();
            }
        }
        _ => {}
    }
}

async fn fetch_live_information(
    target: &str,
    username: Option<&str>,
    password: Option<&str>,
    options: &ExecutionOptions,
) -> Result<LiveDeviceInfo, AppError> {
    let transport = build_http_transport(options, username, password)?;
    let attempts = options.retries.saturating_add(1);
    let mut last_error = None;
    let mut last_retryable = true;
    for attempt in 0..attempts {
        let mut client = OnvifClient::new(target);
        if let (Some(username), Some(password)) = (username, password) {
            client = client.with_credentials(username, password);
        }
        client = client.with_transport(transport.clone());
        let request = async {
            if should_sync_clock(options.clock_sync, password.is_some()) {
                let device_time = client.get_system_date_and_time().await?;
                client = client.with_utc_offset(device_time.utc_offset_secs());
            }
            client.get_device_info().await
        };
        match timeout(options.timeout, request).await {
            Ok(Ok(information)) => return Ok(live_information(information)),
            Ok(Err(error)) => {
                last_retryable = is_retryable_onvif_error(&error);
                last_error = Some(error.to_string());
            }
            Err(_) => {
                last_retryable = true;
                last_error = Some(format!(
                    "Request exceeded the {} ms timeout.",
                    options.timeout.as_millis()
                ));
            }
        }
        if !last_retryable {
            break;
        }
        if attempt + 1 < attempts {
            sleep(retry_delay(attempt, target)).await;
        }
    }
    Err(AppError::device_operation_failed(
        last_error.unwrap_or_else(|| "Device request failed without an error.".to_owned()),
        last_retryable,
    ))
}

fn build_http_transport(
    options: &ExecutionOptions,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Arc<dyn Transport>, AppError> {
    let mut transport = HttpTransport::new();
    if !options.ca_certificates.is_empty() {
        transport = transport
            .with_root_certificates_pem(&options.ca_certificates)
            .map_err(|error| {
                AppError::invalid_argument(format!("Invalid --ca-certificate input: {error}"))
            })?;
    }
    if let (Some(username), Some(password)) = (username, password) {
        transport = transport.with_credentials(username, password);
    }
    Ok(Arc::new(transport))
}

fn should_sync_clock(policy: ClockSyncPolicy, has_credentials: bool) -> bool {
    match policy {
        ClockSyncPolicy::Auto => has_credentials,
        ClockSyncPolicy::Always => true,
        ClockSyncPolicy::Never => false,
    }
}

async fn scan_discovery_interfaces(
    selectors: &[String],
    timeout: Duration,
    retries: u32,
) -> Result<(Vec<crate::DiscoveryRecord>, Vec<String>, Vec<Warning>), AppError> {
    let selected = resolve_discovery_interfaces(selectors)?;
    scan_selected_discovery_interfaces(selected, timeout, retries, |timeout, address| async move {
        oxvif::discovery::probe_result_on(timeout, &[address]).await
    })
    .await
}

async fn scan_selected_discovery_interfaces<F, Fut>(
    selected: Vec<(Ipv4Addr, String)>,
    timeout: Duration,
    retries: u32,
    probe: F,
) -> Result<(Vec<crate::DiscoveryRecord>, Vec<String>, Vec<Warning>), AppError>
where
    F: Fn(Duration, Ipv4Addr) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = std::io::Result<Vec<oxvif::discovery::DiscoveredDevice>>> + Send + 'static,
{
    let selected_interfaces = selected
        .iter()
        .map(|(_, label)| label.clone())
        .collect::<Vec<_>>();
    let mut tasks = tokio::task::JoinSet::new();
    for (address, label) in selected {
        let probe = probe.clone();
        tasks.spawn(async move {
            let attempts = retries.saturating_add(1);
            for attempt in 0..attempts {
                let result = probe(timeout, address).await;
                if result.is_ok() || attempt + 1 == attempts {
                    return (label, result);
                }
                sleep(retry_delay(attempt, &label)).await;
            }
            unreachable!("discovery retry policy always executes at least once")
        });
    }

    let mut observations = Vec::new();
    let mut warnings = Vec::new();
    let mut successful_interfaces = 0usize;
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok((_, Ok(discovered))) => {
                successful_interfaces += 1;
                observations.extend(discovered);
            }
            Ok((label, Err(error))) => warnings.push(Warning {
                code: "DISCOVERY_INTERFACE_FAILED".to_owned(),
                message: format!("Discovery on `{label}` failed: {error}"),
            }),
            Err(error) => warnings.push(Warning {
                code: "DISCOVERY_INTERFACE_FAILED".to_owned(),
                message: format!("A discovery interface task failed: {error}"),
            }),
        }
    }
    if successful_interfaces == 0 {
        return Err(AppError::discovery_failed(
            warnings
                .iter()
                .map(|warning| warning.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }
    Ok((
        merge_discovery_observations(observations),
        selected_interfaces,
        warnings,
    ))
}

fn snapshot_summary(snapshot: crate::DiscoverySnapshotView) -> crate::DiscoverySnapshotSummary {
    crate::DiscoverySnapshotSummary {
        id: snapshot.id,
        saved_at_unix_ms: snapshot.saved_at_unix_ms,
        generation: snapshot.generation,
        interfaces: snapshot.interfaces,
        device_count: snapshot.devices.len(),
    }
}

fn resolve_discovery_interfaces(selectors: &[String]) -> Result<Vec<(Ipv4Addr, String)>, AppError> {
    let available = oxvif::discovery::discovery_interfaces()
        .map_err(|error| AppError::discovery_failed(error.to_string()))?;
    if available.is_empty() {
        return Err(AppError::discovery_failed(
            "No non-loopback IPv4 discovery interfaces are available.",
        ));
    }
    if selectors.is_empty() {
        return Ok(available
            .into_iter()
            .map(|interface| {
                (
                    interface.address,
                    format!("{}={}", interface.name, interface.address),
                )
            })
            .collect());
    }

    let mut selected = Vec::new();
    for selector in selectors {
        if let Ok(address) = selector.parse::<Ipv4Addr>() {
            if !available
                .iter()
                .any(|interface| interface.address == address)
            {
                return Err(AppError::invalid_argument(format!(
                    "IPv4 address `{selector}` is not assigned to a local discovery interface."
                )));
            }
            let interface = available
                .iter()
                .find(|interface| interface.address == address)
                .expect("address presence checked above");
            selected.push((address, format!("{}={}", interface.name, interface.address)));
            continue;
        }

        let matches = available
            .iter()
            .filter(|interface| interface.name.eq_ignore_ascii_case(selector))
            .collect::<Vec<_>>();
        if matches.is_empty() {
            let choices = available
                .iter()
                .map(|interface| format!("{}={}", interface.name, interface.address))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(AppError::invalid_argument(format!(
                "Unknown discovery interface `{selector}`. Available interfaces: {choices}"
            )));
        }
        for interface in matches {
            selected.push((
                interface.address,
                format!("{}={}", interface.name, interface.address),
            ));
        }
    }
    selected.sort();
    selected.dedup();
    Ok(selected)
}

fn merge_discovery_observations(
    observations: Vec<oxvif::DiscoveredDevice>,
) -> Vec<crate::DiscoveryRecord> {
    let mut devices = BTreeMap::<String, crate::DiscoveryRecord>::new();
    for observation in observations {
        let record = devices
            .entry(observation.endpoint.clone())
            .or_insert_with(|| crate::DiscoveryRecord {
                endpoint: observation.endpoint,
                types: Vec::new(),
                scopes: Vec::new(),
                xaddrs: Vec::new(),
                manufacturer: None,
                model: None,
                firmware_version: None,
                serial_number: None,
            });
        merge_unique(&mut record.types, observation.types);
        merge_unique(&mut record.scopes, observation.scopes);
        merge_unique(&mut record.xaddrs, observation.xaddrs);
    }
    devices.into_values().collect()
}

fn merge_unique(target: &mut Vec<String>, incoming: Vec<String>) {
    target.extend(incoming);
    target.sort();
    target.dedup();
}

struct ResolvedTarget {
    device_id: Option<String>,
    selected_by: Option<String>,
    target: String,
    username: Option<String>,
    password: Option<SecretString>,
}

impl ResolvedTarget {
    fn password(&self) -> Option<&str> {
        self.password.as_ref().map(SecretString::expose_secret)
    }
}

struct Outcome {
    data: CommandData,
    warnings: Vec<Warning>,
    device_id: Option<String>,
    selected_by: Option<String>,
    target: Option<String>,
}

impl Outcome {
    fn data(data: CommandData) -> Self {
        Self {
            data,
            warnings: Vec::new(),
            device_id: None,
            selected_by: None,
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
            selected_by: None,
            target: Some(device.target),
        }
    }

    fn device_selected(action: &str, device: crate::DeviceView, selected_by: String) -> Self {
        let mut outcome = Self::device(action, device);
        outcome.selected_by = Some(selected_by);
        outcome
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
        DeviceAddRequest, DeviceCredentialSetRequest, DeviceIdRequest, DeviceSetupRequest,
        MemoryCredentialStore, NewDevice, SecretString,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn status_server(
        status: u16,
        reason: &'static str,
    ) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("status fixture should bind");
        let address = listener.local_addr().expect("fixture address");
        let requests = Arc::new(AtomicUsize::new(0));
        let observed = requests.clone();
        let task = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                observed.fetch_add(1, Ordering::SeqCst);
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request).await;
                let response = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });
        (
            format!("http://{address}/onvif/device_service"),
            requests,
            task,
        )
    }

    async fn transient_then_proxy_server(
        upstream_url: &str,
    ) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
        let upstream_url = url::Url::parse(upstream_url).expect("upstream URL should parse");
        let upstream_host = upstream_url
            .host_str()
            .expect("upstream URL should have a host")
            .to_owned();
        let upstream_port = upstream_url
            .port_or_known_default()
            .expect("upstream URL should have a port");
        let upstream_path = upstream_url.path().to_owned();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("proxy fixture should bind");
        let address = listener.local_addr().expect("proxy fixture address");
        let requests = Arc::new(AtomicUsize::new(0));
        let observed = requests.clone();
        let task = tokio::spawn(async move {
            while let Ok((mut client, _)) = listener.accept().await {
                let request_number = observed.fetch_add(1, Ordering::SeqCst) + 1;
                if request_number == 1 {
                    let mut request = [0_u8; 4096];
                    let _ = client.read(&mut request).await;
                    let _ = client
                        .write_all(
                            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        )
                        .await;
                    continue;
                }
                let Ok(mut upstream) =
                    tokio::net::TcpStream::connect((upstream_host.as_str(), upstream_port)).await
                else {
                    continue;
                };
                let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
            }
        });
        (format!("http://{address}{upstream_path}"), requests, task)
    }

    async fn hanging_server() -> (
        String,
        Arc<AtomicUsize>,
        Arc<AtomicUsize>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("hanging fixture should bind");
        let address = listener.local_addr().expect("hanging fixture address");
        let requests = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let observed_requests = requests.clone();
        let observed_active = active.clone();
        let task = tokio::spawn(async move {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            observed_requests.fetch_add(1, Ordering::SeqCst);
            observed_active.fetch_add(1, Ordering::SeqCst);
            let mut request = [0_u8; 4096];
            loop {
                match stream.read(&mut request).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
            observed_active.fetch_sub(1, Ordering::SeqCst);
        });
        (
            format!("http://{address}/onvif/device_service"),
            requests,
            active,
            task,
        )
    }

    struct RacingCredentialStore {
        registry: RegistryStore,
        secret: std::sync::Mutex<Option<String>>,
        fail_delete: bool,
    }

    struct FailingSetCredentialStore;

    impl CredentialStore for FailingSetCredentialStore {
        fn set(&self, _reference: &str, _password: &str) -> Result<(), AppError> {
            Err(AppError::credential_backend_unavailable("store"))
        }

        fn get(&self, _reference: &str) -> Result<Option<SecretString>, AppError> {
            Ok(None)
        }

        fn delete(&self, _reference: &str) -> Result<(), AppError> {
            Ok(())
        }
    }

    impl CredentialStore for RacingCredentialStore {
        fn set(&self, _reference: &str, password: &str) -> Result<(), AppError> {
            *self
                .secret
                .lock()
                .map_err(|_| AppError::internal("fixture lock poisoned"))? =
                Some(password.to_owned());
            self.registry.add(NewDevice {
                id: "race-camera".to_owned(),
                name: Some("Concurrent device".to_owned()),
                target: "192.0.2.99".to_owned(),
                tags: Vec::new(),
            })?;
            Ok(())
        }

        fn get(&self, _reference: &str) -> Result<Option<SecretString>, AppError> {
            let secret = self
                .secret
                .lock()
                .map_err(|_| AppError::internal("fixture lock poisoned"))
                .map(|secret| secret.clone())?;
            secret.map(SecretString::new).transpose()
        }

        fn delete(&self, _reference: &str) -> Result<(), AppError> {
            if self.fail_delete {
                return Err(AppError::credential_unavailable(
                    "injected credential cleanup failure",
                ));
            }
            *self
                .secret
                .lock()
                .map_err(|_| AppError::internal("fixture lock poisoned"))? = None;
            Ok(())
        }
    }

    #[tokio::test]
    async fn setup_verifies_then_persists_credential_device_and_current_selection() {
        let server = oxvif::mock::MockServer::start()
            .await
            .expect("mock server should start");
        let directory = tempfile::tempdir().expect("temp directory");
        let registry = RegistryStore::at(directory.path());
        let credentials = Arc::new(MemoryCredentialStore::default());
        let application = Application::with_stores(registry.clone(), credentials.clone());
        let options = ExecutionOptions {
            timeout: Duration::from_secs(20),
            ..ExecutionOptions::default()
        };

        let result = application
            .execute(
                CommandRequest::DeviceSetup(DeviceSetupRequest {
                    device: NewDevice {
                        id: "front-door".to_owned(),
                        name: Some("Front Door".to_owned()),
                        target: server.device_url().to_owned(),
                        tags: vec!["entrance".to_owned()],
                    },
                    username: "admin".to_owned(),
                    password: SecretString::new("setup-secret").expect("secret"),
                    verify: true,
                    set_current: true,
                }),
                &options,
            )
            .await
            .expect("setup should succeed");

        assert_eq!(result.meta.command.as_deref(), Some("setup"));
        let device = registry.get("front-door").expect("device should persist");
        assert!(device.has_credentials);
        assert!(device.manufacturer.is_some());
        assert_eq!(
            registry
                .current()
                .expect("current should load")
                .map(|d| d.id),
            Some("front-door".to_owned())
        );
        assert_eq!(
            credentials
                .get("device/front-door")
                .expect("credential lookup")
                .as_ref()
                .map(SecretString::expose_secret),
            Some("setup-secret")
        );
        let contents = std::fs::read_to_string(directory.path().join("devices.toml"))
            .expect("registry should exist");
        assert!(!contents.contains("setup-secret"));
    }

    #[tokio::test]
    async fn setup_refuses_to_overwrite_a_preexisting_native_secret() {
        let directory = tempfile::tempdir().expect("temp directory");
        let registry = RegistryStore::at(directory.path());
        let credentials = Arc::new(MemoryCredentialStore::default());
        credentials
            .set("device/front-door", "existing-secret")
            .expect("fixture credential");
        let application = Application::with_stores(registry.clone(), credentials.clone());

        let error = application
            .execute(
                CommandRequest::DeviceSetup(DeviceSetupRequest {
                    device: NewDevice {
                        id: "front-door".to_owned(),
                        name: None,
                        target: "192.0.2.20".to_owned(),
                        tags: Vec::new(),
                    },
                    username: "admin".to_owned(),
                    password: SecretString::new("new-secret").expect("secret"),
                    verify: false,
                    set_current: false,
                }),
                &ExecutionOptions::default(),
            )
            .await
            .expect_err("existing secret must be preserved");

        assert_eq!(error.code, crate::ErrorCode::ResourceAlreadyExists);
        assert!(registry.get("front-door").is_err());
        assert_eq!(
            credentials
                .get("device/front-door")
                .expect("credential lookup")
                .as_ref()
                .map(SecretString::expose_secret),
            Some("existing-secret")
        );
    }

    #[tokio::test]
    async fn failed_native_secret_write_never_persists_a_credential_reference() {
        let directory = tempfile::tempdir().expect("temp directory");
        let registry = RegistryStore::at(directory.path());
        registry
            .add(NewDevice {
                id: "camera".to_owned(),
                name: None,
                target: "192.0.2.20".to_owned(),
                tags: Vec::new(),
            })
            .expect("fixture device");
        let application =
            Application::with_stores(registry.clone(), Arc::new(FailingSetCredentialStore));

        let error = application
            .execute(
                CommandRequest::DeviceCredentialSet(DeviceCredentialSetRequest {
                    id: "camera".to_owned(),
                    username: "sensitive-account".to_owned(),
                    password: SecretString::new("sensitive-secret").expect("secret"),
                }),
                &ExecutionOptions::default(),
            )
            .await
            .expect_err("native persistence failure must stop registry mutation");

        assert_eq!(error.code, crate::ErrorCode::CredentialUnavailable);
        assert!(!format!("{error:?}").contains("sensitive"));
        assert!(!registry.get("camera").expect("device").has_credentials);
        let contents = std::fs::read_to_string(directory.path().join("devices.toml"))
            .expect("registry should exist");
        assert!(!contents.contains("credential_ref"));
        assert!(!contents.contains("sensitive"));
    }

    #[tokio::test]
    async fn every_device_execution_path_applies_the_shared_custom_root_factory() {
        let (target, requests, _, fixture) = hanging_server().await;
        let directory = tempfile::tempdir().expect("temp directory");
        let registry = RegistryStore::at(directory.path());
        let credentials = Arc::new(MemoryCredentialStore::default());
        let application = Application::with_stores(registry.clone(), credentials);
        let invalid_roots = ExecutionOptions {
            ca_certificates: vec![b"not a PEM certificate".to_vec()],
            timeout: Duration::from_millis(50),
            ..ExecutionOptions::default()
        };

        let setup_error = application
            .execute(
                CommandRequest::DeviceSetup(DeviceSetupRequest {
                    device: NewDevice {
                        id: "setup-camera".to_owned(),
                        name: None,
                        target: target.clone(),
                        tags: Vec::new(),
                    },
                    username: "admin".to_owned(),
                    password: SecretString::new("secret").expect("secret"),
                    verify: true,
                    set_current: false,
                }),
                &invalid_roots,
            )
            .await
            .expect_err("setup must validate shared roots before connecting");
        assert_eq!(setup_error.code, crate::ErrorCode::InvalidArgument);

        application
            .execute(
                CommandRequest::DeviceAdd(DeviceAddRequest {
                    device: NewDevice {
                        id: "camera".to_owned(),
                        name: None,
                        target: target.clone(),
                        tags: Vec::new(),
                    },
                }),
                &ExecutionOptions::default(),
            )
            .await
            .expect("fixture device");
        application
            .execute(
                CommandRequest::DeviceCredentialSet(DeviceCredentialSetRequest {
                    id: "camera".to_owned(),
                    username: "admin".to_owned(),
                    password: SecretString::new("secret").expect("secret"),
                }),
                &ExecutionOptions::default(),
            )
            .await
            .expect("fixture credential");

        for request in [
            CommandRequest::DeviceInfo(crate::DeviceConnectRequest {
                selector: crate::TargetSelector {
                    device: Some("camera".to_owned()),
                    ..crate::TargetSelector::default()
                },
            }),
            CommandRequest::HealthCheck(crate::DeviceConnectRequest {
                selector: crate::TargetSelector {
                    device: Some("camera".to_owned()),
                    ..crate::TargetSelector::default()
                },
            }),
            CommandRequest::DeviceRefresh(DeviceIdRequest {
                id: "camera".to_owned(),
            }),
        ] {
            let error = application
                .execute(request, &invalid_roots)
                .await
                .expect_err("device path must validate shared roots");
            assert_eq!(error.code, crate::ErrorCode::InvalidArgument);
        }

        application
            .execute(
                CommandRequest::GroupCreate(crate::GroupCreateRequest {
                    group: crate::NewGroup {
                        id: "fleet".to_owned(),
                        name: None,
                    },
                }),
                &ExecutionOptions::default(),
            )
            .await
            .expect("fixture group");
        application
            .execute(
                CommandRequest::GroupMemberAdd(crate::GroupMemberAddRequest {
                    group_id: "fleet".to_owned(),
                    device_id: "camera".to_owned(),
                    alias: "camera".to_owned(),
                }),
                &ExecutionOptions::default(),
            )
            .await
            .expect("fixture group member");
        let fleet_error = application
            .execute(
                CommandRequest::DeviceInfo(crate::DeviceConnectRequest {
                    selector: crate::TargetSelector {
                        group: Some("fleet".to_owned()),
                        ..crate::TargetSelector::default()
                    },
                }),
                &invalid_roots,
            )
            .await
            .expect_err("fleet items must validate shared roots");
        assert_eq!(fleet_error.code, crate::ErrorCode::FleetFailed);

        application
            .execute(
                CommandRequest::CredentialProfileSet(crate::CredentialProfileSetRequest {
                    id: "factory-admin".to_owned(),
                    username: "admin".to_owned(),
                    password: SecretString::new("secret").expect("secret"),
                }),
                &ExecutionOptions::default(),
            )
            .await
            .expect("fixture profile");
        registry
            .save_discovery_snapshot(
                "scan",
                vec![crate::DiscoveryRecord {
                    endpoint: "urn:uuid:test-camera".to_owned(),
                    types: Vec::new(),
                    scopes: Vec::new(),
                    xaddrs: vec![target],
                    manufacturer: None,
                    model: None,
                    firmware_version: None,
                    serial_number: None,
                }],
            )
            .expect("fixture snapshot");
        let enrich_error = application
            .execute(
                CommandRequest::DiscoveryEnrich(crate::DiscoveryEnrichRequest {
                    id: "scan".to_owned(),
                    credential_profile: "factory-admin".to_owned(),
                    filters: Vec::new(),
                    jobs: 1,
                }),
                &invalid_roots,
            )
            .await
            .expect_err("enrichment must validate shared roots");
        assert_eq!(enrich_error.code, crate::ErrorCode::DiscoveryFailed);

        assert_eq!(requests.load(Ordering::SeqCst), 0);
        fixture.abort();
    }

    #[tokio::test]
    async fn failed_setup_verification_leaves_no_local_state() {
        let directory = tempfile::tempdir().expect("temp directory");
        let registry = RegistryStore::at(directory.path());
        let credentials = Arc::new(MemoryCredentialStore::default());
        let application = Application::with_stores(registry.clone(), credentials.clone());
        let options = ExecutionOptions {
            timeout: Duration::from_millis(50),
            ..ExecutionOptions::default()
        };

        let error = application
            .execute(
                CommandRequest::DeviceSetup(DeviceSetupRequest {
                    device: NewDevice {
                        id: "offline".to_owned(),
                        name: None,
                        target: "127.0.0.1:9".to_owned(),
                        tags: Vec::new(),
                    },
                    username: "admin".to_owned(),
                    password: SecretString::new("never-store").expect("secret"),
                    verify: true,
                    set_current: true,
                }),
                &options,
            )
            .await
            .expect_err("verification should fail");

        assert_eq!(error.code, crate::ErrorCode::DeviceConnectionFailed);
        assert!(registry.get("offline").is_err());
        assert_eq!(
            credentials
                .get("device/offline")
                .expect("credential lookup"),
            None
        );
    }

    #[tokio::test]
    async fn setup_cleans_its_new_secret_if_registry_add_loses_a_race() {
        let directory = tempfile::tempdir().expect("temp directory");
        let registry = RegistryStore::at(directory.path());
        let credentials = Arc::new(RacingCredentialStore {
            registry: registry.clone(),
            secret: std::sync::Mutex::new(None),
            fail_delete: false,
        });
        let application = Application::with_stores(registry.clone(), credentials.clone());

        let error = application
            .execute(
                CommandRequest::DeviceSetup(DeviceSetupRequest {
                    device: NewDevice {
                        id: "race-camera".to_owned(),
                        name: Some("Setup device".to_owned()),
                        target: "192.0.2.20".to_owned(),
                        tags: Vec::new(),
                    },
                    username: "admin".to_owned(),
                    password: SecretString::new("temporary-secret").expect("secret"),
                    verify: false,
                    set_current: false,
                }),
                &ExecutionOptions::default(),
            )
            .await
            .expect_err("concurrent add must win");

        assert_eq!(error.code, crate::ErrorCode::DeviceAlreadyExists);
        assert_eq!(
            credentials
                .get("device/race-camera")
                .expect("secret lookup"),
            None
        );
        assert_eq!(
            registry
                .get("race-camera")
                .expect("concurrent device must remain")
                .name,
            "Concurrent device"
        );
    }

    #[tokio::test]
    async fn setup_reports_an_incomplete_rollback() {
        let directory = tempfile::tempdir().expect("temp directory");
        let registry = RegistryStore::at(directory.path());
        let credentials = Arc::new(RacingCredentialStore {
            registry: registry.clone(),
            secret: std::sync::Mutex::new(None),
            fail_delete: true,
        });
        let application = Application::with_stores(registry, credentials.clone());

        let error = application
            .execute(
                CommandRequest::DeviceSetup(DeviceSetupRequest {
                    device: NewDevice {
                        id: "race-camera".to_owned(),
                        name: None,
                        target: "192.0.2.20".to_owned(),
                        tags: Vec::new(),
                    },
                    username: "admin".to_owned(),
                    password: SecretString::new("temporary-secret").expect("secret"),
                    verify: false,
                    set_current: false,
                }),
                &ExecutionOptions::default(),
            )
            .await
            .expect_err("incomplete rollback should be reported");

        assert!(error.message.contains("rollback was incomplete"));
        assert!(error.suggested_action.is_some());
        assert_eq!(
            credentials
                .get("device/race-camera")
                .expect("secret lookup")
                .as_ref()
                .map(SecretString::expose_secret),
            Some("temporary-secret")
        );
    }

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
                .as_ref()
                .map(SecretString::expose_secret),
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
                .as_ref()
                .map(SecretString::expose_secret),
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

    #[tokio::test]
    async fn discovery_enrichment_uses_profile_and_atomically_updates_snapshot() {
        let server = oxvif::mock::MockServer::builder()
            .enforce_auth(true)
            .start()
            .await
            .expect("mock server should start");
        let directory = tempfile::tempdir().expect("temp directory");
        let registry = RegistryStore::at(directory.path());
        let credentials = Arc::new(MemoryCredentialStore::default());
        credentials
            .set("profile/factory-admin", "admin")
            .expect("secret should set");
        registry
            .set_credential_profile("factory-admin", "admin", "profile/factory-admin")
            .expect("profile should set");
        registry
            .save_discovery_snapshot(
                "scan",
                vec![
                    crate::DiscoveryRecord {
                        endpoint: "uuid:mock-camera".to_owned(),
                        types: Vec::new(),
                        scopes: Vec::new(),
                        xaddrs: vec![server.device_url().to_owned()],
                        manufacturer: None,
                        model: None,
                        firmware_version: None,
                        serial_number: None,
                    },
                    crate::DiscoveryRecord {
                        endpoint: "uuid:no-target".to_owned(),
                        types: Vec::new(),
                        scopes: Vec::new(),
                        xaddrs: Vec::new(),
                        manufacturer: None,
                        model: None,
                        firmware_version: None,
                        serial_number: None,
                    },
                ],
            )
            .expect("snapshot should save");
        let application = Application::with_stores(registry.clone(), credentials);

        let result = application
            .execute(
                CommandRequest::DiscoveryEnrich(crate::DiscoveryEnrichRequest {
                    id: "scan".to_owned(),
                    credential_profile: "factory-admin".to_owned(),
                    filters: Vec::new(),
                    jobs: 2,
                }),
                &ExecutionOptions::default(),
            )
            .await
            .expect("enrichment should succeed");
        let CommandData::DiscoveryEnrichment {
            enriched, failed, ..
        } = result.data
        else {
            panic!("expected enrichment result");
        };
        assert_eq!(enriched, 1);
        assert_eq!(failed, 1);
        assert_eq!(result.warnings.len(), 1);
        let snapshot = registry
            .get_discovery_snapshot("scan", &[])
            .expect("snapshot should load");
        assert!(
            snapshot
                .devices
                .iter()
                .find(|device| device.endpoint == "uuid:mock-camera")
                .expect("mock endpoint should remain")
                .manufacturer
                .is_some()
        );
        let persisted =
            std::fs::read_to_string(directory.path().join("snapshots").join("scan.json"))
                .expect("snapshot file should read");
        assert!(!persisted.contains("admin"));
    }

    #[tokio::test]
    async fn total_discovery_enrichment_failure_preserves_snapshot() {
        let directory = tempfile::tempdir().expect("temp directory");
        let registry = RegistryStore::at(directory.path());
        let credentials = Arc::new(MemoryCredentialStore::default());
        credentials
            .set("profile/factory-admin", "secret")
            .expect("secret should set");
        registry
            .set_credential_profile("factory-admin", "admin", "profile/factory-admin")
            .expect("profile should set");
        registry
            .save_discovery_snapshot(
                "scan",
                vec![crate::DiscoveryRecord {
                    endpoint: "uuid:no-target".to_owned(),
                    types: Vec::new(),
                    scopes: Vec::new(),
                    xaddrs: Vec::new(),
                    manufacturer: None,
                    model: None,
                    firmware_version: None,
                    serial_number: None,
                }],
            )
            .expect("snapshot should save");
        let before = registry
            .get_discovery_snapshot("scan", &[])
            .expect("snapshot should load");
        let application = Application::with_stores(registry.clone(), credentials);
        let error = application
            .execute(
                CommandRequest::DiscoveryEnrich(crate::DiscoveryEnrichRequest {
                    id: "scan".to_owned(),
                    credential_profile: "factory-admin".to_owned(),
                    filters: Vec::new(),
                    jobs: 1,
                }),
                &ExecutionOptions::default(),
            )
            .await
            .expect_err("total failure should be typed");
        assert_eq!(error.code, crate::ErrorCode::DiscoveryFailed);
        assert_eq!(
            registry
                .get_discovery_snapshot("scan", &[])
                .expect("snapshot should remain")
                .devices,
            before.devices
        );
    }

    #[tokio::test]
    async fn read_only_diagnostics_run_against_mock_device() {
        let server = oxvif::mock::MockServer::start()
            .await
            .expect("mock server should start");
        let directory = tempfile::tempdir().expect("temp directory");
        let application = Application::with_stores(
            RegistryStore::at(directory.path()),
            Arc::new(MemoryCredentialStore::default()),
        );
        let target = || crate::TargetSelector {
            device: None,
            target: Some(server.device_url().to_owned()),
            ..crate::TargetSelector::default()
        };
        let options = ExecutionOptions {
            timeout: Duration::from_secs(20),
            ..ExecutionOptions::default()
        };

        let capabilities = application
            .execute(
                CommandRequest::DeviceCapabilities(crate::DeviceConnectRequest {
                    selector: target(),
                }),
                &options,
            )
            .await
            .expect("capabilities should succeed");
        let CommandData::DeviceDiagnostic { result, .. } = capabilities.data else {
            panic!("expected diagnostic result");
        };
        assert!(result.get("media").is_some());

        let profiles = application
            .execute(
                CommandRequest::MediaProfiles(crate::DeviceConnectRequest { selector: target() }),
                &options,
            )
            .await
            .expect("profiles should succeed");
        let CommandData::DeviceDiagnostic { result, .. } = profiles.data else {
            panic!("expected diagnostic result");
        };
        let profile = result[0]["token"]
            .as_str()
            .expect("profile token")
            .to_owned();

        let requests = [
            CommandRequest::DeviceServices(crate::DeviceConnectRequest { selector: target() }),
            CommandRequest::MediaStreamUri(crate::ProfileConnectRequest {
                selector: target(),
                profile: profile.clone(),
            }),
            CommandRequest::MediaSnapshotUri(crate::ProfileConnectRequest {
                selector: target(),
                profile: profile.clone(),
            }),
            CommandRequest::PtzStatus(crate::ProfileConnectRequest {
                selector: target(),
                profile: profile.clone(),
            }),
            CommandRequest::PtzPresets(crate::ProfileConnectRequest {
                selector: target(),
                profile,
            }),
            CommandRequest::HealthCheck(crate::DeviceConnectRequest { selector: target() }),
        ];
        for request in requests {
            let result = application
                .execute(request, &options)
                .await
                .expect("diagnostic should succeed");
            assert!(matches!(result.data, CommandData::DeviceDiagnostic { .. }));
        }
    }

    #[tokio::test]
    async fn group_and_view_diagnostics_are_bounded_deterministic_and_partial() {
        let server_a = oxvif::mock::MockServer::start()
            .await
            .expect("mock A should start");
        let server_b = oxvif::mock::MockServer::start()
            .await
            .expect("mock B should start");
        let directory = tempfile::tempdir().expect("temp directory");
        let registry = RegistryStore::at(directory.path());
        for (id, target, tags) in [
            (
                "camera-a",
                server_a.device_url(),
                vec!["healthy".to_owned()],
            ),
            (
                "camera-b",
                server_b.device_url(),
                vec!["healthy".to_owned()],
            ),
            (
                "camera-c",
                "http://127.0.0.1:9/onvif/device_service",
                Vec::new(),
            ),
        ] {
            registry
                .add(NewDevice {
                    id: id.to_owned(),
                    name: None,
                    target: target.to_owned(),
                    tags,
                })
                .expect("device should add");
        }
        registry
            .create_group(crate::NewGroup {
                id: "factory".to_owned(),
                name: None,
            })
            .expect("group should create");
        for (id, alias) in [
            ("camera-c", "cam-003"),
            ("camera-a", "cam-001"),
            ("camera-b", "cam-002"),
        ] {
            registry
                .add_group_member("factory", id, alias)
                .expect("member should add");
        }
        registry
            .create_view(crate::NewSavedView {
                id: "healthy".to_owned(),
                name: None,
                filters: vec!["tag=healthy".parse().expect("filter should parse")],
                match_mode: crate::MatchMode::All,
            })
            .expect("view should create");
        let application =
            Application::with_stores(registry, Arc::new(MemoryCredentialStore::default()));
        let options = ExecutionOptions {
            timeout: Duration::from_secs(2),
            jobs: 2,
            ..ExecutionOptions::default()
        };

        let partial = application
            .execute(
                CommandRequest::DeviceCapabilities(crate::DeviceConnectRequest {
                    selector: crate::TargetSelector {
                        group: Some("factory".to_owned()),
                        ..crate::TargetSelector::default()
                    },
                }),
                &options,
            )
            .await
            .expect("partial fleet should return structured items");
        assert_eq!(partial.exit_code(), 6);
        let CommandData::FleetDiagnostic {
            succeeded,
            failed,
            items,
            ..
        } = &partial.data
        else {
            panic!("expected fleet result");
        };
        assert_eq!((*succeeded, *failed), (2, 1));
        assert_eq!(
            items
                .iter()
                .map(|item| item.device_id.as_str())
                .collect::<Vec<_>>(),
            ["camera-a", "camera-b", "camera-c"]
        );
        let jsonl = crate::render_success(crate::OutputFormat::JsonLines, &partial)
            .expect("JSONL should render");
        let lines = jsonl.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 4);
        let summary: serde_json::Value =
            serde_json::from_str(lines[3]).expect("summary should parse");
        assert_eq!(summary["data"]["kind"], "fleet_summary");
        assert_eq!(summary["ok"], false);

        let successful = application
            .execute(
                CommandRequest::DeviceServices(crate::DeviceConnectRequest {
                    selector: crate::TargetSelector {
                        view: Some("healthy".to_owned()),
                        ..crate::TargetSelector::default()
                    },
                }),
                &options,
            )
            .await
            .expect("view fleet should succeed");
        assert_eq!(successful.exit_code(), 0);
        let CommandData::FleetDiagnostic { failed, total, .. } = successful.data else {
            panic!("expected fleet result");
        };
        assert_eq!((total, failed), (2, 0));
    }

    #[tokio::test]
    async fn all_failed_fleet_is_a_typed_error() {
        let directory = tempfile::tempdir().expect("temp directory");
        let registry = RegistryStore::at(directory.path());
        registry
            .add(NewDevice {
                id: "offline".to_owned(),
                name: None,
                target: "http://127.0.0.1:9/onvif/device_service".to_owned(),
                tags: Vec::new(),
            })
            .expect("device should add");
        registry
            .create_group(crate::NewGroup {
                id: "offline".to_owned(),
                name: None,
            })
            .expect("group should create");
        registry
            .add_group_member("offline", "offline", "cam-001")
            .expect("member should add");
        let application =
            Application::with_stores(registry, Arc::new(MemoryCredentialStore::default()));
        let error = application
            .execute(
                CommandRequest::DeviceCapabilities(crate::DeviceConnectRequest {
                    selector: crate::TargetSelector {
                        group: Some("offline".to_owned()),
                        ..crate::TargetSelector::default()
                    },
                }),
                &ExecutionOptions {
                    timeout: Duration::from_secs(1),
                    ..ExecutionOptions::default()
                },
            )
            .await
            .expect_err("all failures should be an error");
        assert_eq!(error.code, crate::ErrorCode::FleetFailed);
    }

    #[test]
    fn diagnostic_uri_sanitizer_removes_userinfo() {
        let mut value = serde_json::json!({
            "uri": "rtsp://admin:top-secret@192.0.2.10/stream",
            "nested": ["http://user:pass@example.test/snapshot.jpg"]
        });
        sanitize_uri_values(&mut value);

        assert_eq!(value["uri"], "rtsp://192.0.2.10/stream");
        assert_eq!(value["nested"][0], "http://example.test/snapshot.jpg");
        assert!(!value.to_string().contains("top-secret"));
    }

    #[test]
    fn retry_policy_retries_only_transient_transport_statuses() {
        let transient = OnvifError::Transport(TransportError::HttpStatus {
            status: 503,
            body: "Service Unavailable".to_owned(),
        });
        let authentication = OnvifError::Transport(TransportError::HttpStatus {
            status: 401,
            body: "Unauthorized".to_owned(),
        });
        let soap_fault = OnvifError::Soap(oxvif::soap::SoapError::Fault {
            code: "s:Sender".to_owned(),
            reason: "Invalid request".to_owned(),
            subcode: None,
            detail: None,
        });

        assert!(is_retryable_onvif_error(&transient));
        assert!(!is_retryable_onvif_error(&authentication));
        assert!(!is_retryable_onvif_error(&soap_fault));
    }

    #[tokio::test]
    async fn diagnostic_retries_transient_status_but_not_deterministic_status() {
        for (status, reason, retries, expected_requests, expected_retryable) in [
            (503, "Service Unavailable", 2, 3, true),
            (400, "Bad Request", 2, 1, false),
        ] {
            let (target, requests, server) = status_server(status, reason).await;
            let resolved = ResolvedTarget {
                device_id: None,
                selected_by: Some("test fixture".to_owned()),
                target,
                username: None,
                password: None,
            };
            let error = execute_diagnostic(
                &resolved,
                DiagnosticOperation::Capabilities,
                &ExecutionOptions {
                    timeout: Duration::from_secs(1),
                    retries,
                    ..ExecutionOptions::default()
                },
            )
            .await
            .expect_err("status fixture should fail");
            server.abort();

            assert_eq!(requests.load(Ordering::SeqCst), expected_requests);
            assert_eq!(error.retryable, expected_retryable);
        }
    }

    #[tokio::test]
    async fn diagnostic_recovers_when_a_retryable_first_attempt_is_followed_by_success() {
        let upstream = oxvif::mock::MockServer::start()
            .await
            .expect("mock server should start");
        let (target, requests, proxy) = transient_then_proxy_server(upstream.device_url()).await;
        let resolved = ResolvedTarget {
            device_id: None,
            selected_by: Some("transient proxy fixture".to_owned()),
            target,
            username: None,
            password: None,
        };

        let result = execute_diagnostic(
            &resolved,
            DiagnosticOperation::Capabilities,
            &ExecutionOptions {
                timeout: Duration::from_secs(2),
                retries: 1,
                ..ExecutionOptions::default()
            },
        )
        .await
        .expect("second attempt should succeed");
        proxy.abort();

        assert!(result.is_object());
        assert_eq!(requests.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn diagnostic_timeout_closes_the_connection_and_leaves_no_active_fixture_work() {
        let (target, requests, active, server) = hanging_server().await;
        let resolved = ResolvedTarget {
            device_id: None,
            selected_by: Some("hanging fixture".to_owned()),
            target,
            username: None,
            password: None,
        };

        let error = execute_diagnostic(
            &resolved,
            DiagnosticOperation::Capabilities,
            &ExecutionOptions {
                timeout: Duration::from_millis(100),
                ..ExecutionOptions::default()
            },
        )
        .await
        .expect_err("hanging request should time out");
        tokio::time::timeout(Duration::from_secs(2), server)
            .await
            .expect("fixture should observe the cancelled connection")
            .expect("fixture task should finish cleanly");

        assert!(error.retryable);
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn cancelling_a_command_during_retry_backoff_prevents_the_next_attempt() {
        let (target, requests, server) = status_server(503, "Service Unavailable").await;
        let resolved = ResolvedTarget {
            device_id: None,
            selected_by: Some("retry cancellation fixture".to_owned()),
            target,
            username: None,
            password: None,
        };
        let options = ExecutionOptions {
            timeout: Duration::from_secs(1),
            retries: 2,
            ..ExecutionOptions::default()
        };
        let future = execute_diagnostic(&resolved, DiagnosticOperation::Capabilities, &options);

        assert!(
            tokio::time::timeout(Duration::from_millis(50), future)
                .await
                .is_err(),
            "parent timeout should cancel the retry backoff"
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
        server.abort();

        assert_eq!(requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cancelling_cli_discovery_aborts_all_interface_workers() {
        struct ActiveGuard(Arc<AtomicUsize>);
        impl Drop for ActiveGuard {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }

        let started = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let observed_started = started.clone();
        let observed_active = active.clone();
        let probe = move |_timeout: Duration, _address: Ipv4Addr| {
            let started = observed_started.clone();
            let active = observed_active.clone();
            async move {
                started.fetch_add(1, Ordering::SeqCst);
                active.fetch_add(1, Ordering::SeqCst);
                let _guard = ActiveGuard(active);
                std::future::pending::<std::io::Result<Vec<oxvif::discovery::DiscoveredDevice>>>()
                    .await
            }
        };
        let future = scan_selected_discovery_interfaces(
            vec![(Ipv4Addr::LOCALHOST, "loopback-test".to_owned())],
            Duration::from_secs(30),
            2,
            probe,
        );

        assert!(
            tokio::time::timeout(Duration::from_millis(100), future)
                .await
                .is_err(),
            "the parent timeout should cancel the CLI discovery wrapper"
        );
        tokio::task::yield_now().await;

        assert_eq!(started.load(Ordering::SeqCst), 1);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn fleet_timeout_releases_the_bounded_worker_for_the_next_device() {
        let (hanging_target, requests, active, hanging_server) = hanging_server().await;
        let healthy_server = oxvif::mock::MockServer::start()
            .await
            .expect("healthy mock should start");
        let directory = tempfile::tempdir().expect("temp directory");
        let registry = RegistryStore::at(directory.path());
        for (id, target) in [
            ("camera-a-hanging", hanging_target.as_str()),
            ("camera-b-healthy", healthy_server.device_url()),
        ] {
            registry
                .add(NewDevice {
                    id: id.to_owned(),
                    name: None,
                    target: target.to_owned(),
                    tags: Vec::new(),
                })
                .expect("device should add");
        }
        registry
            .create_group(crate::NewGroup {
                id: "bounded".to_owned(),
                name: None,
            })
            .expect("group should create");
        for (id, alias) in [
            ("camera-a-hanging", "cam-001"),
            ("camera-b-healthy", "cam-002"),
        ] {
            registry
                .add_group_member("bounded", id, alias)
                .expect("member should add");
        }
        let application =
            Application::with_stores(registry, Arc::new(MemoryCredentialStore::default()));

        let outcome = application
            .execute(
                CommandRequest::DeviceCapabilities(crate::DeviceConnectRequest {
                    selector: crate::TargetSelector {
                        group: Some("bounded".to_owned()),
                        ..crate::TargetSelector::default()
                    },
                }),
                &ExecutionOptions {
                    timeout: Duration::from_millis(100),
                    jobs: 1,
                    ..ExecutionOptions::default()
                },
            )
            .await
            .expect("healthy device should run after the timed-out worker");
        tokio::time::timeout(Duration::from_secs(2), hanging_server)
            .await
            .expect("fixture should observe the cancelled connection")
            .expect("fixture task should finish cleanly");

        let CommandData::FleetDiagnostic {
            succeeded,
            failed,
            items,
            ..
        } = outcome.data
        else {
            panic!("expected fleet diagnostic");
        };
        assert_eq!((succeeded, failed), (1, 1));
        assert_eq!(items[0].device_id, "camera-a-hanging");
        assert!(!items[0].ok);
        assert_eq!(items[1].device_id, "camera-b-healthy");
        assert!(items[1].ok);
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn retry_backoff_is_bounded_and_deterministic() {
        assert_eq!(retry_delay(3, "camera-a"), retry_delay(3, "camera-a"));
        assert!(retry_delay(1, "camera-a") > retry_delay(0, "camera-a"));
        assert!(retry_delay(99, "camera-a") <= Duration::from_secs(2));
    }

    #[test]
    fn automatic_clock_sync_is_credential_aware_and_never_mutates_device_time() {
        assert!(should_sync_clock(ClockSyncPolicy::Auto, true));
        assert!(!should_sync_clock(ClockSyncPolicy::Auto, false));
        assert!(should_sync_clock(ClockSyncPolicy::Always, false));
        assert!(!should_sync_clock(ClockSyncPolicy::Never, true));
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

    #[test]
    fn discovery_observations_merge_fields_across_interfaces() {
        let devices = merge_discovery_observations(vec![
            oxvif::DiscoveredDevice {
                endpoint: "urn:uuid:camera".to_owned(),
                types: vec!["Device".to_owned()],
                scopes: vec!["scope:a".to_owned()],
                xaddrs: vec!["http://192.0.2.10/onvif/device_service".to_owned()],
            },
            oxvif::DiscoveredDevice {
                endpoint: "urn:uuid:camera".to_owned(),
                types: vec!["NetworkVideoTransmitter".to_owned()],
                scopes: vec!["scope:b".to_owned()],
                xaddrs: vec!["http://198.51.100.10/onvif/device_service".to_owned()],
            },
        ]);

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].types.len(), 2);
        assert_eq!(devices[0].scopes.len(), 2);
        assert_eq!(devices[0].xaddrs.len(), 2);
    }
}
