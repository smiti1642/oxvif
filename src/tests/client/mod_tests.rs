//! Unit tests for the `OnvifClient` constructor/builder surface
//! (`src/client/mod.rs`).
//!
//! Only the two members that no other test file reaches are covered here:
//! [`OnvifClient::device_url`] and [`OnvifClient::with_utc_offset`]. `new`,
//! `with_credentials` and `with_transport` are exercised by essentially every
//! service test (`with_credentials` specifically by
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

    // `with_credentials` installs its own HTTP transport, so `with_transport`
    // has to come after it.
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
