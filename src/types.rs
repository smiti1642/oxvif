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
            ptz_url:      caps.path(&["PTZ",     "XAddr"]).map(|n| n.text().to_string()),
            imaging_url:  caps.path(&["Imaging", "XAddr"]).map(|n| n.text().to_string()),
            recording_url: caps.path(&["Extension", "Recording", "XAddr"]).map(|n| n.text().to_string()),
            search_url:    caps.path(&["Extension", "Search",    "XAddr"]).map(|n| n.text().to_string()),
            replay_url:    caps.path(&["Extension", "Replay",    "XAddr"]).map(|n| n.text().to_string()),
            device_io_url: caps.path(&["Extension", "DeviceIO",  "XAddr"]).map(|n| n.text().to_string()),
        })
    }
}

fn parse_device_caps(d: &XmlNode) -> DeviceCapabilities {
    DeviceCapabilities {
        url: xml_str(d, "XAddr"),
        network: d.child("Network").map(|n| NetworkCapabilities {
            ip_filter:          xml_bool(n, "IPFilter"),
            zero_configuration: xml_bool(n, "ZeroConfiguration"),
            ip_version6:        xml_bool(n, "IPVersion6"),
            dyn_dns:            xml_bool(n, "DynDNS"),
        }).unwrap_or_default(),
        system: d.child("System").map(|n| SystemCapabilities {
            discovery_resolve: xml_bool(n, "DiscoveryResolve"),
            discovery_bye:     xml_bool(n, "DiscoveryBye"),
            remote_discovery:  xml_bool(n, "RemoteDiscovery"),
            system_backup:     xml_bool(n, "SystemBackup"),
            system_logging:    xml_bool(n, "SystemLogging"),
            firmware_upgrade:  xml_bool(n, "FirmwareUpgrade"),
        }).unwrap_or_default(),
        io: d.child("IO").map(|n| IoCapabilities {
            input_connectors: xml_u32(n, "InputConnectors"),
            relay_outputs:    xml_u32(n, "RelayOutputs"),
        }).unwrap_or_default(),
        security: d.child("Security").map(|n| SecurityCapabilities {
            tls_1_2:               xml_bool(n, "TLS1.2"),
            onboard_key_generation: xml_bool(n, "OnboardKeyGeneration"),
            access_policy_config:  xml_bool(n, "AccessPolicyConfig"),
            x509_token:            xml_bool(n, "X.509Token"),
            username_token:        xml_bool(n, "UsernameToken"),
        }).unwrap_or_default(),
    }
}

fn parse_media_caps(m: &XmlNode) -> MediaCapabilities {
    MediaCapabilities {
        url: xml_str(m, "XAddr"),
        streaming: m.child("StreamingCapabilities").map(|n| StreamingCapabilities {
            rtp_multicast: xml_bool(n, "RTPMulticast"),
            rtp_tcp:       xml_bool(n, "RTP_TCP"),
            rtp_rtsp_tcp:  xml_bool(n, "RTP_RTSP_TCP"),
        }).unwrap_or_default(),
        max_profiles: xml_u32(m, "MaximumNumberOfProfiles"),
    }
}

fn parse_events_caps(e: &XmlNode) -> EventsCapabilities {
    EventsCapabilities {
        url:                    xml_str(e, "XAddr"),
        ws_subscription_policy: xml_bool(e, "WSSubscriptionPolicySupport"),
        ws_pull_point:          xml_bool(e, "WSPullPointSupport"),
    }
}

fn parse_analytics_caps(a: &XmlNode) -> AnalyticsCapabilities {
    AnalyticsCapabilities {
        url:                      xml_str(a, "XAddr"),
        rule_support:             xml_bool(a, "RuleSupport"),
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
            manufacturer:     xml_str(resp, "Manufacturer").unwrap_or_default(),
            model:            xml_str(resp, "Model").unwrap_or_default(),
            firmware_version: xml_str(resp, "FirmwareVersion").unwrap_or_default(),
            serial_number:    xml_str(resp, "SerialNumber").unwrap_or_default(),
            hardware_id:      xml_str(resp, "HardwareId").unwrap_or_default(),
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
                name:  xml_str(p, "Name").unwrap_or_default(),
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
            invalid_after_reboot:  xml_bool(media_uri, "InvalidAfterReboot"),
            timeout: xml_str(media_uri, "Timeout").unwrap_or_default(),
        })
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
            assert_eq!(caps.device.url.as_deref(),    Some("http://192.168.1.1/onvif/device_service"));
            assert_eq!(caps.media.url.as_deref(),     Some("http://192.168.1.1/onvif/media_service"));
            assert_eq!(caps.ptz_url.as_deref(),       Some("http://192.168.1.1/onvif/ptz_service"));
            assert_eq!(caps.events.url.as_deref(),    Some("http://192.168.1.1/onvif/events_service"));
            assert_eq!(caps.imaging_url.as_deref(),   Some("http://192.168.1.1/onvif/imaging_service"));
            assert_eq!(caps.analytics.url.as_deref(), Some("http://192.168.1.1/onvif/analytics_service"));
        }

        #[test]
        fn test_extension_service_urls_parsed() {
            let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
            assert_eq!(caps.device_io_url.as_deref(),  Some("http://192.168.1.1/onvif/deviceio_service"));
            assert_eq!(caps.recording_url.as_deref(),  Some("http://192.168.1.1/onvif/recording_service"));
            assert_eq!(caps.search_url.as_deref(),     Some("http://192.168.1.1/onvif/search_service"));
            assert_eq!(caps.replay_url.as_deref(),     Some("http://192.168.1.1/onvif/replay_service"));
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
}
