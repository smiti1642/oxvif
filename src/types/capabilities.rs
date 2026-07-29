use super::{xml_bool, xml_str, xml_u32};
use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── Capabilities sub-structs ──────────────────────────────────────────────────

/// Network capabilities from `Device/Network`.
///
/// From the device-level `GetCapabilities`. The device service's own answer is
/// [`DeviceNetworkCapabilities`](super::DeviceNetworkCapabilities), which is a
/// wider set and distinguishes "said no" from "did not say".
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct NetworkCapabilities {
    /// The device can allow or deny traffic by source address.
    pub ip_filter: bool,
    /// Zero-configuration (link-local, 169.254.0.0/16) addressing is supported.
    pub zero_configuration: bool,
    /// The device has an IPv6 stack.
    pub ip_version6: bool,
    /// The device can register itself with a dynamic DNS provider.
    pub dyn_dns: bool,
}

/// System capabilities from `Device/System`.
///
/// From the device-level `GetCapabilities`. The device service's own answer is
/// [`DeviceSystemCapabilities`](super::DeviceSystemCapabilities).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct SystemCapabilities {
    /// The device answers WS-Discovery `Resolve` messages.
    pub discovery_resolve: bool,
    /// The device sends a WS-Discovery `Bye` when going offline.
    pub discovery_bye: bool,
    /// The device can be discovered beyond the local subnet, via a discovery
    /// proxy rather than multicast.
    pub remote_discovery: bool,
    /// `GetSystemBackup` / `RestoreSystem` are supported.
    pub system_backup: bool,
    /// `GetSystemLog` is supported.
    pub system_logging: bool,
    /// `StartFirmwareUpgrade` / `UpgradeSystemFirmware` are supported.
    pub firmware_upgrade: bool,
}

/// I/O capabilities from `Device/IO`.
///
/// From the device-level `GetCapabilities`. `tds:DeviceServiceCapabilities` has
/// no I/O counterpart — the DeviceIO service answers that.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct IoCapabilities {
    /// Number of digital inputs on the device.
    pub input_connectors: Option<u32>,
    /// Number of relay outputs on the device.
    pub relay_outputs: Option<u32>,
}

/// Security capabilities from `Device/Security`.
///
/// From the device-level `GetCapabilities`. The device service's own answer is
/// [`DeviceSecurityCapabilities`](super::DeviceSecurityCapabilities), which
/// covers all three TLS versions and the user-account limits.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct SecurityCapabilities {
    /// TLS 1.2 is supported for HTTPS.
    pub tls_1_2: bool,
    /// The device can generate a key pair itself, so a private key never has to
    /// be uploaded to it.
    pub onboard_key_generation: bool,
    /// Access policies can be read and written over ONVIF.
    pub access_policy_config: bool,
    /// WS-Security X.509 certificate tokens are accepted for authentication.
    pub x509_token: bool,
    /// `true` if the device supports WS-Security `UsernameToken`.
    pub username_token: bool,
}

/// Device management service capabilities.
///
/// From the device-level `GetCapabilities` — *that the device service exists,
/// and at what URL*. What it can **do** is
/// [`DeviceServiceCapabilities`](super::DeviceServiceCapabilities).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct DeviceCapabilities {
    /// Device management service endpoint URL.
    pub url: Option<String>,
    /// Addressing and network-service capabilities.
    pub network: NetworkCapabilities,
    /// Discovery, backup, logging and firmware-upgrade capabilities.
    pub system: SystemCapabilities,
    /// Digital input / relay output counts.
    pub io: IoCapabilities,
    /// Authentication and TLS capabilities.
    pub security: SecurityCapabilities,
}

/// RTP streaming capabilities from `Media/StreamingCapabilities`.
///
/// From the device-level `GetCapabilities`. Media1's own answer is
/// [`MediaStreamingCapabilities`](super::MediaStreamingCapabilities) and
/// Media2's is
/// [`Media2StreamingCapabilities`](super::Media2StreamingCapabilities); all
/// three have different field sets and are deliberately separate types.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct StreamingCapabilities {
    /// RTP over multicast UDP is offered.
    pub rtp_multicast: bool,
    /// RTP delivered directly over TCP.
    pub rtp_tcp: bool,
    /// RTP interleaved in the RTSP TCP connection — the transport that gets
    /// through NAT and firewalls, and what most clients should ask for.
    pub rtp_rtsp_tcp: bool,
}

/// Media service capabilities.
///
/// From the device-level `GetCapabilities`. Media1's own answer is
/// [`MediaServiceCapabilities`](super::MediaServiceCapabilities).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct MediaCapabilities {
    /// Media service endpoint URL.
    pub url: Option<String>,
    /// Which RTP transports the device offers.
    pub streaming: StreamingCapabilities,
    /// Maximum number of media profiles the device supports.
    pub max_profiles: Option<u32>,
}

/// Events service capabilities.
///
/// From the device-level `GetCapabilities`. The events service's own answer is
/// [`EventsServiceCapabilities`](super::EventsServiceCapabilities) — which has
/// no `WSPullPointSupport`; that attribute is only here.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct AnalyticsCapabilities {
    /// Analytics service endpoint URL.
    pub url: Option<String>,
    /// Analytics rules can be listed and configured.
    pub rule_support: bool,
    /// Analytics modules can be listed and configured.
    pub analytics_module_support: bool,
}

/// PTZ service capabilities.
///
/// From the device-level `GetCapabilities` — the URL only. What the PTZ service
/// can do is [`PtzServiceCapabilities`](super::PtzServiceCapabilities).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct PtzCapabilities {
    /// PTZ service endpoint URL (`None` if not supported).
    pub url: Option<String>,
}

/// Imaging service capabilities.
///
/// From the device-level `GetCapabilities` — the URL only. See
/// [`ImagingServiceCapabilities`](super::ImagingServiceCapabilities).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct ImagingCapabilities {
    /// Imaging service endpoint URL (`None` if not supported).
    pub url: Option<String>,
}

/// Recording service capabilities.
///
/// From the device-level `GetCapabilities` — the URL only. See
/// [`RecordingServiceCapabilities`](super::RecordingServiceCapabilities).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct RecordingCapabilities {
    /// Recording service endpoint URL (`None` if not supported).
    pub url: Option<String>,
}

/// Search service capabilities.
///
/// From the device-level `GetCapabilities` — the URL only. See
/// [`SearchServiceCapabilities`](super::SearchServiceCapabilities).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct SearchCapabilities {
    /// Search service endpoint URL (`None` if not supported).
    pub url: Option<String>,
}

/// Replay service capabilities.
///
/// From the device-level `GetCapabilities` — the URL only. See
/// [`ReplayServiceCapabilities`](super::ReplayServiceCapabilities).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct ReplayCapabilities {
    /// Replay service endpoint URL (`None` if not supported).
    pub url: Option<String>,
}

/// Media2 service capabilities.
///
/// From the device-level `GetCapabilities` — the URL only. What the Media2
/// service can do is
/// [`Media2ServiceCapabilities`](super::Media2ServiceCapabilities).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct Media2Capabilities {
    /// Media2 service endpoint URL (`None` if device does not support Media2).
    pub url: Option<String>,
}

/// DeviceIO service capabilities.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
/// This answers **which services exist and where**. What each service can *do*
/// is a separate call per service — see
/// [`DeviceServiceCapabilities`](super::DeviceServiceCapabilities) and its
/// eight siblings. A `false` here means "the device did not say yes", because
/// this type cannot distinguish absent from denied; the per-service types can.
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    /// Device management service — always present on a conformant device.
    pub device: DeviceCapabilities,
    /// Media1 service: profiles, encoders, stream URIs.
    pub media: MediaCapabilities,
    /// Events service: PullPoint and push subscriptions.
    pub events: EventsCapabilities,
    /// Analytics service.
    pub analytics: AnalyticsCapabilities,
    /// PTZ service. `ptz.url` is `None` on a fixed camera.
    pub ptz: PtzCapabilities,
    /// Imaging service: brightness, focus, white balance.
    pub imaging: ImagingCapabilities,
    /// Recording service: recordings and recording jobs on the device.
    pub recording: RecordingCapabilities,
    /// Search service: querying what the device has recorded.
    pub search: SearchCapabilities,
    /// Replay service: playback URIs for stored recordings.
    pub replay: ReplayCapabilities,
    /// Media2 service. Most devices do **not** report this here — Media2 is
    /// discoverable only via `GetServices`, which is why
    /// [`OnvifSession`](crate::OnvifSession) calls that as a fallback.
    pub media2: Media2Capabilities,
    /// DeviceIO service: relay outputs and digital inputs.
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
        max_profiles: m
            .path(&[
                "Extension",
                "ProfileCapabilities",
                "MaximumNumberOfProfiles",
            ])
            .and_then(|n| n.text().parse().ok()),
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
