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

/// `GovLength` and `Profile` are **attributes** of `tt:VideoEncoder2Configuration`,
/// and both sides have to agree about that.
///
/// Same shape as [`imaging_move_options_ranges_survive_the_round_trip`]: it
/// asserts the values the mock actually emits, so it reddens whether the client
/// drifts off the schema or the mock does. Until 0.15 both rendered and parsed
/// the two names as child elements, and every test in the repository agreed with
/// them — `VideoEncoderConfiguration2::from_xml` used `xml_u32` / `xml_str`, the
/// mock's `render_video_encoder` wrote `<tt:GovLength>`, and against a real
/// camera both fields came back `None`.
///
/// The two channels read here disagree on **both** values (`VEC_1` is 25/`Main`,
/// `VEC_3` is 50/`High`), so this cannot pass against a renderer that emits one
/// channel's answer for every token, nor against a parser that returns a
/// constant.
#[tokio::test]
async fn media2_encoder_gov_length_and_profile_are_attributes() {
    let (_srv, s) = setup().await;

    let cfgs = s.get_video_encoder_configurations_media2().await.unwrap();
    let by = |t: &str| {
        cfgs.iter()
            .find(|c| c.token == t)
            .unwrap_or_else(|| panic!("mock seeds {t}"))
    };

    let one = by("VEC_1");
    assert_eq!(one.gov_length, Some(25), "VEC_1 GovLength");
    assert_eq!(one.profile.as_deref(), Some("Main"), "VEC_1 Profile");

    let three = by("VEC_3");
    assert_eq!(three.gov_length, Some(50), "VEC_3 GovLength");
    assert_eq!(three.profile.as_deref(), Some("High"), "VEC_3 Profile");

    // The same type is inlined into `tr2:ConfigurationSet`, rendered by the same
    // helper. A profile's copy must carry the attributes too — that second path
    // is the one the schema-shape checker counted separately.
    let profiles = s.get_profiles_media2().await.unwrap();
    let bound = profiles
        .iter()
        .find(|p| p.video_encoder_token.as_deref() == Some("VEC_1"))
        .expect("a seeded profile binds VEC_1");
    assert_eq!(bound.token, "Profile_1");

    // Write both attributes through the client and read them back. `Baseline`
    // and 90 are neither channel's factory value, so a mock that ignored the
    // request body could not produce them.
    let mut cfg = one.clone();
    cfg.gov_length = Some(90);
    cfg.profile = Some("Baseline".into());
    s.set_video_encoder_configuration_media2(&cfg)
        .await
        .unwrap();

    let after = s.get_video_encoder_configurations_media2().await.unwrap();
    let one = after.iter().find(|c| c.token == "VEC_1").unwrap();
    assert_eq!(one.gov_length, Some(90), "GovLength after Set");
    assert_eq!(
        one.profile.as_deref(),
        Some("Baseline"),
        "Profile after Set"
    );

    // And the sibling channel is untouched, so the write went to the token it
    // named rather than to whatever the mock happened to render first.
    let three = after.iter().find(|c| c.token == "VEC_3").unwrap();
    assert_eq!(three.gov_length, Some(50));
    assert_eq!(three.profile.as_deref(), Some("High"));
}

/// `tt:VideoEncoder2ConfigurationOptions` declares only `Encoding`,
/// `QualityRange`, `ResolutionsAvailable` and `BitrateRange` as child elements.
/// `GovLengthRange`, `FrameRatesSupported` and `ProfilesSupported` are
/// `xs:attribute`s, and the last two are `xs:list`-typed — one attribute holds
/// the whole space-separated collection, so this is a change of cardinality and
/// not only of location.
///
/// The checker in `tests/mock_schema_shape.rs` can see almost none of this:
/// the type carries an `xs:any`, which suppresses its `UNKNOWN-CHILD` rule for
/// the whole type, and `GovLengthRange` / `FrameRateRange` are real `tt:`
/// element names on the *Media1* options types, so `UNKNOWN-NAME` cannot fire
/// for them either. Only `ProfilesSupported` was ever visible. **This test is
/// what asserts the rest**, by reading the values back through the client.
///
/// The two sensors disagree on all three, so an answer for the wrong channel is
/// as red as an unread attribute.
#[tokio::test]
async fn media2_encoder_options_lists_are_attributes() {
    let (_srv, s) = setup().await;

    let by = |o: &oxvif::VideoEncoderConfigurationOptions2, enc: &str| {
        o.options
            .iter()
            .find(|x| x.encoding.as_str() == enc)
            .unwrap_or_else(|| panic!("mock offers {enc}"))
            .clone()
    };

    // Sensor 1, the 5MP lens.
    let lens1 = s
        .get_video_encoder_configuration_options_media2("VEC_1")
        .await
        .unwrap();

    let h264 = by(&lens1, "H264");
    let gov = h264.gov_length_range.expect("GovLengthRange attribute");
    assert_eq!((gov.min, gov.max), (1, 300), "VEC_1 H264 GovLengthRange");
    assert_eq!(
        h264.profiles,
        ["Baseline", "Main", "High"],
        "one attribute, three profiles"
    );
    // 12.5 fps is why this list is `f32`: an integer parse drops it and the
    // length goes to three.
    assert_eq!(h264.frame_rates.len(), 4, "VEC_1 H264 FrameRatesSupported");
    assert!((h264.frame_rates[0] - 30.0).abs() < 1e-5);
    assert!((h264.frame_rates[3] - 12.5).abs() < 1e-5, "fractional rate");

    let h265 = by(&lens1, "H265");
    let gov = h265.gov_length_range.expect("GovLengthRange attribute");
    assert_eq!((gov.min, gov.max), (1, 600), "VEC_1 H265 GovLengthRange");
    assert_eq!(h265.profiles, ["Main", "Main10"]);
    assert_eq!(h265.frame_rates.len(), 2);

    // Sensor 2, the 720p lens — every list is a different one.
    let lens2 = s
        .get_video_encoder_configuration_options_media2("VEC_3")
        .await
        .unwrap();

    let h264_2 = by(&lens2, "H264");
    let gov2 = h264_2.gov_length_range.expect("GovLengthRange attribute");
    assert_eq!((gov2.min, gov2.max), (2, 150), "VEC_3 H264 GovLengthRange");
    assert_eq!(h264_2.profiles, ["Baseline", "Main"]);
    assert_eq!(h264_2.frame_rates.len(), 2);

    // Stated as the inequality too, so dropping the token cannot leave this
    // green by handing both callers the same answer.
    assert_ne!(gov.max, gov2.max);
    assert_ne!(h264.profiles, h264_2.profiles);
    assert_ne!(h264.frame_rates.len(), h264_2.frame_rates.len());
}

/// `MaximumNumberOfProfiles` is an `xs:attribute` of
/// `tt:VideoSourceConfigurationOptions`, whose only child elements are
/// `BoundsRange`, `VideoSourceTokensAvailable` and `Extension`.
///
/// Media1 and Media2 return the *same* type here, so both are asserted — a fix
/// applied to one renderer and not the other is the divergence `CLAUDE.md`
/// step 5b exists for.
#[tokio::test]
async fn video_source_options_max_profiles_is_an_attribute() {
    let (_srv, s) = setup().await;

    let m1 = s
        .get_video_source_configuration_options("VSC_1")
        .await
        .unwrap();
    assert_eq!(m1.max_limit, Some(5), "Media1 MaximumNumberOfProfiles");
    // The sibling elements still parse, so the assertion above is about the
    // attribute and not about the whole `Options` block being missed.
    assert_eq!(m1.source_tokens, ["VS_1"]);
    assert_eq!(m1.bounds_range.expect("BoundsRange").width_range.max, 2592);

    let m2 = s
        .get_video_source_configuration_options_media2("VSC_2")
        .await
        .unwrap();
    assert_eq!(m2.max_limit, Some(5), "Media2 MaximumNumberOfProfiles");
    assert_eq!(m2.source_tokens, ["VS_2"]);
    assert_eq!(m2.bounds_range.expect("BoundsRange").width_range.max, 1280);
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

/// `PTZStatus/UtcTime` must be the *current* date.
///
/// It was the literal `2026-04-23T00:00:00Z` until 0.15 — the second hardcoded
/// clock in the mock, after the `2026-04-15` in `GetSystemDateAndTime`. A frozen
/// timestamp is invisible to every other test here: nothing else reads the
/// field, and it stays syntactically valid forever while drifting a day further
/// into the past each day.
///
/// The date is computed independently on this side from `SystemTime`, so the
/// assertion cannot be satisfied by any constant. Both the before- and after-call
/// dates are accepted, which is only a real set of two across a midnight
/// rollover.
#[tokio::test]
async fn ptz_status_utc_time_is_the_real_clock() {
    use oxvif::soap::security::unix_secs_to_iso8601;

    fn today() -> String {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        unix_secs_to_iso8601(secs as i64)[..10].to_string()
    }

    let (_srv, s) = setup().await;
    let profile = s.get_profiles().await.unwrap()[0].token.clone();

    let before = today();
    let status = s.ptz_get_status(&profile).await.unwrap();
    let after = today();

    let utc = status.utc_time.expect("mock must report PTZStatus/UtcTime");
    let date = &utc[..10];
    assert!(
        date == before || date == after,
        "PTZStatus/UtcTime {utc} is not today ({before}) — the mock's PTZ clock is frozen again"
    );
    assert!(
        utc.ends_with('Z') && utc.len() == 20,
        "PTZStatus/UtcTime {utc} is not an ISO-8601 UTC instant"
    );
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

/// The client's focus-range parser and the mock's focus-range renderer must
/// agree on element names.
///
/// **Nothing in this repository connected them until this test.** Through 0.14
/// both sides said `PositionSpace` / `SpeedSpace` — names borrowed from PTZ and
/// declared nowhere for focus — so every range was `None` against a real
/// camera. `455c40a` corrected the mock and the client parser stayed wrong, and
/// still no test went red: the only mock-driven call was
/// `s.imaging_get_move_options("VS_1").await.unwrap()` in
/// `tests/mock_multi_sensor.rs`, a hollow positive that asserts no field, and
/// the unit fixture in `src/tests/client/imaging_tests.rs` had been written to
/// agree with the parser rather than the schema.
///
/// So this asserts the *values* the mock emits, not that the call succeeded.
/// Either side drifting off the schema names reddens it.
#[tokio::test]
async fn imaging_move_options_ranges_survive_the_round_trip() {
    let (_srv, s) = setup().await;

    let opts = s.imaging_get_move_options("VS_1").await.unwrap();

    let pos = opts
        .absolute_position_range
        .expect("mock emits tt:Absolute/tt:Position");
    assert_eq!((pos.min, pos.max), (0.0, 1.0));

    let abs_speed = opts
        .absolute_speed_range
        .expect("mock emits tt:Absolute/tt:Speed");
    assert_eq!((abs_speed.min, abs_speed.max), (0.0, 1.0));

    let cont = opts
        .continuous_speed_range
        .expect("mock emits tt:Continuous/tt:Speed");
    assert_eq!((cont.min, cont.max), (-1.0, 1.0));

    // The mock models absolute and continuous focus and omits `Relative`
    // entirely — a legal shape, since all three families are [0..1]. `None`
    // here is the mock's actual answer, not a parse failure.
    assert!(opts.relative_distance_range.is_none());
    assert!(opts.relative_speed_range.is_none());
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

/// `GetRecordings` reports every member of `tt:RecordingSourceInformation`.
///
/// All five are `minOccurs=1`. The mock rendered four of them and had no field
/// at all for `Address`, so `CreateRecording` accepted one and discarded it —
/// the `SetNetworkInterfaces`/`MTU` shape. Nothing failed, because
/// `RecordingSourceInformation::address` is `Option` and `None` reads as "the
/// device did not say".
///
/// The two seeded recordings **disagree on all five**, so a renderer that
/// answers from a constant, or from the first entry for both, goes red here.
/// `Rec_002`'s empty address is the case that keeps `None` observable: the
/// element is required, so it goes out empty rather than being dropped.
#[tokio::test]
async fn recording_source_information_is_complete_and_per_recording() {
    let (_srv, s) = setup().await;

    let recs = s.get_recordings().await.unwrap();
    let one = recs.iter().find(|r| r.token == "Rec_001").expect("Rec_001");
    assert_eq!(one.source.source_id, "rtsp://mock/live");
    assert_eq!(one.source.name, "MockCamera");
    assert_eq!(one.source.location, "Lab");
    assert_eq!(one.source.description, "Mock recording");
    assert_eq!(
        one.source.address.as_deref(),
        Some("http://192.168.1.100/onvif/device_service")
    );

    let two = recs.iter().find(|r| r.token == "Rec_002").expect("Rec_002");
    assert_eq!(two.source.source_id, "");
    assert_eq!(two.source.location, "");
    assert_eq!(
        two.source.address, None,
        "an entry with no address sends the required element empty, and empty \
         must still read as None"
    );
}

/// `SystemLogUris` holds repeated **`SystemLog`**, not `SystemLogUri`.
///
/// `SystemLogUri` is the *type*. Reading the type name as the element name left
/// `system_log_uri` `None` against every conformant device, and the mock had
/// been written to agree with the parser — so mock, unit fixture and client all
/// agreed with each other and with nothing else. This drives the real chain.
#[tokio::test]
async fn system_log_uri_is_reported() {
    let (_srv, s) = setup().await;

    let uris = s.get_system_uris().await.unwrap();
    let log = uris.system_log_uri.expect("SystemLogUris/SystemLog/Uri");
    assert!(log.ends_with("/syslog"), "got {log}");
    // The two siblings still parse, so the assertion above is about the log
    // entry and not about the whole response being missed.
    assert!(
        uris.support_info_uri
            .expect("SupportInfoUri")
            .ends_with("/support")
    );
    assert!(
        uris.system_backup_uri
            .expect("SystemBackupUri")
            .ends_with("/backup")
    );
}

/// `tr2:EncoderInstanceInfo` groups instances under **`Codec`**, with `Encoding`
/// one level inside it.
///
/// The parser iterated `children_named("Encoding")`, which matched the wrapper
/// only because the mock had *named* the wrapper `Encoding`. Against a real
/// device `encodings` was empty while `total` still parsed — a half-populated
/// struct, which is harder to notice than an error.
#[tokio::test]
async fn media2_encoder_instances_are_grouped_by_codec() {
    let (_srv, s) = setup().await;

    let inst = s.get_video_encoder_instances_media2("VSC_1").await.unwrap();
    assert_eq!(inst.total, 4);
    assert_eq!(
        inst.encodings.len(),
        2,
        "the two Codec entries must both be seen"
    );
    assert_eq!(inst.encodings[0].encoding, oxvif::VideoEncoding::H264);
    assert_eq!(inst.encodings[1].encoding, oxvif::VideoEncoding::H265);
    assert_eq!(inst.encodings[0].number + inst.encodings[1].number, 4);
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

/// The three seeded storage entries **disagree on every optional field**, and
/// this asserts the disagreement rather than the presence of a list.
///
/// Before 0.15 `GetStorageConfigurations` was one static entry carrying a
/// `LocalPath` and nothing else, so `storage_uri` and `user` — both parsed by
/// `StorageConfiguration` — were never fed by the mock at all (audit §6, the
/// "storage credential fields"). An assertion that only counted entries, or
/// only read `local_path`, would have passed against that fixture too.
#[tokio::test]
async fn storage_entries_differ_on_every_optional_field() {
    let (_srv, s) = setup().await;
    let got = s.get_storage_configurations().await.unwrap();

    let by = |t: &str| {
        got.iter()
            .find(|e| e.token == t)
            .unwrap_or_else(|| panic!("no storage entry {t}"))
            .clone()
    };

    // A path, no URI, no credentials.
    let sd = by("SD_01");
    assert_eq!(sd.storage_type, "LocalStorage");
    assert_eq!(sd.local_path, "/mnt/sd");
    assert_eq!(sd.storage_uri, "", "local card has no network URI");
    assert_eq!(sd.user, "", "local card has no credentials");

    // Every field populated — the only entry that proves `user` is emitted.
    let nas = by("NAS_01");
    assert_eq!(nas.storage_type, "NFS");
    assert_eq!(nas.local_path, "/mnt/nas");
    assert_eq!(nas.storage_uri, "nfs://192.168.1.50/records");
    assert_eq!(nas.user, "recorder");

    // A URI and nothing else — the entry that stops `local_path` and `user`
    // from being satisfiable by a constant, since SD_01 and NAS_01 both carry
    // a path. It does *not* pin omitted-vs-empty on the wire: the parser
    // collapses both to `""`, measured.
    let cifs = by("CIFS_01");
    assert_eq!(cifs.storage_type, "CIFS");
    assert_eq!(cifs.local_path, "");
    assert_eq!(cifs.storage_uri, "smb://192.168.1.60/cam");
    assert_eq!(cifs.user, "");
}

/// The two metadata configurations invert every boolean, and only one carries a
/// multicast address — the `Option` distinction the parser really can see,
/// unlike the Storage string fields.
///
/// `tt:MetadataConfiguration/Multicast` is required, so both configurations
/// send the block; what distinguishes them is the optional
/// `tt:IPAddress/IPv4Address` inside it.
#[tokio::test]
async fn metadata_configs_differ_on_every_field() {
    let (srv, s) = setup().await;
    let url = format!("{}/onvif/media2", srv.base_url());
    let all = s
        .client()
        .get_metadata_configurations_media2(&url, None, None)
        .await
        .unwrap();
    assert_eq!(all.len(), 2, "two seeded metadata configurations");

    let one = all.iter().find(|c| c.token == "MetaConf_1").unwrap();
    assert_eq!(one.name, "MetadataConfig");
    assert_eq!(one.use_count, 1);
    assert!(one.analytics);
    assert!(!one.ptz_status);
    assert!(one.ptz_position);
    assert_eq!(one.multicast_address.as_deref(), Some("239.0.1.10"));
    assert_eq!(one.multicast_port, Some(40010));

    let two = all.iter().find(|c| c.token == "MetaConf_2").unwrap();
    assert_eq!(two.name, "MetadataMinimal");
    assert_eq!(two.use_count, 0);
    assert!(!two.analytics);
    assert!(two.ptz_status);
    assert!(!two.ptz_position);
    assert_eq!(
        two.multicast_address, None,
        "a config with no group must omit IPv4Address, not send it empty"
    );
    assert_eq!(
        two.multicast_port,
        Some(0),
        "Multicast/Port is required, so it is 0 rather than absent"
    );

    // The options getter answers for the addressed configuration, and the two
    // members `tt:PTZStatusFilterOptions` requires are what it discriminates
    // on. They were an `Extension/AnalyticsSupported` that ONVIF does not
    // declare until 0.15.
    let o1 = s
        .client()
        .get_metadata_configuration_options_media2(&url, Some("MetaConf_1"), None)
        .await
        .unwrap();
    let o2 = s
        .client()
        .get_metadata_configuration_options_media2(&url, Some("MetaConf_2"), None)
        .await
        .unwrap();
    assert!(o1.pan_tilt_status_supported);
    assert!(!o1.zoom_status_supported);
    assert!(!o2.pan_tilt_status_supported);
    assert!(o2.zoom_status_supported);
    assert!(o1.ptz_status_filter_supported && o2.ptz_status_filter_supported);
}

#[tokio::test]
async fn metadata_unknown_token_is_refused() {
    let (srv, s) = setup().await;
    let url = format!("{}/onvif/media2", srv.base_url());

    let mut cfg = s
        .client()
        .get_metadata_configurations_media2(&url, Some("MetaConf_1"), None)
        .await
        .unwrap()
        .remove(0);
    cfg.token = "MetaConf_99".into();
    let err = s
        .client()
        .set_metadata_configuration_media2(&url, &cfg)
        .await
        .unwrap_err();
    assert_fault(
        err,
        "ter:NoConfig",
        "NoSuchMetadataConfig-SETMETA-5811: MetaConf_99",
    );

    // The options getter is addressed, not a filter, so it faults too — and
    // with its own tag, so this assertion cannot be satisfied by the one above.
    let err = s
        .client()
        .get_metadata_configuration_options_media2(&url, Some("MetaConf_99"), None)
        .await
        .unwrap_err();
    assert_fault(
        err,
        "ter:NoConfig",
        "NoSuchMetadataConfig-METAOPT-5812: MetaConf_99",
    );

    // But `GetMetadataConfigurations` is a *filter*: an unmatched token is an
    // empty list, not an error. Asserting the difference is the point.
    let empty = s
        .client()
        .get_metadata_configurations_media2(&url, Some("MetaConf_99"), None)
        .await
        .unwrap();
    assert!(
        empty.is_empty(),
        "a filter that matches nothing returns nothing, and does not fault"
    );
}

#[tokio::test]
async fn storage_unknown_token_is_refused() {
    let (_srv, s) = setup().await;

    // A token that names no entry is refused rather than quietly created —
    // otherwise a typo is indistinguishable from a successful update.
    let err = s
        .set_storage_configuration("SD_99", "NFS", "/mnt/x", "", "")
        .await
        .unwrap_err();
    assert_fault(err, "ter:InvalidArgVal", "NoSuchStorage-STOR-5802: SD_99");

    // `Data/@type` is required by the schema; an empty one is refused too,
    // and with a *different* code than the unknown-token case so asserting
    // both proves more than asserting either.
    let err = s
        .set_storage_configuration("SD_01", "", "/mnt/x", "", "")
        .await
        .unwrap_err();
    assert_fault(
        err,
        "env:Sender",
        "NoStorageType-STOR-5801: Data/@type is required",
    );
}
