//! Record half of Persona B: tap a live transport into a [`FixtureStore`].

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::transport::{HttpTransport, Transport, TransportError};
use crate::{OnvifError, OnvifSession};

use super::fixture::FixtureStore;
use super::surface::{SurfaceSelection, SweepProgress, drive_surface_with_progress};

/// One final Transport failure, separate from a stored SOAP Fault response.
/// Request redaction is targeted; review usernames, tokens and custom fields.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecordedTransportFailure {
    /// Exact full Action, retaining service identity.
    pub action: String,
    /// Targeted-redacted request identity, retaining meaningful channel tokens.
    pub request_raw: String,
    /// Controlled classification, with no raw error/body/URL text.
    pub error_class: crate::session::SessionErrorClass,
    /// Numeric final HTTP error status, if known.
    pub http_status: Option<u16>,
}

/// Opt-in bounded transport evidence. This does not count Digest HTTP exchanges.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecordingEvidence {
    /// Up to 512 failures and 4 MiB of retained Action/request bytes.
    pub failures: Vec<RecordedTransportFailure>,
    /// Events refused by the evidence budget; never silently treat them as success.
    pub omitted: usize,
    /// Collection was disabled or its mutex was unavailable.
    pub unavailable: bool,
}

impl RecordingEvidence {
    fn record(&mut self, action: &str, body: &str, error: &TransportError) {
        const BYTES: usize = 4 * 1024 * 1024;
        let retained = self.failures.iter().fold(0_usize, |sum, row| {
            sum.saturating_add(row.action.len())
                .saturating_add(row.request_raw.len())
        });
        if self.failures.len() >= 512
            || retained
                .saturating_add(action.len())
                .saturating_add(body.len())
                > BYTES
        {
            self.omitted = self.omitted.saturating_add(1);
            return;
        }
        use crate::session::SessionErrorClass as Class;
        let (error_class, http_status) = match error {
            TransportError::HttpStatus { status, .. } => (
                if matches!(status, 401 | 403) {
                    Class::Authentication
                } else {
                    Class::Transport
                },
                Some(*status),
            ),
            TransportError::Http(error) if error.is_timeout() => (Class::Timeout, None),
            TransportError::Deadline => (Class::Timeout, None),
            TransportError::ResponseTooLarge { .. } => (Class::ResponseLimit, None),
            _ => (Class::Transport, None),
        };
        let request_raw =
            crate::redact::scrub_url_userinfo(&crate::redact::redact_credentials(body));
        if retained
            .saturating_add(action.len())
            .saturating_add(request_raw.len())
            > BYTES
        {
            self.omitted = self.omitted.saturating_add(1);
            return;
        }
        self.failures.push(RecordedTransportFailure {
            action: action.into(),
            request_raw,
            error_class,
            http_status,
        });
    }
}

/// Wraps a real [`Transport`] and records each **successful** SOAP exchange
/// into a shared [`FixtureStore`]. Drive a normal `OnvifSession` through it
/// against a camera, then [`FixtureStore::save`] the result.
///
/// ```no_run
/// use std::sync::{Arc, Mutex};
/// use oxvif::OnvifSession;
/// use oxvif::metamorph::{FixtureStore, RecordingTransport};
/// use oxvif::transport::{HttpTransport, Transport};
///
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let store = Arc::new(Mutex::new(FixtureStore::new("acme-cam")));
/// let inner: Arc<dyn Transport> = Arc::new(HttpTransport::new());
/// let tap = Arc::new(RecordingTransport::new(inner, store.clone()));
/// let session = OnvifSession::builder("http://cam/onvif/device_service")
///     .with_transport(tap)
///     .build()
///     .await?;
/// session.get_device_info().await?;
/// store.lock().unwrap().save("/private/oxvif-captures/acme-cam")?;
/// # Ok(()) }
/// ```
pub struct RecordingTransport {
    inner: Arc<dyn Transport>,
    store: Arc<Mutex<FixtureStore>>,
    evidence: Option<Mutex<RecordingEvidence>>,
}

impl RecordingTransport {
    /// Tap `inner`, recording each successful exchange into `store`.
    pub fn new(inner: Arc<dyn Transport>, store: Arc<Mutex<FixtureStore>>) -> Self {
        Self {
            inner,
            store,
            evidence: None,
        }
    }

    /// Also retain bounded typed transport failures separately from SOAP responses.
    /// This is opt-in capture evidence, not a secret-free support bundle.
    pub fn with_evidence(mut self) -> Self {
        self.evidence = Some(Mutex::new(RecordingEvidence::default()));
        self
    }

    /// Drain opt-in evidence. Disabled/unavailable collection is explicit.
    pub fn take_evidence(&self) -> RecordingEvidence {
        match self
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.lock().ok())
        {
            Some(mut evidence) => std::mem::take(&mut *evidence),
            None => RecordingEvidence {
                unavailable: true,
                ..RecordingEvidence::default()
            },
        }
    }
}

#[async_trait]
impl Transport for RecordingTransport {
    async fn soap_post(
        &self,
        url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        let resp = match self.inner.soap_post(url, action, body.clone()).await {
            Ok(response) => response,
            Err(error) => {
                if let Some(evidence) = &self.evidence
                    && let Ok(mut evidence) = evidence.lock()
                {
                    evidence.record(action, &body, &error);
                }
                return Err(error);
            }
        };
        self.store.lock().unwrap().record(action, &body, &resp);
        Ok(resp)
    }
}

/// Clone a real camera's standard read surface into a [`FixtureStore`] in one
/// call: builds a session over an [`HttpTransport`] tapped by a
/// [`RecordingTransport`], drives
/// [`drive_standard_surface`](super::drive_standard_surface), and returns the
/// recorded set — no camera needed afterwards. This is the library form of
/// `examples/metamorph_record.rs`, so a caller (e.g. oxdm's "clone this camera"
/// button) never copies the operation list.
///
/// `label` names the store (e.g. `"hikvision-ds2cd"`); `credentials` are the
/// WS-Security / HTTP-Digest user and password, or `None` for an open device.
/// Fails only if the initial session cannot be built (unreachable / unauthorised
/// device); individual reads are best-effort and a missing service is skipped.
///
/// ```no_run
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// use oxvif::metamorph::record_standard_surface;
/// let clone = record_standard_surface(
///     "http://192.168.1.100/onvif/device_service",
///     Some(("admin", "password")),
///     "hikvision-ds2cd",
/// )
/// .await?;
/// clone.save("clones/hikvision-ds2cd")?;
/// # Ok(()) }
/// ```
pub async fn record_standard_surface(
    device_url: &str,
    credentials: Option<(&str, &str)>,
    label: impl Into<String>,
) -> Result<FixtureStore, OnvifError> {
    let (store, _report, _) =
        record_into(device_url, credentials, label, None, false, |_| {}).await?;
    Ok(store)
}

/// Clone a **chosen subset** of a real camera's read surface into a
/// [`FixtureStore`], returning it alongside a [`SweepReport`](super::SweepReport)
/// of what each selected operation did (recorded / failed / skipped). Same as
/// [`record_standard_surface`] but drives only the operations in `selection`
/// (prerequisites auto-included) — the entry point for a "pick which commands to
/// capture" UI or a targeted quirk-reproduction run.
///
/// ```no_run
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// use oxvif::metamorph::{SurfaceGroup, SurfaceSelection, record_surface};
/// // Just the media zone, plus the single GetStreamUri command.
/// let selection = SurfaceSelection::from_groups(&[SurfaceGroup::Media]);
/// let (clone, report) = record_surface(
///     "http://192.168.1.100/onvif/device_service",
///     Some(("admin", "password")),
///     "hikvision-ds2cd",
///     &selection,
/// )
/// .await?;
/// clone.save("clones/hikvision-ds2cd")?;
/// for op in report.skipped() {
///     eprintln!("skipped {}: {:?}", op.action_name(), report.outcome(op));
/// }
/// # Ok(()) }
/// ```
pub async fn record_surface(
    device_url: &str,
    credentials: Option<(&str, &str)>,
    label: impl Into<String>,
    selection: &SurfaceSelection,
) -> Result<(FixtureStore, super::SweepReport), OnvifError> {
    record_surface_with_progress(device_url, credentials, label, selection, |_| {}).await
}

/// [`record_surface`], reporting sweep progress as it goes.
///
/// `progress` fires **once per selected operation** (after prerequisite
/// expansion) with [`SweepProgress::total`] fixed for the whole run — see
/// [`drive_surface_with_progress`](super::drive_surface_with_progress) for the
/// exact firing rule and why the unit is an operation rather than an HTTP
/// request.
///
/// No event is emitted for the initial session build (service discovery), which
/// happens before the sweep and before `total` is meaningful; a UI should show
/// an indeterminate "connecting" state until the first event arrives.
///
/// Passing [`SurfaceSelection::recommended`] gives the progress-reporting form
/// of [`record_standard_surface`].
///
/// ```no_run
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// use oxvif::metamorph::{SurfaceSelection, record_surface_with_progress};
/// let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
/// let (clone, report) = record_surface_with_progress(
///     "http://192.168.1.100/onvif/device_service",
///     Some(("admin", "password")),
///     "hikvision-ds2cd",
///     &SurfaceSelection::recommended(),
///     move |p| {
///         let _ = tx.send(p);
///     },
/// )
/// .await?;
/// # let _ = (clone, report);
/// # Ok(()) }
/// ```
pub async fn record_surface_with_progress(
    device_url: &str,
    credentials: Option<(&str, &str)>,
    label: impl Into<String>,
    selection: &SurfaceSelection,
    progress: impl Fn(SweepProgress) + Send + Sync,
) -> Result<(FixtureStore, super::SweepReport), OnvifError> {
    record_into(
        device_url,
        credentials,
        label,
        Some(selection),
        false,
        progress,
    )
    .await
    .map(|(store, report, _)| (store, report))
}

/// Record the same selected parameter-aware surface with separate bounded
/// transport failure evidence. SOAP Fault bodies remain stored exchanges and
/// must be classified separately by typed parsing. All selected profiles/sources
/// are driven by the existing surface driver; no suffix-named files are used.
/// Initial Session failures retain the original error; use a custom
/// RecordingTransport with evidence to inspect a failed initialization capture.
pub async fn record_surface_with_evidence(
    device_url: &str,
    credentials: Option<(&str, &str)>,
    label: impl Into<String>,
    selection: &SurfaceSelection,
    progress: impl Fn(SweepProgress) + Send + Sync,
) -> Result<(FixtureStore, super::SweepReport, RecordingEvidence), OnvifError> {
    record_into(
        device_url,
        credentials,
        label,
        Some(selection),
        true,
        progress,
    )
    .await
}

/// Shared body of [`record_standard_surface`] / [`record_surface`]: build a
/// tapped session, drive the surface, return the recorded store + sweep report.
async fn record_into(
    device_url: &str,
    credentials: Option<(&str, &str)>,
    label: impl Into<String>,
    selection: Option<&SurfaceSelection>,
    evidence: bool,
    progress: impl Fn(SweepProgress) + Send + Sync,
) -> Result<(FixtureStore, super::SweepReport, RecordingEvidence), OnvifError> {
    let mut http = HttpTransport::new();
    if let Some((u, p)) = credentials {
        http = http.with_credentials(u.to_string(), p.to_string());
    }
    let store = Arc::new(Mutex::new(FixtureStore::new(label)));
    let mut tap = RecordingTransport::new(Arc::new(http), store.clone());
    if evidence {
        tap = tap.with_evidence();
    }
    let tap = Arc::new(tap);

    let mut builder = OnvifSession::builder(device_url).with_transport(tap.clone());
    if let Some((u, p)) = credentials {
        builder = builder.with_credentials(u.to_string(), p.to_string());
    }
    let session = builder.build().await?;

    // `None` means the recommended sweep — exactly what `drive_standard_surface`
    // delegates to, taken directly here so the progress callback reaches it.
    let sel = selection
        .cloned()
        .unwrap_or_else(SurfaceSelection::recommended);
    let report = drive_surface_with_progress(&session, &sel, progress).await;

    let recorded = store.lock().unwrap().clone();
    Ok((recorded, report, tap.take_evidence()))
}

#[cfg(test)]
mod tests {
    use super::*;
    // Only `records_over_http_and_reports_progress` names a `SurfaceOp`, and it
    // needs a bound port. Ungated this is an unused import under
    // `--features metamorph` alone — the same warning class as the `Arc` import
    // fixed in 8031ab0, invisible for the same reason.
    #[cfg(feature = "mock-server")]
    use crate::metamorph::SurfaceOp;

    #[tokio::test]
    async fn summary_counts_soap_faults_but_not_failed_transport_attempts() {
        struct Reply(bool);
        #[async_trait]
        impl Transport for Reply {
            async fn soap_post(
                &self,
                _: &str,
                _: &str,
                _: String,
            ) -> Result<String, TransportError> {
                if self.0 {
                    Ok("<Envelope><Body><Fault/></Body></Envelope>".into())
                } else {
                    Err(TransportError::HttpStatus {
                        status: 503,
                        body: "unavailable".into(),
                    })
                }
            }
        }
        let store = Arc::new(Mutex::new(FixtureStore::new("test")));
        let failed = RecordingTransport::new(Arc::new(Reply(false)), store.clone());
        assert!(
            failed
                .soap_post("http://unused", "urn:Get", "<Get/>".into())
                .await
                .is_err()
        );
        assert_eq!(store.lock().unwrap().summary().fixtures, 0);
        let faulted = RecordingTransport::new(Arc::new(Reply(true)), store.clone());
        faulted
            .soap_post("http://unused", "urn:Get", "<Get/>".into())
            .await
            .unwrap();
        let summary = store.lock().unwrap().summary();
        assert_eq!(summary.fixtures, 1);
        assert_eq!(summary.faults, 1);
        assert_eq!(summary.unreadable, 0);
    }

    /// The Dioxus-desktop shape: the callback is a closure that sends into a
    /// `tokio::sync::mpsc::UnboundedSender`, and the future stays `Send` so it
    /// can be spawned. The future is deliberately **never polled** — no request
    /// is issued and no camera is needed; this pins the callback bound and the
    /// future's `Send`-ness at compile time.
    #[test]
    fn progress_callback_accepts_an_mpsc_sender() {
        fn assert_send<T: Send>(_: &T) {}

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<SweepProgress>();
        let sel = SurfaceSelection::none();
        let fut = record_surface_with_progress(
            "http://192.0.2.1/onvif/device_service",
            Some(("admin", "password")),
            "unreachable",
            &sel,
            move |p| {
                let _ = tx.send(p);
            },
        );
        assert_send(&fut);
    }

    /// End-to-end over a real bound port: the recorder's progress reaches the
    /// caller, `total` is the prerequisite-expanded selection, and the recorded
    /// clone is the same one [`record_surface`] produces.
    #[cfg(feature = "mock-server")]
    #[tokio::test]
    async fn records_over_http_and_reports_progress() {
        let server = crate::mock::MockServer::start()
            .await
            .expect("start mock server");
        let sel = SurfaceSelection::none().with(SurfaceOp::GetStreamUri);
        // GetStreamUri (picked) + GetProfiles (its prerequisite).
        let total = 2;

        let seen = std::sync::Mutex::new(Vec::new());
        let (clone, report) =
            record_surface_with_progress(server.device_url(), None, "mock-cam", &sel, |p| {
                seen.lock().unwrap().push(p)
            })
            .await
            .expect("record over the mock server");
        let seen = seen.into_inner().unwrap();

        assert!(
            seen.iter().all(|p| p.total == total),
            "total fixed for the whole run: {seen:?}"
        );
        assert_eq!(
            seen.iter().map(|p| p.done).collect::<Vec<_>>(),
            vec![1, 2],
            "one tick per selected op, ending at total"
        );
        assert_eq!(
            report.outcome(SurfaceOp::GetStreamUri),
            Some(crate::metamorph::OpOutcome::Recorded)
        );
        assert!(
            !clone.is_empty(),
            "the sweep's exchanges landed in the clone"
        );
    }
}
