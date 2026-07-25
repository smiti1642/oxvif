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
use crate::soap::{SoapError, find_response, parse_soap_body};
use crate::transport::{Transport, TransportError};

use super::fixture::{FixtureProgress, FixtureStore};

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
    /// oxvif's parser rejected it (malformed body, missing required field, or an
    /// out-of-range value / enum) — an interop quirk.
    ///
    /// A device-returned SOAP Fault is **not** counted here; see
    /// [`ParseStatus::Faulted`].
    Failed,
    /// The device answered with a well-formed SOAP `<Fault>` — it *declined* the
    /// operation (`NotAuthorized`, `InvalidArgs`, `ActionNotSupported`, …).
    ///
    /// This is correct device behaviour, not an interop quirk: nothing is wrong
    /// with the device's encoding and nothing is wrong with oxvif's parser. It is
    /// kept apart from [`ParseStatus::Failed`] so that sweeping a camera with a
    /// restricted account does not flood the failure list with non-problems.
    /// [`ParseVerdict::error`] carries the fault's code and reason so a UI can
    /// show *why* the device declined.
    ///
    /// A fault reaches this stage because `HttpTransport` returns `Ok(body)` for
    /// HTTP 400 and 500 — exactly how ONVIF devices carry SOAP Faults — so the
    /// fault body is recorded into the clone like any other response.
    Faulted,
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
    /// The parser error, when `status` is [`ParseStatus::Failed`]; the device's
    /// fault code and reason, when `status` is [`ParseStatus::Faulted`].
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
    ///
    /// Strictly [`ParseStatus::Failed`]: "oxvif choked on this". Operations the
    /// device *declined* with a SOAP Fault are **not** included — they are
    /// [`ParseStatus::Faulted`] and are listed by [`Self::faulted`] instead.
    pub fn failures(&self) -> impl Iterator<Item = &ParseVerdict> {
        self.verdicts
            .iter()
            .filter(|v| v.status == ParseStatus::Failed)
    }

    /// The operations the device declined with a well-formed SOAP Fault — a
    /// restricted account, an unsupported command, a rejected argument. Correct
    /// device behaviour, reported separately from [`Self::failures`] so a UI can
    /// surface "the device said no" apart from "oxvif cannot parse this".
    pub fn faulted(&self) -> impl Iterator<Item = &ParseVerdict> {
        self.verdicts
            .iter()
            .filter(|v| v.status == ParseStatus::Faulted)
    }

    /// Whether nothing oxvif choked on (no [`ParseStatus::Failed`] verdict).
    ///
    /// [`ParseStatus::Unverified`] and [`ParseStatus::Faulted`] do not make this
    /// false: neither says anything about oxvif's ability to parse the device.
    pub fn all_parsed(&self) -> bool {
        self.verdicts
            .iter()
            .all(|v| v.status != ParseStatus::Failed)
    }

    /// Serialise the report as a compact single-line JSON string. Extracted
    /// values are embedded as-is (they are already `serde_json::Value`).
    pub fn to_json(&self) -> String {
        // serde_json on the fully-serializable ParseReport — infallible.
        serde_json::to_string(self).expect("ParseReport is fully serializable")
    }

    /// Serialise the report as pretty-printed JSON (indented, line-separated).
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("ParseReport is fully serializable")
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
        self.verify_parsing_with_progress(|_| {}).await
    }

    /// [`Self::verify_parsing`], reporting progress as it goes.
    ///
    /// `progress` is invoked **once per recorded fixture**, immediately after
    /// that fixture's verdict is computed, with
    /// [`done`](FixtureProgress::done) counting fixtures *completed* — so the
    /// first call carries `done == 1` and the last `done == total`. `total` is
    /// [`FixtureStore::len`], known before any work starts, so a determinate
    /// progress bar is possible.
    ///
    /// The callback is `Fn + Send + Sync` so it can be a closure that pushes
    /// into a channel shared with a UI thread, and so the returned future stays
    /// `Send`.
    ///
    /// ```no_run
    /// # async fn run() -> std::io::Result<()> {
    /// use oxvif::metamorph::FixtureStore;
    /// let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    /// let store = FixtureStore::load("clones/hikvision-ds2cd")?;
    /// let report = store
    ///     .verify_parsing_with_progress(move |p| {
    ///         let _ = tx.send(p);
    ///     })
    ///     .await;
    /// # let _ = report;
    /// # Ok(()) }
    /// ```
    pub async fn verify_parsing_with_progress(
        &self,
        progress: impl Fn(FixtureProgress) + Send + Sync,
    ) -> ParseReport {
        let total = self.fixtures().len();
        let mut verdicts = Vec::new();
        for (i, f) in self.fixtures().iter().enumerate() {
            let (status, error, value) = parse_check(&f.action, &f.response_raw).await;
            verdicts.push(ParseVerdict {
                action: f.action.clone(),
                key_canon: f.key_canon.clone(),
                status,
                error,
                value,
            });
            progress(FixtureProgress {
                action: f.action.clone(),
                key_canon: f.key_canon.clone(),
                done: i + 1,
                total,
            });
        }
        ParseReport {
            device: self.device().to_string(),
            verdicts,
        }
    }
}

/// Detect a device-returned SOAP Fault in `response`.
///
/// Matches on the `<Fault>` **element** (local name, so any namespace prefix
/// works) as a direct child of the SOAP `Body` — a response that merely contains
/// the word "Fault" in some text node is never mistaken for one. Extraction of
/// the SOAP 1.1 / 1.2 code and reason is delegated to
/// [`find_response`], which turns exactly that element into
/// [`SoapError::Fault`]; the expected tag passed to it is irrelevant because its
/// fault check runs first.
fn recorded_fault(response: &str) -> Option<SoapError> {
    let body = parse_soap_body(response).ok()?;
    body.child("Fault")?;
    match find_response(&body, "Fault") {
        Err(e @ SoapError::Fault { .. }) => Some(e),
        _ => None,
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
///
/// A recorded SOAP Fault short-circuits **before** the typed parser runs and
/// yields [`ParseStatus::Faulted`] — the device declined the operation, which is
/// not an oxvif parse problem. This check is deliberately ahead of the
/// action-to-parser routing too: "the device said no" is true whether or not
/// oxvif happens to have a parser wired for that operation.
async fn parse_check(
    action: &str,
    response: &str,
) -> (ParseStatus, Option<String>, Option<serde_json::Value>) {
    if let Some(fault) = recorded_fault(response) {
        return (ParseStatus::Faulted, Some(fault.to_string()), None);
    }

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
    const USERS: &str = "http://www.onvif.org/ver10/device/wsdl/GetUsers";

    /// A SOAP 1.2 Fault envelope, as a device carries one over HTTP 400/500.
    fn soap_fault(code: &str, reason: &str) -> String {
        format!(
            r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
                 <s:Body>
                   <s:Fault>
                     <s:Code><s:Value>{code}</s:Value>
                       <s:Subcode><s:Value>ter:NotAuthorized</s:Value></s:Subcode>
                     </s:Code>
                     <s:Reason><s:Text xml:lang="en">{reason}</s:Text></s:Reason>
                   </s:Fault>
                 </s:Body>
               </s:Envelope>"#
        )
    }

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

    /// A device that *declines* an operation is not an oxvif parse problem.
    /// `HttpTransport` hands HTTP 400/500 bodies back as `Ok`, so a fault is
    /// recorded into the clone like any other response and reaches `parse_check`.
    #[tokio::test]
    async fn recorded_soap_fault_is_faulted_not_failed() {
        let mut store = FixtureStore::new("clone");
        store.record(
            USERS,
            "<Envelope><Body><GetUsers/></Body></Envelope>",
            &soap_fault("s:Sender", "Sender not Authorized"),
        );

        let report = store.verify_parsing().await;
        let v = &report.verdicts[0];
        assert_eq!(
            v.status,
            ParseStatus::Faulted,
            "a well-formed device fault is Faulted, never Failed: {v:?}"
        );
        let err = v.error.as_deref().expect("fault carries its reason");
        assert!(err.contains("Sender not Authorized"), "reason text: {err}");
        assert!(err.contains("s:Sender"), "fault code: {err}");
        assert!(v.value.is_none());

        // `failures()` keeps meaning "oxvif choked"; `faulted()` is the separate
        // "the device said no" list; `all_parsed()` stays true.
        assert_eq!(report.failures().count(), 0);
        assert_eq!(report.faulted().count(), 1);
        assert!(
            report.all_parsed(),
            "a declined operation must not read as an oxvif parse failure"
        );
    }

    /// The fault check matches the `<Fault>` *element*, so a response whose text
    /// merely mentions the word parses normally.
    #[tokio::test]
    async fn fault_word_in_element_text_is_not_a_fault() {
        let mut store = FixtureStore::new("clone");
        store.record(
            HOSTNAME,
            "<Envelope><Body><GetHostname/></Body></Envelope>",
            "<Envelope><Body><GetHostnameResponse><HostnameInformation>\
             <FromDHCP>false</FromDHCP><Name>Fault</Name>\
             </HostnameInformation></GetHostnameResponse></Body></Envelope>",
        );

        let report = store.verify_parsing().await;
        assert_eq!(
            report.verdicts[0].status,
            ParseStatus::Parsed,
            "'Fault' as element text must not trip the fault check: {:?}",
            report.verdicts[0]
        );
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

    /// A report covering all three verdict kinds, built through the public API.
    ///
    /// Each fixture needs its own `request_raw`: [`FixtureStore::record`] derives
    /// the key from the request body alone (the action is not an input) and
    /// upserts, so sharing one placeholder body would collapse all three into a
    /// single entry. Real recordings carry the operation element, which is what
    /// keeps them distinct here too.
    async fn mixed_report() -> ParseReport {
        let mut store = FixtureStore::new("clone");
        store.record(
            HOSTNAME,
            "<Envelope><Body><GetHostname/></Body></Envelope>",
            &valid_hostname_response(),
        );
        store.record(
            "http://www.onvif.org/ver10/device/wsdl/GetScopes",
            "<Envelope><Body><GetScopes/></Body></Envelope>",
            "<Envelope><Body><SomethingElse/></Body></Envelope>",
        );
        store.record(
            "http://www.onvif.org/ver10/device/wsdl/SetHostname",
            "<Envelope><Body><SetHostname><Name>cam</Name></SetHostname></Body></Envelope>",
            "<Envelope><Body><SetHostnameResponse/></Body></Envelope>",
        );
        store.record(
            USERS,
            "<Envelope><Body><GetUsers/></Body></Envelope>",
            &soap_fault("s:Sender", "Sender not Authorized"),
        );
        assert_eq!(store.len(), 4, "each fixture must survive the key upsert");

        let report = store.verify_parsing().await;
        assert_eq!(report.verdicts.len(), 4, "one verdict per stored fixture");

        // Enforce the claim above. Without this, a change to the parse match
        // arms could quietly collapse every verdict to `Unverified` and the
        // round-trip tests would still pass — no longer covering `Some`/`None`
        // for either `error` or `value`.
        for want in [
            ParseStatus::Parsed,
            ParseStatus::Failed,
            ParseStatus::Faulted,
            ParseStatus::Unverified,
        ] {
            assert!(
                report.verdicts.iter().any(|v| v.status == want),
                "fixture no longer produces a {want:?} verdict: {:?}",
                report.verdicts
            );
        }
        report
    }

    #[tokio::test]
    async fn to_json_round_trips() {
        let report = mixed_report().await;
        let json = report.to_json();

        let back: ParseReport = serde_json::from_str(&json).expect("to_json emits valid JSON");
        assert_eq!(back.device, report.device);
        assert_eq!(back.verdicts.len(), report.verdicts.len());
        for (a, b) in back.verdicts.iter().zip(&report.verdicts) {
            assert_eq!(a.action, b.action);
            assert_eq!(a.key_canon, b.key_canon);
            assert_eq!(a.status, b.status);
            assert_eq!(a.error, b.error);
            assert_eq!(a.value, b.value);
        }
        // Re-serialising the round-tripped report reproduces the same bytes.
        assert_eq!(back.to_json(), json);
    }

    #[tokio::test]
    async fn to_json_pretty_is_indented() {
        let report = mixed_report().await;
        let compact = report.to_json();
        let pretty = report.to_json_pretty();

        assert_ne!(pretty, compact);
        assert!(pretty.contains('\n'), "pretty JSON is line-separated");
        assert!(pretty.contains("\n  \""), "pretty JSON is indented");
        assert!(!compact.contains('\n'), "compact JSON is single-line");
        // Both encode the same document.
        let a: serde_json::Value = serde_json::from_str(&compact).unwrap();
        let b: serde_json::Value = serde_json::from_str(&pretty).unwrap();
        assert_eq!(a, b);
    }

    // ── progress ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn progress_fires_once_per_fixture_and_ends_at_total() {
        let mut store = FixtureStore::new("clone");
        store.record(
            HOSTNAME,
            "<Envelope><Body><GetHostname/></Body></Envelope>",
            &valid_hostname_response(),
        );
        store.record(
            "http://www.onvif.org/ver10/device/wsdl/GetScopes",
            "<Envelope><Body><GetScopes/></Body></Envelope>",
            "<Envelope><Body><SomethingElse/></Body></Envelope>",
        );
        store.record(
            USERS,
            "<Envelope><Body><GetUsers/></Body></Envelope>",
            &soap_fault("s:Sender", "Sender not Authorized"),
        );

        let seen = std::sync::Mutex::new(Vec::new());
        let report = store
            .verify_parsing_with_progress(|p| seen.lock().unwrap().push(p))
            .await;
        let seen = seen.into_inner().unwrap();

        assert_eq!(seen.len(), store.len(), "one event per recorded fixture");
        assert!(
            seen.iter().all(|p| p.total == store.len()),
            "total is the store size for every event: {seen:?}"
        );
        let dones: Vec<usize> = seen.iter().map(|p| p.done).collect();
        assert_eq!(dones, vec![1, 2, 3], "done counts completed fixtures");
        assert_eq!(
            seen.last().map(|p| p.done),
            Some(seen.len()),
            "the pass ends at done == total"
        );
        // Events identify the fixture by the same key the verdicts carry.
        let keys: Vec<&str> = seen.iter().map(|p| p.key_canon.as_str()).collect();
        let verdict_keys: Vec<&str> = report
            .verdicts
            .iter()
            .map(|v| v.key_canon.as_str())
            .collect();
        assert_eq!(keys, verdict_keys);
    }

    /// The Dioxus-desktop shape: the callback is a closure that sends into a
    /// `tokio::sync::mpsc::UnboundedSender`, and the future stays `Send` so it
    /// can be spawned. Compile-checked, not assumed.
    #[tokio::test]
    async fn progress_callback_accepts_an_mpsc_sender() {
        fn assert_send<T: Send>(t: T) -> T {
            t
        }

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<FixtureProgress>();
        let mut store = FixtureStore::new("clone");
        store.record(
            HOSTNAME,
            "<Envelope><Body><GetHostname/></Body></Envelope>",
            &valid_hostname_response(),
        );

        let report = assert_send(store.verify_parsing_with_progress(move |p| {
            let _ = tx.send(p);
        }))
        .await;
        assert_eq!(report.verdicts.len(), 1);

        let got = rx.recv().await.expect("one progress event");
        assert_eq!((got.done, got.total), (1, 1));
        assert!(rx.recv().await.is_none(), "sender dropped with the future");
    }
}
