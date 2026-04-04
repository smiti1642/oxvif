use super::{xml_bool, xml_str, xml_u32};
use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

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

/// PTZ service capabilities.
#[derive(Debug, Clone, Default)]
pub struct PtzCapabilities {
    /// PTZ service endpoint URL (`None` if not supported).
    pub url: Option<String>,
}

/// Imaging service capabilities.
#[derive(Debug, Clone, Default)]
pub struct ImagingCapabilities {
    /// Imaging service endpoint URL (`None` if not supported).
    pub url: Option<String>,
}

/// Recording service capabilities.
#[derive(Debug, Clone, Default)]
pub struct RecordingCapabilities {
    /// Recording service endpoint URL (`None` if not supported).
    pub url: Option<String>,
}

/// Search service capabilities.
#[derive(Debug, Clone, Default)]
pub struct SearchCapabilities {
    /// Search service endpoint URL (`None` if not supported).
    pub url: Option<String>,
}

/// Replay service capabilities.
#[derive(Debug, Clone, Default)]
pub struct ReplayCapabilities {
    /// Replay service endpoint URL (`None` if not supported).
    pub url: Option<String>,
}

/// Media2 service capabilities.
#[derive(Debug, Clone, Default)]
pub struct Media2Capabilities {
    /// Media2 service endpoint URL (`None` if device does not support Media2).
    pub url: Option<String>,
}

/// DeviceIO service capabilities.
#[derive(Debug, Clone, Default)]
pub struct DeviceIoCapabilities {
    /// DeviceIO service endpoint URL (`None` if not supported).
    pub url: Option<String>,
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
    pub ptz: PtzCapabilities,
    pub imaging: ImagingCapabilities,
    pub recording: RecordingCapabilities,
    pub search: SearchCapabilities,
    pub replay: ReplayCapabilities,
    pub media2: Media2Capabilities,
    pub device_io: DeviceIoCapabilities,
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
            ptz: PtzCapabilities {
                url: caps.path(&["PTZ", "XAddr"]).map(|n| n.text().to_string()),
            },
            imaging: ImagingCapabilities {
                url: caps
                    .path(&["Imaging", "XAddr"])
                    .map(|n| n.text().to_string()),
            },
            recording: RecordingCapabilities {
                url: caps
                    .path(&["Extension", "Recording", "XAddr"])
                    .map(|n| n.text().to_string()),
            },
            search: SearchCapabilities {
                url: caps
                    .path(&["Extension", "Search", "XAddr"])
                    .map(|n| n.text().to_string()),
            },
            replay: ReplayCapabilities {
                url: caps
                    .path(&["Extension", "Replay", "XAddr"])
                    .map(|n| n.text().to_string()),
            },
            device_io: DeviceIoCapabilities {
                url: caps
                    .path(&["Extension", "DeviceIO", "XAddr"])
                    .map(|n| n.text().to_string()),
            },
            media2: Media2Capabilities {
                url: caps
                    .path(&["Extension", "Media2", "XAddr"])
                    .map(|n| n.text().to_string()),
            },
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
