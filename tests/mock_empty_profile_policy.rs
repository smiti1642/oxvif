//! Project-specific mock policy controls, not a normative token lexical test.
#![cfg(feature = "mock")]

use oxvif::{
    OnvifClient, OnvifError,
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
const REASON: &str = "The mock does not support empty profile tokens";

fn policy(code: &str) -> SoapError {
    SoapError::Fault {
        code: code.into(),
        subcode: Some("mock:RequestPolicy".into()),
        reason: REASON.into(),
        detail: None,
    }
}

fn client_policy(error: OnvifError, code: &str) {
    let OnvifError::Soap(error) = error else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(error, policy(code));
}

async fn request(transport: &dyn Transport, url: &str, ns: &str, op: &str, fields: &str) -> String {
    transport
        .soap_post(
            url,
            &format!("{ns}/{op}"),
            format!("<m:{op} xmlns:m='{ns}'>{fields}</m:{op}>"),
        )
        .await
        .unwrap()
}

async fn exercise(
    transport: Arc<dyn Transport>,
    url: &str,
    state: &MockState,
    hooks: &AtomicUsize,
) {
    let client = OnvifClient::new(url).with_transport(transport.clone());
    let before = serde_json::to_value(&*state.read()).unwrap();
    let notifications = hooks.load(Ordering::SeqCst);
    let error = client
        .create_profile(url, "must-not-commit-empty", Some(""))
        .await
        .unwrap_err();
    assert_eq!(
        serde_json::to_value(&*state.read()).unwrap(),
        before,
        "client failure must not leave a committed empty-token profile"
    );
    assert_eq!(hooks.load(Ordering::SeqCst), notifications);
    client_policy(error, "s:Sender");

    for token in [
        "<m:Token/>",
        "<m:Token></m:Token>",
        "<m:Token><![CDATA[]]></m:Token>",
    ] {
        let xml = request(
            transport.as_ref(),
            url,
            M1,
            "CreateProfile",
            &format!("<m:Name>empty-raw</m:Name>{token}"),
        )
        .await;
        assert_eq!(
            find_response(&parse_soap_body(&xml).unwrap(), "CreateProfileResponse").unwrap_err(),
            policy("s:Sender")
        );
        assert!(xml.contains("xmlns:mock=\"urn:oxvif:mock:error\""));
        assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
        assert_eq!(hooks.load(Ordering::SeqCst), notifications);
    }

    // Omission allocates, including after refusals: they cannot consume the hint.
    let created = client
        .create_profile(url, "allocated-control", None)
        .await
        .unwrap();
    assert_eq!(created.token, "Profile_5");
    assert_eq!(created.name, "allocated-control");
    assert_eq!(hooks.load(Ordering::SeqCst), notifications + 1);
    client
        .delete_profile_media2(url, &created.token)
        .await
        .unwrap();

    // Preserve a legacy invalid seed verbatim; reads must disclose it, not erase it.
    state.modify(|device| {
        let mut invalid = device.profiles.profiles[3].clone();
        invalid.token.clear();
        invalid.name = "legacy-empty-identity".into();
        device.profiles.profiles.push(invalid);
    });
    let before = serde_json::to_value(&*state.read()).unwrap();
    let notifications = hooks.load(Ordering::SeqCst);
    client_policy(client.get_profiles(url).await.unwrap_err(), "s:Receiver");
    client_policy(
        client.get_profiles_media2(url).await.unwrap_err(),
        "s:Receiver",
    );
    client_policy(client.get_profile(url, "").await.unwrap_err(), "s:Sender");
    client_policy(
        client
            .create_profile(url, "still-empty", Some(""))
            .await
            .unwrap_err(),
        "s:Sender",
    );
    for (ns, op, fields, code) in [
        (M1, "GetProfiles", "", "s:Receiver"),
        (M2, "GetProfiles", "", "s:Receiver"),
        (M1, "GetProfile", "<m:ProfileToken/>", "s:Sender"),
        (M2, "GetProfiles", "<m:Token/>", "s:Sender"),
    ] {
        let xml = request(transport.as_ref(), url, ns, op, fields).await;
        assert_eq!(
            find_response(&parse_soap_body(&xml).unwrap(), &format!("{op}Response")).unwrap_err(),
            policy(code)
        );
    }
    let missing = request(transport.as_ref(), url, M1, "GetProfile", "").await;
    assert_eq!(
        find_response(&parse_soap_body(&missing).unwrap(), "GetProfileResponse").unwrap_err(),
        SoapError::Fault {
            code: "ter:NoProfile".into(),
            subcode: None,
            reason: "Profile not found:".into(),
            detail: None,
        }
    );
    assert_eq!(
        client.get_profile(url, "Profile_1").await.unwrap().token,
        "Profile_1"
    );
    let valid = request(
        transport.as_ref(),
        url,
        M2,
        "GetProfiles",
        "<m:Token>Profile_1</m:Token>",
    )
    .await;
    let body = parse_soap_body(&valid).unwrap();
    let profiles = find_response(&body, "GetProfilesResponse").unwrap();
    assert_eq!(profiles.children_named("Profiles").count(), 1);
    assert_eq!(
        profiles.child("Profiles").unwrap().attr("token"),
        Some("Profile_1")
    );
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
    assert_eq!(hooks.load(Ordering::SeqCst), notifications);
}

#[tokio::test]
async fn in_process_empty_profile_identity_is_an_explicit_policy_refusal() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let observed = hooks.clone();
    let mut state = MockState::new();
    state.set_on_change(Arc::new(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
    }));
    let transport = Arc::new(MockTransport::with_state(state));
    exercise(transport.clone(), "http://mock", transport.device(), &hooks).await;
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn http_empty_profile_identity_is_an_explicit_policy_refusal() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let observed = hooks.clone();
    let server = oxvif::mock::MockServer::builder()
        .on_change(Arc::new(move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
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
