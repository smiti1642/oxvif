//! Opt-in raw-SOAP capture for the checks that fail (see
//! [`HealthCheck::with_capture`](super::HealthCheck::with_capture)).
//!
//! [`CaptureTransport`] wraps the real transport and records the
//! request/response of every SOAP call that **failed** — a transport error or a
//! SOAP Fault response — so the report can carry the raw evidence a maintainer
//! needs to see *why* a brand rejected a call. Successful calls are not stored:
//! it keeps the capture small and, crucially, keeps the credential-bearing
//! happy-path requests out of the report. The request that *is* stored has its
//! WS-Security `Password`/`Nonce` blanked ([`crate::redact::redact_credentials`]).

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::report::CapturedExchange;
use crate::redact::redact_credentials;
use crate::soap::parse_soap_body;
use crate::transport::{Transport, TransportError};

/// Transport tap that records only the failing SOAP exchanges. Wrap the real
/// transport; drain with [`take`](Self::take) after the run.
pub(super) struct CaptureTransport {
    inner: Arc<dyn Transport>,
    seen: Mutex<Vec<CapturedExchange>>,
}

impl CaptureTransport {
    pub(super) fn new(inner: Arc<dyn Transport>) -> Self {
        Self {
            inner,
            seen: Mutex::new(Vec::new()),
        }
    }

    /// Drain the captured exchanges.
    pub(super) fn take(&self) -> Vec<CapturedExchange> {
        std::mem::take(&mut self.seen.lock().unwrap())
    }

    fn record(&self, action: &str, request: String, response: String, http_status: Option<u16>) {
        let action = action.rsplit('/').next().unwrap_or(action).to_string();
        let mut g = self.seen.lock().unwrap();
        // Keep the latest failing exchange per action: a functional check and the
        // parse-coverage pass can hit the same op, and the fault is identical, so
        // storing both would just be noise.
        g.retain(|e| e.action != action);
        g.push(CapturedExchange {
            action,
            request,
            response,
            http_status,
        });
    }
}

#[async_trait]
impl Transport for CaptureTransport {
    async fn soap_post(
        &self,
        url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        // Redact before the move, so the stored request never holds the digest.
        let request = redact_credentials(&body);
        match self.inner.soap_post(url, action, body).await {
            Ok(resp) => {
                if is_soap_fault(&resp) {
                    self.record(action, request, resp.clone(), None);
                }
                Ok(resp)
            }
            Err(e) => {
                let http_status = match &e {
                    TransportError::HttpStatus { status, .. } => Some(*status),
                    TransportError::Http(_) => None,
                };
                self.record(action, request, e.to_string(), http_status);
                Err(e)
            }
        }
    }
}

/// `true` when a `200`/`400`/`500` response body is a SOAP Fault (the transport
/// collapses fault statuses to `Ok`, so a fault is only visible in the body).
fn is_soap_fault(body: &str) -> bool {
    parse_soap_body(body)
        .ok()
        .is_some_and(|b| b.child("Fault").is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_soap_fault_body() {
        let fault = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
            <s:Body><s:Fault><s:Code><s:Value>s:Sender</s:Value></s:Code></s:Fault></s:Body>
          </s:Envelope>"#;
        let ok = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
            <s:Body><trt:GetStreamUriResponse/></s:Body></s:Envelope>"#;
        assert!(is_soap_fault(fault));
        assert!(!is_soap_fault(ok));
    }

    // A fake inner transport: faults for one action, succeeds for another.
    struct FaultyInner;
    #[async_trait]
    impl Transport for FaultyInner {
        async fn soap_post(
            &self,
            _url: &str,
            action: &str,
            _body: String,
        ) -> Result<String, TransportError> {
            if action.ends_with("GetStreamUri") {
                Ok(
                    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"><s:Body>
                     <s:Fault><s:Code><s:Value>s:Sender</s:Value>
                       <s:Subcode><s:Value>ter:NotAuthorized</s:Value></s:Subcode></s:Code>
                     </s:Fault></s:Body></s:Envelope>"#
                        .to_string(),
                )
            } else {
                Ok(
                    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"><s:Body>
                     <trt:GetProfilesResponse/></s:Body></s:Envelope>"#
                        .to_string(),
                )
            }
        }
    }

    #[tokio::test]
    async fn captures_only_the_failing_exchange_with_redacted_request() {
        let cap = CaptureTransport::new(Arc::new(FaultyInner));
        let body = "<s:Envelope><s:Header><wsse:Password>DIGEST==</wsse:Password>\
                    </s:Header><s:Body/></s:Envelope>";
        // A successful op — not captured.
        cap.soap_post("http://d/onvif", ".../GetProfiles", body.to_string())
            .await
            .unwrap();
        // A faulting op — captured.
        cap.soap_post("http://d/onvif", ".../GetStreamUri", body.to_string())
            .await
            .unwrap();

        let got = cap.take();
        assert_eq!(got.len(), 1, "only the fault should be captured");
        assert_eq!(got[0].action, "GetStreamUri");
        assert!(got[0].response.contains("ter:NotAuthorized"));
        assert!(
            got[0].request.contains("[redacted]") && !got[0].request.contains("DIGEST=="),
            "request digest must be redacted: {}",
            got[0].request
        );
        assert!(cap.take().is_empty(), "take drains the buffer");
    }
}
