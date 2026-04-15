//! Mutable device state — persisted to TOML file with file locking.

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::RwLock;

/// Default state file path.
fn default_state_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".oxvif")
        .join("mock_device.toml")
}

/// Parse CLI args for optional `--config <path>`, otherwise use default.
pub fn resolve_state_path() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--config" {
            if let Some(p) = args.get(i + 1) {
                return PathBuf::from(p);
            }
        }
    }
    default_state_path()
}

// ── Device State ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceState {
    #[serde(default = "default_device_info")]
    pub info: DeviceInfo,
    #[serde(default = "default_hostname")]
    pub hostname: String,
    #[serde(default)]
    pub hostname_from_dhcp: bool,
    #[serde(default = "default_users")]
    pub users: Vec<MockUser>,
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,
    #[serde(default = "default_tz")]
    pub timezone: String,
    #[serde(default)]
    pub daylight_savings: bool,
    #[serde(default = "default_dns")]
    pub dns: DnsState,
    #[serde(default = "default_ntp")]
    pub ntp: NtpState,
    #[serde(default = "default_gateway")]
    pub gateway_ipv4: Vec<String>,
    #[serde(default = "default_discovery_mode")]
    pub discovery_mode: String,
    #[serde(default = "default_imaging")]
    pub imaging: ImagingState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub firmware_version: String,
    pub serial_number: String,
    pub hardware_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockUser {
    pub username: String,
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsState {
    pub from_dhcp: bool,
    pub servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NtpState {
    pub from_dhcp: bool,
    pub servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagingState {
    pub brightness: f32,
    pub color_saturation: f32,
    pub contrast: f32,
    pub sharpness: f32,
    pub exposure_mode: String,
    pub white_balance_mode: String,
    pub backlight_compensation: String,
    pub wide_dynamic_range_mode: String,
    pub wide_dynamic_range_level: f32,
    pub ir_cut_filter: String,
    pub focus_mode: String,
}

// ── Defaults ────────────────────────────────────────────────────────────────

fn default_device_info() -> DeviceInfo {
    DeviceInfo {
        manufacturer: "oxvif-mock".into(),
        model: "MockCam-1080p".into(),
        firmware_version: "1.0.0".into(),
        serial_number: "MOCK-0001".into(),
        hardware_id: "1.0".into(),
    }
}
fn default_hostname() -> String {
    "mock-camera".into()
}
fn default_users() -> Vec<MockUser> {
    vec![
        MockUser {
            username: "admin".into(),
            level: "Administrator".into(),
        },
        MockUser {
            username: "operator".into(),
            level: "Operator".into(),
        },
    ]
}
fn default_scopes() -> Vec<String> {
    vec![
        "onvif://www.onvif.org/name/MockCamera".into(),
        "onvif://www.onvif.org/type/video_encoder".into(),
        "onvif://www.onvif.org/location/country/taiwan".into(),
    ]
}
fn default_tz() -> String {
    "UTC".into()
}
fn default_dns() -> DnsState {
    DnsState {
        from_dhcp: false,
        servers: vec!["8.8.8.8".into(), "8.8.4.4".into()],
    }
}
fn default_ntp() -> NtpState {
    NtpState {
        from_dhcp: false,
        servers: vec!["pool.ntp.org".into()],
    }
}
fn default_gateway() -> Vec<String> {
    vec!["192.168.1.1".into()]
}
fn default_discovery_mode() -> String {
    "Discoverable".into()
}
fn default_imaging() -> ImagingState {
    ImagingState {
        brightness: 60.0,
        color_saturation: 50.0,
        contrast: 50.0,
        sharpness: 50.0,
        exposure_mode: "AUTO".into(),
        white_balance_mode: "AUTO".into(),
        backlight_compensation: "OFF".into(),
        wide_dynamic_range_mode: "OFF".into(),
        wide_dynamic_range_level: 50.0,
        ir_cut_filter: "AUTO".into(),
        focus_mode: "AUTO".into(),
    }
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            info: default_device_info(),
            hostname: default_hostname(),
            hostname_from_dhcp: false,
            users: default_users(),
            scopes: default_scopes(),
            timezone: default_tz(),
            daylight_savings: false,
            dns: default_dns(),
            ntp: default_ntp(),
            gateway_ipv4: default_gateway(),
            discovery_mode: default_discovery_mode(),
            imaging: default_imaging(),
        }
    }
}

// ── Persistent shared state ─────────────────────────────────────────────────

pub struct PersistentState {
    state: RwLock<DeviceState>,
    path: PathBuf,
}

/// Thread-safe wrapper — replaces the old `type SharedState = RwLock<DeviceState>`.
pub type SharedState = PersistentState;

impl PersistentState {
    /// Load from file or create with defaults.
    pub fn load(path: PathBuf) -> Self {
        let state = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<DeviceState>(&content) {
                    Ok(s) => {
                        eprintln!("  Loaded state from {}", path.display());
                        s
                    }
                    Err(e) => {
                        eprintln!(
                            "  [WARN] Failed to parse {}: {e}, using defaults",
                            path.display()
                        );
                        DeviceState::default()
                    }
                },
                Err(e) => {
                    eprintln!(
                        "  [WARN] Failed to read {}: {e}, using defaults",
                        path.display()
                    );
                    DeviceState::default()
                }
            }
        } else {
            eprintln!("  No state file found, creating {}", path.display());
            let s = DeviceState::default();
            let ps = PersistentState {
                state: RwLock::new(s.clone()),
                path: path.clone(),
            };
            ps.flush();
            return ps;
        };

        PersistentState {
            state: RwLock::new(state),
            path,
        }
    }

    /// Read access (no file I/O).
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, DeviceState> {
        self.state.read().unwrap()
    }

    /// Flush current state to disk with file lock.
    fn flush(&self) {
        let state = self.state.read().unwrap();
        let content = match toml::to_string_pretty(&*state) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  [ERROR] Failed to serialize state: {e}");
                return;
            }
        };
        drop(state);

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Write with file lock
        match std::fs::File::create(&self.path) {
            Ok(file) => {
                if let Err(e) = file.lock_exclusive() {
                    eprintln!("  [WARN] File lock failed: {e}");
                }
                if let Err(e) = std::fs::write(&self.path, &content) {
                    eprintln!("  [ERROR] Failed to write {}: {e}", self.path.display());
                }
                let _ = FileExt::unlock(&file);
            }
            Err(e) => {
                eprintln!("  [ERROR] Failed to create {}: {e}", self.path.display());
            }
        }
    }
}

impl PersistentState {
    /// Get a mutable reference, modify it, then call flush.
    pub fn modify(&self, f: impl FnOnce(&mut DeviceState)) {
        {
            let mut guard = self.state.write().unwrap();
            f(&mut guard);
        }
        self.flush();
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::device;

    fn new_state() -> PersistentState {
        PersistentState {
            state: RwLock::new(DeviceState::default()),
            path: PathBuf::from("/dev/null"), // don't actually write in tests
        }
    }

    #[test]
    fn get_hostname_returns_default() {
        let s = new_state();
        let xml = device::resp_hostname(&s);
        assert!(xml.contains("mock-camera"), "default hostname");
    }

    #[test]
    fn set_hostname_then_get() {
        let s = new_state();
        let body = r#"<tds:SetHostname><tt:Name>new-host</tt:Name></tds:SetHostname>"#;
        device::handle_set_hostname(&s, body);
        let xml = device::resp_hostname(&s);
        assert!(xml.contains("new-host"));
        assert!(!xml.contains("mock-camera"));
    }

    #[test]
    fn get_users_returns_defaults() {
        let s = new_state();
        let xml = device::resp_users(&s);
        assert!(xml.contains("admin"));
        assert!(xml.contains("operator"));
    }

    #[test]
    fn create_user_then_get() {
        let s = new_state();
        let body = r#"<tds:CreateUsers><tds:User><tt:Username>viewer</tt:Username><tt:Password>pass</tt:Password><tt:UserLevel>User</tt:UserLevel></tds:User></tds:CreateUsers>"#;
        device::handle_create_users(&s, body);
        let xml = device::resp_users(&s);
        assert!(xml.contains("viewer"));
    }

    #[test]
    fn delete_user_then_get() {
        let s = new_state();
        let body = r#"<tds:DeleteUsers><tt:Username>operator</tt:Username></tds:DeleteUsers>"#;
        device::handle_delete_users(&s, body);
        let xml = device::resp_users(&s);
        assert!(xml.contains("admin"));
        assert!(!xml.contains("operator"));
    }

    #[test]
    fn set_user_level_then_get() {
        let s = new_state();
        let body = r#"<tds:SetUser><tds:User><tt:Username>operator</tt:Username><tt:UserLevel>Administrator</tt:UserLevel></tds:User></tds:SetUser>"#;
        device::handle_set_user(&s, body);
        let xml = device::resp_users(&s);
        assert_eq!(xml.matches("Administrator").count(), 2);
    }

    #[test]
    fn set_dns_then_get() {
        let s = new_state();
        let body = r#"<tds:SetDNS><tt:FromDHCP>false</tt:FromDHCP><tt:DNSManual><tt:Type>IPv4</tt:Type><tt:IPv4Address>1.1.1.1</tt:IPv4Address></tt:DNSManual></tds:SetDNS>"#;
        device::handle_set_dns(&s, body);
        let xml = device::resp_dns(&s);
        assert!(xml.contains("1.1.1.1"));
        assert!(!xml.contains("8.8.8.8"));
    }

    #[test]
    fn set_ntp_then_get() {
        let s = new_state();
        let body = r#"<tds:SetNTP><tt:FromDHCP>false</tt:FromDHCP><tt:NTPManual><tt:Type>DNS</tt:Type><tt:DNSname>time.google.com</tt:DNSname></tt:NTPManual></tds:SetNTP>"#;
        device::handle_set_ntp(&s, body);
        let xml = device::resp_ntp(&s);
        assert!(xml.contains("time.google.com"));
        assert!(!xml.contains("pool.ntp.org"));
    }

    #[test]
    fn set_scopes_then_get() {
        let s = new_state();
        let body = r#"<tds:SetScopes><tt:ScopeItem>onvif://www.onvif.org/name/NewCam</tt:ScopeItem></tds:SetScopes>"#;
        device::handle_set_scopes(&s, body);
        let xml = device::resp_scopes(&s);
        assert!(xml.contains("NewCam"));
        assert!(!xml.contains("MockCamera"));
    }

    #[test]
    fn set_timezone_then_get() {
        let s = new_state();
        let body = r#"<tds:SetSystemDateAndTime><tt:TimeZone><tt:TZ>CST-8</tt:TZ></tt:TimeZone><tt:DaylightSavings>true</tt:DaylightSavings></tds:SetSystemDateAndTime>"#;
        device::handle_set_system_date_and_time(&s, body);
        let xml = device::resp_system_date_and_time(&s);
        assert!(xml.contains("CST-8"));
        assert!(xml.contains("<tt:DaylightSavings>true</tt:DaylightSavings>"));
    }

    #[test]
    fn device_info_reads_from_state() {
        let s = new_state();
        let xml = device::resp_device_info(&s);
        assert!(xml.contains("oxvif-mock"));
        assert!(xml.contains("MockCam-1080p"));
    }

    #[test]
    fn serialization_roundtrip() {
        let state = DeviceState::default();
        let toml_str = toml::to_string_pretty(&state).unwrap();
        let parsed: DeviceState = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.hostname, state.hostname);
        assert_eq!(parsed.imaging.brightness, state.imaging.brightness);
        assert_eq!(parsed.users.len(), state.users.len());
    }
}
