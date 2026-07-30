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
use oxvif::{
    ImagingSettings, OnvifSession, PtzPresetTour, PtzPresetTourDirection, PtzPresetTourOperation,
    PtzPresetTourPresetDetail, PtzPresetTourSpot, PtzPresetTourStartingCondition,
    PtzPresetTourState, PtzPresetTourStatus,
};

/// Assert a SOAP Fault's exact code and reason.
///
/// Per `CLAUDE.md`'s "no hollow tests": `assert!(res.is_err())` stays green when
/// the device returns a completely different error, so the negatives below pin
/// the fixture's own strings.
#[track_caller]
fn assert_fault(err: oxvif::OnvifError, code: &str, reason: &str) {
    match err {
        oxvif::OnvifError::Soap(oxvif::soap::SoapError::Fault {
            code: got_code,
            reason: got_reason,
            ..
        }) => {
            assert_eq!(got_code, code, "fault code");
            assert_eq!(got_reason, reason, "fault reason");
        }
        other => panic!("expected SoapError::Fault, got {other:?}"),
    }
}

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
    // The motorised lens. `VS_2` is fixed-focus and answers differently —
    // see tests/mock_multi_sensor.rs.
    let vsc = "VS_1";

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

    // Recording list — the mock seeds two, and they disagree: `Rec_001` carries
    // a track, `Rec_002` carries none.
    let recordings = s.get_recordings().await.unwrap();
    let tokens: Vec<&str> = recordings.iter().map(|r| r.token.as_str()).collect();
    assert_eq!(tokens, ["Rec_001", "Rec_002"]);

    // Search session returns a token.
    let token = s.find_recordings(None, "PT60S").await.unwrap();
    assert!(!token.is_empty());

    // Replay URI for a recording. The token must name a recording that exists:
    // this used to pass `"rec1"`, which matched nothing, and a token-blind
    // handler answered anyway — the same shape as the `VideoSource_1` token
    // reconciled in 0.15.
    let uri = s
        .get_replay_uri("Rec_001", "RTP-Unicast", "RTSP")
        .await
        .unwrap();
    assert!(uri.starts_with("rtsp://"));
    assert!(
        uri.ends_with("Rec_001"),
        "the URI must name the recording asked for, got {uri}"
    );
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

/// Preset tours are the first PTZ feature with real state behind it, so the
/// round trip is the test that matters: a tour created here must come back
/// from a later `GetPresetTours`, or the mock is a fixture printer rather than
/// an integration harness.
#[tokio::test]
async fn ptz_preset_tour_round_trip() {
    let (_srv, s) = setup().await;
    let profile = s.get_profiles().await.unwrap()[0].token.clone();

    // The seeded tour has two stops — one is not enough to tell a parser that
    // reads the whole list from one that returns the first.
    let seeded = s.ptz_get_preset_tours(&profile).await.unwrap();
    assert_eq!(seeded.len(), 1);
    assert_eq!(seeded[0].token.as_deref(), Some("Tour_1"));
    assert_eq!(seeded[0].tour_spots.len(), 2);

    // What the device will accept, before building anything.
    let opts = s.ptz_get_preset_tour_options(&profile, None).await.unwrap();
    assert!(opts.auto_start);
    assert_eq!(
        opts.starting_condition.directions,
        [
            PtzPresetTourDirection::Forward,
            PtzPresetTourDirection::Backward
        ]
    );
    assert!(
        opts.tour_spot
            .preset_detail
            .preset_tokens
            .contains(&"Preset_1".to_string())
    );

    // Create → modify → read back.
    let token = s.ptz_create_preset_tour(&profile).await.unwrap();
    assert_ne!(token, "Tour_1");

    let tour = PtzPresetTour {
        token: Some(token.clone()),
        name: Some("Night sweep".into()),
        status: PtzPresetTourStatus {
            state: PtzPresetTourState::Idle,
            current_tour_spot: None,
        },
        auto_start: true,
        starting_condition: PtzPresetTourStartingCondition {
            random_preset_order: Some(true),
            recurring_time: Some(2),
            recurring_duration: None,
            direction: Some(PtzPresetTourDirection::Backward),
        },
        tour_spots: vec![PtzPresetTourSpot {
            preset_detail: PtzPresetTourPresetDetail::PresetToken("Preset_2".into()),
            speed: None,
            stay_time: Some("PT15S".into()),
        }],
    };
    s.ptz_modify_preset_tour(&profile, &tour).await.unwrap();

    let stored = s.ptz_get_preset_tour(&profile, &token).await.unwrap();
    assert_eq!(stored.name.as_deref(), Some("Night sweep"));
    assert!(stored.auto_start);
    assert_eq!(stored.starting_condition.random_preset_order, Some(true));
    assert_eq!(stored.starting_condition.recurring_time, Some(2));
    assert_eq!(
        stored.starting_condition.direction,
        Some(PtzPresetTourDirection::Backward)
    );
    assert_eq!(stored.tour_spots.len(), 1);
    assert_eq!(stored.tour_spots[0].stay_time.as_deref(), Some("PT15S"));

    // Operate moves the state, and the state is observable.
    s.ptz_operate_preset_tour(&profile, &token, PtzPresetTourOperation::Start)
        .await
        .unwrap();
    let touring = s.ptz_get_preset_tour(&profile, &token).await.unwrap();
    assert_eq!(touring.status.state, PtzPresetTourState::Touring);

    s.ptz_operate_preset_tour(&profile, &token, PtzPresetTourOperation::Pause)
        .await
        .unwrap();
    let paused = s.ptz_get_preset_tour(&profile, &token).await.unwrap();
    assert_eq!(paused.status.state, PtzPresetTourState::Paused);

    // Remove, and it is gone from the list.
    s.ptz_remove_preset_tour(&profile, &token).await.unwrap();
    let after = s.ptz_get_preset_tours(&profile).await.unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].token.as_deref(), Some("Tour_1"));
}

/// The two `SendAuxiliaryCommand` operations, driven over HTTP against the
/// same mock device, proving they are distinct endpoints rather than one
/// method with two names — and that what the Device service *advertises* in
/// `Misc/@AuxiliaryCommands` is what the PTZ service actually *accepts*. That
/// link is the only reason a caller can use the PTZ operation without
/// guessing, so it is worth a test rather than only a doc comment.
#[tokio::test]
async fn auxiliary_commands_are_discoverable_and_accepted() {
    let (_srv, s) = setup().await;
    let profile = s.get_profiles().await.unwrap()[0].token.clone();

    let advertised = s
        .device_get_service_capabilities()
        .await
        .unwrap()
        .misc
        .expect("Misc")
        .auxiliary_commands;
    assert!(advertised.contains(&"tt:Wiper|On".to_string()));

    // Every advertised command is accepted by the PTZ operation.
    for cmd in &advertised {
        let answer = s.ptz_send_auxiliary_command(&profile, cmd).await.unwrap();
        assert!(answer.contains(cmd), "got {answer} for {cmd}");
    }

    // Something not on the list faults rather than silently succeeding.
    assert!(
        s.ptz_send_auxiliary_command(&profile, "tt:Sprinkler|On")
            .await
            .is_err()
    );

    // The Device operation is a different endpoint and still answers.
    assert_eq!(s.send_auxiliary_command("tt:Wiper|On").await.unwrap(), "OK");
}

/// The Profile G lifecycle end to end, which could not be exercised at all
/// before the mock grew recording state: `CreateRecording` answered `Rec_new`
/// and `GetRecordings` never listed it. `docs/active/mock-audit-2026-07.md` §4.2.
///
/// This matters beyond the mock. `HealthCheck::with_liveness_probes(true)`
/// claims to "genuinely exercise Profile G" — against a facade, its Profile G
/// verdict was measuring a door panel.
#[tokio::test]
async fn recording_lifecycle_is_observable() {
    let (_srv, s) = setup().await;

    let before = s.get_recordings().await.unwrap().len();

    let token = s
        .create_recording(&oxvif::RecordingConfiguration {
            source_name: "Loading Bay".into(),
            source_id: "urn:uuid:mock-lifecycle".into(),
            location: "Bay 4".into(),
            description: "created by mock_workflow".into(),
            content: "Motion events".into(),
            maximum_retention_time: "P7D".into(),
        })
        .await
        .unwrap();

    // The device assigned a token, and it is in the list.
    let listed = s.get_recordings().await.unwrap();
    assert_eq!(listed.len(), before + 1);
    let mine = listed
        .iter()
        .find(|r| r.token == token)
        .unwrap_or_else(|| panic!("{token} not listed: {listed:?}"));
    assert_eq!(mine.source.name, "Loading Bay");
    assert_eq!(mine.content, "Motion events");
    assert!(
        mine.tracks.is_empty(),
        "a new recording holds no tracks yet"
    );

    // Add a track, and read it back off the recording it was added to.
    let track = s.create_track(&token, "Audio", "audioTrack").await.unwrap();
    let with_track = s.get_recordings().await.unwrap();
    let mine = with_track.iter().find(|r| r.token == token).unwrap();
    assert_eq!(mine.tracks.len(), 1);
    assert_eq!(mine.tracks[0].token, track);
    assert_eq!(mine.tracks[0].track_type, "Audio");
    // …and it went on *this* recording, not on the seeded one.
    let seeded = with_track.iter().find(|r| r.token == "Rec_001").unwrap();
    assert_eq!(seeded.tracks.len(), 1);
    assert_eq!(seeded.tracks[0].token, "VIDEO001");

    // A job for it, then flip its mode and read the new mode back.
    let job = s
        .create_recording_job(&oxvif::RecordingJobConfiguration {
            recording_token: token.clone(),
            mode: "Idle".into(),
            priority: 5,
            source_token: "Profile_1".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        s.get_recording_job_state(&job).await.unwrap().active_state,
        "Idle"
    );
    s.set_recording_job_mode(&job, "Active").await.unwrap();
    assert_eq!(
        s.get_recording_job_state(&job).await.unwrap().active_state,
        "Active"
    );
    // The seeded jobs are untouched — job state is a per-token question.
    assert_eq!(
        s.get_recording_job_state("Job_002")
            .await
            .unwrap()
            .active_state,
        "Idle"
    );

    // The search surface sees it too.
    let found = s.search_recordings(None).await.unwrap();
    let mine = found
        .iter()
        .find(|r| r.recording_token == token)
        .unwrap_or_else(|| panic!("a created recording must be findable: {found:?}"));
    assert_eq!(mine.recording_status, "Initiated");
    // A brand-new recording has no time bounds yet, and the mock omits them
    // rather than inventing a range. The seeded ones do carry bounds, so this
    // distinction is observable rather than merely written down.
    assert_eq!(mine.earliest_recording, None);
    assert!(
        found
            .iter()
            .any(|r| r.recording_token == "Rec_001" && r.earliest_recording.is_some()),
        "the seeded recordings still report their bounds: {found:?}",
    );

    // Deleting the recording takes its jobs with it — a job pointing at nothing
    // is not a state a device would report.
    s.delete_recording(&token).await.unwrap();
    assert_eq!(s.get_recordings().await.unwrap().len(), before);
    assert!(
        !s.get_recording_jobs()
            .await
            .unwrap()
            .iter()
            .any(|j| j.token == job),
        "the recording's job must go with it",
    );
}

/// Tokens that name nothing are refused, rather than answered for whichever
/// fixture the handler happened to hold.
#[tokio::test]
async fn recording_unknown_tokens_are_refused() {
    let (_srv, s) = setup().await;

    let err = s.delete_recording("Rec_999").await.unwrap_err();
    assert_fault(
        err,
        "ter:NoRecording",
        "NoSuchRecording-DELREC-5701: Rec_999",
    );

    let err = s
        .get_replay_uri("Rec_999", "RTP-Unicast", "RTSP")
        .await
        .unwrap_err();
    assert_fault(
        err,
        "ter:NoRecording",
        "NoSuchRecording-REPLAY-5709: Rec_999",
    );

    let err = s.get_recording_job_state("Job_999").await.unwrap_err();
    assert_fault(err, "ter:NoJob", "NoSuchJob-JOBSTATE-5708: Job_999");

    let err = s
        .set_recording_job_mode("Job_001", "Sideways")
        .await
        .unwrap_err();
    assert_fault(
        err,
        "ter:InvalidArgVal",
        "BadJobMode-SETJOBMODE-5705: Sideways",
    );
}
