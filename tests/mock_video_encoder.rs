#![cfg(feature = "mock")]
use oxvif::{OnvifClient, mock::MockTransport, soap::parse_soap_body, transport::Transport};
use std::sync::Arc;

const M1: &str = "http://www.onvif.org/ver10/media/wsdl";
const M2: &str = "http://www.onvif.org/ver20/media/wsdl";
#[cfg(feature = "metamorph")]
const ENCODER_REPLAY_READS: &[(&str, &str, &str, bool)] = &[
    (
        M1,
        "GetProfile",
        "<m:ProfileToken>Profile_1</m:ProfileToken>",
        true,
    ),
    (M1, "GetProfiles", "", true),
    (M2, "GetProfiles", "<m:Type>All</m:Type>", true),
    (M1, "GetVideoEncoderConfigurations", "", true),
    (
        M1,
        "GetVideoEncoderConfiguration",
        "<m:ConfigurationToken>VEC_1</m:ConfigurationToken>",
        true,
    ),
    (
        M2,
        "GetVideoEncoderConfigurations",
        "<m:ConfigurationToken>VEC_1</m:ConfigurationToken>",
        true,
    ),
    (M1, "GetVideoSources", "", false),
    (
        M2,
        "GetVideoEncoderConfigurationOptions",
        "<m:ConfigurationToken>VEC_1</m:ConfigurationToken>",
        false,
    ),
];

#[cfg(feature = "metamorph")]
fn encoder_recordings() -> oxvif::metamorph::FixtureStore {
    let mut store = oxvif::metamorph::FixtureStore::new("synthetic-encoder-replay-017");
    for (index, (ns, op, fields, _)) in ENCODER_REPLAY_READS.iter().enumerate() {
        let request = format!(
            "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:m='{ns}' xmlns:tt='http://www.onvif.org/ver10/schema'><s:Body><m:{op}>{fields}</m:{op}></s:Body></s:Envelope>"
        );
        // Deliberately distinctive raw fixtures prove which responder answered.
        // They are not protocol-shape or real-camera evidence.
        store.record(
            &format!("{ns}/{op}"),
            &request,
            &format!("<recorded-encoder-017-{index}/>"),
        );
    }
    store
}

#[cfg(feature = "metamorph")]
async fn encoder_replay_contract(t: &dyn Transport, url: &str, state: &oxvif::mock::MockState) {
    let before = serde_json::to_value(&*state.read()).unwrap();
    for (index, (ns, op, fields, _)) in ENCODER_REPLAY_READS.iter().enumerate() {
        assert_eq!(
            post_at(t, url, ns, op, fields).await,
            format!("<recorded-encoder-017-{index}/>")
        );
    }

    let invalid =
        setting("VEC_1").replace("<tt:Quality>6</tt:Quality>", "<tt:Quality>NaN</tt:Quality>");
    let failed = post_at(t, url, M2, "SetVideoEncoderConfiguration", &invalid).await;
    assert_eq!(
        parse_soap_body(&failed).unwrap().children[0].local_name,
        "Fault"
    );
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
    for (index, (ns, op, fields, _)) in ENCODER_REPLAY_READS.iter().enumerate() {
        assert_eq!(
            post_at(t, url, ns, op, fields).await,
            format!("<recorded-encoder-017-{index}/>"),
            "a rejected encoder write must preserve every recording"
        );
    }

    let committed = post_at(
        t,
        url,
        M2,
        "SetVideoEncoderConfiguration",
        &setting("VEC_1"),
    )
    .await;
    assert_eq!(
        parse_soap_body(&committed).unwrap().children[0].local_name,
        "SetVideoEncoderConfigurationResponse"
    );
    for (index, (ns, op, fields, affected)) in ENCODER_REPLAY_READS.iter().enumerate() {
        let response = post_at(t, url, ns, op, fields).await;
        if *affected {
            assert!(
                !response.contains("recorded-encoder-017"),
                "committed encoder write replayed stale {ns}/{op}"
            );
            assert!(
                response.contains("<tt:Name>scoped-963</tt:Name>"),
                "dependent read did not expose committed encoder: {ns}/{op}: {response}"
            );
            assert_ne!(
                parse_soap_body(&response).unwrap().children[0].local_name,
                "Fault"
            );
        } else {
            assert_eq!(
                response,
                format!("<recorded-encoder-017-{index}/>"),
                "unrelated physical-source/options recording was retired"
            );
        }
    }
}

#[cfg(feature = "metamorph")]
#[tokio::test]
async fn media2_encoder_commit_retires_recorded_views_in_process() {
    let replay = oxvif::metamorph::MetamorphTransport::new(encoder_recordings());
    encoder_replay_contract(&replay, "http://mock", replay.device()).await;
}

#[cfg(feature = "metamorph-server")]
#[tokio::test]
async fn media2_encoder_commit_retires_recorded_views_over_http() {
    let server = oxvif::mock::MockServer::builder()
        .port(0)
        .replay(encoder_recordings())
        .start()
        .await
        .unwrap();
    encoder_replay_contract(
        &oxvif::transport::HttpTransport::default(),
        server.device_url(),
        server.device(),
    )
    .await;
}
async fn post(t: &dyn Transport, ns: &str, op: &str, fields: &str) -> String {
    post_at(t, "http://mock", ns, op, fields).await
}

async fn post_at(t: &dyn Transport, url: &str, ns: &str, op: &str, fields: &str) -> String {
    t.soap_post(url, &format!("{ns}/{op}"), format!("<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:m='{ns}' xmlns:tt='http://www.onvif.org/ver10/schema'><s:Body><m:{op}>{fields}</m:{op}></s:Body></s:Envelope>")).await.unwrap()
}

fn assert_fault(xml: &str, code: &str, subcodes: &[&str], reason: &str) {
    let body = parse_soap_body(xml).unwrap();
    assert_eq!(body.children.len(), 1);
    let fault = &body.children[0];
    assert_eq!(fault.local_name, "Fault", "{xml}");
    let mut node = fault.child("Code").unwrap();
    assert_eq!(node.child("Value").unwrap().text(), code);
    for subcode in subcodes {
        node = node.child("Subcode").unwrap();
        assert_eq!(node.child("Value").unwrap().text(), *subcode);
    }
    assert!(node.child("Subcode").is_none());
    assert_eq!(fault.path(&["Reason", "Text"]).unwrap().text(), reason);
}

fn setting(token: &str) -> String {
    format!(
        "<m:Configuration token='{token}' GovLength='25' Profile='Main'><tt:Name>scoped-963</tt:Name><tt:UseCount>999</tt:UseCount><tt:Encoding>H264</tt:Encoding><tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution><tt:RateControl><tt:FrameRateLimit>25</tt:FrameRateLimit><tt:BitrateLimit>2048</tt:BitrateLimit></tt:RateControl><tt:Quality>6</tt:Quality></m:Configuration>"
    )
}

async fn contract(
    t: &dyn Transport,
    url: &str,
    state: &oxvif::mock::MockState,
    hooks: &std::sync::atomic::AtomicUsize,
) {
    use std::sync::atomic::Ordering;
    for ns in [M1, M2] {
        let xml = post_at(t, url, ns, "GetVideoEncoderConfigurationOptions", "").await;
        assert!(xml.contains("<tt:Width>2592</tt:Width>"), "{xml}");
        assert!(xml.contains("<tt:Width>352</tt:Width>"));
        for selector in [
            "<m:ConfigurationToken>absent-963</m:ConfigurationToken>",
            "<m:ProfileToken>absent-963</m:ProfileToken>",
        ] {
            let xml = post_at(t, url, ns, "GetVideoEncoderConfigurationOptions", selector).await;
            let config = selector.contains("ConfigurationToken");
            assert_fault(
                &xml,
                "s:Sender",
                &[
                    "ter:InvalidArgVal",
                    if config {
                        "ter:NoConfig"
                    } else {
                        "ter:NoProfile"
                    },
                ],
                if config {
                    "Encoder configuration not found: absent-963"
                } else {
                    "Profile not found: absent-963"
                },
            );
        }
    }
    // Under the explicit logical compatibility model a profile selector validates
    // existence, but does not reduce the catalogue to its currently bound encoder.
    let xml = post_at(
        t,
        url,
        M2,
        "GetVideoEncoderConfigurations",
        "<m:ProfileToken>Profile_3</m:ProfileToken>",
    )
    .await;
    assert_eq!(
        parse_soap_body(&xml).unwrap().children[0]
            .children_named("Configurations")
            .count(),
        4
    );
    for fields in [
        "<m:ConfigurationToken/>",
        "<m:ConfigurationToken>VEC_1</m:ConfigurationToken><m:ConfigurationToken>VEC_2</m:ConfigurationToken>",
        "<m:ConfigurationToken><m:Nested>VEC_1</m:Nested></m:ConfigurationToken>",
    ] {
        assert_fault(
            &post_at(t, url, M2, "GetVideoEncoderConfigurations", fields).await,
            "s:Sender",
            &["ter:InvalidArgs"],
            "Invalid Args",
        );
    }
    assert_fault(
        &post_at(
            t,
            url,
            M1,
            "GetVideoEncoderConfigurations",
            "<m:ConfigurationToken>VEC_1</m:ConfigurationToken>",
        )
        .await,
        "s:Sender",
        &["ter:TagMismatch"],
        "Tag Mismatch",
    );
    let full = setting("VEC_1");
    for (fields, subcodes, reason) in [
        (
            full.replace("<tt:Quality>6", "<tt:Quality>NaN"),
            vec!["ter:InvalidArgVal", "ter:ConfigModify"],
            "Invalid mock encoder setting: Quality",
        ),
        (
            full.replace("Profile='Main'", "Profile='unsupported-964'"),
            vec!["ter:InvalidArgVal", "ter:ConfigModify"],
            "Invalid mock encoder setting: Profile",
        ),
        (
            full.replace("GovLength='25'", "GovLength='999'"),
            vec!["ter:InvalidArgVal", "ter:ConfigModify"],
            "Invalid mock encoder setting: GovLength",
        ),
        (
            full.replace("<tt:Width>1920", "<tt:Width>17"),
            vec!["ter:InvalidArgVal", "ter:ConfigModify"],
            "Invalid mock encoder setting: Resolution",
        ),
        (
            full.replace("<tt:Name>", "<tt:Name xmlns:tt='urn:foreign'>"),
            vec!["ter:TagMismatch"],
            "Tag Mismatch",
        ),
        (
            full.replace("</tt:Name>", "</tt:Name><tt:Name>duplicate-964</tt:Name>"),
            vec!["ter:InvalidArgs"],
            "Invalid Args",
        ),
        (
            full.replace("Profile='Main'", "Profile='Main' Signed='true'"),
            vec!["mock:UnmodeledEffect"],
            "Unmodeled encoder configuration setting",
        ),
    ] {
        let before = serde_json::to_value(&*state.read()).unwrap();
        let calls = hooks.load(Ordering::SeqCst);
        assert_fault(
            &post_at(t, url, M2, "SetVideoEncoderConfiguration", &fields).await,
            "s:Sender",
            &subcodes,
            reason,
        );
        assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
        assert_eq!(hooks.load(Ordering::SeqCst), calls);
    }
    let media1 = full
        .replace(" GovLength='25' Profile='Main'", "")
        .replace("<tt:RateControl>", "<tt:Quality>6</tt:Quality><tt:RateControl>")
        .replace("</tt:FrameRateLimit>", "</tt:FrameRateLimit><tt:EncodingInterval>1</tt:EncodingInterval>")
        .replace("</tt:RateControl><tt:Quality>6</tt:Quality>", "</tt:RateControl><tt:H264><tt:GovLength>25</tt:GovLength><tt:H264Profile>Main</tt:H264Profile></tt:H264><tt:Multicast><tt:Address><tt:Type>IPv4</tt:Type><tt:IPv4Address>0.0.0.0</tt:IPv4Address></tt:Address><tt:Port>0</tt:Port><tt:TTL>1</tt:TTL><tt:AutoStart>false</tt:AutoStart></tt:Multicast><tt:SessionTimeout>PT0S</tt:SessionTimeout>")
        + "<m:ForcePersistence>false</m:ForcePersistence>";
    for fields in [
        media1.replace("<tt:EncodingInterval>1", "<tt:EncodingInterval>2"),
        media1.replace("0.0.0.0", "239.1.2.3"),
        media1.replace("PT0S", "PT60S"),
    ] {
        let before = serde_json::to_value(&*state.read()).unwrap();
        let calls = hooks.load(Ordering::SeqCst);
        assert_fault(
            &post_at(t, url, M1, "SetVideoEncoderConfiguration", &fields).await,
            "s:Sender",
            &["mock:UnmodeledEffect"],
            "Unmodeled encoder configuration setting",
        );
        assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
        assert_eq!(hooks.load(Ordering::SeqCst), calls);
    }
    let accepted = post_at(t, url, M1, "SetVideoEncoderConfiguration", &media1).await;
    assert_eq!(
        parse_soap_body(&accepted).unwrap().children[0].local_name,
        "SetVideoEncoderConfigurationResponse"
    );
    let calls = hooks.load(Ordering::SeqCst);
    let xml = post_at(
        t,
        url,
        M2,
        "SetVideoEncoderConfiguration",
        &full
            .replace("2048", "-99")
            .replace("<tt:Quality>6", "<tt:Quality>99"),
    )
    .await;
    assert_eq!(
        parse_soap_body(&xml).unwrap().children[0].local_name,
        "SetVideoEncoderConfigurationResponse"
    );
    assert_eq!(state.read().video_encoders[0].bitrate_limit, 64);
    assert_eq!(state.read().video_encoders[0].quality, 10.0);
    assert_eq!(state.read().video_encoders[0].use_count, 1);
    assert_eq!(state.read().video_encoders[1].bitrate_limit, 1024);
    assert_eq!(hooks.load(Ordering::SeqCst), calls + 1);
    // Decoded identities must survive selection, write and both profile renderers.
    const TOKEN: &str = "Encoder & <北>";
    state.modify(|s| {
        s.video_encoders[0].token = TOKEN.into();
        s.profiles.profiles[0].video_encoder_config_token = Some(TOKEN.into());
    });
    let xml = post_at(
        t,
        url,
        M2,
        "SetVideoEncoderConfiguration",
        &setting("Encoder &amp; &lt;北&gt;").replace("scoped-963", "N &amp; &lt;北&gt;"),
    )
    .await;
    assert_eq!(
        parse_soap_body(&xml).unwrap().children[0].local_name,
        "SetVideoEncoderConfigurationResponse"
    );
    assert_eq!(state.read().video_encoders[0].name, "N & <北>");
    for ns in [M1, M2] {
        let op = if ns == M1 {
            "GetVideoEncoderConfiguration"
        } else {
            "GetVideoEncoderConfigurations"
        };
        let xml = post_at(
            t,
            url,
            ns,
            op,
            "<m:ConfigurationToken>Encoder &amp; &lt;北&gt;</m:ConfigurationToken>",
        )
        .await;
        let body = parse_soap_body(&xml).unwrap();
        let config = &body.children[0].children[0];
        assert_eq!(config.attr("token"), Some(TOKEN));
        assert_eq!(config.child("Name").unwrap().text(), "N & <北>");
        let fields = if ns == M2 { "<m:Type>All</m:Type>" } else { "" };
        let xml = post_at(t, url, ns, "GetProfiles", fields).await;
        assert!(xml.contains("Encoder &amp; &lt;北&gt;"), "{xml}");
    }
    for (token, total, has_h265) in [("VSC_1", 4, true), ("VSC_2", 2, false)] {
        let xml = post_at(
            t,
            url,
            M2,
            "GetVideoEncoderInstances",
            &format!("<m:ConfigurationToken>{token}</m:ConfigurationToken>"),
        )
        .await;
        let body = parse_soap_body(&xml).unwrap();
        let info = body.children[0].child("Info").unwrap();
        assert_eq!(info.child("Total").unwrap().text(), total.to_string());
        assert_eq!(xml.contains("<tr2:Encoding>H265</tr2:Encoding>"), has_h265);
    }
    assert_fault(
        &post_at(
            t,
            url,
            M2,
            "GetVideoEncoderInstances",
            "<m:ConfigurationToken>VEC_3</m:ConfigurationToken>",
        )
        .await,
        "s:Sender",
        &["ter:InvalidArgVal", "ter:NoConfig"],
        "Encoder configuration not found: VEC_3",
    );
}

#[tokio::test]
async fn encoder_contract_in_process() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let hooks = Arc::new(AtomicUsize::new(0));
    let captured = hooks.clone();
    let mut state = oxvif::mock::MockState::new();
    state.set_on_change(Arc::new(move |_| {
        captured.fetch_add(1, Ordering::SeqCst);
    }));
    let t = MockTransport::with_state(state);
    contract(&t, "http://mock", t.device(), &hooks).await;
}
#[cfg(feature = "mock-server")]
#[tokio::test]
async fn encoder_contract_http() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let hooks = Arc::new(AtomicUsize::new(0));
    let captured = hooks.clone();
    let server = oxvif::mock::MockServer::builder()
        .on_change(Arc::new(move |_| {
            captured.fetch_add(1, Ordering::SeqCst);
        }))
        .start()
        .await
        .unwrap();
    contract(
        &oxvif::transport::HttpTransport::default(),
        server.device_url(),
        server.device(),
        &hooks,
    )
    .await;
}

#[tokio::test]
async fn advertised_encoder_choices_roundtrip() {
    let t = MockTransport::new();
    let client = OnvifClient::new("http://mock").with_transport(Arc::new(t.clone()));
    for token in ["VEC_1", "VEC_2", "VEC_3", "VEC_4"] {
        let options = client
            .get_video_encoder_configuration_options_media2("http://mock", token)
            .await
            .unwrap();
        for option in options.options {
            let mut c = client
                .get_video_encoder_configuration_media2("http://mock", token)
                .await
                .unwrap();
            c.encoding = option.encoding.clone();
            c.gov_length = option.gov_length_range.as_ref().map(|r| r.min as u32);
            c.profile = option.profiles.first().cloned();
            for rate in option.frame_rates {
                c.rate_control.as_mut().unwrap().frame_rate_limit = rate;
                client
                    .set_video_encoder_configuration_media2("http://mock", &c)
                    .await
                    .unwrap();
                let read = client
                    .get_video_encoder_configuration_media2("http://mock", token)
                    .await
                    .unwrap();
                assert_eq!(read.rate_control.unwrap().frame_rate_limit, rate);
                assert_eq!(read.encoding, c.encoding);
            }
            for resolution in option.resolutions {
                c.resolution = resolution;
                client
                    .set_video_encoder_configuration_media2("http://mock", &c)
                    .await
                    .unwrap();
                let read = client
                    .get_video_encoder_configuration_media2("http://mock", token)
                    .await
                    .unwrap();
                assert_eq!(read.resolution.width, c.resolution.width);
                assert_eq!(read.resolution.height, c.resolution.height);
            }
        }
    }
}

#[tokio::test]
async fn encoder_missing_reference_is_not_an_empty_success() {
    let t = MockTransport::new();
    let xml = post(
        &t,
        M2,
        "GetVideoEncoderConfigurations",
        "<m:ConfigurationToken>missing-ve1-961</m:ConfigurationToken>",
    )
    .await;
    let body = parse_soap_body(&xml).unwrap();
    assert_eq!(body.children[0].local_name, "Fault");
    let fault = &body.children[0];
    assert_eq!(
        fault
            .path(&["Code", "Subcode", "Subcode", "Value"])
            .unwrap()
            .text(),
        "ter:NoConfig"
    );
    assert_eq!(
        fault.path(&["Reason", "Text"]).unwrap().text(),
        "Encoder configuration not found: missing-ve1-961"
    );
}

#[tokio::test]
async fn encoder_invalid_late_field_preserves_everything() {
    let t = MockTransport::new();
    let before = serde_json::to_value(&*t.device().read()).unwrap();
    let fields = "<m:Configuration token='VEC_1' GovLength='25' Profile='Main'><tt:Name>must-not-write-962</tt:Name><tt:UseCount>1</tt:UseCount><tt:Encoding>H264</tt:Encoding><tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution><tt:Quality>NaN</tt:Quality></m:Configuration>";
    let xml = post(&t, M2, "SetVideoEncoderConfiguration", fields).await;
    let body = parse_soap_body(&xml).unwrap();
    assert_eq!(body.children[0].local_name, "Fault");
    assert_eq!(
        body.children[0].path(&["Reason", "Text"]).unwrap().text(),
        "Invalid mock encoder setting: Quality"
    );
    assert_eq!(serde_json::to_value(&*t.device().read()).unwrap(), before);
}

#[tokio::test]
async fn encoder_bitrate_adapts_and_codec_views_are_explicit() {
    let t = MockTransport::new();
    let client = OnvifClient::new("http://mock").with_transport(Arc::new(t.clone()));
    let mut config = client
        .get_video_encoder_configuration_media2("http://mock", "VEC_1")
        .await
        .unwrap();
    config.rate_control.as_mut().unwrap().bitrate_limit = 999999;
    config.rate_control.as_mut().unwrap().frame_rate_limit = f32::MAX;
    client
        .set_video_encoder_configuration_media2("http://mock", &config)
        .await
        .unwrap();
    assert_eq!(t.device().read().video_encoders[0].bitrate_limit, 16384);
    assert_eq!(t.device().read().video_encoders[0].frame_rate_limit, 30.0);
    config.encoding = oxvif::VideoEncoding::H265;
    config.profile = Some("Main".into());
    client
        .set_video_encoder_configuration_media2("http://mock", &config)
        .await
        .unwrap();
    let xml = post(
        &t,
        M1,
        "GetVideoEncoderConfiguration",
        "<m:ConfigurationToken>VEC_1</m:ConfigurationToken>",
    )
    .await;
    let body = parse_soap_body(&xml).unwrap();
    assert_eq!(body.children[0].local_name, "Fault");
    assert_eq!(
        body.children[0].path(&["Reason", "Text"]).unwrap().text(),
        "Encoder codec cannot be represented by Media1; use Media2"
    );
}
