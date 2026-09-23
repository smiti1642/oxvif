//! EP2 project-authored behavior tests; independent schemas inspect captured wire.
#![cfg(feature = "mock")]

use oxvif::{
    OnvifClient, OnvifError,
    mock::{MockState, MockTransport, state::PendingIoEvent},
    soap::SoapError,
    transport::Transport,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn rejected(result: Result<(), OnvifError>, reason: &str) {
    assert!(
        matches!(result, Err(OnvifError::Soap(SoapError::Fault { code, subcode: Some(subcode), reason: got, detail: None }))
        if code == "s:Sender" && subcode == "mock:RequestPolicy" && got == reason)
    );
}

async fn lifecycle(t: Arc<dyn Transport>, url: &str, state: &MockState, hooks: &AtomicUsize) {
    let c = OnvifClient::new(url).with_transport(t.clone());
    let caps = c.events_get_service_capabilities(url).await.unwrap();
    assert_eq!(caps.max_pull_points, Some(4));
    assert_eq!(caps.max_notification_producers, Some(0));
    assert_eq!(caps.ws_subscription_policy_support, Some(false));
    let a = c
        .create_pull_point_subscription(
            url,
            Some("tns1:Device/Trigger/DigitalInput"),
            Some("PT60S"),
        )
        .await
        .unwrap();
    let b = c
        .create_pull_point_subscription(url, Some("tns1:Device/Trigger/Relay"), Some("PT2M"))
        .await
        .unwrap();
    let all = c
        .create_pull_point_subscription(url, None, None)
        .await
        .unwrap();
    assert_ne!(a.reference_url, b.reference_url);
    assert_ne!(a.reference_url, all.reference_url);
    assert!(a.termination_time < b.termination_time);
    state.modify(|s| {
        s.pending_io_events = vec![
            PendingIoEvent {
                kind: "DigitalInput",
                token: "input & <北>".into(),
                logical_state: "active".into(),
            },
            PendingIoEvent {
                kind: "RelayOutput",
                token: "relay-ep2".into(),
                logical_state: "inactive".into(),
            },
        ];
    });
    let before = hooks.load(Ordering::SeqCst);
    let snapshot = serde_json::to_value(&*state.read()).unwrap();
    let ingress = format!("{:?}", state.read().pending_io_events);
    let action = "http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/PullMessagesRequest";
    for (fields, reason) in [
        ("<tev:Timeout>PT0S</tev:Timeout>", "missing request field"),
        (
            "<tev:Timeout>PT0S</tev:Timeout><tev:MessageLimit>1</tev:MessageLimit><tev:MessageLimit>2</tev:MessageLimit>",
            "duplicate request field",
        ),
        (
            "<tev:MessageLimit>1</tev:MessageLimit><tev:Timeout>PT0S</tev:Timeout>",
            "invalid operation field sequence",
        ),
        (
            "<tev:Timeout><tev:Nested/></tev:Timeout><tev:MessageLimit>1</tev:MessageLimit>",
            "expected scalar request field",
        ),
    ] {
        let xml = t
            .soap_post(
                &a.reference_url,
                action,
                oxvif::soap::SoapEnvelope::new(format!(
                    "<tev:PullMessages>{fields}</tev:PullMessages>"
                ))
                .build(),
            )
            .await
            .unwrap();
        let body = oxvif::soap::parse_soap_body(&xml).unwrap();
        assert!(
            matches!(oxvif::soap::find_response(&body, "PullMessagesResponse"), Err(SoapError::Fault { code, subcode: Some(subcode), reason: got, detail: None }) if code == "s:Sender" && subcode == "ter:InvalidArgs" && got == "Invalid Args"),
            "{reason}: {xml}"
        );
    }
    rejected(
        c.pull_messages(url, "PT0S", 1).await.map(|_| ()),
        "Unknown or expired pull-point endpoint",
    );
    rejected(
        c.renew_subscription(&a.reference_url, "PT0S")
            .await
            .map(|_| ()),
        "Pull-point lifetime must be within 1..3600 seconds",
    );
    rejected(
        c.pull_messages(&a.reference_url, "PT61S", 1)
            .await
            .map(|_| ()),
        "Pull-point timeout must be within 0..60 seconds",
    );
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), snapshot);
    assert_eq!(format!("{:?}", state.read().pending_io_events), ingress);
    assert_eq!(hooks.load(Ordering::SeqCst), before);
    let am = c.pull_messages(&a.reference_url, "PT0S", 1).await.unwrap();
    assert_eq!(am.len(), 1);
    assert_eq!(am[0].topic, "tns1:Device/Trigger/DigitalInput");
    assert_eq!(am[0].source["InputToken"], "input & <北>");
    assert_eq!(am[0].data["LogicalState"], "true");
    assert!(state.read().pending_io_events.is_empty());
    let bm = c.pull_messages(&b.reference_url, "PT0S", 1).await.unwrap();
    assert_eq!(bm.len(), 1);
    assert_eq!(bm[0].topic, "tns1:Device/Trigger/Relay");
    assert_eq!(bm[0].source["RelayToken"], "relay-ep2");
    assert_eq!(bm[0].data["LogicalState"], "false");
    // An unrelated consumer receives both, and MessageLimit leaves the second queued.
    assert_eq!(
        c.pull_messages(&all.reference_url, "PT0S", 1)
            .await
            .unwrap()[0]
            .source["InputToken"],
        "input & <北>"
    );
    assert_eq!(
        c.pull_messages(&all.reference_url, "PT0S", 999)
            .await
            .unwrap()[0]
            .source["RelayToken"],
        "relay-ep2"
    );
    assert_eq!(hooks.load(Ordering::SeqCst), before + 4);
    let expires = c
        .renew_subscription(&a.reference_url, "PT3M")
        .await
        .unwrap();
    assert!(expires > b.termination_time);
    c.unsubscribe(&a.reference_url).await.unwrap();
    let after = hooks.load(Ordering::SeqCst);
    rejected(
        c.unsubscribe(&a.reference_url).await,
        "Unknown or expired pull-point endpoint",
    );
    rejected(
        c.pull_messages(&a.reference_url, "PT0S", 1)
            .await
            .map(|_| ()),
        "Unknown or expired pull-point endpoint",
    );
    rejected(
        c.renew_subscription(&a.reference_url, "PT1M")
            .await
            .map(|_| ()),
        "Unknown or expired pull-point endpoint",
    );
    assert_eq!(hooks.load(Ordering::SeqCst), after);
    assert!(
        c.pull_messages(&b.reference_url, "PT0S", 1)
            .await
            .unwrap()
            .is_empty()
    );
    let replacement = c
        .create_pull_point_subscription(url, None, None)
        .await
        .unwrap();
    assert_ne!(a.reference_url, replacement.reference_url);
    let other = OnvifClient::new("http://mock").with_transport(Arc::new(MockTransport::new()));
    rejected(
        other
            .pull_messages(&replacement.reference_url, "PT0S", 1)
            .await
            .map(|_| ()),
        "Unknown or expired pull-point endpoint",
    );
}

#[tokio::test]
async fn subscription_lifecycle_and_fanout_in_process() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let count = hooks.clone();
    let mut state = MockState::new();
    state.set_on_change(Arc::new(move |_| {
        count.fetch_add(1, Ordering::SeqCst);
    }));
    let t = MockTransport::with_state(state);
    lifecycle(Arc::new(t.clone()), "http://mock", t.device(), &hooks).await;
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn subscription_lifecycle_and_fanout_http() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let count = hooks.clone();
    let server = oxvif::mock::MockServer::builder()
        .on_change(Arc::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        }))
        .start()
        .await
        .unwrap();
    lifecycle(
        Arc::new(oxvif::transport::HttpTransport::new()),
        server.device_url(),
        server.device(),
        &hooks,
    )
    .await;
}

async fn capacity(t: Arc<dyn Transport>, url: &str) {
    let c = OnvifClient::new(url).with_transport(t);
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let c = c.clone();
        let url = url.to_owned();
        tasks.spawn(async move { c.create_pull_point_subscription(&url, None, None).await });
    }
    let mut urls = std::collections::BTreeSet::new();
    let mut faults = 0;
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(sub) => assert!(urls.insert(sub.reference_url)),
            Err(OnvifError::Soap(SoapError::Fault {
                code,
                subcode,
                reason,
                detail,
            })) => {
                assert_eq!(code, "s:Receiver");
                assert_eq!(subcode.as_deref(), Some("mock:RequestLimit"));
                assert_eq!(reason, "Pull-point capacity exceeded");
                assert_eq!(detail, None);
                faults += 1;
            }
            other => panic!("unexpected capacity result: {other:?}"),
        }
    }
    assert_eq!(urls.len(), 4);
    assert_eq!(faults, 12);
    let first = urls.first().unwrap();
    c.unsubscribe(first).await.unwrap();
    let next = c
        .create_pull_point_subscription(url, None, None)
        .await
        .unwrap();
    assert!(!urls.contains(&next.reference_url));
    assert!(next.reference_url.ends_with("subscription_5"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn subscription_capacity_and_ids_are_atomic() {
    capacity(Arc::new(MockTransport::new()), "http://mock").await;
}
#[cfg(feature = "mock-server")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn subscription_capacity_and_ids_are_atomic_http() {
    let server = oxvif::mock::MockServer::start().await.unwrap();
    capacity(
        Arc::new(oxvif::transport::HttpTransport::new()),
        server.device_url(),
    )
    .await;
}

#[cfg(feature = "metamorph")]
#[tokio::test]
async fn built_in_replay_defers_live_subscription_lifecycle() {
    use oxvif::metamorph::{FixtureStore, RecordingTransport};
    let store = Arc::new(std::sync::Mutex::new(FixtureStore::new("ep2")));
    let recording = RecordingTransport::new(Arc::new(MockTransport::new()), store.clone());
    let recorded_client = OnvifClient::new("http://mock").with_transport(Arc::new(recording));
    let recorded = recorded_client
        .create_pull_point_subscription("http://mock", None, None)
        .await
        .unwrap();
    recorded_client
        .pull_messages(&recorded.reference_url, "PT0S", 1)
        .await
        .unwrap();
    recorded_client
        .renew_subscription(&recorded.reference_url, "PT2M")
        .await
        .unwrap();
    recorded_client
        .unsubscribe(&recorded.reference_url)
        .await
        .unwrap();
    assert_eq!(store.lock().unwrap().len(), 4);
    let transport = oxvif::metamorph::MetamorphTransport::new(store.lock().unwrap().clone());
    let c = OnvifClient::new("http://mock").with_transport(Arc::new(transport));
    let sub = c
        .create_pull_point_subscription("http://mock", None, None)
        .await
        .unwrap();
    let second = c
        .create_pull_point_subscription("http://mock", None, None)
        .await
        .unwrap();
    assert_ne!(
        sub.reference_url, second.reference_url,
        "captured create cannot replace a live allocation"
    );
    assert_eq!(
        c.pull_messages(&sub.reference_url, "PT0S", 1)
            .await
            .unwrap()[0]
            .topic,
        "tns1:VideoSource/MotionAlarm"
    );
    c.unsubscribe(&sub.reference_url).await.unwrap();
    rejected(
        c.pull_messages(&sub.reference_url, "PT0S", 1)
            .await
            .map(|_| ()),
        "Unknown or expired pull-point endpoint",
    );
}
