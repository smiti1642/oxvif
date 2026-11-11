use super::*;
use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use oxvif::{
    OnvifSession,
    client::AuthenticationMode,
    transport::{HttpTransport, HttpTransportOptions, SoapRequestOptions},
};

const INFO: &str = "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation";
const CAPS: &str = "http://www.onvif.org/ver10/device/wsdl/GetCapabilities";
const SERVICES: &str = "http://www.onvif.org/ver10/device/wsdl/GetServices";
const CLOCK: &str = "http://www.onvif.org/ver10/device/wsdl/GetSystemDateAndTime";
const DEVICE_INFO: &str = "<d:GetDeviceInformationResponse><d:Manufacturer>Fixture Cam</d:Manufacturer><d:Model>Test</d:Model><d:FirmwareVersion>1</d:FirmwareVersion><d:SerialNumber>2</d:SerialNumber><d:HardwareId>3</d:HardwareId></d:GetDeviceInformationResponse>";
type Received = Arc<Mutex<Vec<(Option<String>, String)>>>;

#[tokio::test]
async fn authentication_modes_emit_only_the_selected_mechanisms() {
    for mode in [
        AuthenticationMode::Combined,
        AuthenticationMode::Digest,
        AuthenticationMode::WsSecurity,
    ] {
        let expect_digest = mode != AuthenticationMode::WsSecurity;
        let expect_wsse = mode != AuthenticationMode::Digest;
        let received = Arc::new(Mutex::new(Vec::new()));
        let router = Router::new().route("/soap", post(move |
            State(received): State<Received>, headers: HeaderMap, body: String,
        | async move {
            let auth = headers.get("authorization").map(|v| v.to_str().unwrap().to_owned());
            received.lock().unwrap().push((auth.clone(), body));
            if expect_digest && auth.is_none() {
                (StatusCode::UNAUTHORIZED, [("www-authenticate", "Digest realm=\"fixture\", nonce=\"fixture-nonce\", qop=\"auth\", algorithm=MD5")], String::new())
            } else {
                (StatusCode::OK, [("content-type", "application/soap+xml")], envelope(DEVICE_INFO))
            }
        })).with_state(received.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/soap", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let info = OnvifClient::new(url)
            .with_authentication_mode(mode)
            .with_credentials("fixture-user", "fixture-password")
            .get_device_info()
            .await
            .unwrap();
        assert_eq!(info.manufacturer, "Fixture Cam");
        let calls = received.lock().unwrap();
        assert_eq!(calls.len(), if expect_digest { 2 } else { 1 });
        assert_eq!(calls[0].0, None);
        if expect_digest {
            let digest = calls[1].0.as_deref().unwrap();
            assert!(digest.starts_with("Digest "));
            assert!(digest.contains("username=\"fixture-user\""));
        }
        for (_, body) in calls.iter() {
            assert_eq!(
                body.contains("<wsse:Username>fixture-user</wsse:Username>"),
                expect_wsse
            );
            assert!(!body.contains("fixture-password"));
        }
        server.abort();
    }
}

#[tokio::test]
async fn authentication_builder_order_and_auto_clock_are_independent_of_transport() {
    for mode in [
        AuthenticationMode::Combined,
        AuthenticationMode::Digest,
        AuthenticationMode::WsSecurity,
    ] {
        for reversed in [false, true] {
            let script = Script::new(vec![(INFO, reply(DEVICE_INFO))]);
            let client = if reversed {
                OnvifClient::new(DEVICE)
                    .with_transport(script.clone())
                    .with_credentials("custom", "secret")
                    .with_authentication_mode(mode)
            } else {
                OnvifClient::new(DEVICE)
                    .with_authentication_mode(mode)
                    .with_credentials("custom", "secret")
                    .with_transport(script.clone())
            };
            assert_eq!(
                client.get_device_info().await.unwrap().manufacturer,
                "Fixture Cam"
            );
            assert_eq!(
                script.calls()[0]
                    .body
                    .contains("<wsse:Username>custom</wsse:Username>"),
                mode.uses_ws_security()
            );
            script.assert_done();
        }
        let mut steps = Vec::new();
        if mode.uses_ws_security() {
            steps.push((CLOCK, reply("<d:GetSystemDateAndTimeResponse><d:SystemDateAndTime/></d:GetSystemDateAndTimeResponse>")));
        }
        steps.extend([
            (
                CAPS,
                reply("<d:GetCapabilitiesResponse><d:Capabilities/></d:GetCapabilitiesResponse>"),
            ),
            (SERVICES, reply("<d:GetServicesResponse/>")),
        ]);
        let script = Script::new(steps);
        let (session, report) = OnvifSession::builder(DEVICE)
            .with_credentials("custom", "secret")
            .with_transport(script.clone())
            .with_authentication_mode(mode)
            .with_auto_clock_sync()
            .build_with_report()
            .await
            .unwrap();
        assert_eq!(session.client().authentication_mode(), mode);
        assert_eq!(
            report.stages[0].outcome,
            if mode.uses_ws_security() {
                oxvif::session::SessionStageOutcome::Succeeded
            } else {
                oxvif::session::SessionStageOutcome::Skipped
            }
        );
        script.assert_done();
    }
}

#[tokio::test(start_paused = true)]
async fn legacy_custom_transport_inherits_outer_exchange_deadline() {
    let script = Script::new(vec![(INFO, Reply::Pending)]);
    let started = tokio::time::Instant::now();
    let error = script
        .soap_post_with_options(
            DEVICE,
            INFO,
            "<request/>".into(),
            SoapRequestOptions::with_timeout(Duration::from_secs(3)),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, TransportError::Deadline));
    assert_eq!(started.elapsed(), Duration::from_secs(3));
    assert_eq!(script.calls()[0].action, INFO);
    script.assert_done();
}

#[tokio::test]
async fn request_deadline_overrides_only_one_exchange_and_refuses_redirects() {
    let router = Router::new()
        .route(
            "/soap",
            post(|| async {
                tokio::time::sleep(Duration::from_millis(250)).await;
                (StatusCode::OK, envelope(DEVICE_INFO))
            }),
        )
        .route(
            "/redirect",
            post(|| async { (StatusCode::TEMPORARY_REDIRECT, [("location", "/soap")], "") }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let transport = HttpTransport::from_options(HttpTransportOptions {
        timeout: Duration::from_millis(100),
        ..Default::default()
    })
    .unwrap();
    let url = format!("{base}/soap");
    let (normal, extended) = tokio::join!(
        transport.soap_post(&url, INFO, "<request/>".into()),
        transport.soap_post_with_options(
            &url,
            INFO,
            "<request/>".into(),
            SoapRequestOptions::with_timeout(Duration::from_secs(2))
        ),
    );
    assert!(matches!(normal, Err(TransportError::Http(error)) if error.is_timeout()));
    assert!(extended.unwrap().contains("Fixture Cam"));
    assert_eq!(transport.options().timeout, Duration::from_millis(100));
    let error = transport
        .soap_post_with_options(
            &format!("{base}/redirect"),
            INFO,
            "<request/>".into(),
            SoapRequestOptions::with_timeout(Duration::from_secs(2)),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        TransportError::HttpStatus { status: 307, .. }
    ));
    server.abort();
}

#[tokio::test]
async fn digest_challenge_and_retry_share_the_exchange_deadline() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let exchanges = Arc::new(AtomicUsize::new(0));
    let router = Router::new()
        .route(
            "/digest",
            post(
                |State(exchanges): State<Arc<AtomicUsize>>, headers: HeaderMap| async move {
                    exchanges.fetch_add(1, Ordering::Relaxed);
                    if let Some(auth) = headers.get("authorization") {
                        assert!(auth.to_str().unwrap().starts_with("Digest "));
                        tokio::time::sleep(Duration::from_millis(450)).await;
                        (
                            StatusCode::OK,
                            [("content-type", "application/soap+xml")],
                            envelope(DEVICE_INFO),
                        )
                    } else {
                        tokio::time::sleep(Duration::from_millis(300)).await;
                        (
                            StatusCode::UNAUTHORIZED,
                            [(
                                "www-authenticate",
                                "Digest realm=\"fixture\", nonce=\"lease\", qop=\"auth\"",
                            )],
                            String::new(),
                        )
                    }
                },
            ),
        )
        .route(
            "/basic",
            post(|headers: HeaderMap| async move {
                assert!(headers.get("authorization").is_none());
                (
                    StatusCode::UNAUTHORIZED,
                    [("www-authenticate", "Basic realm=\"fixture\"")],
                    "",
                )
            }),
        )
        .with_state(exchanges.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let transport = HttpTransport::new().with_credentials("fixture-user", "fixture-password");
    let basic = transport
        .soap_post_with_options(
            &format!("{base}/basic"),
            INFO,
            "<request/>".into(),
            SoapRequestOptions::with_timeout(Duration::from_secs(2)),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        basic,
        TransportError::HttpStatus { status: 401, .. }
    ));
    let deadline = transport
        .soap_post_with_options(
            &format!("{base}/digest"),
            INFO,
            "<request/>".into(),
            SoapRequestOptions::with_timeout(Duration::from_millis(620)),
        )
        .await
        .unwrap_err();
    assert!(matches!(deadline, TransportError::Deadline));
    assert_eq!(exchanges.load(Ordering::Relaxed), 2);
    server.abort();
}
