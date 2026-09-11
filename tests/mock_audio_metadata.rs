#![cfg(feature = "mock")]
use oxvif::{OnvifClient, mock::MockTransport, soap::parse_soap_body, transport::Transport};
use std::sync::Arc;
const M1: &str = "http://www.onvif.org/ver10/media/wsdl";
const M2: &str = "http://www.onvif.org/ver20/media/wsdl";
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

#[tokio::test]
async fn media2_audio_uses_media_subtype_vocabulary() {
    let t = MockTransport::new();
    let client = OnvifClient::new("http://mock").with_transport(Arc::new(t));
    let configs = client
        .get_audio_encoder_configurations_media2("http://mock")
        .await
        .unwrap();
    assert_eq!(configs[0].encoding.as_str(), "PCMU");
    assert_eq!(configs[1].encoding.as_str(), "MP4A-LATM");
}

#[tokio::test]
async fn metadata_does_not_claim_persistent_streaming() {
    let t = MockTransport::new();
    let xml = post(&t, M2, "GetMetadataConfigurations", "").await;
    let body = parse_soap_body(&xml).unwrap();
    assert_eq!(body.children[0].children.len(), 2);
    for c in &body.children[0].children {
        assert_eq!(c.path(&["Multicast", "AutoStart"]).unwrap().text(), "false");
    }
}

#[tokio::test]
async fn invalid_audio_write_preserves_state() {
    let t = MockTransport::new();
    let before = serde_json::to_value(&*t.device().read()).unwrap();
    let xml = post(&t, M2, "SetAudioEncoderConfiguration", "<m:Configuration token='AEC_1'><tt:Name>not-committed-971</tt:Name><tt:UseCount>1</tt:UseCount><tt:Encoding>PCMU</tt:Encoding><tt:Bitrate>64</tt:Bitrate><tt:SampleRate>invalid-971</tt:SampleRate></m:Configuration>").await;
    assert_fault(
        &xml,
        "s:Sender",
        &["ter:InvalidArgVal", "ter:ConfigModify"],
        "Invalid mock audio/metadata setting: SampleRate",
    );
    assert_eq!(serde_json::to_value(&*t.device().read()).unwrap(), before);
}

fn audio_setting(m2: bool) -> String {
    let mc = "<tt:Multicast><tt:Address><tt:Type>IPv6</tt:Type><tt:IPv6Address>ff15::971</tt:IPv6Address></tt:Address><tt:Port>4971</tt:Port><tt:TTL>17</tt:TTL><tt:AutoStart>true</tt:AutoStart></tt:Multicast>";
    let tail = if m2 {
        format!("{mc}<tt:Bitrate>64</tt:Bitrate><tt:SampleRate>8</tt:SampleRate>")
    } else {
        format!(
            "<tt:Bitrate>64</tt:Bitrate><tt:SampleRate>8</tt:SampleRate>{mc}<tt:SessionTimeout>PT71S</tt:SessionTimeout>"
        )
    };
    format!(
        "<m:Configuration token='AEC_1'><tt:Name>audio &amp; 971</tt:Name><tt:UseCount>991</tt:UseCount><tt:Encoding>{}</tt:Encoding>{tail}</m:Configuration>{}",
        if m2 { "PCMU" } else { "G711" },
        if m2 {
            ""
        } else {
            "<m:ForcePersistence>true</m:ForcePersistence>"
        }
    )
}
fn metadata_setting() -> String {
    "<m:Configuration token='MetaConf_1'><tt:Name>metadata &amp; 971</tt:Name><tt:UseCount>991</tt:UseCount><tt:PTZStatus><tt:Status>true</tt:Status><tt:Position>false</tt:Position></tt:PTZStatus><tt:Analytics>false</tt:Analytics><tt:Multicast><tt:Address><tt:Type>IPv6</tt:Type><tt:IPv6Address>ff15::972</tt:IPv6Address></tt:Address><tt:Port>4972</tt:Port><tt:TTL>27</tt:TTL><tt:AutoStart>true</tt:AutoStart></tt:Multicast><tt:SessionTimeout>PT72S</tt:SessionTimeout></m:Configuration>".into()
}
async fn contract(
    t: &dyn Transport,
    url: &str,
    state: &oxvif::mock::MockState,
    hooks: &std::sync::atomic::AtomicUsize,
) {
    use std::sync::atomic::Ordering;
    let before = serde_json::to_value(&*state.read()).unwrap();
    let ops = [
        ("GetAudioSourceConfigurations", "ASC_2", 2),
        ("GetAudioEncoderConfigurations", "AEC_2", 2),
        ("GetAudioOutputConfigurations", "AOC_1", 1),
        ("GetAudioDecoderConfigurations", "ADC_1", 1),
        ("GetMetadataConfigurations", "MetaConf_2", 2),
    ];
    for (op, token, count) in ops {
        let xml = post_at(t, url, M2, op, "<m:ProfileToken>Profile_3</m:ProfileToken>").await;
        assert_eq!(
            parse_soap_body(&xml).unwrap().children[0]
                .children_named("Configurations")
                .count(),
            count,
            "{op}: {xml}"
        );
        let xml=post_at(t,url,M2,op,&format!("<m:ConfigurationToken>{token}</m:ConfigurationToken><m:ProfileToken>Profile_3</m:ProfileToken>")).await;
        let b = parse_soap_body(&xml).unwrap();
        assert_eq!(b.children[0].children.len(), 1, "{xml}");
        assert_eq!(
            b.children[0].children[0].attr("token"),
            Some(token),
            "{xml}"
        );
        for (field, leaf, reason) in [
            (
                "ConfigurationToken",
                "ter:NoConfig",
                "Configuration not found: absent-971",
            ),
            (
                "ProfileToken",
                "ter:NoProfile",
                "Profile not found: absent-971",
            ),
        ] {
            let xml = post_at(
                t,
                url,
                M2,
                op,
                &format!("<m:{field}>absent-971</m:{field}>"),
            )
            .await;
            assert_fault(&xml, "s:Sender", &["ter:InvalidArgVal", leaf], reason);
        }
        for fields in [
            "<m:ConfigurationToken/>",
            "<tt:ConfigurationToken>ignored</tt:ConfigurationToken>",
            "<m:ConfigurationToken>one</m:ConfigurationToken><m:ConfigurationToken>two</m:ConfigurationToken>",
        ] {
            let xml = post_at(t, url, M2, op, fields).await;
            if fields.starts_with("<tt:") {
                assert_fault(&xml, "s:Sender", &["ter:TagMismatch"], "Tag Mismatch");
            } else {
                assert_fault(&xml, "s:Sender", &["ter:InvalidArgs"], "Invalid Args");
            }
        }
    }
    for (ns, op) in [
        (M1, "GetAudioSources"),
        (M1, "GetAudioSourceConfigurations"),
        (M1, "GetAudioEncoderConfigurations"),
    ] {
        let xml = post_at(t, url, ns, op, "").await;
        assert_eq!(parse_soap_body(&xml).unwrap().children[0].children.len(), 2);
        let xml = post_at(
            t,
            url,
            ns,
            op,
            "<m:ConfigurationToken>ignored</m:ConfigurationToken>",
        )
        .await;
        assert_fault(&xml, "s:Sender", &["ter:TagMismatch"], "Tag Mismatch");
    }
    for (ns, op) in [
        (M1, "GetAudioEncoderConfigurationOptions"),
        (M2, "GetAudioEncoderConfigurationOptions"),
        (M2, "GetMetadataConfigurationOptions"),
    ] {
        let xml = post_at(t, url, ns, op, "").await;
        assert!(
            xml.contains(if op.contains("Metadata") {
                "<tt:ZoomStatusSupported>true"
            } else if ns == M1 {
                "<tt:Encoding>AAC"
            } else {
                "<tt:Encoding>MP4A-LATM"
            }),
            "{xml}"
        );
        let xml = post_at(
            t,
            url,
            ns,
            op,
            "<m:ProfileToken>absent-971</m:ProfileToken>",
        )
        .await;
        assert_fault(
            &xml,
            "s:Sender",
            &["ter:InvalidArgVal", "ter:NoProfile"],
            "Profile not found: absent-971",
        );
    }
    for (ns, op, valid) in [
        (M1, "SetAudioEncoderConfiguration", audio_setting(false)),
        (M2, "SetAudioEncoderConfiguration", audio_setting(true)),
        (M2, "SetMetadataConfiguration", metadata_setting()),
    ] {
        for (bad, subs, reason) in [
            (
                valid
                    .replace("<tt:TTL>17", "<tt:TTL>bad")
                    .replace("<tt:TTL>27", "<tt:TTL>bad"),
                vec!["ter:InvalidArgVal", "ter:ConfigModify"],
                "Invalid mock audio/metadata setting: Multicast/TTL",
            ),
            (
                valid.replace("<tt:Name>", "<tt:Name/><tt:Name>"),
                vec!["ter:InvalidArgs"],
                "Invalid Args",
            ),
            (
                valid
                    .replace("<tt:Name>", "<wrong:Name xmlns:wrong='urn:wrong'>")
                    .replace("</tt:Name>", "</wrong:Name>"),
                vec!["ter:TagMismatch"],
                "Tag Mismatch",
            ),
            (
                valid.replace(" token=", " unmodeled='value' token="),
                vec!["mock:UnmodeledEffect"],
                "Unmodeled audio/metadata configuration setting",
            ),
        ] {
            let xml = post_at(t, url, ns, op, &bad).await;
            assert_fault(&xml, "s:Sender", &subs, reason);
            assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
            assert_eq!(hooks.load(Ordering::SeqCst), 0);
        }
    }
    for (ns, op, valid) in [
        (M1, "SetAudioEncoderConfiguration", audio_setting(false)),
        (M2, "SetAudioEncoderConfiguration", audio_setting(true)),
        (M2, "SetMetadataConfiguration", metadata_setting()),
    ] {
        let xml = post_at(t, url, ns, op, &valid).await;
        assert_eq!(
            parse_soap_body(&xml).unwrap().children[0].local_name,
            format!("{op}Response"),
            "{xml}"
        );
    }
    assert_eq!(hooks.load(Ordering::SeqCst), 3);
    let snap = state.read();
    let a = &snap.audio_encoders[0];
    assert_eq!(
        (
            &*a.name,
            &*a.encoding,
            a.bitrate,
            a.sample_rate,
            a.use_count
        ),
        ("audio & 971", "G711", 64, 8, 1)
    );
    let m = a.multicast.as_ref().unwrap();
    assert_eq!(
        (&*m.address, m.port, m.ttl, m.auto_start),
        ("ff15::971", 4971, 17, false)
    );
    assert_eq!(a.session_timeout.as_deref(), Some("PT71S"));
    let m = &snap.metadata[0];
    assert_eq!(
        (
            &*m.name,
            m.analytics,
            m.ptz_status,
            m.ptz_position,
            m.use_count
        ),
        ("metadata & 971", false, true, false, 1)
    );
    assert_eq!(
        (
            &*m.multicast.address,
            m.multicast.port,
            m.multicast.ttl,
            m.multicast.auto_start
        ),
        ("ff15::972", 4972, 27, false)
    );
    assert_eq!(m.session_timeout, "PT60S", "deprecated duration is ignored");
    assert_eq!(
        serde_json::to_value(&snap.audio_encoders[1]).unwrap(),
        before["audio_encoders"][1]
    );
    assert_eq!(
        serde_json::to_value(&snap.metadata[1]).unwrap(),
        before["metadata"][1]
    );
}
#[tokio::test]
async fn audio_metadata_contract_in_process() {
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
async fn audio_metadata_contract_http() {
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
async fn every_advertised_audio_combination_is_writable() {
    let t = MockTransport::new();
    let c = OnvifClient::new("http://mock").with_transport(Arc::new(t.clone()));
    for m2 in [false, true] {
        for token in ["AEC_1", "AEC_2"] {
            let options = if m2 {
                c.get_audio_encoder_configuration_options_media2("http://mock", token)
                    .await
                    .unwrap()
            } else {
                c.get_audio_encoder_configuration_options("http://mock", token)
                    .await
                    .unwrap()
            };
            for option in options.options {
                for bitrate in &option.bitrate_list {
                    for rate in &option.sample_rate_list {
                        let mut cfg = c
                            .get_audio_encoder_configuration("http://mock", token)
                            .await
                            .unwrap();
                        cfg.encoding = option.encoding.clone();
                        cfg.bitrate = *bitrate;
                        cfg.sample_rate = *rate;
                        if m2 {
                            cfg.multicast = None;
                            c.set_audio_encoder_configuration_media2("http://mock", &cfg)
                                .await
                                .unwrap();
                        } else {
                            c.set_audio_encoder_configuration("http://mock", &cfg)
                                .await
                                .unwrap();
                        }
                        let rows = if m2 {
                            c.get_audio_encoder_configurations_media2("http://mock")
                                .await
                                .unwrap()
                        } else {
                            c.get_audio_encoder_configurations("http://mock")
                                .await
                                .unwrap()
                        };
                        let got = rows.iter().find(|c| c.token == token).unwrap();
                        assert_eq!(
                            (&got.encoding, got.bitrate, got.sample_rate),
                            (&option.encoding, *bitrate, *rate)
                        );
                        assert!(
                            got.multicast.is_some(),
                            "omission must preserve the shared multicast settings"
                        );
                    }
                }
            }
        }
    }
}
#[tokio::test]
async fn invalid_audio_and_metadata_snapshots_fault_without_repair() {
    let t = MockTransport::new();
    t.device().modify(|s| {
        s.audio_encoders[0].multicast.as_mut().unwrap().auto_start = true;
        s.metadata[0].multicast.ttl = 256;
    });
    let before = serde_json::to_value(&*t.device().read()).unwrap();
    for op in ["GetAudioEncoderConfigurations", "GetMetadataConfigurations"] {
        let xml = post(&t, M2, op, "").await;
        assert_fault(
            &xml,
            "s:Receiver",
            &["mock:RequestPolicy"],
            "Invalid mock audio/metadata snapshot",
        );
    }
    assert_eq!(serde_json::to_value(&*t.device().read()).unwrap(), before);
}

#[cfg(feature = "metamorph")]
fn recordings() -> oxvif::metamorph::FixtureStore {
    let mut store = oxvif::metamorph::FixtureStore::new("am1-975");
    for (ns, op) in [
        (M1, "GetAudioEncoderConfigurations"),
        (M2, "GetAudioEncoderConfigurations"),
        (M1, "GetProfiles"),
        (M2, "GetProfiles"),
        (M2, "GetMetadataConfigurations"),
        (M1, "GetAudioSources"),
    ] {
        let fields = if ns == M2 && op == "GetProfiles" {
            "<m:Type>All</m:Type>"
        } else {
            ""
        };
        let xml = format!(
            "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:m='{ns}' xmlns:tt='http://www.onvif.org/ver10/schema'><s:Body><m:{op}>{fields}</m:{op}></s:Body></s:Envelope>"
        );
        store.record(&format!("{ns}/{op}"), &xml, "<recorded-am1-975/>");
    }
    store
}
#[cfg(feature = "metamorph")]
async fn replay(t: &dyn Transport, url: &str) {
    for (ns, op, setting, reads) in [
        (
            M2,
            "SetAudioEncoderConfiguration",
            audio_setting(true),
            vec![
                (M1, "GetAudioEncoderConfigurations"),
                (M2, "GetAudioEncoderConfigurations"),
                (M1, "GetProfiles"),
                (M2, "GetProfiles"),
            ],
        ),
        (
            M2,
            "SetMetadataConfiguration",
            metadata_setting(),
            vec![(M2, "GetMetadataConfigurations")],
        ),
    ] {
        let bad = setting
            .replace("<tt:TTL>17", "<tt:TTL>bad")
            .replace("<tt:TTL>27", "<tt:TTL>bad");
        let xml = post_at(t, url, ns, op, &bad).await;
        assert_fault(
            &xml,
            "s:Sender",
            &["ter:InvalidArgVal", "ter:ConfigModify"],
            "Invalid mock audio/metadata setting: Multicast/TTL",
        );
        for (ns, op) in &reads {
            assert_eq!(
                post_at(
                    t,
                    url,
                    ns,
                    op,
                    if *ns == M2 && *op == "GetProfiles" {
                        "<m:Type>All</m:Type>"
                    } else {
                        ""
                    }
                )
                .await,
                "<recorded-am1-975/>"
            );
        }
        let xml = post_at(t, url, ns, op, &setting).await;
        assert_eq!(
            parse_soap_body(&xml).unwrap().children[0].local_name,
            format!("{op}Response")
        );
        for (ns, read) in reads {
            let xml = post_at(
                t,
                url,
                ns,
                read,
                if ns == M2 && read == "GetProfiles" {
                    "<m:Type>All</m:Type>"
                } else {
                    ""
                },
            )
            .await;
            assert!(
                xml.contains(if op.contains("Metadata") {
                    "metadata &amp; 971"
                } else {
                    "audio &amp; 971"
                }),
                "{xml}"
            );
        }
        assert_eq!(
            post_at(t, url, M1, "GetAudioSources", "").await,
            "<recorded-am1-975/>"
        );
    }
}
#[cfg(feature = "metamorph")]
#[tokio::test]
async fn audio_metadata_replay_commit_boundary() {
    replay(
        &oxvif::metamorph::MetamorphTransport::new(recordings()),
        "http://mock",
    )
    .await;
}
#[cfg(feature = "metamorph-server")]
#[tokio::test]
async fn audio_metadata_http_replay_commit_boundary() {
    let s = oxvif::mock::MockServer::builder()
        .replay(recordings())
        .start()
        .await
        .unwrap();
    replay(&oxvif::transport::HttpTransport::default(), s.device_url()).await;
}

#[tokio::test]
async fn audio_option_integer_items_preserve_every_value() {
    let t = MockTransport::new();
    let c = OnvifClient::new("http://mock").with_transport(Arc::new(t.clone()));
    for ns in [M1, M2] {
        let xml = post(
            &t,
            ns,
            "GetAudioEncoderConfigurationOptions",
            "<m:ConfigurationToken>AEC_2</m:ConfigurationToken>",
        )
        .await;
        let b = parse_soap_body(&xml).unwrap();
        let options = &b.children[0].children[0];
        let option = if ns == M1 {
            &options.children[0]
        } else {
            options
        };
        let values: Vec<_> = option
            .child("BitrateList")
            .unwrap()
            .children_named("Items")
            .map(|n| n.text())
            .collect();
        assert_eq!(values, ["64", "128", "256"]);
        let opts = if ns == M1 {
            c.get_audio_encoder_configuration_options("http://mock", "AEC_2")
                .await
                .unwrap()
        } else {
            c.get_audio_encoder_configuration_options_media2("http://mock", "AEC_2")
                .await
                .unwrap()
        };
        assert_eq!(opts.options[0].bitrate_list, [64, 128, 256]);
        assert_eq!(opts.options[0].sample_rate_list, [16, 44, 48]);
    }
}
