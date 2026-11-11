//! Scripted and real HTTP controls for I02 through I06.
use async_trait::async_trait;
use oxvif::{
    OnvifClient,
    transport::{Transport, TransportError},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

#[path = "interoperability/authentication.rs"]
mod authentication;
#[path = "interoperability/events.rs"]
mod events;
#[path = "interoperability/media.rs"]
mod media;

const DEVICE: &str = "http://camera.test/device";
const SOAP: &str = "http://www.w3.org/2003/05/soap-envelope";
const WSA: &str = "http://www.w3.org/2005/08/addressing";
const SCHEMA: &str = "http://www.onvif.org/ver10/schema";
const EVENT_NS: &str = "http://www.onvif.org/ver10/events/wsdl";
const WSNT: &str = "http://docs.oasis-open.org/wsn/b-2";

fn envelope(body: &str) -> String {
    format!(
        r#"<s:Envelope xmlns:s="{SOAP}" xmlns:a="{WSA}" xmlns:n="{WSNT}" xmlns:e="{EVENT_NS}" xmlns:t="{SCHEMA}" xmlns:d="http://www.onvif.org/ver10/device/wsdl" xmlns:m="http://www.onvif.org/ver10/media/wsdl" xmlns:m2="http://www.onvif.org/ver20/media/wsdl" xmlns:p="http://www.onvif.org/ver20/ptz/wsdl"><s:Body>{body}</s:Body></s:Envelope>"#
    )
}

enum Reply {
    Xml(String),
    Status(u16),
    Pending,
}

fn reply(body: &str) -> Reply {
    Reply::Xml(envelope(body))
}

#[derive(Clone, Debug)]
struct Call {
    url: String,
    action: String,
    body: String,
    at: tokio::time::Instant,
}

struct Script {
    steps: Mutex<VecDeque<(&'static str, Reply)>>,
    calls: Mutex<Vec<Call>>,
    changed: tokio::sync::Notify,
}

impl Script {
    fn new(steps: Vec<(&'static str, Reply)>) -> Arc<Self> {
        Arc::new(Self {
            steps: Mutex::new(steps.into()),
            calls: Mutex::new(Vec::new()),
            changed: tokio::sync::Notify::new(),
        })
    }
    fn client(self: &Arc<Self>) -> OnvifClient {
        OnvifClient::new(DEVICE).with_transport(self.clone())
    }
    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
    fn assert_done(&self) {
        assert!(self.steps.lock().unwrap().is_empty(), "unconsumed replies");
    }
    async fn until_calls(&self, count: usize) {
        loop {
            let changed = self.changed.notified();
            if self.calls.lock().unwrap().len() >= count {
                return;
            }
            changed.await;
        }
    }
}

#[async_trait]
impl Transport for Script {
    async fn soap_post(
        &self,
        url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        if let Some(directory) = std::env::var_os("OXVIF_INTEROP_WIRE") {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let index = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::path::PathBuf::from(directory).join(format!("{index:03}.xml"));
            std::fs::write(path, &body).unwrap();
        }
        self.calls.lock().unwrap().push(Call {
            url: url.into(),
            action: action.into(),
            body,
            at: tokio::time::Instant::now(),
        });
        self.changed.notify_one();
        let (expected, response) = self
            .steps
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected SOAP request");
        assert_eq!(action, expected);
        match response {
            Reply::Xml(xml) => Ok(xml),
            Reply::Status(status) => Err(TransportError::HttpStatus {
                status,
                body: "fixture-status-body".into(),
            }),
            Reply::Pending => std::future::pending().await,
        }
    }
}
