//! Project-authored replay identity controls. Raw recorded markers intentionally
//! are not SOAP fixtures and do not represent schema or HTTP-binding acceptance.
#![cfg(feature = "metamorph")]

use std::sync::Arc;

use oxvif::{
    OnvifClient, OnvifError,
    metamorph::{FixtureStore, MetamorphTransport},
    mock::MockState,
    soap::{SoapError, parse_soap_body},
    transport::Transport,
};

const M1: &str = "http://www.onvif.org/ver10/media/wsdl";
const M2: &str = "http://www.onvif.org/ver20/media/wsdl";

struct RecordedRead {
    action: String,
    request: String,
    response: &'static str,
}

fn fixture() -> (MockState, FixtureStore, Vec<RecordedRead>) {
    let state = MockState::new();
    state.modify(|s| {
        s.profiles.profiles[0].name = "survivor-effect-831".into();
        s.profiles.profiles[1].fixed = false;
    });
    let fixed = state.read().profiles.profiles[0].token.clone();
    let reads = vec![
        RecordedRead {
            action: format!("{M1}/GetProfile"),
            request: format!(
                "<m:GetProfile xmlns:m='{M1}'><m:ProfileToken>{fixed}</m:ProfileToken></m:GetProfile>"
            ),
            response: "<recorded-one-831/>",
        },
        RecordedRead {
            action: format!("{M1}/GetProfiles"),
            request: format!("<m:GetProfiles xmlns:m='{M1}'/>"),
            response: "<recorded-all-831/>",
        },
        RecordedRead {
            action: format!("{M2}/GetProfiles"),
            request: format!("<m:GetProfiles xmlns:m='{M2}'><m:Type>All</m:Type></m:GetProfiles>"),
            response: "<recorded-second-view-831/>",
        },
        RecordedRead {
            action: "urn:unrelated/GetProfiles".into(),
            request: "<opaque-query/>".into(),
            response: "<unrelated-recording-831/>",
        },
    ];
    let mut store = FixtureStore::new("effect-controls");
    for read in &reads {
        store.record(&read.action, &read.request, read.response);
    }
    (state, store, reads)
}

async fn verify_deletion(
    transport: Arc<dyn Transport>,
    base: &str,
    state: &MockState,
    reads: &[RecordedRead],
    media2: bool,
) {
    let before = state.read().clone();
    let fixed = before.profiles.profiles[0].token.clone();
    let removable = before.profiles.profiles[1].token.clone();
    let client = OnvifClient::new(base).with_transport(transport.clone());
    for (token, subcode, reason) in [
        (
            "absent-effect-831",
            "ter:InvalidArgVal",
            "Profile not found: absent-effect-831".to_owned(),
        ),
        (
            fixed.as_str(),
            "ter:Action",
            format!("Cannot delete fixed profile: {fixed}"),
        ),
    ] {
        let error = if media2 {
            client.delete_profile_media2(base, token).await.unwrap_err()
        } else {
            client.delete_profile(base, token).await.unwrap_err()
        };
        let OnvifError::Soap(fault) = error else {
            panic!("expected explicit SOAP refusal")
        };
        assert_eq!(
            fault,
            SoapError::Fault {
                code: "s:Sender".into(),
                subcode: Some(subcode.into()),
                reason,
                detail: None,
            }
        );
        assert_eq!(
            serde_json::to_value(&*state.read()).unwrap(),
            serde_json::to_value(&before).unwrap()
        );
        for read in reads {
            assert_eq!(
                transport
                    .soap_post(base, &read.action, read.request.clone())
                    .await
                    .unwrap(),
                read.response
            );
        }
    }
    if media2 {
        client
            .delete_profile_media2(base, &removable)
            .await
            .unwrap();
    } else {
        client.delete_profile(base, &removable).await.unwrap();
    }
    let mut expected = before;
    expected.profiles.profiles.remove(1);
    assert_eq!(
        serde_json::to_value(&*state.read()).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    for (index, read) in reads.iter().enumerate() {
        let response = transport
            .soap_post(base, &read.action, read.request.clone())
            .await
            .unwrap();
        if index == 3 {
            assert_eq!(
                response, read.response,
                "same operation tail in another service must survive"
            );
        } else {
            assert_ne!(
                response, read.response,
                "committed deletion must retire this profile view"
            );
            let body = parse_soap_body(&response).unwrap();
            if index == 0 {
                assert_eq!(
                    body.path(&["GetProfileResponse", "Profile", "Name"])
                        .unwrap()
                        .text(),
                    "survivor-effect-831"
                );
            } else {
                let profiles: Vec<_> = body
                    .child("GetProfilesResponse")
                    .unwrap()
                    .children_named("Profiles")
                    .collect();
                assert_eq!(profiles.len(), expected.profiles.profiles.len());
                assert!(
                    !profiles
                        .iter()
                        .any(|p| p.attr("token") == Some(removable.as_str()))
                );
            }
        }
    }
}

#[tokio::test]
async fn in_process_deletion_retires_both_views_only_after_commit() {
    for media2 in [false, true] {
        let (state, store, reads) = fixture();
        let transport = MetamorphTransport::new(store.clone()).with_state(state);
        verify_deletion(
            Arc::new(transport.clone()),
            "http://mock",
            transport.device(),
            &reads,
            media2,
        )
        .await;
        let independent = MetamorphTransport::new(store);
        for read in reads {
            assert_eq!(
                independent
                    .soap_post("http://mock", &read.action, read.request)
                    .await
                    .unwrap(),
                read.response
            );
        }
    }
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn http_deletion_retires_both_views_only_after_commit() {
    for media2 in [false, true] {
        let (state, store, reads) = fixture();
        let initial_state = state.read().clone();
        let server = oxvif::mock::MockServer::builder()
            .initial_state(initial_state)
            .replay(store)
            .start()
            .await
            .unwrap();
        verify_deletion(
            Arc::new(oxvif::transport::HttpTransport::new()),
            server.device_url(),
            server.device(),
            &reads,
            media2,
        )
        .await;
    }
}
