//! Synthetic credentials only; no environment secrets or external devices.
#![cfg(feature = "mock")]

use base64::{Engine as _, engine::general_purpose::STANDARD};
use oxvif::{
    OnvifClient,
    mock::{MockState, MockTransport, state::MockUser},
    soap::{
        SoapEnvelope, SoapError, WsSecurityToken, find_response, parse_soap_body,
        security::compute_digest,
    },
    transport::{Transport, TransportError},
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation";
const USER: &str = " authuser-583 &amp;尾 ";
const PASSWORD: &str = "mock-only-password-583";
const CREATED: &str = "2001-02-03T04:05:06Z";

fn envelope(username: &str, password: &str) -> String {
    let nonce = b"mock-only-nonce-583";
    let token = WsSecurityToken::from_parts(
        username,
        STANDARD.encode(compute_digest(nonce, CREATED, password)),
        STANDARD.encode(nonce),
        CREATED,
    );
    SoapEnvelope::new("<tds:GetDeviceInformation/>".into())
        .with_security(token)
        .build()
}

fn seed(state: &MockState) {
    state.modify(|device| {
        device.users.push(MockUser {
            username: USER.into(),
            password: PASSWORD.into(),
            level: "User".into(),
        });
        device.users.push(MockUser {
            username: USER.trim().into(),
            password: "different-password-583".into(),
            level: "User".into(),
        });
    });
}

async fn read(transport: &dyn Transport, url: &str, body: String) -> String {
    transport.soap_post(url, ACTION, body).await.unwrap()
}

fn assert_refusal(xml: &str, reason: &str) {
    let root = parse_soap_body(xml).unwrap();
    assert_eq!(
        find_response(&root, "GetDeviceInformationResponse").expect_err(reason),
        SoapError::Fault {
            code: "s:Sender".into(),
            subcode: Some("wsse:FailedAuthentication".into()),
            reason: reason.into(),
            detail: None,
        }
    );
    for marker in [
        "authuser-583",
        PASSWORD,
        "mock-only-nonce-583",
        &STANDARD.encode(b"mock-only-nonce-583"),
        &STANDARD.encode(compute_digest(b"mock-only-nonce-583", CREATED, PASSWORD)),
    ] {
        assert!(
            !xml.contains(marker),
            "authentication error reflected a credential marker"
        );
    }
}

async fn exercise(
    transport: Arc<dyn Transport>,
    url: &str,
    state: &MockState,
    hooks: &AtomicUsize,
    http: bool,
) {
    seed(state);
    let before = serde_json::to_value(&*state.read()).unwrap();
    let notifications = hooks.load(Ordering::SeqCst);
    let expected = state.read().info.manufacturer.clone();
    for (username, password) in [("admin", "admin"), (USER, PASSWORD)] {
        let client = OnvifClient::new(url)
            .with_credentials(username, password)
            .with_transport(transport.clone());
        assert_eq!(
            client.get_device_info().await.unwrap().manufacturer,
            expected
        );
    }
    let good = envelope(USER, PASSWORD);
    let nonce_encoding = " EncodingType=\"http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-soap-message-security-1.0#Base64Binary\"";
    assert!(good.contains(nonce_encoding));
    for body in [
        good.clone(),
        good.replace(nonce_encoding, ""),
        good.replace(CREATED, &format!(" {CREATED} ")).replace(&STANDARD.encode(compute_digest(b"mock-only-nonce-583", CREATED, PASSWORD)), &STANDARD.encode(compute_digest(b"mock-only-nonce-583", &format!(" {CREATED} "), PASSWORD))),
        good.replace(&STANDARD.encode(compute_digest(b"mock-only-nonce-583", CREATED, PASSWORD)), &format!(" \n{}\t", STANDARD.encode(compute_digest(b"mock-only-nonce-583", CREATED, PASSWORD)))),
        good.replace("wsse:", "alias:")
            .replace("xmlns:wsse=", "xmlns:alias="),
        good.replace(
            " authuser-583 &amp;amp;尾 ",
            "<![CDATA[ authuser-583 &amp;尾 ]]>",
        ),
        good.replace(&STANDARD.encode(b"mock-only-nonce-583"), &format!(" \t{}\r\n", STANDARD.encode(b"mock-only-nonce-583"))),
        good.replace("<wsse:Username>", "<x:Username xmlns:x='urn:decoy'>wrong</x:Username><wsse:Username>"),
        good.replace("<wsse:Security>", "<wsse:Security s:role='http://www.w3.org/2003/05/soap-envelope/role/ultimateReceiver'>"),
    ] {
        let xml = read(transport.as_ref(), url, body).await;
        let root = parse_soap_body(&xml).unwrap();
        assert_eq!(
            find_response(&root, "GetDeviceInformationResponse")
                .unwrap()
                .child("Manufacturer")
                .unwrap()
                .text(),
            expected
        );
    }
    let security_start = good.find("<wsse:Security>").unwrap();
    let security_end = good.find("</wsse:Security>").unwrap() + "</wsse:Security>".len();
    let security = &good[security_start..security_end];
    let mut cases = vec![
        (
            good.replace(&STANDARD.encode(b"mock-only-nonce-583"), " \t")
                .replace(
                    &STANDARD.encode(compute_digest(b"mock-only-nonce-583", CREATED, PASSWORD)),
                    &STANDARD.encode(compute_digest(b"", CREATED, PASSWORD)),
                ),
            "Invalid nonce base64",
        ),
        (
            good.replace(security, "")
                .replace("<tds:GetDeviceInformation/>", security),
            "Missing Username",
        ),
        (
            good.replace(
                "<wsse:Security>",
                "<wsse:Security xmlns:wsse='urn:foreign'>",
            ),
            "Missing Username",
        ),
        (
            good.replace(security, &format!("{security}{security}")),
            "Invalid WS-Security header",
        ),
        (
            good.replace("<wsse:Username>", "<wsse:Username><wsse:Nested>")
                .replace("</wsse:Username>", "</wsse:Nested></wsse:Username>"),
            "Invalid WS-Security header",
        ),
        (
            good.replace(
                "</wsse:Username>",
                "</wsse:Username><wsse:Username>duplicate-583</wsse:Username>",
            ),
            "Invalid WS-Security header",
        ),
        (
            good.replace("#PasswordDigest", "#PasswordText"),
            "Unsupported WS-Security password type",
        ),
        (
            good.replace("#Base64Binary", "#Unsupported"),
            "Unsupported nonce encoding",
        ),
        (
            good.replace(
                "<wsse:Security>",
                "<wsse:Security s:role='urn:other-recipient'>",
            ),
            "Unsupported WS-Security role",
        ),
        (
            good.replace(&STANDARD.encode(b"mock-only-nonce-583"), "not-base64-583!"),
            "Invalid nonce base64",
        ),
        (envelope("unknown-authuser-583", PASSWORD), "Unknown user"),
        (
            envelope(USER, "incorrect-password-583"),
            "Password digest mismatch",
        ),
        ("<broken".into(), "Invalid WS-Security header"),
    ];
    for (name, ns, reason) in [
        ("Username", "wsse", "Missing Username"),
        ("Password", "wsse", "Missing Password digest"),
        ("Nonce", "wsse", "Missing Nonce"),
        ("Created", "wsu", "Missing Created timestamp"),
    ] {
        let delimiter = if matches!(name, "Password" | "Nonce") {
            " "
        } else {
            ">"
        };
        let start = good.find(&format!("<{ns}:{name}{delimiter}")).unwrap();
        let end = good.find(&format!("</{ns}:{name}>")).unwrap() + format!("</{ns}:{name}>").len();
        cases.push((format!("{}{}", &good[..start], &good[end..]), reason));
        cases.push((
            format!("{}<{ns}:{name}/>{}", &good[..start], &good[end..]),
            reason,
        ));
    }
    cases.push((
        good.replace(
            "<wsse:UsernameToken>",
            "<wsse:UsernameToken/><wsse:UsernameToken>",
        ),
        "Invalid WS-Security header",
    ));
    cases.push((
        good.replace("</s:Header>", "</s:Header><s:Header/>"),
        "Invalid WS-Security header",
    ));
    cases.push((
        format!("{}{}", "<nested>".repeat(65), "</nested>".repeat(65)),
        "Invalid WS-Security header",
    ));
    let type_start = good.find(" Type=").unwrap();
    let type_end = type_start
        + " Type=\"".len()
        + good[type_start + " Type=\"".len()..].find('"').unwrap()
        + 1;
    cases.push((
        format!("{}{}", &good[..type_start], &good[type_end..]),
        "Unsupported WS-Security password type",
    ));
    for (body, reason) in cases {
        assert_ne!(
            body, good,
            "negative fixture must change the request: {reason}"
        );
        assert_refusal(&read(transport.as_ref(), url, body).await, reason);
        assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
        assert_eq!(hooks.load(Ordering::SeqCst), notifications);
    }
    let oversized = transport
        .soap_post(
            url,
            ACTION,
            format!("{}{good}", " ".repeat(2 * 1024 * 1024)),
        )
        .await;
    match oversized {
        Ok(xml) if !http => assert_refusal(&xml, "Invalid WS-Security header"),
        Err(TransportError::HttpStatus { status, body }) if http => {
            assert_eq!(status, 413);
            assert_eq!(
                body,
                "Failed to buffer the request body: length limit exceeded"
            );
        }
        _ => panic!("unexpected oversized-auth result for http={http}"),
    }
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
    assert_eq!(hooks.load(Ordering::SeqCst), notifications);
    // The credential table is live; a prior success cannot cache the old password.
    state.modify(|device| {
        device
            .users
            .iter_mut()
            .find(|user| user.username == USER)
            .unwrap()
            .password = "rotated-password-583".into()
    });
    assert_refusal(
        &read(transport.as_ref(), url, good).await,
        "Password digest mismatch",
    );
    let xml = read(
        transport.as_ref(),
        url,
        envelope(USER, "rotated-password-583"),
    )
    .await;
    assert_eq!(
        find_response(
            &parse_soap_body(&xml).unwrap(),
            "GetDeviceInformationResponse"
        )
        .unwrap()
        .child("Manufacturer")
        .unwrap()
        .text(),
        expected
    );
    assert_eq!(hooks.load(Ordering::SeqCst), notifications + 1);
}

#[tokio::test]
async fn in_process_auth_uses_scoped_decoded_credentials() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let observed = hooks.clone();
    let mut state = MockState::new();
    state.set_on_change(Arc::new(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
    }));
    let transport = Arc::new(MockTransport::with_state(state).with_auth());
    exercise(
        transport.clone(),
        "http://mock",
        transport.device(),
        &hooks,
        false,
    )
    .await;
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn http_auth_uses_scoped_decoded_credentials() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let observed = hooks.clone();
    let server = oxvif::mock::MockServer::builder()
        .enforce_auth(true)
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
        true,
    )
    .await;
}
