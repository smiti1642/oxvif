//! Record-and-replay SOAP transports for fixture-based testing.
//!
//! Gated on the `mock` feature (same intent: testing without a real camera).
//!
//! [`CapturingTransport`] wraps any [`Transport`] and writes every SOAP
//! exchange to disk as `<action>.req.xml` and `<action>.resp.xml`.
//! [`FixtureTransport`] reads such a directory and replays the captured
//! response for each action.
//!
//! # Credentials are redacted before anything is written
//!
//! An authenticated SOAP request carries a WS-Security `UsernameToken`, and a
//! `GetStreamUri` response can carry `rtsp://user:pass@host/…`. By default
//! `CapturingTransport` blanks the `<wsse:Password>` and `<wsse:Nonce>` of every
//! request and strips `user:pass@` from every URL in a response, so a capture
//! directory is safe to commit. `<wsse:Username>` and `<wsu:Created>` are kept —
//! they are not secret and they are what makes a capture readable.
//!
//! Both transforms can be turned off — see
//! [`with_raw_requests`](CapturingTransport::with_raw_requests) and
//! [`with_raw_responses`](CapturingTransport::with_raw_responses) — for the one
//! case that needs the untouched bytes: debugging WS-Security itself, where the
//! digest *is* the thing under investigation. A directory recorded that way
//! holds live credentials; treat it as secret material.
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
//! For anything beyond parser fixtures, prefer the `metamorph` feature: its
//! `FixtureStore` records the same exchanges param-aware and keyed by
//! `(action, canonical request)`, so one action with two different tokens does
//! not overwrite itself the way the last-write-wins files here do.
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

use crate::redact::{redact_credentials, scrub_url_userinfo};
use crate::transport::{Transport, TransportError};

/// Wraps any [`Transport`] and writes every request/response pair to disk.
///
/// Construct with [`CapturingTransport::new`], pass the inner transport, and
/// hand the wrapper to `OnvifClient::with_transport` / `OnvifSession::builder`.
///
/// Credentials are redacted on the way to disk unless you opt out — see the
/// [module docs](self).
pub struct CapturingTransport {
    inner: Arc<dyn Transport>,
    out_dir: PathBuf,
    /// Write requests with the WS-Security `Password`/`Nonce` intact.
    raw_requests: bool,
    /// Write responses with URL `user:pass@` userinfo intact.
    raw_responses: bool,
}

impl CapturingTransport {
    /// Wrap `inner` so every SOAP call is also written under `out_dir`.
    /// The directory is created lazily on first call.
    ///
    /// Credentials are redacted from both directions by default.
    pub fn new(inner: Arc<dyn Transport>, out_dir: impl Into<PathBuf>) -> Self {
        Self {
            inner,
            out_dir: out_dir.into(),
            raw_requests: false,
            raw_responses: false,
        }
    }

    /// Write `*.req.xml` **verbatim**, keeping the WS-Security `<wsse:Password>`
    /// digest and `<wsse:Nonce>`.
    ///
    /// The one reason to want this is debugging WS-Security itself — a device
    /// rejecting the digest, where the digest is the evidence. The captured
    /// files then contain a live credential: do not commit them, and delete the
    /// directory when the investigation ends.
    pub fn with_raw_requests(mut self) -> Self {
        self.raw_requests = true;
        self
    }

    /// Write `*.resp.xml` **verbatim**, keeping any `user:pass@` userinfo in
    /// URLs the device returned (`GetStreamUri`, `GetSnapshotUri`).
    ///
    /// Use when a test needs the exact URI the device produced, credentials
    /// included. The captured files then contain a live credential: do not
    /// commit them.
    pub fn with_raw_responses(mut self) -> Self {
        self.raw_responses = true;
        self
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
        // Redact before writing, never after: the file must not exist in a
        // credential-bearing form even briefly.
        let to_write = if self.raw_requests {
            body.clone()
        } else {
            redact_credentials(&body)
        };
        if let Err(e) = fs::write(&req_path, &to_write) {
            eprintln!("CapturingTransport: failed to write {req_path:?}: {e}");
        }
        let result = self.inner.soap_post(url, action, body).await;
        if let Ok(ref resp) = result {
            let to_write = if self.raw_responses {
                resp.clone()
            } else {
                scrub_url_userinfo(resp)
            };
            if let Err(e) = fs::write(&resp_path, &to_write) {
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

    // ── Redaction ─────────────────────────────────────────────────────────
    //
    // The transport is driven directly rather than through `OnvifClient`, so the
    // fixture strings below are exactly what reaches disk and each assertion
    // names the secret it is looking for.

    /// A request carrying a WS-Security header, and a response carrying an RTSP
    /// URI with embedded credentials. Every secret has a distinctive value, so
    /// an assertion can only pass by acting on the byte it names.
    const SECRET_REQ: &str = "<s:Envelope><s:Header><wsse:Security><wsse:UsernameToken>\
         <wsse:Username>admin</wsse:Username>\
         <wsse:Password Type=\"#PasswordDigest\">DIGEST-cap-4417</wsse:Password>\
         <wsse:Nonce EncodingType=\"Base64Binary\">NONCE-cap-4417</wsse:Nonce>\
         </wsse:UsernameToken></wsse:Security></s:Header>\
         <s:Body><trt:GetStreamUri/></s:Body></s:Envelope>";

    const SECRET_RESP: &str = "<s:Envelope><s:Body><trt:GetStreamUriResponse><tt:Uri>\
         rtsp://admin:RTSPPASS-cap-4417@10.0.0.9:554/Streaming/101\
         </tt:Uri></trt:GetStreamUriResponse></s:Body></s:Envelope>";

    const STREAM_URI_ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetStreamUri";

    /// Drive one exchange through a `CapturingTransport` built by `build`, and
    /// return `(request file, response file)` as written.
    async fn capture_once(
        label: &str,
        build: impl FnOnce(CapturingTransport) -> CapturingTransport,
    ) -> (String, String) {
        let dir = tmp_dir(label);
        let cap = build(CapturingTransport::new(
            crate::tests::common::mock(SECRET_RESP),
            &dir,
        ));
        cap.soap_post("http://cam", STREAM_URI_ACTION, SECRET_REQ.into())
            .await
            .expect("inner transport answers");

        let req = fs::read_to_string(dir.join("GetStreamUri.req.xml")).unwrap();
        let resp = fs::read_to_string(dir.join("GetStreamUri.resp.xml")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        (req, resp)
    }

    /// The default: neither the password digest, the nonce, nor the RTSP
    /// password may appear in either file, while the non-secret context that
    /// makes a capture readable survives.
    #[tokio::test]
    async fn capture_redacts_credentials_by_default() {
        let (req, resp) = capture_once("redacted", |c| c).await;

        assert!(!req.contains("DIGEST-cap-4417"), "digest leaked: {req}");
        assert!(!req.contains("NONCE-cap-4417"), "nonce leaked: {req}");
        assert!(req.contains(">[redacted]</wsse:Password>"));
        assert!(req.contains(">[redacted]</wsse:Nonce>"));
        assert!(
            req.contains("<wsse:Username>admin</wsse:Username>"),
            "username is not secret and must survive: {req}"
        );

        assert!(
            !resp.contains("RTSPPASS-cap-4417"),
            "rtsp pw leaked: {resp}"
        );
        assert!(!resp.contains("admin:"), "rtsp userinfo leaked: {resp}");
        assert!(
            resp.contains("rtsp://10.0.0.9:554/Streaming/101"),
            "host and path must survive: {resp}"
        );
    }

    /// `with_raw_requests` restores the untouched request — and must not also
    /// un-redact the response, or the two opt-outs are not independent.
    #[tokio::test]
    async fn with_raw_requests_keeps_the_request_verbatim_only() {
        let (req, resp) = capture_once("raw-req", |c| c.with_raw_requests()).await;

        assert_eq!(req, SECRET_REQ, "request must be byte-identical");
        assert!(
            !resp.contains("RTSPPASS-cap-4417"),
            "response redaction is independent and stays on: {resp}"
        );
    }

    /// The mirror: `with_raw_responses` restores the response only.
    #[tokio::test]
    async fn with_raw_responses_keeps_the_response_verbatim_only() {
        let (req, resp) = capture_once("raw-resp", |c| c.with_raw_responses()).await;

        assert_eq!(resp, SECRET_RESP, "response must be byte-identical");
        assert!(
            !req.contains("DIGEST-cap-4417"),
            "request redaction is independent and stays on: {req}"
        );
    }

    /// Redaction changes only what is written. The inner transport — and so the
    /// device — must still receive the credential-bearing request, or the
    /// capture would break authentication instead of protecting it.
    #[tokio::test]
    async fn redaction_does_not_alter_what_the_device_receives() {
        let dir = tmp_dir("passthrough");
        let (inner, captured) = crate::tests::common::RecordingTransport::new(SECRET_RESP);
        let cap = CapturingTransport::new(inner, &dir);
        cap.soap_post("http://cam", STREAM_URI_ACTION, SECRET_REQ.into())
            .await
            .unwrap();

        let c = captured.lock().unwrap();
        assert_eq!(c.body, SECRET_REQ, "the wire request must be untouched");
        assert_eq!(c.action, STREAM_URI_ACTION);
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
