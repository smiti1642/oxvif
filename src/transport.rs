//! HTTP transport abstraction for SOAP over HTTP/HTTPS.
//!
//! [`Transport`] is a thin async trait that isolates the network layer from
//! SOAP encoding and ONVIF business logic. The default implementation,
//! [`HttpTransport`], uses `reqwest` with `rustls`. In unit tests you can
//! swap in any mock that implements the trait via
//! [`OnvifClient::with_transport`](crate::client::OnvifClient::with_transport).
//!
//! ## HTTP status handling
//!
//! | Status | Returned as |
//! |--------|-------------|
//! | 200    | `Ok(body)`  |
//! | 500    | `Ok(body)`  — SOAP Fault; the SOAP layer parses the fault detail |
//! | other  | `Err(TransportError::HttpStatus { status, body })` |
//!
//! ## HTTP Digest Authentication
//!
//! ONVIF Profile T §7.1 mandates HTTP Digest Authentication for clients.
//! When credentials are supplied via [`HttpTransport::with_credentials`],
//! the transport automatically handles the 401 challenge-response flow
//! using [`diqwest`].  The digest session (nonce, realm) is cached so that
//! subsequent requests avoid the extra round-trip.

use async_trait::async_trait;
use thiserror::Error;

const DEFAULT_HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

// ── Error ─────────────────────────────────────────────────────────────────────

/// Errors produced by the transport layer before SOAP parsing begins.
#[derive(Debug, Error)]
pub enum TransportError {
    /// The underlying HTTP client returned an error (connection refused, TLS
    /// handshake failure, timeout, etc.).
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// The server responded with an unexpected HTTP status code.
    ///
    /// HTTP 500 is **not** included here; it is passed up as `Ok` so the SOAP
    /// layer can extract the `<s:Fault>` detail.
    #[error("HTTP {status}: {body}")]
    HttpStatus { status: u16, body: String },
}

/// Errors returned while constructing the production HTTP transport.
#[derive(Debug, Error)]
pub enum HttpTransportBuildError {
    /// A supplied PEM bundle included private-key material. Trust stores must
    /// contain public certificates only.
    #[error("CA certificate bundle #{index} contains private-key material")]
    PrivateKeyMaterial { index: usize },

    /// A supplied PEM bundle was malformed.
    #[error("CA certificate bundle #{index} is invalid: {source}")]
    InvalidCertificate {
        index: usize,
        #[source]
        source: reqwest::Error,
    },

    /// A supplied PEM bundle contained no certificates.
    #[error("CA certificate bundle #{index} contains no certificates")]
    EmptyCertificateBundle { index: usize },

    /// The configured HTTP client could not be constructed.
    #[error("HTTP client construction failed: {0}")]
    Client(#[source] reqwest::Error),
}

// ── Trait ─────────────────────────────────────────────────────────────────────

/// Mockable HTTP transport for SOAP requests.
///
/// Implement this trait to replace the default `reqwest`-based transport,
/// for example to add retry logic, custom TLS roots, or test mocks.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a SOAP request and return the raw response body.
    ///
    /// # Arguments
    /// * `url`    – Full endpoint URL (e.g. `http://192.168.1.1/onvif/device_service`)
    /// * `action` – SOAP action URI placed in the `Content-Type` header
    /// * `body`   – Complete serialised SOAP envelope
    async fn soap_post(
        &self,
        url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError>;
}

// ── HttpTransport ─────────────────────────────────────────────────────────────

/// Production HTTP transport backed by [`reqwest`] with `rustls`.
///
/// Optionally performs HTTP Digest Authentication (RFC 7616) when credentials
/// are provided.  This is required by ONVIF Profile T §7.1 and by some
/// device vendors (Hikvision, Dahua, etc.) that protect SOAP endpoints at the
/// HTTP layer in addition to WS-Security.
pub struct HttpTransport {
    client: reqwest::Client,
    /// Optional digest auth session that caches nonce/realm across requests.
    digest_session: Option<diqwest::DigestAuthSession>,
}

impl HttpTransport {
    /// Create a new transport with a 10-second connection/read timeout.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(DEFAULT_HTTP_TIMEOUT)
                .build()
                .expect("failed to build reqwest client"),
            digest_session: None,
        }
    }

    /// Merge one or more PEM certificate bundles into the platform trust
    /// roots used by this transport.
    ///
    /// Normal certificate-chain and hostname verification remain enabled. A
    /// bundle containing private-key material is rejected before construction.
    pub fn with_root_certificates_pem<I, B>(
        mut self,
        bundles: I,
    ) -> Result<Self, HttpTransportBuildError>
    where
        I: IntoIterator<Item = B>,
        B: AsRef<[u8]>,
    {
        let mut certificates = Vec::new();
        for (offset, bundle) in bundles.into_iter().enumerate() {
            let index = offset + 1;
            let pem = bundle.as_ref();
            if contains_private_key_material(pem) {
                return Err(HttpTransportBuildError::PrivateKeyMaterial { index });
            }
            let parsed = reqwest::Certificate::from_pem_bundle(pem)
                .map_err(|source| HttpTransportBuildError::InvalidCertificate { index, source })?;
            if parsed.is_empty() {
                return Err(HttpTransportBuildError::EmptyCertificateBundle { index });
            }
            certificates.extend(parsed);
        }
        self.client = reqwest::Client::builder()
            .timeout(DEFAULT_HTTP_TIMEOUT)
            .tls_certs_merge(certificates)
            .build()
            .map_err(HttpTransportBuildError::Client)?;
        Ok(self)
    }

    /// Enable HTTP Digest Authentication for all requests.
    ///
    /// When set, the transport automatically handles 401 challenges.
    /// Typically the same `(username, password)` used for WS-Security.
    /// The digest session caches the server nonce/realm so subsequent
    /// requests use preemptive auth without an extra 401 round-trip.
    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.digest_session = Some(diqwest::DigestAuthSession::new(username, password));
        self
    }
}

fn contains_private_key_material(pem: &[u8]) -> bool {
    const MARKER: &[u8] = b"PRIVATE KEY-----";
    pem.windows(MARKER.len()).any(|window| window == MARKER)
}

impl Default for HttpTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn soap_post(
        &self,
        url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        // ONVIF spec §5.2: the SOAPAction is carried in the Content-Type
        // parameter rather than a separate header (SOAP 1.2 style).
        let content_type = format!("application/soap+xml; charset=utf-8; action=\"{action}\"");

        let response = if let Some(ref session) = self.digest_session {
            use diqwest::WithDigestAuth as _;

            self.client
                .post(url)
                .header("Content-Type", &content_type)
                .header("User-Agent", concat!("oxvif/", env!("CARGO_PKG_VERSION")))
                .body(body)
                .send_digest_auth(session)
                .await
                .map_err(|e| match e {
                    diqwest::error::Error::Reqwest(re) => TransportError::Http(re),
                    other => TransportError::HttpStatus {
                        status: 401,
                        body: other.to_string(),
                    },
                })?
        } else {
            // No credentials — plain request (WS-Security only).
            self.client
                .post(url)
                .header("Content-Type", &content_type)
                .header("User-Agent", concat!("oxvif/", env!("CARGO_PKG_VERSION")))
                .body(body)
                .send()
                .await?
        };

        let status = response.status().as_u16();
        let text = response.text().await?;

        // HTTP 200 is the normal success case.
        // HTTP 400 and 500 carry SOAP Fault bodies; return them as Ok so
        // the SOAP layer can parse the structured fault code and reason.
        // (Some devices return SOAP Faults as 400 instead of 500.)
        if status == 200 || status == 400 || status == 500 {
            Ok(text)
        } else {
            Err(TransportError::HttpStatus { status, body: text })
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_rustls::{
        TlsAcceptor,
        rustls::{ServerConfig, pki_types::PrivatePkcs8KeyDer},
    };

    const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetCapabilities";
    const SOAP_BODY: &str = r#"<s:Envelope><s:Body><tds:GetCapabilities/></s:Body></s:Envelope>"#;

    fn sample_response() -> &'static str {
        r#"<s:Envelope><s:Body><tds:GetCapabilitiesResponse/></s:Body></s:Envelope>"#
    }

    #[test]
    fn custom_root_rejects_private_key_material() {
        let error = match HttpTransport::new().with_root_certificates_pem([
            b"-----BEGIN PRIVATE KEY-----\nsecret\n-----END PRIVATE KEY-----".as_slice(),
        ]) {
            Ok(_) => panic!("private keys must not enter a trust store"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            HttpTransportBuildError::PrivateKeyMaterial { index: 1 }
        ));
    }

    #[test]
    fn custom_root_rejects_malformed_pem() {
        let error = match HttpTransport::new()
            .with_root_certificates_pem([b"not a certificate".as_slice()])
        {
            Ok(_) => panic!("malformed PEM must fail"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            HttpTransportBuildError::InvalidCertificate { index: 1, .. }
                | HttpTransportBuildError::EmptyCertificateBundle { index: 1 }
        ));
    }

    #[tokio::test]
    async fn custom_root_is_required_and_trusted_for_a_private_https_server() {
        let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()])
            .expect("test certificate should generate");
        let certificate_pem = certified.cert.pem();
        let private_key = PrivatePkcs8KeyDer::from(certified.signing_key.serialize_der());
        let tls = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certified.cert.der().clone()], private_key.into())
            .expect("test TLS config should build");
        let acceptor = TlsAcceptor::from(std::sync::Arc::new(tls));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("TLS fixture should bind");
        let address = listener.local_addr().expect("TLS fixture address");
        let server = tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.expect("TLS client should connect");
                let Ok(mut stream) = acceptor.accept(stream).await else {
                    continue;
                };
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request).await;
                let body = sample_response();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/soap+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("TLS response should write");
                break;
            }
        });
        let url = format!("https://localhost:{}/onvif/device_service", address.port());

        let untrusted = HttpTransport::new()
            .soap_post(&url, ACTION, SOAP_BODY.to_owned())
            .await;
        assert!(matches!(untrusted, Err(TransportError::Http(_))));

        let trusted =
            match HttpTransport::new().with_root_certificates_pem([certificate_pem.as_bytes()]) {
                Ok(transport) => transport,
                Err(error) => panic!("generated CA should be valid: {error}"),
            };
        let response = trusted
            .soap_post(&url, ACTION, SOAP_BODY.to_owned())
            .await
            .expect("custom CA should establish HTTPS");
        assert_eq!(response, sample_response());
        server.await.expect("TLS fixture should finish");
    }

    #[tokio::test]
    async fn test_200_returns_body() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/onvif/device_service")
            .with_status(200)
            .with_header("content-type", "application/soap+xml; charset=utf-8")
            .with_body(sample_response())
            .create_async()
            .await;

        let t = HttpTransport::new();
        let result = t
            .soap_post(
                &format!("{}/onvif/device_service", server.url()),
                ACTION,
                SOAP_BODY.to_string(),
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), sample_response());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_500_returns_ok_for_soap_fault() {
        let fault_xml = r#"<s:Envelope><s:Body><s:Fault><s:Code><s:Value>s:Sender</s:Value></s:Code></s:Fault></s:Body></s:Envelope>"#;

        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/onvif/device_service")
            .with_status(500)
            .with_body(fault_xml)
            .create_async()
            .await;

        let t = HttpTransport::new();
        let result = t
            .soap_post(
                &format!("{}/onvif/device_service", server.url()),
                ACTION,
                SOAP_BODY.to_string(),
            )
            .await;

        assert!(
            result.is_ok(),
            "HTTP 500 should be Ok so SOAP layer can parse the Fault"
        );
        assert_eq!(result.unwrap(), fault_xml);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_400_returns_ok_for_soap_fault() {
        let fault_xml = r#"<s:Envelope><s:Body><s:Fault><s:Code><s:Value>s:Sender</s:Value></s:Code></s:Fault></s:Body></s:Envelope>"#;

        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/onvif/device_service")
            .with_status(400)
            .with_body(fault_xml)
            .create_async()
            .await;

        let t = HttpTransport::new();
        let result = t
            .soap_post(
                &format!("{}/onvif/device_service", server.url()),
                ACTION,
                SOAP_BODY.to_string(),
            )
            .await;

        assert!(
            result.is_ok(),
            "HTTP 400 should be Ok so SOAP layer can parse the Fault"
        );
        assert_eq!(result.unwrap(), fault_xml);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_non_soap_status_returns_err() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/onvif/device_service")
            .with_status(403)
            .with_body("Forbidden")
            .create_async()
            .await;

        let t = HttpTransport::new();
        let result = t
            .soap_post(
                &format!("{}/onvif/device_service", server.url()),
                ACTION,
                SOAP_BODY.to_string(),
            )
            .await;

        assert!(matches!(
            result,
            Err(TransportError::HttpStatus { status: 403, .. })
        ));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_content_type_contains_action() {
        let expected_ct = format!("application/soap+xml; charset=utf-8; action=\"{ACTION}\"");

        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/onvif/device_service")
            .match_header("content-type", expected_ct.as_str())
            .with_status(200)
            .with_body(sample_response())
            .create_async()
            .await;

        let t = HttpTransport::new();
        let _ = t
            .soap_post(
                &format!("{}/onvif/device_service", server.url()),
                ACTION,
                SOAP_BODY.to_string(),
            )
            .await;

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_body_is_sent_as_is() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/onvif/device_service")
            .match_body(SOAP_BODY)
            .with_status(200)
            .with_body(sample_response())
            .create_async()
            .await;

        let t = HttpTransport::new();
        let _ = t
            .soap_post(
                &format!("{}/onvif/device_service", server.url()),
                ACTION,
                SOAP_BODY.to_string(),
            )
            .await;

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_digest_auth_handles_401_challenge() {
        let mut server = mockito::Server::new_async().await;

        // First request returns 401 with digest challenge.
        let _challenge = server
            .mock("POST", "/onvif/device_service")
            .with_status(401)
            .with_header(
                "WWW-Authenticate",
                r#"Digest realm="ONVIF", nonce="abc123", qop="auth""#,
            )
            .create_async()
            .await;

        // Second request (with Authorization header) returns 200.
        let _success = server
            .mock("POST", "/onvif/device_service")
            .match_header("Authorization", mockito::Matcher::Regex("Digest ".into()))
            .with_status(200)
            .with_body(sample_response())
            .create_async()
            .await;

        let t = HttpTransport::new().with_credentials("admin", "password");
        let result = t
            .soap_post(
                &format!("{}/onvif/device_service", server.url()),
                ACTION,
                SOAP_BODY.to_string(),
            )
            .await;

        assert!(result.is_ok(), "digest auth should succeed: {result:?}");
        assert_eq!(result.unwrap(), sample_response());
    }

    #[tokio::test]
    async fn test_no_digest_credentials_passes_through_401() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/onvif/device_service")
            .with_status(401)
            .with_body("Unauthorized")
            .create_async()
            .await;

        // No credentials — 401 should be returned as error, not retried.
        let t = HttpTransport::new();
        let result = t
            .soap_post(
                &format!("{}/onvif/device_service", server.url()),
                ACTION,
                SOAP_BODY.to_string(),
            )
            .await;

        assert!(matches!(
            result,
            Err(TransportError::HttpStatus { status: 401, .. })
        ));
        mock.assert_async().await;
    }
}
