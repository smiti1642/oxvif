//! Bound-port HTTP mock server (`feature = "mock-server"`).
//!
//! Wraps the same dispatcher/state as [`MockTransport`](crate::mock::MockTransport)
//! in an axum server on a real TCP port — for when a test (or another process,
//! or a non-Rust client) needs an actual HTTP endpoint. The server runs on a
//! background task and shuts down gracefully when the [`MockServer`] is dropped.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::discovery::{DiscoveredDevice, new_uuid};
use crate::mock::discovery_responder::DiscoveryResponder;
use crate::mock::fault_injection::{FaultInjector, PendingFault};
use crate::mock::responder::{Chain, RequestCtx};
use crate::mock::state::{ChangeHook, DeviceState, MockState};
use crate::mock::{helpers, snapshot};

const SOAP_CT: &str = "application/soap+xml; charset=utf-8";

/// Shared server context handed to every axum handler.
struct Ctx {
    base: String,
    state: MockState,
    faults: Arc<FaultInjector>,
    enforce_auth: bool,
    /// A loaded camera clone to replay reads from (feature `metamorph`); `None`
    /// for a plain synthetic mock.
    #[cfg(feature = "metamorph")]
    replay: Option<ReplayHandle>,
}

/// Replay state for a cloned camera served over the bound port: the recorded
/// fixtures plus the shared copy-on-write invalidation set, so a `Set` still
/// retires that operation family's fixtures and `Set → Get` round-trips.
#[cfg(feature = "metamorph")]
struct ReplayHandle {
    store: Arc<crate::metamorph::FixtureStore>,
    invalidated: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
}

/// Builder for [`MockServer`].
#[derive(Default)]
pub struct MockServerBuilder {
    port: u16,
    initial_state: Option<DeviceState>,
    on_change: Option<ChangeHook>,
    enforce_auth: bool,
    discoverable: Option<Vec<String>>,
    #[cfg(feature = "metamorph")]
    replay: Option<crate::metamorph::FixtureStore>,
}

impl MockServerBuilder {
    /// TCP port to bind. `0` (the default) picks an ephemeral free port.
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Seed the device with a caller-supplied state (e.g. loaded from disk).
    pub fn initial_state(mut self, state: DeviceState) -> Self {
        self.initial_state = Some(state);
        self
    }

    /// Persistence hook fired after every mutation — the seam for the caller to
    /// write state to disk. The server itself never touches the filesystem.
    pub fn on_change(mut self, hook: ChangeHook) -> Self {
        self.on_change = Some(hook);
        self
    }

    /// Enforce WS-Security `PasswordDigest` (default `false`). With it off, a
    /// credential-less client works out of the box.
    pub fn enforce_auth(mut self, yes: bool) -> Self {
        self.enforce_auth = yes;
        self
    }

    /// Make the server answer WS-Discovery `Probe`s so a client (oxdm, ODM,
    /// Frigate) finds it on the LAN, advertising the given ONVIF `scopes`
    /// (e.g. `onvif://www.onvif.org/name/MockCam`). Off by default.
    ///
    /// Best-effort: this binds the shared UDP port `3702` and joins the ONVIF
    /// multicast group — if the bind fails (port already in use, sandboxed CI)
    /// the HTTP server still starts, just undiscoverable. At most one
    /// discoverable server can run per host.
    pub fn discoverable(mut self, scopes: Vec<String>) -> Self {
        self.discoverable = Some(scopes);
        self
    }

    /// Serve a recorded camera clone (the "container"): reads replay the
    /// recorded responses — quirks and all — from `store`, while writes and
    /// unrecorded operations fall to synthetic `DeviceState` with coarse
    /// copy-on-write, so `Set → Get` still round-trips. Any HTTP ONVIF client
    /// (oxdm, Frigate, ODM) can then drive the clone at [`device_url`].
    ///
    /// Requires the `metamorph` feature (this method exists under
    /// `metamorph-server` = `metamorph` + `mock-server`).
    ///
    /// [`device_url`]: MockServer::device_url
    #[cfg(feature = "metamorph")]
    pub fn replay(mut self, store: crate::metamorph::FixtureStore) -> Self {
        self.replay = Some(store);
        self
    }

    /// Bind the socket and spawn the server on a background task.
    pub async fn start(self) -> std::io::Result<MockServer> {
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], self.port))).await?;
        let local = listener.local_addr()?;
        let base = format!("http://{local}");

        let mut state = match self.initial_state {
            Some(s) => MockState::with_state(s),
            None => MockState::new(),
        };
        if let Some(hook) = self.on_change {
            state.set_on_change(hook);
        }

        let ctx = Arc::new(Ctx {
            base: base.clone(),
            state,
            faults: Arc::new(FaultInjector::new()),
            enforce_auth: self.enforce_auth,
            #[cfg(feature = "metamorph")]
            replay: self.replay.map(|store| ReplayHandle {
                store: Arc::new(store),
                invalidated: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            }),
        });

        let app = Router::new()
            .route("/mock/snapshot.jpg", get(handle_snapshot))
            .route(
                "/mock/digital-input/{token}/pulse",
                post(handle_digital_input_pulse),
            )
            .route(
                "/mock/digital-input/{token}/set",
                post(handle_digital_input_set),
            )
            .route("/admin/inject_fault", post(handle_inject_fault))
            .route("/admin/clear_faults", post(handle_clear_faults))
            .route("/{*path}", post(handle_soap))
            .with_state(ctx.clone());

        let (tx, rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = rx.await;
                })
                .await;
        });

        let device_url = format!("{base}/onvif/device");

        // Opt-in WS-Discovery. Best-effort: a failed :3702 bind must not sink
        // the HTTP server, so log and carry on undiscoverable.
        let discovery = match self.discoverable {
            Some(scopes) => {
                let dev = DiscoveredDevice {
                    endpoint: format!("urn:uuid:{}", new_uuid()),
                    types: vec![
                        "dn:NetworkVideoTransmitter".to_string(),
                        "tds:Device".to_string(),
                    ],
                    scopes,
                    xaddrs: vec![device_url.clone()],
                };
                match DiscoveryResponder::spawn("0.0.0.0:3702", true, dev).await {
                    Ok(r) => Some(r),
                    Err(e) => {
                        tracing::warn!("mock discovery responder failed to bind :3702: {e}");
                        None
                    }
                }
            }
            None => None,
        };

        Ok(MockServer {
            device_url,
            base,
            port: local.port(),
            ctx,
            shutdown: Some(tx),
            _discovery: discovery,
        })
    }
}

/// A bound-port mock ONVIF server. Shuts down on drop.
///
/// ```no_run
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let server = oxvif::mock::MockServer::start().await?;
/// let client = oxvif::OnvifClient::new(server.device_url());
/// let info = client.get_device_info().await?;
/// assert_eq!(info.manufacturer, "oxvif-mock");
/// # Ok(()) }
/// ```
pub struct MockServer {
    device_url: String,
    base: String,
    port: u16,
    ctx: Arc<Ctx>,
    shutdown: Option<oneshot::Sender<()>>,
    /// Kept alive so the WS-Discovery responder (if any) shuts down with the
    /// server. `None` unless [`MockServerBuilder::discoverable`] was set.
    _discovery: Option<DiscoveryResponder>,
}

impl MockServer {
    /// Start a server on an ephemeral port with defaults (no auth, no persistence).
    pub async fn start() -> std::io::Result<Self> {
        MockServerBuilder::default().start().await
    }

    /// Configure a server before starting it.
    pub fn builder() -> MockServerBuilder {
        MockServerBuilder::default()
    }

    /// Device service URL — pass to [`OnvifClient::new`](crate::OnvifClient::new).
    pub fn device_url(&self) -> &str {
        &self.device_url
    }

    /// Base URL (`http://127.0.0.1:<port>`).
    pub fn base_url(&self) -> &str {
        &self.base
    }

    /// The bound port.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Access the shared device state (seed / assert).
    pub fn device(&self) -> &MockState {
        &self.ctx.state
    }

    /// Arm a single-shot SOAP Fault for the next matching action.
    pub fn inject_fault(
        &self,
        action_suffix: impl Into<String>,
        code: impl Into<String>,
        reason: impl Into<String>,
    ) {
        self.ctx.faults.inject(PendingFault {
            action_suffix: action_suffix.into(),
            code: code.into(),
            reason: reason.into(),
        });
    }

    /// Drop every queued fault.
    pub fn clear_faults(&self) {
        self.ctx.faults.clear_all();
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

async fn handle_soap(
    State(ctx): State<Arc<Ctx>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let action = helpers::extract_action(&headers).unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body);

    // Default pipeline: armed fault → auth gate → synthetic dispatch. With a
    // replay clone loaded, the replay responder is spliced in just ahead of the
    // synthetic terminal, so recorded reads win and writes still land in state.
    #[cfg(feature = "metamorph")]
    let chain = match &ctx.replay {
        Some(rp) => {
            let replay =
                crate::metamorph::ReplayResponder::new(rp.store.clone(), rp.invalidated.clone());
            Chain::mock_with_extra(ctx.faults.clone(), ctx.enforce_auth, vec![Box::new(replay)])
        }
        None => Chain::default_mock(ctx.faults.clone(), ctx.enforce_auth),
    };
    #[cfg(not(feature = "metamorph"))]
    let chain = Chain::default_mock(ctx.faults.clone(), ctx.enforce_auth);
    let rctx = RequestCtx {
        action: &action,
        base: &ctx.base,
        body: &body_str,
        state: &ctx.state,
    };
    let xml = chain.respond(&rctx).await;
    (StatusCode::OK, [(header::CONTENT_TYPE, SOAP_CT)], xml)
}

async fn handle_snapshot(State(ctx): State<Arc<Ctx>>) -> impl IntoResponse {
    let bmp = snapshot::generate_test_bmp(&ctx.state);
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "image/bmp"),
            (header::CACHE_CONTROL, "no-cache, no-store"),
        ],
        bmp,
    )
}

/// `POST /admin/inject_fault?action=<suffix>&code=<faultcode>&reason=<text>` —
/// test-only helper for non-Rust harnesses (Rust callers use
/// [`MockServer::inject_fault`]). No auth: the server binds to 127.0.0.1.
async fn handle_inject_fault(
    State(ctx): State<Arc<Ctx>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let action_suffix = params.get("action").cloned().unwrap_or_default();
    if action_suffix.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            "missing required 'action' query parameter\n".to_string(),
        );
    }
    let code = params
        .get("code")
        .cloned()
        .unwrap_or_else(|| "s:Receiver".to_string());
    let reason = params
        .get("reason")
        .cloned()
        .unwrap_or_else(|| "Injected fault".to_string());
    ctx.faults.inject(PendingFault {
        action_suffix,
        code,
        reason,
    });
    (StatusCode::OK, "fault injected\n".to_string())
}

/// `POST /admin/clear_faults` — drop every queued fault.
async fn handle_clear_faults(State(ctx): State<Arc<Ctx>>) -> impl IntoResponse {
    ctx.faults.clear_all();
    (StatusCode::OK, "faults cleared\n".to_string())
}

/// `POST /mock/digital-input/:token/pulse` — flip the input to `active`
/// (queueing a Trigger/DigitalInput event), then immediately flip it back
/// to `inactive` (queueing the trailing event). The two events surface
/// on the next two `PullMessages` calls. 404 on unknown token.
///
/// Synchronous (no sleep) so tests can poll deterministically.
async fn handle_digital_input_pulse(
    State(ctx): State<Arc<Ctx>>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    let exists = ctx
        .state
        .read()
        .digital_inputs
        .iter()
        .any(|d| d.token == token);
    if !exists {
        return (
            StatusCode::NOT_FOUND,
            format!("unknown digital input: {token}\n"),
        );
    }
    ctx.state.modify(|s| {
        if let Some(d) = s.digital_inputs.iter_mut().find(|d| d.token == token) {
            d.logical_state = "active".into();
        }
        s.pending_io_events
            .push(crate::mock::state::PendingIoEvent {
                kind: "DigitalInput",
                token: token.clone(),
                logical_state: "active".into(),
            });
        if let Some(d) = s.digital_inputs.iter_mut().find(|d| d.token == token) {
            d.logical_state = "inactive".into();
        }
        s.pending_io_events
            .push(crate::mock::state::PendingIoEvent {
                kind: "DigitalInput",
                token: token.clone(),
                logical_state: "inactive".into(),
            });
    });
    (StatusCode::OK, "pulsed\n".to_string())
}

/// `POST /mock/digital-input/:token/set?state=active|inactive` — set
/// the logical state explicitly (no auto-revert) and queue a single
/// Trigger/DigitalInput event. 400 on missing/invalid state, 404 on
/// unknown token.
async fn handle_digital_input_set(
    State(ctx): State<Arc<Ctx>>,
    Path(token): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let state_param = params.get("state").cloned().unwrap_or_default();
    if state_param != "active" && state_param != "inactive" {
        return (
            StatusCode::BAD_REQUEST,
            "expected ?state=active or ?state=inactive\n".to_string(),
        );
    }
    let exists = ctx
        .state
        .read()
        .digital_inputs
        .iter()
        .any(|d| d.token == token);
    if !exists {
        return (
            StatusCode::NOT_FOUND,
            format!("unknown digital input: {token}\n"),
        );
    }
    ctx.state.modify(|s| {
        if let Some(d) = s.digital_inputs.iter_mut().find(|d| d.token == token) {
            d.logical_state = state_param.clone();
        }
        s.pending_io_events
            .push(crate::mock::state::PendingIoEvent {
                kind: "DigitalInput",
                token: token.clone(),
                logical_state: state_param.clone(),
            });
    });
    (StatusCode::OK, format!("set {state_param}\n"))
}

#[cfg(test)]
mod tests {
    use crate::OnvifClient;
    use crate::mock::MockServer;

    #[tokio::test]
    async fn bound_server_roundtrips_via_real_http() {
        let server = MockServer::start().await.unwrap();
        let client = OnvifClient::new(server.device_url());
        let info = client.get_device_info().await.unwrap();
        assert_eq!(info.manufacturer, "oxvif-mock");
    }

    #[tokio::test]
    async fn bound_server_set_then_get_roundtrips() {
        let server = MockServer::start().await.unwrap();
        let client = OnvifClient::new(server.device_url());
        client.set_hostname("bound-cam").await.unwrap();
        let h = client.get_hostname().await.unwrap();
        assert_eq!(h.name.as_deref(), Some("bound-cam"));
        // Server-side state reflects it too.
        assert_eq!(server.device().read().hostname, "bound-cam");
    }

    #[tokio::test]
    async fn bound_server_start_firmware_upgrade_returns_upload_uri() {
        let server = MockServer::start().await.unwrap();
        let client = OnvifClient::new(server.device_url());
        let start = client.start_firmware_upgrade().await.unwrap();
        assert!(start.upload_uri.ends_with("/upload/firmware"));
        assert_eq!(start.expected_down_time, "PT30S");
    }

    #[tokio::test]
    async fn bound_server_start_system_restore_returns_upload_uri() {
        let server = MockServer::start().await.unwrap();
        let client = OnvifClient::new(server.device_url());
        let start = client.start_system_restore().await.unwrap();
        assert!(start.upload_uri.ends_with("/upload/restore"));
    }

    #[tokio::test]
    async fn bound_server_system_uris_includes_backup() {
        let server = MockServer::start().await.unwrap();
        let client = OnvifClient::new(server.device_url());
        let uris = client.get_system_uris().await.unwrap();
        assert!(uris.system_backup_uri.is_some());
    }

    /// The bound-port replay "container": record a camera, serve the clone from
    /// a real HTTP `MockServer`, and drive it with a client over the network —
    /// reads replay the recorded device and `Set → Get` still round-trips via
    /// synthetic copy-on-write.
    #[cfg(feature = "metamorph")]
    #[tokio::test]
    async fn bound_replay_server_serves_clone_and_cow_roundtrips() {
        use crate::metamorph::{FixtureStore, RecordingTransport};
        use std::sync::{Arc, Mutex};

        // 1. "Real camera": a synthetic mock with a distinctive hostname.
        let real = crate::mock::MockTransport::new();
        real.device()
            .modify(|s| s.hostname = "real-camera-host".to_string());

        // 2. Record its standard reads into a FixtureStore.
        let store = Arc::new(Mutex::new(FixtureStore::new("mock-real")));
        let rec = RecordingTransport::new(Arc::new(real), store.clone());
        let rc = OnvifClient::new("http://real").with_transport(Arc::new(rec));
        rc.get_device_info().await.unwrap();
        rc.get_hostname().await.unwrap();
        let recorded = store.lock().unwrap().clone();

        // 3. Serve the clone from a bound MockServer, drive it over real HTTP.
        let server = MockServer::builder()
            .replay(recorded)
            .start()
            .await
            .unwrap();
        let client = OnvifClient::new(server.device_url());

        // Reads replay the recorded camera verbatim.
        let host = client.get_hostname().await.unwrap();
        assert_eq!(
            host.name.as_deref(),
            Some("real-camera-host"),
            "bound replay must serve the recorded hostname, not the synthetic default"
        );

        // Copy-on-write: after a Set, the family falls to live synthetic state.
        client.set_hostname("changed-host").await.unwrap();
        let host2 = client.get_hostname().await.unwrap();
        assert_eq!(
            host2.name.as_deref(),
            Some("changed-host"),
            "Set then Get must reflect the new value via synthetic COW"
        );
    }
}
