//! Typed response structs for ONVIF operations.
//!
//! Each struct corresponds to the response of one ONVIF operation and is
//! parsed from the relevant `<…Response>` XML node by a `pub(crate)`
//! `from_xml` / `vec_from_xml` method. All parsing logic lives here, keeping
//! [`client`](crate::client) free of XML concerns.
//!
//! | Struct | Source operation |
//! |--------|-----------------|
//! | [`Capabilities`]  | `GetCapabilities`       |
//! | [`DeviceInfo`]    | `GetDeviceInformation`  |
//! | [`MediaProfile`]  | `GetProfiles`           |
//! | [`StreamUri`]     | `GetStreamUri`          |

use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── XML helpers ───────────────────────────────────────────────────────────────

/// Parse a boolean child element. Returns `true` for `"true"` or `"1"`.
fn xml_bool(node: &XmlNode, child: &str) -> bool {
    node.child(child)
        .is_some_and(|n| n.text() == "true" || n.text() == "1")
}

/// Parse an optional `u32` child element.
fn xml_u32(node: &XmlNode, child: &str) -> Option<u32> {
    node.child(child).and_then(|n| n.text().parse().ok())
}

/// Extract the text of a child element as an owned `String`.
fn xml_str(node: &XmlNode, child: &str) -> Option<String> {
    node.child(child).map(|n| n.text().to_string())
}

// ── Capabilities sub-structs ──────────────────────────────────────────────────

/// Network capabilities from `Device/Network`.
#[derive(Debug, Clone, Default)]
pub struct NetworkCapabilities {
    pub ip_filter: bool,
    pub zero_configuration: bool,
    pub ip_version6: bool,
    pub dyn_dns: bool,
}

/// System capabilities from `Device/System`.
#[derive(Debug, Clone, Default)]
pub struct SystemCapabilities {
    pub discovery_resolve: bool,
    pub discovery_bye: bool,
    pub remote_discovery: bool,
    pub system_backup: bool,
    pub system_logging: bool,
    pub firmware_upgrade: bool,
}

/// I/O capabilities from `Device/IO`.
#[derive(Debug, Clone, Default)]
pub struct IoCapabilities {
    /// Number of digital inputs on the device.
    pub input_connectors: Option<u32>,
    /// Number of relay outputs on the device.
    pub relay_outputs: Option<u32>,
}

/// Security capabilities from `Device/Security`.
#[derive(Debug, Clone, Default)]
pub struct SecurityCapabilities {
    pub tls_1_2: bool,
    pub onboard_key_generation: bool,
    pub access_policy_config: bool,
    pub x509_token: bool,
    /// `true` if the device supports WS-Security `UsernameToken`.
    pub username_token: bool,
}

/// Device management service capabilities.
#[derive(Debug, Clone, Default)]
pub struct DeviceCapabilities {
    /// Device management service endpoint URL.
    pub url: Option<String>,
    pub network: NetworkCapabilities,
    pub system: SystemCapabilities,
    pub io: IoCapabilities,
    pub security: SecurityCapabilities,
}

/// RTP streaming capabilities from `Media/StreamingCapabilities`.
#[derive(Debug, Clone, Default)]
pub struct StreamingCapabilities {
    pub rtp_multicast: bool,
    pub rtp_tcp: bool,
    pub rtp_rtsp_tcp: bool,
}

/// Media service capabilities.
#[derive(Debug, Clone, Default)]
pub struct MediaCapabilities {
    /// Media service endpoint URL.
    pub url: Option<String>,
    pub streaming: StreamingCapabilities,
    /// Maximum number of media profiles the device supports.
    pub max_profiles: Option<u32>,
}

/// Events service capabilities.
#[derive(Debug, Clone, Default)]
pub struct EventsCapabilities {
    /// Events service endpoint URL.
    pub url: Option<String>,
    /// `true` if WS-BaseNotification (push) subscriptions are supported.
    pub ws_subscription_policy: bool,
    /// `true` if WS-PullPoint subscriptions are supported.
    pub ws_pull_point: bool,
}

/// Analytics service capabilities.
#[derive(Debug, Clone, Default)]
pub struct AnalyticsCapabilities {
    /// Analytics service endpoint URL.
    pub url: Option<String>,
    pub rule_support: bool,
    pub analytics_module_support: bool,
}

// ── Capabilities ──────────────────────────────────────────────────────────────

/// Full device capabilities returned by `GetCapabilities`.
///
/// Top-level service structs have a `url` field for the service endpoint.
/// Absent services have `url: None` and boolean fields default to `false`.
///
/// # Usage
///
/// ```no_run
/// # use oxvif::{OnvifClient, OnvifError};
/// # async fn run() -> Result<(), OnvifError> {
/// let client = OnvifClient::new("http://192.168.1.1/onvif/device_service");
/// let caps = client.get_capabilities().await?;
///
/// if let Some(media_url) = &caps.media.url {
///     let profiles = client.get_profiles(media_url).await?;
/// }
///
/// // Check before attempting firmware upgrade
/// if caps.device.system.firmware_upgrade {
///     println!("Device supports firmware upgrade");
/// }
///
/// // Choose streaming protocol
/// if caps.media.streaming.rtp_rtsp_tcp {
///     println!("RTSP/TCP streaming supported");
/// }
/// # Ok(()) }
/// ```
#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    pub device: DeviceCapabilities,
    pub media: MediaCapabilities,
    pub events: EventsCapabilities,
    pub analytics: AnalyticsCapabilities,
    /// PTZ service endpoint URL (`None` if not supported).
    pub ptz_url: Option<String>,
    /// Imaging service endpoint URL (`None` if not supported).
    pub imaging_url: Option<String>,
    // Extension services
    /// Recording service endpoint URL (`None` if not supported).
    pub recording_url: Option<String>,
    /// Search service endpoint URL (`None` if not supported).
    pub search_url: Option<String>,
    /// Replay service endpoint URL (`None` if not supported).
    pub replay_url: Option<String>,
    /// DeviceIO service endpoint URL (`None` if not supported).
    pub device_io_url: Option<String>,
    /// Media2 service endpoint URL (`None` if device does not support Media2).
    pub media2_url: Option<String>,
}

impl Capabilities {
    /// Parse from a `GetCapabilitiesResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let caps = resp
            .child("Capabilities")
            .ok_or_else(|| SoapError::missing("Capabilities"))?;

        Ok(Self {
            device: caps
                .child("Device")
                .map(parse_device_caps)
                .unwrap_or_default(),
            media: caps
                .child("Media")
                .map(parse_media_caps)
                .unwrap_or_default(),
            events: caps
                .child("Events")
                .map(parse_events_caps)
                .unwrap_or_default(),
            analytics: caps
                .child("Analytics")
                .map(parse_analytics_caps)
                .unwrap_or_default(),
            ptz_url: caps.path(&["PTZ", "XAddr"]).map(|n| n.text().to_string()),
            imaging_url: caps
                .path(&["Imaging", "XAddr"])
                .map(|n| n.text().to_string()),
            recording_url: caps
                .path(&["Extension", "Recording", "XAddr"])
                .map(|n| n.text().to_string()),
            search_url: caps
                .path(&["Extension", "Search", "XAddr"])
                .map(|n| n.text().to_string()),
            replay_url: caps
                .path(&["Extension", "Replay", "XAddr"])
                .map(|n| n.text().to_string()),
            device_io_url: caps
                .path(&["Extension", "DeviceIO", "XAddr"])
                .map(|n| n.text().to_string()),
            media2_url: caps
                .path(&["Extension", "Media2", "XAddr"])
                .map(|n| n.text().to_string()),
        })
    }
}

fn parse_device_caps(d: &XmlNode) -> DeviceCapabilities {
    DeviceCapabilities {
        url: xml_str(d, "XAddr"),
        network: d
            .child("Network")
            .map(|n| NetworkCapabilities {
                ip_filter: xml_bool(n, "IPFilter"),
                zero_configuration: xml_bool(n, "ZeroConfiguration"),
                ip_version6: xml_bool(n, "IPVersion6"),
                dyn_dns: xml_bool(n, "DynDNS"),
            })
            .unwrap_or_default(),
        system: d
            .child("System")
            .map(|n| SystemCapabilities {
                discovery_resolve: xml_bool(n, "DiscoveryResolve"),
                discovery_bye: xml_bool(n, "DiscoveryBye"),
                remote_discovery: xml_bool(n, "RemoteDiscovery"),
                system_backup: xml_bool(n, "SystemBackup"),
                system_logging: xml_bool(n, "SystemLogging"),
                firmware_upgrade: xml_bool(n, "FirmwareUpgrade"),
            })
            .unwrap_or_default(),
        io: d
            .child("IO")
            .map(|n| IoCapabilities {
                input_connectors: xml_u32(n, "InputConnectors"),
                relay_outputs: xml_u32(n, "RelayOutputs"),
            })
            .unwrap_or_default(),
        security: d
            .child("Security")
            .map(|n| SecurityCapabilities {
                tls_1_2: xml_bool(n, "TLS1.2"),
                onboard_key_generation: xml_bool(n, "OnboardKeyGeneration"),
                access_policy_config: xml_bool(n, "AccessPolicyConfig"),
                x509_token: xml_bool(n, "X.509Token"),
                username_token: xml_bool(n, "UsernameToken"),
            })
            .unwrap_or_default(),
    }
}

fn parse_media_caps(m: &XmlNode) -> MediaCapabilities {
    MediaCapabilities {
        url: xml_str(m, "XAddr"),
        streaming: m
            .child("StreamingCapabilities")
            .map(|n| StreamingCapabilities {
                rtp_multicast: xml_bool(n, "RTPMulticast"),
                rtp_tcp: xml_bool(n, "RTP_TCP"),
                rtp_rtsp_tcp: xml_bool(n, "RTP_RTSP_TCP"),
            })
            .unwrap_or_default(),
        max_profiles: xml_u32(m, "MaximumNumberOfProfiles"),
    }
}

fn parse_events_caps(e: &XmlNode) -> EventsCapabilities {
    EventsCapabilities {
        url: xml_str(e, "XAddr"),
        ws_subscription_policy: xml_bool(e, "WSSubscriptionPolicySupport"),
        ws_pull_point: xml_bool(e, "WSPullPointSupport"),
    }
}

fn parse_analytics_caps(a: &XmlNode) -> AnalyticsCapabilities {
    AnalyticsCapabilities {
        url: xml_str(a, "XAddr"),
        rule_support: xml_bool(a, "RuleSupport"),
        analytics_module_support: xml_bool(a, "AnalyticsModuleSupport"),
    }
}

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

// ── MediaProfile ──────────────────────────────────────────────────────────────

/// A single media profile returned by `GetProfiles`.
///
/// Pass `token` to [`get_stream_uri`](crate::client::OnvifClient::get_stream_uri)
/// to retrieve the RTSP URI for this profile.
#[derive(Debug, Clone)]
pub struct MediaProfile {
    /// Opaque identifier used in subsequent media service calls.
    pub token: String,
    /// Human-readable profile name (e.g. `"mainStream"`, `"subStream"`).
    pub name: String,
    /// `true` if the profile is fixed and cannot be deleted.
    pub fixed: bool,
}

impl MediaProfile {
    /// Parse all `<trt:Profiles>` children from a `GetProfilesResponse` node.
    /// Returns an empty `Vec` if the response contains no profiles.
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        Ok(resp
            .children_named("Profiles")
            .map(|p| Self {
                token: p.attr("token").unwrap_or("").to_string(),
                fixed: p.attr("fixed") == Some("true"),
                name: xml_str(p, "Name").unwrap_or_default(),
            })
            .collect())
    }
}

// ── StreamUri ─────────────────────────────────────────────────────────────────

/// RTSP stream URI returned by `GetStreamUri`.
#[derive(Debug, Clone)]
pub struct StreamUri {
    /// The RTSP URI to open with a media player (e.g. `rtsp://…/stream`).
    pub uri: String,
    /// If `true`, the URI becomes invalid after the first RTSP connection.
    pub invalid_after_connect: bool,
    /// If `true`, the URI becomes invalid after the device reboots.
    pub invalid_after_reboot: bool,
    /// ISO 8601 duration until the URI expires (e.g. `"PT0S"` = no expiry).
    pub timeout: String,
}

impl StreamUri {
    /// Parse from a `GetStreamUriResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let media_uri = resp
            .child("MediaUri")
            .ok_or_else(|| SoapError::missing("MediaUri"))?;

        let uri = media_uri
            .child("Uri")
            .map(|n| n.text().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| SoapError::missing("Uri"))?;

        Ok(Self {
            uri,
            invalid_after_connect: xml_bool(media_uri, "InvalidAfterConnect"),
            invalid_after_reboot: xml_bool(media_uri, "InvalidAfterReboot"),
            timeout: xml_str(media_uri, "Timeout").unwrap_or_default(),
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
fn civil_to_unix(year: i32, month: i32, day: i32, hour: i32, min: i32, sec: i32) -> i64 {
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

// ── SnapshotUri ───────────────────────────────────────────────────────────────

/// HTTP snapshot URI returned by `GetSnapshotUri`.
///
/// Fetch the URI with any HTTP client to retrieve a JPEG image.
#[derive(Debug, Clone)]
pub struct SnapshotUri {
    /// HTTP URL of the JPEG snapshot endpoint.
    pub uri: String,
    /// If `true`, the URI becomes invalid after the first HTTP request.
    pub invalid_after_connect: bool,
    /// If `true`, the URI becomes invalid after the device reboots.
    pub invalid_after_reboot: bool,
    /// ISO 8601 duration until the URI expires (e.g. `"PT0S"` = no expiry).
    pub timeout: String,
}

impl SnapshotUri {
    /// Parse from a `GetSnapshotUriResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let media_uri = resp
            .child("MediaUri")
            .ok_or_else(|| SoapError::missing("MediaUri"))?;

        let uri = media_uri
            .child("Uri")
            .map(|n| n.text().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| SoapError::missing("Uri"))?;

        Ok(Self {
            uri,
            invalid_after_connect: xml_bool(media_uri, "InvalidAfterConnect"),
            invalid_after_reboot: xml_bool(media_uri, "InvalidAfterReboot"),
            timeout: xml_str(media_uri, "Timeout").unwrap_or_default(),
        })
    }
}

// ── PtzPreset ─────────────────────────────────────────────────────────────────

/// A named PTZ preset position returned by `GetPresets`.
#[derive(Debug, Clone)]
pub struct PtzPreset {
    /// Opaque preset identifier; pass to `ptz_goto_preset`.
    pub token: String,
    /// Human-readable preset name.
    pub name: String,
    /// Stored pan (x) and tilt (y) position, range `[-1.0, 1.0]`.
    /// `None` if the preset has no stored position.
    pub pan_tilt: Option<(f32, f32)>,
    /// Stored zoom position, range `[0.0, 1.0]`.
    /// `None` if the preset has no stored zoom.
    pub zoom: Option<f32>,
}

impl PtzPreset {
    /// Parse all `<Preset>` children from a `GetPresetsResponse` node.
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        Ok(resp
            .children_named("Preset")
            .map(|p| Self {
                token: p.attr("token").unwrap_or("").to_string(),
                name: xml_str(p, "Name").unwrap_or_default(),
                pan_tilt: p.path(&["PTZPosition", "PanTilt"]).and_then(|n| {
                    let x = n.attr("x")?.parse().ok()?;
                    let y = n.attr("y")?.parse().ok()?;
                    Some((x, y))
                }),
                zoom: p
                    .path(&["PTZPosition", "Zoom"])
                    .and_then(|n| n.attr("x")?.parse().ok()),
            })
            .collect())
    }
}

// ── Shared primitives ─────────────────────────────────────────────────────────

/// Width × height in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl std::fmt::Display for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// Integer min/max range (inclusive).
#[derive(Debug, Clone, Copy, Default)]
pub struct IntRange {
    pub min: i32,
    pub max: i32,
}

/// Floating-point min/max range (inclusive).
#[derive(Debug, Clone, Copy, Default)]
pub struct FloatRange {
    pub min: f32,
    pub max: f32,
}

fn parse_resolution(node: &XmlNode) -> Option<Resolution> {
    Some(Resolution {
        width: xml_u32(node, "Width")?,
        height: xml_u32(node, "Height")?,
    })
}

fn parse_int_range_node(node: &XmlNode) -> IntRange {
    IntRange {
        min: node
            .child("Min")
            .and_then(|n| n.text().parse().ok())
            .unwrap_or(0),
        max: node
            .child("Max")
            .and_then(|n| n.text().parse().ok())
            .unwrap_or(0),
    }
}

// ── VideoSource ───────────────────────────────────────────────────────────────

/// A physical video input channel returned by `GetVideoSources`.
#[derive(Debug, Clone)]
pub struct VideoSource {
    /// Opaque token identifying this physical input.
    pub token: String,
    /// Maximum frame rate this input can deliver.
    pub framerate: f32,
    /// Native resolution of this input.
    pub resolution: Resolution,
}

impl VideoSource {
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        Ok(resp
            .children_named("VideoSources")
            .map(|n| Self {
                token: n.attr("token").unwrap_or("").to_string(),
                framerate: n
                    .child("Framerate")
                    .and_then(|f| f.text().parse().ok())
                    .unwrap_or(0.0),
                resolution: n
                    .child("Resolution")
                    .and_then(parse_resolution)
                    .unwrap_or_default(),
            })
            .collect())
    }
}

// ── VideoSourceConfiguration ──────────────────────────────────────────────────

/// Rectangular crop/position window applied to a physical video source.
///
/// Returned by `GetVideoSourceConfiguration(s)`.
/// Pass a modified copy to `SetVideoSourceConfiguration`.
#[derive(Debug, Clone)]
pub struct VideoSourceConfiguration {
    /// Opaque token for this configuration.
    pub token: String,
    pub name: String,
    /// Number of profiles currently referencing this configuration.
    pub use_count: u32,
    /// Token of the physical `VideoSource` this config reads from.
    pub source_token: String,
    /// Crop window within the physical sensor.
    pub bounds: SourceBounds,
}

/// Rectangular region within a video source, in pixels.
#[derive(Debug, Clone, Default)]
pub struct SourceBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl VideoSourceConfiguration {
    pub(crate) fn from_xml(node: &XmlNode) -> Result<Self, OnvifError> {
        Ok(Self {
            token: node.attr("token").unwrap_or("").to_string(),
            name: xml_str(node, "Name").unwrap_or_default(),
            use_count: xml_u32(node, "UseCount").unwrap_or(0),
            source_token: xml_str(node, "SourceToken").unwrap_or_default(),
            bounds: node
                .child("Bounds")
                .map(|b| SourceBounds {
                    x: b.attr("x").and_then(|v| v.parse().ok()).unwrap_or(0),
                    y: b.attr("y").and_then(|v| v.parse().ok()).unwrap_or(0),
                    width: b.attr("width").and_then(|v| v.parse().ok()).unwrap_or(0),
                    height: b.attr("height").and_then(|v| v.parse().ok()).unwrap_or(0),
                })
                .unwrap_or_default(),
        })
    }

    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("Configurations")
            .map(Self::from_xml)
            .collect()
    }

    /// Serialise to a `<trt:Configuration>` XML fragment for `SetVideoSourceConfiguration`.
    pub(crate) fn to_xml_body(&self) -> String {
        format!(
            "<trt:Configuration token=\"{token}\">\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:SourceToken>{source_token}</tt:SourceToken>\
               <tt:Bounds x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"/>\
             </trt:Configuration>",
            token = self.token,
            name = self.name,
            use_count = self.use_count,
            source_token = self.source_token,
            x = self.bounds.x,
            y = self.bounds.y,
            w = self.bounds.width,
            h = self.bounds.height,
        )
    }

    /// Serialise to a `<tr2:Configuration>` XML fragment for `SetVideoSourceConfiguration` (Media2).
    pub(crate) fn to_xml_body_media2(&self) -> String {
        format!(
            "<tr2:Configuration token=\"{token}\">\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:SourceToken>{source_token}</tt:SourceToken>\
               <tt:Bounds x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"/>\
             </tr2:Configuration>",
            token = self.token,
            name = self.name,
            use_count = self.use_count,
            source_token = self.source_token,
            x = self.bounds.x,
            y = self.bounds.y,
            w = self.bounds.width,
            h = self.bounds.height,
        )
    }
}

// ── VideoSourceConfigurationOptions ──────────────────────────────────────────

/// Valid parameter ranges for `SetVideoSourceConfiguration`.
#[derive(Debug, Clone, Default)]
pub struct VideoSourceConfigurationOptions {
    /// Available video source tokens that can be referenced.
    pub source_tokens: Vec<String>,
    /// Maximum profiles that may reference a single video source configuration.
    pub max_limit: Option<u32>,
    /// Valid ranges for the `bounds` crop window.
    pub bounds_range: Option<BoundsRange>,
}

/// Valid coordinate ranges for `SourceBounds`.
#[derive(Debug, Clone, Default)]
pub struct BoundsRange {
    pub x_range: IntRange,
    pub y_range: IntRange,
    pub width_range: IntRange,
    pub height_range: IntRange,
}

impl VideoSourceConfigurationOptions {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let opts = resp
            .child("Options")
            .ok_or_else(|| SoapError::missing("Options"))?;
        Ok(Self {
            max_limit: xml_u32(opts, "MaximumNumberOfProfiles"),
            bounds_range: opts.child("BoundsRange").map(|br| BoundsRange {
                x_range: br
                    .child("XRange")
                    .map(parse_int_range_node)
                    .unwrap_or_default(),
                y_range: br
                    .child("YRange")
                    .map(parse_int_range_node)
                    .unwrap_or_default(),
                width_range: br
                    .child("WidthRange")
                    .map(parse_int_range_node)
                    .unwrap_or_default(),
                height_range: br
                    .child("HeightRange")
                    .map(parse_int_range_node)
                    .unwrap_or_default(),
            }),
            source_tokens: opts
                .children_named("VideoSourceTokensAvailable")
                .map(|n| n.text().to_string())
                .collect(),
        })
    }
}

// ── VideoEncoding ─────────────────────────────────────────────────────────────

/// Video compression format.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum VideoEncoding {
    Jpeg,
    #[default]
    H264,
    H265,
    Other(String),
}

impl VideoEncoding {
    fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "JPEG" => Self::Jpeg,
            "H264" => Self::H264,
            "H265" | "H.265" => Self::H265,
            _ => Self::Other(s.to_string()),
        }
    }

    /// Returns the ONVIF wire string for this encoding (e.g. `"H264"`).
    pub fn as_str(&self) -> &str {
        match self {
            Self::Jpeg => "JPEG",
            Self::H264 => "H264",
            Self::H265 => "H265",
            Self::Other(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for VideoEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── VideoEncoderConfiguration ─────────────────────────────────────────────────

/// Video codec settings for one stream, returned by `GetVideoEncoderConfiguration(s)`.
///
/// Pass a modified copy to `SetVideoEncoderConfiguration` to change resolution,
/// frame rate, bitrate, or codec profile.
#[derive(Debug, Clone)]
pub struct VideoEncoderConfiguration {
    /// Opaque token for this configuration.
    pub token: String,
    pub name: String,
    /// Number of profiles currently referencing this configuration.
    pub use_count: u32,
    pub encoding: VideoEncoding,
    pub resolution: Resolution,
    /// Encoder quality level. Valid range is device-specific; see `GetVideoEncoderConfigurationOptions`.
    pub quality: f32,
    pub rate_control: Option<VideoRateControl>,
    /// H.264 specific settings; `None` when `encoding != H264`.
    pub h264: Option<H264Configuration>,
    /// H.265 specific settings; `None` when `encoding != H265`.
    pub h265: Option<H265Configuration>,
}

/// Frame rate, encoding interval, and bitrate limits.
#[derive(Debug, Clone)]
pub struct VideoRateControl {
    /// Maximum frames per second the encoder produces.
    pub frame_rate_limit: u32,
    /// Encode every Nth frame (1 = all frames).
    pub encoding_interval: u32,
    /// Maximum bitrate in kbps.
    pub bitrate_limit: u32,
}

/// H.264-specific codec settings.
#[derive(Debug, Clone)]
pub struct H264Configuration {
    /// Group-of-pictures length (keyframe interval in frames).
    pub gov_length: u32,
    /// H.264 profile: `"Baseline"`, `"Main"`, `"High"`, or `"Extended"`.
    pub profile: String,
}

/// H.265-specific codec settings.
#[derive(Debug, Clone)]
pub struct H265Configuration {
    /// Group-of-pictures length (keyframe interval in frames).
    pub gov_length: u32,
    /// H.265 profile: `"Main"`, `"Main10"`, etc.
    pub profile: String,
}

impl VideoEncoderConfiguration {
    pub(crate) fn from_xml(node: &XmlNode) -> Result<Self, OnvifError> {
        Ok(Self {
            token: node.attr("token").unwrap_or("").to_string(),
            name: xml_str(node, "Name").unwrap_or_default(),
            use_count: xml_u32(node, "UseCount").unwrap_or(0),
            encoding: xml_str(node, "Encoding")
                .map(|s| VideoEncoding::from_str(&s))
                .unwrap_or_default(),
            resolution: node
                .child("Resolution")
                .and_then(parse_resolution)
                .unwrap_or_default(),
            quality: node
                .child("Quality")
                .and_then(|n| n.text().parse().ok())
                .unwrap_or(0.0),
            rate_control: node.child("RateControl").map(|rc| VideoRateControl {
                frame_rate_limit: xml_u32(rc, "FrameRateLimit").unwrap_or(0),
                encoding_interval: xml_u32(rc, "EncodingInterval").unwrap_or(1),
                bitrate_limit: xml_u32(rc, "BitrateLimit").unwrap_or(0),
            }),
            h264: node.child("H264").map(|n| H264Configuration {
                gov_length: xml_u32(n, "GovLength").unwrap_or(0),
                profile: xml_str(n, "H264Profile").unwrap_or_default(),
            }),
            h265: node.child("H265").map(|n| H265Configuration {
                gov_length: xml_u32(n, "GovLength").unwrap_or(0),
                profile: xml_str(n, "H265Profile").unwrap_or_default(),
            }),
        })
    }

    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("Configurations")
            .map(Self::from_xml)
            .collect()
    }

    /// Serialise to a `<trt:Configuration>` XML fragment for `SetVideoEncoderConfiguration`.
    pub(crate) fn to_xml_body(&self) -> String {
        let res = format!(
            "<tt:Resolution><tt:Width>{}</tt:Width><tt:Height>{}</tt:Height></tt:Resolution>",
            self.resolution.width, self.resolution.height
        );
        let rate = match &self.rate_control {
            Some(rc) => format!(
                "<tt:RateControl>\
                   <tt:FrameRateLimit>{}</tt:FrameRateLimit>\
                   <tt:EncodingInterval>{}</tt:EncodingInterval>\
                   <tt:BitrateLimit>{}</tt:BitrateLimit>\
                 </tt:RateControl>",
                rc.frame_rate_limit, rc.encoding_interval, rc.bitrate_limit
            ),
            None => String::new(),
        };
        let h264 = match &self.h264 {
            Some(h) => format!(
                "<tt:H264>\
                   <tt:GovLength>{}</tt:GovLength>\
                   <tt:H264Profile>{}</tt:H264Profile>\
                 </tt:H264>",
                h.gov_length, h.profile
            ),
            None => String::new(),
        };
        let h265 = match &self.h265 {
            Some(h) => format!(
                "<tt:H265>\
                   <tt:GovLength>{}</tt:GovLength>\
                   <tt:H265Profile>{}</tt:H265Profile>\
                 </tt:H265>",
                h.gov_length, h.profile
            ),
            None => String::new(),
        };
        format!(
            "<trt:Configuration token=\"{token}\">\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:Encoding>{encoding}</tt:Encoding>\
               {res}{rate}{h264}{h265}\
               <tt:Quality>{quality}</tt:Quality>\
             </trt:Configuration>",
            token = self.token,
            name = self.name,
            use_count = self.use_count,
            encoding = self.encoding,
            quality = self.quality,
        )
    }
}

// ── VideoEncoderConfigurationOptions ─────────────────────────────────────────

/// Valid parameter ranges for `SetVideoEncoderConfiguration`.
#[derive(Debug, Clone, Default)]
pub struct VideoEncoderConfigurationOptions {
    pub quality_range: Option<FloatRange>,
    pub jpeg: Option<JpegOptions>,
    pub h264: Option<H264Options>,
    pub h265: Option<H265Options>,
}

/// Valid options for JPEG encoding.
#[derive(Debug, Clone, Default)]
pub struct JpegOptions {
    pub resolutions: Vec<Resolution>,
    pub frame_rate_range: Option<IntRange>,
    pub encoding_interval_range: Option<IntRange>,
}

/// Valid options for H.264 encoding.
#[derive(Debug, Clone, Default)]
pub struct H264Options {
    pub resolutions: Vec<Resolution>,
    pub gov_length_range: Option<IntRange>,
    pub frame_rate_range: Option<IntRange>,
    pub encoding_interval_range: Option<IntRange>,
    pub bitrate_range: Option<IntRange>,
    /// Supported H.264 profiles (e.g. `"Baseline"`, `"Main"`, `"High"`).
    pub profiles: Vec<String>,
}

/// Valid options for H.265 encoding.
#[derive(Debug, Clone, Default)]
pub struct H265Options {
    pub resolutions: Vec<Resolution>,
    pub gov_length_range: Option<IntRange>,
    pub frame_rate_range: Option<IntRange>,
    pub encoding_interval_range: Option<IntRange>,
    pub bitrate_range: Option<IntRange>,
    /// Supported H.265 profiles.
    pub profiles: Vec<String>,
}

impl VideoEncoderConfigurationOptions {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let opts = resp
            .child("Options")
            .ok_or_else(|| SoapError::missing("Options"))?;
        Ok(Self {
            quality_range: opts.child("QualityRange").map(|qr| FloatRange {
                min: qr
                    .child("Min")
                    .and_then(|n| n.text().parse().ok())
                    .unwrap_or(0.0),
                max: qr
                    .child("Max")
                    .and_then(|n| n.text().parse().ok())
                    .unwrap_or(0.0),
            }),
            jpeg: opts.child("JPEG").map(|jpeg| JpegOptions {
                resolutions: jpeg
                    .children_named("ResolutionsAvailable")
                    .filter_map(parse_resolution)
                    .collect(),
                frame_rate_range: jpeg.child("FrameRateRange").map(parse_int_range_node),
                encoding_interval_range: jpeg
                    .child("EncodingIntervalRange")
                    .map(parse_int_range_node),
            }),
            h264: opts.child("H264").map(|h| H264Options {
                resolutions: h
                    .children_named("ResolutionsAvailable")
                    .filter_map(parse_resolution)
                    .collect(),
                gov_length_range: h.child("GovLengthRange").map(parse_int_range_node),
                frame_rate_range: h.child("FrameRateRange").map(parse_int_range_node),
                encoding_interval_range: h.child("EncodingIntervalRange").map(parse_int_range_node),
                bitrate_range: h.child("BitrateRange").map(parse_int_range_node),
                profiles: h
                    .children_named("H264ProfilesSupported")
                    .map(|n| n.text().to_string())
                    .collect(),
            }),
            h265: opts.child("H265").map(|h| H265Options {
                resolutions: h
                    .children_named("ResolutionsAvailable")
                    .filter_map(parse_resolution)
                    .collect(),
                gov_length_range: h.child("GovLengthRange").map(parse_int_range_node),
                frame_rate_range: h.child("FrameRateRange").map(parse_int_range_node),
                encoding_interval_range: h.child("EncodingIntervalRange").map(parse_int_range_node),
                bitrate_range: h.child("BitrateRange").map(parse_int_range_node),
                profiles: h
                    .children_named("H265ProfilesSupported")
                    .map(|n| n.text().to_string())
                    .collect(),
            }),
        })
    }
}

// ── MediaProfile2 ─────────────────────────────────────────────────────────────

/// A Media2 profile returned by `GetProfiles` (Media2).
///
/// Compared with [`MediaProfile`], this carries optional references to the
/// video source and video encoder configurations currently bound to the profile.
#[derive(Debug, Clone)]
pub struct MediaProfile2 {
    pub token: String,
    pub name: String,
    pub fixed: bool,
    /// Token of the bound `VideoSourceConfiguration`, if any.
    pub video_source_token: Option<String>,
    /// Token of the bound `VideoEncoderConfiguration2`, if any.
    pub video_encoder_token: Option<String>,
}

impl MediaProfile2 {
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        Ok(resp
            .children_named("Profiles")
            .map(|p| Self {
                token: p.attr("token").unwrap_or("").to_string(),
                name: xml_str(p, "Name").unwrap_or_default(),
                fixed: p.attr("fixed") == Some("true"),
                video_source_token: p
                    .path(&["Configurations", "VideoSource"])
                    .and_then(|n| n.attr("token"))
                    .map(str::to_string),
                video_encoder_token: p
                    .path(&["Configurations", "VideoEncoder"])
                    .and_then(|n| n.attr("token"))
                    .map(str::to_string),
            })
            .collect())
    }
}

// ── VideoEncoderConfiguration2 ────────────────────────────────────────────────

/// Video encoder configuration for Media2 — flat structure with native H.265.
///
/// Unlike [`VideoEncoderConfiguration`] (Media1), this uses a **flat** layout:
/// `gov_length` and `profile` are top-level fields, not nested under a codec
/// sub-struct. Use with `get_video_encoder_configurations_media2` and
/// `set_video_encoder_configuration_media2`.
#[derive(Debug, Clone)]
pub struct VideoEncoderConfiguration2 {
    pub token: String,
    pub name: String,
    pub use_count: u32,
    pub encoding: VideoEncoding,
    pub resolution: Resolution,
    pub quality: f32,
    /// Codec-specific rate control. `None` if the device omits it.
    pub rate_control: Option<VideoRateControl2>,
    /// Group-of-pictures length (keyframe interval in frames).
    pub gov_length: Option<u32>,
    /// Codec profile (e.g. `"High"` for H.264, `"Main"` for H.265).
    pub profile: Option<String>,
}

/// Simplified rate control for Media2 (no `EncodingInterval`).
#[derive(Debug, Clone)]
pub struct VideoRateControl2 {
    pub frame_rate_limit: u32,
    pub bitrate_limit: u32,
}

impl VideoEncoderConfiguration2 {
    pub(crate) fn from_xml(node: &XmlNode) -> Result<Self, OnvifError> {
        Ok(Self {
            token: node.attr("token").unwrap_or("").to_string(),
            name: xml_str(node, "Name").unwrap_or_default(),
            use_count: xml_u32(node, "UseCount").unwrap_or(0),
            encoding: xml_str(node, "Encoding")
                .map(|s| VideoEncoding::from_str(&s))
                .unwrap_or_default(),
            resolution: node
                .child("Resolution")
                .and_then(parse_resolution)
                .unwrap_or_default(),
            quality: node
                .child("Quality")
                .and_then(|n| n.text().parse().ok())
                .unwrap_or(0.0),
            rate_control: node.child("RateControl").map(|rc| VideoRateControl2 {
                frame_rate_limit: xml_u32(rc, "FrameRateLimit").unwrap_or(0),
                bitrate_limit: xml_u32(rc, "BitrateLimit").unwrap_or(0),
            }),
            gov_length: xml_u32(node, "GovLength"),
            profile: xml_str(node, "Profile"),
        })
    }

    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("Configurations")
            .map(Self::from_xml)
            .collect()
    }

    /// Serialise to a `<tr2:Configuration>` XML fragment for `SetVideoEncoderConfiguration` (Media2).
    pub(crate) fn to_xml_body(&self) -> String {
        let res = format!(
            "<tt:Resolution><tt:Width>{}</tt:Width><tt:Height>{}</tt:Height></tt:Resolution>",
            self.resolution.width, self.resolution.height
        );
        let rate = match &self.rate_control {
            Some(rc) => format!(
                "<tt:RateControl>\
                   <tt:FrameRateLimit>{}</tt:FrameRateLimit>\
                   <tt:BitrateLimit>{}</tt:BitrateLimit>\
                 </tt:RateControl>",
                rc.frame_rate_limit, rc.bitrate_limit
            ),
            None => String::new(),
        };
        let gov = self
            .gov_length
            .map(|g| format!("<tt:GovLength>{g}</tt:GovLength>"))
            .unwrap_or_default();
        let profile = self
            .profile
            .as_deref()
            .map(|p| format!("<tt:Profile>{p}</tt:Profile>"))
            .unwrap_or_default();
        format!(
            "<tr2:Configuration token=\"{token}\">\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:Encoding>{encoding}</tt:Encoding>\
               {res}{rate}{gov}{profile}\
               <tt:Quality>{quality}</tt:Quality>\
             </tr2:Configuration>",
            token = self.token,
            name = self.name,
            use_count = self.use_count,
            encoding = self.encoding,
            quality = self.quality,
        )
    }
}

// ── VideoEncoderConfigurationOptions2 ────────────────────────────────────────

/// Valid parameter ranges for `SetVideoEncoderConfiguration` (Media2).
///
/// Media2 returns one [`VideoEncoderOptions2`] entry per supported encoding.
/// Match on `opts.options[i].encoding` to find the set relevant to you.
#[derive(Debug, Clone, Default)]
pub struct VideoEncoderConfigurationOptions2 {
    /// One entry per encoding type the device supports (H264, H265, JPEG, …).
    pub options: Vec<VideoEncoderOptions2>,
}

/// Per-encoding options entry within [`VideoEncoderConfigurationOptions2`].
#[derive(Debug, Clone, Default)]
pub struct VideoEncoderOptions2 {
    pub encoding: VideoEncoding,
    pub quality_range: Option<FloatRange>,
    pub resolutions: Vec<Resolution>,
    pub bitrate_range: Option<IntRange>,
    /// Discrete supported frame rates (may be empty if range is used instead).
    pub frame_rates: Vec<u32>,
    pub frame_rate_range: Option<IntRange>,
    pub gov_length_range: Option<IntRange>,
    /// Supported codec profiles (e.g. `"Main"`, `"High"`).
    pub profiles: Vec<String>,
}

impl VideoEncoderConfigurationOptions2 {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        Ok(Self {
            options: resp
                .children_named("Options")
                .map(|opt| VideoEncoderOptions2 {
                    encoding: xml_str(opt, "Encoding")
                        .map(|s| VideoEncoding::from_str(&s))
                        .unwrap_or_default(),
                    quality_range: opt.child("QualityRange").map(|qr| FloatRange {
                        min: qr
                            .child("Min")
                            .and_then(|n| n.text().parse().ok())
                            .unwrap_or(0.0),
                        max: qr
                            .child("Max")
                            .and_then(|n| n.text().parse().ok())
                            .unwrap_or(0.0),
                    }),
                    resolutions: opt
                        .children_named("ResolutionsAvailable")
                        .filter_map(parse_resolution)
                        .collect(),
                    bitrate_range: opt.child("BitrateRange").map(parse_int_range_node),
                    frame_rates: opt
                        .children_named("FrameRatesSupported")
                        .filter_map(|n| n.text().parse().ok())
                        .collect(),
                    frame_rate_range: opt.child("FrameRateRange").map(parse_int_range_node),
                    gov_length_range: opt.child("GovLengthRange").map(parse_int_range_node),
                    profiles: opt
                        .children_named("ProfilesSupported")
                        .map(|n| n.text().to_string())
                        .collect(),
                })
                .collect(),
        })
    }
}

// ── VideoEncoderInstances ─────────────────────────────────────────────────────

/// Encoder capacity info returned by `GetVideoEncoderInstances` (Media2).
#[derive(Debug, Clone, Default)]
pub struct VideoEncoderInstances {
    /// Total number of encoder instances available on the source.
    pub total: u32,
    /// Per-encoding breakdown of available instances.
    pub encodings: Vec<EncoderInstanceInfo>,
}

/// Available instance count for one encoding type.
#[derive(Debug, Clone)]
pub struct EncoderInstanceInfo {
    pub encoding: VideoEncoding,
    pub number: u32,
}

impl VideoEncoderInstances {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let info = resp
            .child("Info")
            .ok_or_else(|| SoapError::missing("Info"))?;
        Ok(Self {
            total: xml_u32(info, "Total").unwrap_or(0),
            encodings: info
                .children_named("Encoding")
                .map(|e| EncoderInstanceInfo {
                    encoding: xml_str(e, "Encoding")
                        .map(|s| VideoEncoding::from_str(&s))
                        .unwrap_or_default(),
                    number: xml_u32(e, "Number").unwrap_or(0),
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soap::XmlNode;

    fn parse(xml: &str) -> XmlNode {
        XmlNode::parse(xml).unwrap()
    }

    mod capabilities {
        use super::*;

        /// Full response exercising every field defined in the ONVIF spec.
        const FULL: &str = r#"<GetCapabilitiesResponse>
          <Capabilities>
            <Device>
              <XAddr>http://192.168.1.1/onvif/device_service</XAddr>
              <Network>
                <IPFilter>false</IPFilter>
                <ZeroConfiguration>true</ZeroConfiguration>
                <IPVersion6>false</IPVersion6>
                <DynDNS>false</DynDNS>
              </Network>
              <System>
                <DiscoveryResolve>true</DiscoveryResolve>
                <DiscoveryBye>true</DiscoveryBye>
                <RemoteDiscovery>false</RemoteDiscovery>
                <SystemBackup>true</SystemBackup>
                <SystemLogging>true</SystemLogging>
                <FirmwareUpgrade>true</FirmwareUpgrade>
              </System>
              <IO>
                <InputConnectors>1</InputConnectors>
                <RelayOutputs>2</RelayOutputs>
              </IO>
              <Security>
                <TLS1.2>true</TLS1.2>
                <OnboardKeyGeneration>false</OnboardKeyGeneration>
                <AccessPolicyConfig>false</AccessPolicyConfig>
                <X.509Token>false</X.509Token>
                <UsernameToken>true</UsernameToken>
              </Security>
            </Device>
            <Media>
              <XAddr>http://192.168.1.1/onvif/media_service</XAddr>
              <StreamingCapabilities>
                <RTPMulticast>false</RTPMulticast>
                <RTP_TCP>true</RTP_TCP>
                <RTP_RTSP_TCP>true</RTP_RTSP_TCP>
              </StreamingCapabilities>
              <MaximumNumberOfProfiles>5</MaximumNumberOfProfiles>
            </Media>
            <PTZ>
              <XAddr>http://192.168.1.1/onvif/ptz_service</XAddr>
            </PTZ>
            <Events>
              <XAddr>http://192.168.1.1/onvif/events_service</XAddr>
              <WSSubscriptionPolicySupport>true</WSSubscriptionPolicySupport>
              <WSPullPointSupport>true</WSPullPointSupport>
            </Events>
            <Imaging>
              <XAddr>http://192.168.1.1/onvif/imaging_service</XAddr>
            </Imaging>
            <Analytics>
              <XAddr>http://192.168.1.1/onvif/analytics_service</XAddr>
              <RuleSupport>true</RuleSupport>
              <AnalyticsModuleSupport>true</AnalyticsModuleSupport>
            </Analytics>
            <Extension>
              <DeviceIO>  <XAddr>http://192.168.1.1/onvif/deviceio_service</XAddr>  </DeviceIO>
              <Recording> <XAddr>http://192.168.1.1/onvif/recording_service</XAddr> </Recording>
              <Search>    <XAddr>http://192.168.1.1/onvif/search_service</XAddr>    </Search>
              <Replay>    <XAddr>http://192.168.1.1/onvif/replay_service</XAddr>    </Replay>
            </Extension>
          </Capabilities>
        </GetCapabilitiesResponse>"#;

        // ── Service URLs ──────────────────────────────────────────────────────

        #[test]
        fn test_all_service_urls_parsed() {
            let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
            assert_eq!(
                caps.device.url.as_deref(),
                Some("http://192.168.1.1/onvif/device_service")
            );
            assert_eq!(
                caps.media.url.as_deref(),
                Some("http://192.168.1.1/onvif/media_service")
            );
            assert_eq!(
                caps.ptz_url.as_deref(),
                Some("http://192.168.1.1/onvif/ptz_service")
            );
            assert_eq!(
                caps.events.url.as_deref(),
                Some("http://192.168.1.1/onvif/events_service")
            );
            assert_eq!(
                caps.imaging_url.as_deref(),
                Some("http://192.168.1.1/onvif/imaging_service")
            );
            assert_eq!(
                caps.analytics.url.as_deref(),
                Some("http://192.168.1.1/onvif/analytics_service")
            );
        }

        #[test]
        fn test_extension_service_urls_parsed() {
            let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
            assert_eq!(
                caps.device_io_url.as_deref(),
                Some("http://192.168.1.1/onvif/deviceio_service")
            );
            assert_eq!(
                caps.recording_url.as_deref(),
                Some("http://192.168.1.1/onvif/recording_service")
            );
            assert_eq!(
                caps.search_url.as_deref(),
                Some("http://192.168.1.1/onvif/search_service")
            );
            assert_eq!(
                caps.replay_url.as_deref(),
                Some("http://192.168.1.1/onvif/replay_service")
            );
        }

        // ── Device sub-capabilities ───────────────────────────────────────────

        #[test]
        fn test_device_network_capabilities() {
            let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
            assert!(!caps.device.network.ip_filter);
            assert!(caps.device.network.zero_configuration);
            assert!(!caps.device.network.ip_version6);
            assert!(!caps.device.network.dyn_dns);
        }

        #[test]
        fn test_device_system_capabilities() {
            let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
            assert!(caps.device.system.discovery_resolve);
            assert!(caps.device.system.discovery_bye);
            assert!(!caps.device.system.remote_discovery);
            assert!(caps.device.system.system_backup);
            assert!(caps.device.system.system_logging);
            assert!(caps.device.system.firmware_upgrade);
        }

        #[test]
        fn test_device_io_capabilities() {
            let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
            assert_eq!(caps.device.io.input_connectors, Some(1));
            assert_eq!(caps.device.io.relay_outputs, Some(2));
        }

        #[test]
        fn test_device_security_capabilities() {
            let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
            assert!(caps.device.security.tls_1_2);
            assert!(!caps.device.security.onboard_key_generation);
            assert!(!caps.device.security.x509_token);
            assert!(caps.device.security.username_token);
        }

        // ── Media sub-capabilities ────────────────────────────────────────────

        #[test]
        fn test_media_streaming_capabilities() {
            let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
            assert!(!caps.media.streaming.rtp_multicast);
            assert!(caps.media.streaming.rtp_tcp);
            assert!(caps.media.streaming.rtp_rtsp_tcp);
        }

        #[test]
        fn test_media_max_profiles() {
            let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
            assert_eq!(caps.media.max_profiles, Some(5));
        }

        // ── Events sub-capabilities ───────────────────────────────────────────

        #[test]
        fn test_events_capabilities() {
            let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
            assert!(caps.events.ws_subscription_policy);
            assert!(caps.events.ws_pull_point);
        }

        // ── Analytics sub-capabilities ────────────────────────────────────────

        #[test]
        fn test_analytics_capabilities() {
            let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
            assert!(caps.analytics.rule_support);
            assert!(caps.analytics.analytics_module_support);
        }

        // ── Absence / error cases ─────────────────────────────────────────────

        #[test]
        fn test_optional_services_absent_are_none() {
            let xml = r#"<GetCapabilitiesResponse>
              <Capabilities>
                <Device><XAddr>http://192.168.1.1/onvif/device_service</XAddr></Device>
                <Media> <XAddr>http://192.168.1.1/onvif/media_service</XAddr> </Media>
              </Capabilities>
            </GetCapabilitiesResponse>"#;
            let caps = Capabilities::from_xml(&parse(xml)).unwrap();
            assert!(caps.ptz_url.is_none());
            assert!(caps.events.url.is_none());
            assert!(caps.imaging_url.is_none());
            assert!(caps.analytics.url.is_none());
            assert!(caps.recording_url.is_none());
        }

        #[test]
        fn test_absent_boolean_fields_default_to_false() {
            let xml = r#"<GetCapabilitiesResponse>
              <Capabilities>
                <Device><XAddr>http://192.168.1.1/onvif/device_service</XAddr></Device>
              </Capabilities>
            </GetCapabilitiesResponse>"#;
            let caps = Capabilities::from_xml(&parse(xml)).unwrap();
            assert!(!caps.device.network.ip_filter);
            assert!(!caps.device.system.firmware_upgrade);
            assert!(!caps.device.security.username_token);
            assert!(!caps.media.streaming.rtp_tcp);
            assert!(!caps.events.ws_pull_point);
        }

        #[test]
        fn test_missing_capabilities_node_is_error() {
            let err = Capabilities::from_xml(&parse("<GetCapabilitiesResponse/>")).unwrap_err();
            assert!(matches!(
                err,
                OnvifError::Soap(SoapError::MissingField("Capabilities"))
            ));
        }
    }

    mod device_info {
        use super::*;

        const FULL: &str = r#"<GetDeviceInformationResponse>
          <Manufacturer>Hikvision</Manufacturer>
          <Model>DS-2CD2085G1-I</Model>
          <FirmwareVersion>V5.6.1 build 190813</FirmwareVersion>
          <SerialNumber>DS-2CD2085G1-I20190619AACH123456789</SerialNumber>
          <HardwareId>0x00</HardwareId>
        </GetDeviceInformationResponse>"#;

        #[test]
        fn test_all_fields_parsed() {
            let info = DeviceInfo::from_xml(&parse(FULL)).unwrap();
            assert_eq!(info.manufacturer, "Hikvision");
            assert_eq!(info.model, "DS-2CD2085G1-I");
            assert_eq!(info.firmware_version, "V5.6.1 build 190813");
            assert_eq!(info.serial_number, "DS-2CD2085G1-I20190619AACH123456789");
            assert_eq!(info.hardware_id, "0x00");
        }

        #[test]
        fn test_absent_fields_default_to_empty_string() {
            let info = DeviceInfo::from_xml(&parse("<GetDeviceInformationResponse/>")).unwrap();
            assert_eq!(info.manufacturer, "");
            assert_eq!(info.model, "");
            assert_eq!(info.firmware_version, "");
        }

        #[test]
        fn test_partial_response_fills_present_fields() {
            let xml = r#"<GetDeviceInformationResponse>
              <Manufacturer>Axis</Manufacturer>
              <Model>P3245-V</Model>
            </GetDeviceInformationResponse>"#;
            let info = DeviceInfo::from_xml(&parse(xml)).unwrap();
            assert_eq!(info.manufacturer, "Axis");
            assert_eq!(info.model, "P3245-V");
            assert_eq!(info.firmware_version, "");
        }
    }

    mod media_profile {
        use super::*;

        const TWO_PROFILES: &str = r#"<GetProfilesResponse>
          <Profiles token="Profile_1" fixed="true">
            <Name>mainStream</Name>
          </Profiles>
          <Profiles token="Profile_2" fixed="false">
            <Name>subStream</Name>
          </Profiles>
        </GetProfilesResponse>"#;

        #[test]
        fn test_two_profiles_returned() {
            let profiles = MediaProfile::vec_from_xml(&parse(TWO_PROFILES)).unwrap();
            assert_eq!(profiles.len(), 2);
        }

        #[test]
        fn test_profile_fields() {
            let profiles = MediaProfile::vec_from_xml(&parse(TWO_PROFILES)).unwrap();
            assert_eq!(profiles[0].token, "Profile_1");
            assert_eq!(profiles[0].name, "mainStream");
            assert!(profiles[0].fixed);
            assert_eq!(profiles[1].token, "Profile_2");
            assert_eq!(profiles[1].name, "subStream");
            assert!(!profiles[1].fixed);
        }

        #[test]
        fn test_empty_response_returns_empty_vec() {
            let profiles = MediaProfile::vec_from_xml(&parse("<GetProfilesResponse/>")).unwrap();
            assert!(profiles.is_empty());
        }

        #[test]
        fn test_fixed_absent_defaults_to_false() {
            let xml = r#"<GetProfilesResponse>
              <Profiles token="tok"><Name>noFixed</Name></Profiles>
            </GetProfilesResponse>"#;
            let profiles = MediaProfile::vec_from_xml(&parse(xml)).unwrap();
            assert!(!profiles[0].fixed);
        }
    }

    mod stream_uri {
        use super::*;

        const FULL: &str = r#"<GetStreamUriResponse>
          <MediaUri>
            <Uri>rtsp://192.168.1.1:554/Streaming/Channels/101</Uri>
            <InvalidAfterConnect>false</InvalidAfterConnect>
            <InvalidAfterReboot>false</InvalidAfterReboot>
            <Timeout>PT0S</Timeout>
          </MediaUri>
        </GetStreamUriResponse>"#;

        #[test]
        fn test_all_fields_parsed() {
            let uri = StreamUri::from_xml(&parse(FULL)).unwrap();
            assert_eq!(uri.uri, "rtsp://192.168.1.1:554/Streaming/Channels/101");
            assert!(!uri.invalid_after_connect);
            assert!(!uri.invalid_after_reboot);
            assert_eq!(uri.timeout, "PT0S");
        }

        #[test]
        fn test_invalid_after_connect_and_reboot_true() {
            let xml = r#"<GetStreamUriResponse>
              <MediaUri>
                <Uri>rtsp://192.168.1.1/stream</Uri>
                <InvalidAfterConnect>true</InvalidAfterConnect>
                <InvalidAfterReboot>true</InvalidAfterReboot>
              </MediaUri>
            </GetStreamUriResponse>"#;
            let uri = StreamUri::from_xml(&parse(xml)).unwrap();
            assert!(uri.invalid_after_connect);
            assert!(uri.invalid_after_reboot);
        }

        #[test]
        fn test_missing_media_uri_is_error() {
            let err = StreamUri::from_xml(&parse("<GetStreamUriResponse/>")).unwrap_err();
            assert!(matches!(
                err,
                OnvifError::Soap(SoapError::MissingField("MediaUri"))
            ));
        }

        #[test]
        fn test_missing_uri_element_is_error() {
            let xml = "<GetStreamUriResponse><MediaUri/></GetStreamUriResponse>";
            let err = StreamUri::from_xml(&parse(xml)).unwrap_err();
            assert!(matches!(
                err,
                OnvifError::Soap(SoapError::MissingField("Uri"))
            ));
        }

        #[test]
        fn test_empty_uri_text_is_error() {
            let xml =
                "<GetStreamUriResponse><MediaUri><Uri></Uri></MediaUri></GetStreamUriResponse>";
            let err = StreamUri::from_xml(&parse(xml)).unwrap_err();
            assert!(matches!(
                err,
                OnvifError::Soap(SoapError::MissingField("Uri"))
            ));
        }

        #[test]
        fn test_timeout_absent_defaults_to_empty() {
            let xml = r#"<GetStreamUriResponse>
              <MediaUri><Uri>rtsp://x/s</Uri></MediaUri>
            </GetStreamUriResponse>"#;
            let uri = StreamUri::from_xml(&parse(xml)).unwrap();
            assert_eq!(uri.timeout, "");
        }
    }

    mod system_date_time {
        use super::*;

        const FULL: &str = r#"<GetSystemDateAndTimeResponse>
          <SystemDateAndTime>
            <DateTimeType>NTP</DateTimeType>
            <DaylightSavings>true</DaylightSavings>
            <TimeZone><TZ>CST-8</TZ></TimeZone>
            <UTCDateTime>
              <Time><Hour>10</Hour><Minute>30</Minute><Second>45</Second></Time>
              <Date><Year>2024</Year><Month>6</Month><Day>15</Day></Date>
            </UTCDateTime>
          </SystemDateAndTime>
        </GetSystemDateAndTimeResponse>"#;

        #[test]
        fn test_utc_unix_correct() {
            let dt = SystemDateTime::from_xml(&parse(FULL)).unwrap();
            // 2024-06-15T10:30:45Z = 1_718_447_445
            assert_eq!(dt.utc_unix, Some(1_718_447_445));
        }

        #[test]
        fn test_daylight_savings_parsed() {
            let dt = SystemDateTime::from_xml(&parse(FULL)).unwrap();
            assert!(dt.daylight_savings);
        }

        #[test]
        fn test_timezone_parsed() {
            let dt = SystemDateTime::from_xml(&parse(FULL)).unwrap();
            assert_eq!(dt.timezone, "CST-8");
        }

        #[test]
        fn test_missing_utc_datetime_gives_none() {
            let xml = r#"<GetSystemDateAndTimeResponse>
              <SystemDateAndTime>
                <DaylightSavings>false</DaylightSavings>
              </SystemDateAndTime>
            </GetSystemDateAndTimeResponse>"#;
            let dt = SystemDateTime::from_xml(&parse(xml)).unwrap();
            assert!(dt.utc_unix.is_none());
        }

        #[test]
        fn test_missing_system_date_and_time_is_error() {
            let err =
                SystemDateTime::from_xml(&parse("<GetSystemDateAndTimeResponse/>")).unwrap_err();
            assert!(matches!(
                err,
                OnvifError::Soap(SoapError::MissingField("SystemDateAndTime"))
            ));
        }

        #[test]
        fn test_civil_to_unix_epoch() {
            assert_eq!(civil_to_unix(1970, 1, 1, 0, 0, 0), 0);
        }

        #[test]
        fn test_civil_to_unix_known_date() {
            // 2024-01-01T00:00:00Z = 1_704_067_200
            assert_eq!(civil_to_unix(2024, 1, 1, 0, 0, 0), 1_704_067_200);
        }
    }

    mod snapshot_uri {
        use super::*;

        const FULL: &str = r#"<GetSnapshotUriResponse>
          <MediaUri>
            <Uri>http://192.168.1.1/onvif/snapshot?channel=1</Uri>
            <InvalidAfterConnect>false</InvalidAfterConnect>
            <InvalidAfterReboot>true</InvalidAfterReboot>
            <Timeout>PT60S</Timeout>
          </MediaUri>
        </GetSnapshotUriResponse>"#;

        #[test]
        fn test_all_fields_parsed() {
            let uri = SnapshotUri::from_xml(&parse(FULL)).unwrap();
            assert_eq!(uri.uri, "http://192.168.1.1/onvif/snapshot?channel=1");
            assert!(!uri.invalid_after_connect);
            assert!(uri.invalid_after_reboot);
            assert_eq!(uri.timeout, "PT60S");
        }

        #[test]
        fn test_missing_media_uri_is_error() {
            let err = SnapshotUri::from_xml(&parse("<GetSnapshotUriResponse/>")).unwrap_err();
            assert!(matches!(
                err,
                OnvifError::Soap(SoapError::MissingField("MediaUri"))
            ));
        }

        #[test]
        fn test_empty_uri_is_error() {
            let xml =
                "<GetSnapshotUriResponse><MediaUri><Uri/></MediaUri></GetSnapshotUriResponse>";
            let err = SnapshotUri::from_xml(&parse(xml)).unwrap_err();
            assert!(matches!(
                err,
                OnvifError::Soap(SoapError::MissingField("Uri"))
            ));
        }
    }

    mod ptz_preset {
        use super::*;

        const TWO_PRESETS: &str = r#"<GetPresetsResponse>
          <Preset token="1">
            <Name>Front Gate</Name>
            <PTZPosition>
              <PanTilt x="0.1" y="-0.2"/>
              <Zoom x="0.5"/>
            </PTZPosition>
          </Preset>
          <Preset token="2">
            <Name>Parking Lot</Name>
          </Preset>
        </GetPresetsResponse>"#;

        #[test]
        fn test_two_presets_returned() {
            let presets = PtzPreset::vec_from_xml(&parse(TWO_PRESETS)).unwrap();
            assert_eq!(presets.len(), 2);
        }

        #[test]
        fn test_preset_fields() {
            let presets = PtzPreset::vec_from_xml(&parse(TWO_PRESETS)).unwrap();
            assert_eq!(presets[0].token, "1");
            assert_eq!(presets[0].name, "Front Gate");
        }

        #[test]
        fn test_preset_position_parsed() {
            let presets = PtzPreset::vec_from_xml(&parse(TWO_PRESETS)).unwrap();
            let (pan, tilt) = presets[0].pan_tilt.unwrap();
            assert!((pan - 0.1).abs() < 1e-5);
            assert!((tilt - (-0.2)).abs() < 1e-5);
            assert!((presets[0].zoom.unwrap() - 0.5).abs() < 1e-5);
        }

        #[test]
        fn test_preset_without_position_is_none() {
            let presets = PtzPreset::vec_from_xml(&parse(TWO_PRESETS)).unwrap();
            assert!(presets[1].pan_tilt.is_none());
            assert!(presets[1].zoom.is_none());
        }

        #[test]
        fn test_empty_response_returns_empty_vec() {
            let presets = PtzPreset::vec_from_xml(&parse("<GetPresetsResponse/>")).unwrap();
            assert!(presets.is_empty());
        }
    }

    mod video {
        use super::*;

        // ── VideoSource ───────────────────────────────────────────────────────

        const TWO_SOURCES: &str = r#"<GetVideoSourcesResponse>
          <VideoSources token="VideoSource_1">
            <Framerate>25</Framerate>
            <Resolution><Width>1920</Width><Height>1080</Height></Resolution>
          </VideoSources>
          <VideoSources token="VideoSource_2">
            <Framerate>15</Framerate>
            <Resolution><Width>1280</Width><Height>720</Height></Resolution>
          </VideoSources>
        </GetVideoSourcesResponse>"#;

        #[test]
        fn test_video_sources_count() {
            let sources = VideoSource::vec_from_xml(&parse(TWO_SOURCES)).unwrap();
            assert_eq!(sources.len(), 2);
        }

        #[test]
        fn test_video_sources_fields() {
            let sources = VideoSource::vec_from_xml(&parse(TWO_SOURCES)).unwrap();
            assert_eq!(sources[0].token, "VideoSource_1");
            assert!((sources[0].framerate - 25.0).abs() < 1e-5);
            assert_eq!(
                sources[0].resolution,
                Resolution {
                    width: 1920,
                    height: 1080
                }
            );
            assert_eq!(sources[1].token, "VideoSource_2");
            assert_eq!(
                sources[1].resolution,
                Resolution {
                    width: 1280,
                    height: 720
                }
            );
        }

        // ── VideoSourceConfiguration ──────────────────────────────────────────

        const VSC_XML: &str = r#"<Configuration token="VSC_1">
          <Name>VideoSourceConfig</Name>
          <UseCount>2</UseCount>
          <SourceToken>VideoSource_1</SourceToken>
          <Bounds x="0" y="0" width="1920" height="1080"/>
        </Configuration>"#;

        #[test]
        fn test_video_source_configuration_from_xml() {
            let cfg = VideoSourceConfiguration::from_xml(&parse(VSC_XML)).unwrap();
            assert_eq!(cfg.token, "VSC_1");
            assert_eq!(cfg.name, "VideoSourceConfig");
            assert_eq!(cfg.use_count, 2);
            assert_eq!(cfg.source_token, "VideoSource_1");
            assert_eq!(cfg.bounds.x, 0);
            assert_eq!(cfg.bounds.y, 0);
            assert_eq!(cfg.bounds.width, 1920);
            assert_eq!(cfg.bounds.height, 1080);
        }

        #[test]
        fn test_video_source_configuration_to_xml_body_round_trip() {
            let cfg = VideoSourceConfiguration {
                token: "tok1".into(),
                name: "MyCfg".into(),
                use_count: 1,
                source_token: "src1".into(),
                bounds: SourceBounds {
                    x: 10,
                    y: 20,
                    width: 640,
                    height: 480,
                },
            };
            let xml = cfg.to_xml_body();
            assert!(xml.contains("token=\"tok1\""));
            assert!(xml.contains("<tt:Name>MyCfg</tt:Name>"));
            assert!(xml.contains("<tt:SourceToken>src1</tt:SourceToken>"));
            assert!(xml.contains("x=\"10\""));
            assert!(xml.contains("y=\"20\""));
            assert!(xml.contains("width=\"640\""));
            assert!(xml.contains("height=\"480\""));
        }

        // ── VideoSourceConfigurationOptions ───────────────────────────────────

        const VSCO_XML: &str = r#"<GetVideoSourceConfigurationOptionsResponse>
          <Options>
            <MaximumNumberOfProfiles>5</MaximumNumberOfProfiles>
            <BoundsRange>
              <XRange><Min>0</Min><Max>0</Max></XRange>
              <YRange><Min>0</Min><Max>0</Max></YRange>
              <WidthRange><Min>320</Min><Max>1920</Max></WidthRange>
              <HeightRange><Min>240</Min><Max>1080</Max></HeightRange>
            </BoundsRange>
            <VideoSourceTokensAvailable>VideoSource_1</VideoSourceTokensAvailable>
            <VideoSourceTokensAvailable>VideoSource_2</VideoSourceTokensAvailable>
          </Options>
        </GetVideoSourceConfigurationOptionsResponse>"#;

        #[test]
        fn test_video_source_configuration_options_from_xml() {
            let opts = VideoSourceConfigurationOptions::from_xml(&parse(VSCO_XML)).unwrap();
            assert_eq!(opts.max_limit, Some(5));
            assert_eq!(opts.source_tokens.len(), 2);
            assert_eq!(opts.source_tokens[0], "VideoSource_1");
            let br = opts.bounds_range.unwrap();
            assert_eq!(br.width_range.min, 320);
            assert_eq!(br.width_range.max, 1920);
            assert_eq!(br.height_range.min, 240);
            assert_eq!(br.height_range.max, 1080);
        }

        // ── VideoEncoding ─────────────────────────────────────────────────────

        #[test]
        fn test_video_encoding_from_str() {
            assert_eq!(VideoEncoding::from_str("JPEG"), VideoEncoding::Jpeg);
            assert_eq!(VideoEncoding::from_str("H264"), VideoEncoding::H264);
            assert_eq!(VideoEncoding::from_str("H265"), VideoEncoding::H265);
            assert_eq!(VideoEncoding::from_str("H.265"), VideoEncoding::H265);
            assert_eq!(
                VideoEncoding::from_str("MPEG4"),
                VideoEncoding::Other("MPEG4".into())
            );
        }

        // ── VideoEncoderConfiguration ─────────────────────────────────────────

        const VEC_XML: &str = r#"<Configuration token="VideoEncoder_1">
          <Name>MainStream</Name>
          <UseCount>1</UseCount>
          <Encoding>H264</Encoding>
          <Resolution><Width>1920</Width><Height>1080</Height></Resolution>
          <Quality>5</Quality>
          <RateControl>
            <FrameRateLimit>25</FrameRateLimit>
            <EncodingInterval>1</EncodingInterval>
            <BitrateLimit>4096</BitrateLimit>
          </RateControl>
          <H264>
            <GovLength>30</GovLength>
            <H264Profile>Main</H264Profile>
          </H264>
        </Configuration>"#;

        #[test]
        fn test_video_encoder_configuration_from_xml() {
            let cfg = VideoEncoderConfiguration::from_xml(&parse(VEC_XML)).unwrap();
            assert_eq!(cfg.token, "VideoEncoder_1");
            assert_eq!(cfg.name, "MainStream");
            assert_eq!(cfg.use_count, 1);
            assert_eq!(cfg.encoding, VideoEncoding::H264);
            assert_eq!(
                cfg.resolution,
                Resolution {
                    width: 1920,
                    height: 1080
                }
            );
            assert!((cfg.quality - 5.0).abs() < 1e-5);
            let rc = cfg.rate_control.unwrap();
            assert_eq!(rc.frame_rate_limit, 25);
            assert_eq!(rc.encoding_interval, 1);
            assert_eq!(rc.bitrate_limit, 4096);
            let h264 = cfg.h264.unwrap();
            assert_eq!(h264.gov_length, 30);
            assert_eq!(h264.profile, "Main");
            assert!(cfg.h265.is_none());
        }

        const TWO_VEC_XML: &str = r#"<GetVideoEncoderConfigurationsResponse>
          <Configurations token="VideoEncoder_1">
            <Name>MainStream</Name>
            <UseCount>1</UseCount>
            <Encoding>H264</Encoding>
            <Resolution><Width>1920</Width><Height>1080</Height></Resolution>
            <Quality>5</Quality>
          </Configurations>
          <Configurations token="VideoEncoder_2">
            <Name>SubStream</Name>
            <UseCount>1</UseCount>
            <Encoding>JPEG</Encoding>
            <Resolution><Width>640</Width><Height>480</Height></Resolution>
            <Quality>3</Quality>
          </Configurations>
        </GetVideoEncoderConfigurationsResponse>"#;

        #[test]
        fn test_video_encoder_configuration_vec_from_xml() {
            let cfgs = VideoEncoderConfiguration::vec_from_xml(&parse(TWO_VEC_XML)).unwrap();
            assert_eq!(cfgs.len(), 2);
            assert_eq!(cfgs[0].token, "VideoEncoder_1");
            assert_eq!(cfgs[1].encoding, VideoEncoding::Jpeg);
        }

        #[test]
        fn test_video_encoder_configuration_to_xml_body_round_trip() {
            let cfg = VideoEncoderConfiguration {
                token: "enc1".into(),
                name: "Main".into(),
                use_count: 1,
                encoding: VideoEncoding::H264,
                resolution: Resolution {
                    width: 1280,
                    height: 720,
                },
                quality: 4.0,
                rate_control: Some(VideoRateControl {
                    frame_rate_limit: 30,
                    encoding_interval: 1,
                    bitrate_limit: 2048,
                }),
                h264: Some(H264Configuration {
                    gov_length: 25,
                    profile: "Baseline".into(),
                }),
                h265: None,
            };
            let xml = cfg.to_xml_body();
            assert!(xml.contains("token=\"enc1\""));
            assert!(xml.contains("<tt:Encoding>H264</tt:Encoding>"));
            assert!(xml.contains("<tt:Width>1280</tt:Width>"));
            assert!(xml.contains("<tt:FrameRateLimit>30</tt:FrameRateLimit>"));
            assert!(xml.contains("<tt:GovLength>25</tt:GovLength>"));
            assert!(xml.contains("<tt:H264Profile>Baseline</tt:H264Profile>"));
        }

        // ── VideoEncoderConfigurationOptions ──────────────────────────────────

        const VECO_XML: &str = r#"<GetVideoEncoderConfigurationOptionsResponse>
          <Options>
            <QualityRange><Min>1</Min><Max>10</Max></QualityRange>
            <JPEG>
              <ResolutionsAvailable><Width>1920</Width><Height>1080</Height></ResolutionsAvailable>
              <ResolutionsAvailable><Width>1280</Width><Height>720</Height></ResolutionsAvailable>
              <FrameRateRange><Min>1</Min><Max>30</Max></FrameRateRange>
              <EncodingIntervalRange><Min>1</Min><Max>1</Max></EncodingIntervalRange>
            </JPEG>
            <H264>
              <ResolutionsAvailable><Width>1920</Width><Height>1080</Height></ResolutionsAvailable>
              <GovLengthRange><Min>1</Min><Max>150</Max></GovLengthRange>
              <FrameRateRange><Min>1</Min><Max>30</Max></FrameRateRange>
              <EncodingIntervalRange><Min>1</Min><Max>1</Max></EncodingIntervalRange>
              <BitrateRange><Min>32</Min><Max>16384</Max></BitrateRange>
              <H264ProfilesSupported>Baseline</H264ProfilesSupported>
              <H264ProfilesSupported>Main</H264ProfilesSupported>
              <H264ProfilesSupported>High</H264ProfilesSupported>
            </H264>
          </Options>
        </GetVideoEncoderConfigurationOptionsResponse>"#;

        #[test]
        fn test_video_encoder_configuration_options_from_xml() {
            let opts = VideoEncoderConfigurationOptions::from_xml(&parse(VECO_XML)).unwrap();
            let qr = opts.quality_range.unwrap();
            assert!((qr.min - 1.0).abs() < 1e-5);
            assert!((qr.max - 10.0).abs() < 1e-5);
            let jpeg = opts.jpeg.unwrap();
            assert_eq!(jpeg.resolutions.len(), 2);
            assert_eq!(
                jpeg.resolutions[0],
                Resolution {
                    width: 1920,
                    height: 1080
                }
            );
            let fr = jpeg.frame_rate_range.unwrap();
            assert_eq!(fr.min, 1);
            assert_eq!(fr.max, 30);
            let h264 = opts.h264.unwrap();
            assert_eq!(h264.profiles.len(), 3);
            assert_eq!(h264.profiles[0], "Baseline");
            let br = h264.bitrate_range.unwrap();
            assert_eq!(br.min, 32);
            assert_eq!(br.max, 16384);
            let glr = h264.gov_length_range.unwrap();
            assert_eq!(glr.max, 150);
            assert!(opts.h265.is_none());
        }
    }

    mod media2 {
        use super::*;

        // ── MediaProfile2 ─────────────────────────────────────────────────────

        const TWO_PROFILES2: &str = r#"<GetProfilesResponse>
          <Profiles token="Profile_A" fixed="true">
            <Name>mainStream</Name>
            <Configurations>
              <VideoSource token="VSC_1"/>
              <VideoEncoder token="VEC_1"/>
            </Configurations>
          </Profiles>
          <Profiles token="Profile_B" fixed="false">
            <Name>subStream</Name>
            <Configurations>
              <VideoSource token="VSC_1"/>
            </Configurations>
          </Profiles>
        </GetProfilesResponse>"#;

        #[test]
        fn test_media_profile2_vec_from_xml() {
            let profiles = MediaProfile2::vec_from_xml(&parse(TWO_PROFILES2)).unwrap();
            assert_eq!(profiles.len(), 2);
            assert_eq!(profiles[0].token, "Profile_A");
            assert_eq!(profiles[0].name, "mainStream");
            assert!(profiles[0].fixed);
            assert_eq!(profiles[0].video_source_token.as_deref(), Some("VSC_1"));
            assert_eq!(profiles[0].video_encoder_token.as_deref(), Some("VEC_1"));
            assert_eq!(profiles[1].token, "Profile_B");
            assert_eq!(profiles[1].name, "subStream");
            assert!(!profiles[1].fixed);
            assert_eq!(profiles[1].video_source_token.as_deref(), Some("VSC_1"));
            assert!(profiles[1].video_encoder_token.is_none());
        }

        // ── VideoEncoderConfiguration2 ────────────────────────────────────────

        const H265_CONFIG: &str = r#"<Configurations token="VEC_H265">
          <Name>H265Stream</Name>
          <UseCount>1</UseCount>
          <Encoding>H265</Encoding>
          <Resolution><Width>3840</Width><Height>2160</Height></Resolution>
          <Quality>7</Quality>
          <RateControl>
            <FrameRateLimit>30</FrameRateLimit>
            <BitrateLimit>8192</BitrateLimit>
          </RateControl>
          <GovLength>60</GovLength>
          <Profile>Main</Profile>
        </Configurations>"#;

        #[test]
        fn test_video_encoder_configuration2_from_xml_h265() {
            let cfg = VideoEncoderConfiguration2::from_xml(&parse(H265_CONFIG)).unwrap();
            assert_eq!(cfg.token, "VEC_H265");
            assert_eq!(cfg.name, "H265Stream");
            assert_eq!(cfg.encoding, VideoEncoding::H265);
            assert_eq!(
                cfg.resolution,
                Resolution {
                    width: 3840,
                    height: 2160
                }
            );
            assert!((cfg.quality - 7.0).abs() < 1e-5);
            let rc = cfg.rate_control.unwrap();
            assert_eq!(rc.frame_rate_limit, 30);
            assert_eq!(rc.bitrate_limit, 8192);
            assert_eq!(cfg.gov_length, Some(60));
            assert_eq!(cfg.profile.as_deref(), Some("Main"));
        }

        #[test]
        fn test_video_encoder_configuration2_to_xml_body() {
            let cfg = VideoEncoderConfiguration2 {
                token: "enc2".into(),
                name: "H265Main".into(),
                use_count: 1,
                encoding: VideoEncoding::H265,
                resolution: Resolution {
                    width: 1920,
                    height: 1080,
                },
                quality: 6.0,
                rate_control: Some(VideoRateControl2 {
                    frame_rate_limit: 25,
                    bitrate_limit: 4096,
                }),
                gov_length: Some(50),
                profile: Some("Main".into()),
            };
            let xml = cfg.to_xml_body();
            assert!(xml.contains("token=\"enc2\""));
            assert!(xml.contains("<tt:Encoding>H265</tt:Encoding>"));
            assert!(xml.contains("<tt:Width>1920</tt:Width>"));
            assert!(xml.contains("<tt:FrameRateLimit>25</tt:FrameRateLimit>"));
            assert!(xml.contains("<tt:BitrateLimit>4096</tt:BitrateLimit>"));
            assert!(xml.contains("<tt:GovLength>50</tt:GovLength>"));
            assert!(xml.contains("<tt:Profile>Main</tt:Profile>"));
            // No EncodingInterval (Media2 only has FrameRateLimit + BitrateLimit)
            assert!(!xml.contains("EncodingInterval"));
        }

        // ── VideoEncoderConfigurationOptions2 ────────────────────────────────

        const OPTIONS2_XML: &str = r#"<GetVideoEncoderConfigurationOptionsResponse>
          <Options>
            <Encoding>H264</Encoding>
            <QualityRange><Min>1</Min><Max>10</Max></QualityRange>
            <ResolutionsAvailable><Width>1920</Width><Height>1080</Height></ResolutionsAvailable>
            <ResolutionsAvailable><Width>1280</Width><Height>720</Height></ResolutionsAvailable>
            <BitrateRange><Min>32</Min><Max>16384</Max></BitrateRange>
            <GovLengthRange><Min>1</Min><Max>150</Max></GovLengthRange>
            <ProfilesSupported>Baseline</ProfilesSupported>
            <ProfilesSupported>Main</ProfilesSupported>
            <ProfilesSupported>High</ProfilesSupported>
          </Options>
          <Options>
            <Encoding>H265</Encoding>
            <QualityRange><Min>1</Min><Max>10</Max></QualityRange>
            <ResolutionsAvailable><Width>3840</Width><Height>2160</Height></ResolutionsAvailable>
            <BitrateRange><Min>64</Min><Max>32768</Max></BitrateRange>
            <GovLengthRange><Min>1</Min><Max>200</Max></GovLengthRange>
            <ProfilesSupported>Main</ProfilesSupported>
            <ProfilesSupported>Main10</ProfilesSupported>
          </Options>
        </GetVideoEncoderConfigurationOptionsResponse>"#;

        #[test]
        fn test_video_encoder_configuration_options2_from_xml() {
            let opts = VideoEncoderConfigurationOptions2::from_xml(&parse(OPTIONS2_XML)).unwrap();
            assert_eq!(opts.options.len(), 2);

            let h264 = &opts.options[0];
            assert_eq!(h264.encoding, VideoEncoding::H264);
            let qr = h264.quality_range.unwrap();
            assert!((qr.min - 1.0).abs() < 1e-5);
            assert!((qr.max - 10.0).abs() < 1e-5);
            assert_eq!(h264.resolutions.len(), 2);
            assert_eq!(h264.profiles.len(), 3);
            assert_eq!(h264.profiles[1], "Main");
            let br = h264.bitrate_range.unwrap();
            assert_eq!(br.max, 16384);

            let h265 = &opts.options[1];
            assert_eq!(h265.encoding, VideoEncoding::H265);
            assert_eq!(h265.resolutions.len(), 1);
            assert_eq!(
                h265.resolutions[0],
                Resolution {
                    width: 3840,
                    height: 2160
                }
            );
            assert_eq!(h265.profiles.len(), 2);
            assert_eq!(h265.profiles[0], "Main");
            let glr = h265.gov_length_range.unwrap();
            assert_eq!(glr.max, 200);
        }

        // ── VideoEncoderInstances ─────────────────────────────────────────────

        const INSTANCES_XML: &str = r#"<GetVideoEncoderInstancesResponse>
          <Info>
            <Total>4</Total>
            <Encoding>
              <Encoding>H264</Encoding>
              <Number>2</Number>
            </Encoding>
            <Encoding>
              <Encoding>H265</Encoding>
              <Number>2</Number>
            </Encoding>
          </Info>
        </GetVideoEncoderInstancesResponse>"#;

        #[test]
        fn test_video_encoder_instances_from_xml() {
            let inst = VideoEncoderInstances::from_xml(&parse(INSTANCES_XML)).unwrap();
            assert_eq!(inst.total, 4);
            assert_eq!(inst.encodings.len(), 2);
            assert_eq!(inst.encodings[0].encoding, VideoEncoding::H264);
            assert_eq!(inst.encodings[0].number, 2);
            assert_eq!(inst.encodings[1].encoding, VideoEncoding::H265);
            assert_eq!(inst.encodings[1].number, 2);
        }
    }
}
