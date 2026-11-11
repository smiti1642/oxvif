//! HB1: real local HTTP, without camera writes or external networking.
#![cfg(feature = "mock-server")]

use oxvif::{
    mock::{MockServer, MockTransport},
    soap::{SoapEnvelope, SoapError, WsSecurityToken, find_response, parse_soap_body},
    transport::{HttpTransport, Transport},
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation";

fn body() -> String {
    SoapEnvelope::new("<tds:GetDeviceInformation/>".into()).build()
}
fn content_type() -> String {
    format!("application/soap+xml; action=\"{ACTION}\"")
}
fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

#[tokio::test]
async fn header_refusals_preserve_fault_nonce_device_and_hooks() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let seen = hooks.clone();
    let server = MockServer::builder()
        .enforce_auth(true)
        .on_change(Arc::new(move |_| {
            seen.fetch_add(1, Ordering::SeqCst);
        }))
        .start()
        .await
        .unwrap();
    let before = serde_json::to_value(&*server.device().read()).unwrap();
    let valid = SoapEnvelope::new("<tds:GetDeviceInformation/>".into())
        .with_security(WsSecurityToken::generate("admin", "admin", 0))
        .build();
    server.inject_fault("GetDeviceInformation", "s:Receiver", "hb1-armed");
    let client = client();
    for (ct, coding, status) in [
        (None, None, 415),
        (Some("text/xml"), None, 415),
        (Some("application/soap+xml; charset=utf-16"), None, 415),
        (
            Some("application/soap+xml; action=\"urn:secret\"; action=\"urn:secret\""),
            None,
            400,
        ),
        (Some("application/soap+xml; action=\"\""), None, 400),
        (
            Some("application/soap+xml; action=\"urn:bad[part]/GetDeviceInformation\""),
            None,
            400,
        ),
        (
            Some("application/soap+xml; action=\"http://[not-an-ip]/GetDeviceInformation\""),
            None,
            400,
        ),
        (
            Some("application/soap+xml; action=\"http://example.test:abc/GetDeviceInformation\""),
            None,
            400,
        ),
        (Some("application/soap+xml; x=\"unfinished\\"), None, 400),
        (Some("application/soap+xml"), Some("gzip"), 415),
    ] {
        let mut request = client.post(server.device_url()).body(valid.clone());
        if let Some(ct) = ct {
            request = request.header("Content-Type", ct);
        }
        if let Some(coding) = coding {
            request = request.header("Content-Encoding", coding);
        }
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(
            response.headers()["content-type"],
            "text/plain; charset=utf-8"
        );
        let text = response.text().await.unwrap();
        assert_eq!(
            text,
            if status == 415 {
                "Unsupported mock SOAP HTTP representation.\n"
            } else {
                "Invalid mock SOAP HTTP headers.\n"
            }
        );
        assert_eq!(
            serde_json::to_value(&*server.device().read()).unwrap(),
            before
        );
        assert_eq!(hooks.load(Ordering::SeqCst), 0);
    }
    let mut duplicate = reqwest::header::HeaderMap::new();
    duplicate.append("content-type", content_type().parse().unwrap());
    duplicate.append("content-type", content_type().parse().unwrap());
    let response = client
        .post(server.device_url())
        .headers(duplicate)
        .body(valid.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    for (status, expected) in [
        (500, "hb1-armed"),
        (200, "GetDeviceInformationResponse"),
        (400, "Reused WS-Security nonce"),
    ] {
        let response = client
            .post(server.device_url())
            .header("Content-Type", content_type())
            .body(valid.clone())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(
            response.headers()["content-type"],
            "application/soap+xml; charset=utf-8"
        );
        assert!(response.text().await.unwrap().contains(expected));
    }
    assert_eq!(
        serde_json::to_value(&*server.device().read()).unwrap(),
        before
    );
    assert_eq!(hooks.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn recognized_fault_status_keeps_payload_and_typed_client_contract() {
    let server = MockServer::start().await.unwrap();
    let mock = MockTransport::new();
    let client = client();
    for (code, status) in [
        ("env:Sender", 400),
        ("s:Receiver", 500),
        ("s:VersionMismatch", 500),
        ("s:MustUnderstand", 500),
        ("s:DataEncodingUnknown", 500),
        ("ter:NoProfile", 200),
    ] {
        let reason = "literal <&> 北門";
        server.inject_fault("GetDeviceInformation", code, reason);
        mock.inject_fault("GetDeviceInformation", code, reason);
        let expected = mock.soap_post("http://mock", ACTION, body()).await.unwrap();
        let response = client
            .post(server.device_url())
            .header("Content-Type", content_type())
            .body(body())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{code}");
        assert_eq!(response.text().await.unwrap(), expected);
        server.inject_fault("GetDeviceInformation", code, reason);
        let xml = HttpTransport::new()
            .soap_post(server.device_url(), ACTION, body())
            .await
            .unwrap();
        assert_eq!(
            find_response(&parse_soap_body(&xml).unwrap(), "unused").unwrap_err(),
            SoapError::Fault {
                code: code.into(),
                reason: reason.into(),
                subcode: None,
                detail: None,
            }
        );
    }
    // An actual service refusal, not only injected Faults, maps to Sender/400.
    let response = client
        .post(server.device_url())
        .header(
            "Content-Type",
            content_type().replace("GetDeviceInformation", "SetHostname"),
        )
        .body(body())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    assert!(response.text().await.unwrap().contains("ter:TagMismatch"));
}

#[tokio::test]
async fn quoted_parameters_and_optional_action_keep_routing_policy() {
    let server = MockServer::start().await.unwrap();
    let client = client();
    let response = client
        .post(server.device_url())
        .header(
            "Content-Type",
            format!("Application/SOAP+XML; CHARSET=\"UTF-8\"; x=\"a;b\"; ACTION=\"{ACTION}\""),
        )
        .header("Content-Encoding", "identity")
        .header("SOAPAction", "ignored-legacy-header")
        .body(body())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response
            .text()
            .await
            .unwrap()
            .contains("GetDeviceInformationResponse")
    );
    // Absence is accepted by HTTP, then follows the existing empty-Action path.
    let response = client
        .post(server.device_url())
        .header("Content-Type", "application/soap+xml")
        .header("SOAPAction", ACTION)
        .body(body())
        .send()
        .await
        .unwrap();
    let xml = response.text().await.unwrap();
    let expected = MockTransport::new()
        .soap_post("http://mock", "", body())
        .await
        .unwrap();
    assert_eq!(xml, expected);
}

#[tokio::test]
async fn http_header_length_and_encoding_boundaries_are_exact() {
    let server = MockServer::start().await.unwrap();
    let client = client();
    let prefix = format!("{}; x=", content_type());
    let at_limit = format!("{prefix}{}", "a".repeat(8192 - prefix.len()));
    for (value, status) in [(at_limit.clone(), 200), (at_limit + "a", 400)] {
        let response = client
            .post(server.device_url())
            .header("Content-Type", value)
            .body(body())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        let ct = response.headers()["content-type"]
            .to_str()
            .unwrap()
            .to_owned();
        let xml = response.text().await.unwrap();
        if status == 200 {
            assert_eq!(ct, "application/soap+xml; charset=utf-8");
            assert_eq!(
                find_response(
                    &parse_soap_body(&xml).unwrap(),
                    "GetDeviceInformationResponse"
                )
                .unwrap()
                .child("Manufacturer")
                .unwrap()
                .text(),
                "oxvif-mock"
            );
        } else {
            assert_eq!(ct, "text/plain; charset=utf-8");
            assert_eq!(xml, "Invalid mock SOAP HTTP headers.\n");
        }
    }
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("content-type", content_type().parse().unwrap());
    headers.append("content-encoding", "identity".parse().unwrap());
    headers.append("content-encoding", "identity".parse().unwrap());
    let response = client
        .post(server.device_url())
        .headers(headers)
        .body(body())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    assert_eq!(
        response.headers()["content-type"],
        "text/plain; charset=utf-8"
    );
    assert_eq!(
        response.text().await.unwrap(),
        "Invalid mock SOAP HTTP headers.\n"
    );
    // Axum's Bytes extractor runs before the adapter and rejects oversized bodies
    // even when their representation headers would otherwise yield 415.
    let response = client
        .post(server.device_url())
        .header("Content-Type", "text/xml")
        .body("x".repeat(2 * 1024 * 1024 + 1))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
    assert_eq!(
        response.text().await.unwrap(),
        "Failed to buffer the request body: length limit exceeded"
    );
}

#[cfg(feature = "metamorph")]
#[tokio::test]
async fn rejected_write_headers_preserve_device_hooks_and_replay() {
    use oxvif::{OnvifClient, metamorph::FixtureStore, mock::DeviceState};
    let get = ACTION.replace("GetDeviceInformation", "GetHostname");
    let set = ACTION.replace("GetDeviceInformation", "SetHostname");
    let request = SoapEnvelope::new("<tds:GetHostname/>".into())
        .with_wsa_to("http://recorded-hb1")
        .build();
    let recorded = "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:tds='http://www.onvif.org/ver10/device/wsdl'><s:Body><tds:GetHostnameResponse><tds:HostnameInformation><FromDHCP>false</FromDHCP><Name>recorded-hb1-217</Name></tds:HostnameInformation></tds:GetHostnameResponse></s:Body></s:Envelope>";
    let mut store = FixtureStore::new("synthetic-hb1-refusal");
    store.record(&get, &request, recorded);
    let hooks = Arc::new(AtomicUsize::new(0));
    let seen = hooks.clone();
    let state = DeviceState {
        hostname: "live-hb1-218".into(),
        ..Default::default()
    };
    let server = MockServer::builder()
        .initial_state(state)
        .replay(store)
        .on_change(Arc::new(move |_| {
            seen.fetch_add(1, Ordering::SeqCst);
        }))
        .start()
        .await
        .unwrap();
    let before = serde_json::to_value(&*server.device().read()).unwrap();
    let write = SoapEnvelope::new(
        "<tds:SetHostname><tds:Name>committed-hb1-219</tds:Name></tds:SetHostname>".into(),
    )
    .build();
    let http = client();
    let typed = OnvifClient::new(server.device_url());
    assert_eq!(
        typed.get_hostname().await.unwrap().name.as_deref(),
        Some("recorded-hb1-217")
    );
    for (ct, expected_status) in [
        (format!("text/xml; action=\"{set}\""), 415),
        (
            format!("application/soap+xml; action=\"{set}\"; charset=utf-16"),
            415,
        ),
        (
            format!("application/soap+xml; action=\"{set}\"; action=\"{set}\""),
            400,
        ),
        (
            "application/soap+xml; action=\"urn:bad[part]/SetHostname\"".into(),
            400,
        ),
    ] {
        let response = http
            .post(server.device_url())
            .header("Content-Type", ct)
            .body(write.clone())
            .send()
            .await
            .unwrap();
        let status = response.status().as_u16();
        let content_type = response.headers()["content-type"]
            .to_str()
            .unwrap()
            .to_owned();
        let text = response.text().await.unwrap();
        // Assert effects before the rejection shape: bypassing the media gate
        // must fail on the reproduced write, not just the expected HTTP code.
        assert_eq!(
            serde_json::to_value(&*server.device().read()).unwrap(),
            before
        );
        assert_eq!(hooks.load(Ordering::SeqCst), 0);
        assert_eq!(
            typed.get_hostname().await.unwrap().name.as_deref(),
            Some("recorded-hb1-217")
        );
        assert_eq!(content_type, "text/plain; charset=utf-8");
        assert_eq!(
            text,
            if expected_status == 415 {
                "Unsupported mock SOAP HTTP representation.\n"
            } else {
                "Invalid mock SOAP HTTP headers.\n"
            }
        );
        assert_eq!(status, expected_status);
    }
    let response = http
        .post(server.device_url())
        .header(
            "Content-Type",
            format!("application/soap+xml; action=\"{set}\""),
        )
        .body(write)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        find_response(
            &parse_soap_body(&response.text().await.unwrap()).unwrap(),
            "SetHostnameResponse"
        )
        .unwrap()
        .local_name,
        "SetHostnameResponse"
    );
    assert_eq!(server.device().read().hostname, "committed-hb1-219");
    assert_eq!(hooks.load(Ordering::SeqCst), 1);
    assert_eq!(
        typed.get_hostname().await.unwrap().name.as_deref(),
        Some("committed-hb1-219")
    );
}

#[tokio::test]
async fn actual_auth_and_service_refusals_keep_typed_client_and_session_errors() {
    use oxvif::{OnvifClient, OnvifError, OnvifSession};
    let authenticated = MockServer::builder()
        .enforce_auth(true)
        .start()
        .await
        .unwrap();
    let expected_auth = SoapError::Fault {
        code: "s:Sender".into(),
        subcode: Some("wsse:FailedAuthentication".into()),
        reason: "Missing Username".into(),
        detail: None,
    };
    let error = OnvifClient::new(authenticated.device_url())
        .get_device_info()
        .await
        .unwrap_err();
    match error {
        OnvifError::Soap(error) => assert_eq!(error, expected_auth),
        other => panic!("{other:?}"),
    }
    let failure = OnvifSession::builder(authenticated.device_url())
        .build_with_report()
        .await
        .err()
        .unwrap();
    assert_eq!(
        failure.report.stages[1].outcome,
        oxvif::session::SessionStageOutcome::Failed
    );
    assert_eq!(
        failure.report.stages[1].error_class,
        Some(oxvif::session::SessionErrorClass::DeviceFault)
    );
    assert_eq!(failure.report.stages[1].http_status, None);
    match failure.error {
        OnvifError::Soap(error) => assert_eq!(error, expected_auth),
        other => panic!("{other:?}"),
    }
    let server = MockServer::start().await.unwrap();
    let before = serde_json::to_value(&*server.device().read()).unwrap();
    let error = OnvifClient::new(server.device_url())
        .delete_profile(server.device_url(), "absent-hb1-220")
        .await
        .unwrap_err();
    match error {
        OnvifError::Soap(error) => assert_eq!(
            error,
            SoapError::Fault {
                code: "s:Sender".into(),
                subcode: Some("ter:InvalidArgVal".into()),
                reason: "Profile not found: absent-hb1-220".into(),
                detail: None,
            }
        ),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        serde_json::to_value(&*server.device().read()).unwrap(),
        before
    );
}

#[cfg(feature = "metamorph")]
#[tokio::test]
async fn raw_replay_status_requires_valid_xml_and_preserves_exact_bytes() {
    use oxvif::metamorph::FixtureStore;
    let soap = "http://www.w3.org/2003/05/soap-envelope";
    let http = client();
    for (body, status) in [
        (
            "<s:Fault><s:Code><s:Value>s:Sender</s:Value></s:Code></s:Fault>",
            400,
        ),
        (
            "<s:Fault><s:Code><s:Value>s:Receiver</s:Value></s:Code></s:Fault>",
            500,
        ),
        (
            "<s:Fault x='<'><s:Code><s:Value>s:Sender</s:Value></s:Code></s:Fault>",
            200,
        ),
        (
            "<s:Fault 1bad='x'><s:Code><s:Value>s:Sender</s:Value></s:Code></s:Fault>",
            200,
        ),
        (
            "<s:Fault><s:Code><s:Value>s:Sender</s:Value></s:Code><1bad/></s:Fault>",
            200,
        ),
    ] {
        let xml = format!("<s:Envelope xmlns:s='{soap}'><s:Body>{body}</s:Body></s:Envelope>");
        let mut store = FixtureStore::new("synthetic-hb1-raw");
        store.record(ACTION, &self::body(), &xml);
        let expected = store.fixtures()[0].response_raw.clone();
        assert_eq!(expected, xml);
        let server = MockServer::builder().replay(store).start().await.unwrap();
        let response = http
            .post(server.device_url())
            .header("Content-Type", content_type())
            .body(self::body())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(
            response.headers()["content-type"],
            "application/soap+xml; charset=utf-8"
        );
        assert_eq!(
            response.bytes().await.unwrap().as_ref(),
            expected.as_bytes()
        );
    }
}
