//! Record-and-replay SOAP transports for fixture-based testing.
//!
//! Gated on the `mock` feature (same intent: testing without a real camera).
//!
//! [`CapturingTransport`] wraps any [`Transport`] and writes every SOAP
//! exchange to disk as `<action>.req.xml` and `<action>.resp.xml`.
//! [`FixtureTransport`] reads such a directory and replays the captured
//! response for each action.
//!
//! Typical workflow:
//!
//! 1. Point an `OnvifSession` at a real camera through `CapturingTransport`
//!    and run [`HealthCheck`](crate::health::HealthCheck) — this dumps a
//!    full set of fixtures for that device.
//! 2. Commit those fixtures under `tests/fixtures/<vendor>-<model>/`.
//! 3. Use `FixtureTransport` in unit tests to drive parsing / behaviour
//!    against the captured responses — no camera required after step 1.
//!
//! The companion `examples/record-fixtures.rs` is the canonical recorder.
//!
//! Filenames use the **last URL segment of the SOAP action**, stripped to
//! `[A-Za-z0-9_-]`. So
//! `http://www.onvif.org/ver10/media/wsdl/GetProfiles` →
//! `GetProfiles.req.xml` / `GetProfiles.resp.xml`. Repeated calls
//! overwrite (last-write-wins) — sufficient for a single recorder run.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::transport::{Transport, TransportError};

/// Wraps any [`Transport`] and writes every request/response pair to disk.
///
/// Construct with [`CapturingTransport::new`], pass the inner transport, and
/// hand the wrapper to `OnvifClient::with_transport` / `OnvifSession::builder`.
pub struct CapturingTransport {
    inner: Arc<dyn Transport>,
    out_dir: PathBuf,
}

impl CapturingTransport {
    /// Wrap `inner` so every SOAP call is also written under `out_dir`.
    /// The directory is created lazily on first call.
    pub fn new(inner: Arc<dyn Transport>, out_dir: impl Into<PathBuf>) -> Self {
        Self {
            inner,
            out_dir: out_dir.into(),
        }
    }
}

#[async_trait]
impl Transport for CapturingTransport {
    async fn soap_post(
        &self,
        url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        let name = safe_action_name(action);
        let req_path = self.out_dir.join(format!("{name}.req.xml"));
        let resp_path = self.out_dir.join(format!("{name}.resp.xml"));

        if let Err(e) = fs::create_dir_all(&self.out_dir) {
            eprintln!(
                "CapturingTransport: failed to create {:?}: {e}",
                self.out_dir
            );
        }
        if let Err(e) = fs::write(&req_path, &body) {
            eprintln!("CapturingTransport: failed to write {req_path:?}: {e}");
        }
        let result = self.inner.soap_post(url, action, body).await;
        if let Ok(ref resp) = result {
            if let Err(e) = fs::write(&resp_path, resp) {
                eprintln!("CapturingTransport: failed to write {resp_path:?}: {e}");
            }
        }
        result
    }
}

/// Replays SOAP responses from a directory of captured fixtures.
///
/// Looks up `<dir>/<safe_action_name>.resp.xml`. If the file is missing,
/// returns [`TransportError::HttpStatus`] `{ status: 404, body: <path> }`
/// so tests can distinguish a missing fixture from a real protocol error.
pub struct FixtureTransport {
    dir: PathBuf,
}

impl FixtureTransport {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }
}

#[async_trait]
impl Transport for FixtureTransport {
    async fn soap_post(
        &self,
        _url: &str,
        action: &str,
        _body: String,
    ) -> Result<String, TransportError> {
        let name = safe_action_name(action);
        let path = self.dir.join(format!("{name}.resp.xml"));
        match fs::read_to_string(&path) {
            Ok(s) => Ok(s),
            Err(_) => Err(TransportError::HttpStatus {
                status: 404,
                body: format!("fixture not found: {}", path.display()),
            }),
        }
    }
}

/// Take the last URL segment of `action` and keep only `[A-Za-z0-9_-]` —
/// just enough to be a safe file basename across platforms.
fn safe_action_name(action: &str) -> String {
    let last = action.rsplit('/').next().unwrap_or(action);
    let name: String = last
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if name.is_empty() {
        "Unnamed".to_string()
    } else {
        name
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OnvifClient;
    use crate::mock::MockTransport;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn tmp_dir(label: &str) -> PathBuf {
        let id = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!(
            "oxvif-fixtures-{}-{}-{label}",
            std::process::id(),
            id,
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn safe_action_name_strips_url_and_specials() {
        assert_eq!(
            safe_action_name("http://www.onvif.org/ver10/media/wsdl/GetProfiles"),
            "GetProfiles"
        );
        assert_eq!(safe_action_name("Simple"), "Simple");
        assert_eq!(safe_action_name("with spaces!"), "withspaces");
        // Edge case: trailing slash falls back.
        assert_eq!(safe_action_name("https://x/"), "Unnamed");
    }

    #[tokio::test]
    async fn capturing_then_replay_yields_identical_response() {
        let dir = tmp_dir("roundtrip");

        // 1. Record: drive a real call through CapturingTransport wrapping a MockTransport.
        let inner: Arc<dyn Transport> = Arc::new(MockTransport::new());
        let cap = CapturingTransport::new(inner.clone(), &dir);
        let client = OnvifClient::new("http://mock").with_transport(Arc::new(cap));
        let caps_recorded = client
            .get_capabilities()
            .await
            .expect("mock returns Capabilities");

        // The req + resp files for GetCapabilities exist.
        assert!(dir.join("GetCapabilities.req.xml").exists());
        assert!(dir.join("GetCapabilities.resp.xml").exists());

        // 2. Replay: point a FixtureTransport at the same directory.
        let fix = FixtureTransport::new(&dir);
        let client2 = OnvifClient::new("http://replay").with_transport(Arc::new(fix));
        let caps_replayed = client2
            .get_capabilities()
            .await
            .expect("fixture replay returns Capabilities");

        // The two parses produce the same service URLs.
        assert_eq!(
            caps_recorded.device.url.as_deref(),
            caps_replayed.device.url.as_deref()
        );
        assert_eq!(
            caps_recorded.media.url.as_deref(),
            caps_replayed.media.url.as_deref()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn missing_fixture_returns_404() {
        let dir = tmp_dir("missing");
        let fix = FixtureTransport::new(&dir);
        let result = fix
            .soap_post(
                "http://test",
                "http://www.onvif.org/ver10/device/wsdl/GetCapabilities",
                "<body/>".into(),
            )
            .await;
        match result {
            Err(TransportError::HttpStatus { status, body }) => {
                assert_eq!(status, 404);
                assert!(body.contains("GetCapabilities.resp.xml"));
            }
            other => panic!("expected 404 HttpStatus, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
