//! Executable audit of known gaps, NOT conformance acceptance.
//! When a fix changes a result, replace that expectation with the corrected
//! invariant and update K13-K16 in the source audit and operation cards. Never
//! restore a defect merely to make this baseline green. No hardware is used.
#![cfg(feature = "mock")]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use oxvif::{
    OnvifClient, OnvifError,
    mock::{MockState, MockTransport},
    soap::{SoapError, parse_soap_body},
    transport::Transport,
};

fn assert_fault(error: OnvifError, expected_code: &str, expected_reason: &str) {
    match error {
        OnvifError::Soap(SoapError::Fault { code, reason, .. }) => {
            assert_eq!(code, expected_code);
            assert_eq!(reason, expected_reason);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn known_gap_k13_generated_profile_token_collides_with_seeded_token() {
    for media2 in [false, true] {
        let transport = MockTransport::new();
        transport.device().modify(|state| {
            state.profiles.next_token_id = 42;
            state.profiles.profiles[0].token = "Profile_42".to_owned();
        });
        let client = OnvifClient::new("http://mock").with_transport(Arc::new(transport.clone()));
        let token = if media2 {
            client
                .create_profile_media2("http://mock", "K13-new")
                .await
                .unwrap()
        } else {
            client
                .create_profile("http://mock", "K13-new", None)
                .await
                .unwrap()
                .token
        };
        assert_eq!(token, "Profile_42");
        let state = transport.device().read();
        assert_eq!(
            state
                .profiles
                .profiles
                .iter()
                .filter(|p| p.token == token)
                .count(),
            2,
            "K13 changed: replace the known-gap baseline with a uniqueness regression"
        );
        assert_eq!(state.profiles.next_token_id, 43);
    }
}

#[tokio::test]
async fn known_gap_k14_rejected_delete_notifies_change_hook() {
    for media2 in [false, true] {
        let mut state = MockState::new();
        let before = state.read().profiles.profiles.len();
        let notifications = Arc::new(AtomicUsize::new(0));
        let capture = Arc::clone(&notifications);
        state.set_on_change(Arc::new(move |_| {
            capture.fetch_add(1, Ordering::SeqCst);
        }));
        let transport = MockTransport::with_state(state);
        let client = OnvifClient::new("http://mock").with_transport(Arc::new(transport.clone()));
        let error = if media2 {
            client
                .delete_profile_media2("http://mock", "K14-absent")
                .await
                .unwrap_err()
        } else {
            client
                .delete_profile("http://mock", "K14-absent")
                .await
                .unwrap_err()
        };
        assert_fault(error, "ter:NoProfile", "Profile not found: K14-absent");
        assert_eq!(transport.device().read().profiles.profiles.len(), before);
        assert_eq!(
            notifications.load(Ordering::SeqCst),
            1,
            "K14 changed: replace the known-gap baseline with a no-notification regression"
        );
    }
}

#[tokio::test]
async fn known_gap_k15_profile_name_is_interpreted_as_markup() {
    for (version, prefix) in [("ver10", "trt"), ("ver20", "tr2")] {
        let transport = MockTransport::new();
        transport.device().modify(|state| {
            state.profiles.profiles[0].name =
                "before<AuditMarker>injected</AuditMarker>after".to_owned();
        });
        let ns = format!("http://www.onvif.org/{version}/media/wsdl");
        let xml = transport
            .soap_post(
                "http://mock",
                &format!("{ns}/GetProfiles"),
                format!("<{prefix}:GetProfiles xmlns:{prefix}='{ns}'/>"),
            )
            .await
            .unwrap();
        let body = parse_soap_body(&xml).unwrap();
        let name = body
            .path(&["GetProfilesResponse", "Profiles", "Name"])
            .unwrap();
        assert_eq!(
            name.child("AuditMarker")
                .expect("K15 changed: assert literal escaped text after the fix")
                .text(),
            "injected"
        );
    }
}

#[tokio::test]
async fn known_gap_k16_late_invalid_binding_leaves_first_write_applied() {
    let transport = MockTransport::new();
    let (profile, config) = {
        let state = transport.device().read();
        (
            state.profiles.profiles[0].token.clone(),
            state.video_source_configs[0].token.clone(),
        )
    };
    transport.device().modify(|state| {
        state.profiles.profiles[0].video_source_config_token = None;
    });
    let ns = "http://www.onvif.org/ver20/media/wsdl";
    let xml = transport.soap_post("http://mock", &format!("{ns}/AddConfiguration"), format!(
        "<m:AddConfiguration xmlns:m='{ns}'><m:ProfileToken>{profile}</m:ProfileToken>\
         <m:Configuration><m:Type>VideoSource</m:Type><m:Token>{config}</m:Token></m:Configuration>\
         <m:Configuration><m:Type>VideoEncoder</m:Type><m:Token>K16-absent</m:Token></m:Configuration>\
         </m:AddConfiguration>"
    )).await.unwrap();
    let body = parse_soap_body(&xml).unwrap();
    let fault = body.child("Fault").unwrap();
    assert_eq!(
        fault.path(&["Code", "Value"]).unwrap().text(),
        "ter:NoConfig"
    );
    assert_eq!(
        fault.path(&["Reason", "Text"]).unwrap().text(),
        "NoSuchConfig-ADDCFG2-5543: K16-absent"
    );
    assert_eq!(
        transport.device().read().profiles.profiles[0]
            .video_source_config_token
            .as_deref(),
        Some(config.as_str()),
        "K16 changed: replace the known-gap baseline with an atomicity regression"
    );
}
