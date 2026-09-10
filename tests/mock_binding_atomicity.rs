//! Project-authored state/notification controls, not schema-conformance fixtures.
#![cfg(feature = "mock")]

use std::sync::{Arc, Mutex};

use oxvif::{
    OnvifClient,
    mock::{MockState, MockTransport},
    soap::parse_soap_body,
    transport::Transport,
};

type Observations = Arc<Mutex<Vec<(Option<String>, Option<String>)>>>;

fn hook(observed: &Observations) -> oxvif::mock::state::ChangeHook {
    let observed = observed.clone();
    Arc::new(move |state| {
        let profile = &state.profiles.profiles[0];
        observed.lock().unwrap().push((
            profile.video_source_config_token.clone(),
            profile.video_encoder_config_token.clone(),
        ));
    })
}

async fn request(
    transport: &dyn Transport,
    url: &str,
    operation: &str,
    profile: &str,
    second: &str,
) -> String {
    let ns = "http://www.onvif.org/ver20/media/wsdl";
    transport.soap_post(url, &format!("{ns}/{operation}"), format!(
        "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:m='{ns}'>\
         <s:Body><m:{operation}><m:ProfileToken>{profile}</m:ProfileToken>\
         <m:Configuration><m:Type>VideoSource</m:Type><m:Token>VSC_2</m:Token></m:Configuration>\
         <m:Configuration><m:Type>VideoEncoder</m:Type><m:Token>{second}</m:Token></m:Configuration>\
         </m:{operation}></s:Body></s:Envelope>"
    )).await.unwrap()
}

async fn exercise(
    transport: Arc<dyn Transport>,
    url: &str,
    state: &MockState,
    observed: &Observations,
) {
    let before = state.read().clone();
    let profile = &before.profiles.profiles[0];
    assert!(
        profile.fixed,
        "exercise legal binding changes on a fixed profile"
    );
    for (token, code, reason) in [
        (
            "absent-835",
            "ter:NoConfig",
            "NoSuchConfig-ADDCFG2-5543: absent-835",
        ),
        ("VSC_2", "ter:NoConfig", "NoSuchConfig-ADDCFG2-5543: VSC_2"),
        (
            "",
            "env:Sender",
            "NoToken-ADDCFG2-5543: ProfileToken and ConfigurationToken are both required",
        ),
    ] {
        let xml = request(
            transport.as_ref(),
            url,
            "AddConfiguration",
            &profile.token,
            token,
        )
        .await;
        let body = parse_soap_body(&xml).unwrap();
        let fault = body.child("Fault").expect("exact rejection response");
        assert_eq!(fault.path(&["Code", "Value"]).unwrap().text(), code);
        assert_eq!(fault.path(&["Reason", "Text"]).unwrap().text(), reason);
        assert_eq!(
            serde_json::to_value(&*state.read()).unwrap(),
            serde_json::to_value(&before).unwrap()
        );
        assert_eq!(*observed.lock().unwrap(), Vec::new());
    }

    let xml = request(
        transport.as_ref(),
        url,
        "AddConfiguration",
        &profile.token,
        "VEC_3",
    )
    .await;
    assert_eq!(
        parse_soap_body(&xml).unwrap().children[0].local_name,
        "AddConfigurationResponse"
    );
    let mut expected = before.clone();
    expected.profiles.profiles[0].video_source_config_token = Some("VSC_2".into());
    expected.profiles.profiles[0].video_encoder_config_token = Some("VEC_3".into());
    assert_eq!(
        serde_json::to_value(&*state.read()).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    assert_eq!(
        *observed.lock().unwrap(),
        vec![(Some("VSC_2".into()), Some("VEC_3".into()))]
    );
    let client = OnvifClient::new(url).with_transport(transport.clone());
    let media1 = client.get_profile(url, &profile.token).await.unwrap();
    assert_eq!(media1.video_source_config_token.as_deref(), Some("VSC_2"));
    assert_eq!(media1.video_encoder_token.as_deref(), Some("VEC_3"));
    let media2 = client.get_profiles_media2(url).await.unwrap();
    let media2 = media2.iter().find(|p| p.token == profile.token).unwrap();
    assert_eq!(media2.video_source_config_token.as_deref(), Some("VSC_2"));
    assert_eq!(media2.video_encoder_token.as_deref(), Some("VEC_3"));

    for count in [2, 3] {
        let xml = request(
            transport.as_ref(),
            url,
            "RemoveConfiguration",
            &profile.token,
            "ignored-836",
        )
        .await;
        assert_eq!(
            parse_soap_body(&xml).unwrap().children[0].local_name,
            "RemoveConfigurationResponse"
        );
        expected.profiles.profiles[0].video_source_config_token = None;
        expected.profiles.profiles[0].video_encoder_config_token = None;
        assert_eq!(
            serde_json::to_value(&*state.read()).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        let observations = observed.lock().unwrap();
        assert_eq!(observations.len(), count);
        assert_eq!(observations[count - 1], (None, None));
    }
}

#[tokio::test]
async fn in_process_binding_is_atomic_and_notifies_once() {
    let observed = Observations::default();
    let mut state = MockState::new();
    state.set_on_change(hook(&observed));
    let transport = MockTransport::with_state(state);
    exercise(
        Arc::new(transport.clone()),
        "http://mock",
        transport.device(),
        &observed,
    )
    .await;
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn http_binding_is_atomic_and_notifies_once() {
    use oxvif::{mock::MockServer, transport::HttpTransport};
    let observed = Observations::default();
    let server = MockServer::builder()
        .on_change(hook(&observed))
        .start()
        .await
        .unwrap();
    exercise(
        Arc::new(HttpTransport::default()),
        &format!("{}/onvif/media2", server.base_url()),
        server.device(),
        &observed,
    )
    .await;
}
