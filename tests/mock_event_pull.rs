//! Bounded queue/filter consistency controls for the immediate mock event model.
#![cfg(feature = "mock")]
use oxvif::{
    OnvifClient,
    mock::{MockState, MockTransport, state::PendingIoEvent},
    transport::Transport,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn event(kind: &'static str, token: &str) -> PendingIoEvent {
    PendingIoEvent {
        kind,
        token: token.into(),
        logical_state: "active".into(),
    }
}

async fn exercise(
    transport: Arc<dyn Transport>,
    url: &str,
    state: &MockState,
    hooks: &AtomicUsize,
) {
    let client = OnvifClient::new(url).with_transport(transport);
    let subscription = client
        .create_pull_point_subscription(url, Some("tns1:Device/Trigger/DigitalInput"), None)
        .await
        .unwrap();
    let url = subscription.reference_url.as_str();
    state.modify(|s| {
        s.pending_io_events = vec![
            event("RelayOutput", "excluded"),
            event("DigitalInput", "input & <one> \"'"),
        ];
        s.event_seq = 12;
    });
    let before = hooks.load(Ordering::SeqCst);
    let messages = client.pull_messages(url, "PT0S", 1).await.unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].topic, "tns1:Device/Trigger/DigitalInput");
    assert_eq!(
        messages[0].source.get("InputToken").map(String::as_str),
        Some("input & <one> \"'")
    );
    assert_eq!(
        messages[0].data.get("LogicalState").map(String::as_str),
        Some("true")
    );
    assert!(state.read().pending_io_events.is_empty());
    assert_eq!(state.read().event_seq, 12);
    assert_eq!(hooks.load(Ordering::SeqCst), before + 1);
    assert!(
        client
            .pull_messages(url, "PT0S", 1)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(state.read().event_seq, 13);
    assert_eq!(hooks.load(Ordering::SeqCst), before + 2);

    let other = MockTransport::new();
    let other_client = OnvifClient::new("http://mock").with_transport(Arc::new(other.clone()));
    let other_subscription = other_client
        .create_pull_point_subscription("http://mock", None, None)
        .await
        .unwrap();
    assert_eq!(
        other_client
            .pull_messages(&other_subscription.reference_url, "PT0S", 1)
            .await
            .unwrap()[0]
            .topic,
        "tns1:VideoSource/MotionAlarm"
    );
    assert_eq!(state.read().event_seq, 13);
    assert_eq!(other.device().read().event_seq, 1);
}

#[tokio::test]
async fn event_pull_filter_and_hooks_in_process() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let count = hooks.clone();
    let mut state = MockState::new();
    state.set_on_change(Arc::new(move |_| {
        count.fetch_add(1, Ordering::SeqCst);
    }));
    let mock = MockTransport::with_state(state);
    exercise(Arc::new(mock.clone()), "http://mock", mock.device(), &hooks).await;
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn event_pull_filter_and_hooks_http() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let count = hooks.clone();
    let server = oxvif::mock::MockServer::builder()
        .on_change(Arc::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        }))
        .start()
        .await
        .unwrap();
    exercise(
        Arc::new(oxvif::transport::HttpTransport::new()),
        server.device_url(),
        server.device(),
        &hooks,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn event_pull_concurrent_consumers_take_unique_queue_slots() {
    let mock = MockTransport::new();
    let client = OnvifClient::new("http://mock").with_transport(Arc::new(mock.clone()));
    let subscription = client
        .create_pull_point_subscription("http://mock", None, None)
        .await
        .unwrap();
    mock.device().modify(|s| {
        s.pending_io_events = (0..32)
            .map(|i| event("DigitalInput", &i.to_string()))
            .collect();
    });
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..32 {
        let url = subscription.reference_url.clone();
        let client = OnvifClient::new("http://mock").with_transport(Arc::new(mock.clone()));
        tasks.spawn(async move {
            client
                .pull_messages(&url, "PT0S", 1)
                .await
                .unwrap()
                .remove(0)
                .source
                .remove("InputToken")
                .unwrap()
        });
    }
    let mut tokens = std::collections::BTreeSet::new();
    while let Some(result) = tasks.join_next().await {
        assert!(tokens.insert(result.unwrap()));
    }
    assert_eq!(tokens, (0..32).map(|i| i.to_string()).collect());
    assert!(mock.device().read().pending_io_events.is_empty());
    assert_eq!(mock.device().read().event_seq, 0);
}
