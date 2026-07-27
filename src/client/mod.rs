//! High-level ONVIF client.
//!
//! [`OnvifClient`] is the primary entry point for the oxvif library. It is
//! intentionally **stateless**: the service URLs discovered via
//! `get_capabilities()` are returned to the caller rather than cached
//! internally. This design makes the client cheaply cloneable and safe to
//! share across threads behind an `Arc`.
//!
//! ## Authentication
//!
//! When credentials are supplied via [`with_credentials`], every request
//! includes a WS-Security `UsernameToken` with a freshly generated nonce.
//! If the device clock differs from the local clock, call [`with_utc_offset`]
//! after `GetSystemDateAndTime` to keep timestamps in sync.
//!
//! ## Testing
//!
//! Inject a custom [`Transport`] via
//! [`with_transport`] to unit-test without a real device. The builder methods
//! are order-independent: an installed transport is used whether credentials
//! were set before or after it.
//!
//! [`with_credentials`]: OnvifClient::with_credentials
//! [`with_utc_offset`]: OnvifClient::with_utc_offset
//! [`with_transport`]: OnvifClient::with_transport

use std::sync::{Arc, OnceLock};

use crate::error::OnvifError;
use crate::soap::{SoapEnvelope, WsSecurityToken};
use crate::transport::{HttpTransport, Transport};

mod device;
mod events;
mod imaging;
mod media;
mod media2;
mod ptz;
mod recording;

pub use events::notification_listener;

// ── OnvifClient ───────────────────────────────────────────────────────────────

/// Async ONVIF device client.
///
/// # Quick start
///
/// ```no_run
/// use oxvif::{OnvifClient, OnvifError};
///
/// async fn run() -> Result<(), OnvifError> {
///     let client = OnvifClient::new("http://192.168.1.100/onvif/device_service")
///         .with_credentials("admin", "password");
///
///     let caps     = client.get_capabilities().await?;
///     let media    = caps.media.url.as_deref().unwrap();
///     let profiles = client.get_profiles(media).await?;
///     let uri      = client.get_stream_uri(media, &profiles[0].token).await?;
///
///     println!("RTSP: {}", uri.uri);
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct OnvifClient {
    device_url: String,
    credentials: Option<(String, String)>,
    /// Seconds to add to local UTC when generating WS-Security timestamps.
    /// Set via [`with_utc_offset`](Self::with_utc_offset) after calling
    /// `GetSystemDateAndTime` if the device clock differs from local UTC.
    utc_offset: i64,
    /// The transport installed by [`with_transport`](Self::with_transport), if
    /// any. `None` means "use the default HTTP transport", which is built on
    /// first use by [`transport`](Self::transport) — not at construction — so
    /// that credentials set *after* this field are still picked up.
    transport: Option<Arc<dyn Transport>>,
    /// Memoised default transport, so a client does not build a fresh
    /// `HttpTransport` (and thus a fresh connection pool) per request. Any
    /// builder method that changes an input this is derived from must clear it.
    default_transport: OnceLock<Arc<dyn Transport>>,
}

impl OnvifClient {
    /// Create a client targeting the ONVIF device service at `device_url`.
    ///
    /// `device_url` is the endpoint returned by WS-Discovery or entered
    /// manually (e.g. `http://192.168.1.100/onvif/device_service`).
    pub fn new(device_url: impl Into<String>) -> Self {
        Self {
            device_url: device_url.into(),
            credentials: None,
            utc_offset: 0,
            transport: None,
            default_transport: OnceLock::new(),
        }
    }

    /// Set the credentials used for WS-Security `UsernameToken` authentication
    /// and HTTP Digest Authentication.
    ///
    /// WS-Security credentials are embedded in the SOAP header of every
    /// request.  HTTP Digest credentials are used at the transport layer to
    /// handle 401 challenges from devices that require HTTP-level
    /// authentication (ONVIF Profile T §7.1).
    ///
    /// The HTTP Digest half applies only to the **default** transport. A
    /// transport installed with [`with_transport`](Self::with_transport) is
    /// never replaced or reconfigured by this method — it owns its own
    /// transport-level authentication. The WS-Security header is added either
    /// way, so builder call order carries no meaning.
    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.credentials = Some((username.into(), password.into()));
        // The default transport derives its Digest credentials from this field,
        // so a previously memoised one is now stale.
        self.default_transport = OnceLock::new();
        self
    }

    /// Adjust the `<wsu:Created>` timestamp by `offset_secs` seconds.
    ///
    /// Obtain the offset by subtracting local UTC from the value returned by
    /// `GetSystemDateAndTime`. Ignored when no credentials are set.
    pub fn with_utc_offset(mut self, offset_secs: i64) -> Self {
        self.utc_offset = offset_secs;
        self
    }

    /// Replace the default [`HttpTransport`] with a custom implementation.
    ///
    /// Primarily used in tests to inject a mock transport without a live device.
    ///
    /// The installed transport is used for every request regardless of where
    /// this call sits in the builder chain;
    /// [`with_credentials`](Self::with_credentials) will not replace it.
    pub fn with_transport(mut self, transport: Arc<dyn Transport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Return the device service URL this client was constructed with.
    pub fn device_url(&self) -> &str {
        &self.device_url
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// The transport every request goes through: the one installed by
    /// [`with_transport`](Self::with_transport) if there is one, otherwise a
    /// default [`HttpTransport`] built on first use and memoised.
    ///
    /// Resolving here rather than in the builders is what makes the builder
    /// methods order-independent — the default transport cannot be constructed
    /// before every credential that configures it has been supplied.
    fn transport(&self) -> &Arc<dyn Transport> {
        match &self.transport {
            Some(t) => t,
            None => self.default_transport.get_or_init(|| {
                let http = HttpTransport::new();
                let http = match &self.credentials {
                    Some((user, pass)) => http.with_credentials(user, pass),
                    None => http,
                };
                Arc::new(http)
            }),
        }
    }

    fn security_token(&self) -> Option<WsSecurityToken> {
        self.credentials
            .as_ref()
            .map(|(user, pass)| WsSecurityToken::generate(user, pass, self.utc_offset))
    }

    /// Build a SOAP envelope, attach a WS-Security header if credentials are
    /// set, serialise to XML, and POST to `url`. Logs the action + body
    /// at trace level so `RUST_LOG=oxvif=trace` reveals the exact wire
    /// shape — invaluable for chasing schema-validation faults from
    /// strict cameras.
    pub(crate) async fn call(
        &self,
        url: &str,
        action: &str,
        body: &str,
    ) -> Result<String, OnvifError> {
        let mut envelope = SoapEnvelope::new(body.to_string()).with_wsa_to(url);
        if let Some(token) = self.security_token() {
            envelope = envelope.with_security(token);
        }
        let xml = envelope.build();
        tracing::trace!(action, url, body = %xml, "SOAP request");
        let resp = self.transport().soap_post(url, action, xml).await?;
        tracing::trace!(action, response = %resp, "SOAP response");
        Ok(resp)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────
//
// The module below covers only the constructor/builder surface defined in this
// file. Per-service tests are attached to the service module they exercise; see
// the `mod tests;` declaration at the foot of each of `device.rs`, `media.rs`,
// `media2.rs`, `ptz.rs`, `imaging.rs`, `events.rs` and `recording.rs`.

#[cfg(test)]
#[path = "../tests/client/mod_tests.rs"]
mod tests;
