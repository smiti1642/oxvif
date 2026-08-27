use std::{net::IpAddr, str::FromStr};

use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{AppError, DeviceView};

/// One explicit member of a static device Group.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GroupMemberView {
    pub alias: String,
    pub device_id: String,
}

/// Safe, serializable view of a static device Group.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GroupView {
    pub id: String,
    pub name: String,
    pub members: Vec<GroupMemberView>,
}

/// Fields accepted when a Group is created.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewGroup {
    pub id: String,
    pub name: Option<String>,
}

/// A registered-device field used by a dynamic View filter.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceFilterField {
    Id,
    Name,
    Target,
    DeviceUuid,
    Manufacturer,
    Model,
    FirmwareVersion,
    SerialNumber,
    Tag,
    IpCidr,
}

impl DeviceFilterField {
    fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "id" => Ok(Self::Id),
            "name" => Ok(Self::Name),
            "target" => Ok(Self::Target),
            "device_uuid" | "uuid" => Ok(Self::DeviceUuid),
            "manufacturer" => Ok(Self::Manufacturer),
            "model" => Ok(Self::Model),
            "firmware" | "firmware_version" => Ok(Self::FirmwareVersion),
            "serial" | "serial_number" => Ok(Self::SerialNumber),
            "tag" => Ok(Self::Tag),
            "ip_cidr" | "ip-cidr" => Ok(Self::IpCidr),
            _ => Err(AppError::invalid_argument(format!(
                "Unknown device filter field `{value}`."
            ))),
        }
    }
}

/// One equality or CIDR clause in a dynamic device View.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct DeviceFilter {
    pub field: DeviceFilterField,
    pub value: String,
}

impl FromStr for DeviceFilter {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (field, value) = split_filter(value)?;
        let field = DeviceFilterField::parse(field)?;
        if field == DeviceFilterField::IpCidr {
            value.parse::<IpNet>().map_err(|error| {
                AppError::invalid_argument(format!("Invalid IP CIDR `{value}`: {error}"))
            })?;
        }
        Ok(Self {
            field,
            value: value.to_owned(),
        })
    }
}

/// Safe, serializable definition of a dynamic device View.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SavedView {
    pub id: String,
    pub name: String,
    pub filters: Vec<DeviceFilter>,
}

/// Fields accepted when a dynamic View is created.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewSavedView {
    pub id: String,
    pub name: Option<String>,
    pub filters: Vec<DeviceFilter>,
}

/// A WS-Discovery field accepted by snapshot filtering.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryFilterField {
    Endpoint,
    DeviceUuid,
    Type,
    Scope,
    Xaddr,
    IpCidr,
    Manufacturer,
    Model,
    FirmwareVersion,
    SerialNumber,
}

impl DiscoveryFilterField {
    fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "endpoint" => Ok(Self::Endpoint),
            "device_uuid" | "uuid" => Ok(Self::DeviceUuid),
            "type" => Ok(Self::Type),
            "scope" => Ok(Self::Scope),
            "xaddr" | "target" => Ok(Self::Xaddr),
            "ip_cidr" | "ip-cidr" => Ok(Self::IpCidr),
            "manufacturer" => Ok(Self::Manufacturer),
            "model" => Ok(Self::Model),
            "firmware" | "firmware_version" => Ok(Self::FirmwareVersion),
            "serial" | "serial_number" => Ok(Self::SerialNumber),
            _ => Err(AppError::invalid_argument(format!(
                "Unknown discovery filter field `{value}`."
            ))),
        }
    }
}

/// One filter applied to a named discovery snapshot.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct DiscoveryFilter {
    pub field: DiscoveryFilterField,
    pub value: String,
}

impl FromStr for DiscoveryFilter {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (field, value) = split_filter(value)?;
        let field = DiscoveryFilterField::parse(field)?;
        if field == DiscoveryFilterField::IpCidr {
            value.parse::<IpNet>().map_err(|error| {
                AppError::invalid_argument(format!("Invalid IP CIDR `{value}`: {error}"))
            })?;
        }
        Ok(Self {
            field,
            value: value.to_owned(),
        })
    }
}

/// One device observation retained in a discovery snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct DiscoveryRecord {
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub xaddrs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
}

/// Summary returned when named discovery snapshots are listed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiscoverySnapshotSummary {
    pub id: String,
    pub saved_at_unix_ms: u64,
    pub device_count: usize,
}

/// Safe, serializable contents of one named discovery snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiscoverySnapshotView {
    pub id: String,
    pub saved_at_unix_ms: u64,
    pub devices: Vec<DiscoveryRecord>,
}

/// Safe view of a reusable credential profile. Secret material is never present.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CredentialProfileView {
    pub id: String,
    pub username: String,
    pub has_credentials: bool,
}

pub(crate) fn device_matches(device: &DeviceView, filters: &[DeviceFilter]) -> bool {
    filters.iter().all(|filter| match filter.field {
        DeviceFilterField::Id => equal(&device.id, &filter.value),
        DeviceFilterField::Name => equal(&device.name, &filter.value),
        DeviceFilterField::Target => equal(&device.target, &filter.value),
        DeviceFilterField::DeviceUuid => optional_equal(&device.device_uuid, &filter.value),
        DeviceFilterField::Manufacturer => optional_equal(&device.manufacturer, &filter.value),
        DeviceFilterField::Model => optional_equal(&device.model, &filter.value),
        DeviceFilterField::FirmwareVersion => {
            optional_equal(&device.firmware_version, &filter.value)
        }
        DeviceFilterField::SerialNumber => optional_equal(&device.serial_number, &filter.value),
        DeviceFilterField::Tag => device.tags.iter().any(|tag| equal(tag, &filter.value)),
        DeviceFilterField::IpCidr => target_in_cidr(&device.target, &filter.value),
    })
}

pub(crate) fn discovery_matches(device: &DiscoveryRecord, filters: &[DiscoveryFilter]) -> bool {
    filters.iter().all(|filter| match filter.field {
        DiscoveryFilterField::Endpoint | DiscoveryFilterField::DeviceUuid => {
            equal(&device.endpoint, &filter.value)
        }
        DiscoveryFilterField::Type => device.types.iter().any(|value| equal(value, &filter.value)),
        DiscoveryFilterField::Scope => device
            .scopes
            .iter()
            .any(|value| equal(value, &filter.value)),
        DiscoveryFilterField::Xaddr => device
            .xaddrs
            .iter()
            .any(|value| equal(value, &filter.value)),
        DiscoveryFilterField::IpCidr => device
            .xaddrs
            .iter()
            .any(|target| target_in_cidr(target, &filter.value)),
        DiscoveryFilterField::Manufacturer => optional_equal(&device.manufacturer, &filter.value),
        DiscoveryFilterField::Model => optional_equal(&device.model, &filter.value),
        DiscoveryFilterField::FirmwareVersion => {
            optional_equal(&device.firmware_version, &filter.value)
        }
        DiscoveryFilterField::SerialNumber => optional_equal(&device.serial_number, &filter.value),
    })
}

fn split_filter(value: &str) -> Result<(&str, &str), AppError> {
    let (field, value) = value.split_once('=').ok_or_else(|| {
        AppError::invalid_argument("Filters use `field=value` syntax, for example `tag=outdoor`.")
    })?;
    let field = field.trim();
    let value = value.trim();
    if field.is_empty() || value.is_empty() {
        return Err(AppError::invalid_argument(
            "Filter fields and values must not be empty.",
        ));
    }
    Ok((field, value))
}

fn equal(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn optional_equal(left: &Option<String>, right: &str) -> bool {
    left.as_deref().is_some_and(|left| equal(left, right))
}

fn target_in_cidr(target: &str, cidr: &str) -> bool {
    let Ok(network) = cidr.parse::<IpNet>() else {
        return false;
    };
    target_ip(target).is_some_and(|ip| network.contains(&ip))
}

fn target_ip(target: &str) -> Option<IpAddr> {
    if let Ok(ip) = target.parse::<IpAddr>() {
        return Some(ip);
    }
    Url::parse(target).ok()?.host_str()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typed_filters_and_rejects_unknown_fields() {
        assert_eq!(
            "manufacturer=GeoVision"
                .parse::<DeviceFilter>()
                .expect("valid filter")
                .field,
            DeviceFilterField::Manufacturer
        );
        assert!("ip-cidr=192.168.0.0/24".parse::<DeviceFilter>().is_ok());
        assert!("unknown=value".parse::<DeviceFilter>().is_err());
        assert!("tag=".parse::<DeviceFilter>().is_err());
    }

    #[test]
    fn cidr_filter_reads_ip_from_onvif_url() {
        assert!(target_in_cidr(
            "http://192.168.20.15/onvif/device_service",
            "192.168.20.0/24"
        ));
        assert!(!target_in_cidr(
            "http://192.168.30.15/onvif/device_service",
            "192.168.20.0/24"
        ));
    }
}
