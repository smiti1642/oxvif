//! Project-authored profile assembly controls, not copied schema fixtures.
#![cfg(feature = "mock")]

use oxvif::{
    mock::{MockState, MockTransport},
    soap::{find_response, parse_soap_body},
    transport::Transport,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const M1: &str = "http://www.onvif.org/ver10/media/wsdl";
const M2: &str = "http://www.onvif.org/ver20/media/wsdl";

async fn post(t: &dyn Transport, url: &str, ns: &str, op: &str, fields: &str) -> String {
    t.soap_post(url, &format!("{ns}/{op}"), format!("<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:m='{ns}'><s:Body><m:{op}>{fields}</m:{op}></s:Body></s:Envelope>")).await.unwrap()
}

fn config(kind: &str, token: &str) -> String {
    format!("<m:Configuration><m:Type>{kind}</m:Type><m:Token>{token}</m:Token></m:Configuration>")
}

fn success(xml: &str, response: &str) {
    let body = parse_soap_body(xml).unwrap();
    assert_eq!(body.children.len(), 1);
    assert_eq!(find_response(&body, response).unwrap().local_name, response);
}

fn fault(xml: &str, code: &str, first: &str, last: &str, reason: &str) {
    let body = parse_soap_body(xml).unwrap();
    assert_eq!(body.children[0].local_name, "Fault", "{xml}");
    let fault = body.child("Fault").unwrap();
    assert_eq!(fault.path(&["Code", "Value"]).unwrap().text(), code);
    assert_eq!(
        fault.path(&["Code", "Subcode", "Value"]).unwrap().text(),
        first
    );
    assert_eq!(
        fault
            .path(&["Code", "Subcode", "Subcode", "Value"])
            .unwrap()
            .text(),
        last
    );
    assert_eq!(fault.path(&["Reason", "Text"]).unwrap().text(), reason);
}

async fn assembly(t: &dyn Transport, url: &str, state: &MockState, hooks: &AtomicUsize) {
    let initial = hooks.load(Ordering::SeqCst);
    let created = post(
        t,
        url,
        M2,
        "CreateProfile",
        &format!(
            "<m:Name>assembly-901</m:Name>{}{}{}",
            config("VideoSource", "VSC_2"),
            config("VideoEncoder", "VEC_3"),
            config("All", "ignored")
        ),
    )
    .await;
    let body = parse_soap_body(&created).unwrap();
    let token = find_response(&body, "CreateProfileResponse")
        .unwrap()
        .child("Token")
        .unwrap()
        .text()
        .to_owned();
    {
        let state = state.read();
        let p = state
            .profiles
            .profiles
            .iter()
            .find(|p| p.token == token)
            .unwrap();
        assert_eq!(p.video_source_config_token.as_deref(), Some("VSC_2"));
        assert_eq!(p.video_encoder_config_token.as_deref(), Some("VEC_3"));
        assert_eq!(p.name, "assembly-901");
        assert!(!p.fixed);
        let refs = state
            .profiles
            .profiles
            .iter()
            .filter(|p| p.video_encoder_config_token.as_deref() == Some("VEC_3"))
            .count();
        assert_eq!(
            state
                .video_encoders
                .iter()
                .find(|c| c.token == "VEC_3")
                .unwrap()
                .use_count as usize,
            refs
        );
    }
    assert_eq!(hooks.load(Ordering::SeqCst), initial + 1);
    let profile = format!("<m:ProfileToken>{token}</m:ProfileToken>");
    let rename = post(
        t,
        url,
        M2,
        "AddConfiguration",
        &format!("{profile}<m:Name> new &amp; 名 </m:Name>"),
    )
    .await;
    success(&rename, "AddConfigurationResponse");
    assert_eq!(
        state
            .read()
            .profiles
            .profiles
            .iter()
            .find(|p| p.token == token)
            .unwrap()
            .name,
        " new & 名 "
    );
    let before = serde_json::to_value(&*state.read()).unwrap();
    let count = hooks.load(Ordering::SeqCst);
    let rejected = post(
        t,
        url,
        M2,
        "AddConfiguration",
        &format!(
            "{profile}<m:Name>must-not-commit</m:Name>{}",
            config("VideoEncoder", "missing-901")
        ),
    )
    .await;
    fault(
        &rejected,
        "s:Sender",
        "ter:InvalidArgVal",
        "ter:NoConfig",
        "NoSuchConfig-ADDCFG2-5543: missing-901",
    );
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
    assert_eq!(hooks.load(Ordering::SeqCst), count);
    let conflict = post(
        t,
        url,
        M2,
        "AddConfiguration",
        &format!(
            "{profile}{}{}",
            config("VideoEncoder", "VEC_1"),
            config("VideoEncoder", "VEC_3")
        ),
    )
    .await;
    fault(
        &conflict,
        "s:Receiver",
        "ter:Action",
        "ter:ConfigurationConflict",
        "Conflicting configurations for one mock profile slot",
    );
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
    assert_eq!(hooks.load(Ordering::SeqCst), count);
    for offset in [1, 2] {
        let removed = post(
            t,
            url,
            M2,
            "RemoveConfiguration",
            &format!("{profile}{}", config("All", "ignored-902")),
        )
        .await;
        success(&removed, "RemoveConfigurationResponse");
        let snapshot = state.read();
        let p = snapshot
            .profiles
            .profiles
            .iter()
            .find(|p| p.token == token)
            .unwrap();
        assert_eq!(
            (
                &p.video_source_config_token,
                &p.video_encoder_config_token,
                &p.audio_source_config_token,
                &p.audio_encoder_config_token,
                &p.ptz_config_token
            ),
            (&None, &None, &None, &None, &None)
        );
        assert_eq!(p.name, " new & 名 ");
        assert_eq!(hooks.load(Ordering::SeqCst), count + offset);
    }
    for ns in [M1, M2] {
        let fields = if ns == M1 {
            String::new()
        } else {
            "<m:Type>All</m:Type>".into()
        };
        let read = post(t, url, ns, "GetProfiles", &fields).await;
        success(&read, "GetProfilesResponse");
        assert!(read.contains(" new &amp; 名 "));
    }
    let deleted = post(t, url, M1, "DeleteProfile", &profile).await;
    success(&deleted, "DeleteProfileResponse");
    assert!(
        !state
            .read()
            .profiles
            .profiles
            .iter()
            .any(|p| p.token == token)
    );
}

async fn capacity(t: &dyn Transport, url: &str, state: &MockState) {
    for ns in [M1, M2] {
        let xml = post(t, url, ns, "GetServiceCapabilities", "").await;
        let body = parse_soap_body(&xml).unwrap();
        let maximum: usize = body.children[0]
            .path(&["Capabilities", "ProfileCapabilities"])
            .unwrap()
            .attr("MaximumNumberOfProfiles")
            .unwrap()
            .parse()
            .unwrap();
        while state.read().profiles.profiles.len() < maximum {
            let xml = post(t, url, ns, "CreateProfile", "<m:Name>capacity-903</m:Name>").await;
            success(&xml, "CreateProfileResponse");
        }
        let before = serde_json::to_value(&*state.read()).unwrap();
        let xml = post(t, url, ns, "CreateProfile", "<m:Name>overflow-903</m:Name>").await;
        fault(
            &xml,
            "s:Receiver",
            "ter:Action",
            "ter:MaxNVTProfiles",
            "Mock profile capacity reached",
        );
        assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
    }
}

async fn binding_fields(t: &dyn Transport, url: &str, state: &MockState) {
    // This token belongs to the source catalogue only and deliberately needs XML
    // decoding. The independent control value stays different from its spelling.
    state.modify(|s| {
        let mut extra = s.video_source_configs[1].clone();
        extra.token = " source &amp; <北> ".into();
        extra.use_count = 0;
        s.video_source_configs.push(extra);
    });
    let encoded = " source &amp;amp; &lt;北&gt; ";
    for (ns, op, fields) in [
        (
            M1,
            "AddVideoSourceConfiguration",
            format!(
                "<m:ProfileToken>Profile_4</m:ProfileToken><m:ConfigurationToken>{encoded}</m:ConfigurationToken>"
            ),
        ),
        (
            M2,
            "AddConfiguration",
            format!(
                "<m:ProfileToken>Profile_4</m:ProfileToken>{}{}",
                config("VideoSource", encoded),
                config("VideoSource", encoded)
            ),
        ),
    ] {
        let xml = post(t, url, ns, op, &fields).await;
        success(&xml, &format!("{op}Response"));
        assert_eq!(
            state.read().profiles.profiles[3]
                .video_source_config_token
                .as_deref(),
            Some(" source &amp; <北> ")
        );
        assert_eq!(
            state.read().video_source_configs.last().unwrap().use_count,
            1
        );
    }
    let before = serde_json::to_value(&*state.read()).unwrap();
    for bad in [
        "<m:Configuration><m:Type>VideoSource</m:Type><m:Type>VideoEncoder</m:Type><m:Token>VSC_1</m:Token></m:Configuration>",
        "<m:Configuration><m:Type>VideoSource</m:Type><m:Token>VSC_1</m:Token><m:Token>VSC_2</m:Token></m:Configuration>",
        "<m:Configuration><m:Type>VideoSource</m:Type><m:Token><m:Nested>VSC_1</m:Nested></m:Token></m:Configuration>",
    ] {
        for (op, prefix) in [
            ("CreateProfile", "<m:Name>rejected-904</m:Name>"),
            (
                "AddConfiguration",
                "<m:ProfileToken>Profile_4</m:ProfileToken><m:Name>do-not-rename</m:Name>",
            ),
        ] {
            let xml = post(t, url, M2, op, &format!("{prefix}{bad}")).await;
            let parsed = parse_soap_body(&xml).unwrap();
            assert_eq!(
                find_response(&parsed, "unused").unwrap_err(),
                oxvif::soap::SoapError::Fault {
                    code: "s:Sender".into(),
                    reason: "Invalid Args".into(),
                    subcode: Some("ter:InvalidArgs".into()),
                    detail: None,
                }
            );
            assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
        }
    }
    // A late unknown reference cannot allocate a profile/counter or publish a
    // valid earlier binding. The error includes the distinctive missing token.
    let xml = post(
        t,
        url,
        M2,
        "CreateProfile",
        &format!(
            "<m:Name>rollback-905</m:Name>{}{}",
            config("VideoSource", "VSC_1"),
            config("PTZ", "absent-905")
        ),
    )
    .await;
    fault(
        &xml,
        "s:Sender",
        "ter:InvalidArgVal",
        "ter:NoConfig",
        "NoSuchConfig-CREATECFG2: absent-905",
    );
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
}

#[tokio::test]
async fn profile_binding_fields_are_scoped_and_atomic() {
    let t = MockTransport::new();
    binding_fields(&t, "http://mock", t.device()).await;
}

#[cfg(feature = "metamorph")]
fn recordings() -> oxvif::metamorph::FixtureStore {
    let mut store = oxvif::metamorph::FixtureStore::new("assembly-906");
    for ns in [M1, M2] {
        let body = format!("<m:GetVideoEncoderConfigurations xmlns:m='{ns}'/>");
        store.record(
            &format!("{ns}/GetVideoEncoderConfigurations"),
            &body,
            "<recorded-count-906/>",
        );
        store.record(
            &format!("{ns}/GetVideoEncoderConfigurationOptions"),
            &format!("<m:GetVideoEncoderConfigurationOptions xmlns:m='{ns}'><m:ProfileToken>Profile_4</m:ProfileToken></m:GetVideoEncoderConfigurationOptions>"),
            "<recorded-options-966/>",
        );
    }
    store.record(
        &format!("{M2}/GetVideoEncoderInstances"),
        &format!("<m:GetVideoEncoderInstances xmlns:m='{M2}'/>"),
        "<recorded-instances-907/>",
    );
    store
}

#[cfg(feature = "metamorph")]
async fn replay_counts(t: &dyn Transport, url: &str) {
    async fn read(t: &dyn Transport, url: &str, ns: &str) -> String {
        t.soap_post(
            url,
            &format!("{ns}/GetVideoEncoderConfigurations"),
            format!("<m:GetVideoEncoderConfigurations xmlns:m='{ns}'/>"),
        )
        .await
        .unwrap()
    }
    for ns in [M1, M2] {
        assert_eq!(read(t, url, ns).await, "<recorded-count-906/>");
    }
    let xml = post(
        t,
        url,
        M2,
        "AddConfiguration",
        &format!(
            "<m:ProfileToken>Profile_4</m:ProfileToken>{}",
            config("VideoEncoder", "missing-906")
        ),
    )
    .await;
    fault(
        &xml,
        "s:Sender",
        "ter:InvalidArgVal",
        "ter:NoConfig",
        "NoSuchConfig-ADDCFG2-5543: missing-906",
    );
    for ns in [M1, M2] {
        assert_eq!(read(t, url, ns).await, "<recorded-count-906/>");
        assert_eq!(t.soap_post(url, &format!("{ns}/GetVideoEncoderConfigurationOptions"),
            format!("<m:GetVideoEncoderConfigurationOptions xmlns:m='{ns}'><m:ProfileToken>Profile_4</m:ProfileToken></m:GetVideoEncoderConfigurationOptions>")).await.unwrap(), "<recorded-options-966/>");
    }
    let xml = post(
        t,
        url,
        M2,
        "AddConfiguration",
        &format!(
            "<m:ProfileToken>Profile_4</m:ProfileToken>{}",
            config("VideoEncoder", "VEC_3")
        ),
    )
    .await;
    success(&xml, "AddConfigurationResponse");
    for ns in [M1, M2] {
        let xml = read(t, url, ns).await;
        let parsed = parse_soap_body(&xml).unwrap();
        let response = find_response(&parsed, "GetVideoEncoderConfigurationsResponse").unwrap();
        let encoder = response
            .children_named("Configurations")
            .find(|c| c.attr("token") == Some("VEC_3"))
            .unwrap();
        assert_eq!(encoder.child("UseCount").unwrap().text(), "2");
        let options = t.soap_post(url, &format!("{ns}/GetVideoEncoderConfigurationOptions"),
            format!("<m:GetVideoEncoderConfigurationOptions xmlns:m='{ns}'><m:ProfileToken>Profile_4</m:ProfileToken></m:GetVideoEncoderConfigurationOptions>")).await.unwrap();
        assert_eq!(
            parse_soap_body(&options).unwrap().children[0].local_name,
            "GetVideoEncoderConfigurationOptionsResponse"
        );
        assert!(options.contains("<tt:Width>2592</tt:Width>"));
    }
    assert_eq!(
        t.soap_post(
            url,
            &format!("{M2}/GetVideoEncoderInstances"),
            format!("<m:GetVideoEncoderInstances xmlns:m='{M2}'/>"),
        )
        .await
        .unwrap(),
        "<recorded-instances-907/>"
    );
}

#[cfg(feature = "metamorph")]
#[tokio::test]
async fn profile_commit_retires_recorded_reference_counts() {
    let t = oxvif::metamorph::MetamorphTransport::new(recordings());
    replay_counts(&t, "http://mock").await;
}

#[cfg(feature = "metamorph-server")]
#[tokio::test]
async fn http_profile_commit_retires_recorded_reference_counts() {
    let server = oxvif::mock::MockServer::builder()
        .replay(recordings())
        .start()
        .await
        .unwrap();
    replay_counts(
        &oxvif::transport::HttpTransport::default(),
        server.device_url(),
    )
    .await;
}

#[tokio::test]
async fn profile_assembly_preserves_atomic_intent() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let mut state = MockState::new();
    let captured = hooks.clone();
    state.set_on_change(Arc::new(move |_| {
        captured.fetch_add(1, Ordering::SeqCst);
    }));
    let t = MockTransport::with_state(state);
    assembly(&t, "http://mock", t.device(), &hooks).await;
}

#[tokio::test]
async fn profile_creation_obeys_advertised_capacity() {
    let t = MockTransport::new();
    capacity(&t, "http://mock", t.device()).await;
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn http_profile_assembly_and_capacity() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let captured = hooks.clone();
    let server = oxvif::mock::MockServer::builder()
        .on_change(Arc::new(move |_| {
            captured.fetch_add(1, Ordering::SeqCst);
        }))
        .start()
        .await
        .unwrap();
    let t = oxvif::transport::HttpTransport::default();
    let url = server.device_url();
    assembly(&t, url, server.device(), &hooks).await;
    binding_fields(&t, url, server.device()).await;
    capacity(&t, url, server.device()).await;
}
