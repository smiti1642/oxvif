use super::{xml_bool, xml_str};
use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── DeviceInfo ────────────────────────────────────────────────────────────────

/// Hardware and firmware information returned by `GetDeviceInformation`.
///
/// Absent fields in the device response are represented as empty strings.
#[derive(Debug, Clone, Default)]
pub struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub firmware_version: String,
    pub serial_number: String,
    pub hardware_id: String,
}

impl DeviceInfo {
    /// Parse from a `GetDeviceInformationResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        Ok(Self {
            manufacturer: xml_str(resp, "Manufacturer").unwrap_or_default(),
            model: xml_str(resp, "Model").unwrap_or_default(),
            firmware_version: xml_str(resp, "FirmwareVersion").unwrap_or_default(),
            serial_number: xml_str(resp, "SerialNumber").unwrap_or_default(),
            hardware_id: xml_str(resp, "HardwareId").unwrap_or_default(),
        })
    }
}

// ── SystemDateTime ────────────────────────────────────────────────────────────

/// Device clock information returned by `GetSystemDateAndTime`.
///
/// The primary use-case is computing the UTC offset for WS-Security:
///
/// ```no_run
/// # use oxvif::{OnvifClient, OnvifError};
/// # async fn run() -> Result<(), OnvifError> {
/// let client = OnvifClient::new("http://192.168.1.1/onvif/device_service");
/// let dt     = client.get_system_date_and_time().await?;
/// let client = client.with_utc_offset(dt.utc_offset_secs());
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
pub struct SystemDateTime {
    /// Device UTC clock as a Unix timestamp (seconds since 1970-01-01T00:00:00Z).
    /// `None` if the response contained no `UTCDateTime` element.
    pub utc_unix: Option<i64>,
    /// POSIX timezone string (e.g. `"CST-8"`).  Empty if absent.
    pub timezone: String,
    /// Whether daylight saving time is currently active on the device.
    pub daylight_savings: bool,
}

impl SystemDateTime {
    /// Seconds between the device UTC clock and the local system UTC clock.
    ///
    /// Returns `0` when `utc_unix` is `None`.
    /// Pass the result to [`OnvifClient::with_utc_offset`](crate::client::OnvifClient::with_utc_offset).
    pub fn utc_offset_secs(&self) -> i64 {
        match self.utc_unix {
            Some(device_utc) => {
                let local_utc = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                device_utc - local_utc
            }
            None => 0,
        }
    }

    /// Parse from a `GetSystemDateAndTimeResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let sdt = resp
            .child("SystemDateAndTime")
            .ok_or_else(|| SoapError::missing("SystemDateAndTime"))?;

        Ok(Self {
            utc_unix: sdt.child("UTCDateTime").and_then(parse_datetime_node),
            timezone: sdt
                .path(&["TimeZone", "TZ"])
                .map(|n| n.text().to_string())
                .unwrap_or_default(),
            daylight_savings: xml_bool(sdt, "DaylightSavings"),
        })
    }
}

/// Parse year/month/day/hour/minute/second from an ONVIF `DateTime` node and
/// return a Unix timestamp (seconds since epoch).
fn parse_datetime_node(node: &XmlNode) -> Option<i64> {
    let year = node
        .path(&["Date", "Year"])
        .and_then(|n| n.text().parse::<i32>().ok())?;
    let month = node
        .path(&["Date", "Month"])
        .and_then(|n| n.text().parse::<i32>().ok())?;
    let day = node
        .path(&["Date", "Day"])
        .and_then(|n| n.text().parse::<i32>().ok())?;
    let hour = node
        .path(&["Time", "Hour"])
        .and_then(|n| n.text().parse::<i32>().ok())?;
    let min = node
        .path(&["Time", "Minute"])
        .and_then(|n| n.text().parse::<i32>().ok())?;
    let sec = node
        .path(&["Time", "Second"])
        .and_then(|n| n.text().parse::<i32>().ok())?;
    Some(civil_to_unix(year, month, day, hour, min, sec))
}

/// Convert a proleptic Gregorian calendar date + time to a Unix timestamp.
/// Uses the Howard Hinnant days-from-civil algorithm.
pub(crate) fn civil_to_unix(year: i32, month: i32, day: i32, hour: i32, min: i32, sec: i32) -> i64 {
    let mut y = year as i64;
    let m = month as i64;
    if m <= 2 {
        y -= 1;
    }
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    days * 86_400 + hour as i64 * 3600 + min as i64 * 60 + sec as i64
}

// ── Hostname ──────────────────────────────────────────────────────────────────

/// Hostname configuration returned by `GetHostname`.
#[derive(Debug, Clone)]
pub struct Hostname {
    /// `true` if the hostname is assigned by DHCP rather than set manually.
    pub from_dhcp: bool,
    /// The configured hostname. `None` if no hostname is set.
    pub name: Option<String>,
}

impl Hostname {
    /// Parse from a `GetHostnameResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let info = resp
            .child("HostnameInformation")
            .ok_or_else(|| SoapError::missing("HostnameInformation"))?;
        Ok(Self {
            from_dhcp: xml_bool(info, "FromDHCP"),
            name: xml_str(info, "Name").filter(|s| !s.is_empty()),
        })
    }
}

// ── NtpInfo ───────────────────────────────────────────────────────────────────

/// NTP configuration returned by `GetNTP`.
#[derive(Debug, Clone)]
pub struct NtpInfo {
    /// `true` if NTP servers are obtained from DHCP rather than set manually.
    pub from_dhcp: bool,
    /// Manually configured NTP server addresses (DNS names or IP strings).
    /// Empty when `from_dhcp` is `true` or no servers are configured.
    pub servers: Vec<String>,
}

impl NtpInfo {
    /// Parse from a `GetNTPResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let info = resp
            .child("NTPInformation")
            .ok_or_else(|| SoapError::missing("NTPInformation"))?;
        Ok(Self {
            from_dhcp: xml_bool(info, "FromDHCP"),
            servers: info
                .children_named("NTPManual")
                .chain(info.children_named("NTPFromDHCP"))
                .filter_map(|entry| {
                    // Prefer DNS name, then IPv4, then IPv6
                    xml_str(entry, "DNSname")
                        .filter(|s| !s.is_empty())
                        .or_else(|| xml_str(entry, "IPv4Address").filter(|s| !s.is_empty()))
                        .or_else(|| xml_str(entry, "IPv6Address").filter(|s| !s.is_empty()))
                })
                .collect(),
        })
    }
}

// ── OnvifService ─────────────────────────────────────────────────────────────

/// A single service entry returned by `GetServices`.
///
/// `GetServices` is the proper ONVIF mechanism for discovering all service
/// endpoints, including Media2. Use [`OnvifService::is_media2`] to identify
/// the Media2 entry.
#[derive(Debug, Clone)]
pub struct OnvifService {
    /// Service namespace URI, e.g. `"http://www.onvif.org/ver20/media/wsdl"`.
    pub namespace: String,
    /// Service endpoint URL.
    pub url: String,
    pub version_major: u32,
    pub version_minor: u32,
}

impl OnvifService {
    /// Returns `true` if this entry is the Media2 service.
    pub fn is_media2(&self) -> bool {
        self.namespace == "http://www.onvif.org/ver20/media/wsdl"
    }

    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        Ok(resp
            .children_named("Service")
            .map(|s| Self {
                namespace: xml_str(s, "Namespace").unwrap_or_default(),
                url: xml_str(s, "XAddr").unwrap_or_default(),
                version_major: s
                    .path(&["Version", "Major"])
                    .and_then(|n| n.text().parse().ok())
                    .unwrap_or(0),
                version_minor: s
                    .path(&["Version", "Minor"])
                    .and_then(|n| n.text().parse().ok())
                    .unwrap_or(0),
            })
            .collect())
    }
}

// ── User ──────────────────────────────────────────────────────────────────────

/// A device user account returned by `GetUsers`.
#[derive(Debug, Clone)]
pub struct User {
    pub username: String,
    /// Access level: `"Administrator"`, `"Operator"`, `"User"`, `"Anonymous"`, or `"Extended"`.
    pub user_level: String,
}

impl User {
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("User")
            .map(|n| {
                let username = xml_str(n, "Username").unwrap_or_default();
                let user_level = xml_str(n, "UserLevel").unwrap_or_default();
                Ok(Self {
                    username,
                    user_level,
                })
            })
            .collect()
    }
}

// ── NetworkInterface ──────────────────────────────────────────────────────────

/// Network interface configuration returned by `GetNetworkInterfaces`.
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub token: String,
    pub enabled: bool,
    pub name: String,
    pub hw_address: String,
    pub mtu: u32,
    pub ipv4_enabled: bool,
    /// Manual or DHCP-assigned IPv4 address. Empty when DHCP is active and no address is available.
    pub ipv4_address: String,
    pub ipv4_prefix_length: u32,
    pub ipv4_from_dhcp: bool,
    /// `true` if the IPv6 stack is enabled on this interface.
    pub ipv6_enabled: bool,
    /// `true` if the IPv6 address is obtained via DHCPv6.
    pub ipv6_from_dhcp: bool,
    /// Manually configured or link-local IPv6 address, if available.
    pub ipv6_address: Option<String>,
}

impl NetworkInterface {
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("NetworkInterfaces")
            .map(|n| {
                let token = n
                    .attr("token")
                    .filter(|t| !t.is_empty())
                    .ok_or_else(|| SoapError::missing("NetworkInterfaces/@token"))?
                    .to_string();
                let enabled = xml_bool(n, "Enabled");
                let name = n
                    .path(&["Info", "Name"])
                    .map(|x| x.text().to_string())
                    .unwrap_or_default();
                let hw_address = n
                    .path(&["Info", "HwAddress"])
                    .map(|x| x.text().to_string())
                    .unwrap_or_default();
                let mtu = n
                    .path(&["Info", "MTU"])
                    .and_then(|x| x.text().parse().ok())
                    .unwrap_or(0);
                let ipv4_enabled = n
                    .path(&["IPv4", "Enabled"])
                    .map(|x| x.text() == "true" || x.text() == "1")
                    .unwrap_or(false);
                let ipv4_from_dhcp = n
                    .path(&["IPv4", "Config", "FromDHCP"])
                    .map(|x| x.text() == "true" || x.text() == "1")
                    .unwrap_or(false);
                let ipv4_address = n
                    .path(&["IPv4", "Config", "Manual", "Address"])
                    .map(|x| x.text().to_string())
                    .unwrap_or_default();
                let ipv4_prefix_length = n
                    .path(&["IPv4", "Config", "Manual", "PrefixLength"])
                    .and_then(|x| x.text().parse().ok())
                    .unwrap_or(0);
                let ipv6_enabled = n
                    .path(&["IPv6", "Enabled"])
                    .map(|x| x.text() == "true" || x.text() == "1")
                    .unwrap_or(false);
                let ipv6_from_dhcp = n
                    .path(&["IPv6", "Config", "DHCP"])
                    .map(|x| {
                        let t = x.text();
                        t == "Stateful" || t == "Stateless" || t == "Both"
                    })
                    .unwrap_or(false);
                let ipv6_address = n
                    .path(&["IPv6", "Config", "Manual", "Address"])
                    .map(|x| x.text().to_string())
                    .filter(|s| !s.is_empty())
                    .or_else(|| {
                        n.path(&["IPv6", "Config", "LinkLocal", "Address"])
                            .map(|x| x.text().to_string())
                            .filter(|s| !s.is_empty())
                    });
                Ok(Self {
                    token,
                    enabled,
                    name,
                    hw_address,
                    mtu,
                    ipv4_enabled,
                    ipv4_address,
                    ipv4_prefix_length,
                    ipv4_from_dhcp,
                    ipv6_enabled,
                    ipv6_from_dhcp,
                    ipv6_address,
                })
            })
            .collect()
    }
}

// ── NetworkProtocol ───────────────────────────────────────────────────────────

/// A network protocol entry returned by `GetNetworkProtocols`.
#[derive(Debug, Clone)]
pub struct NetworkProtocol {
    /// Protocol name, e.g. `"HTTP"`, `"HTTPS"`, `"RTSP"`.
    pub name: String,
    pub enabled: bool,
    /// Configured port numbers (typically one element).
    pub ports: Vec<u32>,
}

impl NetworkProtocol {
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        Ok(resp
            .children_named("NetworkProtocols")
            .map(|n| Self {
                name: xml_str(n, "Name").unwrap_or_default(),
                enabled: xml_bool(n, "Enabled"),
                ports: n
                    .children_named("Port")
                    .filter_map(|p| p.text().parse().ok())
                    .collect(),
            })
            .collect())
    }
}

// ── DnsInformation ────────────────────────────────────────────────────────────

/// DNS configuration returned by `GetDNS`.
#[derive(Debug, Clone)]
pub struct DnsInformation {
    pub from_dhcp: bool,
    /// Manually configured DNS server addresses (IPv4 or IPv6 strings).
    pub servers: Vec<String>,
    /// DNS search domain suffixes configured on the device.
    pub search_domains: Vec<String>,
}

impl DnsInformation {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let info = resp
            .child("DNSInformation")
            .ok_or_else(|| SoapError::missing("DNSInformation"))?;
        Ok(Self {
            from_dhcp: xml_bool(info, "FromDHCP"),
            servers: info
                .children_named("DNSManual")
                .chain(info.children_named("DNSFromDHCP"))
                .filter_map(|e| {
                    xml_str(e, "IPv4Address")
                        .filter(|s| !s.is_empty())
                        .or_else(|| xml_str(e, "IPv6Address").filter(|s| !s.is_empty()))
                        .or_else(|| xml_str(e, "DNSname").filter(|s| !s.is_empty()))
                })
                .collect(),
            search_domains: info
                .children_named("SearchDomain")
                .map(|n| n.text().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        })
    }
}

// ── NetworkGateway ────────────────────────────────────────────────────────────

/// Default gateway configuration returned by `GetNetworkDefaultGateway`.
#[derive(Debug, Clone)]
pub struct NetworkGateway {
    pub ipv4_addresses: Vec<String>,
    pub ipv6_addresses: Vec<String>,
}

impl NetworkGateway {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let gw = resp
            .child("NetworkGateway")
            .ok_or_else(|| SoapError::missing("NetworkGateway"))?;
        Ok(Self {
            ipv4_addresses: gw
                .children_named("IPv4Address")
                .filter_map(|n| {
                    let t = n.text().to_string();
                    if t.is_empty() { None } else { Some(t) }
                })
                .collect(),
            ipv6_addresses: gw
                .children_named("IPv6Address")
                .filter_map(|n| {
                    let t = n.text().to_string();
                    if t.is_empty() { None } else { Some(t) }
                })
                .collect(),
        })
    }
}

// ── SystemLog ─────────────────────────────────────────────────────────────────

/// System log content returned by `GetSystemLog`.
#[derive(Debug, Clone)]
pub struct SystemLog {
    /// Plain-text log content. `None` if the device returned binary data only.
    pub string: Option<String>,
}

impl SystemLog {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let log = resp
            .child("SystemLog")
            .ok_or_else(|| SoapError::missing("SystemLog"))?;
        Ok(Self {
            string: log
                .child("String")
                .map(|n| n.text().to_string())
                .filter(|s| !s.is_empty()),
        })
    }
}

// ── RelayOutput ───────────────────────────────────────────────────────────────

/// A relay output port returned by `GetRelayOutputs`.
#[derive(Debug, Clone)]
pub struct RelayOutput {
    pub token: String,
    /// `"Bistable"` (latching) or `"Monostable"` (timed).
    pub mode: String,
    /// ISO 8601 duration for monostable mode (e.g. `"PT1S"`).
    pub delay_time: String,
    /// Idle electrical state: `"closed"` or `"open"`.
    pub idle_state: String,
}

impl RelayOutput {
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("RelayOutputs")
            .map(|n| {
                let token = n
                    .attr("token")
                    .filter(|t| !t.is_empty())
                    .ok_or_else(|| SoapError::missing("RelayOutputs/@token"))?
                    .to_string();
                let props = n.child("Properties");
                let mode = props
                    .and_then(|p| p.child("Mode"))
                    .map(|x| x.text().to_string())
                    .unwrap_or_default();
                let delay_time = props
                    .and_then(|p| p.child("DelayTime"))
                    .map(|x| x.text().to_string())
                    .unwrap_or_default();
                let idle_state = props
                    .and_then(|p| p.child("IdleState"))
                    .map(|x| x.text().to_string())
                    .unwrap_or_default();
                Ok(Self {
                    token,
                    mode,
                    delay_time,
                    idle_state,
                })
            })
            .collect()
    }
}

// ── StorageConfiguration ──────────────────────────────────────────────────────

/// A storage location (SD card, NAS, etc.) returned by `GetStorageConfigurations`.
#[derive(Debug, Clone)]
pub struct StorageConfiguration {
    pub token: String,
    /// `"LocalStorage"` or `"NFS"`.
    pub storage_type: String,
    /// Mount path on the device (e.g. `"/mnt/sd"`).
    pub local_path: String,
    /// Network URI for NFS shares.
    pub storage_uri: String,
    /// Username for authenticated shares (empty if anonymous or local).
    pub user: String,
    /// Whether anonymous access is used.
    pub use_anonymous: bool,
    /// Operational status of the storage location (e.g. `"Connected"`, `"NotConnected"`).
    pub storage_status: Option<String>,
}

impl StorageConfiguration {
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("StorageConfigurations")
            .map(|n| {
                let token = n
                    .attr("token")
                    .filter(|t| !t.is_empty())
                    .ok_or_else(|| SoapError::missing("StorageConfigurations/@token"))?
                    .to_string();
                let storage_type = xml_str(n, "StorageType").unwrap_or_default();
                let local_path = xml_str(n, "LocalPath").unwrap_or_default();
                let storage_uri = xml_str(n, "StorageUri").unwrap_or_default();
                let user = n
                    .child("UserInfo")
                    .and_then(|u| u.child("Username"))
                    .map(|x| x.text().to_string())
                    .unwrap_or_default();
                let use_anonymous = n
                    .child("UserInfo")
                    .and_then(|u| u.child("UseAnonymous"))
                    .map(|x| x.text() == "true" || x.text() == "1")
                    .unwrap_or(false);
                let storage_status = xml_str(n, "StorageStatus").filter(|s| !s.is_empty());
                Ok(Self {
                    token,
                    storage_type,
                    local_path,
                    storage_uri,
                    user,
                    use_anonymous,
                    storage_status,
                })
            })
            .collect()
    }
}

// ── SystemUris ────────────────────────────────────────────────────────────────

/// HTTP URIs for system management tasks returned by `GetSystemUris`.
#[derive(Debug, Clone)]
pub struct SystemUris {
    /// URI for uploading a firmware image.
    pub firmware_upgrade_uri: Option<String>,
    /// URI for downloading the system log.
    pub system_log_uri: Option<String>,
    /// URI for downloading a support-info bundle.
    pub support_info_uri: Option<String>,
}

impl SystemUris {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        Ok(Self {
            firmware_upgrade_uri: xml_str(resp, "FirmwareUpgrade")
                .or_else(|| xml_str(resp, "FirmwareUpgradeUri")),
            system_log_uri: xml_str(resp, "SystemLog").or_else(|| xml_str(resp, "SystemLogUri")),
            support_info_uri: xml_str(resp, "SupportInfo")
                .or_else(|| xml_str(resp, "SupportInfoUri")),
        })
    }
}
