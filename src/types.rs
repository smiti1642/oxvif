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
#[path = "tests/types_tests.rs"]
mod tests;
