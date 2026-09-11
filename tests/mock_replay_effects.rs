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
    expected.video_source_configs[0].use_count = 1;
    expected.video_encoders[1].use_count = 0;
    expected.ptz_configs[0].use_count = 1;
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

async fn verify_creation(
    transport: Arc<dyn Transport>,
    base: &str,
    state: &MockState,
    reads: &[RecordedRead],
    media2: bool,
) {
    let before = state.read().clone();
    let namespace = if media2 { M2 } else { M1 };
    let client = OnvifClient::new(base).with_transport(transport.clone());
    let mut refusals = vec![
        (
            format!("<m:CreateProfile xmlns:m='{namespace}'/>"),
            "s:Sender",
            Some("ter:InvalidArgs"),
            "Invalid Args",
        ),
        (
            format!(
                "<m:CreateProfile xmlns:m='{namespace}'><m:Name>one</m:Name><m:Name>two</m:Name></m:CreateProfile>"
            ),
            "s:Sender",
            Some("ter:InvalidArgs"),
            "Invalid Args",
        ),
    ];
    if !media2 {
        // Profile assembly migrates the reviewed duplicate hierarchy; replay
        // must still preserve all recordings after this exact refusal.
        refusals.push((format!("<m:CreateProfile xmlns:m='{M1}'><m:Name>duplicate</m:Name><m:Token>Profile_1</m:Token></m:CreateProfile>"), "s:Sender", Some("ter:InvalidArgVal"), "Profile token already in use: Profile_1"));
        refusals.push((format!("<m:CreateProfile xmlns:m='{M1}'><m:Name>empty-policy</m:Name><m:Token/></m:CreateProfile>"), "s:Sender", Some("mock:RequestPolicy"), "The mock does not support empty profile tokens"));
    }
    for (request, code, subcode, reason) in refusals {
        let response = transport
            .soap_post(base, &format!("{namespace}/CreateProfile"), request)
            .await
            .unwrap();
        let body = parse_soap_body(&response).unwrap();
        assert_eq!(
            oxvif::soap::find_response(&body, "CreateProfileResponse").unwrap_err(),
            SoapError::Fault {
                code: code.into(),
                subcode: subcode.map(str::to_owned),
                reason: reason.into(),
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
                read.response,
                "refused creation must retain recorded reads: {}",
                read.action
            );
        }
    }
    let name = "created-effect-922";
    let token = if media2 {
        client.create_profile_media2(base, name).await.unwrap()
    } else {
        client.create_profile(base, name, None).await.unwrap().token
    };
    assert_eq!(token, "Profile_5");
    let mut expected = before;
    expected.profiles.next_token_id = 6;
    expected
        .profiles
        .profiles
        .push(oxvif::mock::state::ProfileEntry {
            token: token.clone(),
            name: name.into(),
            fixed: false,
            video_source_config_token: None,
            video_encoder_config_token: None,
            audio_source_config_token: None,
            audio_encoder_config_token: None,
            ptz_config_token: None,
        });
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
                "unrelated service recording must survive"
            );
            continue;
        }
        assert_ne!(
            response, read.response,
            "committed creation must retire this profile view"
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
            assert_eq!(profiles.len(), 5);
            assert_eq!(
                profiles
                    .iter()
                    .find(|p| p.attr("token") == Some(token.as_str()))
                    .unwrap()
                    .child("Name")
                    .unwrap()
                    .text(),
                name
            );
        }
    }
}

#[tokio::test]
async fn in_process_creation_retires_both_views_only_after_commit() {
    for media2 in [false, true] {
        let (state, store, reads) = fixture();
        let transport = MetamorphTransport::new(store.clone()).with_state(state);
        verify_creation(
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
async fn http_creation_retires_both_views_only_after_commit() {
    for media2 in [false, true] {
        let (state, store, reads) = fixture();
        let initial_state = state.read().clone();
        let server = oxvif::mock::MockServer::builder()
            .initial_state(initial_state)
            .replay(store)
            .start()
            .await
            .unwrap();
        verify_creation(
            Arc::new(oxvif::transport::HttpTransport::new()),
            server.device_url(),
            server.device(),
            &reads,
            media2,
        )
        .await;
    }
}

#[derive(Clone, Copy)]
struct BindingCase {
    namespace: &'static str,
    operation: &'static str,
    tag: &'static str,
    kinds: &'static [(&'static str, &'static str)],
    add: bool,
}

fn binding_cases() -> Vec<(BindingCase, bool)> {
    [
        BindingCase {
            namespace: M1,
            operation: "AddVideoSourceConfiguration",
            tag: "ADDVSC-5533",
            kinds: &[("VideoSource", "VSC_1")],
            add: true,
        },
        BindingCase {
            namespace: M1,
            operation: "RemoveVideoSourceConfiguration",
            tag: "RMVSC-5534",
            kinds: &[("VideoSource", "VSC_1")],
            add: false,
        },
        BindingCase {
            namespace: M1,
            operation: "AddVideoEncoderConfiguration",
            tag: "ADDVEC-5531",
            kinds: &[("VideoEncoder", "VEC_1")],
            add: true,
        },
        BindingCase {
            namespace: M1,
            operation: "RemoveVideoEncoderConfiguration",
            tag: "RMVEC-5532",
            kinds: &[("VideoEncoder", "VEC_1")],
            add: false,
        },
        BindingCase {
            namespace: M2,
            operation: "AddConfiguration",
            tag: "ADDCFG2-5543",
            kinds: &[("VideoSource", "VSC_1"), ("VideoEncoder", "VEC_1")],
            add: true,
        },
        BindingCase {
            namespace: M2,
            operation: "RemoveConfiguration",
            tag: "RMCFG2-5544",
            kinds: &[("VideoSource", "VSC_1"), ("VideoEncoder", "VEC_1")],
            add: false,
        },
    ]
    .into_iter()
    .flat_map(|case| [(case, false), (case, true)])
    .filter(|(case, already_empty)| !case.add || !already_empty)
    .collect()
}

fn set_test_bindings(
    profile: &mut oxvif::mock::state::ProfileEntry,
    case: &BindingCase,
    attached: bool,
) {
    for (kind, token) in case.kinds {
        let slot = match *kind {
            "VideoSource" => &mut profile.video_source_config_token,
            "VideoEncoder" => &mut profile.video_encoder_config_token,
            _ => panic!("unknown test kind"),
        };
        *slot = attached.then(|| (*token).to_owned());
    }
}

fn binding_fixture(
    case: &BindingCase,
    already_empty: bool,
) -> (MockState, FixtureStore, Vec<RecordedRead>) {
    let (state, mut store, mut reads) = fixture();
    state.modify(|device| {
        set_test_bindings(
            &mut device.profiles.profiles[0],
            case,
            !case.add && !already_empty,
        )
    });
    let family = case
        .operation
        .strip_prefix(if case.add { "Add" } else { "Remove" })
        .unwrap();
    let unrelated = RecordedRead {
        action: format!("urn:unrelated/Get{family}"),
        request: "<family-query/>".into(),
        response: "<unrelated-binding-family-923/>",
    };
    store.record(&unrelated.action, &unrelated.request, unrelated.response);
    reads.push(unrelated);
    (state, store, reads)
}

fn binding_request(case: &BindingCase, profile: &str, bad_last: bool) -> String {
    let mut fields = format!("<m:ProfileToken>{profile}</m:ProfileToken>");
    for (index, (kind, token)) in case.kinds.iter().enumerate() {
        let token = if bad_last && index + 1 == case.kinds.len() {
            "absent-config-923"
        } else {
            token
        };
        if case.namespace == M2 {
            fields.push_str(&format!("<m:Configuration><m:Type>{kind}</m:Type><m:Token>{token}</m:Token></m:Configuration>"));
        } else if case.add {
            fields.push_str(&format!(
                "<m:ConfigurationToken>{token}</m:ConfigurationToken>"
            ));
        }
    }
    format!(
        "<m:{op} xmlns:m='{ns}'>{fields}</m:{op}>",
        op = case.operation,
        ns = case.namespace
    )
}

async fn verify_binding(
    transport: Arc<dyn Transport>,
    base: &str,
    state: &MockState,
    reads: &[RecordedRead],
    case: &BindingCase,
) {
    let before = state.read().clone();
    let action = format!("{}/{}", case.namespace, case.operation);
    let mut refusals = vec![(
        "absent-profile-923",
        false,
        "ter:NoProfile",
        format!("NoSuchProfile-{}: absent-profile-923", case.tag),
    )];
    if case.add {
        refusals.push((
            "Profile_1",
            true,
            "ter:NoConfig",
            format!("NoSuchConfig-{}: absent-config-923", case.tag),
        ));
    }
    for (profile, bad_last, leaf, reason) in refusals {
        let xml = transport
            .soap_post(base, &action, binding_request(case, profile, bad_last))
            .await
            .unwrap();
        let body = parse_soap_body(&xml).unwrap();
        assert_eq!(
            oxvif::soap::find_response(&body, &format!("{}Response", case.operation)).unwrap_err(),
            SoapError::Fault {
                code: "s:Sender".into(),
                subcode: Some("ter:InvalidArgVal".into()),
                reason,
                detail: None,
            }
        );
        assert_eq!(
            body.path(&["Fault", "Code", "Subcode", "Subcode", "Value"])
                .unwrap()
                .text(),
            leaf
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
                read.response,
                "refused {} must preserve {}",
                case.operation,
                read.action
            );
        }
    }
    let xml = transport
        .soap_post(base, &action, binding_request(case, "Profile_1", false))
        .await
        .unwrap();
    let body = parse_soap_body(&xml).unwrap();
    let response =
        oxvif::soap::find_response(&body, &format!("{}Response", case.operation)).unwrap();
    assert!(response.children.is_empty());
    let mut expected = before.clone();
    set_test_bindings(&mut expected.profiles.profiles[0], case, case.add);
    for (kind, _) in case.kinds {
        let field = match *kind {
            "VideoSource" => "video_source_config_token",
            "VideoEncoder" => "video_encoder_config_token",
            _ => panic!("unreviewed binding fixture kind"),
        };
        let old = serde_json::to_value(&before.profiles.profiles[0]).unwrap();
        let new = serde_json::to_value(&expected.profiles.profiles[0]).unwrap();
        for token in [old[field].as_str(), new[field].as_str()]
            .into_iter()
            .flatten()
        {
            let profiles = serde_json::to_value(&expected.profiles.profiles).unwrap();
            let count = profiles
                .as_array()
                .unwrap()
                .iter()
                .filter(|p| p[field].as_str() == Some(token))
                .count() as u32;
            match *kind {
                "VideoSource" => {
                    expected
                        .video_source_configs
                        .iter_mut()
                        .find(|c| c.token == token)
                        .unwrap()
                        .use_count = count
                }
                "VideoEncoder" => {
                    expected
                        .video_encoders
                        .iter_mut()
                        .find(|c| c.token == token)
                        .unwrap()
                        .use_count = count
                }
                _ => unreachable!(),
            }
        }
    }
    assert_eq!(
        serde_json::to_value(&*state.read()).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    for (index, read) in reads.iter().enumerate() {
        let xml = transport
            .soap_post(base, &read.action, read.request.clone())
            .await
            .unwrap();
        if index >= 3 {
            assert_eq!(
                xml, read.response,
                "committed binding must not retire another service"
            );
            continue;
        }
        assert_ne!(
            xml, read.response,
            "committed {} must retire {}",
            case.operation, read.action
        );
        let body = parse_soap_body(&xml).unwrap();
        let profiles: Vec<_> = if index == 0 {
            body.child("GetProfileResponse")
                .unwrap()
                .children_named("Profile")
                .collect()
        } else {
            body.child("GetProfilesResponse")
                .unwrap()
                .children_named("Profiles")
                .collect()
        };
        assert_eq!(profiles.len(), if index == 0 { 1 } else { 4 });
        let profile = profiles
            .iter()
            .find(|p| p.attr("token") == Some("Profile_1"))
            .unwrap();
        assert_eq!(profile.child("Name").unwrap().text(), "survivor-effect-831");
        for (kind, token) in case.kinds {
            let config = if index == 2 {
                profile
                    .child("Configurations")
                    .and_then(|configs| configs.child(kind))
            } else {
                profile.child(&format!("{kind}Configuration"))
            };
            assert_eq!(
                config.map(|config| config.attr("token").unwrap()),
                case.add.then_some(*token)
            );
        }
    }
}

#[tokio::test]
async fn in_process_binding_effects_refresh_profiles_without_cross_service_retirement() {
    for (case, already_empty) in binding_cases() {
        let (state, store, reads) = binding_fixture(&case, already_empty);
        let transport = MetamorphTransport::new(store.clone()).with_state(state);
        verify_binding(
            Arc::new(transport.clone()),
            "http://mock",
            transport.device(),
            &reads,
            &case,
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
async fn http_binding_effects_refresh_profiles_without_cross_service_retirement() {
    for (case, already_empty) in binding_cases() {
        let (state, store, reads) = binding_fixture(&case, already_empty);
        let initial_state = state.read().clone();
        let server = oxvif::mock::MockServer::builder()
            .initial_state(initial_state)
            .replay(store)
            .start()
            .await
            .unwrap();
        verify_binding(
            Arc::new(oxvif::transport::HttpTransport::new()),
            server.device_url(),
            server.device(),
            &reads,
            &case,
        )
        .await;
    }
}
