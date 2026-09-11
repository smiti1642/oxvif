//! Project-authored profile identity and binding probes; not a schema fixture set.
#![cfg(feature = "mock")]

use oxvif::{
    OnvifClient,
    mock::{MockState, MockTransport},
    soap::{SoapError, find_response, parse_soap_body},
    transport::Transport,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const M1: &str = "http://www.onvif.org/ver10/media/wsdl";
const M2: &str = "http://www.onvif.org/ver20/media/wsdl";

async fn request(transport: &dyn Transport, url: &str, ns: &str, op: &str, fields: &str) -> String {
    transport.soap_post(url, &format!("{ns}/{op}"), format!(
        "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:m='{ns}' xmlns:x='urn:decoy'><s:Header><m:Token>header-token</m:Token><m:ProfileToken>Profile_4</m:ProfileToken></s:Header><s:Body><m:{op}>{fields}</m:{op}></s:Body></s:Envelope>"
    )).await.unwrap()
}

async fn workflow(
    transport: Arc<dyn Transport>,
    url: &str,
    state: &MockState,
    hooks: &AtomicUsize,
) {
    let client = OnvifClient::new(url).with_transport(transport.clone());
    let original = state.read().clone();
    for (index, (literal, encoded)) in [
        ("plain-identity", "plain-identity"),
        ("  edge spaces  ", "  edge spaces  "),
        (
            "amp &amp; <tag> \"'\r\n\tend",
            "amp &amp;amp; &lt;tag&gt; &quot;&apos;&#13;&#10;&#9;end",
        ),
        (" \r\n\t ", " &#13;&#10;&#9; "),
    ]
    .into_iter()
    .enumerate()
    {
        let notifications = hooks.load(Ordering::SeqCst);
        let response = request(
            transport.as_ref(),
            url,
            M1,
            "CreateProfile",
            &format!("<m:Name>identity-probe</m:Name><m:Token>{encoded}</m:Token>"),
        )
        .await;
        assert!(
            state
                .read()
                .profiles
                .profiles
                .iter()
                .any(|p| p.token == literal),
            "literal state: {response}"
        );
        let body = parse_soap_body(&response).unwrap();
        let created = find_response(&body, "CreateProfileResponse")
            .unwrap()
            .child("Profile")
            .unwrap();
        assert_eq!(created.attr("token"), Some(literal));
        assert!(
            response.contains(&format!("token=\"{encoded}\"")),
            "attribute encoding: {response}"
        );
        assert_eq!(hooks.load(Ordering::SeqCst), notifications + 1);

        let fetched = client.get_profile(url, literal).await.unwrap();
        assert_eq!(fetched.token, literal);
        assert_eq!(fetched.name, "identity-probe");
        assert!(
            client
                .get_profiles(url)
                .await
                .unwrap()
                .iter()
                .any(|p| p.token == literal)
        );
        assert!(
            client
                .get_profiles_media2(url)
                .await
                .unwrap()
                .iter()
                .any(|p| p.token == literal)
        );
        let selected = request(
            transport.as_ref(),
            url,
            M2,
            "GetProfiles",
            &format!("<m:Token>{encoded}</m:Token>"),
        )
        .await;
        assert!(
            selected.contains(&format!("token=\"{encoded}\"")),
            "Media2 attribute encoding: {selected}"
        );
        assert_eq!(
            find_response(&parse_soap_body(&selected).unwrap(), "GetProfilesResponse")
                .unwrap()
                .children_named("Profiles")
                .count(),
            1
        );

        for step in 0..6 {
            match step {
                0 => client
                    .add_video_source_configuration(url, literal, "VSC_2")
                    .await
                    .unwrap(),
                1 => client
                    .add_video_encoder_configuration(url, literal, "VEC_3")
                    .await
                    .unwrap(),
                2 => client
                    .add_configuration_media2(url, literal, "PTZ", "PTZConfig_2")
                    .await
                    .unwrap(),
                3 => client
                    .remove_video_source_configuration(url, literal)
                    .await
                    .unwrap(),
                4 => client
                    .remove_video_encoder_configuration(url, literal)
                    .await
                    .unwrap(),
                5 => client
                    .remove_configuration_media2(url, literal, "PTZ", "PTZConfig_2")
                    .await
                    .unwrap(),
                _ => unreachable!(),
            }
            let profile = state
                .read()
                .profiles
                .profiles
                .iter()
                .find(|p| p.token == literal)
                .unwrap()
                .clone();
            assert_eq!(
                profile.video_source_config_token.as_deref(),
                (step < 3).then_some("VSC_2")
            );
            assert_eq!(
                profile.video_encoder_config_token.as_deref(),
                ((1..4).contains(&step)).then_some("VEC_3")
            );
            assert_eq!(
                profile.ptz_config_token.as_deref(),
                ((2..5).contains(&step)).then_some("PTZConfig_2")
            );
            let first = client.get_profile(url, literal).await.unwrap();
            let second = client
                .get_profiles_media2(url)
                .await
                .unwrap()
                .into_iter()
                .find(|p| p.token == literal)
                .unwrap();
            assert_eq!(first.ptz_config_token, profile.ptz_config_token);
            assert_eq!(second.ptz_config_token, profile.ptz_config_token);
            assert_eq!(hooks.load(Ordering::SeqCst), notifications + step + 2);
            if step == 2 {
                assert_eq!(
                    client.ptz_get_status(url, literal).await.unwrap().zoom,
                    Some(0.8)
                );
            }
        }

        if index % 2 == 0 {
            client.delete_profile(url, literal).await.unwrap();
        } else {
            client.delete_profile_media2(url, literal).await.unwrap();
        }
        assert_eq!(hooks.load(Ordering::SeqCst), notifications + 8);
        let mut actual = state.read().clone();
        actual.profiles.next_token_id = original.profiles.next_token_id;
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(&original).unwrap()
        );
    }

    let created = client
        .create_profile(url, "client-generated", Some("client &amp;\r\n\tend"))
        .await
        .unwrap();
    assert_eq!(created.token, "client &amp;\r\n\tend");
    assert_eq!(
        client.get_profile(url, &created.token).await.unwrap().token,
        created.token
    );
    client
        .delete_profile_media2(url, &created.token)
        .await
        .unwrap();
}

async fn refusals(transport: &dyn Transport, url: &str, state: &MockState, hooks: &AtomicUsize) {
    for (ns, op, field, rest) in [
        (
            M1,
            "CreateProfile",
            "Token",
            "<m:Name>identity-probe</m:Name>",
        ),
        (M1, "GetProfile", "ProfileToken", ""),
        (
            M1,
            "AddVideoSourceConfiguration",
            "ProfileToken",
            "<m:ConfigurationToken>VSC_2</m:ConfigurationToken>",
        ),
        (
            M1,
            "AddVideoEncoderConfiguration",
            "ProfileToken",
            "<m:ConfigurationToken>VEC_3</m:ConfigurationToken>",
        ),
        (M1, "RemoveVideoSourceConfiguration", "ProfileToken", ""),
        (M1, "RemoveVideoEncoderConfiguration", "ProfileToken", ""),
        (
            M2,
            "AddConfiguration",
            "ProfileToken",
            "<m:Configuration><m:Type>PTZ</m:Type><m:Token>PTZConfig_2</m:Token></m:Configuration>",
        ),
        (
            M2,
            "RemoveConfiguration",
            "ProfileToken",
            "<m:Configuration><m:Type>PTZ</m:Type></m:Configuration>",
        ),
    ] {
        for identity in [
            format!("<m:{field}>Profile_1</m:{field}><m:{field}>Profile_3</m:{field}>"),
            format!("<m:{field}><m:Nested>Profile_1</m:Nested></m:{field}>"),
        ] {
            let before = serde_json::to_value(&*state.read()).unwrap();
            let notifications = hooks.load(Ordering::SeqCst);
            let xml = request(transport, url, ns, op, &format!("{rest}{identity}")).await;
            assert_eq!(
                find_response(&parse_soap_body(&xml).unwrap(), &format!("{op}Response"))
                    .unwrap_err(),
                SoapError::Fault {
                    code: "s:Sender".into(),
                    subcode: Some("ter:InvalidArgs".into()),
                    reason: "Invalid Args".into(),
                    detail: None,
                },
                "{op}: {xml}"
            );
            assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
            assert_eq!(hooks.load(Ordering::SeqCst), notifications);
        }

        // This is a paired identity probe, not acceptance of arbitrary extensions.
        let baseline = state.read().clone();
        let control = MockTransport::with_state(MockState::with_state(baseline));
        let token = if op == "CreateProfile" {
            "decoy-control"
        } else {
            "Profile_3"
        };
        let valid = format!("{rest}<m:{field}>{token}</m:{field}>");
        let expected = request(&control, url, ns, op, &valid).await;
        find_response(
            &parse_soap_body(&expected).unwrap(),
            &format!("{op}Response"),
        )
        .unwrap();
        let decoys = format!(
            "<x:{field}>Profile_1</x:{field}><x:Extension><m:{field}>Profile_1</m:{field}></x:Extension>{valid}"
        );
        let notifications = hooks.load(Ordering::SeqCst);
        let actual = request(transport, url, ns, op, &decoys).await;
        assert_eq!(actual, expected, "direct field must win: {op}");
        assert_eq!(
            serde_json::to_value(&*state.read()).unwrap(),
            serde_json::to_value(&*control.device().read()).unwrap()
        );
        assert_eq!(
            hooks.load(Ordering::SeqCst),
            notifications + usize::from(op != "GetProfile")
        );
    }
}

#[tokio::test]
async fn in_process_media_profile_identity_survives_the_workflow() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let observed = hooks.clone();
    let mut state = MockState::new();
    state.set_on_change(Arc::new(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
    }));
    let transport = Arc::new(MockTransport::with_state(state));
    workflow(transport.clone(), "http://mock", transport.device(), &hooks).await;
    refusals(
        transport.as_ref(),
        "http://mock",
        transport.device(),
        &hooks,
    )
    .await;
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn http_media_profile_identity_survives_the_workflow() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let observed = hooks.clone();
    let server = oxvif::mock::MockServer::builder()
        .on_change(Arc::new(move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
        }))
        .start()
        .await
        .unwrap();
    let transport = Arc::new(oxvif::transport::HttpTransport::new());
    workflow(
        transport.clone(),
        server.device_url(),
        server.device(),
        &hooks,
    )
    .await;
    refusals(
        transport.as_ref(),
        server.device_url(),
        server.device(),
        &hooks,
    )
    .await;
}
