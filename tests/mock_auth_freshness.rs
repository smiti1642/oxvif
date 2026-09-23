//! AF1 synthetic credentials and local transports only.
#![cfg(feature = "mock")]
use base64::{Engine as _, engine::general_purpose::STANDARD};
use oxvif::{
    mock::{MockState, MockTransport},
    soap::{
        SoapEnvelope, SoapError, WsSecurityToken, find_response, parse_soap_body,
        security::{compute_digest, unix_secs_to_iso8601},
    },
    transport::Transport,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const READ: &str = "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation";
const DATE: &str = "http://www.onvif.org/ver10/device/wsdl/GetSystemDateAndTime";
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
fn body(nonce: &[u8], created: &str) -> String {
    SoapEnvelope::new("<tds:GetDeviceInformation/>".into())
        .with_security(WsSecurityToken::from_parts(
            "admin",
            STANDARD.encode(compute_digest(nonce, created, "admin")),
            STANDARD.encode(nonce),
            created,
        ))
        .build()
}
fn success(xml: &str) {
    let root = parse_soap_body(xml).unwrap();
    assert_eq!(
        find_response(&root, "GetDeviceInformationResponse")
            .unwrap()
            .child("Manufacturer")
            .unwrap()
            .text(),
        "oxvif-mock"
    );
}
fn refusal(xml: &str, reason: &str) {
    let root = parse_soap_body(xml).unwrap();
    assert_eq!(
        find_response(&root, "unused").unwrap_err(),
        SoapError::Fault {
            code: "s:Sender".into(),
            subcode: Some("wsse:FailedAuthentication".into()),
            reason: reason.into(),
            detail: None
        }
    );
    assert!(!xml.contains("af1-nonce"));
}

async fn exercise(t: Arc<dyn Transport>, url: &str, state: &MockState, hooks: &AtomicUsize) {
    let before = serde_json::to_value(&*state.read()).unwrap();
    let current = unix_secs_to_iso8601(now());
    for (offset, expected) in [
        (-3600, "Stale Created timestamp"),
        (3600, "Future Created timestamp"),
    ] {
        let invalid = body(b"af1-nonce-retry", &unix_secs_to_iso8601(now() + offset));
        refusal(&t.soap_post(url, READ, invalid).await.unwrap(), expected);
    }
    let valid = body(b"af1-nonce-retry", &current);
    success(&t.soap_post(url, READ, valid.clone()).await.unwrap());
    refusal(
        &t.soap_post(url, READ, valid.clone()).await.unwrap(),
        "Reused WS-Security nonce",
    );
    let lexical = valid.replace(
        &STANDARD.encode(b"af1-nonce-retry"),
        &format!(" \t{}\r\n", STANDARD.encode(b"af1-nonce-retry")),
    );
    refusal(
        &t.soap_post(url, READ, lexical).await.unwrap(),
        "Reused WS-Security nonce",
    );

    let exempt = body(b"af1-nonce-exempt", &current);
    let date = t
        .soap_post(
            url,
            DATE,
            exempt.replace("GetDeviceInformation", "GetSystemDateAndTime"),
        )
        .await
        .unwrap();
    assert!(
        find_response(
            &parse_soap_body(&date).unwrap(),
            "GetSystemDateAndTimeResponse"
        )
        .is_ok()
    );
    success(&t.soap_post(url, READ, exempt).await.unwrap());
    // A valid token is consumed even when the operation's payload is refused.
    let failed_operation = body(b"af1-nonce-operation", &current);
    let xml = t
        .soap_post(
            url,
            "http://www.onvif.org/ver10/device/wsdl/SetHostname",
            failed_operation.clone(),
        )
        .await
        .unwrap();
    assert!(
        matches!(find_response(&parse_soap_body(&xml).unwrap(), "unused"), Err(SoapError::Fault { subcode: Some(s), .. }) if s == "ter:TagMismatch")
    );
    refusal(
        &t.soap_post(url, READ, failed_operation).await.unwrap(),
        "Reused WS-Security nonce",
    );

    let shared = body(b"af1-nonce-concurrent", &current);
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let t = t.clone();
        let url = url.to_owned();
        let body = shared.clone();
        tasks.spawn(async move { t.soap_post(&url, READ, body).await.unwrap() });
    }
    let mut accepted = 0;
    while let Some(result) = tasks.join_next().await {
        let xml = result.unwrap();
        if find_response(
            &parse_soap_body(&xml).unwrap(),
            "GetDeviceInformationResponse",
        )
        .is_ok()
        {
            success(&xml);
            accepted += 1;
        } else {
            refusal(&xml, "Reused WS-Security nonce");
        }
    }
    assert_eq!(accepted, 1);
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
    assert_eq!(hooks.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn freshness_replay_and_exemption_in_process() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let seen = hooks.clone();
    let mut state = MockState::new();
    state.set_on_change(Arc::new(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
    }));
    let t = MockTransport::with_state(state).with_auth();
    exercise(Arc::new(t.clone()), "http://mock", t.device(), &hooks).await;
}

#[cfg(feature = "mock-server")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn freshness_replay_and_exemption_http() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let seen = hooks.clone();
    let s = oxvif::mock::MockServer::builder()
        .enforce_auth(true)
        .on_change(Arc::new(move |_| {
            seen.fetch_add(1, Ordering::SeqCst);
        }))
        .start()
        .await
        .unwrap();
    exercise(
        Arc::new(oxvif::transport::HttpTransport::new()),
        s.device_url(),
        s.device(),
        &hooks,
    )
    .await;
}

#[tokio::test]
async fn disabled_auth_and_armed_fault_do_not_consume_nonce_but_clones_share_it() {
    let base = MockTransport::new();
    let auth = base.clone().with_auth();
    let valid = body(b"af1-nonce-order", &unix_secs_to_iso8601(now()));
    for _ in 0..2 {
        success(
            &base
                .soap_post("http://mock", READ, valid.clone())
                .await
                .unwrap(),
        );
    }
    auth.inject_fault("GetDeviceInformation", "s:Receiver", "af1-armed");
    let xml = auth
        .soap_post("http://mock", READ, valid.clone())
        .await
        .unwrap();
    assert!(
        matches!(find_response(&parse_soap_body(&xml).unwrap(),"unused"),Err(SoapError::Fault { reason, .. }) if reason=="af1-armed")
    );
    success(
        &auth
            .soap_post("http://mock", READ, valid.clone())
            .await
            .unwrap(),
    );
    refusal(
        &auth
            .clone()
            .soap_post("http://mock", READ, valid.clone())
            .await
            .unwrap(),
        "Reused WS-Security nonce",
    );
    success(
        &MockTransport::new()
            .with_auth()
            .soap_post("http://mock", READ, valid)
            .await
            .unwrap(),
    );
}
