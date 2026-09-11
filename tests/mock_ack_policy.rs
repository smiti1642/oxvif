#![cfg(feature = "mock")]

use oxvif::{
    mock::{AckOnlyOperation, MockState, MockTransport},
    soap::{SoapEnvelope, SoapError, find_response, parse_soap_body},
    transport::Transport,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const REFUSAL: &str = "This mock does not model the requested effect; explicitly opt in to acknowledgment-only behavior";
const CASES: [(&str, &str, &str); 3] = [
    (
        "http://www.onvif.org/ver10/device/wsdl/SetSystemFactoryDefault",
        "<tds:SetSystemFactoryDefault><tds:FactoryDefault>Soft</tds:FactoryDefault></tds:SetSystemFactoryDefault>",
        "SetSystemFactoryDefaultResponse",
    ),
    (
        "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/UnsubscribeRequest",
        "<n:Unsubscribe xmlns:n='http://docs.oasis-open.org/wsn/b-2'/>",
        "UnsubscribeResponse",
    ),
    (
        "http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/SetSynchronizationPointRequest",
        "<tev:SetSynchronizationPoint/>",
        "SetSynchronizationPointResponse",
    ),
];
const OPERATIONS: [AckOnlyOperation; 3] = [
    AckOnlyOperation::DeviceFactoryDefault,
    AckOnlyOperation::EventsUnsubscribe,
    AckOnlyOperation::EventsSynchronizationPoint,
];
const NAMESPACES: [&str; 3] = [
    "http://www.onvif.org/ver10/device/wsdl",
    "http://docs.oasis-open.org/wsn/b-2",
    "http://www.onvif.org/ver10/events/wsdl",
];

fn assert_ack(xml: &str, index: usize) {
    use quick_xml::{NsReader, events::Event, name::ResolveResult};
    let body = parse_soap_body(xml).unwrap();
    assert_eq!(body.children.len(), 1);
    let response = find_response(&body, CASES[index].2).unwrap();
    assert!(response.children.is_empty());
    assert_eq!(response.text(), "");
    let mut reader = NsReader::from_str(xml);
    let mut found = 0;
    loop {
        match reader.read_resolved_event().unwrap() {
            (namespace, Event::Start(element) | Event::Empty(element))
                if element.local_name().as_ref() == CASES[index].2 =>
            {
                assert!(
                    matches!(namespace, ResolveResult::Bound(ns) if ns.as_ref() == NAMESPACES[index])
                );
                found += 1;
            }
            (_, Event::Eof) => break,
            _ => {}
        }
    }
    assert_eq!(found, 1);
}

async fn check_selected(
    t: &dyn Transport,
    url: &str,
    state: &MockState,
    hooks: &AtomicUsize,
    selected: &[usize],
) {
    let before = serde_json::to_value(&*state.read()).unwrap();
    for (index, (action, payload, _)) in CASES.iter().enumerate() {
        assert_eq!(OPERATIONS[index].action(), *action);
        let xml = t
            .soap_post(url, action, SoapEnvelope::new((*payload).into()).build())
            .await
            .unwrap();
        if selected.contains(&index) {
            assert_ack(&xml, index);
        } else {
            assert_refused(&xml);
        }
        let mismatch = t
            .soap_post(
                url,
                action,
                SoapEnvelope::new("<tds:GetDeviceInformation/>".into()).build(),
            )
            .await
            .unwrap();
        assert_eq!(
            find_response(&parse_soap_body(&mismatch).unwrap(), "unused").unwrap_err(),
            SoapError::Fault {
                code: "s:Sender".into(),
                reason: "Tag Mismatch".into(),
                subcode: Some("ter:TagMismatch".into()),
                detail: None,
            }
        );
        assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
        assert_eq!(hooks.load(Ordering::SeqCst), 0);
    }
    let xml = t
        .soap_post(
            url,
            "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation",
            SoapEnvelope::new("<tds:GetDeviceInformation/>".into()).build(),
        )
        .await
        .unwrap();
    assert_eq!(
        parse_soap_body(&xml)
            .unwrap()
            .path(&["GetDeviceInformationResponse", "Manufacturer"])
            .unwrap()
            .text(),
        "oxvif-mock"
    );
    // Selection never authorizes another service or an Action alias.
    let alias = "http://www.onvif.org/ver10/media/wsdl/SetSynchronizationPoint";
    let xml = t
        .soap_post(
            url,
            alias,
            "<x:SetSynchronizationPoint xmlns:x='http://www.onvif.org/ver10/media/wsdl'/>".into(),
        )
        .await
        .unwrap();
    assert_eq!(
        find_response(&parse_soap_body(&xml).unwrap(), "unused").unwrap_err(),
        SoapError::Fault {
            code: "s:Receiver".into(),
            reason: format!("Not implemented: {alias}"),
            subcode: None,
            detail: None
        }
    );
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
    assert_eq!(hooks.load(Ordering::SeqCst), 0);
}

fn seeded_device() -> oxvif::mock::DeviceState {
    let mock = MockState::new();
    let mut state = mock.read().clone();
    state.hostname = "must-not-reset-ack-529".into();
    state
}

fn tracked_state() -> (MockState, Arc<AtomicUsize>) {
    let hooks = Arc::new(AtomicUsize::new(0));
    let seen = hooks.clone();
    let mut state = MockState::with_state(seeded_device());
    state.set_on_change(Arc::new(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
    }));
    (state, hooks)
}

#[tokio::test]
async fn selections_are_exact_additive_and_clone_local() {
    let (state, hooks) = tracked_state();
    let base = MockTransport::with_state(state);
    for (index, operation) in OPERATIONS.iter().enumerate() {
        let one = base.clone().with_acknowledgment_only(*operation);
        check_selected(&one, "http://mock", one.device(), &hooks, &[index]).await;
    }
    let all = OPERATIONS
        .iter()
        .fold(base.clone(), |t, op| t.with_acknowledgment_only(*op))
        .with_acknowledgment_only(OPERATIONS[0]);
    check_selected(&all, "http://mock", all.device(), &hooks, &[0, 1, 2]).await;
    check_default(&base, "http://mock", base.device(), &hooks).await;
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn http_selections_are_exact_and_additive() {
    #[cfg(feature = "metamorph")]
    let replay_modes = [false, true];
    #[cfg(not(feature = "metamorph"))]
    let replay_modes = [false];
    for _replay in replay_modes {
        for selected in [vec![0], vec![1], vec![2], vec![0, 1, 2, 0]] {
            let hooks = Arc::new(AtomicUsize::new(0));
            let seen = hooks.clone();
            let builder = oxvif::mock::MockServer::builder()
                .initial_state(seeded_device())
                .on_change(Arc::new(move |_| {
                    seen.fetch_add(1, Ordering::SeqCst);
                }));
            #[cfg(feature = "metamorph")]
            let builder = if _replay {
                builder.replay(oxvif::metamorph::FixtureStore::default())
            } else {
                builder
            };
            let server = selected
                .iter()
                .fold(builder, |builder, index| {
                    builder.with_acknowledgment_only(OPERATIONS[*index])
                })
                .start()
                .await
                .unwrap();
            check_selected(
                &oxvif::transport::HttpTransport::new(),
                server.device_url(),
                server.device(),
                &hooks,
                &selected,
            )
            .await;
        }
    }
}

async fn check_auth_precedence(t: &dyn Transport, url: &str) {
    let body = SoapEnvelope::new(CASES[0].1.into()).build();
    let xml = t.soap_post(url, CASES[0].0, body.clone()).await.unwrap();
    assert_eq!(
        find_response(&parse_soap_body(&xml).unwrap(), "unused").unwrap_err(),
        SoapError::Fault {
            code: "s:Receiver".into(),
            reason: "injected-ack-529".into(),
            subcode: None,
            detail: None
        }
    );
    let xml = t.soap_post(url, CASES[0].0, body).await.unwrap();
    assert_eq!(
        find_response(&parse_soap_body(&xml).unwrap(), "unused").unwrap_err(),
        SoapError::Fault {
            code: "s:Sender".into(),
            reason: "Missing Username".into(),
            subcode: Some("wsse:FailedAuthentication".into()),
            detail: None
        }
    );
}

#[tokio::test]
async fn opt_in_does_not_bypass_fault_or_auth_gates() {
    let (state, hooks) = tracked_state();
    let t = MockTransport::with_state(state)
        .with_auth()
        .with_acknowledgment_only(OPERATIONS[0]);
    let before = serde_json::to_value(&*t.device().read()).unwrap();
    t.inject_fault("SetSystemFactoryDefault", "s:Receiver", "injected-ack-529");
    check_auth_precedence(&t, "http://mock").await;
    assert_eq!(serde_json::to_value(&*t.device().read()).unwrap(), before);
    assert_eq!(hooks.load(Ordering::SeqCst), 0);
    #[cfg(feature = "mock-server")]
    {
        let server = oxvif::mock::MockServer::builder()
            .enforce_auth(true)
            .with_acknowledgment_only(OPERATIONS[0])
            .start()
            .await
            .unwrap();
        let before = serde_json::to_value(&*server.device().read()).unwrap();
        server.inject_fault("SetSystemFactoryDefault", "s:Receiver", "injected-ack-529");
        check_auth_precedence(&oxvif::transport::HttpTransport::new(), server.device_url()).await;
        assert_eq!(
            serde_json::to_value(&*server.device().read()).unwrap(),
            before
        );
    }
}

#[cfg(feature = "metamorph")]
#[tokio::test]
async fn replay_and_adapter_fallback_share_policy_without_raw_or_invalidation_changes() {
    use oxvif::{
        metamorph::{
            AdapterTransport, DeviceAdapter, DeviceIdentity, FixtureStore, MetamorphTransport,
            ReplayResponder,
        },
        mock::{RequestCtx, Responder},
    };
    use std::{collections::HashSet, sync::Mutex};
    struct RawAdapter(bool);
    #[async_trait::async_trait]
    impl DeviceAdapter for RawAdapter {
        fn identity(&self) -> DeviceIdentity {
            DeviceIdentity::default()
        }
        fn stream_uri(&self, _: &str) -> Option<String> {
            None
        }
        async fn respond_raw(&self, _: &str, _: &str) -> Option<String> {
            self.0.then(|| "<raw-ack-529/>".into())
        }
    }
    let (state, hooks) = tracked_state();
    let replay = MetamorphTransport::new(FixtureStore::default()).with_state(state);
    check_default(&replay, "http://mock", replay.device(), &hooks).await;
    let replay = OPERATIONS
        .iter()
        .fold(replay, |t, op| t.with_acknowledgment_only(*op));
    check_selected(&replay, "http://mock", replay.device(), &hooks, &[0, 1, 2]).await;

    let invalidated = Arc::new(Mutex::new(HashSet::from(["keep-ack-529".into()])));
    let responder = ReplayResponder::new(Arc::new(FixtureStore::default()), invalidated.clone());
    for (action, payload, _) in CASES {
        assert_eq!(
            responder
                .respond(&RequestCtx {
                    action,
                    base: "http://mock",
                    body: payload,
                    state: replay.device()
                })
                .await,
            None
        );
    }
    assert_eq!(
        *invalidated.lock().unwrap(),
        HashSet::from(["keep-ack-529".into()])
    );

    let adapter = AdapterTransport::new(Arc::new(RawAdapter(false)));
    check_default(
        &adapter,
        "http://mock",
        adapter.device(),
        &AtomicUsize::new(0),
    )
    .await;
    let adapter = OPERATIONS
        .iter()
        .fold(adapter, |t, op| t.with_acknowledgment_only(*op));
    for (index, (action, payload, _)) in CASES.iter().enumerate() {
        let before = serde_json::to_value(&*adapter.device().read()).unwrap();
        let xml = adapter
            .soap_post(
                "http://mock",
                action,
                SoapEnvelope::new((*payload).into()).build(),
            )
            .await
            .unwrap();
        assert_ack(&xml, index);
        assert_eq!(
            serde_json::to_value(&*adapter.device().read()).unwrap(),
            before
        );
    }
    let raw = AdapterTransport::new(Arc::new(RawAdapter(true)));
    for (action, _, _) in CASES {
        assert_eq!(
            raw.soap_post("http://mock", action, "malformed <".into())
                .await
                .unwrap(),
            "<raw-ack-529/>"
        );
    }
}

fn assert_refused(xml: &str) {
    let body = parse_soap_body(xml).unwrap();
    assert_eq!(
        find_response(&body, "unused").unwrap_err(),
        SoapError::Fault {
            code: "s:Receiver".into(),
            reason: REFUSAL.into(),
            subcode: Some("mock:UnmodeledEffect".into()),
            detail: None,
        }
    );
}

async fn check_default(t: &dyn Transport, url: &str, state: &MockState, hooks: &AtomicUsize) {
    let before = serde_json::to_value(&*state.read()).unwrap();
    let mut responses = Vec::new();
    for (action, payload, _) in CASES {
        let xml = t
            .soap_post(url, action, SoapEnvelope::new(payload.into()).build())
            .await
            .unwrap();
        responses.push(find_response(&parse_soap_body(&xml).unwrap(), "unused").unwrap_err());
        assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
        assert_eq!(hooks.load(Ordering::SeqCst), 0);
    }
    assert_eq!(
        responses,
        CASES
            .iter()
            .map(|_| SoapError::Fault {
                code: "s:Receiver".into(),
                reason: REFUSAL.into(),
                subcode: Some("mock:UnmodeledEffect".into()),
                detail: None,
            })
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn unmodeled_empty_successes_refuse_in_process() {
    let (state, hooks) = tracked_state();
    let t = MockTransport::with_state(state);
    check_default(&t, "http://mock", t.device(), &hooks).await;
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn unmodeled_empty_successes_refuse_over_http() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let seen = hooks.clone();
    let server = oxvif::mock::MockServer::builder()
        .initial_state(seeded_device())
        .on_change(Arc::new(move |_| {
            seen.fetch_add(1, Ordering::SeqCst);
        }))
        .start()
        .await
        .unwrap();
    check_default(
        &oxvif::transport::HttpTransport::new(),
        server.device_url(),
        server.device(),
        &hooks,
    )
    .await;
}
