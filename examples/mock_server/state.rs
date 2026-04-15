//! Mutable device state for the stateful mock server.

use std::sync::RwLock;

/// All mutable device settings. Protected by `RwLock` in `MockState`.
pub struct DeviceState {
    // ── Device info (read-only, but stored here for consistency) ─────────
    pub manufacturer: String,
    pub model: String,
    pub firmware_version: String,
    pub serial_number: String,
    pub hardware_id: String,

    // ── Hostname ────────────────────────────────────────────────────────
    pub hostname: String,
    pub hostname_from_dhcp: bool,

    // ── Users ───────────────────────────────────────────────────────────
    pub users: Vec<MockUser>,

    // ── Scopes ──────────────────────────────────────────────────────────
    pub scopes: Vec<String>,

    // ── Date/Time ───────────────────────────────────────────────────────
    pub timezone: String,
    pub daylight_savings: bool,

    // ── DNS ─────────────────────────────────────────────────────────────
    pub dns_servers: Vec<String>,
    pub dns_from_dhcp: bool,

    // ── NTP ─────────────────────────────────────────────────────────────
    pub ntp_servers: Vec<String>,
    pub ntp_from_dhcp: bool,

    // ── Network ─────────────────────────────────────────────────────────
    pub gateway_ipv4: Vec<String>,
    pub discovery_mode: String,
}

pub struct MockUser {
    pub username: String,
    pub level: String,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            manufacturer: "oxvif-mock".to_string(),
            model: "MockCam-1080p".to_string(),
            firmware_version: "1.0.0".to_string(),
            serial_number: "MOCK-0001".to_string(),
            hardware_id: "1.0".to_string(),

            hostname: "mock-camera".to_string(),
            hostname_from_dhcp: false,

            users: vec![
                MockUser { username: "admin".to_string(), level: "Administrator".to_string() },
                MockUser { username: "operator".to_string(), level: "Operator".to_string() },
            ],

            scopes: vec![
                "onvif://www.onvif.org/name/MockCamera".to_string(),
                "onvif://www.onvif.org/type/video_encoder".to_string(),
                "onvif://www.onvif.org/location/country/taiwan".to_string(),
            ],

            timezone: "UTC".to_string(),
            daylight_savings: false,

            dns_servers: vec!["8.8.8.8".to_string(), "8.8.4.4".to_string()],
            dns_from_dhcp: false,

            ntp_servers: vec!["pool.ntp.org".to_string()],
            ntp_from_dhcp: false,

            gateway_ipv4: vec!["192.168.1.1".to_string()],
            discovery_mode: "Discoverable".to_string(),
        }
    }
}

/// Thread-safe wrapper for device state.
pub type SharedState = RwLock<DeviceState>;
