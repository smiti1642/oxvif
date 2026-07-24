//! Record half of Persona B: tap a live transport into a [`FixtureStore`].

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::transport::{HttpTransport, Transport, TransportError};
use crate::{OnvifError, OnvifSession};

use super::fixture::FixtureStore;

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

    drive_standard_surface(&session).await;

    let recorded = store.lock().unwrap().clone();
    Ok(recorded)
}

/// Drive the standard ONVIF read surface against `session`: device info, time,
/// services, hostname, per-profile stream / snapshot URIs, encoder configs,
/// imaging, PTZ nodes, and network interfaces. Per-profile reads exercise the
/// param-aware fixture key (`token=A` vs `token=B`).
///
/// Every call is best-effort — a device that lacks a service is simply skipped.
/// When `session`'s transport is a [`RecordingTransport`], each successful
/// exchange lands in its store; this is the op list [`record_standard_surface`]
/// records, exposed on its own for callers that manage the session themselves.
pub async fn drive_standard_surface(session: &OnvifSession) {
    let _ = session.get_device_info().await;
    let _ = session.get_system_date_and_time().await;
    let _ = session.get_services().await;
    let _ = session.get_hostname().await;
    if let Ok(profiles) = session.get_profiles().await {
        for p in &profiles {
            let _ = session.get_stream_uri(&p.token).await;
            let _ = session.get_snapshot_uri(&p.token).await;
        }
    }
    let _ = session.get_video_encoder_configurations().await;
    if let Ok(sources) = session.get_video_sources().await
        && let Some(s) = sources.first()
    {
        let _ = session.get_imaging_settings(&s.token).await;
    }
    let _ = session.ptz_get_nodes().await;
    let _ = session.get_network_interfaces().await;
}
