//! Unit tests for the `OnvifClient` constructor/builder surface
//! (`src/client/mod.rs`).
//!
//! Covered here: the members no other test file reaches —
//! [`OnvifClient::device_url`] and [`OnvifClient::with_utc_offset`] — plus the
//! *interaction* between [`OnvifClient::with_credentials`] and
//! [`OnvifClient::with_transport`], which no single-method test can see. Each
//! of those two in isolation is exercised by essentially every service test
//! (`with_credentials` specifically by
//! `client::device::tests::test_credentials_add_ws_security_header`).

use super::*;
use crate::soap::security::unix_secs_to_iso8601;
use crate::tests::common::*;

// ── device_url ────────────────────────────────────────────────────────────────

#[test]
fn device_url_returns_the_url_the_client_was_constructed_with() {
    const URL: &str = "http://10.7.0.4:8899/onvif/device_service";

    let client = OnvifClient::new(URL);
    assert_eq!(client.device_url(), URL);

    // A second client must report its own URL, not a shared or defaulted one.
    let other = OnvifClient::new("http://192.0.2.77/onvif/device_service");
    assert_eq!(other.device_url(), "http://192.0.2.77/onvif/device_service");

    // The builders return `Self` by value; none of them may drop or rewrite
    // the URL on the way through.
    let built = OnvifClient::new(URL)
        .with_credentials("admin", "password")
        .with_utc_offset(42)
        .with_transport(mock("<unused/>"));
    assert_eq!(built.device_url(), URL);
}

// ── with_utc_offset ───────────────────────────────────────────────────────────

/// A minimal `GetDeviceInformationResponse`; `DeviceInfo::from_xml` defaults
/// every absent field, so this parses cleanly and the call returns `Ok`.
fn device_info_response() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body>
           <tds:GetDeviceInformationResponse>
             <tds:Manufacturer>oxvif-test</tds:Manufacturer>
           </tds:GetDeviceInformationResponse>
         </s:Body>
       </s:Envelope>"#
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Pull the `<wsu:Created>` text out of an emitted request body.
#[track_caller]
fn created_timestamp(body: &str) -> String {
    let start = body
        .find("<wsu:Created>")
        .expect("request carries a WS-Security <wsu:Created>")
        + "<wsu:Created>".len();
    let end = body[start..]
        .find("</wsu:Created>")
        .expect("<wsu:Created> is closed");
    body[start..start + end].to_string()
}

/// `with_utc_offset` has exactly one observable effect: it shifts the
/// WS-Security `<wsu:Created>` timestamp of every subsequent request by that
/// many seconds (`OnvifClient::security_token` → `WsSecurityToken::generate`).
///
/// The local clock is read either side of the call, so the expected timestamp
/// is the *set* of stamps a clock inside that window could legitimately have
/// produced — exact, and independent of how long the test takes to run.
/// A zero-offset control captured in the same window rules out a coincidence.
#[tokio::test]
async fn with_utc_offset_shifts_the_ws_security_created_timestamp() {
    // 7 days + 1h 1m 1s. Large enough that no plausible clock jitter reaches
    // it, and not a round number, so an accidental unit slip cannot match.
    const OFFSET_SECS: i64 = 608_461;
    const URL: &str = "http://192.168.1.1/onvif/device_service";

    let (shifted_transport, shifted) = RecordingTransport::new(device_info_response());
    let (plain_transport, plain) = RecordingTransport::new(device_info_response());

    let before = unix_now();

    OnvifClient::new(URL)
        .with_credentials("admin", "password")
        .with_utc_offset(OFFSET_SECS)
        .with_transport(shifted_transport)
        .get_device_info()
        .await
        .unwrap();

    OnvifClient::new(URL)
        .with_credentials("admin", "password")
        .with_transport(plain_transport)
        .get_device_info()
        .await
        .unwrap();

    let after = unix_now();

    let shifted_created = created_timestamp(&shifted.lock().unwrap().body);
    let plain_created = created_timestamp(&plain.lock().unwrap().body);

    let stamps = |offset: i64| -> Vec<String> {
        (before..=after)
            .map(|t| unix_secs_to_iso8601(t + offset))
            .collect()
    };

    assert!(
        stamps(OFFSET_SECS).contains(&shifted_created),
        "offset client stamped {shifted_created}, expected one of {:?}",
        stamps(OFFSET_SECS)
    );
    assert!(
        stamps(0).contains(&plain_created),
        "control client stamped {plain_created}, expected one of {:?}",
        stamps(0)
    );
    assert_ne!(
        shifted_created, plain_created,
        "the offset must change the emitted timestamp"
    );
}

// ── with_transport × with_credentials: order independence ─────────────────────
//
// `with_credentials` used to assign a fresh `HttpTransport` over whatever
// `transport` held, so `.with_transport(t).with_credentials(u, p)` silently
// discarded `t` and went to the network. Both orders must now behave alike.

const ORDER_URL: &str = "http://127.0.0.1:1/onvif/device_service";

/// The action `get_device_info` sends, so the assertion names the operation
/// that was actually routed rather than merely "something was routed".
const GET_DEVICE_INFO: &str = "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation";

/// `.with_transport(t).with_credentials(..)` — the order that used to lose `t`.
///
/// `ORDER_URL` points at a closed port, so if the installed transport were
/// dropped the request would leave for the network and the recorder would stay
/// empty. The capture is asserted before the result is unwrapped, so the
/// regression surfaces as a failed assertion rather than a transport error.
#[tokio::test]
async fn with_credentials_after_with_transport_keeps_the_installed_transport() {
    let (transport, captured) = RecordingTransport::new(device_info_response());

    let res = OnvifClient::new(ORDER_URL)
        .with_transport(transport)
        .with_credentials("admin", "s3cret-after")
        .get_device_info()
        .await;

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action, GET_DEVICE_INFO,
        "installed transport saw the call"
    );
    assert_eq!(c.url, ORDER_URL, "and was handed the device URL");
    assert!(
        c.body.contains("<wsse:Username>admin</wsse:Username>"),
        "credentials still reach the SOAP header: {}",
        c.body
    );

    assert_eq!(res.unwrap().manufacturer, "oxvif-test");
}

/// The mirror image — `.with_credentials(..).with_transport(t)` — which worked
/// before the fix. Pinning it proves the fix did not simply move the hazard to
/// the other order. Same fixture, a different password, so the two tests cannot
/// pass on each other's captured body.
#[tokio::test]
async fn with_transport_after_with_credentials_keeps_the_installed_transport() {
    let (transport, captured) = RecordingTransport::new(device_info_response());

    let res = OnvifClient::new(ORDER_URL)
        .with_credentials("operator", "s3cret-before")
        .with_transport(transport)
        .get_device_info()
        .await;

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action, GET_DEVICE_INFO,
        "installed transport saw the call"
    );
    assert!(
        c.body.contains("<wsse:Username>operator</wsse:Username>"),
        "credentials still reach the SOAP header: {}",
        c.body
    );

    assert_eq!(res.unwrap().manufacturer, "oxvif-test");
}

/// With no transport installed the client falls back to a lazily built,
/// memoised `HttpTransport` whose HTTP Digest credentials come from
/// `self.credentials`. Setting credentials *after* something has already forced
/// that transport into existence must therefore discard it — otherwise the
/// memoised, credential-less one is kept and Digest never reaches the wire.
///
/// `first` is held across the `with_credentials` call so the old allocation
/// cannot be freed and its address reused, which would make `ptr_eq` lie.
#[test]
fn credentials_discard_a_default_transport_that_was_already_built() {
    let client = OnvifClient::new(ORDER_URL);
    let first = client.transport().clone();

    let client = client.with_credentials("admin", "s3cret-default");
    assert!(
        !std::sync::Arc::ptr_eq(&first, client.transport()),
        "with_credentials must discard a default transport built before it, \
         or the HTTP Digest credentials would never reach the wire"
    );
}
