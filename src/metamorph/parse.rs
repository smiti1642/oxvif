//! Parse verification: run oxvif's **own typed parser** over each recorded clone
//! response and report whether it parses, plus the value it extracts.
//!
//! This is the value / type-level half of the quirk diff, complementary to — not
//! a replacement for — the structural [`diff_against_synthetic`] /
//! [`diff_details`]. The two answer different questions:
//!
//! - **Structural SOAP diff** ([`diff_details`]): how the device's *wire shape*
//!   differs from oxvif's synthetic baseline. Baseline-relative, oxvif-independent
//!   wire truth — the evidence layer.
//! - **Parse verification** (here): will oxvif's parser *choke* on this device,
//!   and what does it extract. Baseline-free, oxvif-opinionated — the verdict
//!   layer.
//!
//! A quirk the structural diff is blind to but this catches: a device that
//! returns `<Width>1080p</Width>` where oxvif expects an integer — same element
//! path (no structural drift), but the typed parser rejects it. Conversely a
//! harmless vendor extension shows as structural drift but parses fine here.
//!
//! Both reports are keyed by `(action, key_canon)`, so a UI can join them per
//! operation: the parse verdict as the headline badge, the side-by-side SOAP diff
//! as the drill-down evidence, and [`ParseVerdict::value`] as "what oxvif got"
//! next to the raw XML.
//!
//! [`diff_against_synthetic`]: FixtureStore::diff_against_synthetic
//! [`diff_details`]: FixtureStore::diff_details

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::OnvifClient;
use crate::transport::{Transport, TransportError};

use super::fixture::FixtureStore;

/// Service URL handed to the client — a single-response transport ignores it.
const URL: &str = "http://parse-verify";
/// Dummy token for per-token reads — the transport returns the recorded response
/// regardless of request args, so the token value is irrelevant.
const TOK: &str = "x";

/// Outcome of parsing one recorded response with oxvif's typed parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ParseStatus {
    /// oxvif's parser accepted the response.
    Parsed,
    /// oxvif's parser rejected it (SOAP fault, malformed body, missing required
    /// field, or an out-of-range value / enum) — an interop quirk.
    Failed,
    /// No parser is wired for this operation (e.g. a write or event op that the
    /// clone sweep never records), so it was left unchecked.
    Unverified,
}

/// The parse result for one recorded operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseVerdict {
    /// The SOAP action URI this exchange answered.
    pub action: String,
    /// The canonical, ephemera-masked request (the fixture key) — the join key
    /// shared with [`OperationQuirk`](super::OperationQuirk) /
    /// [`OperationDiff`](super::OperationDiff).
    pub key_canon: String,
    /// Whether oxvif parsed the response.
    pub status: ParseStatus,
    /// The parser error, when `status` is [`ParseStatus::Failed`].
    pub error: Option<String>,
    /// The extracted typed value as JSON, when `status` is
    /// [`ParseStatus::Parsed`] — "what oxvif got", for display beside the raw XML.
    pub value: Option<serde_json::Value>,
}

/// The result of parse-verifying a whole clone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseReport {
    /// The device label the clone was recorded for.
    pub device: String,
    /// One verdict per recorded exchange, in the store's insertion order.
    pub verdicts: Vec<ParseVerdict>,
}

impl ParseReport {
    /// The operations whose response oxvif failed to parse — the interop quirks.
    pub fn failures(&self) -> impl Iterator<Item = &ParseVerdict> {
        self.verdicts
            .iter()
            .filter(|v| v.status == ParseStatus::Failed)
    }

    /// Whether every checked operation parsed (no failures; unverified ignored).
    pub fn all_parsed(&self) -> bool {
        self.verdicts
            .iter()
            .all(|v| v.status != ParseStatus::Failed)
    }
}

impl FixtureStore {
    /// Run oxvif's typed parser over every recorded response and report whether
    /// each parses, plus the extracted value. See the [module docs](crate::metamorph)
    /// for how this complements the structural [`diff_against_synthetic`].
    ///
    /// [`diff_against_synthetic`]: FixtureStore::diff_against_synthetic
    ///
    /// ```no_run
    /// # async fn run() -> std::io::Result<()> {
    /// use oxvif::metamorph::FixtureStore;
    /// let store = FixtureStore::load("clones/hikvision-ds2cd")?;
    /// let report = store.verify_parsing().await;
    /// for v in report.failures() {
    ///     eprintln!("oxvif cannot parse {}: {}", v.action, v.error.as_deref().unwrap_or(""));
    /// }
    /// # Ok(()) }
    /// ```
    pub async fn verify_parsing(&self) -> ParseReport {
        let mut verdicts = Vec::new();
        for f in self.fixtures() {
            let (status, error, value) = parse_check(&f.action, &f.response_raw).await;
            verdicts.push(ParseVerdict {
                action: f.action.clone(),
                key_canon: f.key_canon.clone(),
                status,
                error,
                value,
            });
        }
        ParseReport {
            device: self.device().to_string(),
            verdicts,
        }
    }
}

/// A [`Transport`] that answers every request with the same fixed response — so a
/// client method parses exactly the recorded bytes, ignoring its request args.
struct SingleResponse(String);

#[async_trait]
impl Transport for SingleResponse {
    async fn soap_post(&self, _: &str, _: &str, _: String) -> Result<String, TransportError> {
        Ok(self.0.clone())
    }
}

/// Route `action` to oxvif's matching read parser, run it over `response`, and
/// classify the result. Unmapped operations return [`ParseStatus::Unverified`].
async fn parse_check(
    action: &str,
    response: &str,
) -> (ParseStatus, Option<String>, Option<serde_json::Value>) {
    let c = OnvifClient::new(URL).with_transport(Arc::new(SingleResponse(response.to_string())));
    let op = action.rsplit('/').next().unwrap_or("");

    // `chk!` runs a client read call and maps Ok → Parsed(+JSON), Err → Failed.
    macro_rules! chk {
        ($call:expr) => {
            match $call.await {
                Ok(v) => (ParseStatus::Parsed, None, serde_json::to_value(&v).ok()),
                Err(e) => (ParseStatus::Failed, Some(e.to_string()), None),
            }
        };
    }

    if action.contains("/events/wsdl/") {
        return match op {
            "GetEventPropertiesRequest" => chk!(c.get_event_properties(URL)),
            _ => (ParseStatus::Unverified, None, None),
        };
    }
    let Some(tail) = action.strip_prefix("http://www.onvif.org/") else {
        return (ParseStatus::Unverified, None, None);
    };

    if tail.starts_with("ver10/device/wsdl/") {
        match op {
            "GetCapabilities" => chk!(c.get_capabilities()),
            "GetServices" => chk!(c.get_services()),
            "GetSystemDateAndTime" => chk!(c.get_system_date_and_time()),
            "GetDeviceInformation" => chk!(c.get_device_info()),
            "GetHostname" => chk!(c.get_hostname()),
            "GetScopes" => chk!(c.get_scopes()),
            "GetUsers" => chk!(c.get_users()),
            "GetNetworkInterfaces" => chk!(c.get_network_interfaces()),
            "GetNetworkProtocols" => chk!(c.get_network_protocols()),
            "GetDNS" => chk!(c.get_dns()),
            "GetNTP" => chk!(c.get_ntp()),
            "GetNetworkDefaultGateway" => chk!(c.get_network_default_gateway()),
            _ => (ParseStatus::Unverified, None, None),
        }
    } else if tail.starts_with("ver20/media/wsdl/") {
        match op {
            "GetProfiles" => chk!(c.get_profiles_media2(URL)),
            "GetStreamUri" => chk!(c.get_stream_uri_media2(URL, TOK)),
            "GetSnapshotUri" => chk!(c.get_snapshot_uri_media2(URL, TOK)),
            "GetVideoSourceConfigurations" => chk!(c.get_video_source_configurations_media2(URL)),
            "GetVideoSourceConfigurationOptions" => {
                chk!(c.get_video_source_configuration_options_media2(URL, None))
            }
            "GetVideoEncoderConfigurations" => chk!(c.get_video_encoder_configurations_media2(URL)),
            "GetVideoEncoderConfigurationOptions" => {
                chk!(c.get_video_encoder_configuration_options_media2(URL, None))
            }
            "GetVideoEncoderInstances" => chk!(c.get_video_encoder_instances_media2(URL, TOK)),
            _ => (ParseStatus::Unverified, None, None),
        }
    } else if tail.starts_with("ver10/media/wsdl/") {
        match op {
            "GetProfiles" => chk!(c.get_profiles(URL)),
            "GetProfile" => chk!(c.get_profile(URL, TOK)),
            "GetStreamUri" => chk!(c.get_stream_uri(URL, TOK)),
            "GetSnapshotUri" => chk!(c.get_snapshot_uri(URL, TOK)),
            "GetVideoSources" => chk!(c.get_video_sources(URL)),
            "GetVideoSourceConfigurations" => chk!(c.get_video_source_configurations(URL)),
            "GetVideoSourceConfiguration" => chk!(c.get_video_source_configuration(URL, TOK)),
            "GetVideoSourceConfigurationOptions" => {
                chk!(c.get_video_source_configuration_options(URL, None))
            }
            "GetVideoEncoderConfigurations" => chk!(c.get_video_encoder_configurations(URL)),
            "GetVideoEncoderConfiguration" => chk!(c.get_video_encoder_configuration(URL, TOK)),
            "GetVideoEncoderConfigurationOptions" => {
                chk!(c.get_video_encoder_configuration_options(URL, None))
            }
            "GetOSDs" => chk!(c.get_osds(URL, None)),
            "GetOSD" => chk!(c.get_osd(URL, TOK)),
            "GetOSDOptions" => chk!(c.get_osd_options(URL, TOK)),
            "GetAudioSources" => chk!(c.get_audio_sources(URL)),
            "GetAudioSourceConfigurations" => chk!(c.get_audio_source_configurations(URL)),
            "GetAudioEncoderConfigurations" => chk!(c.get_audio_encoder_configurations(URL)),
            "GetAudioEncoderConfiguration" => chk!(c.get_audio_encoder_configuration(URL, TOK)),
            "GetAudioEncoderConfigurationOptions" => {
                chk!(c.get_audio_encoder_configuration_options(URL, TOK))
            }
            _ => (ParseStatus::Unverified, None, None),
        }
    } else if tail.starts_with("ver20/ptz/wsdl/") {
        match op {
            "GetConfigurations" => chk!(c.ptz_get_configurations(URL)),
            "GetConfiguration" => chk!(c.ptz_get_configuration(URL, TOK)),
            "GetConfigurationOptions" => chk!(c.ptz_get_configuration_options(URL, TOK)),
            "GetNodes" => chk!(c.ptz_get_nodes(URL)),
            "GetNode" => chk!(c.ptz_get_node(URL, TOK)),
            "GetPresets" => chk!(c.ptz_get_presets(URL, TOK)),
            "GetStatus" => chk!(c.ptz_get_status(URL, TOK)),
            "GetCompatibleConfigurations" => chk!(c.ptz_get_compatible_configurations(URL, TOK)),
            _ => (ParseStatus::Unverified, None, None),
        }
    } else if tail.starts_with("ver20/imaging/wsdl/") {
        match op {
            "GetImagingSettings" => chk!(c.get_imaging_settings(URL, TOK)),
            "GetOptions" => chk!(c.get_imaging_options(URL, TOK)),
            "GetMoveOptions" => chk!(c.imaging_get_move_options(URL, TOK)),
            "GetStatus" => chk!(c.imaging_get_status(URL, TOK)),
            _ => (ParseStatus::Unverified, None, None),
        }
    } else {
        (ParseStatus::Unverified, None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::dispatch::dispatch;
    use crate::mock::state::MockState;

    const HOSTNAME: &str = "http://www.onvif.org/ver10/device/wsdl/GetHostname";

    fn valid_hostname_response() -> String {
        let state = MockState::new();
        dispatch(
            HOSTNAME,
            "http://x",
            &state,
            "<Envelope><Body><GetHostname/></Body></Envelope>",
        )
    }

    #[tokio::test]
    async fn parses_valid_response_and_extracts_value() {
        let mut store = FixtureStore::new("clone");
        store.record(HOSTNAME, "<req/>", &valid_hostname_response());

        let report = store.verify_parsing().await;
        assert!(report.all_parsed());
        let v = &report.verdicts[0];
        assert_eq!(v.status, ParseStatus::Parsed);
        assert!(v.value.is_some(), "parsed value serialized to JSON");
        assert!(v.error.is_none());
    }

    #[tokio::test]
    async fn flags_response_the_parser_rejects() {
        // A body with no GetHostnameResponse → find_response fails → parse error.
        let mut store = FixtureStore::new("clone");
        store.record(
            HOSTNAME,
            "<req/>",
            "<Envelope><Body><SomethingElse/></Body></Envelope>",
        );

        let report = store.verify_parsing().await;
        assert!(!report.all_parsed());
        let v = &report.verdicts[0];
        assert_eq!(v.status, ParseStatus::Failed);
        assert!(v.error.is_some(), "failure carries the parser error");
        assert!(v.value.is_none());
        assert_eq!(report.failures().count(), 1);
    }

    #[tokio::test]
    async fn unmapped_operation_is_unverified() {
        // A write op the sweep never records has no parser wired.
        let mut store = FixtureStore::new("clone");
        store.record(
            "http://www.onvif.org/ver10/device/wsdl/SetHostname",
            "<req/>",
            "<Envelope><Body><SetHostnameResponse/></Body></Envelope>",
        );

        let report = store.verify_parsing().await;
        assert_eq!(report.verdicts[0].status, ParseStatus::Unverified);
        assert!(report.all_parsed(), "unverified is not a failure");
    }
}
