//! Proves the hand-copied client-test fixtures still match what the mock
//! actually serves.
//!
//! The unit tests in `src/tests/client/*_tests.rs` embed a copy of each
//! service's `GetServiceCapabilities` body, because `src/mock/` is behind
//! `#[cfg(feature = "mock")]` and those tests compile without it. A copy that
//! nobody checks drifts. This file drives the **real** `MockServer` over HTTP
//! through the **real** client, so at least one service is proven equal rather
//! than equal by convention — and so a change to a mock responder that breaks
//! the parser is caught here instead of on a camera.
//!
//! Run with: `cargo test --features mock-server --test mock_service_capabilities`
#![cfg(feature = "mock-server")]

use oxvif::OnvifSession;
use oxvif::mock::MockServer;

async fn setup() -> (MockServer, OnvifSession) {
    let server = MockServer::start().await.expect("start mock server");
    let session = OnvifSession::builder(server.device_url())
        .build()
        .await
        .expect("build session");
    (server, session)
}

/// PTZ is the service §3.6 of the Tier 1 plan nominates for this check: its
/// fixture carries the whole spread — a `Vec<String>` attribute, two present
/// booleans, and two deliberately omitted ones.
#[tokio::test]
async fn ptz_service_capabilities_over_http_match_the_unit_fixture() {
    let (_srv, s) = setup().await;

    let caps = s.ptz_get_service_capabilities().await.unwrap();

    // Identical assertions to `ptz_service_capabilities_parses_move_and_track`
    // in `src/tests/client/ptz_tests.rs`. If the two ever disagree, one of the
    // two XML bodies has drifted.
    assert_eq!(caps.move_and_track, ["PresetToken", "PTZVector"]);
    assert_eq!(caps.eflip, None);
    assert_eq!(caps.reverse, None);
    assert_eq!(caps.move_status, Some(true));
    assert_eq!(caps.status_position, Some(true));
    assert_eq!(caps.get_compatible_configurations, Some(true));
}

/// All nine round-trip over HTTP without a parse error, and each returns its
/// *own* service's answer. Recording, Search and Replay share one mock module
/// and one client file, so "answered at all" is not enough — the three are
/// distinguished by a field only one of them has.
#[tokio::test]
async fn every_service_capabilities_call_round_trips() {
    let (_srv, s) = setup().await;

    let device = s.device_get_service_capabilities().await.unwrap();
    assert_eq!(device.security.tls1_2, Some(true));
    assert_eq!(
        device.misc.expect("Misc").auxiliary_commands.len(),
        5,
        "the discoverable auxiliary-command list"
    );

    // Media1 and Media2 disagree on `VideoSourceMode` on purpose.
    let media = s.media_get_service_capabilities().await.unwrap();
    let media2 = s.media2_get_service_capabilities().await.unwrap();
    assert_eq!(media.video_source_mode, Some(false));
    assert_eq!(media2.video_source_mode, Some(true));
    assert_eq!(media.streaming.rtp_tcp, Some(true), "Media1-only transport");
    assert_eq!(media2.webrtc, Some(0), "a session count, not a flag");

    assert_eq!(
        s.imaging_get_service_capabilities()
            .await
            .unwrap()
            .adaptable_preset,
        Some(false)
    );
    assert_eq!(
        s.events_get_service_capabilities()
            .await
            .unwrap()
            .max_event_brokers,
        Some(0)
    );

    let recording = s.recording_get_service_capabilities().await.unwrap();
    let search = s.search_get_service_capabilities().await.unwrap();
    let replay = s.replay_get_service_capabilities().await.unwrap();
    assert_eq!(recording.max_recordings, Some(2.5), "xs:float, not xs:int");
    assert_eq!(
        recording.onboard_storage, None,
        "schema default not applied"
    );
    assert_eq!(search.nl_search, Some(false));
    assert_eq!(replay.session_timeout_range, Some((1.0, 600.0)));
}
