//! End-to-end proof that a per-channel query reaches the right channel.
//!
//! The mock device is a **two-sensor** camera (see the section comment above
//! `VideoEncoderState` in `src/mock/state.rs`): one ONVIF endpoint, two lenses,
//! two streams each. The two sensors deliberately disagree about what they can
//! do — sensor 1 goes to 2592x1944, sensor 2 stops at 1280x720.
//!
//! That disagreement is the whole point. CLAUDE.md's multi-sensor rule says a
//! single-sensor fixture cannot cover a per-channel feature, "because it passes
//! just as well against a parser that ignores the token entirely". These tests
//! run the **real client** against the **real mock server over HTTP**, so every
//! layer in between — client body construction, dispatch, responder — has to
//! carry the token for the assertions to hold.
//!
//! Measured against a real two-sensor device on 2026-07-28: a token-less
//! `GetVideoEncoderConfigurationOptions` returned lens 0's list, which a caller
//! would then display for lens 1 as well. Nothing in the response said which
//! lens had answered. That is the bug this file exists to make impossible.
//!
//! Run with: `cargo test --features mock-server --test mock_multi_sensor`
#![cfg(feature = "mock-server")]

use oxvif::OnvifSession;
use oxvif::error::OnvifError;
use oxvif::mock::MockServer;
use oxvif::soap::SoapError;

async fn setup() -> (MockServer, OnvifSession) {
    let server = MockServer::start().await.expect("start mock server");
    let session = OnvifSession::builder(server.device_url())
        .build()
        .await
        .expect("build session");
    (server, session)
}

#[track_caller]
fn assert_fault(err: OnvifError, code: &str, reason: &str) {
    match err {
        OnvifError::Soap(SoapError::Fault {
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

/// The device presents two sensors, not one.
#[tokio::test]
async fn device_exposes_two_video_sources() {
    let (_srv, s) = setup().await;

    let sources = s.get_video_sources().await.unwrap();
    let tokens: Vec<&str> = sources.iter().map(|v| v.token.as_str()).collect();
    assert_eq!(tokens, ["VS_1", "VS_2"]);
    // Different sensors, not the same one listed twice.
    assert_eq!(
        (sources[0].resolution.width, sources[0].resolution.height),
        (2592, 1944)
    );
    assert_eq!(
        (sources[1].resolution.width, sources[1].resolution.height),
        (1280, 720)
    );
}

/// Four profiles, two per sensor — so a client that walks profiles reaches
/// both lenses without having to guess configuration tokens.
#[tokio::test]
async fn profiles_cover_both_sensors() {
    let (_srv, s) = setup().await;

    let profiles = s.get_profiles().await.unwrap();
    assert_eq!(profiles.len(), 4);

    // `video_source_token` is the `<SourceToken>` *inside* the source config —
    // i.e. which physical lens the profile is looking through.
    let lens_of = |p: &oxvif::MediaProfile| p.video_source_token.clone();
    assert_eq!(lens_of(&profiles[0]).as_deref(), Some("VS_1"));
    assert_eq!(lens_of(&profiles[1]).as_deref(), Some("VS_1"));
    assert_eq!(lens_of(&profiles[2]).as_deref(), Some("VS_2"));
    assert_eq!(lens_of(&profiles[3]).as_deref(), Some("VS_2"));
}

/// **The regression test.** Same operation, two channels, two answers.
///
/// Before 0.15 the responder took no arguments, so both halves of this returned
/// sensor 1's list and the inequality below could not have failed.
#[tokio::test]
async fn video_encoder_options_answer_for_the_channel_asked_about() {
    let (_srv, s) = setup().await;

    let lens1 = s
        .get_video_encoder_configuration_options("VEC_1")
        .await
        .unwrap();
    let lens2 = s
        .get_video_encoder_configuration_options("VEC_3")
        .await
        .unwrap();

    let max_w = |o: &oxvif::VideoEncoderConfigurationOptions| {
        o.h264
            .as_ref()
            .expect("H264 options")
            .resolutions
            .iter()
            .map(|r| r.width)
            .max()
            .expect("at least one resolution")
    };

    assert_eq!(max_w(&lens1), 2592, "sensor 1 is 5MP");
    assert_eq!(max_w(&lens2), 1280, "sensor 2 stops at 720p");
    // Stated as the inequality too: if the token is ever dropped again these
    // become equal, and this line says why that is wrong in one place.
    assert!(
        max_w(&lens1) > max_w(&lens2),
        "the two lenses must not report the same ceiling"
    );
}

/// The bounds a device reports are the addressed *sensor's*, so this is
/// per-channel for the same reason.
#[tokio::test]
async fn video_source_options_answer_for_the_channel_asked_about() {
    let (_srv, s) = setup().await;

    let lens1 = s
        .get_video_source_configuration_options("VSC_1")
        .await
        .unwrap();
    let lens2 = s
        .get_video_source_configuration_options("VSC_2")
        .await
        .unwrap();

    let max_w = |o: &oxvif::VideoSourceConfigurationOptions| {
        o.bounds_range
            .as_ref()
            .expect("BoundsRange")
            .width_range
            .max
    };
    assert_eq!(max_w(&lens1), 2592);
    assert_eq!(max_w(&lens2), 1280);
    assert_eq!(lens1.source_tokens, ["VS_1"]);
    assert_eq!(lens2.source_tokens, ["VS_2"]);
}

/// Media2 goes further: *which codecs exist* is per-channel too. Only the 5MP
/// sensor advertises H.265, so "this device supports H265" is not a
/// device-wide fact and a caller that treats it as one is wrong.
#[tokio::test]
async fn media2_encoder_options_offer_h265_on_one_sensor_only() {
    let (_srv, s) = setup().await;

    let lens1 = s
        .get_video_encoder_configuration_options_media2("VEC_1")
        .await
        .unwrap();
    let lens2 = s
        .get_video_encoder_configuration_options_media2("VEC_3")
        .await
        .unwrap();

    let encodings = |o: &oxvif::VideoEncoderConfigurationOptions2| {
        o.options
            .iter()
            .map(|x| x.encoding.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(encodings(&lens1), ["H264", "H265"]);
    assert_eq!(encodings(&lens2), ["H264"]);
}

/// Every configuration must run a resolution it also offers — checked through
/// the client rather than against the state struct, so it covers the render and
/// parse round-trip as well as the fixture.
#[tokio::test]
async fn every_channel_runs_a_resolution_it_advertises() {
    let (_srv, s) = setup().await;

    let configs = s.get_video_encoder_configurations().await.unwrap();
    assert_eq!(configs.len(), 4, "four channels");

    for c in &configs {
        let opts = s
            .get_video_encoder_configuration_options(&c.token)
            .await
            .unwrap();
        let Some(h264) = opts.h264.as_ref() else {
            continue; // JPEG channel: no H264 block to check against.
        };
        let current = (c.resolution.width, c.resolution.height);
        assert!(
            h264.resolutions
                .iter()
                .any(|r| (r.width, r.height) == current),
            "{} runs {}x{} but its own options do not list it: {:?}",
            c.token,
            current.0,
            current.1,
            h264.resolutions
                .iter()
                .map(|r| (r.width, r.height))
                .collect::<Vec<_>>()
        );
    }
}

// ── Negatives ────────────────────────────────────────────────────────────────

/// A token from the *other* service's namespace, or a typo, must be refused
/// rather than silently answered for some default channel.
#[tokio::test]
async fn video_encoder_options_unknown_channel_is_refused() {
    let (_srv, s) = setup().await;

    let err = s
        .get_video_encoder_configuration_options("VEC_99")
        .await
        .unwrap_err();
    assert_fault(err, "env:Sender", "NoSuchConfig-VECOPT-5508: VEC_99");
}

#[tokio::test]
async fn video_source_options_unknown_channel_is_refused() {
    let (_srv, s) = setup().await;

    let err = s
        .get_video_source_configuration_options("VSC_9")
        .await
        .unwrap_err();
    assert_fault(err, "env:Sender", "NoSuchConfig-VSCOPT-5504: VSC_9");
}

#[tokio::test]
async fn media2_encoder_options_unknown_channel_is_refused() {
    let (_srv, s) = setup().await;

    let err = s
        .get_video_encoder_configuration_options_media2("VEC_42")
        .await
        .unwrap_err();
    assert_fault(err, "env:Sender", "NoSuchConfig-VECOPT2-5514: VEC_42");
}

/// An empty token is not a way to ask token-lessly. The client sends it, the
/// device treats it as absent, and the caller is told so.
#[tokio::test]
async fn empty_channel_token_is_refused_as_missing() {
    let (_srv, s) = setup().await;

    let err = s
        .get_video_encoder_configuration_options("")
        .await
        .unwrap_err();
    assert_fault(err, "env:Sender", "NoConfigToken-VECOPT-5507");
}

// ── Imaging: every operation is per-VideoSourceToken ─────────────────────────
//
// The client has always sent `VideoSourceToken` on all seven imaging methods —
// it is a required `&str`, not an `Option`. The mock was the half that ignored
// it, so every imaging test in the tree passed against a device that answered
// for one lens no matter which was asked about.
//
// The two lenses differ three ways: current values, level scale (0–100 vs
// 0–255), and focus support. `VS_2` is fixed-focus, which is the common real
// pairing and the only way to express "*this channel* has no focus" as opposed
// to "this device has none".

#[tokio::test]
async fn imaging_settings_answer_for_the_lens_asked_about() {
    let (_srv, s) = setup().await;

    let one = s.get_imaging_settings("VS_1").await.unwrap();
    let two = s.get_imaging_settings("VS_2").await.unwrap();

    assert_eq!(one.brightness, Some(60.0));
    assert_eq!(two.brightness, Some(45.0));
    assert_eq!(one.ir_cut_filter.as_deref(), Some("AUTO"));
    assert_eq!(two.ir_cut_filter.as_deref(), Some("ON"));
    // The fixed lens reports no auto-focus mode at all — `None`, not a string
    // the caller has to know to disbelieve.
    assert_eq!(one.focus_mode.as_deref(), Some("AUTO"));
    assert_eq!(two.focus_mode, None);
}

#[tokio::test]
async fn imaging_options_answer_for_the_lens_asked_about() {
    let (_srv, s) = setup().await;

    let one = s.get_imaging_options("VS_1").await.unwrap();
    let two = s.get_imaging_options("VS_2").await.unwrap();

    assert_eq!(one.brightness.as_ref().map(|r| r.max), Some(100.0));
    assert_eq!(two.brightness.as_ref().map(|r| r.max), Some(255.0));
    // Auto-focus modes are a per-channel capability, not a device one.
    assert_eq!(one.focus_af_modes, ["AUTO", "MANUAL"]);
    assert!(two.focus_af_modes.is_empty());
}

#[tokio::test]
async fn imaging_status_reports_focus_only_on_the_focusable_lens() {
    let (_srv, s) = setup().await;

    let one = s.imaging_get_status("VS_1").await.unwrap();
    assert_eq!(one.focus_position, Some(0.5));

    // An empty `Status` is a legal response — `FocusStatus20` is [0..1] and it
    // is the type's only content. This must parse, not error.
    let two = s.imaging_get_status("VS_2").await.unwrap();
    assert_eq!(two.focus_position, None);
}

#[tokio::test]
async fn setting_one_lens_does_not_move_the_other() {
    let (_srv, s) = setup().await;

    let mut settings = s.get_imaging_settings("VS_2").await.unwrap();
    settings.brightness = Some(7.0);
    s.set_imaging_settings("VS_2", &settings).await.unwrap();

    assert_eq!(
        s.get_imaging_settings("VS_2").await.unwrap().brightness,
        Some(7.0)
    );
    assert_eq!(
        s.get_imaging_settings("VS_1").await.unwrap().brightness,
        Some(60.0),
        "writing VS_2 must not disturb VS_1"
    );
}

// ── Imaging negatives ────────────────────────────────────────────────────────

/// A lens with no focus motor refuses the focus operations rather than
/// reporting a range it cannot honour.
#[tokio::test]
async fn focus_operations_are_refused_on_the_fixed_lens() {
    let (_srv, s) = setup().await;

    // The focusable lens answers.
    s.imaging_get_move_options("VS_1").await.unwrap();

    let err = s.imaging_get_move_options("VS_2").await.unwrap_err();
    assert_fault(err, "env:Sender", "NoFocusSupport-IMGMOVEOPT-5611: VS_2");

    let err = s
        .imaging_move("VS_2", &oxvif::FocusMove::Continuous { speed: 0.5 })
        .await
        .unwrap_err();
    assert_fault(err, "env:Sender", "NoFocusSupport-IMGMOVE-5614: VS_2");

    let err = s.imaging_stop("VS_2").await.unwrap_err();
    assert_fault(err, "env:Sender", "NoFocusSupport-IMGSTOP-5617: VS_2");
}

#[tokio::test]
async fn imaging_settings_unknown_lens_is_refused() {
    let (_srv, s) = setup().await;

    // `VideoSource_1` is the token the mock's own tests used to pass — it never
    // matched anything, and a token-blind device answered anyway.
    let err = s.get_imaging_settings("VideoSource_1").await.unwrap_err();
    assert_fault(
        err,
        "env:Sender",
        "NoSuchVideoSource-IMGSET-5602: VideoSource_1",
    );
}

#[tokio::test]
async fn imaging_options_unknown_lens_is_refused() {
    let (_srv, s) = setup().await;

    let err = s.get_imaging_options("VS_7").await.unwrap_err();
    assert_fault(err, "env:Sender", "NoSuchVideoSource-IMGOPT-5606: VS_7");
}

#[tokio::test]
async fn empty_lens_token_is_refused_as_missing() {
    let (_srv, s) = setup().await;

    let err = s.get_imaging_settings("").await.unwrap_err();
    assert_fault(err, "env:Sender", "NoVideoSourceToken-IMGSET-5601");
}

// ── PTZ per-profile ──────────────────────────────────────────────────────────
//
// A dual-head camera has one PTZ endpoint and several profiles, and *every*
// operation that moves or reads a head takes a `ProfileToken`. Until 0.15 the
// mock held one position and one preset list for the whole device, and 26 of
// its 27 PTZ dispatch arms did not even receive the request body — so a test
// asserting "my code addressed the right head" passed against a mock that could
// not tell one head from another. `docs/active/mock-audit-2026-07.md` §4.1.
//
// The four seeded heads deliberately disagree on position, on preset count, on
// preset names, and on whether they have tours at all.

/// The measured symptom, inverted into an assertion: this returned the same pan
/// for both profiles before the state was keyed by profile token.
#[tokio::test]
async fn ptz_status_answers_for_the_head_asked_about() {
    let (_srv, s) = setup().await;

    let one = s.ptz_get_status("Profile_1").await.unwrap();
    let three = s.ptz_get_status("Profile_3").await.unwrap();

    assert_eq!(
        (one.pan, one.tilt, one.zoom),
        (Some(0.0), Some(0.0), Some(0.0))
    );
    assert_eq!(
        (three.pan, three.tilt, three.zoom),
        (Some(-0.6), Some(0.35), Some(0.8)),
        "Profile_3 is a different head and is parked somewhere else",
    );
}

#[tokio::test]
async fn ptz_presets_answer_for_the_head_asked_about() {
    let (_srv, s) = setup().await;

    let one = s.ptz_get_presets("Profile_1").await.unwrap();
    let three = s.ptz_get_presets("Profile_3").await.unwrap();
    let four = s.ptz_get_presets("Profile_4").await.unwrap();

    // Counts differ, so a token-blind handler cannot satisfy all three...
    assert_eq!(one.len(), 2);
    assert_eq!(three.len(), 3);
    assert_eq!(four.len(), 0, "an empty preset list is a legitimate answer");

    // ...and so do the names, so the assertion is not count-only.
    let names = |v: &[oxvif::PtzPreset]| v.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&one), ["Home", "Door"]);
    assert_eq!(names(&three), ["Lobby", "Dock", "Roof"]);
}

#[tokio::test]
async fn moving_one_head_does_not_move_the_other() {
    let (_srv, s) = setup().await;

    let before = s.ptz_get_status("Profile_3").await.unwrap();
    s.ptz_absolute_move("Profile_1", 0.9, -0.8, 0.1)
        .await
        .unwrap();

    let after = s.ptz_get_status("Profile_3").await.unwrap();
    assert_eq!(
        (after.pan, after.tilt, after.zoom),
        (before.pan, before.tilt, before.zoom),
        "moving Profile_1 must not move Profile_3",
    );
    assert_eq!(
        s.ptz_get_status("Profile_1").await.unwrap().pan,
        Some(0.9),
        "...and the head that was asked to move must have moved",
    );
}

#[tokio::test]
async fn a_preset_stored_on_one_head_is_not_visible_on_another() {
    let (_srv, s) = setup().await;

    let token = s
        .ptz_set_preset("Profile_2", Some("Loading Bay"), None)
        .await
        .unwrap();

    let two = s.ptz_get_presets("Profile_2").await.unwrap();
    assert!(
        two.iter()
            .any(|p| p.token == token && p.name == "Loading Bay")
    );

    let one = s.ptz_get_presets("Profile_1").await.unwrap();
    assert!(
        !one.iter().any(|p| p.name == "Loading Bay"),
        "Profile_1 has its own preset list: {one:?}",
    );
}

/// Home position is per-head too, and this is the pair that would silently pass
/// against a global one: store home on Profile_2, then send Profile_1 home.
#[tokio::test]
async fn home_position_is_per_head() {
    let (_srv, s) = setup().await;

    s.ptz_absolute_move("Profile_2", 0.7, 0.6, 0.5)
        .await
        .unwrap();
    s.ptz_set_home_position("Profile_2").await.unwrap();

    // Profile_1 has never had a home set, so it goes to the origin, not to
    // Profile_2's stored position.
    s.ptz_absolute_move("Profile_1", -0.3, -0.2, 0.9)
        .await
        .unwrap();
    s.ptz_goto_home_position("Profile_1", None).await.unwrap();

    let one = s.ptz_get_status("Profile_1").await.unwrap();
    assert_eq!(
        (one.pan, one.tilt, one.zoom),
        (Some(0.0), Some(0.0), Some(0.0)),
        "Profile_1 must not inherit Profile_2's home",
    );

    s.ptz_absolute_move("Profile_2", 0.0, 0.0, 0.0)
        .await
        .unwrap();
    s.ptz_goto_home_position("Profile_2", None).await.unwrap();
    let two = s.ptz_get_status("Profile_2").await.unwrap();
    assert_eq!(
        (two.pan, two.tilt, two.zoom),
        (Some(0.7), Some(0.6), Some(0.5))
    );
}

#[tokio::test]
async fn preset_tours_are_per_head() {
    let (_srv, s) = setup().await;

    assert_eq!(s.ptz_get_preset_tours("Profile_1").await.unwrap().len(), 1);
    assert_eq!(
        s.ptz_get_preset_tours("Profile_2").await.unwrap().len(),
        0,
        "Profile_2 ships no tours",
    );

    let created = s.ptz_create_preset_tour("Profile_2").await.unwrap();
    assert_eq!(s.ptz_get_preset_tours("Profile_2").await.unwrap().len(), 1);
    assert_eq!(
        s.ptz_get_preset_tours("Profile_1").await.unwrap().len(),
        1,
        "creating a tour on Profile_2 must not appear on Profile_1",
    );
    // Tour tokens are numbered per head, so both heads now hold a `Tour_1` and
    // they are different tours.
    assert_eq!(created, "Tour_1");
}

/// `GetPresetTourOptions` lists the presets a tour can visit — the *addressed*
/// head's presets, not the device's.
#[tokio::test]
async fn preset_tour_options_list_the_addressed_heads_presets() {
    let (_srv, s) = setup().await;

    let one = s
        .ptz_get_preset_tour_options("Profile_1", None)
        .await
        .unwrap();
    let three = s
        .ptz_get_preset_tour_options("Profile_3", None)
        .await
        .unwrap();

    assert_eq!(
        one.tour_spot.preset_detail.preset_tokens.len(),
        2,
        "got {one:?}"
    );
    assert_eq!(
        three.tour_spot.preset_detail.preset_tokens.len(),
        3,
        "got {three:?}"
    );
}

// ── PTZ negatives ────────────────────────────────────────────────────────────

#[tokio::test]
async fn ptz_unknown_profile_is_refused() {
    let (_srv, s) = setup().await;

    let err = s.ptz_get_status("Profile_9").await.unwrap_err();
    assert_fault(err, "ter:NoProfile", "NoSuchProfile-STATUS-5601: Profile_9");

    let err = s.ptz_get_presets("Profile_9").await.unwrap_err();
    assert_fault(
        err,
        "ter:NoProfile",
        "NoSuchProfile-PRESETS-5602: Profile_9",
    );

    let err = s
        .ptz_absolute_move("Profile_9", 0.0, 0.0, 0.0)
        .await
        .unwrap_err();
    assert_fault(
        err,
        "ter:NoProfile",
        "NoSuchProfile-ABSMOVE-5606: Profile_9",
    );
}

/// An empty token is refused as *missing*, not as unknown — the two are
/// different mistakes and a caller should be able to tell them apart.
#[tokio::test]
async fn ptz_empty_profile_token_is_refused_as_missing() {
    let (_srv, s) = setup().await;

    let err = s.ptz_get_status("").await.unwrap_err();
    assert_fault(
        err,
        "env:Sender",
        "NoProfileToken-STATUS-5601: every PTZ operation is per-profile",
    );
}
