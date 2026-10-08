//! Isolated tracing process: concurrent client tests must not race callsite registration.
use oxvif::{OnvifClient, transport::Transport};

type TraceFields = std::collections::BTreeMap<String, String>;

#[derive(Clone, Default)]
struct TraceRecorder(std::sync::Arc<std::sync::Mutex<Vec<TraceFields>>>);

impl tracing::Subscriber for TraceRecorder {
    fn register_callsite(
        &self,
        _: &'static tracing::Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        tracing::subscriber::Interest::sometimes()
    }
    fn max_level_hint(&self) -> Option<tracing::metadata::LevelFilter> {
        Some(tracing::metadata::LevelFilter::TRACE)
    }
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        metadata.target().starts_with("oxvif::client")
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        struct Values(std::collections::BTreeMap<String, String>);
        impl tracing::field::Visit for Values {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                self.0.insert(field.name().into(), format!("{value:?}"));
            }
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                self.0.insert(field.name().into(), value.into());
            }
            fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
                self.0.insert(field.name().into(), value.to_string());
            }
        }
        let mut values = Values(Default::default());
        event.record(&mut values);
        self.0.lock().unwrap().push(values.0);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn soap_trace_contains_only_metadata_for_success_and_transport_failure() {
    struct PrivateError;
    #[async_trait::async_trait]
    impl Transport for PrivateError {
        async fn soap_post(
            &self,
            _: &str,
            _: &str,
            _: String,
        ) -> Result<String, oxvif::transport::TransportError> {
            Err(oxvif::transport::TransportError::HttpStatus {
                status: 403,
                body: "private-http-error-body".into(),
            })
        }
    }
    let recorder = TraceRecorder::default();
    let _guard = tracing::subscriber::set_default(recorder.clone());
    let response = "<Envelope><Body><GetHostnameResponse><HostnameInformation><FromDHCP>false</FromDHCP><Name>private-device-hostname</Name></HostnameInformation></GetHostnameResponse></Body></Envelope>";
    struct Reply {
        body: &'static str,
        bytes: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }
    #[async_trait::async_trait]
    impl Transport for Reply {
        async fn soap_post(
            &self,
            _: &str,
            _: &str,
            body: String,
        ) -> Result<String, oxvif::transport::TransportError> {
            self.bytes
                .store(body.len(), std::sync::atomic::Ordering::Relaxed);
            Ok(self.body.into())
        }
    }
    let request = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let transport = std::sync::Arc::new(Reply {
        body: response,
        bytes: request.clone(),
    });
    let client =
        OnvifClient::new("http://url-user:url-password@host.invalid/onvif?query-token=private")
            .with_credentials("soap-user-private", "soap-password-private")
            .with_transport(transport);
    assert_eq!(
        client.get_hostname().await.unwrap().name.as_deref(),
        Some("private-device-hostname")
    );
    let request_bytes = request.load(std::sync::atomic::Ordering::Relaxed);
    let client =
        OnvifClient::new("http://url-user:url-password@host.invalid/onvif?query-token=private")
            .with_transport(std::sync::Arc::new(PrivateError));
    assert!(client.get_hostname().await.is_err());
    let events = recorder.0.lock().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0]["action"],
        "http://www.onvif.org/ver10/device/wsdl/GetHostname"
    );
    assert_eq!(events[0]["request_bytes"], request_bytes.to_string());
    assert_eq!(
        events[0]["response_bytes"],
        format!("Some({})", response.len())
    );
    assert_eq!(events[0]["error_class"], "None");
    assert_eq!(events[1]["error_class"], "Some(Authentication)");
    assert_eq!(events[1]["response_bytes"], "None");
    assert!(
        events
            .iter()
            .all(|event| event["elapsed_ms"].parse::<u64>().is_ok())
    );
    let rendered = format!("{events:?}");
    for secret in [
        "url-user",
        "url-password",
        "host.invalid",
        "query-token",
        "soap-user-private",
        "soap-password-private",
        "private-device-hostname",
        "private-http-error-body",
        "<Envelope",
        "<s:Envelope",
    ] {
        assert!(
            !rendered.contains(secret),
            "trace retained private wire material"
        );
    }
}
