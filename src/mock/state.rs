//! In-memory mock device state.

use serde::{Deserialize, Serialize};
use std::sync::RwLock;

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
    #[serde(default = "default_ptz")]
    pub ptz: PtzState,
    #[serde(default = "default_interface")]
    pub interface: NetworkInterfaceState,
    #[serde(default = "default_protocols")]
    pub protocols: Vec<NetworkProtocolState>,
    #[serde(default = "default_osd")]
    pub osd: OsdState,
    #[serde(default = "default_profiles")]
    pub profiles: ProfilesState,
    #[serde(default = "default_video_encoder")]
    pub video_encoder: VideoEncoderState,
    /// Monotonic event counter for the pull-point stream (per-instance,
    /// not persisted). Replaces the former process-global `EVENT_SEQ`.
    #[serde(skip)]
    pub event_seq: u64,
    /// Active pull-point topic filter, set by CreatePullPointSubscription
    /// (per-instance, not persisted). `None` = emit every topic.
    #[serde(skip)]
    pub event_filter: Option<Vec<String>>,
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
pub struct PtzPreset {
    pub token: String,
    pub name: String,
    pub pan: f32,
    pub tilt: f32,
    pub zoom: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzState {
    pub pan: f32,
    pub tilt: f32,
    pub zoom: f32,
    pub home_pan: f32,
    pub home_tilt: f32,
    pub home_zoom: f32,
    pub presets: Vec<PtzPreset>,
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

// ── OSD state ───────────────────────────────────────────────────────────────
//
// OSDs are persisted by `(token, video_source_config_token)`. The mock
// advertises per-type quotas in `GetOSDOptions` (Genetec/late-Hikvision
// shape) and enforces them in `CreateOSD` — over-limit returns
// `ter:InvalidArgs`. This lets clients exercise their quota-gate UI
// against the mock instead of waiting for real-camera failures.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsdState {
    pub osds: Vec<OsdEntry>,
    /// Counter for tokens. Persists across restarts so deleted tokens
    /// don't get reused (matches what real cameras do).
    #[serde(default)]
    pub next_token_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsdEntry {
    pub token: String,
    pub video_source_config_token: String,
    /// `"Text"` or `"Image"`.
    pub osd_type: String,
    /// `"UpperLeft"`, `"UpperRight"`, `"LowerLeft"`, `"LowerRight"`,
    /// or `"Custom"` (uses `position_x`/`position_y`).
    pub position_type: String,
    #[serde(default)]
    pub position_x: Option<f32>,
    #[serde(default)]
    pub position_y: Option<f32>,
    /// Text-OSD payload — `Some` when `osd_type == "Text"`.
    #[serde(default)]
    pub text: Option<OsdTextEntry>,
    /// Image-OSD URL — `Some` when `osd_type == "Image"`.
    #[serde(default)]
    pub image_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsdTextEntry {
    /// `"Plain"`, `"Date"`, `"Time"`, or `"DateAndTime"`.
    pub text_type: String,
    #[serde(default)]
    pub plain_text: Option<String>,
    #[serde(default)]
    pub date_format: Option<String>,
    #[serde(default)]
    pub time_format: Option<String>,
    #[serde(default)]
    pub font_size: Option<u32>,
    #[serde(default)]
    pub font_color: Option<OsdColorEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsdColorEntry {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    #[serde(default)]
    pub colorspace: Option<String>,
    #[serde(default)]
    pub transparent: Option<f32>,
}

/// Per-text-type OSD quotas. Matches what the mock advertises in
/// `GetOSDOptionsResponse`. `CreateOSD` enforces these — over-limit
/// returns a `ter:InvalidArgs` SOAP fault, mirroring Genetec's
/// behaviour.
pub const OSD_QUOTA_TOTAL: u32 = 8;
pub const OSD_QUOTA_PLAIN: u32 = 7;
pub const OSD_QUOTA_DATE: u32 = 1;
pub const OSD_QUOTA_TIME: u32 = 1;
pub const OSD_QUOTA_DATE_AND_TIME: u32 = 1;

// ── Media profile state ─────────────────────────────────────────────────────
//
// Tracks the camera's media profile list. Real cameras seed two or
// three "fixed" profiles (mainStream / subStream / sometimes thirdStream)
// that can't be deleted, plus any user-created ones. The actual
// configuration objects (VSC, VEC, etc.) referenced by the profile
// stay hardcoded in `services/media.rs`'s render helpers — only the
// attachment (which token is bound to which profile) is mutable.
//
// `CreateProfile` adds an entry with no configurations attached, matching
// real-camera behaviour where the caller follows up with
// `AddVideoSourceConfiguration` etc. to fill it in. `DeleteProfile`
// refuses to remove `fixed=true` profiles (per ONVIF spec — returns
// `ter:DeletionOfFixedProfile`).

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilesState {
    pub profiles: Vec<ProfileEntry>,
    /// Counter for generated tokens. Persists so deleted profile
    /// tokens don't get reused.
    #[serde(default)]
    pub next_token_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileEntry {
    pub token: String,
    pub name: String,
    /// `true` for factory-baked profiles that can't be deleted.
    pub fixed: bool,
    #[serde(default)]
    pub video_source_config_token: Option<String>,
    #[serde(default)]
    pub video_encoder_config_token: Option<String>,
    #[serde(default)]
    pub audio_source_config_token: Option<String>,
    #[serde(default)]
    pub audio_encoder_config_token: Option<String>,
}

// ── Video encoder configuration state ─────────────────────────────────────────
//
// The Media2 video encoder config (token `VEC_1`, referenced by the default
// profiles). `GetVideoEncoderConfigurations` renders from here and
// `SetVideoEncoderConfiguration` persists into it, so a Set → Get roundtrip
// reflects the change. Uses Media2's flat, H.265-capable shape.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoEncoderState {
    pub token: String,
    pub name: String,
    pub use_count: u32,
    /// `"H264"`, `"H265"`, or `"JPEG"`.
    pub encoding: String,
    pub width: u32,
    pub height: u32,
    pub quality: f32,
    pub frame_rate_limit: u32,
    pub bitrate_limit: u32,
    pub gov_length: u32,
    pub profile: String,
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
fn default_ptz() -> PtzState {
    PtzState {
        pan: 0.0,
        tilt: 0.0,
        zoom: 0.0,
        home_pan: 0.0,
        home_tilt: 0.0,
        home_zoom: 0.0,
        presets: vec![
            PtzPreset {
                token: "Preset_1".into(),
                name: "Home".into(),
                pan: 0.0,
                tilt: 0.0,
                zoom: 0.0,
            },
            PtzPreset {
                token: "Preset_2".into(),
                name: "Door".into(),
                pan: 0.5,
                tilt: 0.2,
                zoom: 0.0,
            },
        ],
    }
}
fn default_osd() -> OsdState {
    OsdState {
        osds: vec![OsdEntry {
            token: "OSD_1".into(),
            video_source_config_token: "VSC_1".into(),
            osd_type: "Text".into(),
            position_type: "UpperLeft".into(),
            position_x: None,
            position_y: None,
            text: Some(OsdTextEntry {
                text_type: "DateAndTime".into(),
                plain_text: None,
                date_format: Some("MM/dd/yyyy".into()),
                time_format: Some("HH:mm:ss".into()),
                font_size: Some(20),
                font_color: None,
            }),
            image_path: None,
        }],
        next_token_id: 2,
    }
}

fn default_profiles() -> ProfilesState {
    ProfilesState {
        profiles: vec![
            ProfileEntry {
                token: "Profile_1".into(),
                name: "mainStream".into(),
                fixed: true,
                video_source_config_token: Some("VSC_1".into()),
                video_encoder_config_token: Some("VEC_1".into()),
                audio_source_config_token: None,
                audio_encoder_config_token: None,
            },
            ProfileEntry {
                token: "Profile_2".into(),
                name: "subStream".into(),
                fixed: false,
                video_source_config_token: Some("VSC_1".into()),
                video_encoder_config_token: Some("VEC_2".into()),
                audio_source_config_token: None,
                audio_encoder_config_token: None,
            },
        ],
        next_token_id: 3,
    }
}

fn default_video_encoder() -> VideoEncoderState {
    VideoEncoderState {
        token: "VEC_1".into(),
        name: "VideoEncoderConfig".into(),
        use_count: 1,
        encoding: "H265".into(),
        width: 1920,
        height: 1080,
        quality: 5.0,
        frame_rate_limit: 30,
        bitrate_limit: 4096,
        gov_length: 50,
        profile: "Main".into(),
    }
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
            ptz: default_ptz(),
            interface: default_interface(),
            protocols: default_protocols(),
            osd: default_osd(),
            profiles: default_profiles(),
            video_encoder: default_video_encoder(),
            event_seq: 0,
            event_filter: None,
        }
    }
}

// ── In-memory shared state ──────────────────────────────────────────────────
//
// The mock holds its `DeviceState` purely in memory and never touches the
// filesystem. Persistence is opt-in and owned by the caller: register an
// `on_change` hook (the bundled example writes TOML) and it fires after every
// mutation with a snapshot of the new state.

/// Callback fired after each state mutation — the seam for caller-owned
/// persistence without the library doing any file I/O.
pub type ChangeHook = std::sync::Arc<dyn Fn(&DeviceState) + Send + Sync>;

/// Thread-safe in-memory device state shared by `MockTransport` / `MockServer`.
pub struct MockState {
    state: RwLock<DeviceState>,
    on_change: Option<ChangeHook>,
}

/// Internal alias so service handlers keep reading `&SharedState` unchanged.
pub type SharedState = MockState;

impl MockState {
    /// Fresh state seeded with factory defaults and no persistence hook.
    pub fn new() -> Self {
        Self {
            state: RwLock::new(DeviceState::default()),
            on_change: None,
        }
    }

    /// Seed with a caller-supplied state (e.g. loaded from disk by the caller).
    pub fn with_state(state: DeviceState) -> Self {
        Self {
            state: RwLock::new(state),
            on_change: None,
        }
    }

    /// Register a hook invoked after every mutation with a snapshot of the new
    /// state. This is how opt-in persistence is wired — the library performs no
    /// file I/O itself.
    pub fn set_on_change(&mut self, hook: ChangeHook) {
        self.on_change = Some(hook);
    }

    /// Read access.
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, DeviceState> {
        self.state.read().unwrap()
    }

    /// Mutate the state, then fire the change hook (if any).
    pub fn modify(&self, f: impl FnOnce(&mut DeviceState)) {
        {
            let mut guard = self.state.write().unwrap();
            f(&mut guard);
        }
        self.notify();
    }

    /// Like [`modify`](Self::modify) but the closure returns a value
    /// (e.g. a freshly-generated token).
    pub fn modify_returning<R>(&self, f: impl FnOnce(&mut DeviceState) -> R) -> R {
        let result = {
            let mut guard = self.state.write().unwrap();
            f(&mut guard)
        };
        self.notify();
        result
    }

    fn notify(&self) {
        if let Some(hook) = &self.on_change {
            hook(&self.state.read().unwrap());
        }
    }

    /// In-memory instance for tests.
    #[cfg(test)]
    pub fn for_tests() -> Self {
        Self::new()
    }
}

impl Default for MockState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::services::device;

    fn new_state() -> MockState {
        MockState::new()
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
    fn ptz_absolute_move_updates_position() {
        use crate::mock::services::ptz;
        let s = new_state();
        let body = r#"<tptz:AbsoluteMove>
            <tptz:Position>
              <tt:PanTilt x="0.5" y="-0.3"/>
              <tt:Zoom x="0.7"/>
            </tt:Position>
          </tptz:AbsoluteMove>"#;
        ptz::handle_ptz_absolute_move(&s, body);
        let xml = ptz::resp_ptz_status(&s);
        assert!(xml.contains(r#"x="0.5""#));
        assert!(xml.contains(r#"y="-0.3""#));
        assert!(xml.contains(r#"x="0.7""#));
    }

    #[test]
    fn ptz_set_preset_uses_current_position_and_returns_token() {
        use crate::mock::services::ptz;
        let s = new_state();
        // Move first so SetPreset captures a non-zero position.
        let move_body = r#"<tptz:AbsoluteMove><tptz:Position>
            <tt:PanTilt x="0.4" y="0.1"/><tt:Zoom x="0.2"/>
          </tt:Position></tptz:AbsoluteMove>"#;
        ptz::handle_ptz_absolute_move(&s, move_body);

        let body = r#"<tptz:SetPreset>
            <tptz:PresetName>Garden</tptz:PresetName>
          </tptz:SetPreset>"#;
        let resp = ptz::handle_ptz_set_preset(&s, body);
        // Defaults already use Preset_1 and Preset_2, so new one is Preset_3.
        assert!(resp.contains("Preset_3"));

        let presets = ptz::resp_ptz_presets(&s);
        assert!(presets.contains("Garden"));
        assert!(presets.contains(r#"x="0.4""#));
    }

    #[test]
    fn ptz_remove_preset_then_get() {
        use crate::mock::services::ptz;
        let s = new_state();
        let body = r#"<tptz:RemovePreset>
            <tptz:PresetToken>Preset_2</tptz:PresetToken>
          </tptz:RemovePreset>"#;
        ptz::handle_ptz_remove_preset(&s, body);
        let xml = ptz::resp_ptz_presets(&s);
        assert!(xml.contains("Preset_1"));
        assert!(!xml.contains(r#"token="Preset_2""#));
    }

    #[test]
    fn ptz_goto_preset_jumps_position() {
        use crate::mock::services::ptz;
        let s = new_state();
        // Preset_2 default: pan=0.5 tilt=0.2 zoom=0.0
        let body = r#"<tptz:GotoPreset>
            <tptz:PresetToken>Preset_2</tptz:PresetToken>
          </tptz:GotoPreset>"#;
        ptz::handle_ptz_goto_preset(&s, body);
        let xml = ptz::resp_ptz_status(&s);
        assert!(xml.contains(r#"x="0.5""#));
        assert!(xml.contains(r#"y="0.2""#));
    }

    #[test]
    fn ptz_set_home_then_goto_home() {
        use crate::mock::services::ptz;
        let s = new_state();
        // Move, set home, move away, goto home → position should reset to setpoint.
        let move1 = r#"<tptz:AbsoluteMove><tptz:Position>
            <tt:PanTilt x="0.8" y="-0.4"/><tt:Zoom x="0.3"/>
          </tt:Position></tptz:AbsoluteMove>"#;
        ptz::handle_ptz_absolute_move(&s, move1);
        ptz::handle_ptz_set_home_position(&s);

        let move2 = r#"<tptz:AbsoluteMove><tptz:Position>
            <tt:PanTilt x="-0.5" y="0.5"/><tt:Zoom x="0.0"/>
          </tt:Position></tptz:AbsoluteMove>"#;
        ptz::handle_ptz_absolute_move(&s, move2);

        ptz::handle_ptz_goto_home_position(&s);
        let xml = ptz::resp_ptz_status(&s);
        assert!(xml.contains(r#"x="0.8""#));
        assert!(xml.contains(r#"y="-0.4""#));
    }

    // ── OSD CRUD + quota ─────────────────────────────────────────────────

    #[test]
    fn osd_default_state_has_one_datetime_entry() {
        let s = new_state();
        let snap = s.read().osd.clone();
        assert_eq!(snap.osds.len(), 1);
        assert_eq!(snap.osds[0].token, "OSD_1");
        assert_eq!(snap.osds[0].text.as_ref().unwrap().text_type, "DateAndTime");
    }

    #[test]
    fn create_osd_then_appears_in_get() {
        use crate::mock::services::media;
        let s = new_state();
        // Create a Plain text OSD — DateAndTime is at quota (1/1) by default.
        let body = r#"<trt:CreateOSD><trt:OSD>
            <tt:VideoSourceConfigurationToken>VSC_1</tt:VideoSourceConfigurationToken>
            <tt:Type>Text</tt:Type>
            <tt:Position><tt:Type>UpperRight</tt:Type></tt:Position>
            <tt:TextString>
              <tt:Type>Plain</tt:Type>
              <tt:PlainText>Hello camera</tt:PlainText>
              <tt:FontSize>24</tt:FontSize>
            </tt:TextString>
          </trt:OSD></trt:CreateOSD>"#;
        let resp = media::handle_create_osd(&s, body);
        assert!(resp.contains("CreateOSDResponse"));
        assert!(resp.contains("OSD_2"), "new token should be OSD_2");

        let listed = media::resp_osds(&s, "<trt:GetOSDs/>");
        assert!(listed.contains("OSD_1"));
        assert!(listed.contains("OSD_2"));
        assert!(listed.contains("Hello camera"));
    }

    #[test]
    fn create_osd_rejects_when_per_type_quota_full() {
        use crate::mock::services::media;
        let s = new_state();
        // Default already has one DateAndTime — DateAndTime quota is 1.
        // A second one must be rejected.
        let body = r#"<trt:CreateOSD><trt:OSD>
            <tt:VideoSourceConfigurationToken>VSC_1</tt:VideoSourceConfigurationToken>
            <tt:Type>Text</tt:Type>
            <tt:Position><tt:Type>LowerRight</tt:Type></tt:Position>
            <tt:TextString><tt:Type>DateAndTime</tt:Type></tt:TextString>
          </trt:OSD></trt:CreateOSD>"#;
        let resp = media::handle_create_osd(&s, body);
        assert!(resp.contains("Fault"), "should be SOAP fault");
        assert!(resp.contains("InvalidArgs"));
        assert!(resp.contains("DateAndTime"));
        // State unchanged — still just the default one.
        assert_eq!(s.read().osd.osds.len(), 1);
    }

    #[test]
    fn set_osd_updates_existing() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:SetOSD><trt:OSD token="OSD_1">
            <tt:VideoSourceConfigurationToken>VSC_1</tt:VideoSourceConfigurationToken>
            <tt:Type>Text</tt:Type>
            <tt:Position><tt:Type>LowerLeft</tt:Type></tt:Position>
            <tt:TextString>
              <tt:Type>DateAndTime</tt:Type>
              <tt:DateFormat>yyyy-MM-dd</tt:DateFormat>
            </tt:TextString>
          </trt:OSD></trt:SetOSD>"#;
        let resp = media::handle_set_osd(&s, body);
        assert!(resp.contains("SetOSDResponse"));
        assert!(!resp.contains("Fault"));

        let listed = media::resp_osds(&s, "<trt:GetOSDs/>");
        assert!(listed.contains("LowerLeft"));
        assert!(listed.contains("yyyy-MM-dd"));
        // VSC token must be preserved across SetOSD.
        assert!(listed.contains("VSC_1"));
    }

    #[test]
    fn delete_osd_removes_entry() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:DeleteOSD><trt:OSDToken>OSD_1</trt:OSDToken></trt:DeleteOSD>"#;
        let resp = media::handle_delete_osd(&s, body);
        assert!(resp.contains("DeleteOSDResponse"));
        assert_eq!(s.read().osd.osds.len(), 0);
    }

    #[test]
    fn delete_osd_unknown_token_returns_fault() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:DeleteOSD><trt:OSDToken>OSD_99</trt:OSDToken></trt:DeleteOSD>"#;
        let resp = media::handle_delete_osd(&s, body);
        assert!(resp.contains("Fault"));
        assert!(resp.contains("OSD_99"));
        // State untouched.
        assert_eq!(s.read().osd.osds.len(), 1);
    }

    #[test]
    fn get_osds_filters_by_configuration_token() {
        use crate::mock::services::media;
        let s = new_state();
        // Create one OSD on a different VSC.
        let create = r#"<trt:CreateOSD><trt:OSD>
            <tt:VideoSourceConfigurationToken>VSC_OTHER</tt:VideoSourceConfigurationToken>
            <tt:Type>Text</tt:Type>
            <tt:Position><tt:Type>UpperLeft</tt:Type></tt:Position>
            <tt:TextString><tt:Type>Plain</tt:Type><tt:PlainText>Other</tt:PlainText></tt:TextString>
          </trt:OSD></trt:CreateOSD>"#;
        media::handle_create_osd(&s, create);
        assert_eq!(s.read().osd.osds.len(), 2);

        // Filter by VSC_1 — should NOT include the VSC_OTHER one.
        let only_vsc1 = media::resp_osds(
            &s,
            r#"<trt:GetOSDs><trt:ConfigurationToken>VSC_1</trt:ConfigurationToken></trt:GetOSDs>"#,
        );
        assert!(only_vsc1.contains("OSD_1"));
        assert!(!only_vsc1.contains("Other"));
    }

    #[test]
    fn osd_options_advertises_per_type_quotas_via_attributes() {
        use crate::mock::services::media;
        let xml = media::resp_osd_options();
        // Genetec/late-Hikvision shape — attributes on <MaximumNumberOfOSDs>,
        // not element text. oxvif::OnvifSession parses these.
        assert!(xml.contains(r#"Total="8""#));
        assert!(xml.contains(r#"DateAndTime="1""#));
        assert!(xml.contains(r#"Plain="7""#));
    }

    // ── Profile CRUD ─────────────────────────────────────────────────────

    #[test]
    fn profiles_default_state_has_two() {
        use crate::mock::services::media;
        let s = new_state();
        let xml = media::resp_profiles(&s);
        assert!(xml.contains(r#"token="Profile_1" fixed="true""#));
        assert!(xml.contains(r#"token="Profile_2" fixed="false""#));
        assert!(xml.contains("mainStream"));
        assert!(xml.contains("subStream"));
        // Default profiles have video configs attached.
        assert!(xml.contains("VSC_1"));
        assert!(xml.contains("VEC_1"));
        assert!(xml.contains("VEC_2"));
    }

    #[test]
    fn create_profile_then_appears_in_get_profiles() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:CreateProfile><trt:Name>customStream</trt:Name></trt:CreateProfile>"#;
        let resp = media::handle_create_profile(&s, body);
        assert!(resp.contains("CreateProfileResponse"));
        // Default counter starts at 3, so first generated token is Profile_3.
        assert!(resp.contains("Profile_3"));
        assert!(resp.contains("customStream"));
        assert!(resp.contains(r#"fixed="false""#));

        let listed = media::resp_profiles(&s);
        assert!(listed.contains("Profile_3"));
        assert!(listed.contains("customStream"));
        // New profiles have no configurations attached.
        let new_p_block = listed
            .find("Profile_3")
            .and_then(|i| {
                listed[i..]
                    .find("</trt:Profiles>")
                    .map(|j| &listed[i..i + j])
            })
            .unwrap_or("");
        assert!(!new_p_block.contains("VideoSourceConfiguration"));
        assert!(!new_p_block.contains("VideoEncoderConfiguration"));
    }

    #[test]
    fn create_profile_with_explicit_token_honoured() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:CreateProfile>
            <trt:Name>specialName</trt:Name>
            <trt:Token>MyProfile</trt:Token>
          </trt:CreateProfile>"#;
        let resp = media::handle_create_profile(&s, body);
        assert!(resp.contains("MyProfile"));
        // Counter should NOT have been bumped — explicit token, no generation.
        assert_eq!(s.read().profiles.next_token_id, 3);
    }

    #[test]
    fn create_profile_rejects_duplicate_token() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:CreateProfile>
            <trt:Name>dup</trt:Name>
            <trt:Token>Profile_1</trt:Token>
          </trt:CreateProfile>"#;
        let resp = media::handle_create_profile(&s, body);
        assert!(resp.contains("Fault"));
        assert!(resp.contains("ProfileExists"));
        // No new entry, no counter change.
        assert_eq!(s.read().profiles.profiles.len(), 2);
    }

    #[test]
    fn delete_profile_removes_non_fixed() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:DeleteProfile><trt:ProfileToken>Profile_2</trt:ProfileToken></trt:DeleteProfile>"#;
        let resp = media::handle_delete_profile(&s, body);
        assert!(resp.contains("DeleteProfileResponse"));
        assert_eq!(s.read().profiles.profiles.len(), 1);
        assert_eq!(s.read().profiles.profiles[0].token, "Profile_1");
    }

    #[test]
    fn delete_profile_refuses_fixed() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:DeleteProfile><trt:ProfileToken>Profile_1</trt:ProfileToken></trt:DeleteProfile>"#;
        let resp = media::handle_delete_profile(&s, body);
        assert!(resp.contains("Fault"));
        assert!(resp.contains("DeletionOfFixedProfile"));
        // State untouched.
        assert_eq!(s.read().profiles.profiles.len(), 2);
    }

    #[test]
    fn delete_profile_unknown_token_returns_fault() {
        use crate::mock::services::media;
        let s = new_state();
        let body =
            r#"<trt:DeleteProfile><trt:ProfileToken>NoSuch</trt:ProfileToken></trt:DeleteProfile>"#;
        let resp = media::handle_delete_profile(&s, body);
        assert!(resp.contains("Fault"));
        assert!(resp.contains("NoProfile"));
        assert_eq!(s.read().profiles.profiles.len(), 2);
    }

    #[test]
    fn get_profile_by_token() {
        use crate::mock::services::media;
        let s = new_state();
        let body =
            r#"<trt:GetProfile><trt:ProfileToken>Profile_2</trt:ProfileToken></trt:GetProfile>"#;
        let resp = media::resp_profile(&s, body);
        assert!(resp.contains("GetProfileResponse"));
        assert!(resp.contains("subStream"));
        assert!(!resp.contains("mainStream"));
    }

    #[test]
    fn get_profile_unknown_token_returns_fault() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:GetProfile><trt:ProfileToken>Bogus</trt:ProfileToken></trt:GetProfile>"#;
        let resp = media::resp_profile(&s, body);
        assert!(resp.contains("Fault"));
        assert!(resp.contains("NoProfile"));
    }

    // ── Media2 video encoder config (stateful get/set) ───────────────────

    #[test]
    fn media2_get_video_encoder_configurations_returns_default() {
        use crate::mock::services::media2;
        let s = new_state();
        let xml =
            media2::resp_video_encoder_configurations(&s, "<tr2:GetVideoEncoderConfigurations/>");
        assert!(xml.contains("GetVideoEncoderConfigurationsResponse"));
        assert!(xml.contains(r#"token="VEC_1""#));
        assert!(xml.contains("<tt:Encoding>H265</tt:Encoding>"));
        assert!(xml.contains("<tt:Width>1920</tt:Width>"));
    }

    #[test]
    fn media2_set_video_encoder_then_get() {
        use crate::mock::services::media2;
        let s = new_state();
        let body = r#"<tr2:SetVideoEncoderConfiguration><tr2:Configuration token="VEC_1">
            <tt:Name>VideoEncoderConfig</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>H264</tt:Encoding>
            <tt:Resolution><tt:Width>1280</tt:Width><tt:Height>720</tt:Height></tt:Resolution>
            <tt:RateControl><tt:FrameRateLimit>25</tt:FrameRateLimit><tt:BitrateLimit>2048</tt:BitrateLimit></tt:RateControl>
            <tt:GovLength>60</tt:GovLength>
            <tt:Profile>High</tt:Profile>
            <tt:Quality>6</tt:Quality>
          </tr2:Configuration></tr2:SetVideoEncoderConfiguration>"#;
        let resp = media2::handle_set_video_encoder_configuration(&s, body);
        assert!(resp.contains("SetVideoEncoderConfigurationResponse"));

        let xml =
            media2::resp_video_encoder_configurations(&s, "<tr2:GetVideoEncoderConfigurations/>");
        assert!(xml.contains("<tt:Encoding>H264</tt:Encoding>"));
        assert!(xml.contains("<tt:Width>1280</tt:Width>"));
        assert!(xml.contains("<tt:BitrateLimit>2048</tt:BitrateLimit>"));
        assert!(xml.contains("<tt:Profile>High</tt:Profile>"));
        // Old default H265 must be gone after the Set.
        assert!(!xml.contains("H265"));
    }

    #[test]
    fn media2_get_video_encoder_configurations_filters_by_token() {
        use crate::mock::services::media2;
        let s = new_state();
        let xml = media2::resp_video_encoder_configurations(
            &s,
            r#"<tr2:GetVideoEncoderConfigurations><tr2:ConfigurationToken>OTHER</tr2:ConfigurationToken></tr2:GetVideoEncoderConfigurations>"#,
        );
        // Unknown token → response present but no configuration element.
        assert!(xml.contains("GetVideoEncoderConfigurationsResponse"));
        assert!(!xml.contains(r#"token="VEC_1""#));
    }
}
