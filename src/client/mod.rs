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
use crate::transport::{HttpTransport, SoapRequestOptions, Transport};

mod device;
mod events;
mod imaging;
mod media;
mod media2;
mod ptz;
mod recording;

pub use events::{notification_listener, notification_listener_with_peer};

// ── OnvifClient ───────────────────────────────────────────────────────────────

/// Async ONVIF device client.
///
/// Pass literal strings, not pre-escaped XML. Shared request serialization
/// escapes markup and writes CR/LF/tab data as character references so XML
/// normalization does not silently replace those characters. Field-specific
/// lexical constraints and device validation still apply.
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
    authentication: AuthenticationMode,
    direct_calls: bool,
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

/// Credential mechanism used by the client and its default HTTP transport.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AuthenticationMode {
    /// Send UsernameToken and respond to HTTP Digest challenges.
    #[default]
    Combined,
    /// Respond to HTTP Digest challenges only.
    Digest,
    /// Send a WS-Security UsernameToken only.
    WsSecurity,
}

impl AuthenticationMode {
    /// Whether this policy sends a UsernameToken with configured credentials.
    pub fn uses_ws_security(self) -> bool {
        self != Self::Digest
    }
    /// Whether this policy enables HTTP Digest on the built-in transport.
    pub fn uses_digest(self) -> bool {
        self != Self::WsSecurity
    }
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
            authentication: AuthenticationMode::default(),
            direct_calls: false,
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
        let credentials = (username.into(), password.into());
        if self.authentication.uses_digest() && self.credentials.as_ref() != Some(&credentials) {
            self.default_transport = OnceLock::new();
        }
        self.credentials = Some(credentials);
        self
    }

    /// Select the mechanism independently of the credential builder order.
    /// A custom transport continues to own HTTP authentication.
    pub fn with_authentication_mode(mut self, mode: AuthenticationMode) -> Self {
        if self.credentials.is_some() && self.authentication.uses_digest() != mode.uses_digest() {
            self.default_transport = OnceLock::new();
        }
        self.authentication = mode;
        self
    }

    /// Configured authentication policy, excluding credential values.
    pub fn authentication_mode(&self) -> AuthenticationMode {
        self.authentication
    }

    pub(crate) fn direct(&self) -> Self {
        let mut client = self.clone();
        client.direct_calls = true;
        client
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
                    Some((user, pass)) if self.authentication.uses_digest() => {
                        http.with_credentials(user, pass)
                    }
                    _ => http,
                };
                Arc::new(http)
            }),
        }
    }

    fn security_token(&self) -> Option<WsSecurityToken> {
        self.credentials
            .as_ref()
            .filter(|_| self.authentication.uses_ws_security())
            .map(|(user, pass)| WsSecurityToken::generate(user, pass, self.utc_offset))
    }

    /// Build a SOAP envelope, attach a WS-Security header if credentials are
    /// set, serialise to XML, and POST to `url`. Trace records only the fixed
    /// operation Action, elapsed time, serialized request/returned text byte
    /// counts and controlled transport error class. An opaque returned String
    /// is not proof of SOAP success or a particular HTTP status; use explicit
    /// capture APIs for wire-level investigation.
    pub(crate) async fn call(
        &self,
        url: &str,
        action: &str,
        body: &str,
    ) -> Result<String, OnvifError> {
        self.exchange(url, action, body, None).await
    }

    pub(crate) async fn call_addressed(
        &self,
        url: &str,
        action: &str,
        body: &str,
        parameters: &[crate::soap::qualified::QualifiedElement],
        options: SoapRequestOptions,
    ) -> Result<String, OnvifError> {
        self.exchange(url, action, body, Some((parameters, options)))
            .await
    }

    async fn exchange(
        &self,
        url: &str,
        action: &str,
        body: &str,
        addressing: Option<(
            &[crate::soap::qualified::QualifiedElement],
            SoapRequestOptions,
        )>,
    ) -> Result<String, OnvifError> {
        let mut envelope = SoapEnvelope::new(body.to_string()).with_wsa_to(url);
        if let Some(token) = self.security_token() {
            envelope = envelope.with_security(token);
        }
        let xml = match addressing {
            Some((parameters, _)) => envelope.build_addressed(action, parameters),
            None => envelope.build(),
        };
        let request_bytes = xml.len();
        let started = std::time::Instant::now();
        let result = match addressing {
            Some((_, options)) => {
                self.transport()
                    .soap_post_with_options(url, action, xml, options)
                    .await
            }
            None if self.direct_calls => {
                self.transport()
                    .soap_post_with_options(url, action, xml, SoapRequestOptions::default())
                    .await
            }
            None => self.transport().soap_post(url, action, xml).await,
        }
        .map_err(OnvifError::from);
        tracing::trace!(
            action,
            request_bytes,
            response_bytes = ?result.as_ref().ok().map(String::len),
            elapsed_ms = started.elapsed().as_millis() as u64,
            error_class = ?result.as_ref().err().map(crate::session::SessionErrorClass::from_error),
            "SOAP exchange completed"
        );
        result
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
