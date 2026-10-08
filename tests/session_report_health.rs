#![cfg(feature = "health")]

use oxvif::{
    health::{ErrorClass, HealthCheck},
    session::{SessionErrorClass, SessionStageOutcome},
    transport::{Transport, TransportError},
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Refused(AtomicUsize);

#[async_trait::async_trait]
impl Transport for Refused {
    async fn soap_post(&self, _: &str, action: &str, _: String) -> Result<String, TransportError> {
        assert_eq!(
            action,
            "http://www.onvif.org/ver10/device/wsdl/GetCapabilities"
        );
        self.0.fetch_add(1, Ordering::Relaxed);
        Err(TransportError::HttpStatus {
            status: 403,
            body: "private-health-credential-body".into(),
        })
    }
}

#[tokio::test]
async fn health_returns_separate_safe_initialization_metadata_without_repeating_requests() {
    let transport = Arc::new(Refused(AtomicUsize::new(0)));
    let (report, initialization) =
        HealthCheck::new("http://userinfo:password@device.invalid/?query=secret")
            .with_transport(transport.clone())
            .run_with_session_report()
            .await;
    assert_eq!(transport.0.load(Ordering::Relaxed), 1);
    assert_eq!(report.checks.len(), 1);
    assert_eq!(report.checks[0].id, "connect");
    assert_eq!(
        report.checks[0].error.as_ref().unwrap().class,
        ErrorClass::Http
    );
    assert_eq!(
        initialization.stages[0].outcome,
        SessionStageOutcome::Skipped
    );
    assert_eq!(
        initialization.stages[1].outcome,
        SessionStageOutcome::Failed
    );
    assert_eq!(
        initialization.stages[1].error_class,
        Some(SessionErrorClass::Authentication)
    );
    assert_eq!(initialization.stages[1].http_status, Some(403));
    assert_eq!(
        initialization.stages[2].outcome,
        SessionStageOutcome::NotAttempted
    );
    for secret in [
        "userinfo",
        "password",
        "device.invalid",
        "query=",
        "private-health",
    ] {
        assert!(!format!("{initialization:?}").contains(secret));
    }
}
