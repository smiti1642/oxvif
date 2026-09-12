use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{Read, Write},
    net::Ipv4Addr,
    path::{Path, PathBuf},
};

use oxvif::mock::DeviceState;
use serde::{Deserialize, Serialize};

const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_DEVICES: usize = 256;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    #[serde(default = "loopback")]
    pub bind_ip: Ipv4Addr,
    pub advertise_ip: Option<Ipv4Addr>,
    #[serde(default)]
    pub discovery: bool,
    pub discovery_interface: Option<Ipv4Addr>,
    pub devices: Vec<Device>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    pub id: String,
    pub uuid: String,
    pub port: u16,
    pub name: String,
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub state_file: Option<PathBuf>,
}

fn loopback() -> Ipv4Addr {
    Ipv4Addr::LOCALHOST
}

fn uuid() -> String {
    let mut bytes: [u8; 16] = rand::random();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = u128::from_be_bytes(bytes);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        hex >> 96,
        (hex >> 80) & 0xffff,
        (hex >> 64) & 0xffff,
        (hex >> 48) & 0xffff,
        hex & 0xffffffffffff
    )
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}

fn text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn read_bounded(path: &Path) -> Result<String, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| "Cannot open manifest/state file".to_owned())?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read manifest/state file".to_owned())?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("Manifest/state file exceeds 4 MiB".into());
    }
    String::from_utf8(bytes).map_err(|_| "Manifest/state file is not UTF-8".into())
}

impl Manifest {
    pub fn generated(count: usize, base_port: u16) -> Result<Self, String> {
        if !(1..=MAX_DEVICES).contains(&count) {
            return Err("Count must be between 1 and 256".into());
        }
        if base_port == 0 || usize::from(base_port) + count - 1 > usize::from(u16::MAX) {
            return Err("Port range must fit within 1..65535".into());
        }
        let devices = (0..count)
            .map(|i| Device {
                id: format!("camera-{:03}", i + 1),
                uuid: uuid(),
                port: base_port + i as u16,
                name: format!("Camera {:03}", i + 1),
                manufacturer: "oxvif-mock".into(),
                model: format!("MockCamera-{:03}", i + 1),
                serial_number: format!("MOCK-{:03}", i + 1),
                scopes: Vec::new(),
                state_file: None,
            })
            .collect();
        Ok(Self {
            schema_version: 1,
            bind_ip: loopback(),
            advertise_ip: None,
            discovery: false,
            discovery_interface: None,
            devices,
        })
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        // TOML errors may contain source lines with test credentials: do not print them.
        toml::from_str(&read_bounded(path)?)
            .map_err(|_| "Invalid fleet manifest (check fields and TOML syntax)".into())
    }

    pub fn write_new(&self, path: &Path) -> Result<(), String> {
        let content =
            toml::to_string_pretty(self).map_err(|_| "Cannot serialize manifest".to_owned())?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| {
                "Cannot create manifest; destination must not exist and parent must be writable"
                    .to_owned()
            })?;
        if file
            .write_all(content.as_bytes())
            .and_then(|()| file.sync_all())
            .is_err()
        {
            drop(file);
            let _ = std::fs::remove_file(path); // Only our newly created, incomplete file.
            return Err("Cannot finish writing manifest".into());
        }
        Ok(())
    }

    pub fn prepare(&self, path: &Path) -> Result<Vec<DeviceState>, String> {
        if self.schema_version != 1 {
            return Err("Unsupported schema_version; expected 1".into());
        }
        if !(1..=MAX_DEVICES).contains(&self.devices.len()) {
            return Err("Fleet must contain 1..256 devices".into());
        }
        let advertised = self.advertise_ip.unwrap_or(self.bind_ip);
        if advertised.is_unspecified()
            || advertised.is_multicast()
            || advertised.is_broadcast()
            || self.bind_ip.is_multicast()
            || self.bind_ip.is_broadcast()
        {
            return Err("Use a unicast bind_ip and concrete advertise_ip; wildcard binding requires advertise_ip".into());
        }
        if !self.bind_ip.is_unspecified() && advertised != self.bind_ip {
            return Err("advertise_ip must match bind_ip unless binding 0.0.0.0".into());
        }
        let local =
            if_addrs::get_if_addrs().map_err(|_| "Cannot enumerate local interfaces".to_owned())?;
        if !local
            .iter()
            .any(|nic| nic.ip() == std::net::IpAddr::V4(advertised))
        {
            return Err("advertise_ip must be assigned to this host".into());
        }
        if self.discovery {
            if self.discovery_interface != Some(advertised) || advertised.is_loopback() {
                return Err(
                    "Discovery requires discovery_interface equal to the non-loopback advertise_ip"
                        .into(),
                );
            }
        } else if self.discovery_interface.is_some() {
            return Err("discovery_interface requires discovery = true".into());
        }
        let (mut ids, mut uuids, mut ports, mut serials) = (
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
        );
        let mut states = Vec::with_capacity(self.devices.len());
        for (index, device) in self.devices.iter().enumerate() {
            let error = |message: &str| format!("Device {}: {message}", index + 1);
            if device.id.is_empty()
                || device.id.len() > 63
                || !device
                    .id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
            {
                return Err(error(
                    "id must contain 1..63 ASCII letters, digits, '-' or '_'",
                ));
            }
            if !valid_uuid(&device.uuid) || !uuids.insert(device.uuid.to_ascii_lowercase()) {
                return Err(error("invalid or duplicate uuid"));
            }
            if !ids.insert(&device.id) {
                return Err(error("duplicate id"));
            }
            if device.port == 0 || !ports.insert(device.port) {
                return Err(error("zero or duplicate port"));
            }
            if !text(&device.name)
                || !text(&device.manufacturer)
                || !text(&device.model)
                || !text(&device.serial_number)
            {
                return Err(error(
                    "identity fields must be nonempty, control-free and at most 256 bytes",
                ));
            }
            if !serials.insert(&device.serial_number) {
                return Err(error("duplicate serial_number"));
            }
            if device.scopes.len() > 16
                || device.scopes.iter().any(|s| {
                    s.len() > 512
                        || s.chars().any(char::is_whitespace)
                        || s.chars().any(char::is_control)
                        || reqwest::Url::parse(s).is_err()
                })
            {
                return Err(error(
                    "scopes must contain at most 16 absolute URIs of at most 512 bytes without whitespace",
                ));
            }
            let mut state = match &device.state_file {
                Some(file) => {
                    let source = read_bounded(&path.parent().unwrap_or(Path::new(".")).join(file))
                        .map_err(|e| error(&e))?;
                    toml::from_str::<DeviceState>(&source)
                        .map_err(|_| error("invalid DeviceState TOML; factory fallback refused"))?
                }
                None => DeviceState::default(),
            };
            state.hostname = device.id.clone();
            state.info.manufacturer = device.manufacturer.clone();
            state.info.model = device.model.clone();
            state.info.serial_number = device.serial_number.clone();
            state.scopes = device.scopes.clone();
            let mut name_scope = reqwest::Url::parse("onvif://www.onvif.org/name/")
                .map_err(|_| error("invalid scope base"))?;
            name_scope
                .path_segments_mut()
                .map_err(|_| error("invalid scope base"))?
                .pop_if_empty()
                .push(&device.name);
            state.scopes.push(name_scope.to_string());
            states.push(state);
        }
        Ok(states)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("oxvif-fleet-{}", uuid()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn generated_manifest_roundtrip_is_stable_and_never_clobbers() {
        let dir = Scratch::new();
        let path = dir.0.join("lab.toml");
        let manifest = Manifest::generated(4, 18080).unwrap();
        manifest.write_new(&path).unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(
            manifest
                .write_new(&path)
                .unwrap_err()
                .contains("must not exist")
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let loaded = Manifest::load(&path).unwrap();
        let states = loaded.prepare(&path).unwrap();
        assert_eq!(states.len(), 4);
        for (i, state) in states.iter().enumerate() {
            assert_eq!(loaded.devices[i].uuid, manifest.devices[i].uuid);
            assert_eq!(loaded.devices[i].port, 18080 + i as u16);
            assert_eq!(state.info.serial_number, format!("MOCK-{:03}", i + 1));
            assert_eq!(
                state.scopes,
                [format!("onvif://www.onvif.org/name/Camera%20{:03}", i + 1)]
            );
        }
        assert_eq!(
            Manifest::generated(256, 18080)
                .unwrap()
                .prepare(&path)
                .unwrap()
                .len(),
            256
        );
        assert_eq!(
            Manifest::generated(257, 18080).unwrap_err(),
            "Count must be between 1 and 256"
        );
        assert_eq!(
            Manifest::generated(4, 65534).unwrap_err(),
            "Port range must fit within 1..65535"
        );
    }

    #[test]
    fn duplicate_and_network_settings_are_rejected_before_startup() {
        for (kind, expected) in [
            (0, "duplicate id"),
            (1, "duplicate uuid"),
            (2, "duplicate port"),
            (3, "duplicate serial_number"),
        ] {
            let mut m = Manifest::generated(2, 18080).unwrap();
            match kind {
                0 => m.devices[1].id = m.devices[0].id.clone(),
                1 => m.devices[1].uuid = m.devices[0].uuid.to_ascii_uppercase(),
                2 => m.devices[1].port = m.devices[0].port,
                _ => m.devices[1].serial_number = m.devices[0].serial_number.clone(),
            }
            assert!(
                m.prepare(Path::new("lab.toml"))
                    .unwrap_err()
                    .contains(expected)
            );
        }
        let mut m = Manifest::generated(1, 18080).unwrap();
        m.bind_ip = Ipv4Addr::UNSPECIFIED;
        assert!(
            m.prepare(Path::new("lab.toml"))
                .unwrap_err()
                .contains("wildcard binding requires")
        );
        m.bind_ip = loopback();
        m.discovery = true;
        assert!(
            m.prepare(Path::new("lab.toml"))
                .unwrap_err()
                .contains("non-loopback advertise_ip")
        );
        let source = toml::to_string(&m)
            .unwrap()
            .replace("schema_version = 1", "schema_version = 1\nunknown = true");
        assert!(
            toml::from_str::<Manifest>(&source)
                .unwrap_err()
                .to_string()
                .contains("unknown field")
        );
    }

    #[test]
    fn state_files_are_relative_read_only_and_never_silently_replaced() {
        let dir = Scratch::new();
        let path = dir.0.join("lab.toml");
        let state_path = dir.0.join("state.toml");
        let original = "hostname = 'old'\ntimezone = 'UTC+08:00'\n";
        std::fs::write(&state_path, original).unwrap();
        let mut m = Manifest::generated(1, 18080).unwrap();
        m.devices[0].state_file = Some("state.toml".into());
        let states = m.prepare(&path).unwrap();
        assert_eq!(states[0].hostname, "camera-001");
        assert_eq!(states[0].timezone, "UTC+08:00");
        assert_eq!(std::fs::read_to_string(&state_path).unwrap(), original);
        std::fs::write(&state_path, "secret-password = [").unwrap();
        let error = m.prepare(&path).unwrap_err();
        assert!(error.contains("factory fallback refused"));
        assert!(!error.contains("secret-password"));
    }
}
