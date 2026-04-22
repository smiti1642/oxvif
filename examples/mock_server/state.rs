//! Mutable device state — persisted to TOML file with file locking.

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::io::Write;
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
    #[serde(default = "default_interface")]
    pub interface: NetworkInterfaceState,
    #[serde(default = "default_protocols")]
    pub protocols: Vec<NetworkProtocolState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterfaceState {
    pub token: String,
    pub name: String,
    pub mac: String,
    pub mtu: u32,
    pub enabled: bool,
    pub ipv4_from_dhcp: bool,
    pub ipv4_address: String,
    pub ipv4_prefix_length: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkProtocolState {
    pub name: String,
    pub enabled: bool,
    pub ports: Vec<u32>,
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
    /// Plaintext password used to validate WS-Security digests.
    /// `#[serde(default)]` keeps older state files (pre-per-user-auth)
    /// loadable; those users get a blank password until re-set.
    #[serde(default)]
    pub password: String,
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
            password: "admin".into(),
        },
        MockUser {
            username: "operator".into(),
            level: "Operator".into(),
            password: "operator".into(),
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
fn default_interface() -> NetworkInterfaceState {
    NetworkInterfaceState {
        token: "eth0".into(),
        name: "eth0".into(),
        mac: "00:11:22:33:44:55".into(),
        mtu: 1500,
        enabled: true,
        ipv4_from_dhcp: false,
        ipv4_address: "192.168.1.100".into(),
        ipv4_prefix_length: 24,
    }
}
fn default_protocols() -> Vec<NetworkProtocolState> {
    vec![
        NetworkProtocolState {
            name: "HTTP".into(),
            enabled: true,
            ports: vec![80],
        },
        NetworkProtocolState {
            name: "HTTPS".into(),
            enabled: true,
            ports: vec![443],
        },
        NetworkProtocolState {
            name: "RTSP".into(),
            enabled: true,
            ports: vec![554],
        },
    ]
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
            interface: default_interface(),
            protocols: default_protocols(),
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

    /// In-memory instance for tests — never touches the real filesystem
    /// (flush writes to `/dev/null`, which is silently a no-op on both
    /// POSIX and Windows).
    #[cfg(test)]
    pub fn for_tests() -> Self {
        PersistentState {
            state: RwLock::new(DeviceState::default()),
            path: PathBuf::from("/dev/null"),
        }
    }

    /// Flush current state to disk atomically.
    ///
    /// Writes to a sibling `.tmp` file under an exclusive lock, then
    /// renames it over the real state file. A crash mid-flush leaves
    /// either the old file intact or the new file in place — never a
    /// half-written one.
    ///
    /// The lock is held on the tempfile (not the final path), so a
    /// rival process holding a stale lock on the real file doesn't
    /// block us, and on Windows we avoid the same-handle-twice trap
    /// that `File::create + std::fs::write` used to fall into.
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

        // Ensure parent directory exists.
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let tmp_path = {
            // `.with_extension` replaces the extension; use an explicit
            // suffix so paths without an extension still get a distinct
            // tempfile name.
            let mut p = self.path.clone();
            let name = p
                .file_name()
                .map(|n| n.to_owned())
                .unwrap_or_else(|| std::ffi::OsString::from("state"));
            let mut new_name = name;
            new_name.push(".tmp");
            p.set_file_name(new_name);
            p
        };

        match std::fs::File::create(&tmp_path) {
            Ok(mut file) => {
                if let Err(e) = file.lock_exclusive() {
                    eprintln!("  [WARN] File lock failed: {e}");
                }
                let write_ok =
                    file.write_all(content.as_bytes()).is_ok() && file.sync_all().is_ok();
                let _ = FileExt::unlock(&file);
                drop(file);

                if write_ok {
                    if let Err(e) = std::fs::rename(&tmp_path, &self.path) {
                        eprintln!(
                            "  [ERROR] Failed to rename {} -> {}: {e}",
                            tmp_path.display(),
                            self.path.display()
                        );
                        // Best-effort cleanup of the orphan tempfile.
                        let _ = std::fs::remove_file(&tmp_path);
                    }
                } else {
                    eprintln!("  [ERROR] Write to tempfile {} failed", tmp_path.display());
                    let _ = std::fs::remove_file(&tmp_path);
                }
            }
            Err(e) => {
                eprintln!(
                    "  [ERROR] Failed to create tempfile {}: {e}",
                    tmp_path.display()
                );
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
        // ONVIF SetScopes: each URI is sent as a bare <Scopes>URI</Scopes>
        // element — NOT wrapped in <ScopeItem>. The old test was matching
        // a broken parser that looked for the wrong tag; fixed along with
        // `handle_set_scopes` itself.
        let body = r#"<tds:SetScopes><tds:Scopes>onvif://www.onvif.org/name/NewCam</tds:Scopes></tds:SetScopes>"#;
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
    fn set_network_interfaces_updates_ip_and_dhcp() {
        let s = new_state();
        // SetNetworkInterfaces body shape per oxvif's `set_network_interfaces`.
        let body = r#"<tds:SetNetworkInterfaces>
            <tds:InterfaceToken>eth0</tds:InterfaceToken>
            <tds:NetworkInterface>
              <tt:Enabled>true</tt:Enabled>
              <tt:IPv4>
                <tt:Enabled>true</tt:Enabled>
                <tt:DHCP>false</tt:DHCP>
                <tt:Manual>
                  <tt:Address>10.0.0.5</tt:Address>
                  <tt:PrefixLength>16</tt:PrefixLength>
                </tt:Manual>
              </tt:IPv4>
            </tds:NetworkInterface>
          </tds:SetNetworkInterfaces>"#;
        let resp = device::handle_set_network_interfaces(&s, body);
        // Response wraps RebootNeeded — sanity check that the handler ran.
        assert!(resp.contains("SetNetworkInterfacesResponse"));
        assert!(resp.contains("RebootNeeded"));
        let xml = device::resp_network_interfaces(&s);
        assert!(xml.contains("10.0.0.5"));
        assert!(xml.contains("<tt:PrefixLength>16</tt:PrefixLength>"));
        assert!(!xml.contains("192.168.1.100"));
    }

    #[test]
    fn set_network_protocols_updates_and_inserts() {
        let s = new_state();
        // Flip HTTP port, add a brand-new "FTP" entry the mock didn't have.
        let body = r#"<tds:SetNetworkProtocols>
            <tds:NetworkProtocols><tt:Name>HTTP</tt:Name><tt:Enabled>false</tt:Enabled><tt:Port>8080</tt:Port></tds:NetworkProtocols>
            <tds:NetworkProtocols><tt:Name>FTP</tt:Name><tt:Enabled>true</tt:Enabled><tt:Port>21</tt:Port></tds:NetworkProtocols>
          </tds:SetNetworkProtocols>"#;
        device::handle_set_network_protocols(&s, body);
        let xml = device::resp_network_protocols(&s);
        // HTTP should still be there but disabled + new port.
        assert!(xml.contains("<tt:Name>HTTP</tt:Name>"));
        assert!(xml.contains("<tt:Port>8080</tt:Port>"));
        // FTP newly inserted.
        assert!(xml.contains("<tt:Name>FTP</tt:Name>"));
        assert!(xml.contains("<tt:Port>21</tt:Port>"));
    }

    #[test]
    fn set_network_default_gateway_replaces_list() {
        let s = new_state();
        let body = r#"<tds:SetNetworkDefaultGateway>
            <tds:IPv4Address>10.0.0.1</tds:IPv4Address>
            <tds:IPv4Address>10.0.0.254</tds:IPv4Address>
          </tds:SetNetworkDefaultGateway>"#;
        device::handle_set_network_default_gateway(&s, body);
        let xml = device::resp_network_default_gateway(&s);
        assert!(xml.contains("10.0.0.1"));
        assert!(xml.contains("10.0.0.254"));
        // Default was 192.168.1.1 — must be gone after replacement.
        assert!(!xml.contains("192.168.1.1"));
    }

    #[test]
    fn flush_creates_state_file_atomically() {
        // Use a real tempfile path so the flush->rename pathway actually runs.
        let tmp_dir = std::env::temp_dir().join("oxvif_mock_flush_test");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let path = tmp_dir.join("state.toml");
        let _ = std::fs::remove_file(&path);

        let ps = PersistentState {
            state: RwLock::new(DeviceState::default()),
            path: path.clone(),
        };
        ps.flush();

        assert!(path.exists(), "state file should exist after flush");
        let contents = std::fs::read_to_string(&path).expect("readable");
        assert!(contents.contains("hostname"));

        // Tempfile should not linger.
        let tmp_path = path.with_file_name("state.toml.tmp");
        assert!(!tmp_path.exists(), "tempfile should be renamed away");

        let _ = std::fs::remove_file(&path);
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
