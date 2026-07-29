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
