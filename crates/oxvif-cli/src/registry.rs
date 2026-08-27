use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, File, OpenOptions},
    io::Write,
    net::IpAddr,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use atomic_write_file::AtomicWriteFile;
use directories::BaseDirs;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    AppError, CredentialProfileView, DeviceFilter, DiscoveryFilter, DiscoveryRecord,
    DiscoverySnapshotSummary, DiscoverySnapshotView, GroupMemberView, GroupView, NewGroup,
    NewSavedView, SavedView,
    inventory::{device_matches, discovery_matches},
};

pub const REGISTRY_VERSION: u32 = 3;
const REGISTRY_FILE_NAME: &str = "devices.toml";
const LOCK_FILE_NAME: &str = "devices.lock";
const SNAPSHOTS_DIR_NAME: &str = "snapshots";
const SNAPSHOT_FILE_VERSION: u32 = 1;

/// Safe, serializable view of one saved ONVIF device.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_source: Option<String>,
    pub credential_availability: String,
    pub has_credentials: bool,
    pub tags: Vec<String>,
}

/// Fields accepted when a device is first registered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewDevice {
    pub id: String,
    pub name: Option<String>,
    pub target: String,
    pub tags: Vec<String>,
}

/// Optional fields accepted by `device update`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeviceUpdate {
    pub name: Option<String>,
    pub target: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// Metadata returned by a live device and cached by `device refresh`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceMetadata {
    pub manufacturer: String,
    pub model: String,
    pub firmware_version: String,
    pub serial_number: String,
}

#[derive(Clone, Debug)]
pub struct RegistryStore {
    config_dir: PathBuf,
    registry_path: PathBuf,
    lock_path: PathBuf,
    snapshots_dir: PathBuf,
}

impl RegistryStore {
    /// Resolve the user-scoped oxvif configuration directory.
    ///
    /// `OXVIF_CONFIG_DIR` is an explicit override used by tests, containers,
    /// and callers that require isolated state.
    pub fn system() -> Result<Self, AppError> {
        let config_dir = match env::var_os("OXVIF_CONFIG_DIR") {
            Some(path) if !path.is_empty() => PathBuf::from(path),
            _ => BaseDirs::new()
                .map(|directories| directories.config_dir().join("oxvif"))
                .ok_or_else(|| {
                    AppError::config_unavailable(
                        "The platform configuration directory could not be resolved.",
                    )
                })?,
        };
        Ok(Self::at(config_dir))
    }

    /// Construct an isolated store at a caller-supplied directory.
    pub fn at(config_dir: impl Into<PathBuf>) -> Self {
        let config_dir = config_dir.into();
        Self {
            registry_path: config_dir.join(REGISTRY_FILE_NAME),
            lock_path: config_dir.join(LOCK_FILE_NAME),
            snapshots_dir: config_dir.join(SNAPSHOTS_DIR_NAME),
            config_dir,
        }
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn list(&self) -> Result<(Vec<DeviceView>, Option<String>), AppError> {
        let registry = self.load_unlocked()?;
        let devices = registry
            .devices
            .iter()
            .map(|(id, device)| device.view(id))
            .collect();
        Ok((devices, registry.current_device))
    }

    pub fn get(&self, id: &str) -> Result<DeviceView, AppError> {
        validate_device_id(id)?;
        let registry = self.load_unlocked()?;
        registry
            .devices
            .get(id)
            .map(|device| device.view(id))
            .ok_or_else(|| AppError::device_not_found(id))
    }

    pub(crate) fn get_stored(&self, id: &str) -> Result<StoredDevice, AppError> {
        validate_device_id(id)?;
        self.load_unlocked()?
            .devices
            .remove(id)
            .ok_or_else(|| AppError::device_not_found(id))
    }

    pub fn add(&self, new_device: NewDevice) -> Result<DeviceView, AppError> {
        validate_device_id(&new_device.id)?;
        let target = normalize_target(&new_device.target)?;
        let id = new_device.id;
        let name = normalized_name(new_device.name.as_deref(), &id)?;
        let tags = normalize_tags(new_device.tags)?;

        self.mutate(|registry| {
            if registry.devices.contains_key(&id) {
                return Err(AppError::device_exists(&id));
            }
            let device = StoredDevice {
                name,
                target,
                device_uuid: None,
                manufacturer: None,
                model: None,
                firmware_version: None,
                serial_number: None,
                username: None,
                credential_ref: None,
                credential_profile: None,
                tags,
            };
            let view = device.view(&id);
            registry.devices.insert(id, device);
            Ok(view)
        })
    }

    pub fn update(&self, id: &str, update: DeviceUpdate) -> Result<DeviceView, AppError> {
        validate_device_id(id)?;
        if update.name.is_none() && update.target.is_none() && update.tags.is_none() {
            return Err(AppError::invalid_argument(
                "`device update` requires --name, --target, or --tag.",
            ));
        }
        let name = update
            .name
            .as_deref()
            .map(|name| normalized_name(Some(name), id))
            .transpose()?;
        let target = update.target.as_deref().map(normalize_target).transpose()?;
        let tags = update.tags.map(normalize_tags).transpose()?;

        self.mutate(|registry| {
            let device = registry
                .devices
                .get_mut(id)
                .ok_or_else(|| AppError::device_not_found(id))?;
            if let Some(name) = name {
                device.name = name;
            }
            if let Some(target) = target {
                device.target = target;
            }
            if let Some(tags) = tags {
                device.tags = tags;
            }
            Ok(device.view(id))
        })
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<DeviceView, AppError> {
        self.update(
            id,
            DeviceUpdate {
                name: Some(name.to_owned()),
                ..DeviceUpdate::default()
            },
        )
    }

    pub(crate) fn remove(&self, id: &str) -> Result<StoredDevice, AppError> {
        validate_device_id(id)?;
        self.mutate(|registry| {
            let removed = registry
                .devices
                .remove(id)
                .ok_or_else(|| AppError::device_not_found(id))?;
            if registry.current_device.as_deref() == Some(id) {
                registry.current_device = None;
            }
            for group in registry.groups.values_mut() {
                group.members.retain(|_, device_id| device_id != id);
            }
            Ok(removed)
        })
    }

    pub fn set_current(&self, id: &str) -> Result<DeviceView, AppError> {
        validate_device_id(id)?;
        self.mutate(|registry| {
            let device = registry
                .devices
                .get(id)
                .ok_or_else(|| AppError::device_not_found(id))?;
            let view = device.view(id);
            registry.current_device = Some(id.to_owned());
            Ok(view)
        })
    }

    pub fn current(&self) -> Result<Option<DeviceView>, AppError> {
        let registry = self.load_unlocked()?;
        registry
            .current_device
            .as_deref()
            .map(|id| {
                registry
                    .devices
                    .get(id)
                    .map(|device| device.view(id))
                    .ok_or_else(|| {
                        AppError::registry_corrupt(format!(
                            "Current device `{id}` does not exist in the registry."
                        ))
                    })
            })
            .transpose()
    }

    /// Resolve either a canonical device ID or `group-id/local-alias`.
    pub fn resolve_device_selector(&self, selector: &str) -> Result<String, AppError> {
        if let Some((group_id, alias)) = selector.split_once('/') {
            if alias.contains('/') {
                return Err(AppError::invalid_argument(
                    "A grouped device selector must be `group-id/local-alias`.",
                ));
            }
            validate_resource_id("group", group_id)?;
            validate_resource_id("Group-local alias", alias)?;
            let registry = self.load_unlocked()?;
            let group = registry
                .groups
                .get(group_id)
                .ok_or_else(|| AppError::resource_not_found("group", group_id))?;
            return group
                .members
                .get(alias)
                .cloned()
                .ok_or_else(|| AppError::resource_not_found("Group-local alias", selector));
        }
        validate_device_id(selector)?;
        self.get(selector)?;
        Ok(selector.to_owned())
    }

    pub fn list_groups(&self) -> Result<Vec<GroupView>, AppError> {
        Ok(self
            .load_unlocked()?
            .groups
            .iter()
            .map(|(id, group)| group.view(id))
            .collect())
    }

    pub fn get_group(&self, id: &str) -> Result<GroupView, AppError> {
        validate_resource_id("group", id)?;
        self.load_unlocked()?
            .groups
            .get(id)
            .map(|group| group.view(id))
            .ok_or_else(|| AppError::resource_not_found("group", id))
    }

    pub fn create_group(&self, group: NewGroup) -> Result<GroupView, AppError> {
        validate_resource_id("group", &group.id)?;
        let id = group.id;
        let name = normalized_name(group.name.as_deref(), &id)?;
        self.mutate(|registry| {
            if registry.groups.contains_key(&id) {
                return Err(AppError::resource_exists("group", &id));
            }
            let group = StoredGroup {
                name,
                members: BTreeMap::new(),
            };
            let view = group.view(&id);
            registry.groups.insert(id, group);
            Ok(view)
        })
    }

    pub fn delete_group(&self, id: &str) -> Result<GroupView, AppError> {
        validate_resource_id("group", id)?;
        self.mutate(|registry| {
            registry
                .groups
                .remove(id)
                .map(|group| group.view(id))
                .ok_or_else(|| AppError::resource_not_found("group", id))
        })
    }

    pub fn add_group_member(
        &self,
        group_id: &str,
        device_id: &str,
        alias: &str,
    ) -> Result<GroupView, AppError> {
        validate_resource_id("group", group_id)?;
        validate_device_id(device_id)?;
        validate_resource_id("Group-local alias", alias)?;
        self.mutate(|registry| {
            if !registry.devices.contains_key(device_id) {
                return Err(AppError::device_not_found(device_id));
            }
            let group = registry
                .groups
                .get_mut(group_id)
                .ok_or_else(|| AppError::resource_not_found("group", group_id))?;
            if let Some(existing) = group.members.get(alias) {
                return Err(AppError::resource_in_use(format!(
                    "Alias `{group_id}/{alias}` already selects device `{existing}`."
                )));
            }
            if let Some((existing_alias, _)) = group
                .members
                .iter()
                .find(|(_, member_device_id)| member_device_id.as_str() == device_id)
            {
                return Err(AppError::resource_in_use(format!(
                    "Device `{device_id}` is already in group `{group_id}` as `{existing_alias}`."
                )));
            }
            group.members.insert(alias.to_owned(), device_id.to_owned());
            Ok(group.view(group_id))
        })
    }

    pub fn remove_group_member(&self, group_id: &str, alias: &str) -> Result<GroupView, AppError> {
        validate_resource_id("group", group_id)?;
        validate_resource_id("Group-local alias", alias)?;
        self.mutate(|registry| {
            let group = registry
                .groups
                .get_mut(group_id)
                .ok_or_else(|| AppError::resource_not_found("group", group_id))?;
            if group.members.remove(alias).is_none() {
                return Err(AppError::resource_not_found(
                    "Group-local alias",
                    &format!("{group_id}/{alias}"),
                ));
            }
            Ok(group.view(group_id))
        })
    }

    pub fn list_views(&self) -> Result<Vec<SavedView>, AppError> {
        Ok(self
            .load_unlocked()?
            .views
            .iter()
            .map(|(id, view)| view.view(id))
            .collect())
    }

    pub fn get_view(&self, id: &str) -> Result<SavedView, AppError> {
        validate_resource_id("view", id)?;
        self.load_unlocked()?
            .views
            .get(id)
            .map(|view| view.view(id))
            .ok_or_else(|| AppError::resource_not_found("view", id))
    }

    pub fn create_view(&self, view: NewSavedView) -> Result<SavedView, AppError> {
        validate_resource_id("view", &view.id)?;
        if view.filters.is_empty() {
            return Err(AppError::invalid_argument(
                "A dynamic View requires at least one --filter field=value clause.",
            ));
        }
        let id = view.id;
        let name = normalized_name(view.name.as_deref(), &id)?;
        let mut filters = view.filters;
        filters.sort();
        filters.dedup();
        self.mutate(|registry| {
            if registry.views.contains_key(&id) {
                return Err(AppError::resource_exists("view", &id));
            }
            let view = StoredView {
                name,
                filters,
                match_mode: view.match_mode,
            };
            let result = view.view(&id);
            registry.views.insert(id, view);
            Ok(result)
        })
    }

    pub fn delete_view(&self, id: &str) -> Result<SavedView, AppError> {
        validate_resource_id("view", id)?;
        self.mutate(|registry| {
            registry
                .views
                .remove(id)
                .map(|view| view.view(id))
                .ok_or_else(|| AppError::resource_not_found("view", id))
        })
    }

    pub fn evaluate_view(&self, id: &str) -> Result<Vec<DeviceView>, AppError> {
        self.evaluate_view_explained(id).map(|(devices, _)| devices)
    }

    pub fn evaluate_view_explained(
        &self,
        id: &str,
    ) -> Result<(Vec<DeviceView>, crate::ViewExplanation), AppError> {
        validate_resource_id("view", id)?;
        let registry = self.load_unlocked()?;
        let view = registry
            .views
            .get(id)
            .ok_or_else(|| AppError::resource_not_found("view", id))?;
        let all_devices = registry
            .devices
            .iter()
            .map(|(device_id, device)| device.view(device_id))
            .collect::<Vec<_>>();
        let devices = all_devices
            .iter()
            .filter(|device| device_matches(device, &view.filters, view.match_mode))
            .cloned()
            .collect::<Vec<_>>();
        let filters = view
            .filters
            .iter()
            .map(|filter| {
                let matched_devices = all_devices
                    .iter()
                    .filter(|device| {
                        device_matches(device, std::slice::from_ref(filter), crate::MatchMode::All)
                    })
                    .count();
                crate::FilterExplanation {
                    filter: filter.clone(),
                    matched_devices,
                    unmatched_devices: all_devices.len() - matched_devices,
                }
            })
            .collect();
        let explanation = crate::ViewExplanation {
            evaluated_devices: all_devices.len(),
            matched_devices: devices.len(),
            filters,
        };
        Ok((devices, explanation))
    }

    pub fn set_credentials(
        &self,
        id: &str,
        username: &str,
        credential_ref: &str,
    ) -> Result<DeviceView, AppError> {
        validate_device_id(id)?;
        let username = username.trim();
        if username.is_empty() {
            return Err(AppError::invalid_argument("Username must not be empty."));
        }
        self.mutate(|registry| {
            let device = registry
                .devices
                .get_mut(id)
                .ok_or_else(|| AppError::device_not_found(id))?;
            device.username = Some(username.to_owned());
            device.credential_ref = Some(credential_ref.to_owned());
            device.credential_profile = None;
            Ok(device.view(id))
        })
    }

    pub fn clear_credentials(&self, id: &str) -> Result<DeviceView, AppError> {
        validate_device_id(id)?;
        self.mutate(|registry| {
            let device = registry
                .devices
                .get_mut(id)
                .ok_or_else(|| AppError::device_not_found(id))?;
            device.username = None;
            device.credential_ref = None;
            device.credential_profile = None;
            Ok(device.view(id))
        })
    }

    pub fn list_credential_profiles(&self) -> Result<Vec<CredentialProfileView>, AppError> {
        Ok(self
            .load_unlocked()?
            .credential_profiles
            .iter()
            .map(|(id, profile)| profile.view(id))
            .collect())
    }

    pub fn get_credential_profile(&self, id: &str) -> Result<CredentialProfileView, AppError> {
        validate_resource_id("credential profile", id)?;
        self.load_unlocked()?
            .credential_profiles
            .get(id)
            .map(|profile| profile.view(id))
            .ok_or_else(|| AppError::resource_not_found("credential profile", id))
    }

    pub(crate) fn get_stored_credential_profile(
        &self,
        id: &str,
    ) -> Result<StoredCredentialProfile, AppError> {
        validate_resource_id("credential profile", id)?;
        self.load_unlocked()?
            .credential_profiles
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::resource_not_found("credential profile", id))
    }

    pub fn set_credential_profile(
        &self,
        id: &str,
        username: &str,
        credential_ref: &str,
    ) -> Result<CredentialProfileView, AppError> {
        validate_resource_id("credential profile", id)?;
        let username = username.trim();
        if username.is_empty() {
            return Err(AppError::invalid_argument("Username must not be empty."));
        }
        self.mutate(|registry| {
            let profile = StoredCredentialProfile {
                username: username.to_owned(),
                credential_ref: credential_ref.to_owned(),
            };
            let view = profile.view(id);
            registry.credential_profiles.insert(id.to_owned(), profile);
            Ok(view)
        })
    }

    pub(crate) fn remove_credential_profile(
        &self,
        id: &str,
    ) -> Result<StoredCredentialProfile, AppError> {
        validate_resource_id("credential profile", id)?;
        self.mutate(|registry| {
            let users: Vec<&str> = registry
                .devices
                .iter()
                .filter(|(_, device)| device.credential_profile.as_deref() == Some(id))
                .map(|(device_id, _)| device_id.as_str())
                .collect();
            if !users.is_empty() {
                return Err(AppError::resource_in_use(format!(
                    "Credential profile `{id}` is used by device(s): {}.",
                    users.join(", ")
                )));
            }
            registry
                .credential_profiles
                .remove(id)
                .ok_or_else(|| AppError::resource_not_found("credential profile", id))
        })
    }

    pub fn assign_credential_profile(
        &self,
        device_id: &str,
        profile_id: &str,
    ) -> Result<DeviceView, AppError> {
        validate_device_id(device_id)?;
        validate_resource_id("credential profile", profile_id)?;
        self.mutate(|registry| {
            if !registry.credential_profiles.contains_key(profile_id) {
                return Err(AppError::resource_not_found(
                    "credential profile",
                    profile_id,
                ));
            }
            let device = registry
                .devices
                .get_mut(device_id)
                .ok_or_else(|| AppError::device_not_found(device_id))?;
            if device.credential_ref.is_some() {
                return Err(AppError::resource_in_use(format!(
                    "Device `{device_id}` has a device-specific credential; delete it before assigning profile `{profile_id}`."
                )));
            }
            device.username = None;
            device.credential_ref = None;
            device.credential_profile = Some(profile_id.to_owned());
            Ok(device.view(device_id))
        })
    }

    pub fn save_discovery_snapshot(
        &self,
        id: &str,
        devices: Vec<DiscoveryRecord>,
    ) -> Result<DiscoverySnapshotView, AppError> {
        validate_resource_id("discovery snapshot", id)?;
        let devices = normalize_discovery_records(devices);
        let saved_at_unix_ms = unix_millis()?;
        self.mutate(|registry| {
            if registry.discovery_snapshots.contains_key(id) {
                return Err(AppError::resource_exists("discovery snapshot", id));
            }
            let snapshot = StoredDiscoverySnapshot {
                saved_at_unix_ms,
                devices,
            };
            let view = snapshot.view(id, &[]);
            registry
                .discovery_snapshots
                .insert(id.to_owned(), SnapshotEntry::Embedded(snapshot));
            Ok(view)
        })
    }

    pub fn list_discovery_snapshots(&self) -> Result<Vec<DiscoverySnapshotSummary>, AppError> {
        Ok(self
            .load_unlocked()?
            .discovery_snapshots
            .iter()
            .map(|(id, snapshot)| snapshot.summary(id))
            .collect())
    }

    pub fn get_discovery_snapshot(
        &self,
        id: &str,
        filters: &[DiscoveryFilter],
    ) -> Result<DiscoverySnapshotView, AppError> {
        validate_resource_id("discovery snapshot", id)?;
        let registry = self.load_unlocked()?;
        let snapshot = registry
            .discovery_snapshots
            .get(id)
            .ok_or_else(|| AppError::resource_not_found("discovery snapshot", id))?;
        match snapshot {
            SnapshotEntry::Embedded(snapshot) => Ok(snapshot.view(id, filters)),
            SnapshotEntry::External(summary) => {
                let snapshot = self.read_snapshot_file(id)?;
                if snapshot.saved_at_unix_ms != summary.saved_at_unix_ms
                    || snapshot.devices.len() != summary.device_count
                {
                    return Err(AppError::registry_corrupt(format!(
                        "Discovery snapshot `{id}` does not match its registry index."
                    )));
                }
                Ok(snapshot.view(id, filters))
            }
        }
    }

    pub fn remove_discovery_snapshot(
        &self,
        id: &str,
    ) -> Result<DiscoverySnapshotSummary, AppError> {
        validate_resource_id("discovery snapshot", id)?;
        let removed = self.mutate(|registry| {
            registry
                .discovery_snapshots
                .remove(id)
                .map(|snapshot| snapshot.summary(id))
                .ok_or_else(|| AppError::resource_not_found("discovery snapshot", id))
        })?;
        let path = self.snapshot_path(id);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(AppError::registry_io(format!(
                    "Removed snapshot `{id}` from the registry but failed to delete {}: {error}",
                    path.display()
                )));
            }
        }
        Ok(removed)
    }

    pub fn update_metadata(
        &self,
        id: &str,
        metadata: DeviceMetadata,
    ) -> Result<DeviceView, AppError> {
        validate_device_id(id)?;
        self.mutate(|registry| {
            let device = registry
                .devices
                .get_mut(id)
                .ok_or_else(|| AppError::device_not_found(id))?;
            device.manufacturer = Some(metadata.manufacturer);
            device.model = Some(metadata.model);
            device.firmware_version = Some(metadata.firmware_version);
            device.serial_number = Some(metadata.serial_number);
            Ok(device.view(id))
        })
    }

    fn snapshot_path(&self, id: &str) -> PathBuf {
        self.snapshots_dir.join(format!("{id}.json"))
    }

    fn externalize_snapshots(&self, registry: &mut RegistryFile) -> Result<(), AppError> {
        let embedded: Vec<(String, StoredDiscoverySnapshot)> = registry
            .discovery_snapshots
            .iter()
            .filter_map(|(id, entry)| match entry {
                SnapshotEntry::Embedded(snapshot) => Some((id.clone(), snapshot.clone())),
                SnapshotEntry::External(_) => None,
            })
            .collect();
        if embedded.is_empty() {
            return Ok(());
        }
        fs::create_dir_all(&self.snapshots_dir).map_err(|error| {
            AppError::registry_io(format!(
                "Failed to create {}: {error}",
                self.snapshots_dir.display()
            ))
        })?;
        for (id, snapshot) in embedded {
            self.write_snapshot_file(&id, &snapshot)?;
            registry.discovery_snapshots.insert(
                id,
                SnapshotEntry::External(SnapshotIndexEntry {
                    saved_at_unix_ms: snapshot.saved_at_unix_ms,
                    device_count: snapshot.devices.len(),
                }),
            );
        }
        Ok(())
    }

    fn write_snapshot_file(
        &self,
        id: &str,
        snapshot: &StoredDiscoverySnapshot,
    ) -> Result<(), AppError> {
        let document = SnapshotFile {
            version: SNAPSHOT_FILE_VERSION,
            id: id.to_owned(),
            saved_at_unix_ms: snapshot.saved_at_unix_ms,
            devices: snapshot.devices.clone(),
        };
        let serialized = serde_json::to_vec_pretty(&document)
            .map_err(|error| AppError::serialization_failed(error.to_string()))?;
        let path = self.snapshot_path(id);
        let mut destination = AtomicWriteFile::options().open(&path).map_err(|error| {
            AppError::registry_io(format!("Failed to prepare {}: {error}", path.display()))
        })?;
        destination.write_all(&serialized).map_err(|error| {
            AppError::registry_io(format!("Failed to write {}: {error}", path.display()))
        })?;
        destination.commit().map_err(|error| {
            AppError::registry_io(format!(
                "Failed to atomically replace {}: {error}",
                path.display()
            ))
        })
    }

    fn read_snapshot_file(&self, id: &str) -> Result<StoredDiscoverySnapshot, AppError> {
        let path = self.snapshot_path(id);
        let contents = fs::read(&path).map_err(|error| {
            AppError::registry_io(format!("Failed to read {}: {error}", path.display()))
        })?;
        let document: SnapshotFile = serde_json::from_slice(&contents).map_err(|error| {
            AppError::registry_corrupt(format!("Failed to parse {}: {error}", path.display()))
        })?;
        if document.version != SNAPSHOT_FILE_VERSION || document.id != id {
            return Err(AppError::registry_corrupt(format!(
                "Discovery snapshot file {} has incompatible identity or version.",
                path.display()
            )));
        }
        Ok(StoredDiscoverySnapshot {
            saved_at_unix_ms: document.saved_at_unix_ms,
            devices: document.devices,
        })
    }

    fn load_unlocked(&self) -> Result<RegistryFile, AppError> {
        let contents = match fs::read_to_string(&self.registry_path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(RegistryFile::default());
            }
            Err(error) => {
                return Err(AppError::registry_io(format!(
                    "Failed to read {}: {error}",
                    self.registry_path.display()
                )));
            }
        };
        let mut registry: RegistryFile = toml::from_str(&contents).map_err(|error| {
            AppError::registry_corrupt(format!(
                "Failed to parse {}: {error}",
                self.registry_path.display()
            ))
        })?;
        if registry.version > REGISTRY_VERSION || registry.version == 0 {
            return Err(AppError::registry_version(
                registry.version,
                REGISTRY_VERSION,
            ));
        }
        registry.version = REGISTRY_VERSION;
        validate_registry(&registry)?;
        Ok(registry)
    }

    fn mutate<T>(
        &self,
        operation: impl FnOnce(&mut RegistryFile) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        fs::create_dir_all(&self.config_dir).map_err(|error| {
            AppError::registry_io(format!(
                "Failed to create {}: {error}",
                self.config_dir.display()
            ))
        })?;
        let lock = open_lock_file(&self.lock_path)?;
        FileExt::lock_exclusive(&lock).map_err(|error| {
            AppError::registry_io(format!(
                "Failed to lock {}: {error}",
                self.lock_path.display()
            ))
        })?;

        let mut registry = self.load_unlocked()?;
        self.externalize_snapshots(&mut registry)?;
        let result = operation(&mut registry)?;
        self.externalize_snapshots(&mut registry)?;
        let serialized = toml::to_string_pretty(&registry).map_err(|error| {
            AppError::registry_io(format!("Failed to serialize the device registry: {error}"))
        })?;
        let mut destination = AtomicWriteFile::options()
            .open(&self.registry_path)
            .map_err(|error| {
                AppError::registry_io(format!(
                    "Failed to prepare {}: {error}",
                    self.registry_path.display()
                ))
            })?;
        destination
            .write_all(serialized.as_bytes())
            .map_err(|error| {
                AppError::registry_io(format!(
                    "Failed to write {}: {error}",
                    self.registry_path.display()
                ))
            })?;
        destination.commit().map_err(|error| {
            AppError::registry_io(format!(
                "Failed to atomically replace {}: {error}",
                self.registry_path.display()
            ))
        })?;
        FileExt::unlock(&lock).map_err(|error| {
            AppError::registry_io(format!(
                "Failed to unlock {}: {error}",
                self.lock_path.display()
            ))
        })?;
        Ok(result)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RegistryFile {
    version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_device: Option<String>,
    #[serde(default)]
    devices: BTreeMap<String, StoredDevice>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    groups: BTreeMap<String, StoredGroup>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    views: BTreeMap<String, StoredView>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    discovery_snapshots: BTreeMap<String, SnapshotEntry>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    credential_profiles: BTreeMap<String, StoredCredentialProfile>,
}

impl Default for RegistryFile {
    fn default() -> Self {
        Self {
            version: REGISTRY_VERSION,
            current_device: None,
            devices: BTreeMap::new(),
            groups: BTreeMap::new(),
            views: BTreeMap::new(),
            discovery_snapshots: BTreeMap::new(),
            credential_profiles: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct StoredDevice {
    name: String,
    target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    firmware_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    serial_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
}

impl StoredDevice {
    pub(crate) fn view(&self, id: &str) -> DeviceView {
        DeviceView {
            id: id.to_owned(),
            name: self.name.clone(),
            target: self.target.clone(),
            device_uuid: self.device_uuid.clone(),
            manufacturer: self.manufacturer.clone(),
            model: self.model.clone(),
            firmware_version: self.firmware_version.clone(),
            serial_number: self.serial_number.clone(),
            username: self.username.clone(),
            credential_profile: self.credential_profile.clone(),
            credential_source: if self.credential_profile.is_some() {
                Some("profile".to_owned())
            } else if self.credential_ref.is_some() {
                Some("device".to_owned())
            } else {
                None
            },
            credential_availability: if self.credential_ref.is_some()
                || self.credential_profile.is_some()
            {
                "unverified".to_owned()
            } else {
                "none".to_owned()
            },
            has_credentials: self.credential_ref.is_some() || self.credential_profile.is_some(),
            tags: self.tags.clone(),
        }
    }

    pub(crate) fn target(&self) -> &str {
        &self.target
    }

    pub(crate) fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    pub(crate) fn credential_ref(&self) -> Option<&str> {
        self.credential_ref.as_deref()
    }

    pub(crate) fn credential_profile(&self) -> Option<&str> {
        self.credential_profile.as_deref()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredGroup {
    name: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    members: BTreeMap<String, String>,
}

impl StoredGroup {
    fn view(&self, id: &str) -> GroupView {
        GroupView {
            id: id.to_owned(),
            name: self.name.clone(),
            members: self
                .members
                .iter()
                .map(|(alias, device_id)| GroupMemberView {
                    alias: alias.clone(),
                    device_id: device_id.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredView {
    name: String,
    filters: Vec<DeviceFilter>,
    #[serde(default)]
    match_mode: crate::MatchMode,
}

impl StoredView {
    fn view(&self, id: &str) -> SavedView {
        SavedView {
            id: id.to_owned(),
            name: self.name.clone(),
            filters: self.filters.clone(),
            match_mode: self.match_mode,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredDiscoverySnapshot {
    saved_at_unix_ms: u64,
    devices: Vec<DiscoveryRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum SnapshotEntry {
    Embedded(StoredDiscoverySnapshot),
    External(SnapshotIndexEntry),
}

impl SnapshotEntry {
    fn summary(&self, id: &str) -> DiscoverySnapshotSummary {
        match self {
            Self::Embedded(snapshot) => snapshot.summary(id),
            Self::External(summary) => DiscoverySnapshotSummary {
                id: id.to_owned(),
                saved_at_unix_ms: summary.saved_at_unix_ms,
                device_count: summary.device_count,
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SnapshotIndexEntry {
    saved_at_unix_ms: u64,
    device_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SnapshotFile {
    version: u32,
    id: String,
    saved_at_unix_ms: u64,
    devices: Vec<DiscoveryRecord>,
}

impl StoredDiscoverySnapshot {
    fn summary(&self, id: &str) -> DiscoverySnapshotSummary {
        DiscoverySnapshotSummary {
            id: id.to_owned(),
            saved_at_unix_ms: self.saved_at_unix_ms,
            device_count: self.devices.len(),
        }
    }

    fn view(&self, id: &str, filters: &[DiscoveryFilter]) -> DiscoverySnapshotView {
        DiscoverySnapshotView {
            id: id.to_owned(),
            saved_at_unix_ms: self.saved_at_unix_ms,
            devices: self
                .devices
                .iter()
                .filter(|device| discovery_matches(device, filters))
                .cloned()
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct StoredCredentialProfile {
    username: String,
    credential_ref: String,
}

impl StoredCredentialProfile {
    fn view(&self, id: &str) -> CredentialProfileView {
        CredentialProfileView {
            id: id.to_owned(),
            username: self.username.clone(),
            has_credentials: true,
            credential_availability: "unverified".to_owned(),
        }
    }

    pub(crate) fn username(&self) -> &str {
        &self.username
    }

    pub(crate) fn credential_ref(&self) -> &str {
        &self.credential_ref
    }
}

pub fn validate_device_id(id: &str) -> Result<(), AppError> {
    validate_resource_id("device", id)
}

fn validate_resource_id(kind: &str, id: &str) -> Result<(), AppError> {
    let valid = !id.is_empty()
        && id
            .bytes()
            .next()
            .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && id.bytes().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == b'-'
                || character == b'_'
        });
    if valid {
        Ok(())
    } else {
        Err(AppError::invalid_argument(format!(
            "Invalid {kind} ID `{id}`; expected [a-z0-9][a-z0-9_-]*."
        )))
    }
}

fn normalize_discovery_records(mut devices: Vec<DiscoveryRecord>) -> Vec<DiscoveryRecord> {
    for device in &mut devices {
        device.types.sort();
        device.types.dedup();
        device.scopes.sort();
        device.scopes.dedup();
        device.xaddrs = device
            .xaddrs
            .iter()
            .map(|xaddr| redact_url_credentials(xaddr))
            .collect();
        device.xaddrs.sort();
        device.xaddrs.dedup();
    }
    devices.sort_by(|left, right| left.endpoint.cmp(&right.endpoint));
    devices.dedup_by(|left, right| left.endpoint == right.endpoint);
    devices
}

fn redact_url_credentials(value: &str) -> String {
    let Ok(mut url) = Url::parse(value) else {
        return value.to_owned();
    };
    if !url.username().is_empty() || url.password().is_some() {
        let _ = url.set_username("");
        let _ = url.set_password(None);
    }
    url.to_string()
}

fn validate_registry(registry: &RegistryFile) -> Result<(), AppError> {
    if let Some(current) = &registry.current_device
        && !registry.devices.contains_key(current)
    {
        return Err(AppError::registry_corrupt(format!(
            "Current device `{current}` does not exist."
        )));
    }
    for (group_id, group) in &registry.groups {
        let mut device_ids = BTreeSet::new();
        for (alias, device_id) in &group.members {
            if !registry.devices.contains_key(device_id) {
                return Err(AppError::registry_corrupt(format!(
                    "Group selector `{group_id}/{alias}` references missing device `{device_id}`."
                )));
            }
            if !device_ids.insert(device_id) {
                return Err(AppError::registry_corrupt(format!(
                    "Group `{group_id}` contains device `{device_id}` more than once."
                )));
            }
        }
    }
    for (device_id, device) in &registry.devices {
        if let Some(profile_id) = &device.credential_profile
            && !registry.credential_profiles.contains_key(profile_id)
        {
            return Err(AppError::registry_corrupt(format!(
                "Device `{device_id}` references missing credential profile `{profile_id}`."
            )));
        }
    }
    Ok(())
}

fn unix_millis() -> Result<u64, AppError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| AppError::internal(format!("System clock predates Unix epoch: {error}")))?
        .as_millis();
    Ok(u64::try_from(millis).unwrap_or(u64::MAX))
}

pub fn normalize_target(target: &str) -> Result<String, AppError> {
    let target = target.trim();
    if target.is_empty() {
        return Err(AppError::invalid_argument(
            "Device target must not be empty.",
        ));
    }
    let candidate = if target.contains("://") {
        target.to_owned()
    } else if target.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv6()) {
        format!("http://[{target}]/onvif/device_service")
    } else {
        format!("http://{target}/onvif/device_service")
    };
    let parsed = Url::parse(&candidate).map_err(|error| {
        AppError::invalid_argument(format!("Invalid target `{target}`: {error}"))
    })?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host().is_none() {
        return Err(AppError::invalid_argument(
            "Device target must be an HTTP(S) URL or host/IP address.",
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::invalid_argument(
            "Device targets must not contain URL-embedded credentials.",
        ));
    }
    Ok(parsed.to_string())
}

fn normalized_name(name: Option<&str>, id: &str) -> Result<String, AppError> {
    let name = name.unwrap_or(id).trim();
    if name.is_empty() {
        Err(AppError::invalid_argument(
            "Device display name must not be empty.",
        ))
    } else {
        Ok(name.to_owned())
    }
}

fn normalize_tags(tags: Vec<String>) -> Result<Vec<String>, AppError> {
    let mut tags: Vec<String> = tags
        .into_iter()
        .map(|tag| tag.trim().to_owned())
        .filter(|tag| !tag.is_empty())
        .collect();
    tags.sort();
    tags.dedup();
    if tags.iter().any(|tag| tag.len() > 64) {
        return Err(AppError::invalid_argument(
            "Device tags must be 64 characters or fewer.",
        ));
    }
    Ok(tags)
}

fn open_lock_file(path: &Path) -> Result<File, AppError> {
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| {
            AppError::registry_io(format!("Failed to open {}: {error}", path.display()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_host_and_preserves_explicit_url() {
        assert_eq!(
            normalize_target("192.168.1.20").expect("IP should normalize"),
            "http://192.168.1.20/onvif/device_service"
        );
        assert_eq!(
            normalize_target("http://camera.local:8080/custom").expect("URL should parse"),
            "http://camera.local:8080/custom"
        );
        assert!(normalize_target("http://admin:secret@camera.local/onvif").is_err());
    }

    #[test]
    fn rejects_unstable_device_ids() {
        for id in ["", "FrontDoor", "front door", "front/door", "_front"] {
            assert!(validate_device_id(id).is_err(), "{id} should fail");
        }
        for id in ["front-door", "cam_01", "7th-floor"] {
            assert!(validate_device_id(id).is_ok(), "{id} should pass");
        }
    }

    #[test]
    fn registry_round_trip_is_versioned_and_deterministic() {
        let directory = tempfile::tempdir().expect("temp directory");
        let store = RegistryStore::at(directory.path());
        let added = store
            .add(NewDevice {
                id: "front-door".to_owned(),
                name: Some("Front Door".to_owned()),
                target: "192.168.1.20".to_owned(),
                tags: vec!["outdoor".to_owned(), "entrance".to_owned()],
            })
            .expect("device should be added");

        assert_eq!(added.id, "front-door");
        assert_eq!(added.tags, vec!["entrance", "outdoor"]);
        assert_eq!(store.get("front-door").expect("device should load"), added);
        let contents = fs::read_to_string(directory.path().join(REGISTRY_FILE_NAME))
            .expect("registry should exist");
        assert!(contents.starts_with("version = 3"));
        assert!(!contents.to_ascii_lowercase().contains("password"));
    }

    #[test]
    fn selecting_and_removing_current_device_is_consistent() {
        let directory = tempfile::tempdir().expect("temp directory");
        let store = RegistryStore::at(directory.path());
        store
            .add(NewDevice {
                id: "camera".to_owned(),
                name: None,
                target: "192.168.1.30".to_owned(),
                tags: Vec::new(),
            })
            .expect("device should be added");
        store.set_current("camera").expect("selection should work");
        assert_eq!(
            store.current().expect("current should load").unwrap().id,
            "camera"
        );
        store.remove("camera").expect("removal should work");
        assert!(store.current().expect("current should load").is_none());
    }

    #[test]
    fn newer_registry_version_is_not_overwritten() {
        let directory = tempfile::tempdir().expect("temp directory");
        fs::write(directory.path().join(REGISTRY_FILE_NAME), "version = 999\n")
            .expect("fixture should write");
        let store = RegistryStore::at(directory.path());
        assert!(store.list().is_err());
        let contents = fs::read_to_string(directory.path().join(REGISTRY_FILE_NAME))
            .expect("fixture should remain");
        assert_eq!(contents, "version = 999\n");
    }

    #[test]
    fn version_one_registry_migrates_on_next_write() {
        let directory = tempfile::tempdir().expect("temp directory");
        fs::write(
            directory.path().join(REGISTRY_FILE_NAME),
            r#"version = 1

[devices.legacy]
name = "Legacy"
target = "http://192.0.2.10/onvif/device_service"
"#,
        )
        .expect("v1 fixture should write");
        let store = RegistryStore::at(directory.path());
        assert_eq!(
            store.get("legacy").expect("v1 device should load").name,
            "Legacy"
        );

        store
            .create_group(NewGroup {
                id: "migrated".to_owned(),
                name: None,
            })
            .expect("first write should migrate");
        let contents = fs::read_to_string(directory.path().join(REGISTRY_FILE_NAME))
            .expect("registry should load");
        assert!(contents.starts_with("version = 3"));
        assert!(contents.contains("[devices.legacy]"));
    }

    #[test]
    fn group_alias_resolves_exact_device_and_is_cleaned_on_removal() {
        let directory = tempfile::tempdir().expect("temp directory");
        let store = RegistryStore::at(directory.path());
        store
            .add(NewDevice {
                id: "camera-global".to_owned(),
                name: None,
                target: "192.0.2.20".to_owned(),
                tags: Vec::new(),
            })
            .expect("device should add");
        store
            .create_group(NewGroup {
                id: "taipei-f1".to_owned(),
                name: Some("Taipei floor 1".to_owned()),
            })
            .expect("group should add");
        store
            .add_group_member("taipei-f1", "camera-global", "cam-023")
            .expect("member should add");

        assert_eq!(
            store
                .resolve_device_selector("taipei-f1/cam-023")
                .expect("selector should resolve"),
            "camera-global"
        );
        assert!(
            store
                .add_group_member("taipei-f1", "camera-global", "duplicate")
                .is_err()
        );
        store.remove("camera-global").expect("device should remove");
        assert!(
            store
                .get_group("taipei-f1")
                .expect("group should remain")
                .members
                .is_empty()
        );
    }

    #[test]
    fn dynamic_view_filters_a_205_device_inventory_deterministically() {
        let directory = tempfile::tempdir().expect("temp directory");
        let mut registry = RegistryFile::default();
        for index in 1..=205 {
            let id = format!("cam-{index:03}");
            registry.devices.insert(
                id,
                StoredDevice {
                    name: format!("Camera {index:03}"),
                    target: format!("http://192.168.20.{index}/onvif/device_service"),
                    device_uuid: Some(format!("uuid:camera-{index:03}")),
                    manufacturer: Some(
                        if index % 2 == 0 { "Other" } else { "GeoVision" }.to_owned(),
                    ),
                    model: None,
                    firmware_version: None,
                    serial_number: None,
                    username: None,
                    credential_ref: None,
                    credential_profile: None,
                    tags: vec![if index % 2 == 0 { "indoor" } else { "outdoor" }.to_owned()],
                },
            );
        }
        fs::write(
            directory.path().join(REGISTRY_FILE_NAME),
            toml::to_string_pretty(&registry).expect("fixture should serialize"),
        )
        .expect("fixture should write");
        let store = RegistryStore::at(directory.path());
        store
            .create_view(NewSavedView {
                id: "geovision".to_owned(),
                name: None,
                filters: vec![
                    "manufacturer=GeoVision"
                        .parse()
                        .expect("filter should parse"),
                ],
                match_mode: crate::MatchMode::All,
            })
            .expect("view should create");
        let devices = store
            .evaluate_view("geovision")
            .expect("view should evaluate");
        assert_eq!(devices.len(), 103);
        assert_eq!(devices.first().expect("first device").id, "cam-001");
        assert_eq!(devices.last().expect("last device").id, "cam-205");
    }

    #[test]
    fn discovery_snapshot_is_sorted_deduplicated_and_filterable() {
        let directory = tempfile::tempdir().expect("temp directory");
        let store = RegistryStore::at(directory.path());
        let record = |endpoint: &str, ip: &str| DiscoveryRecord {
            endpoint: endpoint.to_owned(),
            types: vec!["NetworkVideoTransmitter".to_owned()],
            scopes: vec!["onvif://www.onvif.org/location/floor1".to_owned()],
            xaddrs: vec![format!("http://{ip}/onvif/device_service")],
            manufacturer: None,
            model: None,
            firmware_version: None,
            serial_number: None,
        };
        store
            .save_discovery_snapshot(
                "factory-scan",
                vec![
                    record("uuid:b", "192.168.30.2"),
                    record("uuid:a", "192.168.20.1"),
                    record("uuid:a", "192.168.20.1"),
                ],
            )
            .expect("snapshot should save");
        let filtered = store
            .get_discovery_snapshot(
                "factory-scan",
                &["ip-cidr=192.168.20.0/24"
                    .parse()
                    .expect("filter should parse")],
            )
            .expect("snapshot should filter");
        assert_eq!(filtered.devices.len(), 1);
        assert_eq!(filtered.devices[0].endpoint, "uuid:a");
        assert_eq!(
            store
                .list_discovery_snapshots()
                .expect("snapshots should list")[0]
                .device_count,
            2
        );
        let registry_contents =
            fs::read_to_string(directory.path().join(REGISTRY_FILE_NAME)).expect("registry");
        assert!(!registry_contents.contains("uuid:a"));
        assert!(
            directory
                .path()
                .join(SNAPSHOTS_DIR_NAME)
                .join("factory-scan.json")
                .exists()
        );
        assert!(
            store
                .save_discovery_snapshot("factory-scan", vec![record("uuid:c", "192.168.20.3")])
                .is_err(),
            "saving under an existing snapshot ID must not silently replace it"
        );
    }

    #[test]
    fn snapshot_persistence_redacts_url_credentials() {
        let directory = tempfile::tempdir().expect("temp directory");
        let store = RegistryStore::at(directory.path());
        store
            .save_discovery_snapshot(
                "redacted",
                vec![DiscoveryRecord {
                    endpoint: "uuid:redacted".to_owned(),
                    types: Vec::new(),
                    scopes: Vec::new(),
                    xaddrs: vec!["http://admin:secret@192.0.2.60/onvif/device_service".to_owned()],
                    manufacturer: None,
                    model: None,
                    firmware_version: None,
                    serial_number: None,
                }],
            )
            .expect("snapshot should save");
        let contents = fs::read_to_string(
            directory
                .path()
                .join(SNAPSHOTS_DIR_NAME)
                .join("redacted.json"),
        )
        .expect("snapshot should read");
        assert!(!contents.contains("secret"));
        assert!(!contents.contains("admin@"));
    }

    #[test]
    fn version_two_embedded_snapshots_migrate_to_separate_files() {
        let directory = tempfile::tempdir().expect("temp directory");
        fs::write(
            directory.path().join(REGISTRY_FILE_NAME),
            r#"version = 2

[discovery_snapshots.old-scan]
saved_at_unix_ms = 123

[[discovery_snapshots.old-scan.devices]]
endpoint = "uuid:legacy"
xaddrs = ["http://192.0.2.50/onvif/device_service"]
"#,
        )
        .expect("v2 fixture should write");
        let store = RegistryStore::at(directory.path());
        assert_eq!(
            store
                .get_discovery_snapshot("old-scan", &[])
                .expect("embedded snapshot should remain readable")
                .devices
                .len(),
            1
        );

        store
            .create_group(NewGroup {
                id: "migration-trigger".to_owned(),
                name: None,
            })
            .expect("next write should migrate");
        let registry = fs::read_to_string(directory.path().join(REGISTRY_FILE_NAME))
            .expect("registry should read");
        assert!(registry.starts_with("version = 3"));
        assert!(!registry.contains("uuid:legacy"));
        let snapshot = directory
            .path()
            .join(SNAPSHOTS_DIR_NAME)
            .join("old-scan.json");
        assert!(snapshot.exists());
        assert!(
            fs::read_to_string(snapshot)
                .expect("snapshot should read")
                .contains("uuid:legacy")
        );
    }
}
