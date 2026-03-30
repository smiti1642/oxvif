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

// ── Capabilities ──────────────────────────────────────────────────────────────

/// Service endpoint URLs returned by `GetCapabilities`.
///
/// Fields are `Option` because a device is not required to support every
/// service. Always check before use.
///
/// # Usage
///
/// ```no_run
/// # use oxvif::{OnvifClient, OnvifError};
/// # async fn run() -> Result<(), OnvifError> {
/// let client = OnvifClient::new("http://192.168.1.1/onvif/device_service");
/// let caps = client.get_capabilities().await?;
///
/// if let Some(media_url) = &caps.media_url {
///     let profiles = client.get_profiles(media_url).await?;
/// }
/// # Ok(()) }
/// ```
#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    pub device_url: Option<String>,
    pub media_url: Option<String>,
    pub ptz_url: Option<String>,
    pub events_url: Option<String>,
    pub imaging_url: Option<String>,
    pub analytics_url: Option<String>,
}

impl Capabilities {
    /// Parse from a `GetCapabilitiesResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let caps = resp
            .child("Capabilities")
            .ok_or_else(|| SoapError::missing("Capabilities"))?;

        Ok(Self {
            device_url: caps
                .path(&["Device", "XAddr"])
                .map(|n| n.text().to_string()),
            media_url: caps.path(&["Media", "XAddr"]).map(|n| n.text().to_string()),
            ptz_url: caps.path(&["PTZ", "XAddr"]).map(|n| n.text().to_string()),
            events_url: caps
                .path(&["Events", "XAddr"])
                .map(|n| n.text().to_string()),
            imaging_url: caps
                .path(&["Imaging", "XAddr"])
                .map(|n| n.text().to_string()),
            analytics_url: caps
                .path(&["Analytics", "XAddr"])
                .map(|n| n.text().to_string()),
        })
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
            manufacturer: resp
                .child("Manufacturer")
                .map(|n| n.text().to_string())
                .unwrap_or_default(),
            model: resp
                .child("Model")
                .map(|n| n.text().to_string())
                .unwrap_or_default(),
            firmware_version: resp
                .child("FirmwareVersion")
                .map(|n| n.text().to_string())
                .unwrap_or_default(),
            serial_number: resp
                .child("SerialNumber")
                .map(|n| n.text().to_string())
                .unwrap_or_default(),
            hardware_id: resp
                .child("HardwareId")
                .map(|n| n.text().to_string())
                .unwrap_or_default(),
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
                name: p
                    .child("Name")
                    .map(|n| n.text().to_string())
                    .unwrap_or_default(),
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
            invalid_after_connect: media_uri
                .child("InvalidAfterConnect")
                .is_some_and(|n| n.text() == "true"),
            invalid_after_reboot: media_uri
                .child("InvalidAfterReboot")
                .is_some_and(|n| n.text() == "true"),
            timeout: media_uri
                .child("Timeout")
                .map(|n| n.text().to_string())
                .unwrap_or_default(),
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

        const FULL: &str = r#"<GetCapabilitiesResponse>
          <Capabilities>
            <Device>    <XAddr>http://192.168.1.1/onvif/device_service</XAddr>    </Device>
            <Media>     <XAddr>http://192.168.1.1/onvif/media_service</XAddr>     </Media>
            <PTZ>       <XAddr>http://192.168.1.1/onvif/ptz_service</XAddr>       </PTZ>
            <Events>    <XAddr>http://192.168.1.1/onvif/events_service</XAddr>    </Events>
            <Imaging>   <XAddr>http://192.168.1.1/onvif/imaging_service</XAddr>   </Imaging>
            <Analytics> <XAddr>http://192.168.1.1/onvif/analytics_service</XAddr> </Analytics>
          </Capabilities>
        </GetCapabilitiesResponse>"#;

        #[test]
        fn test_all_service_urls_parsed() {
            let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
            assert_eq!(
                caps.device_url.as_deref(),
                Some("http://192.168.1.1/onvif/device_service")
            );
            assert_eq!(
                caps.media_url.as_deref(),
                Some("http://192.168.1.1/onvif/media_service")
            );
            assert_eq!(
                caps.ptz_url.as_deref(),
                Some("http://192.168.1.1/onvif/ptz_service")
            );
            assert_eq!(
                caps.events_url.as_deref(),
                Some("http://192.168.1.1/onvif/events_service")
            );
            assert_eq!(
                caps.imaging_url.as_deref(),
                Some("http://192.168.1.1/onvif/imaging_service")
            );
            assert_eq!(
                caps.analytics_url.as_deref(),
                Some("http://192.168.1.1/onvif/analytics_service")
            );
        }

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
            assert!(caps.events_url.is_none());
            assert!(caps.imaging_url.is_none());
            assert!(caps.analytics_url.is_none());
        }

        #[test]
        fn test_missing_capabilities_node_is_error() {
            let err = Capabilities::from_xml(&parse("<GetCapabilitiesResponse/>")).unwrap_err();
            assert!(matches!(
                err,
                OnvifError::Soap(SoapError::MissingField("Capabilities"))
            ));
        }

        #[test]
        fn test_device_url_always_present_in_full_response() {
            let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
            assert!(caps.device_url.is_some());
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
