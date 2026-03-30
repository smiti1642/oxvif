use async_trait::async_trait;
use thiserror::Error;

// ── 錯誤型別 ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// 非 200/500 的 HTTP 狀態（500 含 SOAP Fault，讓 SOAP 層自己解析）
    #[error("HTTP {status}: {body}")]
    HttpStatus { status: u16, body: String },
}

// ── Trait ──────────────────────────────────────────────────────────────────────

/// 可被 mock 替換的 HTTP 傳輸層
#[async_trait]
pub trait Transport: Send + Sync {
    async fn soap_post(
        &self,
        url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError>;
}

// ── 真實實作 ────────────────────────────────────────────────────────────────────

pub struct HttpTransport {
    client: reqwest::Client,
}

impl HttpTransport {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
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
        // 原始 C 碼格式：Content-Type 裡夾帶 action
        let content_type = format!("application/soap+xml; charset=utf-8; action=\"{action}\"");

        let response = self
            .client
            .post(url)
            .header("Content-Type", content_type)
            .header("User-Agent", "rust_ht onvif client/0.1")
            .body(body)
            .send()
            .await?;

        let status = response.status().as_u16();
        let text = response.text().await?;

        // 200 = 正常回應；500 = SOAP Fault（讓 SOAP 層解析）；其他 = 真正的 HTTP 錯誤
        if status == 200 || status == 500 {
            Ok(text)
        } else {
            Err(TransportError::HttpStatus { status, body: text })
        }
    }
}

// ── 單元測試 ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetCapabilities";
    const SOAP_BODY: &str = r#"<s:Envelope><s:Body><tds:GetCapabilities/></s:Body></s:Envelope>"#;

    fn sample_response() -> &'static str {
        r#"<s:Envelope><s:Body><tds:GetCapabilitiesResponse/></s:Body></s:Envelope>"#
    }

    // ── 200 回應正常傳回 ────────────────────────────────────────────────────────
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

    // ── 500 含 SOAP Fault，傳回 Ok 讓 SOAP 層處理 ──────────────────────────────
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
            "500 should be Ok so SOAP layer can parse Fault"
        );
        assert_eq!(result.unwrap(), fault_xml);
        mock.assert_async().await;
    }

    // ── 非 200/500 的狀態碼回傳 Err ────────────────────────────────────────────
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

    // ── Content-Type header 包含 action ────────────────────────────────────────
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

    // ── request body 原封不動送出 ──────────────────────────────────────────────
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
