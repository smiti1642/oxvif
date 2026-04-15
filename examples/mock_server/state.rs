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
                MockUser {
                    username: "admin".to_string(),
                    level: "Administrator".to_string(),
                },
                MockUser {
                    username: "operator".to_string(),
                    level: "Operator".to_string(),
                },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::device;

    fn new_state() -> SharedState {
        RwLock::new(DeviceState::default())
    }

    // ── Hostname ────────────────────────────────────────────────────────

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
        assert!(xml.contains("new-host"), "hostname should be updated");
        assert!(!xml.contains("mock-camera"), "old hostname should be gone");
    }

    // ── Users ───────────────────────────────────────────────────────────

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
        assert!(xml.contains("User"));
    }

    #[test]
    fn delete_user_then_get() {
        let s = new_state();
        let body = r#"<tds:DeleteUsers><tt:Username>operator</tt:Username></tds:DeleteUsers>"#;
        device::handle_delete_users(&s, body);
        let xml = device::resp_users(&s);
        assert!(xml.contains("admin"), "admin should remain");
        assert!(!xml.contains("operator"), "operator should be deleted");
    }

    #[test]
    fn set_user_level_then_get() {
        let s = new_state();
        let body = r#"<tds:SetUser><tds:User><tt:Username>operator</tt:Username><tt:UserLevel>Administrator</tt:UserLevel></tds:User></tds:SetUser>"#;
        device::handle_set_user(&s, body);
        let xml = device::resp_users(&s);
        // operator should now be Administrator, not Operator
        assert!(xml.contains("operator"));
        // Count occurrences of "Administrator" — should be 2 (admin + operator)
        assert_eq!(xml.matches("Administrator").count(), 2);
    }

    // ── DNS ─────────────────────────────────────────────────────────────

    #[test]
    fn set_dns_then_get() {
        let s = new_state();
        let body = r#"<tds:SetDNS><tt:FromDHCP>false</tt:FromDHCP><tt:DNSManual><tt:Type>IPv4</tt:Type><tt:IPv4Address>1.1.1.1</tt:IPv4Address></tt:DNSManual></tds:SetDNS>"#;
        device::handle_set_dns(&s, body);
        let xml = device::resp_dns(&s);
        assert!(xml.contains("1.1.1.1"), "new DNS should appear");
        assert!(!xml.contains("8.8.8.8"), "old DNS should be gone");
    }

    // ── NTP ─────────────────────────────────────────────────────────────

    #[test]
    fn set_ntp_then_get() {
        let s = new_state();
        let body = r#"<tds:SetNTP><tt:FromDHCP>false</tt:FromDHCP><tt:NTPManual><tt:Type>DNS</tt:Type><tt:DNSname>time.google.com</tt:DNSname></tt:NTPManual></tds:SetNTP>"#;
        device::handle_set_ntp(&s, body);
        let xml = device::resp_ntp(&s);
        assert!(xml.contains("time.google.com"));
        assert!(!xml.contains("pool.ntp.org"));
    }

    // ── Scopes ──────────────────────────────────────────────────────────

    #[test]
    fn set_scopes_then_get() {
        let s = new_state();
        let body = r#"<tds:SetScopes><tt:ScopeItem>onvif://www.onvif.org/name/NewCam</tt:ScopeItem></tds:SetScopes>"#;
        device::handle_set_scopes(&s, body);
        let xml = device::resp_scopes(&s);
        assert!(xml.contains("NewCam"));
        assert!(!xml.contains("MockCamera"), "old scope should be replaced");
    }

    // ── DateTime ────────────────────────────────────────────────────────

    #[test]
    fn set_timezone_then_get() {
        let s = new_state();
        let body = r#"<tds:SetSystemDateAndTime><tt:TimeZone><tt:TZ>CST-8</tt:TZ></tt:TimeZone><tt:DaylightSavings>true</tt:DaylightSavings></tds:SetSystemDateAndTime>"#;
        device::handle_set_system_date_and_time(&s, body);
        let xml = device::resp_system_date_and_time(&s);
        assert!(xml.contains("CST-8"), "timezone should be updated");
        assert!(xml.contains("<tt:DaylightSavings>true</tt:DaylightSavings>"));
    }

    // ── DeviceInfo ──────────────────────────────────────────────────────

    #[test]
    fn device_info_reads_from_state() {
        let s = new_state();
        let xml = device::resp_device_info(&s);
        assert!(xml.contains("oxvif-mock"));
        assert!(xml.contains("MockCam-1080p"));
        assert!(xml.contains("MOCK-0001"));
    }
}
