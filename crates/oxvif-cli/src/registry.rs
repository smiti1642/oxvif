use std::{
    collections::BTreeMap,
    env,
    fs::{self, File, OpenOptions},
    io::Write,
    net::IpAddr,
    path::{Path, PathBuf},
};

use atomic_write_file::AtomicWriteFile;
use directories::BaseDirs;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::AppError;

pub const REGISTRY_VERSION: u32 = 1;
const REGISTRY_FILE_NAME: &str = "devices.toml";
const LOCK_FILE_NAME: &str = "devices.lock";

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
            Ok(device.view(id))
        })
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
        let registry: RegistryFile = toml::from_str(&contents).map_err(|error| {
            AppError::registry_corrupt(format!(
                "Failed to parse {}: {error}",
                self.registry_path.display()
            ))
        })?;
        if registry.version != REGISTRY_VERSION {
            return Err(AppError::registry_version(
                registry.version,
                REGISTRY_VERSION,
            ));
        }
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
        let result = operation(&mut registry)?;
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
}

impl Default for RegistryFile {
    fn default() -> Self {
        Self {
            version: REGISTRY_VERSION,
            current_device: None,
            devices: BTreeMap::new(),
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
            has_credentials: self.credential_ref.is_some(),
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
}

pub fn validate_device_id(id: &str) -> Result<(), AppError> {
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
            "Invalid device ID `{id}`; expected [a-z0-9][a-z0-9_-]*."
        )))
    }
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
        assert!(contents.starts_with("version = 1"));
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
}
