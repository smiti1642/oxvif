//! Per-service `GetServiceCapabilities` response types.
//!
//! **These are not [`Capabilities`](super::Capabilities).** The device-level
//! `GetCapabilities` answers *which services exist and at what URL*; each
//! service's own `GetServiceCapabilities` answers *what that service can do*.
//! The two overlap in name only — where a
//! field appears in both (`RTP_RTSP_TCP`, `WSSubscriptionPolicySupport`,
//! `TLS1.2`, …) the two answers come from different operations and may
//! legitimately disagree, so no struct is shared between the two modules.
//!
//! # Why every flag is `Option<bool>`
//!
//! `None` means the attribute was **absent**; `Some(false)` means the device
//! **said no**. Collapsing the two would make it impossible to tell firmware
//! that declines a feature from firmware that never mentioned it, which is the
//! whole point of asking. The device-level structs in
//! [`capabilities`](super::capabilities) use bare `bool` and cannot make that
//! distinction; that is a difference between the modules, not an inconsistency.
//!
//! List-valued attributes (`tt:StringList` / `tt:StringAttrList` /
//! `tt:IntList`) are a deliberate exception: they are `Vec<_>`, empty when
//! absent. For a list "absent" and "present but empty" both mean *no items*, so
//! an `Option<Vec<_>>` would offer a distinction with nothing behind it and
//! force every caller through a double unwrap.
//!
//! # Attribute names that look wrong and are not
//!
//! Four attributes carry dots and cannot be spelled as Rust identifiers:
//! `TLS1.0`, `TLS1.1`, `TLS1.2`, `X.509Token` become `tls1_0`, `tls1_1`,
//! `tls1_2`, `x509_token`. **The lookup string stays dotted** — renaming it to
//! match the field is a silent parse failure, not a compile error.
//!
//! Three flags are negative-sense (`DiscoveryNotSupported`,
//! `NetworkConfigNotSupported`, `UserConfigNotSupported`) and keep the schema's
//! polarity. Inverting them would make `None` mean *supported* for these three
//! and *unknown* for every other field in the same struct.
//!
//! `tev:Capabilities` has **no** `WSPullPointSupport`. That name belongs to the
//! device-level [`EventsCapabilities`](super::EventsCapabilities); the nearest
//! question here is answered by
//! [`max_pull_points`](EventsServiceCapabilities::max_pull_points).

use std::str::FromStr;

use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── Attribute helpers ─────────────────────────────────────────────────────────
//
// This is the first module to parse **attributes** rather than child elements,
// so `xml_bool` / `xml_u32` / `xml_str` in `super` do not apply — they read a
// child element's text, and widening them to cover attributes would change the
// meaning of "absent" for every existing caller.

/// `xs:boolean` attribute. `"true"` / `"1"` → `true`, any other present value
/// → `false`, absent → `None`.
fn attr_bool(n: &XmlNode, name: &str) -> Option<bool> {
    n.attr(name).map(|v| v == "true" || v == "1")
}

/// Numeric attribute. Absent **or unparseable** → `None`.
fn attr_num<T: FromStr>(n: &XmlNode, name: &str) -> Option<T> {
    n.attr(name).and_then(|v| v.parse().ok())
}

/// `xs:string` attribute as an owned `String`.
fn attr_str(n: &XmlNode, name: &str) -> Option<String> {
    n.attr(name).map(str::to_string)
}

/// `tt:StringList` / `tt:StringAttrList` attribute. Absent → empty.
fn attr_list(n: &XmlNode, name: &str) -> Vec<String> {
    n.attr(name)
        .map(|v| v.split_ascii_whitespace().map(str::to_string).collect())
        .unwrap_or_default()
}

/// `tt:IntList` attribute. Absent → empty; unparseable entries are dropped.
fn attr_u32_list(n: &XmlNode, name: &str) -> Vec<u32> {
    n.attr(name)
        .map(|v| {
            v.split_ascii_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// `tt:FloatList` attribute carrying exactly a min/max pair. Anything that is
/// not two parseable floats → `None`.
fn attr_float_pair(n: &XmlNode, name: &str) -> Option<(f32, f32)> {
    let mut it = n.attr(name)?.split_ascii_whitespace();
    let min: f32 = it.next()?.parse().ok()?;
    let max: f32 = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((min, max))
}

/// Fetch the `Capabilities` child of a `GetServiceCapabilitiesResponse`.
///
/// Every one of the nine services wraps its answer in an element named
/// `Capabilities` — including Media2, whose *type* is `tr2:Capabilities2` but
/// whose *element* is not.
fn caps_child(resp: &XmlNode) -> Result<&XmlNode, OnvifError> {
    resp.child("Capabilities")
        .ok_or_else(|| SoapError::missing("Capabilities").into())
}

/// Fetch a child the schema marks `minOccurs="1"`.
///
/// `path` is passed rather than built from `name` because
/// [`SoapError::missing`] takes a `&'static str`; the two must be kept in step
/// by hand at each call site.
fn required_child<'a>(
    caps: &'a XmlNode,
    name: &str,
    path: &'static str,
) -> Result<&'a XmlNode, OnvifError> {
    caps.child(name)
        .ok_or_else(|| SoapError::missing(path).into())
}

// ── Device ────────────────────────────────────────────────────────────────────

/// Addressing and network-service capabilities from `tds:Capabilities/Network`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct DeviceNetworkCapabilities {
    /// Traffic can be allowed or denied by source address.
    pub ip_filter: Option<bool>,
    /// Zero-configuration (link-local) addressing is supported.
    pub zero_configuration: Option<bool>,
    /// The device has an IPv6 stack.
    pub ip_version6: Option<bool>,
    /// The device can register with a dynamic DNS provider.
    pub dyn_dns: Option<bool>,
    /// IEEE 802.11 (Wi-Fi) configuration is supported.
    pub dot11_configuration: Option<bool>,
    /// Number of 802.1X configurations the device can hold.
    pub dot1x_configurations: Option<u32>,
    /// The hostname can be taken from DHCP.
    pub hostname_from_dhcp: Option<bool>,
    /// Number of NTP servers the device accepts.
    pub ntp: Option<u32>,
    /// Stateful DHCPv6 is supported.
    pub dhcpv6: Option<bool>,
}

/// Authentication and TLS capabilities from `tds:Capabilities/Security`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct DeviceSecurityCapabilities {
    /// TLS 1.0 is offered for HTTPS. Schema attribute is `TLS1.0`.
    pub tls1_0: Option<bool>,
    /// TLS 1.1 is offered for HTTPS. Schema attribute is `TLS1.1`.
    pub tls1_1: Option<bool>,
    /// TLS 1.2 is offered for HTTPS. Schema attribute is `TLS1.2`.
    pub tls1_2: Option<bool>,
    /// The device can generate a key pair itself, so a private key never has to
    /// be uploaded to it.
    pub onboard_key_generation: Option<bool>,
    /// Access policies can be read and written over ONVIF.
    pub access_policy_config: Option<bool>,
    /// A default access policy is supported.
    pub default_access_policy: Option<bool>,
    /// IEEE 802.1X port authentication is supported.
    pub dot1x: Option<bool>,
    /// Users can be managed on a remote server rather than on the device.
    pub remote_user_handling: Option<bool>,
    /// WS-Security X.509 certificate tokens are accepted. Schema attribute is
    /// `X.509Token`.
    pub x509_token: Option<bool>,
    /// WS-Security SAML tokens are accepted.
    pub saml_token: Option<bool>,
    /// WS-Security Kerberos tokens are accepted.
    pub kerberos_token: Option<bool>,
    /// WS-Security `UsernameToken` is accepted — what this crate sends.
    pub username_token: Option<bool>,
    /// HTTP Digest authentication is accepted.
    pub http_digest: Option<bool>,
    /// WS-Security REL tokens are accepted.
    pub rel_token: Option<bool>,
    /// JSON Web Tokens are accepted.
    pub json_web_token: Option<bool>,
    /// Maximum number of user accounts.
    pub max_users: Option<u32>,
    /// Maximum length of a user name.
    pub max_user_name_length: Option<u32>,
    /// Maximum length of a password.
    pub max_password_length: Option<u32>,
    /// How many previous passwords are remembered and refused.
    pub max_password_history: Option<u32>,
    /// Maximum number of user roles.
    pub max_user_roles: Option<u32>,
    /// EAP method numbers supported for 802.1X (`tt:IntList`).
    pub supported_eap_methods: Vec<u32>,
    /// Named security policies the device implements.
    pub security_policies: Vec<String>,
    /// Password hashing algorithms the device supports.
    pub hashing_algorithms: Vec<String>,
}

/// Discovery, backup, logging and storage capabilities from
/// `tds:Capabilities/System`.
///
/// The last three fields are **negative-sense** — `Some(true)` means the
/// feature is *not* supported. They keep the schema's polarity deliberately:
/// inverting them would make `None` mean *supported* for those three and
/// *unknown* for every other field in the same struct, and one stray `!` in a
/// parser would silently swap the meaning of a health verdict.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct DeviceSystemCapabilities {
    /// The device answers WS-Discovery `Resolve` messages.
    pub discovery_resolve: Option<bool>,
    /// The device sends a WS-Discovery `Bye` when going offline.
    pub discovery_bye: Option<bool>,
    /// The device can be discovered beyond the local subnet via a proxy.
    pub remote_discovery: Option<bool>,
    /// `GetSystemBackup` / `RestoreSystem` are supported.
    pub system_backup: Option<bool>,
    /// `GetSystemLog` is supported.
    pub system_logging: Option<bool>,
    /// Firmware can be upgraded from a vendor cloud service.
    pub cloud_firmware_upgrade: Option<bool>,
    /// Firmware can be upgraded over the HTTP upload interface.
    pub http_firmware_upgrade: Option<bool>,
    /// System backup can be fetched over HTTP.
    pub http_system_backup: Option<bool>,
    /// System log can be fetched over HTTP.
    pub http_system_logging: Option<bool>,
    /// Support information can be fetched over HTTP.
    pub http_support_information: Option<bool>,
    /// Storage configurations can be read and written.
    pub storage_configuration: Option<bool>,
    /// Maximum number of storage configurations.
    pub max_storage_configurations: Option<u32>,
    /// Number of geo-location entries the device can store.
    pub geo_location_entries: Option<u32>,
    /// Automatic geo-location sources the device uses.
    pub auto_geo: Vec<String>,
    /// Storage types the device accepts (`NFS`, `CIFS`, …).
    pub storage_types_supported: Vec<String>,
    /// Vendor add-ons installed on the device.
    pub addons: Vec<String>,
    /// Storage configurations can be renewed without being recreated.
    pub storage_configuration_renewal: Option<bool>,
    /// Vendor hardware type string.
    pub hardware_type: Option<String>,
    /// **Negative sense**: `Some(true)` means WS-Discovery is *not* supported.
    pub discovery_not_supported: Option<bool>,
    /// **Negative sense**: `Some(true)` means network configuration is *not*
    /// supported.
    pub network_config_not_supported: Option<bool>,
    /// **Negative sense**: `Some(true)` means user configuration is *not*
    /// supported.
    pub user_config_not_supported: Option<bool>,
}

/// `tds:Capabilities/Misc` — optional, and in practice carries one attribute.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct DeviceMiscCapabilities {
    /// The auxiliary commands this device accepts, e.g. `tt:Wiper|On`,
    /// `tt:IRLamp|Auto`.
    ///
    /// This is the **discoverable list** behind the auxiliary-command
    /// operations: there is no other way to learn what a given camera will
    /// take, because the values are vendor-namespaced rather than enumerated by
    /// the schema.
    pub auxiliary_commands: Vec<String>,
}

/// `tds:GetServiceCapabilities` — what the device management service can do.
///
/// Distinct from [`Capabilities`](super::Capabilities), which lists *which
/// services exist and at what URL*. Every flag here is `Option<bool>`: `None`
/// means the device did not mention the attribute, `Some(false)` means it said
/// no.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct DeviceServiceCapabilities {
    /// Addressing and network services.
    pub network: DeviceNetworkCapabilities,
    /// Authentication, TLS and user-account limits.
    pub security: DeviceSecurityCapabilities,
    /// Discovery, backup, logging, firmware and storage.
    pub system: DeviceSystemCapabilities,
    /// Optional extras. `None` when the device sent no `Misc` element.
    pub misc: Option<DeviceMiscCapabilities>,
}

impl DeviceServiceCapabilities {
    /// Parse from a `GetServiceCapabilitiesResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let caps = caps_child(resp)?;
        let net = required_child(caps, "Network", "Capabilities/Network")?;
        let sec = required_child(caps, "Security", "Capabilities/Security")?;
        let sys = required_child(caps, "System", "Capabilities/System")?;

        Ok(Self {
            network: DeviceNetworkCapabilities {
                ip_filter: attr_bool(net, "IPFilter"),
                zero_configuration: attr_bool(net, "ZeroConfiguration"),
                ip_version6: attr_bool(net, "IPVersion6"),
                dyn_dns: attr_bool(net, "DynDNS"),
                dot11_configuration: attr_bool(net, "Dot11Configuration"),
                dot1x_configurations: attr_num(net, "Dot1XConfigurations"),
                hostname_from_dhcp: attr_bool(net, "HostnameFromDHCP"),
                ntp: attr_num(net, "NTP"),
                dhcpv6: attr_bool(net, "DHCPv6"),
            },
            security: DeviceSecurityCapabilities {
                // Dotted lookup strings — see the module docs. These four must
                // not be renamed to match their Rust fields.
                tls1_0: attr_bool(sec, "TLS1.0"),
                tls1_1: attr_bool(sec, "TLS1.1"),
                tls1_2: attr_bool(sec, "TLS1.2"),
                onboard_key_generation: attr_bool(sec, "OnboardKeyGeneration"),
                access_policy_config: attr_bool(sec, "AccessPolicyConfig"),
                default_access_policy: attr_bool(sec, "DefaultAccessPolicy"),
                dot1x: attr_bool(sec, "Dot1X"),
                remote_user_handling: attr_bool(sec, "RemoteUserHandling"),
                x509_token: attr_bool(sec, "X.509Token"),
                saml_token: attr_bool(sec, "SAMLToken"),
                kerberos_token: attr_bool(sec, "KerberosToken"),
                username_token: attr_bool(sec, "UsernameToken"),
                http_digest: attr_bool(sec, "HttpDigest"),
                rel_token: attr_bool(sec, "RELToken"),
                json_web_token: attr_bool(sec, "JsonWebToken"),
                max_users: attr_num(sec, "MaxUsers"),
                max_user_name_length: attr_num(sec, "MaxUserNameLength"),
                max_password_length: attr_num(sec, "MaxPasswordLength"),
                max_password_history: attr_num(sec, "MaxPasswordHistory"),
                max_user_roles: attr_num(sec, "MaxUserRoles"),
                supported_eap_methods: attr_u32_list(sec, "SupportedEAPMethods"),
                security_policies: attr_list(sec, "SecurityPolicies"),
                hashing_algorithms: attr_list(sec, "HashingAlgorithms"),
            },
            system: DeviceSystemCapabilities {
                discovery_resolve: attr_bool(sys, "DiscoveryResolve"),
                discovery_bye: attr_bool(sys, "DiscoveryBye"),
                remote_discovery: attr_bool(sys, "RemoteDiscovery"),
                system_backup: attr_bool(sys, "SystemBackup"),
                system_logging: attr_bool(sys, "SystemLogging"),
                cloud_firmware_upgrade: attr_bool(sys, "CloudFirmwareUpgrade"),
                http_firmware_upgrade: attr_bool(sys, "HttpFirmwareUpgrade"),
                http_system_backup: attr_bool(sys, "HttpSystemBackup"),
                http_system_logging: attr_bool(sys, "HttpSystemLogging"),
                http_support_information: attr_bool(sys, "HttpSupportInformation"),
                storage_configuration: attr_bool(sys, "StorageConfiguration"),
                max_storage_configurations: attr_num(sys, "MaxStorageConfigurations"),
                geo_location_entries: attr_num(sys, "GeoLocationEntries"),
                auto_geo: attr_list(sys, "AutoGeo"),
                storage_types_supported: attr_list(sys, "StorageTypesSupported"),
                addons: attr_list(sys, "Addons"),
                storage_configuration_renewal: attr_bool(sys, "StorageConfigurationRenewal"),
                hardware_type: attr_str(sys, "HardwareType"),
                discovery_not_supported: attr_bool(sys, "DiscoveryNotSupported"),
                network_config_not_supported: attr_bool(sys, "NetworkConfigNotSupported"),
                user_config_not_supported: attr_bool(sys, "UserConfigNotSupported"),
            },
            misc: caps.child("Misc").map(|m| DeviceMiscCapabilities {
                auxiliary_commands: attr_list(m, "AuxiliaryCommands"),
            }),
        })
    }
}

// ── Media1 ────────────────────────────────────────────────────────────────────

/// `trt:ProfileCapabilities`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct MediaProfileCapabilities {
    /// Maximum number of media profiles the service will hold.
    pub maximum_number_of_profiles: Option<u32>,
}

/// `trt:StreamingCapabilities`.
///
/// Not the device-level [`StreamingCapabilities`](super::StreamingCapabilities)
/// and not [`Media2StreamingCapabilities`] — all three have different field
/// sets, and sharing one struct between them is the mistake this naming exists
/// to make impossible.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct MediaStreamingCapabilities {
    /// RTP over multicast UDP is offered.
    pub rtp_multicast: Option<bool>,
    /// RTP delivered directly over TCP. Schema attribute is `RTP_TCP`. Media2
    /// dropped this transport, so [`Media2StreamingCapabilities`] has no
    /// counterpart.
    pub rtp_tcp: Option<bool>,
    /// RTP interleaved in the RTSP TCP connection — the transport that gets
    /// through NAT and firewalls. Schema attribute is `RTP_RTSP_TCP`.
    pub rtp_rtsp_tcp: Option<bool>,
    /// Each stream can be controlled independently rather than only as an
    /// aggregate.
    pub non_aggregate_control: Option<bool>,
    /// **Negative sense**: `Some(true)` means RTSP streaming is *not* offered.
    pub no_rtsp_streaming: Option<bool>,
}

/// `trt:GetServiceCapabilities` — what the Media1 service can do.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct MediaServiceCapabilities {
    /// `GetSnapshotUri` is supported.
    pub snapshot_uri: Option<bool>,
    /// Video rotation can be configured.
    pub rotation: Option<bool>,
    /// Video source modes can be listed and switched.
    pub video_source_mode: Option<bool>,
    /// On-screen display configuration is supported.
    pub osd: Option<bool>,
    /// OSD text can be set temporarily rather than persisted.
    pub temporary_osd_text: Option<bool>,
    /// EXI compression of SOAP messages is supported. Schema attribute is
    /// `EXICompression`.
    pub exi_compression: Option<bool>,
    /// Profile limits. Required by the schema.
    pub profile: MediaProfileCapabilities,
    /// Stream transports. Required by the schema.
    pub streaming: MediaStreamingCapabilities,
}

impl MediaServiceCapabilities {
    /// Parse from a `GetServiceCapabilitiesResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let caps = caps_child(resp)?;
        let profile = required_child(
            caps,
            "ProfileCapabilities",
            "Capabilities/ProfileCapabilities",
        )?;
        let streaming = required_child(
            caps,
            "StreamingCapabilities",
            "Capabilities/StreamingCapabilities",
        )?;

        Ok(Self {
            snapshot_uri: attr_bool(caps, "SnapshotUri"),
            rotation: attr_bool(caps, "Rotation"),
            video_source_mode: attr_bool(caps, "VideoSourceMode"),
            osd: attr_bool(caps, "OSD"),
            temporary_osd_text: attr_bool(caps, "TemporaryOSDText"),
            exi_compression: attr_bool(caps, "EXICompression"),
            profile: MediaProfileCapabilities {
                maximum_number_of_profiles: attr_num(profile, "MaximumNumberOfProfiles"),
            },
            streaming: MediaStreamingCapabilities {
                rtp_multicast: attr_bool(streaming, "RTPMulticast"),
                rtp_tcp: attr_bool(streaming, "RTP_TCP"),
                rtp_rtsp_tcp: attr_bool(streaming, "RTP_RTSP_TCP"),
                non_aggregate_control: attr_bool(streaming, "NonAggregateControl"),
                no_rtsp_streaming: attr_bool(streaming, "NoRTSPStreaming"),
            },
        })
    }
}

// ── Media2 ────────────────────────────────────────────────────────────────────

/// `tr2:ProfileCapabilities`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct Media2ProfileCapabilities {
    /// Maximum number of media profiles the service will hold.
    pub maximum_number_of_profiles: Option<u32>,
    /// Configuration kinds that can be added to a profile (`VideoSource`,
    /// `VideoEncoder`, `Metadata`, …) — the values `AddConfiguration` accepts.
    pub configurations_supported: Vec<String>,
}

/// `tr2:StreamingCapabilities`.
///
/// Media2 dropped `RTP_TCP` and added `RTSPStreaming`, `AutoStartMulticast`,
/// `SecureRTSPStreaming` and `RTSPWebSocketUri`; it is **not** interchangeable
/// with [`MediaStreamingCapabilities`].
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct Media2StreamingCapabilities {
    /// Plain RTSP streaming is offered.
    pub rtsp_streaming: Option<bool>,
    /// RTP over multicast UDP is offered.
    pub rtp_multicast: Option<bool>,
    /// RTP interleaved in the RTSP TCP connection. Schema attribute is
    /// `RTP_RTSP_TCP`.
    pub rtp_rtsp_tcp: Option<bool>,
    /// Each stream can be controlled independently.
    pub non_aggregate_control: Option<bool>,
    /// Multicast starts without a client requesting it.
    pub auto_start_multicast: Option<bool>,
    /// RTSP over TLS is offered.
    pub secure_rtsp_streaming: Option<bool>,
    /// WebSocket endpoint for RTSP-over-WebSocket, if offered.
    pub rtsp_web_socket_uri: Option<String>,
}

/// `tr2:GetServiceCapabilities` — what the Media2 service can do.
///
/// The schema type is `tr2:Capabilities2`; only the *type* carries the `2`, the
/// response element is still `Capabilities`.
///
/// The two optional children `AudioClipCapabilities` and
/// `MulticastAudioDecoderCapabilities` are **deliberately not modelled**: their
/// contents describe audio-clip and multicast-audio-decoder operations that
/// this crate does not implement, so a field here would carry a type whose
/// members have never been exercised against a device.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct Media2ServiceCapabilities {
    /// `GetSnapshotUri` is supported.
    pub snapshot_uri: Option<bool>,
    /// Video rotation can be configured.
    pub rotation: Option<bool>,
    /// Video source modes can be listed and switched.
    pub video_source_mode: Option<bool>,
    /// On-screen display configuration is supported.
    pub osd: Option<bool>,
    /// OSD text can be set temporarily rather than persisted.
    pub temporary_osd_text: Option<bool>,
    /// Privacy masks on an encoder configuration are supported.
    pub mask: Option<bool>,
    /// Privacy masks on a video source are supported.
    pub source_mask: Option<bool>,
    /// Number of concurrent WebRTC sessions.
    ///
    /// This is an **`xs:int` session count, not a flag**: `Some(0)` means
    /// WebRTC is described and no concurrent session is offered. Reading it as
    /// a boolean both reports `0` as supported and loses the number.
    pub webrtc: Option<u32>,
    /// Codecs offered over WebRTC. Schema attribute is `WebRTC_codecs`.
    pub webrtc_codecs: Vec<String>,
    /// Profile limits. Required by the schema.
    pub profile: Media2ProfileCapabilities,
    /// Stream transports. Required by the schema.
    pub streaming: Media2StreamingCapabilities,
}

impl Media2ServiceCapabilities {
    /// Parse from a `GetServiceCapabilitiesResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let caps = caps_child(resp)?;
        let profile = required_child(
            caps,
            "ProfileCapabilities",
            "Capabilities/ProfileCapabilities",
        )?;
        let streaming = required_child(
            caps,
            "StreamingCapabilities",
            "Capabilities/StreamingCapabilities",
        )?;

        Ok(Self {
            snapshot_uri: attr_bool(caps, "SnapshotUri"),
            rotation: attr_bool(caps, "Rotation"),
            video_source_mode: attr_bool(caps, "VideoSourceMode"),
            osd: attr_bool(caps, "OSD"),
            temporary_osd_text: attr_bool(caps, "TemporaryOSDText"),
            mask: attr_bool(caps, "Mask"),
            source_mask: attr_bool(caps, "SourceMask"),
            webrtc: attr_num(caps, "WebRTC"),
            webrtc_codecs: attr_list(caps, "WebRTC_codecs"),
            profile: Media2ProfileCapabilities {
                maximum_number_of_profiles: attr_num(profile, "MaximumNumberOfProfiles"),
                configurations_supported: attr_list(profile, "ConfigurationsSupported"),
            },
            streaming: Media2StreamingCapabilities {
                rtsp_streaming: attr_bool(streaming, "RTSPStreaming"),
                rtp_multicast: attr_bool(streaming, "RTPMulticast"),
                rtp_rtsp_tcp: attr_bool(streaming, "RTP_RTSP_TCP"),
                non_aggregate_control: attr_bool(streaming, "NonAggregateControl"),
                auto_start_multicast: attr_bool(streaming, "AutoStartMulticast"),
                secure_rtsp_streaming: attr_bool(streaming, "SecureRTSPStreaming"),
                rtsp_web_socket_uri: attr_str(streaming, "RTSPWebSocketUri"),
            },
        })
    }
}

// ── PTZ ───────────────────────────────────────────────────────────────────────

/// `tptz:GetServiceCapabilities` — what the PTZ service can do.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct PtzServiceCapabilities {
    /// E-Flip (image flip when passing under the mount) is supported.
    pub eflip: Option<bool>,
    /// Pan/tilt direction can be reversed.
    pub reverse: Option<bool>,
    /// `GetCompatibleConfigurations` is supported.
    pub get_compatible_configurations: Option<bool>,
    /// `GetStatus` reports a `MoveStatus`.
    pub move_status: Option<bool>,
    /// `GetStatus` reports a `Position`.
    pub status_position: Option<bool>,
    /// Move-and-track methods the device offers — values of
    /// `tt:MoveAndTrackMethod` such as `PresetToken`, `GeoLocation`,
    /// `PTZVector`, `ObjectID`.
    ///
    /// Kept as strings rather than an enum: the schema may extend the set, and
    /// a vendor value must not turn a capability query into a parse failure.
    pub move_and_track: Vec<String>,
}

impl PtzServiceCapabilities {
    /// Parse from a `GetServiceCapabilitiesResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let caps = caps_child(resp)?;
        Ok(Self {
            eflip: attr_bool(caps, "EFlip"),
            reverse: attr_bool(caps, "Reverse"),
            get_compatible_configurations: attr_bool(caps, "GetCompatibleConfigurations"),
            move_status: attr_bool(caps, "MoveStatus"),
            status_position: attr_bool(caps, "StatusPosition"),
            move_and_track: attr_list(caps, "MoveAndTrack"),
        })
    }
}

// ── Imaging ───────────────────────────────────────────────────────────────────

/// `timg:GetServiceCapabilities` — what the imaging service can do.
///
/// Three attributes is the complete set.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct ImagingServiceCapabilities {
    /// Electronic image stabilisation is supported.
    pub image_stabilization: Option<bool>,
    /// Imaging presets can be listed and applied.
    pub presets: Option<bool>,
    /// Adaptable imaging presets are supported.
    ///
    /// Schema attribute is `AdaptablePreset` — singular, "Adaptable". The
    /// plausible-looking `AdaptivePresets` is not a real attribute name and
    /// would parse as absent forever.
    pub adaptable_preset: Option<bool>,
}

impl ImagingServiceCapabilities {
    /// Parse from a `GetServiceCapabilitiesResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let caps = caps_child(resp)?;
        Ok(Self {
            image_stabilization: attr_bool(caps, "ImageStabilization"),
            presets: attr_bool(caps, "Presets"),
            adaptable_preset: attr_bool(caps, "AdaptablePreset"),
        })
    }
}

// ── Events ────────────────────────────────────────────────────────────────────

/// `tev:GetServiceCapabilities` — what the events service can do.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct EventsServiceCapabilities {
    /// WS-BaseNotification subscription policy is supported.
    pub ws_subscription_policy_support: Option<bool>,
    /// The WS-BaseNotification pausable subscription manager interface is
    /// supported.
    pub ws_pausable_subscription_manager_interface_support: Option<bool>,
    /// Maximum number of notification producers.
    pub max_notification_producers: Option<u32>,
    /// Maximum number of concurrent pull points.
    ///
    /// This is the nearest thing here to the device-level
    /// `WSPullPointSupport`, which is **not** an attribute of this type.
    pub max_pull_points: Option<u32>,
    /// Notifications survive a reboot.
    pub persistent_notification_storage: Option<bool>,
    /// Event broker protocols the device speaks, e.g. `"mqtt mqtts"`.
    ///
    /// `xs:string` in the schema despite the space-separated content, so it is
    /// kept whole rather than split into a list.
    pub event_broker_protocols: Option<String>,
    /// Maximum number of configured event brokers.
    pub max_event_brokers: Option<u32>,
    /// Metadata can be published over MQTT. Schema attribute is
    /// `MetadataOverMQTT`.
    pub metadata_over_mqtt: Option<bool>,
}

impl EventsServiceCapabilities {
    /// Parse from a `GetServiceCapabilitiesResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let caps = caps_child(resp)?;
        Ok(Self {
            ws_subscription_policy_support: attr_bool(caps, "WSSubscriptionPolicySupport"),
            ws_pausable_subscription_manager_interface_support: attr_bool(
                caps,
                "WSPausableSubscriptionManagerInterfaceSupport",
            ),
            max_notification_producers: attr_num(caps, "MaxNotificationProducers"),
            max_pull_points: attr_num(caps, "MaxPullPoints"),
            persistent_notification_storage: attr_bool(caps, "PersistentNotificationStorage"),
            event_broker_protocols: attr_str(caps, "EventBrokerProtocols"),
            max_event_brokers: attr_num(caps, "MaxEventBrokers"),
            metadata_over_mqtt: attr_bool(caps, "MetadataOverMQTT"),
        })
    }
}

// ── Recording ─────────────────────────────────────────────────────────────────

/// `trc:GetServiceCapabilities` — what the recording service can do.
///
/// The widest of the nine at 21 attributes.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct RecordingServiceCapabilities {
    /// Recordings can be created and deleted at runtime.
    pub dynamic_recordings: Option<bool>,
    /// Tracks can be created and deleted at runtime.
    pub dynamic_tracks: Option<bool>,
    /// Encodings the service can record (`H264`, `AAC`, …).
    pub encoding: Vec<String>,
    /// Maximum rate of a single recording, in kbps. **`xs:float`** in the
    /// schema.
    pub max_rate: Option<f32>,
    /// Maximum combined rate of all recordings, in kbps. **`xs:float`**.
    pub max_total_rate: Option<f32>,
    /// Maximum number of recordings.
    ///
    /// **`xs:float` in the schema**, not an integer, despite reading like a
    /// count. Typed to match the schema rather than to match intuition.
    pub max_recordings: Option<f32>,
    /// Maximum number of recording jobs.
    pub max_recording_jobs: Option<u32>,
    /// `GetRecordingOptions` is supported.
    pub options: Option<bool>,
    /// Metadata tracks can be recorded.
    pub metadata_recording: Option<bool>,
    /// File formats `ExportRecordedData` can produce.
    pub supported_export_file_formats: Vec<String>,
    /// Event-triggered recording is supported.
    pub event_recording: Option<bool>,
    /// Maximum pre-event recording time, as an ISO 8601 duration.
    pub before_event_limit: Option<String>,
    /// Maximum post-event recording time, as an ISO 8601 duration.
    pub after_event_limit: Option<String>,
    /// Storage target formats the service supports.
    pub supported_target_formats: Vec<String>,
    /// Maximum number of encryption entries per recording.
    pub encryption_entry_limit: Option<u32>,
    /// Encryption modes the service supports.
    pub supported_encryption_modes: Vec<String>,
    /// Segment duration can be overridden per recording.
    pub override_segment_duration: Option<bool>,
    /// Asymmetric encryption of recordings is supported.
    pub asymmetric_encryption_supported: Option<bool>,
    /// Scheduled recording is supported.
    pub scheduled_recording: Option<bool>,
    /// The device records to onboard storage.
    ///
    /// The **schema default is `true`**, and this is the only defaulted
    /// attribute across all nine service-capability types. The default is
    /// deliberately *not* applied here — `None` still means "the device did not
    /// say", so the field keeps the same meaning as every other one in this
    /// module. A caller that wants the schema behaviour should read
    /// `onboard_storage.unwrap_or(true)`.
    pub onboard_storage: Option<bool>,
    /// Exporting a segment of a recording is supported.
    pub segment_export: Option<bool>,
}

impl RecordingServiceCapabilities {
    /// Parse from a `GetServiceCapabilitiesResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let caps = caps_child(resp)?;
        Ok(Self {
            dynamic_recordings: attr_bool(caps, "DynamicRecordings"),
            dynamic_tracks: attr_bool(caps, "DynamicTracks"),
            encoding: attr_list(caps, "Encoding"),
            max_rate: attr_num(caps, "MaxRate"),
            max_total_rate: attr_num(caps, "MaxTotalRate"),
            max_recordings: attr_num(caps, "MaxRecordings"),
            max_recording_jobs: attr_num(caps, "MaxRecordingJobs"),
            options: attr_bool(caps, "Options"),
            metadata_recording: attr_bool(caps, "MetadataRecording"),
            supported_export_file_formats: attr_list(caps, "SupportedExportFileFormats"),
            event_recording: attr_bool(caps, "EventRecording"),
            before_event_limit: attr_str(caps, "BeforeEventLimit"),
            after_event_limit: attr_str(caps, "AfterEventLimit"),
            supported_target_formats: attr_list(caps, "SupportedTargetFormats"),
            encryption_entry_limit: attr_num(caps, "EncryptionEntryLimit"),
            supported_encryption_modes: attr_list(caps, "SupportedEncryptionModes"),
            override_segment_duration: attr_bool(caps, "OverrideSegmentDuration"),
            asymmetric_encryption_supported: attr_bool(caps, "AsymmetricEncryptionSupported"),
            scheduled_recording: attr_bool(caps, "ScheduledRecording"),
            onboard_storage: attr_bool(caps, "OnboardStorage"),
            segment_export: attr_bool(caps, "SegmentExport"),
        })
    }
}

// ── Search ────────────────────────────────────────────────────────────────────

/// `tse:GetServiceCapabilities` — what the search service can do.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct SearchServiceCapabilities {
    /// `FindMetadata` is supported.
    pub metadata_search: Option<bool>,
    /// General start events are reported by `FindEvents`.
    pub general_start_events: Option<bool>,
    /// Natural-language search is supported. Schema attribute is `NLSearch`.
    pub nl_search: Option<bool>,
    /// Searching recorded images is supported.
    pub image_search: Option<bool>,
}

impl SearchServiceCapabilities {
    /// Parse from a `GetServiceCapabilitiesResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let caps = caps_child(resp)?;
        Ok(Self {
            metadata_search: attr_bool(caps, "MetadataSearch"),
            general_start_events: attr_bool(caps, "GeneralStartEvents"),
            nl_search: attr_bool(caps, "NLSearch"),
            image_search: attr_bool(caps, "ImageSearch"),
        })
    }
}

// ── Replay ────────────────────────────────────────────────────────────────────

/// `trp:GetServiceCapabilities` — what the replay service can do.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct ReplayServiceCapabilities {
    /// Playback can run backwards.
    pub reverse_playback: Option<bool>,
    /// Permitted replay session timeout range, as `(min, max)` seconds.
    ///
    /// A `tt:FloatList` — a whitespace-separated pair carried **in the
    /// attribute**, not a `Min`/`Max` sub-tree. Anything that is not exactly
    /// two parseable floats yields `None`.
    pub session_timeout_range: Option<(f32, f32)>,
    /// RTP interleaved in the RTSP TCP connection. Schema attribute is
    /// `RTP_RTSP_TCP`.
    pub rtp_rtsp_tcp: Option<bool>,
    /// WebSocket endpoint for RTSP-over-WebSocket replay, if offered.
    pub rtsp_web_socket_uri: Option<String>,
}

impl ReplayServiceCapabilities {
    /// Parse from a `GetServiceCapabilitiesResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let caps = caps_child(resp)?;
        Ok(Self {
            reverse_playback: attr_bool(caps, "ReversePlayback"),
            session_timeout_range: attr_float_pair(caps, "SessionTimeoutRange"),
            rtp_rtsp_tcp: attr_bool(caps, "RTP_RTSP_TCP"),
            rtsp_web_socket_uri: attr_str(caps, "RTSPWebSocketUri"),
        })
    }
}
