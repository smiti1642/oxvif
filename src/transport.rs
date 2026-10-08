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
//! | 400    | `Ok(body)`  — some devices return SOAP Sender faults this way |
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

/// Default maximum accumulated SOAP response body, in bytes (4 MiB).
///
/// This is a workload limit, not a claim about any device's largest response.
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// HTTP deadline and SOAP response limits used by [`HttpTransport`].
///
/// The deadline applies to each HTTP request, including body reads. Digest may
/// perform more than one HTTP request; callers may impose an outer operation
/// deadline. Limits remain unchanged when credentials or private roots are set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HttpTransportOptions {
    /// Positive HTTP request deadline; defaults to ten seconds.
    pub timeout: std::time::Duration,
    /// Positive maximum accumulated response bytes, before UTF-8 decoding.
    /// Applies to successful responses, SOAP faults and HTTP error bodies.
    pub max_response_bytes: usize,
}

impl Default for HttpTransportOptions {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_HTTP_TIMEOUT,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        }
    }
}

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

    /// The response exceeded the configured body limit. No body is retained.
    /// This is a local resource refusal, not a device HTTP status or a
    /// transient error that should be retried automatically.
    #[error("SOAP response exceeds the {limit} byte limit")]
    ResponseTooLarge { limit: usize },
}

/// Errors returned while constructing the production HTTP transport.
#[derive(Debug, Error)]
pub enum HttpTransportBuildError {
    /// A zero deadline would disable the intended positive time budget.
    #[error("HTTP timeout must be greater than zero")]
    InvalidTimeout,

    /// A SOAP body limit must be positive.
    #[error("SOAP response byte limit must be greater than zero")]
    InvalidResponseLimit,

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
    options: HttpTransportOptions,
    root_certificates: Vec<reqwest::Certificate>,
    /// Optional digest auth session that caches nonce/realm across requests.
    digest_session: Option<diqwest::DigestAuthSession>,
}

impl HttpTransport {
    /// Create a transport with a ten-second HTTP deadline and 4 MiB body limit.
    pub fn new() -> Self {
        Self::from_options(HttpTransportOptions::default()).expect("failed to build reqwest client")
    }

    /// Construct a transport with positive deadline and response-byte limits.
    ///
    /// # Errors
    /// Returns a construction error for invalid options or HTTP client setup.
    pub fn from_options(options: HttpTransportOptions) -> Result<Self, HttpTransportBuildError> {
        Ok(Self {
            client: build_http_client(options, &[])?,
            options,
            root_certificates: Vec::new(),
            digest_session: None,
        })
    }

    /// Change the deadline and byte limit, preserving credentials and CA roots.
    ///
    /// # Errors
    /// Returns a construction error for invalid options or HTTP client setup.
    pub fn with_options(
        mut self,
        options: HttpTransportOptions,
    ) -> Result<Self, HttpTransportBuildError> {
        self.client = build_http_client(options, &self.root_certificates)?;
        self.options = options;
        Ok(self)
    }

    /// Return the current HTTP deadline and SOAP response-byte limits.
    pub fn options(&self) -> HttpTransportOptions {
        self.options
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
        self.client = build_http_client(self.options, &certificates)?;
        self.root_certificates = certificates;
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

fn build_http_client(
    options: HttpTransportOptions,
    roots: &[reqwest::Certificate],
) -> Result<reqwest::Client, HttpTransportBuildError> {
    if options.timeout.is_zero() {
        return Err(HttpTransportBuildError::InvalidTimeout);
    }
    if options.max_response_bytes == 0 {
        return Err(HttpTransportBuildError::InvalidResponseLimit);
    }
    reqwest::Client::builder()
        .timeout(options.timeout)
        .tls_certs_merge(roots.iter().cloned())
        .build()
        .map_err(HttpTransportBuildError::Client)
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

        let mut response = if let Some(ref session) = self.digest_session {
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
        let limit = self.options.max_response_bytes;
        if response
            .content_length()
            .is_some_and(|length| length > limit as u64)
        {
            return Err(TransportError::ResponseTooLarge { limit });
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if chunk.len() > limit - bytes.len() {
                return Err(TransportError::ResponseTooLarge { limit });
            }
            bytes.extend_from_slice(&chunk);
        }
        // Preserve the default transport's UTF-8 replacement decoding. Do not
        // let downstream reqwest/charset feature unification change SOAP text
        // interpretation. The cap counts received bytes, not expanded text.
        let text = String::from_utf8_lossy(&bytes).into_owned();

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
    fn options_reject_zero_limits_and_keep_positive_values() {
        let options = HttpTransportOptions {
            timeout: std::time::Duration::ZERO,
            ..HttpTransportOptions::default()
        };
        assert!(matches!(
            HttpTransport::from_options(options).err().unwrap(),
            HttpTransportBuildError::InvalidTimeout
        ));
        let options = HttpTransportOptions {
            max_response_bytes: 0,
            ..HttpTransportOptions::default()
        };
        assert!(matches!(
            HttpTransport::new().with_options(options).err().unwrap(),
            HttpTransportBuildError::InvalidResponseLimit
        ));
        let options = HttpTransportOptions {
            timeout: std::time::Duration::from_secs(23),
            max_response_bytes: 517,
        };
        let t = HttpTransport::from_options(options).unwrap();
        assert_eq!(t.options(), options);
        assert_eq!(
            HttpTransport::new().options(),
            HttpTransportOptions::default()
        );
    }

    // A one-request wire fixture: no mock library inserts Content-Length or
    // hides framing, so unknown-length and chunked bodies exercise real reads.
    async fn wire_response(response: String) -> (String, tokio::task::JoinHandle<()>) {
        wire_response_bytes(response.into_bytes()).await
    }

    async fn wire_response_bytes(response: Vec<u8>) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            loop {
                let count = stream.read(&mut buffer).await.unwrap();
                assert_ne!(count, 0, "request ended before its header");
                request.extend_from_slice(&buffer[..count]);
                if request.windows(4).any(|window| window == b"\r\n\r\n")
                    && request.ends_with(SOAP_BODY.as_bytes())
                {
                    break;
                }
                assert!(request.len() < 65_536);
            }
            assert!(request.starts_with(b"POST /onvif/device_service HTTP/1.1\r\n"));
            stream.write_all(&response).await.unwrap();
            stream.shutdown().await.unwrap();
        });
        (format!("http://{address}/onvif/device_service"), task)
    }

    fn capped_transport(limit: usize) -> HttpTransport {
        HttpTransport::from_options(HttpTransportOptions {
            max_response_bytes: limit,
            ..HttpTransportOptions::default()
        })
        .unwrap()
    }

    #[tokio::test]
    async fn exact_body_cap_preserves_success_fault_and_http_error_bodies() {
        for status in [200, 400, 500, 403] {
            let (url, server) = wire_response(format!(
                "HTTP/1.1 {status} Test\r\nContent-Length: 7\r\nConnection: close\r\n\r\npayload"
            ))
            .await;
            let result = capped_transport(7)
                .soap_post(&url, ACTION, SOAP_BODY.into())
                .await;
            if status == 403 {
                match result.unwrap_err() {
                    TransportError::HttpStatus { status, body } => {
                        assert_eq!(status, 403);
                        assert_eq!(body, "payload");
                    }
                    other => panic!("wrong error: {other}"),
                }
            } else {
                assert_eq!(result.unwrap(), "payload");
            }
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn body_cap_covers_declared_unknown_and_chunked_error_responses() {
        for status in [200, 400, 500, 403] {
            for framing in [
                "Content-Length: 8\r\nConnection: close\r\n\r\nprivateX",
                "Connection: close\r\n\r\nprivateX",
                "Transfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\npriv\r\n4\r\nateX\r\n0\r\n\r\n",
                // Declared oversize is refused even when the body is shorter.
                "Content-Length: 99\r\nConnection: close\r\n\r\nx",
            ] {
                let (url, server) =
                    wire_response(format!("HTTP/1.1 {status} Test\r\n{framing}")).await;
                let error = capped_transport(7)
                    .soap_post(&url, ACTION, SOAP_BODY.into())
                    .await
                    .unwrap_err();
                assert!(matches!(
                    error,
                    TransportError::ResponseTooLarge { limit: 7 }
                ));
                assert_eq!(error.to_string(), "SOAP response exceeds the 7 byte limit");
                assert!(!error.to_string().contains("private"));
                server.await.unwrap();
            }
        }
    }

    #[tokio::test]
    async fn body_cap_counts_utf8_bytes_and_keeps_replacement_decoding() {
        let (url, server) = wire_response(
            "HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\n攝影機".into(),
        )
        .await;
        assert_eq!(
            capped_transport(9)
                .soap_post(&url, ACTION, SOAP_BODY.into())
                .await
                .unwrap(),
            "攝影機"
        );
        server.await.unwrap();
        let mut response = b"HTTP/1.1 200 OK\r\nContent-Type: text/xml; charset=iso-8859-1\r\nContent-Length: 1\r\nConnection: close\r\n\r\n".to_vec();
        response.push(0xff);
        let (url, server) = wire_response_bytes(response).await;
        assert_eq!(
            capped_transport(1)
                .soap_post(&url, ACTION, SOAP_BODY.into())
                .await
                .unwrap(),
            "\u{fffd}",
            "limit counts received bytes; SOAP decoding is UTF-8 regardless of charset"
        );
        server.await.unwrap();
        let (url, server) = wire_response(
            "HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\n攝影機".into(),
        )
        .await;
        assert!(matches!(
            capped_transport(8)
                .soap_post(&url, ACTION, SOAP_BODY.into())
                .await
                .unwrap_err(),
            TransportError::ResponseTooLarge { limit: 8 }
        ));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn configured_deadline_cancels_body_reads_and_releases_the_connection() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            // Drain the request body before checking EOF caused by cancellation.
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            loop {
                let count = stream.read(&mut buffer).await.unwrap();
                assert_ne!(count, 0);
                request.extend_from_slice(&buffer[..count]);
                if request.ends_with(SOAP_BODY.as_bytes()) {
                    break;
                }
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nx")
                .await
                .unwrap();
            let count =
                tokio::time::timeout(std::time::Duration::from_secs(2), stream.read(&mut buffer))
                    .await
                    .unwrap()
                    .unwrap();
            assert_eq!(
                count, 0,
                "timeout must close the unfinished body connection"
            );
        });
        let transport = HttpTransport::from_options(HttpTransportOptions {
            timeout: std::time::Duration::from_millis(100),
            ..HttpTransportOptions::default()
        })
        .unwrap();
        let error = transport
            .soap_post(
                &format!("http://{address}/onvif/device_service"),
                ACTION,
                SOAP_BODY.into(),
            )
            .await
            .unwrap_err();
        match error {
            TransportError::Http(error) => assert!(error.is_timeout()),
            other => panic!("wrong timeout error: {other}"),
        }
        server.await.unwrap();
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

        let options = HttpTransportOptions {
            timeout: std::time::Duration::from_secs(21),
            max_response_bytes: sample_response().len(),
        };
        let trusted = match HttpTransport::from_options(options)
            .unwrap()
            .with_root_certificates_pem([certificate_pem.as_bytes()])
        {
            Ok(transport) => transport,
            Err(error) => panic!("generated CA should be valid: {error}"),
        };
        // Rebuilding after roots must also retain trust and limits.
        let trusted = trusted
            .with_credentials("admin", "password")
            .with_options(options)
            .unwrap();
        assert_eq!(trusted.options(), options);
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

    #[tokio::test]
    async fn digest_success_body_is_capped_after_exactly_one_challenge() {
        let mut server = mockito::Server::new_async().await;
        let challenge = server
            .mock("POST", "/onvif/device_service")
            .match_header("authorization", mockito::Matcher::Missing)
            .with_status(401)
            .with_header(
                "WWW-Authenticate",
                r#"Digest realm="ONVIF", nonce="cap-8821", qop="auth""#,
            )
            .expect(1)
            .create_async()
            .await;
        let success = server
            .mock("POST", "/onvif/device_service")
            .match_header("authorization", mockito::Matcher::Regex("Digest ".into()))
            .with_status(200)
            .with_body("privateX")
            .expect(1)
            .create_async()
            .await;
        let error = HttpTransport::new()
            .with_credentials("admin", "password")
            .with_options(HttpTransportOptions {
                max_response_bytes: 7,
                ..HttpTransportOptions::default()
            })
            .unwrap()
            .soap_post(
                &format!("{}/onvif/device_service", server.url()),
                ACTION,
                SOAP_BODY.into(),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            TransportError::ResponseTooLarge { limit: 7 }
        ));
        challenge.assert_async().await;
        success.assert_async().await;
    }
}
