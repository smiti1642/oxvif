//! Project-authored selector probes; not a schema-derived fixture corpus.
#![cfg(feature = "mock")]

use oxvif::{
    mock::{MockState, MockTransport},
    soap::{SoapError, XmlNode, find_response, parse_soap_body},
    transport::Transport,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const MEDIA2: &str = "http://www.onvif.org/ver20/media/wsdl";

async fn query(transport: &dyn Transport, url: &str, fields: &str) -> Result<XmlNode, SoapError> {
    let body = format!(
        "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:m='{MEDIA2}'><s:Header><m:Token>decoy-733</m:Token><m:Type>All</m:Type></s:Header><s:Body><m:GetProfiles>{fields}</m:GetProfiles></s:Body></s:Envelope>"
    );
    let xml = transport
        .soap_post(url, &format!("{MEDIA2}/GetProfiles"), body)
        .await
        .unwrap();
    let body = parse_soap_body(&xml).unwrap();
    let result = find_response(&body, "GetProfilesResponse").cloned();
    if matches!(&result, Err(SoapError::Fault { subcode: Some(subcode), .. }) if subcode == "ter:InvalidArgVal")
    {
        assert_eq!(
            body.path(&["Fault", "Code", "Subcode", "Subcode", "Value"])
                .unwrap()
                .text(),
            "ter:NoProfile"
        );
    }
    result
}

fn projection(response: &XmlNode) -> Vec<(String, Vec<(String, String)>)> {
    response
        .children_named("Profiles")
        .map(|profile| {
            let bindings = profile
                .child("Configurations")
                .map(|configs| {
                    configs
                        .children
                        .iter()
                        .map(|config| {
                            (
                                config.local_name.clone(),
                                config.attr("token").unwrap().to_owned(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            (profile.attr("token").unwrap().to_owned(), bindings)
        })
        .collect()
}

async fn exercise(transport: &dyn Transport, url: &str, state: &MockState, hooks: &AtomicUsize) {
    let before = serde_json::to_value(&*state.read()).unwrap();
    let bare = projection(&query(transport, url, "").await.unwrap());
    assert_eq!(
        bare,
        ["Profile_1", "Profile_2", "Profile_3", "Profile_4"]
            .map(|token| (token.to_owned(), vec![]))
    );
    let all = projection(&query(transport, url, "<m:Type>All</m:Type>").await.unwrap());
    assert_eq!(all.len(), 4);
    assert_eq!(
        all[0].1,
        [
            ("VideoSource", "VSC_1"),
            ("AudioSource", "ASC_1"),
            ("VideoEncoder", "VEC_1"),
            ("AudioEncoder", "AEC_1"),
            ("PTZ", "PTZConfig_1")
        ]
        .map(|(kind, token)| (kind.to_owned(), token.to_owned()))
    );
    let selected = projection(
        &query(
            transport,
            url,
            "<m:Token>Profile_3</m:Token><m:Type>VideoSource</m:Type><m:Type>PTZ</m:Type>",
        )
        .await
        .unwrap(),
    );
    assert_eq!(
        selected,
        vec![(
            "Profile_3".into(),
            vec![
                ("VideoSource".into(), "VSC_2".into()),
                ("PTZ".into(), "PTZConfig_2".into())
            ]
        )]
    );
    for types in [
        "<m:Type>VideoEncoder</m:Type><m:Type>VideoEncoder</m:Type>",
        "<m:Type>All</m:Type><m:Type>VideoEncoder</m:Type>",
    ] {
        let selected = projection(&query(transport, url, types).await.unwrap());
        assert_eq!(
            selected,
            (1..=4)
                .map(|n| (
                    format!("Profile_{n}"),
                    vec![("VideoEncoder".into(), format!("VEC_{n}"))]
                ))
                .collect::<Vec<_>>()
        );
    }
    let selected = projection(
        &query(transport, url, "<m:Token>Profile_&#51;</m:Token>")
            .await
            .unwrap(),
    );
    assert_eq!(selected, vec![("Profile_3".into(), vec![])]);
    for token in ["missing-733", "", " Profile_3 "] {
        assert_eq!(
            query(transport, url, &format!("<m:Token>{token}</m:Token>"))
                .await
                .unwrap_err(),
            SoapError::Fault {
                code: "s:Sender".into(),
                subcode: Some("ter:InvalidArgVal".into()),
                reason: format!("Profile not found: {token}").trim().to_owned(),
                detail: None,
            }
        );
    }
    for fields in [
        "<m:Token>Profile_1</m:Token><m:Token>Profile_3</m:Token>",
        "<m:Type>VideoSource</m:Type><m:Token>Profile_3</m:Token>",
        "<m:Token><m:Nested>Profile_3</m:Nested></m:Token>",
        "<m:Type><m:Nested>VideoSource</m:Nested></m:Type>",
        "not-container-whitespace",
    ] {
        assert_eq!(
            query(transport, url, fields).await.unwrap_err(),
            SoapError::Fault {
                code: "s:Sender".into(),
                subcode: Some("ter:InvalidArgs".into()),
                reason: "Invalid Args".into(),
                detail: None,
            },
            "{fields}"
        );
    }
    for fields in [
        "<Token xmlns='urn:wrong'>Profile_3</Token>",
        "<m:Wrapper><m:Token>Profile_3</m:Token></m:Wrapper>",
    ] {
        assert_eq!(
            query(transport, url, fields).await.unwrap_err(),
            SoapError::Fault {
                code: "s:Sender".into(),
                subcode: Some("ter:TagMismatch".into()),
                reason: "Tag Mismatch".into(),
                detail: None,
            }
        );
    }
    // A list projection must not erase bindings in shared state.
    assert_eq!(
        projection(&query(transport, url, "<m:Type>All</m:Type>").await.unwrap()),
        all
    );
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
    assert_eq!(hooks.load(Ordering::SeqCst), 0);

    // Seed an empty device only after validating the earlier reads, then ensure
    // both unfiltered and selected queries preserve that new fixture too.
    state.modify(|device| device.profiles.profiles.clear());
    hooks.store(0, Ordering::SeqCst);
    let empty = serde_json::to_value(&*state.read()).unwrap();
    for fields in ["", "<m:Type>All</m:Type>", "<m:Type>AudioSource</m:Type>"] {
        assert_eq!(
            projection(&query(transport, url, fields).await.unwrap()),
            vec![]
        );
    }
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), empty);
    assert_eq!(hooks.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn in_process_media2_profile_selection_is_scoped_and_read_only() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let observed = hooks.clone();
    let mut state = MockState::new();
    state.set_on_change(Arc::new(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
    }));
    let transport = MockTransport::with_state(state);
    exercise(&transport, "http://mock", transport.device(), &hooks).await;
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn http_media2_profile_selection_is_scoped_and_read_only() {
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
        &oxvif::transport::HttpTransport::new(),
        server.device_url(),
        server.device(),
        &hooks,
    )
    .await;
}
