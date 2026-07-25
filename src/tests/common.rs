//! Test-only helpers shared across the per-service client test modules
//! (`src/tests/client/*.rs`) and `src/tests/session_tests.rs`.
//!
//! Anything used by more than one service's tests lives here; fixtures that
//! only one service exercises stay next to that service's tests.

use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use crate::transport::{Transport, TransportError};

// ── MockTransport: returns a fixed XML string ─────────────────────────────

pub(crate) struct MockTransport {
    pub(crate) response: String,
}

#[async_trait]
impl Transport for MockTransport {
    async fn soap_post(
        &self,
        _url: &str,
        _action: &str,
        _body: String,
    ) -> Result<String, TransportError> {
        Ok(self.response.clone())
    }
}

pub(crate) fn mock(xml: &str) -> Arc<dyn Transport> {
    Arc::new(MockTransport {
        response: xml.to_string(),
    })
}

// ── RecordingTransport: records the last call for assertion ───────────────

#[derive(Default)]
pub(crate) struct Captured {
    pub(crate) url: String,
    pub(crate) action: String,
    pub(crate) body: String,
}

pub(crate) struct RecordingTransport {
    response: String,
    captured: Arc<Mutex<Captured>>,
}

impl RecordingTransport {
    pub(crate) fn new(response: &str) -> (Arc<Self>, Arc<Mutex<Captured>>) {
        let captured = Arc::new(Mutex::new(Captured::default()));
        let t = Arc::new(Self {
            response: response.to_string(),
            captured: captured.clone(),
        });
        (t, captured)
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
        let mut c = self.captured.lock().unwrap();
        c.url = url.to_string();
        c.action = action.to_string();
        c.body = body;
        Ok(self.response.clone())
    }
}

// ── ErrorTransport: always fails with a given HTTP status ─────────────────

pub(crate) struct ErrorTransport {
    pub(crate) status: u16,
}

#[async_trait]
impl Transport for ErrorTransport {
    async fn soap_post(
        &self,
        _url: &str,
        _action: &str,
        _body: String,
    ) -> Result<String, TransportError> {
        Err(TransportError::HttpStatus {
            status: self.status,
            body: format!("HTTP {}", self.status),
        })
    }
}

// ── Shared XML fixtures ───────────────────────────────────────────────────

pub(crate) fn empty_response_xml(tag: &str) -> String {
    format!(
        r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:trt="http://www.onvif.org/ver10/media/wsdl">
              <s:Body><trt:{tag}/></s:Body>
            </s:Envelope>"#
    )
}

// ── SOAP Fault ────────────────────────────────────────────────────────────

pub(crate) fn make_soap_fault_xml(code: &str, reason: &str) -> String {
    format!(
        r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
             <s:Body>
               <s:Fault>
                 <s:Code><s:Value>{code}</s:Value></s:Code>
                 <s:Reason><s:Text xml:lang="en">{reason}</s:Text></s:Reason>
               </s:Fault>
             </s:Body>
           </s:Envelope>"#
    )
}
