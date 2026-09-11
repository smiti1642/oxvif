//! Shared mock rate storage, refusal and representation controls.
#![cfg(feature = "mock")]
use oxvif::{
    OnvifClient,
    mock::{MockState, MockTransport},
    soap::parse_soap_body,
    transport::Transport,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const M1: &str = "http://www.onvif.org/ver10/media/wsdl";
const M2: &str = "http://www.onvif.org/ver20/media/wsdl";
const VIEW_REASON: &str = "Encoder frame rate cannot be represented by Media1; use Media2";

fn envelope(ns: &str, op: &str, fields: &str) -> String {
    format!(
        "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:m='{ns}' xmlns:tt='http://www.onvif.org/ver10/schema'><s:Body><m:{op}>{fields}</m:{op}></s:Body></s:Envelope>"
    )
}
async fn post(t: &dyn Transport, url: &str, ns: &str, op: &str, fields: &str) -> String {
    // VE1 requires the actual full Media1 shape, not a Media2 rate fragment.
    let fields = if ns == M1 && op == "SetVideoEncoderConfiguration" {
        fields.replace("<tt:RateControl>", "<tt:Quality>5</tt:Quality><tt:RateControl>")
            .replace("<tt:BitrateLimit>", "<tt:EncodingInterval>1</tt:EncodingInterval><tt:BitrateLimit>")
            .replace("</tt:RateControl><tt:Quality>5</tt:Quality>", "</tt:RateControl>")
            .replace("</m:Configuration>", "<tt:Multicast><tt:Address><tt:Type>IPv4</tt:Type><tt:IPv4Address>0.0.0.0</tt:IPv4Address></tt:Address><tt:Port>0</tt:Port><tt:TTL>1</tt:TTL><tt:AutoStart>false</tt:AutoStart></tt:Multicast><tt:SessionTimeout>PT0S</tt:SessionTimeout></m:Configuration><m:ForcePersistence>true</m:ForcePersistence>")
    } else {
        fields.to_owned()
    };
    t.soap_post(url, &format!("{ns}/{op}"), envelope(ns, op, &fields))
        .await
        .unwrap()
}
fn config(rate: &str) -> String {
    format!(
        "<m:Configuration token='VEC_1'><tt:Name>rate-951</tt:Name><tt:UseCount>1</tt:UseCount><tt:Encoding>H264</tt:Encoding><tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution><tt:RateControl><tt:FrameRateLimit>{rate}</tt:FrameRateLimit><tt:BitrateLimit>4096</tt:BitrateLimit></tt:RateControl><tt:Quality>5</tt:Quality></m:Configuration>"
    )
}
fn fault(xml: &str, code: &str, subcode: &str, reason: &str) {
    let body = parse_soap_body(xml).unwrap();
    assert_eq!(body.children.len(), 1, "{xml}");
    let f = &body.children[0];
    assert_eq!(f.local_name, "Fault", "{xml}");
    assert_eq!(f.path(&["Code", "Value"]).unwrap().text(), code);
    assert_eq!(
        f.path(&["Code", "Subcode", "Value"]).unwrap().text(),
        subcode
    );
    assert_eq!(f.path(&["Reason", "Text"]).unwrap().text(), reason);
}
async fn check(t: &dyn Transport, url: &str, state: &MockState, hooks: &AtomicUsize) {
    for ns in [M1, M2] {
        for bad in ["NaN", "INF", "-1", "bad-952"] {
            let before = serde_json::to_value(&*state.read()).unwrap();
            let calls = hooks.load(Ordering::SeqCst);
            let xml = post(t, url, ns, "SetVideoEncoderConfiguration", &config(bad)).await;
            fault(
                &xml,
                "s:Sender",
                "ter:InvalidArgs",
                "Invalid encoder FrameRateLimit",
            );
            assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
            assert_eq!(hooks.load(Ordering::SeqCst), calls);
        }
        let before = serde_json::to_value(&*state.read()).unwrap();
        let calls = hooks.load(Ordering::SeqCst);
        let xml = post(
            t,
            url,
            ns,
            "SetVideoEncoderConfiguration",
            &config("25").replace("4096", "bad-953"),
        )
        .await;
        fault(
            &xml,
            "s:Sender",
            "ter:InvalidArgs",
            "Invalid encoder BitrateLimit",
        );
        assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
        assert_eq!(hooks.load(Ordering::SeqCst), calls);
    }
    for (body, subcode, reason) in [
        (
            config("12.5").replace(
                "<tt:FrameRateLimit>",
                "<tt:FrameRateLimit xmlns:tt='urn:decoy'>",
            ),
            "ter:TagMismatch",
            "Tag Mismatch",
        ),
        (
            config("12.5").replace(
                "</tt:FrameRateLimit>",
                "</tt:FrameRateLimit><tt:FrameRateLimit>25</tt:FrameRateLimit>",
            ),
            "ter:InvalidArgs",
            "Invalid Args",
        ),
        (
            config("<tt:Decoy>12.5</tt:Decoy>"),
            "ter:InvalidArgs",
            "Invalid Args",
        ),
    ] {
        let before = serde_json::to_value(&*state.read()).unwrap();
        let calls = hooks.load(Ordering::SeqCst);
        fault(
            &post(t, url, M2, "SetVideoEncoderConfiguration", &body).await,
            "s:Sender",
            subcode,
            reason,
        );
        assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
        assert_eq!(hooks.load(Ordering::SeqCst), calls);
    }
    let calls = hooks.load(Ordering::SeqCst);
    let xml = post(t, url, M2, "SetVideoEncoderConfiguration", &config("12.5")).await;
    assert_eq!(
        parse_soap_body(&xml).unwrap().children[0].local_name,
        "SetVideoEncoderConfigurationResponse"
    );
    assert_eq!(hooks.load(Ordering::SeqCst), calls + 1);
    assert_eq!(state.read().video_encoders[0].frame_rate_limit, 12.5);
    assert_eq!(state.read().video_encoders[1].frame_rate_limit, 15.0);
    for op in ["GetVideoEncoderConfigurations", "GetProfiles"] {
        let fields = if op == "GetProfiles" {
            "<m:Type>All</m:Type>"
        } else {
            ""
        };
        let xml = post(t, url, M2, op, fields).await;
        assert!(
            xml.contains("<tt:FrameRateLimit>12.5</tt:FrameRateLimit>"),
            "{xml}"
        );
    }
    for (op, fields) in [
        ("GetVideoEncoderConfigurations", ""),
        (
            "GetVideoEncoderConfiguration",
            "<m:ConfigurationToken>VEC_1</m:ConfigurationToken>",
        ),
        ("GetProfiles", ""),
        ("GetProfile", "<m:ProfileToken>Profile_1</m:ProfileToken>"),
    ] {
        fault(
            &post(t, url, M1, op, fields).await,
            "s:Receiver",
            "mock:RequestPolicy",
            VIEW_REASON,
        );
    }
    let xml = post(
        t,
        url,
        M1,
        "GetVideoEncoderConfiguration",
        "<m:ConfigurationToken>VEC_2</m:ConfigurationToken>",
    )
    .await;
    assert!(xml.contains("<tt:FrameRateLimit>15</tt:FrameRateLimit>"));
    fault(
        &post(t, url, M1, "SetVideoEncoderConfiguration", &config("12.5")).await,
        "s:Sender",
        "ter:InvalidArgs",
        "Invalid encoder FrameRateLimit",
    );
    assert_eq!(state.read().video_encoders[0].frame_rate_limit, 12.5);
    let xml = post(t, url, M1, "SetVideoEncoderConfiguration", &config("20")).await;
    assert_eq!(
        parse_soap_body(&xml).unwrap().children[0].local_name,
        "SetVideoEncoderConfigurationResponse"
    );
    assert_eq!(state.read().video_encoders[0].frame_rate_limit, 20.0);
    assert_eq!(hooks.load(Ordering::SeqCst), calls + 2);
}

#[tokio::test]
async fn rate_state_and_views() {
    let mut state = MockState::new();
    let hooks = Arc::new(AtomicUsize::new(0));
    let captured = hooks.clone();
    state.set_on_change(Arc::new(move |_| {
        captured.fetch_add(1, Ordering::SeqCst);
    }));
    let t = MockTransport::with_state(state);
    check(&t, "http://mock", t.device(), &hooks).await;
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn http_rate_state_and_views() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let captured = hooks.clone();
    let server = oxvif::mock::MockServer::builder()
        .on_change(Arc::new(move |_| {
            captured.fetch_add(1, Ordering::SeqCst);
        }))
        .start()
        .await
        .unwrap();
    check(
        &oxvif::transport::HttpTransport::default(),
        server.device_url(),
        server.device(),
        &hooks,
    )
    .await;
}

#[tokio::test]
async fn public_mock_rate_roundtrip_and_old_snapshot() {
    let t = MockTransport::new();
    let client = OnvifClient::new("http://mock").with_transport(Arc::new(t.clone()));
    let mut config = client
        .get_video_encoder_configuration_media2("http://mock", "VEC_1")
        .await
        .unwrap();
    config.rate_control.as_mut().unwrap().frame_rate_limit = 29.97;
    client
        .set_video_encoder_configuration_media2("http://mock", &config)
        .await
        .unwrap();
    let actual = client
        .get_video_encoder_configuration_media2("http://mock", "VEC_1")
        .await
        .unwrap();
    assert_eq!(
        actual.rate_control.as_ref().unwrap().frame_rate_limit,
        29.97
    );
    assert_eq!(t.device().read().video_encoders[0].frame_rate_limit, 29.97);
    config.rate_control = None;
    client
        .set_video_encoder_configuration_media2("http://mock", &config)
        .await
        .unwrap();
    assert_eq!(
        t.device().read().video_encoders[0].frame_rate_limit,
        29.97,
        "omitted rate control must retain the stored value"
    );
    let mut snapshot = serde_json::to_value(&*t.device().read()).unwrap();
    snapshot["video_encoders"][0]["frame_rate_limit"] = serde_json::json!(25);
    let restored: oxvif::mock::DeviceState = serde_json::from_value(snapshot).unwrap();
    assert_eq!(restored.video_encoders[0].frame_rate_limit, 25.0);
}

#[tokio::test]
async fn invalid_seed_rates_are_explicit_and_not_persisted_as_null() {
    let t = MockTransport::new();
    for value in [f32::NAN, f32::INFINITY, -1.0] {
        t.device()
            .modify(|s| s.video_encoders[0].frame_rate_limit = value);
        for ns in [M1, M2] {
            fault(
                &post(&t, "http://mock", ns, "GetVideoEncoderConfigurations", "").await,
                "s:Receiver",
                "mock:RequestPolicy",
                "Invalid mock encoder frame rate",
            );
            let fields = if ns == M2 { "<m:Type>All</m:Type>" } else { "" };
            fault(
                &post(&t, "http://mock", ns, "GetProfiles", fields).await,
                "s:Receiver",
                "mock:RequestPolicy",
                "Invalid mock encoder frame rate",
            );
        }
        assert_eq!(
            t.device().read().video_encoders[0]
                .frame_rate_limit
                .to_bits(),
            value.to_bits()
        );
        assert_eq!(
            serde_json::to_value(&*t.device().read())
                .unwrap_err()
                .to_string(),
            "frame rate must be finite and nonnegative"
        );
    }
    t.device()
        .modify(|s| s.video_encoders[0].frame_rate_limit = 0.0);
    for ns in [M1, M2] {
        let xml = post(&t, "http://mock", ns, "GetVideoEncoderConfigurations", "").await;
        assert!(xml.contains("<tt:FrameRateLimit>0</tt:FrameRateLimit>"));
    }
}

#[cfg(feature = "metamorph")]
fn recordings() -> oxvif::metamorph::FixtureStore {
    let mut store = oxvif::metamorph::FixtureStore::new("rate-954");
    for ns in [M1, M2] {
        for op in ["GetVideoEncoderConfigurations", "GetProfiles"] {
            store.record(
                &format!("{ns}/{op}"),
                &envelope(ns, op, ""),
                "<recorded-rate-954/>",
            );
        }
    }
    store.record(
        &format!("{M1}/GetVideoSources"),
        &envelope(M1, "GetVideoSources", ""),
        "<unrelated-rate-954/>",
    );
    store
}
#[cfg(feature = "metamorph")]
async fn replay(t: &dyn Transport, url: &str) {
    fault(
        &post(t, url, M2, "SetVideoEncoderConfiguration", &config("NaN")).await,
        "s:Sender",
        "ter:InvalidArgs",
        "Invalid encoder FrameRateLimit",
    );
    for ns in [M1, M2] {
        for op in ["GetVideoEncoderConfigurations", "GetProfiles"] {
            assert_eq!(post(t, url, ns, op, "").await, "<recorded-rate-954/>");
        }
    }
    let xml = post(t, url, M2, "SetVideoEncoderConfiguration", &config("12.5")).await;
    assert_eq!(
        parse_soap_body(&xml).unwrap().children[0].local_name,
        "SetVideoEncoderConfigurationResponse"
    );
    assert!(
        post(t, url, M2, "GetVideoEncoderConfigurations", "")
            .await
            .contains("<tt:FrameRateLimit>12.5</tt:FrameRateLimit>")
    );
    for op in ["GetVideoEncoderConfigurations", "GetProfiles"] {
        fault(
            &post(t, url, M1, op, "").await,
            "s:Receiver",
            "mock:RequestPolicy",
            VIEW_REASON,
        );
    }
    assert_eq!(
        post(t, url, M1, "GetVideoSources", "").await,
        "<unrelated-rate-954/>"
    );
}
#[cfg(feature = "metamorph")]
#[tokio::test]
async fn rate_replay_commit_boundary() {
    replay(
        &oxvif::metamorph::MetamorphTransport::new(recordings()),
        "http://mock",
    )
    .await;
}
#[cfg(feature = "metamorph-server")]
#[tokio::test]
async fn http_rate_replay_commit_boundary() {
    let server = oxvif::mock::MockServer::builder()
        .replay(recordings())
        .start()
        .await
        .unwrap();
    replay(
        &oxvif::transport::HttpTransport::default(),
        server.device_url(),
    )
    .await;
}
