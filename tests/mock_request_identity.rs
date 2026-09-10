//! Direct identity regression probes, independent of client request fixtures.
#![cfg(feature = "mock")]

use oxvif::{OnvifClient, mock::MockTransport, transport::Transport};
use std::sync::Arc;

fn device(token: &str) -> MockTransport {
    let transport = MockTransport::new();
    transport.device().modify(|state| {
        let mut entry = state.profiles.profiles[0].clone();
        entry.token = token.to_owned();
        entry.fixed = false;
        state.profiles.profiles = vec![entry];
    });
    transport
}

async fn reject_action_aliases(transport: &dyn Transport, url: &str) {
    let ns = "http://www.onvif.org/ver10/device/wsdl";
    let body = format!(
        "<d:SetHostname xmlns:d='{ns}'><d:Name>must-not-write-941</d:Name></d:SetHostname>"
    );
    for action in [
        format!("{ns}/alias-941/SetHostname"),
        "https://invalid.example/events/wsdl/EventPortType/GetEventPropertiesRequest".into(),
        "http://www.onvif.org/ver10/events/wsdl/SubscriptionManager/RenewRequest".into(),
        "http://docs.oasis-open.org/wsn/bw-2/NotificationProducer/UnsubscribeRequest".into(),
    ] {
        let xml = transport
            .soap_post(url, &action, body.clone())
            .await
            .unwrap();
        assert_eq!(
            oxvif::soap::find_response(&oxvif::soap::parse_soap_body(&xml).unwrap(), "unused")
                .unwrap_err(),
            oxvif::soap::SoapError::Fault {
                code: "s:Receiver".into(),
                reason: format!("Not implemented: {action}"),
                subcode: None,
                detail: None,
            }
        );
    }
    let xml = transport
        .soap_post(url, &format!("{ns}/SetHostname"), body)
        .await
        .unwrap();
    assert_eq!(
        oxvif::soap::find_response(
            &oxvif::soap::parse_soap_body(&xml).unwrap(),
            "SetHostnameResponse"
        )
        .unwrap()
        .local_name,
        "SetHostnameResponse"
    );
}

#[tokio::test]
async fn in_process_action_identity_rejects_aliases_before_state_changes() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let notifications = Arc::new(AtomicUsize::new(0));
    let observed = notifications.clone();
    let mut state = oxvif::mock::MockState::new();
    state.set_on_change(Arc::new(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
    }));
    let transport = MockTransport::with_state(state);
    reject_action_aliases(&transport, "http://mock").await;
    assert_eq!(transport.device().read().hostname, "must-not-write-941");
    assert_eq!(notifications.load(Ordering::SeqCst), 1);
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn http_action_identity_rejects_aliases_before_state_changes() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let notifications = Arc::new(AtomicUsize::new(0));
    let observed = notifications.clone();
    let server = oxvif::mock::MockServer::builder()
        .on_change(Arc::new(move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
        }))
        .start()
        .await
        .unwrap();
    reject_action_aliases(&oxvif::transport::HttpTransport::new(), server.device_url()).await;
    assert_eq!(server.device().read().hostname, "must-not-write-941");
    assert_eq!(notifications.load(Ordering::SeqCst), 1);
}

// Fixed controls deletion, not binding. This control prevents the old incorrect
// bind_configuration comment from becoming a behavior change during hardening.
#[tokio::test]
async fn fixed_profile_configuration_remains_mutable_in_both_media_services() {
    for media2 in [false, true] {
        let transport = device("fixed-binding-control");
        let config = {
            let mut token = String::new();
            transport.device().modify(|state| {
                state.profiles.profiles[0].fixed = true;
                state.profiles.profiles[0].video_encoder_config_token = None;
                token = state.video_encoders[0].token.clone();
            });
            token
        };
        let ns = if media2 {
            "http://www.onvif.org/ver20/media/wsdl"
        } else {
            "http://www.onvif.org/ver10/media/wsdl"
        };
        let (add, remove, binding) = if media2 {
            (
                "AddConfiguration",
                "RemoveConfiguration",
                format!(
                    "<m:Configuration><m:Type>VideoEncoder</m:Type><m:Token>{config}</m:Token></m:Configuration>"
                ),
            )
        } else {
            (
                "AddVideoEncoderConfiguration",
                "RemoveVideoEncoderConfiguration",
                format!("<m:ConfigurationToken>{config}</m:ConfigurationToken>"),
            )
        };
        for (operation, attached) in [(add, true), (remove, false)] {
            let binding = if attached || media2 {
                binding.as_str()
            } else {
                ""
            };
            let body = format!(
                "<m:{operation} xmlns:m='{ns}'><m:ProfileToken>fixed-binding-control</m:ProfileToken>{binding}</m:{operation}>"
            );
            let xml = transport
                .soap_post("http://mock", &format!("{ns}/{operation}"), body)
                .await
                .unwrap();
            let response = oxvif::soap::parse_soap_body(&xml).unwrap();
            oxvif::soap::find_response(&response, &format!("{operation}Response")).unwrap();
            let state = transport.device().read();
            let entry = &state.profiles.profiles[0];
            assert_eq!(entry.token, "fixed-binding-control");
            assert!(entry.fixed);
            assert_eq!(
                entry.video_encoder_config_token.as_deref(),
                attached.then_some(config.as_str()),
                "configuration state must reflect {operation}; fixed does not mean immutable"
            );
        }
    }
}

#[tokio::test]
async fn delete_profile_preserves_escaped_and_whitespace_identity() {
    for token in ["Profile<&北門", "  Profile spaced  ", "literal&amp;value"] {
        for media2 in [false, true] {
            let transport = device(token);
            let client =
                OnvifClient::new("http://mock").with_transport(Arc::new(transport.clone()));
            let result = if media2 {
                client
                    .delete_profile_media2("http://mock/media2", token)
                    .await
            } else {
                client.delete_profile("http://mock/media", token).await
            };
            assert!(
                result.is_ok(),
                "valid token {token:?}, media2={media2}: {result:?}"
            );
            assert!(
                transport.device().read().profiles.profiles.is_empty(),
                "deletion must alter the addressed state"
            );
        }
    }
}

#[tokio::test]
async fn delete_profile_rejects_ambiguous_or_mislocated_identity_without_mutation() {
    for (version, field) in [("ver10", "ProfileToken"), ("ver20", "Token")] {
        let ns = format!("http://www.onvif.org/{version}/media/wsdl");
        let action = format!("{ns}/DeleteProfile");
        let good = format!("<m:{field}>target</m:{field}>");
        for (body, reason) in [
            (
                format!(
                    "<m:DeleteProfile xmlns:m='{ns}'><m:Extension>{good}</m:Extension></m:DeleteProfile>"
                ),
                "missing request field",
            ),
            (
                format!("<m:DeleteProfile xmlns:m='{ns}'>{good}{good}</m:DeleteProfile>"),
                "duplicate request field",
            ),
            (
                format!("<m:WrongOperation xmlns:m='{ns}'>{good}</m:WrongOperation>"),
                "unexpected operation or namespace",
            ),
            (
                format!("<m:DeleteProfile xmlns:m='urn:wrong'>{good}</m:DeleteProfile>"),
                "unexpected operation or namespace",
            ),
            (
                format!("<m:DeleteProfile xmlns:m='{ns}'>{good}</m:DeleteProfile><Extra/>"),
                "multiple document elements",
            ),
        ] {
            let transport = device("target");
            let xml = transport
                .soap_post("http://mock", &action, body)
                .await
                .unwrap();
            let body = oxvif::soap::parse_soap_body(&xml).unwrap();
            let fault = body.child("Fault").expect("invalid identity must fault");
            assert_eq!(fault.path(&["Code", "Value"]).unwrap().text(), "env:Sender");
            assert_eq!(
                fault.path(&["Reason", "Text"]).unwrap().text(),
                format!("InvalidRequest-DELETEPROFILE: {reason}")
            );
            let state = transport.device().read();
            assert_eq!(state.profiles.profiles.len(), 1);
            assert_eq!(state.profiles.profiles[0].token, "target");
        }
    }
}

#[tokio::test]
async fn unknown_token_fault_preserves_literal_text_and_state() {
    let token = "Missing<&北門";
    for media2 in [false, true] {
        let transport = device("target");
        let client = OnvifClient::new("http://mock").with_transport(Arc::new(transport.clone()));
        let error = if media2 {
            client
                .delete_profile_media2("http://mock/media2", token)
                .await
                .unwrap_err()
        } else {
            client
                .delete_profile("http://mock/media", token)
                .await
                .unwrap_err()
        };
        match error {
            oxvif::OnvifError::Soap(oxvif::soap::SoapError::Fault {
                code,
                reason,
                subcode,
                detail,
            }) => {
                assert_eq!(code, "s:Sender");
                assert_eq!(subcode.as_deref(), Some("ter:InvalidArgVal"));
                assert_eq!(detail, None);
                assert_eq!(reason, format!("Profile not found: {token}"));
            }
            other => panic!("wrong error for missing token: {other:?}"),
        }
        assert_eq!(
            transport.device().read().profiles.profiles[0].token,
            "target"
        );
    }
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn http_delete_uses_the_same_identity_rules() {
    let transport = device("HTTP<&北門");
    let state = transport.device().read().clone();
    let server = oxvif::mock::MockServer::builder()
        .initial_state(state)
        .start()
        .await
        .unwrap();
    let client = OnvifClient::new(server.device_url());
    client
        .delete_profile_media2(&format!("{}/onvif/media2", server.base_url()), "HTTP<&北門")
        .await
        .unwrap();
    assert!(server.device().read().profiles.profiles.is_empty());
}
