//! Record half of Persona B: tap a live transport into a [`FixtureStore`].

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::transport::{HttpTransport, Transport, TransportError};
use crate::{OnvifError, OnvifSession};

use super::fixture::FixtureStore;
use super::surface::{SurfaceSelection, drive_standard_surface, drive_surface};

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
/// store.lock().unwrap().save("tests/fixtures/acme-cam")?;
/// # Ok(()) }
/// ```
pub struct RecordingTransport {
    inner: Arc<dyn Transport>,
    store: Arc<Mutex<FixtureStore>>,
}

impl RecordingTransport {
    /// Tap `inner`, recording each successful exchange into `store`.
    pub fn new(inner: Arc<dyn Transport>, store: Arc<Mutex<FixtureStore>>) -> Self {
        Self { inner, store }
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
        let resp = self.inner.soap_post(url, action, body.clone()).await?;
        self.store.lock().unwrap().record(action, &body, &resp);
        Ok(resp)
    }
}

/// Clone a real camera's standard read surface into a [`FixtureStore`] in one
/// call: builds a session over an [`HttpTransport`] tapped by a
/// [`RecordingTransport`], drives [`drive_standard_surface`], and returns the
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
    let (store, _report) = record_into(device_url, credentials, label, None).await?;
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
    record_into(device_url, credentials, label, Some(selection)).await
}

/// Shared body of [`record_standard_surface`] / [`record_surface`]: build a
/// tapped session, drive the surface, return the recorded store + sweep report.
async fn record_into(
    device_url: &str,
    credentials: Option<(&str, &str)>,
    label: impl Into<String>,
    selection: Option<&SurfaceSelection>,
) -> Result<(FixtureStore, super::SweepReport), OnvifError> {
    let mut http = HttpTransport::new();
    if let Some((u, p)) = credentials {
        http = http.with_credentials(u.to_string(), p.to_string());
    }
    let store = Arc::new(Mutex::new(FixtureStore::new(label)));
    let tap: Arc<dyn Transport> = Arc::new(RecordingTransport::new(Arc::new(http), store.clone()));

    let mut builder = OnvifSession::builder(device_url).with_transport(tap);
    if let Some((u, p)) = credentials {
        builder = builder.with_credentials(u.to_string(), p.to_string());
    }
    let session = builder.build().await?;

    let report = match selection {
        Some(sel) => drive_surface(&session, sel).await,
        None => drive_standard_surface(&session).await,
    };

    let recorded = store.lock().unwrap().clone();
    Ok((recorded, report))
}
