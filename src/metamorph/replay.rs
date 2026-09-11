//! Persona B replay: answer reads from a recorded [`FixtureStore`], with coarse
//! copy-on-write so writes still round-trip through synthetic `DeviceState`.
//! Built-in devices use committed effects for profile creation/deletion and
//! modeled Media bindings, source configuration and encoder writes;
//! remaining mutations and standalone responder
//! construction retain the legacy policy.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

#[cfg(test)]
use crate::mock::canon::{Masking, canonicalize};
use crate::mock::effect::{Effect, EffectObserver, tracks_commit};
use crate::mock::fault_injection::FaultInjector;
use crate::mock::policy::{AckOnlyOperation, AckOnlyPolicy};
use crate::mock::responder::{Chain, RequestCtx, Responder};
use crate::mock::state::MockState;
use crate::transport::{Transport, TransportError};

use super::fixture::FixtureStore;

/// Base URL the replay device uses when it must emit absolute URLs.
const METAMORPH_BASE: &str = "http://metamorph";

/// Chain responder that answers reads from recorded fixtures.
///
/// Spliced just before the synthetic terminal (via `Chain::mock_with_extra`):
///
/// - A **read** (`Get*`) is answered from the fixture matching the canonical
///   (`Masking::Key`) request — unless its operation *family* has been
///   invalidated by a prior write, in which case it passes to synthetic.
///   A key hit additionally checks scoped XML identity against the recorded
///   request. Distinct scalar/namespace/structure values pass to synthetic.
///   Exact raw fixtures remain supported; otherwise malformed XML, mixed content
///   or unresolved `xsi:type` cannot establish equivalence. This is not complete
///   protocol validation. Colliding recorded requests remain separate and are
///   selected through [`FixtureStore::lookup_request`], never key-only lookup.
/// - A **write** (anything not `Get*`) always passes, so `SyntheticResponder`
///   applies it to `DeviceState`, and invalidates that family's replay — the
///   coarse copy-on-write of `docs/active/metamorph.md` D5, so `Set → Get` reflects the
///   new value.
///
/// The public constructor retains this standalone policy because it cannot
/// observe a caller's later responders. Built-in replay devices additionally
/// observe committed profile creation/deletion and Media binding effects: refusals retain recorded
/// reads, and success retires profile reads across both Media services. Source
/// configuration writes also retire their dependent source/profile/options reads
/// only after commit. Encoder writes likewise retire encoder/profile reads after
/// commit, preserving recordings on rate validation refusals. Other
/// mutations still require migration; this is not full dependency tracking.
/// Classified [`AckOnlyOperation`] requests never invalidate replay, including
/// through the standalone constructor: refusal or receipt is not a modeled effect.
pub struct ReplayResponder {
    store: Arc<FixtureStore>,
    invalidated: Arc<Mutex<HashSet<String>>>,
    track_commits: bool,
}

impl ReplayResponder {
    /// A responder over `store`, sharing the `invalidated` family set with the
    /// device so copy-on-write state persists across requests. This standalone
    /// constructor retains legacy pre-write invalidation; it does not observe
    /// whether a subsequent caller-owned responder commits a change. Classified
    /// [`AckOnlyOperation`] requests are the exception and never invalidate here.
    pub fn new(store: Arc<FixtureStore>, invalidated: Arc<Mutex<HashSet<String>>>) -> Self {
        Self {
            store,
            invalidated,
            track_commits: false,
        }
    }

    /// Built-in devices own the terminal and can observe committed effects.
    /// A standalone public responder has no access to a caller's later handlers.
    pub(crate) fn with_commit_tracking(mut self) -> Self {
        self.track_commits = true;
        self
    }

    pub(crate) fn effect_observer(&self) -> EffectObserver {
        let invalidated = self.invalidated.clone();
        Arc::new(move |effect| {
            match effect {
                Effect::AudioEncoderCommitted | Effect::MetadataCommitted => {
                    let mut retired = invalidated.lock().unwrap_or_else(|p| p.into_inner());
                    let actions: &[&str] = if effect == Effect::MetadataCommitted {
                        &["http://www.onvif.org/ver20/media/wsdl/GetMetadataConfigurations"]
                    } else {
                        &[
                            "http://www.onvif.org/ver10/media/wsdl/GetProfile",
                            "http://www.onvif.org/ver10/media/wsdl/GetProfiles",
                            "http://www.onvif.org/ver20/media/wsdl/GetProfiles",
                            "http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfiguration",
                            "http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfigurations",
                            "http://www.onvif.org/ver20/media/wsdl/GetAudioEncoderConfigurations",
                        ]
                    };
                    for action in actions {
                        retired.insert((*action).to_owned());
                    }
                }
                Effect::VideoEncoderCommitted => {
                    let mut retired = invalidated
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    for action in [
                        "http://www.onvif.org/ver10/media/wsdl/GetProfile",
                        "http://www.onvif.org/ver10/media/wsdl/GetProfiles",
                        "http://www.onvif.org/ver20/media/wsdl/GetProfiles",
                        "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfigurations",
                        "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfiguration",
                        "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurations",
                    ] {
                        retired.insert(action.to_owned());
                    }
                }
                Effect::VideoSourceChanged => {
                    let mut retired = invalidated
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    for action in [
                        "http://www.onvif.org/ver10/media/wsdl/GetProfile",
                        "http://www.onvif.org/ver10/media/wsdl/GetProfiles",
                        "http://www.onvif.org/ver20/media/wsdl/GetProfiles",
                        "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfigurations",
                        "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfiguration",
                        "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfigurationOptions",
                        "http://www.onvif.org/ver20/media/wsdl/GetVideoSourceConfigurations",
                        "http://www.onvif.org/ver20/media/wsdl/GetVideoSourceConfigurationOptions",
                        "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderInstances",
                    ] {
                        retired.insert(action.to_owned());
                    }
                }
                Effect::ProfilesChanged => {
                    let mut retired = invalidated
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    // Profile views and configuration reference counts share the
                    // committed profile collection. Conservatively retire each
                    // affected operation, never an unrelated service's suffix.
                    for action in [
                        "http://www.onvif.org/ver10/media/wsdl/GetProfile",
                        "http://www.onvif.org/ver10/media/wsdl/GetProfiles",
                        "http://www.onvif.org/ver20/media/wsdl/GetProfiles",
                        "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfigurations",
                        "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfiguration",
                        "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfigurations",
                        "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfigurationOptions",
                        "http://www.onvif.org/ver20/media/wsdl/GetVideoSourceConfigurationOptions",
                        "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfiguration",
                        "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfigurationOptions",
                        "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurationOptions",
                        "http://www.onvif.org/ver10/media/wsdl/GetAudioSourceConfigurations",
                        "http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfigurations",
                        "http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfiguration",
                        "http://www.onvif.org/ver20/media/wsdl/GetVideoSourceConfigurations",
                        "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurations",
                        "http://www.onvif.org/ver20/media/wsdl/GetAudioSourceConfigurations",
                        "http://www.onvif.org/ver20/media/wsdl/GetAudioEncoderConfigurations",
                        "http://www.onvif.org/ver20/ptz/wsdl/GetConfigurations",
                        "http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfigurationOptions",
                        "http://www.onvif.org/ver20/media/wsdl/GetAudioEncoderConfigurationOptions",
                        "http://www.onvif.org/ver20/media/wsdl/GetAudioOutputConfigurations",
                        "http://www.onvif.org/ver20/media/wsdl/GetAudioDecoderConfigurations",
                        "http://www.onvif.org/ver20/media/wsdl/GetMetadataConfigurations",
                        "http://www.onvif.org/ver20/media/wsdl/GetMetadataConfigurationOptions",
                        "http://www.onvif.org/ver20/ptz/wsdl/GetConfiguration",
                        "http://www.onvif.org/ver20/ptz/wsdl/GetCompatibleConfigurations",
                    ] {
                        retired.insert(action.to_owned());
                    }
                }
            }
        })
    }
}

#[async_trait]
impl Responder for ReplayResponder {
    async fn respond(&self, ctx: &RequestCtx<'_>) -> Option<String> {
        // A classified receipt is never a committed synthetic mutation. The
        // standalone responder also leaves these caller-owned effects alone.
        if AckOnlyOperation::for_action(ctx.action).is_some() {
            return None;
        }
        let op = operation(ctx.action);
        let fam = family(op);
        if is_write(op) {
            if self.track_commits && tracks_commit(ctx.action) {
                return None;
            }
            // Let synthetic apply the write to DeviceState; retire this family's
            // fixtures so subsequent reads see the mutated state.
            self.invalidated.lock().unwrap().insert(fam.to_string());
            return None;
        }
        let retired = self.invalidated.lock().unwrap();
        if retired.contains(fam) || retired.contains(ctx.action) {
            // A prior write moved this family to live DeviceState.
            return None;
        }
        drop(retired);
        self.store
            .lookup_request(ctx.action, ctx.body)
            .map(|f| f.response_raw.clone())
    }
}

/// The last path segment of a SOAP action — the operation name.
fn operation(action: &str) -> &str {
    action.rsplit('/').next().unwrap_or(action)
}

/// A read is a `Get*`; everything else (`Set*`, `Create*`, `Add*`, …) is a
/// write for copy-on-write purposes.
fn is_write(op: &str) -> bool {
    !op.starts_with("Get")
}

/// The read/write family of an operation: its name with a leading CRUD verb
/// stripped, so `GetHostname` and `SetHostname` share family `Hostname`.
fn family(op: &str) -> &str {
    const VERBS: &[&str] = &[
        "Get", "Set", "Create", "Delete", "Add", "Remove", "Start", "Stop", "Modify",
    ];
    VERBS
        .iter()
        .find_map(|v| op.strip_prefix(v).filter(|rest| !rest.is_empty()))
        .unwrap_or(op)
}

/// In-process replay device: a [`Transport`] whose chain answers reads from a
/// [`FixtureStore`] and falls back to synthetic `DeviceState` for writes and
/// unrecorded operations.
///
/// ```no_run
/// use std::sync::Arc;
/// use oxvif::OnvifClient;
/// use oxvif::metamorph::{FixtureStore, MetamorphTransport};
///
/// # fn run() -> std::io::Result<()> {
/// let store = FixtureStore::load("tests/fixtures/hikvision-ds2cd")?;
/// let client = OnvifClient::new("http://replay")
///     .with_transport(Arc::new(MetamorphTransport::new(store)));
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct MetamorphTransport {
    state: Arc<MockState>,
    faults: Arc<FaultInjector>,
    store: Arc<FixtureStore>,
    invalidated: Arc<Mutex<HashSet<String>>>,
    enforce_auth: bool,
    ack_only: AckOnlyPolicy,
}

impl MetamorphTransport {
    /// A replay device over `store`, with fresh synthetic state and auth off.
    pub fn new(store: FixtureStore) -> Self {
        Self {
            state: Arc::new(MockState::new()),
            faults: Arc::new(FaultInjector::new()),
            store: Arc::new(store),
            invalidated: Arc::new(Mutex::new(HashSet::new())),
            enforce_auth: false,
            ack_only: AckOnlyPolicy::default(),
        }
    }

    /// Seed the synthetic fallback state — writes and unrecorded reads use it.
    pub fn with_state(mut self, state: MockState) -> Self {
        self.state = Arc::new(state);
        self
    }

    /// Enforce WS-Security, mirroring
    /// [`MockTransport::with_auth`](crate::mock::MockTransport::with_auth).
    pub fn with_auth(mut self) -> Self {
        self.enforce_auth = true;
        self
    }

    /// Permit one synthetic acknowledgment-only response without invalidating replay.
    /// Selections accumulate and are copied independently by clones. See
    /// [`AckOnlyOperation`] for the operations and explicit lack of modeled effects.
    pub fn with_acknowledgment_only(mut self, operation: AckOnlyOperation) -> Self {
        self.ack_only.enable(operation);
        self
    }

    /// Access the synthetic fallback device state (seed before, assert after).
    pub fn device(&self) -> &MockState {
        &self.state
    }
}

#[async_trait]
impl Transport for MetamorphTransport {
    async fn soap_post(
        &self,
        _url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        // Replay sits between the auth gate and the synthetic terminal.
        let replay = ReplayResponder::new(self.store.clone(), self.invalidated.clone())
            .with_commit_tracking();
        let observer = replay.effect_observer();
        let chain = Chain::mock_with_observer(
            self.faults.clone(),
            self.enforce_auth,
            vec![Box::new(replay)],
            Some(observer),
            self.ack_only.clone(),
        );
        let ctx = RequestCtx {
            action,
            base: METAMORPH_BASE,
            body: &body,
            state: &self.state,
        };
        Ok(chain.respond(&ctx).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OnvifClient;
    use crate::metamorph::RecordingTransport;
    use crate::mock::MockTransport;

    #[test]
    fn family_pairs_set_and_get() {
        assert_eq!(family("GetHostname"), "Hostname");
        assert_eq!(family("SetHostname"), "Hostname");
        assert!(is_write("SetHostname"));
        assert!(!is_write("GetHostname"));
        // A verb-less op is its own family.
        assert_eq!(family("Probe"), "Probe");
    }

    #[test]
    fn operation_is_last_path_segment() {
        assert_eq!(
            operation("http://www.onvif.org/ver10/device/wsdl/GetHostname"),
            "GetHostname"
        );
    }

    #[tokio::test]
    async fn replay_reproduces_reads_and_cow_lets_writes_roundtrip() {
        // 1. "Real camera": a mock with a distinctive hostname so a replayed
        //    GetHostname can be told apart from the metamorph synthetic default.
        let real = MockTransport::new();
        real.device()
            .modify(|s| s.hostname = "real-camera-host".to_string());

        // 2. Record: drive a client through RecordingTransport into a store.
        let store = Arc::new(Mutex::new(FixtureStore::new("mock-real")));
        let rec = RecordingTransport::new(Arc::new(real), store.clone());
        let rc = OnvifClient::new("http://real").with_transport(Arc::new(rec));
        let orig_info = rc.get_device_info().await.unwrap();
        let orig_host = rc.get_hostname().await.unwrap();
        assert_eq!(orig_host.name.as_deref(), Some("real-camera-host"));

        let recorded = store.lock().unwrap().clone();
        assert!(recorded.len() >= 2, "both reads should be recorded");

        // 3. Replay: a MetamorphTransport over the recorded set.
        let meta = MetamorphTransport::new(recorded);
        let mc = OnvifClient::new("http://replay").with_transport(Arc::new(meta));

        // Reads reproduce the original device verbatim.
        let info = mc.get_device_info().await.unwrap();
        assert_eq!(info.manufacturer, orig_info.manufacturer);
        let host = mc.get_hostname().await.unwrap();
        assert_eq!(
            host.name.as_deref(),
            Some("real-camera-host"),
            "GetHostname must replay the recorded value, not the synthetic default"
        );

        // 4. Copy-on-write: after a Set, the Hostname family falls to live state.
        mc.set_hostname("changed-host").await.unwrap();
        let host2 = mc.get_hostname().await.unwrap();
        assert_eq!(
            host2.name.as_deref(),
            Some("changed-host"),
            "Set then Get must reflect the new value via synthetic COW"
        );
    }

    // ── NET 4: replay round-trip, at the responder level ──────────────────────

    const GET_HOSTNAME_ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetHostname";
    const GET_HOSTNAME_RESPONSE: &str = "<Envelope><Body><GetHostnameResponse>\
                                         <Name>recorded-host</Name>\
                                         </GetHostnameResponse></Body></Envelope>";

    fn hostname_request(message_id: &str) -> String {
        format!(
            "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' \
             xmlns:w='http://www.w3.org/2005/08/addressing' \
             xmlns:d='http://www.onvif.org/ver10/device/wsdl'>\
             <s:Header><w:MessageID>uuid:{message_id}</w:MessageID></s:Header>\
             <s:Body><d:GetHostname/></s:Body></s:Envelope>"
        )
    }

    /// A recorded response is replayed verbatim for a matching request, even
    /// when the live request carries a different `MessageID`; an unrecorded
    /// read passes through (so the synthetic terminal answers it).
    #[tokio::test]
    async fn replay_responder_returns_the_recorded_response_for_a_matching_request() {
        let mut store = FixtureStore::new("dev");
        store.record(
            GET_HOSTNAME_ACTION,
            &hostname_request("aaa"),
            GET_HOSTNAME_RESPONSE,
        );

        let responder = ReplayResponder::new(Arc::new(store), Arc::new(Mutex::new(HashSet::new())));
        let state = MockState::new();

        let live = hostname_request("bbb");
        let hit = RequestCtx {
            action: GET_HOSTNAME_ACTION,
            base: METAMORPH_BASE,
            body: &live,
            state: &state,
        };
        assert_eq!(
            responder.respond(&hit).await.as_deref(),
            Some(GET_HOSTNAME_RESPONSE)
        );

        let miss_body = "<Envelope><Body><GetDNS/></Body></Envelope>";
        let miss = RequestCtx {
            action: "http://www.onvif.org/ver10/device/wsdl/GetDNS",
            base: METAMORPH_BASE,
            body: miss_body,
            state: &state,
        };
        assert_eq!(
            responder.respond(&miss).await,
            None,
            "an unrecorded read must fall through to synthetic"
        );
    }

    /// Copy-on-write, at the responder level: a write in a family retires that
    /// family's fixtures, so the recorded read stops being replayed. Reads in
    /// other families keep replaying.
    #[tokio::test]
    async fn a_write_retires_only_its_own_family_from_replay() {
        const GET_DNS_ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetDNS";
        const GET_DNS_REQUEST: &str = "<Envelope><Body><GetDNS/></Body></Envelope>";
        const GET_DNS_RESPONSE: &str = "<Envelope><Body><GetDNSResponse/></Body></Envelope>";

        let mut store = FixtureStore::new("dev");
        store.record(
            GET_HOSTNAME_ACTION,
            &hostname_request("aaa"),
            GET_HOSTNAME_RESPONSE,
        );
        store.record(GET_DNS_ACTION, GET_DNS_REQUEST, GET_DNS_RESPONSE);

        let responder = ReplayResponder::new(Arc::new(store), Arc::new(Mutex::new(HashSet::new())));
        let state = MockState::new();

        let read = hostname_request("aaa");
        let ctx = RequestCtx {
            action: GET_HOSTNAME_ACTION,
            base: METAMORPH_BASE,
            body: &read,
            state: &state,
        };
        assert_eq!(
            responder.respond(&ctx).await.as_deref(),
            Some(GET_HOSTNAME_RESPONSE)
        );

        // A SetHostname passes through (returns None) and invalidates Hostname.
        let write = RequestCtx {
            action: "http://www.onvif.org/ver10/device/wsdl/SetHostname",
            base: METAMORPH_BASE,
            body: "<Envelope><Body><SetHostname><Name>x</Name></SetHostname></Body></Envelope>",
            state: &state,
        };
        assert_eq!(responder.respond(&write).await, None);

        assert_eq!(
            responder.respond(&ctx).await,
            None,
            "after a write the Hostname family must come from live state"
        );

        // A different family is untouched.
        let dns = RequestCtx {
            action: GET_DNS_ACTION,
            base: METAMORPH_BASE,
            body: GET_DNS_REQUEST,
            state: &state,
        };
        assert_eq!(
            responder.respond(&dns).await.as_deref(),
            Some(GET_DNS_RESPONSE)
        );
    }

    // ── D1: replay must not cross two services onto one canonical key ─────────

    const MEDIA1_GET_PROFILES: &str = "http://www.onvif.org/ver10/media/wsdl/GetProfiles";
    const MEDIA2_GET_PROFILES: &str = "http://www.onvif.org/ver20/media/wsdl/GetProfiles";

    const MEDIA1_PROFILES_REQ: &str = "<Envelope><Header><To>http://cam/onvif/Media</To></Header>\
                                       <Body><trt:GetProfiles/></Body></Envelope>";
    const MEDIA2_PROFILES_REQ: &str = "<Envelope><Header><To>http://cam/onvif/Media2</To></Header>\
                                       <Body><tr2:GetProfiles/></Body></Envelope>";

    const MEDIA1_PROFILES_RESP: &str = "<Envelope><Body><trt:GetProfilesResponse>\
                                        <Profiles token=\"media1-profile\"/>\
                                        </trt:GetProfilesResponse></Body></Envelope>";
    const MEDIA2_PROFILES_RESP: &str = "<Envelope><Body><tr2:GetProfilesResponse>\
                                        <Profiles token=\"media2-profile\"/>\
                                        </tr2:GetProfilesResponse></Body></Envelope>";

    /// The defect that matters most: a Media1 read whose canonical body is
    /// identical to Media2's must replay the *Media1* envelope. Answering with
    /// Media2's parses successfully and returns wrong data, with the sweep still
    /// reporting `Recorded`.
    #[tokio::test]
    async fn replay_answers_each_service_with_its_own_recorded_envelope() {
        // Premise: these two requests really do share one canonical key.
        assert_eq!(
            canonicalize(MEDIA1_PROFILES_REQ, Masking::Key),
            canonicalize(MEDIA2_PROFILES_REQ, Masking::Key),
        );

        let mut store = FixtureStore::new("dev");
        store.record(
            MEDIA1_GET_PROFILES,
            MEDIA1_PROFILES_REQ,
            MEDIA1_PROFILES_RESP,
        );
        store.record(
            MEDIA2_GET_PROFILES,
            MEDIA2_PROFILES_REQ,
            MEDIA2_PROFILES_RESP,
        );

        let responder = ReplayResponder::new(Arc::new(store), Arc::new(Mutex::new(HashSet::new())));
        let state = MockState::new();

        let media1 = RequestCtx {
            action: MEDIA1_GET_PROFILES,
            base: METAMORPH_BASE,
            body: MEDIA1_PROFILES_REQ,
            state: &state,
        };
        assert_eq!(
            responder.respond(&media1).await.as_deref(),
            Some(MEDIA1_PROFILES_RESP),
            "a Media1 read must not be answered from the Media2 exchange"
        );

        let media2 = RequestCtx {
            action: MEDIA2_GET_PROFILES,
            base: METAMORPH_BASE,
            body: MEDIA2_PROFILES_REQ,
            state: &state,
        };
        assert_eq!(
            responder.respond(&media2).await.as_deref(),
            Some(MEDIA2_PROFILES_RESP),
            "a Media2 read must not be answered from the Media1 exchange"
        );
    }
}
