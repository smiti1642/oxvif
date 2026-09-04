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

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOperator {
    #[default]
    Eq,
    Neq,
    Contains,
    Prefix,
    In,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    #[default]
    All,
    Any,
}

impl FromStr for MatchMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "all" => Ok(Self::All),
            "any" => Ok(Self::Any),
            _ => Err(AppError::invalid_argument(
                "--match must be `all` or `any`.",
            )),
        }
    }
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
    #[serde(default)]
    pub operator: FilterOperator,
    pub value: String,
}

impl FromStr for DeviceFilter {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (field_spec, value) = split_filter(value)?;
        let (field, operator) = field_spec
            .split_once(':')
            .map_or((field_spec, "eq"), |(field, operator)| (field, operator));
        let field = DeviceFilterField::parse(field)?;
        let operator = match operator {
            "eq" => FilterOperator::Eq,
            "neq" => FilterOperator::Neq,
            "contains" => FilterOperator::Contains,
            "prefix" => FilterOperator::Prefix,
            "in" => FilterOperator::In,
            _ => {
                return Err(AppError::invalid_argument(format!(
                    "Unknown filter operator `{operator}`."
                )));
            }
        };
        if operator == FilterOperator::In
            && field != DeviceFilterField::IpCidr
            && field != DeviceFilterField::Target
        {
            return Err(AppError::invalid_argument(
                "The `in` operator is supported only for target/ip-cidr fields.",
            ));
        }
        if field == DeviceFilterField::IpCidr
            && !matches!(
                operator,
                FilterOperator::Eq | FilterOperator::Neq | FilterOperator::In
            )
        {
            return Err(AppError::invalid_argument(
                "The ip-cidr field supports only `in`, `eq`, and `neq`.",
            ));
        }
        if operator == FilterOperator::In || field == DeviceFilterField::IpCidr {
            value.parse::<IpNet>().map_err(|error| {
                AppError::invalid_argument(format!("Invalid IP CIDR `{value}`: {error}"))
            })?;
        }
        let operator = if field == DeviceFilterField::IpCidr && operator == FilterOperator::Eq {
            FilterOperator::In
        } else {
            operator
        };
        Ok(Self {
            field,
            operator,
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
    pub match_mode: MatchMode,
}

/// Fields accepted when a dynamic View is created.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewSavedView {
    pub id: String,
    pub name: Option<String>,
    pub filters: Vec<DeviceFilter>,
    pub match_mode: MatchMode,
}

/// Match counts for one clause when `view evaluate --explain` is requested.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FilterExplanation {
    pub filter: DeviceFilter,
    pub matched_devices: usize,
    pub unmatched_devices: usize,
}

/// Aggregate explanation of a saved View evaluation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewExplanation {
    pub evaluated_devices: usize,
    pub matched_devices: usize,
    pub filters: Vec<FilterExplanation>,
}

/// A field accepted by live or retained discovery-result filtering.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryFilterField {
    Registration,
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
            "registration" | "registered" | "status" => Ok(Self::Registration),
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

/// One filter applied to a live discovery result or named snapshot query.
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
        if field == DiscoveryFilterField::Registration
            && !matches!(
                value.to_ascii_lowercase().as_str(),
                "saved" | "registered" | "new" | "unregistered" | "incomplete"
            )
        {
            return Err(AppError::invalid_argument(
                "Registration filters accept saved, registered, new, unregistered, or incomplete.",
            ));
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

/// Registration state attached to a discovery record at query time.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryRegistrationStatus {
    Saved,
    New,
    Incomplete,
}

impl DiscoveryRegistrationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Saved => "saved",
            Self::New => "new",
            Self::Incomplete => "incomplete",
        }
    }

    fn matches_filter(self, value: &str) -> bool {
        match value.to_ascii_lowercase().as_str() {
            "saved" | "registered" => self == Self::Saved,
            "new" => self == Self::New,
            "unregistered" => matches!(self, Self::New | Self::Incomplete),
            "incomplete" => self == Self::Incomplete,
            _ => false,
        }
    }
}

/// One discovery result annotated with its current registry state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiscoveryDeviceView {
    #[serde(flatten)]
    pub record: DiscoveryRecord,
    pub registration_status: DiscoveryRegistrationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registered_device_id: Option<String>,
}

impl DiscoveryDeviceView {
    pub(crate) fn new(record: DiscoveryRecord, registered_device_id: Option<String>) -> Self {
        let registration_status = if registered_device_id.is_some() {
            DiscoveryRegistrationStatus::Saved
        } else if record
            .xaddrs
            .iter()
            .any(|target| crate::registry::normalize_target(target).is_ok())
        {
            DiscoveryRegistrationStatus::New
        } else {
            DiscoveryRegistrationStatus::Incomplete
        };
        Self {
            record,
            registration_status,
            registered_device_id,
        }
    }
}

impl std::ops::Deref for DiscoveryDeviceView {
    type Target = DiscoveryRecord;

    fn deref(&self) -> &Self::Target {
        &self.record
    }
}

/// Counts shared by human discovery output and structured Agent output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiscoveryResultSummary {
    pub total_count: usize,
    pub matched_count: usize,
    pub saved_count: usize,
    pub new_count: usize,
    pub incomplete_count: usize,
}

impl DiscoveryResultSummary {
    pub(crate) fn new(total_count: usize, devices: &[DiscoveryDeviceView]) -> Self {
        Self {
            total_count,
            matched_count: devices.len(),
            saved_count: devices
                .iter()
                .filter(|device| device.registration_status == DiscoveryRegistrationStatus::Saved)
                .count(),
            new_count: devices
                .iter()
                .filter(|device| device.registration_status == DiscoveryRegistrationStatus::New)
                .count(),
            incomplete_count: devices
                .iter()
                .filter(|device| {
                    device.registration_status == DiscoveryRegistrationStatus::Incomplete
                })
                .count(),
        }
    }
}

/// Safe, registration-aware contents of one named discovery snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiscoverySnapshotResult {
    pub id: String,
    pub saved_at_unix_ms: u64,
    pub generation: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<String>,
    pub summary: DiscoveryResultSummary,
    pub devices: Vec<DiscoveryDeviceView>,
}

/// Summary returned when named discovery snapshots are listed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiscoverySnapshotSummary {
    pub id: String,
    pub saved_at_unix_ms: u64,
    pub generation: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<String>,
    pub device_count: usize,
}

/// Safe, serializable contents of one named discovery snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiscoverySnapshotView {
    pub id: String,
    pub saved_at_unix_ms: u64,
    pub generation: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<String>,
    pub devices: Vec<DiscoveryRecord>,
}

/// One explicit identity override for a discovery import record.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct DiscoveryImportOverride {
    pub endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

/// Versioned, secret-free import override document accepted from a file or stdin.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct DiscoveryImportOverrides {
    pub version: u32,
    #[serde(default)]
    pub devices: Vec<DiscoveryImportOverride>,
}

/// Outcome proposed for one record in a discovery import plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportDisposition {
    Create,
    AlreadyPresent,
    FilteredOut,
    Conflict,
}

/// Deterministic import proposal for one discovery record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiscoveryImportProposal {
    pub endpoint: String,
    pub source_fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_alias: Option<String>,
    pub disposition: ImportDisposition,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
}

/// Read-only, fingerprinted plan returned before a bulk import is applied.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiscoveryImportPlan {
    pub fingerprint: String,
    pub snapshot_id: String,
    pub snapshot_generation: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_profile: Option<String>,
    pub tags: Vec<String>,
    pub filters: Vec<DiscoveryFilter>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<DiscoveryImportOverride>,
    pub total_records: usize,
    pub create_count: usize,
    pub already_present_count: usize,
    pub filtered_out_count: usize,
    pub conflict_count: usize,
    pub proposals: Vec<DiscoveryImportProposal>,
}

/// Safe view of a reusable credential profile. Secret material is never present.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CredentialProfileView {
    pub id: String,
    pub username: String,
    pub has_credentials: bool,
    pub credential_availability: String,
}

pub(crate) fn device_matches(
    device: &DeviceView,
    filters: &[DeviceFilter],
    match_mode: MatchMode,
) -> bool {
    let matches = |filter: &DeviceFilter| match filter.field {
        DeviceFilterField::Id => compare(&device.id, filter),
        DeviceFilterField::Name => compare(&device.name, filter),
        DeviceFilterField::Target => compare_target(&device.target, filter),
        DeviceFilterField::DeviceUuid => optional_compare(&device.device_uuid, filter),
        DeviceFilterField::Manufacturer => optional_compare(&device.manufacturer, filter),
        DeviceFilterField::Model => optional_compare(&device.model, filter),
        DeviceFilterField::FirmwareVersion => optional_compare(&device.firmware_version, filter),
        DeviceFilterField::SerialNumber => optional_compare(&device.serial_number, filter),
        DeviceFilterField::Tag => collection_compare(&device.tags, filter),
        DeviceFilterField::IpCidr => {
            let contained = target_in_cidr(&device.target, &filter.value);
            if filter.operator == FilterOperator::Neq {
                !contained
            } else {
                contained
            }
        }
    };
    match match_mode {
        MatchMode::All => filters.iter().all(matches),
        MatchMode::Any => filters.iter().any(matches),
    }
}

fn compare(left: &str, filter: &DeviceFilter) -> bool {
    match filter.operator {
        FilterOperator::Eq => equal(left, &filter.value),
        FilterOperator::Neq => !equal(left, &filter.value),
        FilterOperator::Contains => left
            .to_ascii_lowercase()
            .contains(&filter.value.to_ascii_lowercase()),
        FilterOperator::Prefix => left
            .to_ascii_lowercase()
            .starts_with(&filter.value.to_ascii_lowercase()),
        FilterOperator::In => target_in_cidr(left, &filter.value),
    }
}

fn compare_target(left: &str, filter: &DeviceFilter) -> bool {
    if filter.operator == FilterOperator::In {
        target_in_cidr(left, &filter.value)
    } else {
        compare(left, filter)
    }
}

fn optional_compare(left: &Option<String>, filter: &DeviceFilter) -> bool {
    left.as_deref().is_some_and(|left| compare(left, filter))
}

fn collection_compare(values: &[String], filter: &DeviceFilter) -> bool {
    if filter.operator == FilterOperator::Neq {
        !values.is_empty() && values.iter().all(|value| compare(value, filter))
    } else {
        values.iter().any(|value| compare(value, filter))
    }
}

pub(crate) fn discovery_matches(device: &DiscoveryRecord, filters: &[DiscoveryFilter]) -> bool {
    filters.iter().all(|filter| match filter.field {
        DiscoveryFilterField::Registration => true,
        DiscoveryFilterField::Endpoint => equal(&device.endpoint, &filter.value),
        DiscoveryFilterField::DeviceUuid => equal(
            discovery_uuid_value(&device.endpoint),
            discovery_uuid_value(&filter.value),
        ),
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

pub(crate) fn discovery_view_matches(
    device: &DiscoveryDeviceView,
    filters: &[DiscoveryFilter],
) -> bool {
    discovery_matches(&device.record, filters)
        && filters.iter().all(|filter| {
            filter.field != DiscoveryFilterField::Registration
                || device.registration_status.matches_filter(&filter.value)
        })
}

/// Match the same case-insensitive free-text query used by interactive discovery.
///
/// The query covers every identity and addressing field visible in structured
/// discovery output, plus registration aliases used by `registration=` filters.
pub fn discovery_query_matches(device: &DiscoveryDeviceView, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }

    let registration_terms = match device.registration_status {
        DiscoveryRegistrationStatus::Saved => "saved registered",
        DiscoveryRegistrationStatus::New => "new unregistered",
        DiscoveryRegistrationStatus::Incomplete => "incomplete unregistered",
    };
    let scalar_fields = [
        device.endpoint.as_str(),
        device.manufacturer.as_deref().unwrap_or_default(),
        device.model.as_deref().unwrap_or_default(),
        device.firmware_version.as_deref().unwrap_or_default(),
        device.serial_number.as_deref().unwrap_or_default(),
        device.registered_device_id.as_deref().unwrap_or_default(),
        registration_terms,
    ];

    scalar_fields
        .into_iter()
        .chain(device.types.iter().map(String::as_str))
        .chain(device.scopes.iter().map(String::as_str))
        .chain(device.xaddrs.iter().map(String::as_str))
        .any(|value| value.to_lowercase().contains(&query))
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

fn discovery_uuid_value(value: &str) -> &str {
    let value = value.trim();
    if value
        .get(..9)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("urn:uuid:"))
    {
        value.get(9..).unwrap_or(value)
    } else if value
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("uuid:"))
    {
        value.get(5..).unwrap_or(value)
    } else {
        value
    }
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
        assert!("ip-cidr:neq=192.168.0.0/24".parse::<DeviceFilter>().is_ok());
        assert!(
            "ip-cidr:contains=192.168.0.0/24"
                .parse::<DeviceFilter>()
                .is_err()
        );
        assert!("registration=saved".parse::<DiscoveryFilter>().is_ok());
        assert!("status=unregistered".parse::<DiscoveryFilter>().is_ok());
        assert!("registration=unknown".parse::<DiscoveryFilter>().is_err());
    }

    #[test]
    fn registration_filter_distinguishes_saved_new_and_incomplete() {
        let record = DiscoveryRecord {
            endpoint: "urn:uuid:camera".to_owned(),
            types: Vec::new(),
            scopes: Vec::new(),
            xaddrs: vec!["http://192.0.2.10/onvif/device_service".to_owned()],
            manufacturer: None,
            model: None,
            firmware_version: None,
            serial_number: None,
        };
        let saved = DiscoveryDeviceView::new(record.clone(), Some("front-door".to_owned()));
        let new = DiscoveryDeviceView::new(record.clone(), None);
        let mut incomplete_record = record;
        incomplete_record.xaddrs.clear();
        let incomplete = DiscoveryDeviceView::new(incomplete_record, None);

        let saved_filter = ["registration=saved".parse().expect("saved filter")];
        let unregistered_filter = ["registration=unregistered"
            .parse()
            .expect("unregistered filter")];
        assert!(discovery_view_matches(&saved, &saved_filter));
        assert!(!discovery_view_matches(&new, &saved_filter));
        assert!(discovery_view_matches(&new, &unregistered_filter));
        assert!(discovery_view_matches(&incomplete, &unregistered_filter));
    }

    #[test]
    fn discovery_query_searches_all_structured_fields_and_registration_aliases() {
        let mut record = DiscoveryRecord {
            endpoint: "urn:uuid:camera-42".to_owned(),
            types: vec!["tds:Device".to_owned()],
            scopes: vec!["onvif://www.onvif.org/location/loading-dock".to_owned()],
            xaddrs: vec!["http://192.0.2.42/onvif/device_service".to_owned()],
            manufacturer: Some("GeoVision".to_owned()),
            model: Some("GV-TBL8810".to_owned()),
            firmware_version: Some("V111".to_owned()),
            serial_number: Some("SERIAL-42".to_owned()),
        };
        let saved = DiscoveryDeviceView::new(record.clone(), Some("front-door".to_owned()));

        for query in [
            "CAMERA-42",
            "tds:device",
            "loading-dock",
            "192.0.2.42",
            "geovision",
            "tbl8810",
            "v111",
            "serial-42",
            "front-door",
            "registered",
        ] {
            assert!(discovery_query_matches(&saved, query), "query {query}");
        }
        assert!(!discovery_query_matches(&saved, "not-present"));

        record.xaddrs.clear();
        let incomplete = DiscoveryDeviceView::new(record, None);
        assert!(discovery_query_matches(&incomplete, "unregistered"));
        assert!(discovery_query_matches(&incomplete, "incomplete"));
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

    #[test]
    fn discovery_uuid_filter_accepts_bare_or_prefixed_uuid() {
        let device = DiscoveryRecord {
            endpoint: "urn:uuid:ABC-123".to_owned(),
            types: Vec::new(),
            scopes: Vec::new(),
            xaddrs: Vec::new(),
            manufacturer: None,
            model: None,
            firmware_version: None,
            serial_number: None,
        };
        assert!(discovery_matches(
            &device,
            &["uuid=abc-123".parse().expect("filter should parse")]
        ));
        assert!(discovery_matches(
            &device,
            &["uuid=uuid:ABC-123".parse().expect("filter should parse")]
        ));
    }

    #[test]
    fn tag_neq_requires_every_existing_tag_to_differ() {
        let filter = "tag:neq=outdoor"
            .parse::<DeviceFilter>()
            .expect("valid filter");
        assert!(!collection_compare(
            &["outdoor".to_owned(), "loading-bay".to_owned()],
            &filter
        ));
        assert!(collection_compare(&["indoor".to_owned()], &filter));
        assert!(!collection_compare(&[], &filter));
    }
}
