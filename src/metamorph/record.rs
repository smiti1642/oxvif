//! Record half of Persona B: tap a live transport into a [`FixtureStore`].

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::transport::{Transport, TransportError};

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
