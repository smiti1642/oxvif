//! Executable audit of known gaps and repaired invariants, NOT conformance acceptance.
//! When a fix changes a result, replace that expectation with the corrected
//! invariant and update K13-K18 in the audit/preflight and operation cards. Never
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

#[cfg(feature = "metamorph")]
use oxvif::metamorph::{FixtureStore, MetamorphTransport};

// Raw markers deliberately test replay identity, not a normative SOAP fixture.
#[cfg(feature = "metamorph")]
fn profile_replay() -> (MetamorphTransport, String, String, String) {
    let state = MockState::new();
    state.modify(|s| s.profiles.profiles[0].name = "synthetic-K17-K18".to_owned());
    let token = state.read().profiles.profiles[0].token.clone();
    let ns = "http://www.onvif.org/ver10/media/wsdl";
    let one = format!(
        "<m:GetProfile xmlns:m='{ns}'><m:ProfileToken>{token}</m:ProfileToken></m:GetProfile>"
    );
    let all = format!("<m:GetProfiles xmlns:m='{ns}'/>");
    let mut store = FixtureStore::new("synthetic-gap-controls");
    store.record(&format!("{ns}/GetProfile"), &one, "<recorded-one/>");
    store.record(&format!("{ns}/GetProfiles"), &all, "<recorded-all/>");
    (
        MetamorphTransport::new(store).with_state(state),
        ns.to_owned(),
        one,
        all,
    )
}

#[cfg(feature = "metamorph")]
#[tokio::test]
async fn rejected_delete_preserves_unchanged_profile_recording() {
    let (transport, ns, one, _) = profile_replay();
    let before = serde_json::to_value(&*transport.device().read()).unwrap();
    assert_eq!(
        transport
            .soap_post("http://mock", &format!("{ns}/GetProfile"), one.clone())
            .await
            .unwrap(),
        "<recorded-one/>"
    );
    let client = OnvifClient::new("http://mock").with_transport(Arc::new(transport.clone()));
    assert_fault(
        client
            .delete_profile("http://mock", "K17-absent")
            .await
            .unwrap_err(),
        "s:Sender",
        "Profile not found: K17-absent",
    );
    assert_eq!(
        serde_json::to_value(&*transport.device().read()).unwrap(),
        before
    );
    let after = transport
        .soap_post("http://mock", &format!("{ns}/GetProfile"), one)
        .await
        .unwrap();
    assert_eq!(
        after, "<recorded-one/>",
        "rejected deletion must preserve the recorded response"
    );
}

#[cfg(feature = "metamorph")]
#[tokio::test]
async fn known_gap_k18_successful_create_leaves_recorded_profile_list_stale() {
    let (transport, ns, one, all) = profile_replay();
    let before = transport.device().read().profiles.profiles.len();
    assert_eq!(
        transport
            .soap_post("http://mock", &format!("{ns}/GetProfiles"), all.clone())
            .await
            .unwrap(),
        "<recorded-all/>"
    );
    let client = OnvifClient::new("http://mock").with_transport(Arc::new(transport.clone()));
    let created = client
        .create_profile("http://mock", "K18-created", Some("K18-token"))
        .await
        .unwrap();
    assert_eq!(created.token, "K18-token");
    {
        let state = transport.device().read();
        assert_eq!(state.profiles.profiles.len(), before + 1);
        assert_eq!(
            state
                .profiles
                .profiles
                .iter()
                .find(|p| p.token == created.token)
                .unwrap()
                .name,
            "K18-created"
        );
    }
    assert_eq!(
        transport
            .soap_post("http://mock", &format!("{ns}/GetProfiles"), all)
            .await
            .unwrap(),
        "<recorded-all/>",
        "K18 changed: replace the baseline with a list reflecting the committed create"
    );
    // The singular family was retired: this is a dependency mismatch, not a
    // write that failed to run or a replay responder that was never reached.
    let singular = transport
        .soap_post("http://mock", &format!("{ns}/GetProfile"), one)
        .await
        .unwrap();
    assert_eq!(
        parse_soap_body(&singular)
            .unwrap()
            .path(&["GetProfileResponse", "Profile", "Name"])
            .unwrap()
            .text(),
        "synthetic-K17-K18"
    );
    let (independent, independent_ns, independent_one, _) = profile_replay();
    assert_eq!(
        independent
            .soap_post(
                "http://mock",
                &format!("{independent_ns}/GetProfile"),
                independent_one
            )
            .await
            .unwrap(),
        "<recorded-one/>"
    );
}

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
async fn generated_profile_token_skips_seeded_collisions() {
    for media2 in [false, true] {
        let transport = MockTransport::new();
        transport.device().modify(|state| {
            state.profiles.next_token_id = 42;
            state.profiles.profiles[0].token = "Profile_42".to_owned();
            state.profiles.profiles[1].token = "Profile_43".to_owned();
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
        assert_eq!(token, "Profile_44");
        let state = transport.device().read();
        assert_eq!(
            state
                .profiles
                .profiles
                .iter()
                .filter(|p| p.token == token)
                .count(),
            1,
            "generated profile identity must be unique"
        );
        assert_eq!(state.profiles.next_token_id, 45);
        assert_eq!(state.profiles.profiles[0].token, "Profile_42");
        assert_eq!(state.profiles.profiles[1].token, "Profile_43");
    }
}

#[tokio::test]
async fn rejected_delete_preserves_state_and_hook_but_success_notifies() {
    for media2 in [false, true] {
        let mut state = MockState::new();
        state.modify(|s| s.profiles.profiles[1].fixed = false);
        let before = state.read().clone();
        let fixed = before.profiles.profiles[0].token.clone();
        let removable = before.profiles.profiles[1].token.clone();
        let notifications = Arc::new(AtomicUsize::new(0));
        let capture = Arc::clone(&notifications);
        let removed = removable.clone();
        state.set_on_change(Arc::new(move |s| {
            assert!(!s.profiles.profiles.iter().any(|p| p.token == removed));
            capture.fetch_add(1, Ordering::SeqCst);
        }));
        let transport = MockTransport::with_state(state);
        let client = OnvifClient::new("http://mock").with_transport(Arc::new(transport.clone()));
        for (token, reason) in [
            ("K14-absent", "Profile not found: K14-absent".to_owned()),
            (
                fixed.as_str(),
                format!("Cannot delete fixed profile: {fixed}"),
            ),
        ] {
            let error = if media2 {
                client
                    .delete_profile_media2("http://mock", token)
                    .await
                    .unwrap_err()
            } else {
                client
                    .delete_profile("http://mock", token)
                    .await
                    .unwrap_err()
            };
            assert_fault(error, "s:Sender", &reason);
            assert_eq!(
                serde_json::to_value(&*transport.device().read()).unwrap(),
                serde_json::to_value(&before).unwrap()
            );
            assert_eq!(
                notifications.load(Ordering::SeqCst),
                0,
                "rejection must not notify"
            );
        }
        if media2 {
            client
                .delete_profile_media2("http://mock", &removable)
                .await
                .unwrap();
        } else {
            client
                .delete_profile("http://mock", &removable)
                .await
                .unwrap();
        }
        let mut expected = before;
        expected.profiles.profiles.remove(1);
        assert_eq!(
            serde_json::to_value(&*transport.device().read()).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        assert_eq!(
            notifications.load(Ordering::SeqCst),
            1,
            "successful deletion must notify exactly once"
        );
        // Preserve the public helper's existing unconditional notification
        // contract; only reviewed internal operations use the conditional path.
        let returned = transport.device().modify_returning(|_| "K14-public-helper");
        assert_eq!(returned, "K14-public-helper");
        assert_eq!(notifications.load(Ordering::SeqCst), 2);
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
