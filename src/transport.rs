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

use async_trait::async_trait;
use thiserror::Error;

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
pub struct HttpTransport {
    client: reqwest::Client,
}

impl HttpTransport {
    /// Create a new transport with a 10-second connection/read timeout.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to build reqwest client"),
        }
    }
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

        let response = self
            .client
            .post(url)
            .header("Content-Type", content_type)
            .header("User-Agent", concat!("oxvif/", env!("CARGO_PKG_VERSION")))
            .body(body)
            .send()
            .await?;

        let status = response.status().as_u16();
        let text = response.text().await?;

        // HTTP 500 carries a SOAP Fault body; return it as Ok so the SOAP
        // layer can parse the fault code and reason.
        if status == 200 || status == 500 {
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

    const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetCapabilities";
    const SOAP_BODY: &str = r#"<s:Envelope><s:Body><tds:GetCapabilities/></s:Body></s:Envelope>"#;

    fn sample_response() -> &'static str {
        r#"<s:Envelope><s:Body><tds:GetCapabilitiesResponse/></s:Body></s:Envelope>"#
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
    async fn test_non_soap_status_returns_err() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/onvif/device_service")
            .with_status(401)
            .with_body("Unauthorized")
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
            Err(TransportError::HttpStatus { status: 401, .. })
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
}
