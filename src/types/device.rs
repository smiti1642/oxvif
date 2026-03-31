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
