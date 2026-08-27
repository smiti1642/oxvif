use std::{collections::BTreeMap, env, net::Ipv4Addr, sync::Arc, time::Duration};

use oxvif::{DeviceInfo, OnvifClient, OnvifSession, health::HealthCheck};
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
            CommandRequest::AgentGuide => Outcome::data(CommandData::AgentGuide {
                guide: crate::agent::guide(),
            }),
            CommandRequest::AgentPrompt => Outcome::data(CommandData::AgentPrompt {
                prompt: crate::agent::prompt(),
            }),
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
                let (devices, selected_interfaces, warnings) =
                    scan_discovery_interfaces(&request.interfaces, options.timeout).await?;
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
                let mut outcome = Outcome::data(CommandData::DiscoveryScan {
                    devices,
                    saved_snapshot,
                    interfaces: selected_interfaces,
                });
                outcome.warnings = warnings;
                outcome
            }
            CommandRequest::DiscoveryRefresh(request) => {
                let (devices, selected_interfaces, warnings) =
                    scan_discovery_interfaces(&request.interfaces, options.timeout).await?;
                let snapshot = self.registry.refresh_discovery_snapshot(
                    &request.id,
                    devices.clone(),
                    selected_interfaces.clone(),
                )?;
                let mut outcome = Outcome::data(CommandData::DiscoveryScan {
                    devices,
                    saved_snapshot: Some(snapshot_summary(snapshot)),
                    interfaces: selected_interfaces,
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
                            Some(&password),
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
                                Some(&password),
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
                    selected_by: resolved.selected_by,
                    target: Some(resolved.target),
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
            selected_by: Some(id.to_owned()),
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
            resolved.password.as_deref(),
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
}

enum DiagnosticOperation {
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

async fn execute_diagnostic(
    resolved: &ResolvedTarget,
    operation: DiagnosticOperation,
    options: &ExecutionOptions,
) -> Result<serde_json::Value, AppError> {
    if matches!(&operation, DiagnosticOperation::Health) {
        return execute_health_check(resolved, options).await;
    }
    let attempts = options.retries.saturating_add(1);
    let mut last_error = None;
    for _ in 0..attempts {
        let future = async {
            let mut builder = OnvifSession::builder(&resolved.target);
            if let (Some(username), Some(password)) =
                (resolved.username.as_deref(), resolved.password.as_deref())
            {
                builder = builder.with_credentials(username, password);
            }
            let session = builder.build().await.map_err(|error| error.to_string())?;
            let mut value = match &operation {
                DiagnosticOperation::Capabilities => serde_json::to_value(session.capabilities()),
                DiagnosticOperation::Services => {
                    serde_json::to_value(session.get_services().await.map_err(|e| e.to_string())?)
                }
                DiagnosticOperation::MediaProfiles => {
                    serde_json::to_value(session.get_profiles().await.map_err(|e| e.to_string())?)
                }
                DiagnosticOperation::MediaStreamUri(profile) => serde_json::to_value(
                    session
                        .get_stream_uri(profile)
                        .await
                        .map_err(|e| e.to_string())?,
                ),
                DiagnosticOperation::MediaSnapshotUri(profile) => serde_json::to_value(
                    session
                        .get_snapshot_uri(profile)
                        .await
                        .map_err(|e| e.to_string())?,
                ),
                DiagnosticOperation::PtzStatus(profile) => serde_json::to_value(
                    session
                        .ptz_get_status(profile)
                        .await
                        .map_err(|e| e.to_string())?,
                ),
                DiagnosticOperation::PtzPresets(profile) => serde_json::to_value(
                    session
                        .ptz_get_presets(profile)
                        .await
                        .map_err(|e| e.to_string())?,
                ),
                DiagnosticOperation::Health => unreachable!("health is handled above"),
            }
            .map_err(|error| error.to_string())?;
            sanitize_uri_values(&mut value);
            Ok::<_, String>(value)
        };
        match timeout(options.timeout, future).await {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(error)) => last_error = Some(error),
            Err(_) => {
                last_error = Some(format!(
                    "Diagnostic exceeded the {} ms timeout.",
                    options.timeout.as_millis()
                ));
            }
        }
    }
    Err(AppError::device_connection_failed(
        last_error.unwrap_or_else(|| "Device diagnostic failed without an error.".to_owned()),
    ))
}

async fn execute_health_check(
    resolved: &ResolvedTarget,
    options: &ExecutionOptions,
) -> Result<serde_json::Value, AppError> {
    let mut check = HealthCheck::new(&resolved.target);
    if let (Some(username), Some(password)) =
        (resolved.username.as_deref(), resolved.password.as_deref())
    {
        check = check.with_credentials(username, password);
    }
    let report = timeout(options.timeout, check.run()).await.map_err(|_| {
        AppError::device_connection_failed(format!(
            "Health check exceeded the {} ms timeout.",
            options.timeout.as_millis()
        ))
    })?;
    let mut value = serde_json::to_value(report)
        .map_err(|error| AppError::serialization_failed(error.to_string()))?;
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
    let attempts = options.retries.saturating_add(1);
    let mut last_error = None;
    for _ in 0..attempts {
        let mut client = OnvifClient::new(target);
        if let (Some(username), Some(password)) = (username, password) {
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

async fn scan_discovery_interfaces(
    selectors: &[String],
    timeout: Duration,
) -> Result<(Vec<crate::DiscoveryRecord>, Vec<String>, Vec<Warning>), AppError> {
    let selected = resolve_discovery_interfaces(selectors)?;
    let selected_interfaces = selected
        .iter()
        .map(|(_, label)| label.clone())
        .collect::<Vec<_>>();
    let mut tasks = tokio::task::JoinSet::new();
    for (address, label) in selected {
        tasks.spawn(async move {
            let result = oxvif::discovery::probe_result_on(timeout, &[address]).await;
            (label, result)
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
    password: Option<String>,
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
