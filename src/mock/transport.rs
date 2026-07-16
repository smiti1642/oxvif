//! In-process [`Transport`] backed by the mock ONVIF device.
//!
//! Routes each SOAP call straight into the mock dispatcher — no sockets, no
//! HTTP, no async runtime beyond what the client already uses. Ideal for unit
//! tests of client code without a real camera.

use std::sync::Arc;

use async_trait::async_trait;

use crate::mock::fault_injection::{FaultInjector, PendingFault};
use crate::mock::responder::{Chain, RequestCtx};
use crate::mock::state::MockState;
use crate::transport::{Transport, TransportError};

/// Base URL the mock uses when it has to emit absolute URLs (snapshot /
/// subscription references). Arbitrary — there is no real server behind it.
const MOCK_BASE: &str = "http://mock";

/// An in-process mock ONVIF device that implements [`Transport`].
///
/// ```no_run
/// use std::sync::Arc;
/// use oxvif::OnvifClient;
/// use oxvif::mock::MockTransport;
///
/// # async fn run() -> Result<(), oxvif::OnvifError> {
/// let client = OnvifClient::new("http://mock")
///     .with_transport(Arc::new(MockTransport::new()));
/// let info = client.get_device_info().await?;
/// assert_eq!(info.manufacturer, "oxvif-mock");
/// # Ok(()) }
/// ```
///
/// Cheap to clone (everything is behind `Arc`); clones share the same device
/// state and fault queue.
#[derive(Clone)]
pub struct MockTransport {
    state: Arc<MockState>,
    faults: Arc<FaultInjector>,
    /// When `false` (the default) WS-Security is not enforced, so a credential-
    /// less `OnvifClient` works out of the box. Enable via [`with_auth`] to
    /// exercise authentication flows.
    ///
    /// [`with_auth`]: MockTransport::with_auth
    enforce_auth: bool,
}

impl MockTransport {
    /// A mock device seeded with factory defaults; authentication off.
    pub fn new() -> Self {
        Self {
            state: Arc::new(MockState::new()),
            faults: Arc::new(FaultInjector::new()),
            enforce_auth: false,
        }
    }

    /// Build on top of a caller-supplied [`MockState`] (e.g. seeded with a
    /// custom [`DeviceState`](crate::mock::DeviceState)).
    pub fn with_state(state: MockState) -> Self {
        Self {
            state: Arc::new(state),
            faults: Arc::new(FaultInjector::new()),
            enforce_auth: false,
        }
    }

    /// Enforce WS-Security `PasswordDigest` on non-exempt actions, matching a
    /// real device. The client must then supply matching credentials
    /// (default users: `admin`/`admin`, `operator`/`operator`).
    pub fn with_auth(mut self) -> Self {
        self.enforce_auth = true;
        self
    }

    /// Access the shared device state — seed it before a test or assert on it
    /// after (`transport.device().read()` / `.modify(..)`).
    pub fn device(&self) -> &MockState {
        &self.state
    }

    /// Arm a single-shot SOAP Fault for the next call whose action URI ends
    /// with `action_suffix` (e.g. `"GetProfiles"`). Consumed on first match.
    pub fn inject_fault(
        &self,
        action_suffix: impl Into<String>,
        code: impl Into<String>,
        reason: impl Into<String>,
    ) {
        self.faults.inject(PendingFault {
            action_suffix: action_suffix.into(),
            code: code.into(),
            reason: reason.into(),
        });
    }

    /// Drop every queued fault.
    pub fn clear_faults(&self) {
        self.faults.clear_all();
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn soap_post(
        &self,
        _url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        // Default pipeline: armed fault → auth gate → synthetic dispatch.
        let chain = Chain::default_mock(self.faults.clone(), self.enforce_auth);
        let ctx = RequestCtx {
            action,
            base: MOCK_BASE,
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

    fn client_with(t: MockTransport) -> OnvifClient {
        OnvifClient::new("http://mock").with_transport(Arc::new(t))
    }

    #[tokio::test]
    async fn get_device_info_roundtrips_without_credentials() {
        let c = client_with(MockTransport::new());
        let info = c.get_device_info().await.unwrap();
        assert_eq!(info.manufacturer, "oxvif-mock");
        assert_eq!(info.model, "MockCam-1080p");
    }

    #[tokio::test]
    async fn set_then_get_hostname_roundtrips() {
        let c = client_with(MockTransport::new());
        c.set_hostname("lab-cam").await.unwrap();
        let h = c.get_hostname().await.unwrap();
        assert_eq!(h.name.as_deref(), Some("lab-cam"));
    }

    #[tokio::test]
    async fn injected_fault_surfaces_as_soap_fault() {
        use crate::error::OnvifError;
        use crate::soap::SoapError;

        let t = MockTransport::new();
        t.inject_fault("GetProfiles", "ter:NotAuthorized", "nope");
        let c = client_with(t);
        let err = c.get_profiles("http://mock/media").await.unwrap_err();
        assert!(matches!(err, OnvifError::Soap(SoapError::Fault { .. })));
    }

    #[tokio::test]
    async fn with_auth_rejects_missing_credentials() {
        use crate::error::OnvifError;
        use crate::soap::SoapError;

        let c = client_with(MockTransport::new().with_auth());
        // No credentials on the client → digest validation fails.
        let err = c.get_device_info().await.unwrap_err();
        assert!(matches!(err, OnvifError::Soap(SoapError::Fault { .. })));
    }

    #[tokio::test]
    async fn instances_have_independent_state() {
        let a = client_with(MockTransport::new());
        let b = MockTransport::new();
        let bc = client_with(b.clone());
        a.set_hostname("host-a").await.unwrap();
        bc.set_hostname("host-b").await.unwrap();
        assert_eq!(b.device().read().hostname, "host-b");
        assert_ne!(b.device().read().hostname, "host-a");
    }
}
