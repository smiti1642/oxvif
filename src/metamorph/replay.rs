//! Persona B replay: answer reads from a recorded [`FixtureStore`], with coarse
//! copy-on-write so writes still round-trip through synthetic `DeviceState`.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::mock::canon::{Masking, canonicalize};
use crate::mock::fault_injection::FaultInjector;
use crate::mock::responder::{Chain, RequestCtx, Responder};
use crate::mock::state::MockState;
use crate::transport::{Transport, TransportError};

use super::fixture::FixtureStore;

/// Base URL the replay device uses when it must emit absolute URLs.
const METAMORPH_BASE: &str = "http://metamorph";

/// Chain responder that answers reads from recorded fixtures.
///
/// Spliced just before the synthetic terminal (via [`Chain::mock_with_extra`]):
///
/// - A **read** (`Get*`) is answered from the fixture matching the canonical
///   ([`Masking::Key`]) request — unless its operation *family* has been
///   invalidated by a prior write, in which case it passes to synthetic.
/// - A **write** (anything not `Get*`) always passes, so `SyntheticResponder`
///   applies it to `DeviceState`, and invalidates that family's replay — the
///   coarse copy-on-write of `docs/active/metamorph.md` D5, so `Set → Get` reflects the
///   new value.
pub struct ReplayResponder {
    store: Arc<FixtureStore>,
    invalidated: Arc<Mutex<HashSet<String>>>,
}

impl ReplayResponder {
    /// A responder over `store`, sharing the `invalidated` family set with the
    /// device so copy-on-write state persists across requests.
    pub fn new(store: Arc<FixtureStore>, invalidated: Arc<Mutex<HashSet<String>>>) -> Self {
        Self { store, invalidated }
    }
}

#[async_trait]
impl Responder for ReplayResponder {
    async fn respond(&self, ctx: &RequestCtx<'_>) -> Option<String> {
        let op = operation(ctx.action);
        let fam = family(op);
        if is_write(op) {
            // Let synthetic apply the write to DeviceState; retire this family's
            // fixtures so subsequent reads see the mutated state.
            self.invalidated.lock().unwrap().insert(fam.to_string());
            return None;
        }
        if self.invalidated.lock().unwrap().contains(fam) {
            // A prior write moved this family to live DeviceState.
            return None;
        }
        let key = canonicalize(ctx.body, Masking::Key);
        self.store.lookup(&key).map(|f| f.response_raw.clone())
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
        let replay = ReplayResponder::new(self.store.clone(), self.invalidated.clone());
        let chain = Chain::mock_with_extra(
            self.faults.clone(),
            self.enforce_auth,
            vec![Box::new(replay)],
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
}
