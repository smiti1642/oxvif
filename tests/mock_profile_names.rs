//! Project-authored literal-name probes, not a schema-derived fixture corpus.
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

const MEDIA1: &str = "http://www.onvif.org/ver10/media/wsdl";
const MEDIA2: &str = "http://www.onvif.org/ver20/media/wsdl";

async fn request(
    transport: &dyn Transport,
    url: &str,
    namespace: &str,
    operation: &str,
    fields: &str,
) -> String {
    transport.soap_post(url, &format!("{namespace}/{operation}"), format!(
        "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:m='{namespace}'><s:Header><m:Name>header-decoy</m:Name></s:Header><s:Body><m:{operation}>{fields}</m:{operation}></s:Body></s:Envelope>"
    )).await.unwrap()
}

async fn assert_reads(
    transport: &dyn Transport,
    url: &str,
    token: &str,
    literal: &str,
    escaped: &str,
) {
    for (namespace, operation, fields, response, profile_tag, name_prefix) in [
        (
            MEDIA1,
            "GetProfile",
            format!("<m:ProfileToken>{token}</m:ProfileToken>"),
            "GetProfileResponse",
            "Profile",
            "tt",
        ),
        (
            MEDIA1,
            "GetProfiles",
            String::new(),
            "GetProfilesResponse",
            "Profiles",
            "tt",
        ),
        (
            MEDIA2,
            "GetProfiles",
            format!("<m:Token>{token}</m:Token>"),
            "GetProfilesResponse",
            "Profiles",
            "tr2",
        ),
    ] {
        let xml = request(transport, url, namespace, operation, &fields).await;
        // Raw content pins whitespace as well: the public client DOM trims it.
        assert!(
            xml.contains(&format!(
                "<{name_prefix}:Name>{escaped}</{name_prefix}:Name>"
            )),
            "{operation}: {xml}"
        );
        let body = parse_soap_body(&xml).unwrap();
        let profile = find_response(&body, response)
            .unwrap()
            .children_named(profile_tag)
            .find(|profile| profile.attr("token") == Some(token))
            .unwrap();
        let name = profile.child("Name").unwrap();
        assert!(name.children.is_empty());
        assert_eq!(name.text(), literal.trim());
    }
}

async fn exercise(
    transport: Arc<dyn Transport>,
    url: &str,
    state: &MockState,
    hooks: &AtomicUsize,
) {
    let client = OnvifClient::new(url).with_transport(transport.clone());
    // Seed state directly as well as exercising CreateProfile: persisted names
    // must remain literal text on every read path and on both transports.
    let literal = "before<AuditMarker>injected</AuditMarker>after";
    let token = state.modify_returning(|device| {
        let profile = &mut device.profiles.profiles[0];
        profile.name = literal.to_owned();
        profile.token.clone()
    });
    let notifications = hooks.load(Ordering::SeqCst);
    assert_reads(
        transport.as_ref(),
        url,
        &token,
        literal,
        "before&lt;AuditMarker&gt;injected&lt;/AuditMarker&gt;after",
    )
    .await;
    assert_eq!(hooks.load(Ordering::SeqCst), notifications);
    for namespace in [MEDIA1, MEDIA2] {
        for (field, literal, escaped) in [
            (
                "<m:Name> 入口 &amp; &lt;tag&gt; &quot;&apos; &amp;amp; </m:Name>",
                " 入口 & <tag> \"' &amp; ",
                " 入口 &amp; &lt;tag&gt; &quot;&apos; &amp;amp; ",
            ),
            (
                "<m:Name><![CDATA[ before<Probe/>after &amp; ]]></m:Name>",
                " before<Probe/>after &amp; ",
                " before&lt;Probe/&gt;after &amp;amp; ",
            ),
            (
                "<m:Name>&#x20;numeric&#32;&#x26;&#10;</m:Name>",
                " numeric &\n",
                " numeric &amp;&#10;",
            ),
            ("<m:Name/>", "", ""),
            (
                "<m:Name>left&#13;&#10;&#9;right</m:Name>",
                "left\r\n\tright",
                "left&#13;&#10;&#9;right",
            ),
        ] {
            let count = state.read().profiles.profiles.len();
            let notifications = hooks.load(Ordering::SeqCst);
            let xml = request(transport.as_ref(), url, namespace, "CreateProfile", field).await;
            let body = parse_soap_body(&xml).unwrap();
            let response = find_response(&body, "CreateProfileResponse").unwrap();
            let token = if namespace == MEDIA1 {
                response.child("Profile").unwrap().attr("token").unwrap()
            } else {
                response.child("Token").unwrap().text()
            }
            .to_owned();
            {
                let device = state.read();
                assert_eq!(device.profiles.profiles.len(), count + 1);
                assert_eq!(
                    device
                        .profiles
                        .profiles
                        .iter()
                        .find(|p| p.token == token)
                        .unwrap()
                        .name,
                    literal
                );
            }
            assert_eq!(hooks.load(Ordering::SeqCst), notifications + 1);
            assert_reads(transport.as_ref(), url, &token, literal, escaped).await;
            assert_eq!(hooks.load(Ordering::SeqCst), notifications + 1);
            client.delete_profile(url, &token).await.unwrap();
        }

        // Exercise the public encoder too, with a literal entity spelling that
        // distinguishes exactly-once encoding from accidental double decoding.
        let literal = "client &amp; <marker> \"' 中文\r\n\tend";
        let token = if namespace == MEDIA1 {
            let profile = client.create_profile(url, literal, None).await.unwrap();
            assert_eq!(profile.name, literal);
            profile.token
        } else {
            client.create_profile_media2(url, literal).await.unwrap()
        };
        assert_reads(
            transport.as_ref(),
            url,
            &token,
            literal,
            "client &amp;amp; &lt;marker&gt; &quot;&apos; 中文&#13;&#10;&#9;end",
        )
        .await;
        assert_eq!(
            state
                .read()
                .profiles
                .profiles
                .iter()
                .find(|p| p.token == token)
                .unwrap()
                .name,
            literal
        );
        client.delete_profile(url, &token).await.unwrap();

        let before = serde_json::to_value(&*state.read()).unwrap();
        let notifications = hooks.load(Ordering::SeqCst);
        for fields in [
            "",
            "<m:Name>first</m:Name><m:Name>second</m:Name>",
            "<m:Name><m:Nested>decoy</m:Nested></m:Name>",
            "<m:Extension><m:Name>nested-decoy</m:Name></m:Extension>",
            "<m:Name xmlns:m='urn:wrong'>wrong-namespace</m:Name>",
        ] {
            let xml = request(transport.as_ref(), url, namespace, "CreateProfile", fields).await;
            let body = parse_soap_body(&xml).unwrap();
            assert_eq!(
                find_response(&body, "CreateProfileResponse").unwrap_err(),
                SoapError::Fault {
                    code: "s:Sender".into(),
                    subcode: Some("ter:InvalidArgs".into()),
                    reason: "Invalid Args".into(),
                    detail: None,
                }
            );
            assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
            assert_eq!(hooks.load(Ordering::SeqCst), notifications);
        }
    }
}

#[tokio::test]
async fn in_process_profile_names_are_decoded_and_escaped_once() {
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
async fn http_profile_names_are_decoded_and_escaped_once() {
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
