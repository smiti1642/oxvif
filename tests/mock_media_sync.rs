#![cfg(feature = "mock")]
//! Project-generated B16 controls: receipt is never evidence of media delivery.
use oxvif::{
    mock::{AckOnlyOperation, MockState, MockTransport},
    soap::parse_soap_body,
    transport::Transport,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
const NS: [&str; 2] = [
    "http://www.onvif.org/ver10/media/wsdl",
    "http://www.onvif.org/ver20/media/wsdl",
];
const OPS: [AckOnlyOperation; 2] = [
    AckOnlyOperation::MediaSynchronizationPoint,
    AckOnlyOperation::Media2SynchronizationPoint,
];
const REFUSAL: &str = "This mock does not model the requested effect; explicitly opt in to acknowledgment-only behavior";
const TOKEN: &str = " Profile<&同步 ";
const FIELD: &str = "<m:ProfileToken> Profile&lt;&amp;同步 </m:ProfileToken>";
fn wire(ns: &str, op: &str, fields: &str) -> String {
    format!(
        "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:m='{ns}'><s:Body><m:{op}>{fields}</m:{op}></s:Body></s:Envelope>"
    )
}
async fn post(t: &dyn Transport, url: &str, i: usize, fields: &str) -> String {
    t.soap_post(
        url,
        OPS[i].action(),
        wire(NS[i], "SetSynchronizationPoint", fields),
    )
    .await
    .unwrap()
}
fn fault(xml: &str, code: &str, subcodes: &[&str], reason: &str) {
    let body = parse_soap_body(xml).unwrap();
    assert_eq!(body.children.len(), 1);
    let f = &body.children[0];
    assert_eq!(f.local_name, "Fault", "{xml}");
    let mut c = f.child("Code").unwrap();
    assert_eq!(c.child("Value").unwrap().text(), code);
    for expected in subcodes {
        c = c.child("Subcode").unwrap();
        assert_eq!(c.child("Value").unwrap().text(), *expected);
    }
    assert!(c.child("Subcode").is_none());
    assert_eq!(f.path(&["Reason", "Text"]).unwrap().text(), reason);
}
fn ack(xml: &str, i: usize) {
    use quick_xml::{NsReader, events::Event, name::ResolveResult};
    let body = parse_soap_body(xml).unwrap();
    assert_eq!(body.children.len(), 1);
    assert_eq!(
        body.children[0].local_name,
        "SetSynchronizationPointResponse"
    );
    assert!(body.children[0].children.is_empty());
    assert_eq!(body.children[0].text(), "");
    let mut reader = NsReader::from_str(xml);
    let mut found = 0;
    loop {
        match reader.read_resolved_event().unwrap() {
            (ResolveResult::Bound(ns), Event::Empty(e) | Event::Start(e))
                if e.local_name().as_ref() == "SetSynchronizationPointResponse" =>
            {
                assert_eq!(ns.as_ref(), NS[i]);
                found += 1;
            }
            (_, Event::Eof) => break,
            _ => {}
        }
    }
    assert_eq!(found, 1);
}
fn seed() -> oxvif::mock::DeviceState {
    let mut state = oxvif::mock::DeviceState::default();
    state.profiles.profiles[0].token = TOKEN.into();
    state.profiles.profiles[1].token = "other".into();
    state
}
async fn contract(
    t: &dyn Transport,
    url: &str,
    state: &MockState,
    hooks: &AtomicUsize,
    selected: Option<usize>,
) {
    let before = serde_json::to_value(&*state.read()).unwrap();
    for i in 0..2 {
        let xml = post(t, url, i, FIELD).await;
        if selected != Some(i) {
            fault(&xml, "s:Receiver", &["mock:UnmodeledEffect"], REFUSAL);
            continue;
        }
        ack(&xml, i);
        ack(
            &post(t, url, i, "<m:ProfileToken>other</m:ProfileToken>").await,
            i,
        );
        ack(
            &post(
                t,
                url,
                i,
                "<m:ProfileToken><![CDATA[ Profile<&同步 ]]></m:ProfileToken>",
            )
            .await,
            i,
        );
        let alternate = format!(
            "<Envelope xmlns='http://www.w3.org/2003/05/soap-envelope'><Body><SetSynchronizationPoint xmlns='{}'><ProfileToken>other</ProfileToken></SetSynchronizationPoint></Body></Envelope>",
            NS[i]
        );
        ack(
            &t.soap_post(url, OPS[i].action(), alternate).await.unwrap(),
            i,
        );
        for fields in [
            "",
            "<m:ProfileToken/>",
            "<m:ProfileToken>other</m:ProfileToken><m:ProfileToken>other</m:ProfileToken>",
            "<m:ProfileToken><m:Nested>other</m:Nested></m:ProfileToken>",
        ] {
            fault(
                &post(t, url, i, fields).await,
                "s:Sender",
                &["ter:InvalidArgs"],
                "Invalid Args",
            );
        }
        for fields in [
            "<ProfileToken>other</ProfileToken>",
            "<m:Extension><m:ProfileToken>other</m:ProfileToken></m:Extension>",
            "<m:ProfileToken xmlns:m='urn:wrong'>other</m:ProfileToken>",
            "<m:ProfileToken extra='1'>other</m:ProfileToken>",
        ] {
            fault(
                &post(t, url, i, fields).await,
                "s:Sender",
                &["ter:TagMismatch"],
                "Tag Mismatch",
            );
        }
        for (fields, missing) in [
            ("<m:ProfileToken>absent</m:ProfileToken>", "absent"),
            (
                "<m:ProfileToken>Profile&lt;&amp;同步</m:ProfileToken>",
                "Profile<&同步",
            ),
            (
                "<m:ProfileToken>Profile&amp;lt;&amp;amp;同步</m:ProfileToken>",
                "Profile&lt;&amp;同步",
            ),
        ] {
            fault(
                &post(t, url, i, fields).await,
                "s:Sender",
                &["ter:InvalidArgVal", "ter:NoProfile"],
                &format!("Profile not found: {missing}"),
            );
        }
        let header = wire(NS[i], "SetSynchronizationPoint", "").replace(
            "<s:Body>",
            "<s:Header><m:ProfileToken>other</m:ProfileToken></s:Header><s:Body>",
        );
        fault(
            &t.soap_post(url, OPS[i].action(), header).await.unwrap(),
            "s:Sender",
            &["ter:InvalidArgs"],
            "Invalid Args",
        );
        let mismatch = wire(NS[1 - i], "SetSynchronizationPoint", FIELD);
        fault(
            &t.soap_post(url, OPS[i].action(), mismatch).await.unwrap(),
            "s:Sender",
            &["ter:TagMismatch"],
            "Tag Mismatch",
        );
    }
    // Even a selected Media receipt does not enable Events synchronization.
    let events = "http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/SetSynchronizationPointRequest";
    let xml = t
        .soap_post(
            url,
            events,
            wire(
                "http://www.onvif.org/ver10/events/wsdl",
                "SetSynchronizationPoint",
                "",
            ),
        )
        .await
        .unwrap();
    fault(&xml, "s:Receiver", &["mock:UnmodeledEffect"], REFUSAL);
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
    assert_eq!(hooks.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn media_sync_scoped_policy_in_process() {
    for selected in [None, Some(0), Some(1)] {
        let hooks = Arc::new(AtomicUsize::new(0));
        let seen = hooks.clone();
        let mut state = MockState::with_state(seed());
        state.set_on_change(Arc::new(move |_| {
            seen.fetch_add(1, Ordering::SeqCst);
        }));
        let t = MockTransport::with_state(state);
        let t = if let Some(i) = selected {
            t.with_acknowledgment_only(OPS[i])
        } else {
            t
        };
        contract(&t, "http://mock", t.device(), &hooks, selected).await;
    }
}

#[tokio::test]
async fn media_sync_unicode_token_boundaries() {
    let mut state = seed();
    state.profiles.profiles[0].token = "界".repeat(64);
    state.profiles.profiles[1].token = "界".repeat(65);
    let t = MockTransport::with_state(MockState::with_state(state))
        .with_acknowledgment_only(OPS[0])
        .with_acknowledgment_only(OPS[1]);
    let before = serde_json::to_value(&*t.device().read()).unwrap();
    for i in 0..2 {
        ack(
            &post(
                &t,
                "http://mock",
                i,
                &format!("<m:ProfileToken>{}</m:ProfileToken>", "界".repeat(64)),
            )
            .await,
            i,
        );
        fault(
            &post(
                &t,
                "http://mock",
                i,
                &format!("<m:ProfileToken>{}</m:ProfileToken>", "界".repeat(65)),
            )
            .await,
            "s:Sender",
            &["ter:InvalidArgs"],
            "Invalid Args",
        );
    }
    assert_eq!(serde_json::to_value(&*t.device().read()).unwrap(), before);
}

#[cfg(feature = "metamorph")]
#[tokio::test]
async fn media_sync_adapter_fallback_and_explicit_raw_boundary() {
    use oxvif::metamorph::{AdapterTransport, DeviceAdapter, DeviceIdentity};
    struct Adapter(bool);
    #[async_trait::async_trait]
    impl DeviceAdapter for Adapter {
        fn identity(&self) -> DeviceIdentity {
            DeviceIdentity::default()
        }
        fn stream_uri(&self, _: &str) -> Option<String> {
            None
        }
        async fn respond_raw(&self, op: &str, _: &str) -> Option<String> {
            (self.0 && op == "SetSynchronizationPoint").then(|| "<caller-owned-sync-reply/>".into())
        }
    }
    for raw in [false, true] {
        for selected in [None, Some(0), Some(1)] {
            let t = AdapterTransport::new(Arc::new(Adapter(raw)));
            let t = if let Some(i) = selected {
                t.with_acknowledgment_only(OPS[i])
            } else {
                t
            };
            let before = serde_json::to_value(&*t.device().read()).unwrap();
            for i in 0..2 {
                let xml = post(
                    &t,
                    "http://adapter",
                    i,
                    "<m:ProfileToken>Profile_1</m:ProfileToken>",
                )
                .await;
                if raw {
                    assert_eq!(xml, "<caller-owned-sync-reply/>");
                } else if selected == Some(i) {
                    ack(&xml, i);
                } else {
                    fault(&xml, "s:Receiver", &["mock:UnmodeledEffect"], REFUSAL);
                }
            }
            assert_eq!(serde_json::to_value(&*t.device().read()).unwrap(), before);
        }
    }
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn media_sync_scoped_policy_http() {
    #[cfg(feature = "metamorph")]
    let replay_modes = [false, true];
    #[cfg(not(feature = "metamorph"))]
    let replay_modes = [false];
    for _replay in replay_modes {
        for selected in [None, Some(0), Some(1)] {
            let hooks = Arc::new(AtomicUsize::new(0));
            let seen = hooks.clone();
            let b = oxvif::mock::MockServer::builder()
                .initial_state(seed())
                .on_change(Arc::new(move |_| {
                    seen.fetch_add(1, Ordering::SeqCst);
                }));
            let b = if let Some(i) = selected {
                b.with_acknowledgment_only(OPS[i])
            } else {
                b
            };
            #[cfg(feature = "metamorph")]
            let b = if _replay {
                b.replay(oxvif::metamorph::FixtureStore::default())
            } else {
                b
            };
            let s = b.start().await.unwrap();
            contract(
                &oxvif::transport::HttpTransport::new(),
                s.device_url(),
                s.device(),
                &hooks,
                selected,
            )
            .await;
        }
    }
}

#[tokio::test]
async fn media_sync_auth_and_ambiguous_snapshot_refuse() {
    for op in OPS {
        let t = MockTransport::new()
            .with_auth()
            .with_acknowledgment_only(op);
        let xml = t
            .soap_post(
                "http://mock",
                op.action(),
                wire(
                    op.action().rsplit_once('/').unwrap().0,
                    "SetSynchronizationPoint",
                    "<m:ProfileToken>Profile_1</m:ProfileToken>",
                ),
            )
            .await
            .unwrap();
        fault(
            &xml,
            "s:Sender",
            &["wsse:FailedAuthentication"],
            "Missing Username",
        );
        #[cfg(feature = "mock-server")]
        {
            let s = oxvif::mock::MockServer::builder()
                .enforce_auth(true)
                .with_acknowledgment_only(op)
                .start()
                .await
                .unwrap();
            let xml = oxvif::transport::HttpTransport::new()
                .soap_post(
                    s.device_url(),
                    op.action(),
                    wire(
                        op.action().rsplit_once('/').unwrap().0,
                        "SetSynchronizationPoint",
                        "<m:ProfileToken>Profile_1</m:ProfileToken>",
                    ),
                )
                .await
                .unwrap();
            fault(
                &xml,
                "s:Sender",
                &["wsse:FailedAuthentication"],
                "Missing Username",
            );
        }
    }
    let t = MockTransport::new().with_acknowledgment_only(OPS[0]);
    t.device().modify(|s| {
        let duplicate = s.profiles.profiles[0].clone();
        s.profiles.profiles.push(duplicate);
    });
    let before = serde_json::to_value(&*t.device().read()).unwrap();
    fault(
        &post(
            &t,
            "http://mock",
            0,
            "<m:ProfileToken>Profile_1</m:ProfileToken>",
        )
        .await,
        "s:Receiver",
        &["mock:RequestPolicy"],
        "Ambiguous mock profile identity",
    );
    assert_eq!(serde_json::to_value(&*t.device().read()).unwrap(), before);
}

#[cfg(feature = "metamorph")]
#[tokio::test]
async fn media_sync_replay_preserves_reads_but_does_not_replay_write_receipts() {
    use oxvif::metamorph::{FixtureStore, MetamorphTransport};
    for selected in [false, true] {
        let mut store = FixtureStore::new("sync-816");
        let reads: Vec<_> = NS
            .iter()
            .map(|ns| (format!("{ns}/GetProfiles"), wire(ns, "GetProfiles", "")))
            .collect();
        for (action, request) in &reads {
            store.record(action, request, "<recorded-sync-read/>");
        }
        let t = MetamorphTransport::new(store);
        let t = if selected {
            t.with_acknowledgment_only(OPS[0])
                .with_acknowledgment_only(OPS[1])
        } else {
            t
        };
        for i in 0..2 {
            let xml = post(
                &t,
                "http://mock",
                i,
                "<m:ProfileToken>Profile_1</m:ProfileToken>",
            )
            .await;
            if selected {
                ack(&xml, i)
            } else {
                fault(&xml, "s:Receiver", &["mock:UnmodeledEffect"], REFUSAL)
            }
            for (action, request) in &reads {
                assert_eq!(
                    t.soap_post("http://mock", action, request.clone())
                        .await
                        .unwrap(),
                    "<recorded-sync-read/>"
                );
            }
        }
    }
    let mut store = FixtureStore::new("sync-explicit");
    let request = wire(
        NS[0],
        "SetSynchronizationPoint",
        "<m:ProfileToken>Profile_1</m:ProfileToken>",
    );
    store.record(OPS[0].action(), &request, "<explicit-recording/>");
    let t = MetamorphTransport::new(store);
    fault(
        &t.soap_post("http://mock", OPS[0].action(), request.clone())
            .await
            .unwrap(),
        "s:Receiver",
        &["mock:UnmodeledEffect"],
        REFUSAL,
    );
    let t = t.with_acknowledgment_only(OPS[0]);
    ack(
        &t.soap_post("http://mock", OPS[0].action(), request)
            .await
            .unwrap(),
        0,
    );
}

#[tokio::test]
async fn media_sync_explicit_fault_injection_keeps_its_precedence() {
    let t = MockTransport::new();
    t.inject_fault("SetSynchronizationPoint", "s:Receiver", "injected-sync-816");
    let xml = post(
        &t,
        "http://mock",
        0,
        "<m:ProfileToken>Profile_1</m:ProfileToken>",
    )
    .await;
    fault(&xml, "s:Receiver", &[], "injected-sync-816");
    fault(
        &post(
            &t,
            "http://mock",
            0,
            "<m:ProfileToken>Profile_1</m:ProfileToken>",
        )
        .await,
        "s:Receiver",
        &["mock:UnmodeledEffect"],
        REFUSAL,
    );
}

#[cfg(all(feature = "metamorph", feature = "mock-server"))]
#[tokio::test]
async fn media_sync_http_replay_keeps_recorded_profile_views() {
    for selected in [false, true] {
        let mut store = oxvif::metamorph::FixtureStore::new("sync-http");
        for ns in NS {
            store.record(
                &format!("{ns}/GetProfiles"),
                &wire(ns, "GetProfiles", ""),
                "<recorded-sync-http/>",
            );
        }
        let b = oxvif::mock::MockServer::builder().replay(store);
        let b = if selected {
            b.with_acknowledgment_only(OPS[0])
                .with_acknowledgment_only(OPS[1])
        } else {
            b
        };
        let s = b.start().await.unwrap();
        let t = oxvif::transport::HttpTransport::new();
        for i in 0..2 {
            let xml = post(
                &t,
                s.device_url(),
                i,
                "<m:ProfileToken>Profile_1</m:ProfileToken>",
            )
            .await;
            if selected {
                ack(&xml, i);
            } else {
                fault(&xml, "s:Receiver", &["mock:UnmodeledEffect"], REFUSAL);
            }
            for ns in NS {
                assert_eq!(
                    t.soap_post(
                        s.device_url(),
                        &format!("{ns}/GetProfiles"),
                        wire(ns, "GetProfiles", "")
                    )
                    .await
                    .unwrap(),
                    "<recorded-sync-http/>"
                );
            }
        }
    }
}
