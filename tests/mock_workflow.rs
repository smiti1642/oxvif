//! End-to-end integration tests: boot a real `MockServer`, point an
//! `OnvifSession` at it, and exercise a representative oxvif command from every
//! service. Doubles as a copy-paste template for downstream crates.
//!
//! Run with: `cargo test --features mock-server --test mock_workflow`
//!
//! The whole file compiles to nothing without the feature, so a plain
//! `cargo test` is unaffected.
#![cfg(feature = "mock-server")]

use oxvif::mock::MockServer;
use oxvif::{ImagingSettings, OnvifSession};

/// Start a mock server and a session wired to it. The returned `MockServer`
/// must be kept alive for the session to keep working (it shuts down on drop).
async fn setup() -> (MockServer, OnvifSession) {
    let server = MockServer::start().await.expect("start mock server");
    let session = OnvifSession::builder(server.device_url())
        .build()
        .await
        .expect("build session"); // no credentials — mock doesn't enforce auth
    (server, session)
}

#[tokio::test]
async fn device_commands() {
    let (_srv, s) = setup().await;

    // Capabilities were fetched + cached during build().
    assert!(s.capabilities().media.url.is_some());

    let info = s.get_device_info().await.unwrap();
    assert_eq!(info.manufacturer, "oxvif-mock");

    s.get_system_date_and_time().await.unwrap();
    assert!(!s.get_scopes().await.unwrap().is_empty());
    assert!(!s.get_users().await.unwrap().is_empty());

    // Set → Get round-trips through real HTTP.
    s.set_hostname("integration-cam").await.unwrap();
    assert_eq!(
        s.get_hostname().await.unwrap().name.as_deref(),
        Some("integration-cam")
    );
}

#[tokio::test]
async fn media_and_streaming() {
    let (_srv, s) = setup().await;

    let profiles = s.get_profiles().await.unwrap();
    assert!(!profiles.is_empty());
    let token = &profiles[0].token;

    assert!(
        s.get_stream_uri(token)
            .await
            .unwrap()
            .uri
            .starts_with("rtsp://")
    );
    assert!(!s.get_snapshot_uri(token).await.unwrap().uri.is_empty());
    assert!(
        !s.get_video_encoder_configurations()
            .await
            .unwrap()
            .is_empty()
    );
    // OSD list (the default mock seeds one DateAndTime overlay).
    assert!(!s.get_osds(None).await.unwrap().is_empty());
}

#[tokio::test]
async fn media2_encoder_set_then_get() {
    let (_srv, s) = setup().await;

    assert!(!s.get_profiles_media2().await.unwrap().is_empty());

    let mut cfg = s
        .get_video_encoder_configurations_media2()
        .await
        .unwrap()
        .remove(0);
    if let Some(rc) = cfg.rate_control.as_mut() {
        rc.bitrate_limit = 1234;
    }
    s.set_video_encoder_configuration_media2(&cfg)
        .await
        .unwrap();

    let after = s.get_video_encoder_configurations_media2().await.unwrap();
    assert_eq!(after[0].rate_control.as_ref().unwrap().bitrate_limit, 1234);
}

#[tokio::test]
async fn ptz_commands() {
    let (_srv, s) = setup().await;
    let profile = s.get_profiles().await.unwrap()[0].token.clone();

    assert!(!s.ptz_get_nodes().await.unwrap().is_empty());
    assert!(!s.ptz_get_presets(&profile).await.unwrap().is_empty());

    // Move, then status should reflect the new position.
    s.ptz_absolute_move(&profile, 0.5, -0.3, 0.7).await.unwrap();
    let status = s.ptz_get_status(&profile).await.unwrap();
    assert_eq!(status.pan, Some(0.5));
    assert_eq!(status.tilt, Some(-0.3));
}

#[tokio::test]
async fn imaging_set_then_get() {
    let (_srv, s) = setup().await;
    let vsc = "VideoSource_1";

    let mut settings: ImagingSettings = s.get_imaging_settings(vsc).await.unwrap();
    settings.brightness = Some(33.0);
    s.set_imaging_settings(vsc, &settings).await.unwrap();

    let after = s.get_imaging_settings(vsc).await.unwrap();
    assert_eq!(after.brightness, Some(33.0));
}

#[tokio::test]
async fn events_pull_point() {
    let (_srv, s) = setup().await;

    let sub = s
        .create_pull_point_subscription(None, Some("PT60S"))
        .await
        .unwrap();
    let msgs = s
        .pull_messages(&sub.reference_url, "PT1S", 10)
        .await
        .unwrap();
    assert!(!msgs.is_empty(), "mock emits an event per pull");
}

#[tokio::test]
async fn recording_search_replay() {
    let (_srv, s) = setup().await;

    // Recording list (mock seeds one).
    let _ = s.get_recordings().await.unwrap();

    // Search session returns a token.
    let token = s.find_recordings(None, "PT60S").await.unwrap();
    assert!(!token.is_empty());

    // Replay URI for a recording.
    let uri = s
        .get_replay_uri("rec1", "RTP-Unicast", "RTSP")
        .await
        .unwrap();
    assert!(uri.starts_with("rtsp://"));
}

#[tokio::test]
async fn io_relay_and_digital_input_flow() {
    let (srv, s) = setup().await;

    // Defaults: two relays, two inputs.
    let relays = s.get_relay_outputs().await.unwrap();
    assert_eq!(relays.len(), 2);
    assert!(relays.iter().any(|r| r.token == "RelayOutput_1"));

    let inputs = s.get_digital_inputs().await.unwrap();
    assert_eq!(inputs.len(), 2);
    assert!(inputs.iter().any(|d| d.token == "DigitalInput_1"));

    // Flip the bistable relay's logical state. Spec says it doesn't
    // appear in GetRelayOutputs, but the mock holds it for tests.
    s.set_relay_output_state("RelayOutput_1", "active")
        .await
        .unwrap();
    // Drop the guard inside a block so clippy doesn't flag a stale
    // lock held across the next `.await`.
    let r1_logical = {
        let snap = srv.device().read();
        snap.relay_outputs
            .iter()
            .find(|r| r.token == "RelayOutput_1")
            .unwrap()
            .logical_state
            .clone()
    };
    assert_eq!(r1_logical, "active");

    // Configure properties (Bistable → Monostable + delay).
    s.set_relay_output_settings("RelayOutput_1", "Monostable", "PT2S", "open")
        .await
        .unwrap();
    let after = s.get_relay_outputs().await.unwrap();
    let r1_after = after.iter().find(|r| r.token == "RelayOutput_1").unwrap();
    assert_eq!(r1_after.mode, "Monostable");
    assert_eq!(r1_after.delay_time, "PT2S");
    assert_eq!(r1_after.idle_state, "open");

    // Trigger an input pulse through the REST hook, then PullMessages
    // should drain the pending queue in FIFO order:
    //   1. RelayOutput  (queued by SetRelayOutputState above)
    //   2. DigitalInput active  (pulse first half)
    //   3. DigitalInput inactive  (pulse second half)
    let pulse_url = format!("{}/mock/digital-input/DigitalInput_1/pulse", srv.base_url());
    let resp = reqwest::Client::new()
        .post(&pulse_url)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let sub = s
        .create_pull_point_subscription(None, Some("PT60S"))
        .await
        .unwrap();
    let m1 = s
        .pull_messages(&sub.reference_url, "PT1S", 1)
        .await
        .unwrap();
    let m2 = s
        .pull_messages(&sub.reference_url, "PT1S", 1)
        .await
        .unwrap();
    let m3 = s
        .pull_messages(&sub.reference_url, "PT1S", 1)
        .await
        .unwrap();
    assert!(m1[0].topic.contains("RelayOutput"), "got {:?}", m1[0].topic);
    assert!(
        m2[0].topic.contains("DigitalInput"),
        "got {:?}",
        m2[0].topic
    );
    assert!(
        m3[0].topic.contains("DigitalInput"),
        "got {:?}",
        m3[0].topic
    );
}

#[tokio::test]
async fn injected_fault_propagates() {
    use oxvif::OnvifError;
    use oxvif::soap::SoapError;

    let server = MockServer::start().await.unwrap();
    let s = OnvifSession::builder(server.device_url())
        .build()
        .await
        .unwrap();

    // Arm a fault for the next GetDeviceInformation call.
    server.inject_fault("GetDeviceInformation", "ter:NotAuthorized", "denied");
    let err = s.get_device_info().await.unwrap_err();
    assert!(matches!(err, OnvifError::Soap(SoapError::Fault { .. })));

    // Fault was single-shot — the next call succeeds.
    assert_eq!(
        s.get_device_info().await.unwrap().manufacturer,
        "oxvif-mock"
    );
}
