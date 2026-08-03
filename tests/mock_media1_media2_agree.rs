//! Media1 and Media2 are two views of **one** device, so they must agree about
//! what is on it.
//!
//! Reported against 0.14.0 by a C++ ONVIF test suite driving `MockServer`: the
//! harness seeded `DeviceState.profiles` with 20 entries to exercise a
//! fixed-size-array bound in production code, its DLL negotiated Media2, and got
//! **2**. Asserting `count <= 16` against 2 passes while proving nothing, so the
//! suite self-skipped rather than report false coverage.
//!
//! The cause was that Media2's profile family took no `state` at all — it
//! returned a string literal — while its video-encoder family was state-driven,
//! and while every Media1 equivalent was state-driven. Nothing compared the two
//! services, so the divergence had no watcher.
//!
//! These tests are written against the **public API only** (`oxvif::mock` plus
//! the ordinary client), over real HTTP, and are invariant to how either
//! response is rendered: they compare token *sets*, not markup.
#![cfg(feature = "mock-server")]

use std::collections::BTreeSet;

use oxvif::OnvifClient;
use oxvif::mock::state::ProfileEntry;
use oxvif::mock::{DeviceState, MockServer};

/// A device with `n` profiles, tokens `Profile_1..Profile_n`.
///
/// Deliberately more than the four the mock ships with, and more than the 16
/// the reporter's production array holds, so a responder that ignores the state
/// cannot coincidentally produce the right count.
fn state_with_profiles(n: usize) -> DeviceState {
    let mut state = DeviceState::default();
    state.profiles.profiles = (0..n)
        .map(|i| ProfileEntry {
            token: format!("Profile_{}", i + 1),
            name: format!("stream{}", i + 1),
            fixed: true,
            video_source_config_token: Some("VSC_1".into()),
            video_encoder_config_token: Some("VEC_1".into()),
            audio_source_config_token: None,
            audio_encoder_config_token: None,
            // Every seeded profile shares one PTZ configuration, so the two
            // services have something PTZ-shaped to disagree about if either
            // renderer stops reading `ProfileEntry`.
            ptz_config_token: Some("PTZConfig_1".into()),
        })
        .collect();
    state
}

async fn start(state: DeviceState) -> MockServer {
    MockServer::builder()
        .initial_state(state)
        .start()
        .await
        .expect("mock server starts")
}

/// Both services, one client, over HTTP — returns `(media1_tokens, media2_tokens)`.
async fn both_profile_token_sets(server: &MockServer) -> (BTreeSet<String>, BTreeSet<String>) {
    let client = OnvifClient::new(server.device_url());
    let media_url = format!("{}/onvif/media", server.base_url());
    let media2_url = format!("{}/onvif/media2", server.base_url());

    let m1 = client
        .get_profiles(&media_url)
        .await
        .expect("Media1 GetProfiles")
        .into_iter()
        .map(|p| p.token)
        .collect();
    let m2 = client
        .get_profiles_media2(&media2_url)
        .await
        .expect("Media2 GetProfiles")
        .into_iter()
        .map(|p| p.token)
        .collect();
    (m1, m2)
}

/// The reported case, verbatim: seed 20, ask both.
#[tokio::test]
async fn both_services_report_the_same_profiles_for_a_seeded_device() {
    let server = start(state_with_profiles(20)).await;
    let (m1, m2) = both_profile_token_sets(&server).await;

    assert_eq!(
        m1.len(),
        20,
        "Media1 should serve the seeded profiles, got {m1:?}"
    );
    assert_eq!(
        m2, m1,
        "Media1 and Media2 are two views of one device and must report the same \
         profiles.\n  Media1: {m1:?}\n  Media2: {m2:?}",
    );
}

/// …and for the default device, so this cannot be satisfied by special-casing a
/// seeded state.
#[tokio::test]
async fn both_services_agree_on_the_default_device_too() {
    let server = MockServer::start().await.unwrap();
    let (m1, m2) = both_profile_token_sets(&server).await;

    assert!(!m1.is_empty(), "the default device has profiles");
    assert_eq!(
        m2, m1,
        "default device disagrees between services.\n  Media1: {m1:?}\n  Media2: {m2:?}",
    );
}

/// **The PTZ binding must agree too, and the default device must show both
/// answers.**
///
/// Media1 inlines the whole `<tt:PTZConfiguration>` inside the profile; Media2
/// emits `<tr2:PTZ token="…"/>` inside `<tr2:Configurations>`. Two genuinely
/// different shapes over one `ProfileEntry.ptz_config_token` — the same
/// arrangement that let the profile *list* drift, which is what this file
/// exists for.
///
/// Neither renderer emitted a PTZ element at all before this change:
/// `MediaProfile::ptz_config_token` and `MediaProfile2::ptz_config_token` were
/// both parsed and both permanently `None`, so the whole profile → PTZ binding
/// was unobservable from outside.
#[tokio::test]
async fn both_services_agree_on_each_profile_ptz_configuration() {
    let server = MockServer::start().await.unwrap();
    let client = OnvifClient::new(server.device_url());
    let media_url = format!("{}/onvif/media", server.base_url());
    let media2_url = format!("{}/onvif/media2", server.base_url());

    let m1: Vec<(String, Option<String>)> = client
        .get_profiles(&media_url)
        .await
        .expect("Media1 GetProfiles")
        .into_iter()
        .map(|p| (p.token, p.ptz_config_token))
        .collect();
    let m2: Vec<(String, Option<String>)> = client
        .get_profiles_media2(&media2_url)
        .await
        .expect("Media2 GetProfiles")
        .into_iter()
        .map(|p| (p.token, p.ptz_config_token))
        .collect();

    assert_eq!(
        m2, m1,
        "the two services disagree about which PTZ configuration each profile \
         is bound to.\n  Media1: {m1:?}\n  Media2: {m2:?}",
    );
    // Equality is worthless if every entry is `None` — two renderers that both
    // emit nothing agree perfectly. The default device must show both answers.
    assert!(
        m1.iter().any(|(_, c)| c.is_some()),
        "no profile is bound to a PTZ configuration: {m1:?}"
    );
    assert!(
        m1.iter().any(|(_, c)| c.is_none()),
        "every profile is PTZ-bound, so an unbound profile is untested: {m1:?}"
    );
    // …and not every bound profile may name the same configuration, or a
    // renderer emitting a constant token passes.
    let bound: BTreeSet<String> = m1.iter().filter_map(|(_, c)| c.clone()).collect();
    assert!(
        bound.len() >= 2,
        "all bound profiles name one configuration, so a constant would pass: {bound:?}"
    );
}

/// The two services must describe the **same** audio catalogue.
///
/// They did not. Both families were string literals in two files, and they
/// disagreed about the very same tokens:
///
/// | token | Media1 said | Media2 said |
/// |---|---|---|
/// | `ASC_1` | `AudioSourceConfig1` / `AudioSource_1` | `AudioSourceConfig` / `AudioSrc_1` |
/// | `AEC_1` | `AudioEncoder` | `AudioEncoderConfig` |
///
/// Nothing failed, because this file had no audio row — a `CLAUDE.md` step 5b
/// divergence hiding inside the audit's §5 "consistent stub" class, where by
/// definition nobody was looking for one.
///
/// `SessionTimeout` is deliberately **not** compared: it is a member of
/// `tt:AudioEncoderConfiguration` and not of `tt:AudioEncoder2Configuration`,
/// so the two services genuinely differ there and `MediaProfile2` has nowhere
/// to carry it.
#[tokio::test]
async fn both_services_agree_on_the_audio_catalogue() {
    let server = MockServer::start().await.unwrap();
    let client = OnvifClient::new(server.device_url());
    let media_url = format!("{}/onvif/media", server.base_url());
    let media2_url = format!("{}/onvif/media2", server.base_url());

    let srcs = |v: Vec<oxvif::AudioSourceConfiguration>| {
        v.into_iter()
            .map(|c| (c.token, c.name, c.use_count, c.source_token))
            .collect::<Vec<_>>()
    };
    let m1 = srcs(
        client
            .get_audio_source_configurations(&media_url)
            .await
            .expect("Media1 GetAudioSourceConfigurations"),
    );
    let m2 = srcs(
        client
            .get_audio_source_configurations_media2(&media2_url)
            .await
            .expect("Media2 GetAudioSourceConfigurations"),
    );
    assert_eq!(
        m2, m1,
        "the two services disagree about the audio source configurations.\n  \
         Media1: {m1:?}\n  Media2: {m2:?}",
    );
    assert!(m1.len() >= 2, "one entry cannot show a drift: {m1:?}");

    let encs = |v: Vec<oxvif::AudioEncoderConfiguration>| {
        v.into_iter()
            .map(|c| {
                (
                    c.token,
                    c.name,
                    c.encoding.as_str().to_string(),
                    c.bitrate,
                    c.sample_rate,
                    c.multicast.map(|m| (m.address, m.port)),
                )
            })
            .collect::<Vec<_>>()
    };
    let m1 = encs(
        client
            .get_audio_encoder_configurations(&media_url)
            .await
            .expect("Media1 GetAudioEncoderConfigurations"),
    );
    let m2 = encs(
        client
            .get_audio_encoder_configurations_media2(&media2_url)
            .await
            .expect("Media2 GetAudioEncoderConfigurations"),
    );
    assert_eq!(
        m2, m1,
        "the two services disagree about the audio encoder configurations.\n  \
         Media1: {m1:?}\n  Media2: {m2:?}",
    );
    assert!(m1.len() >= 2, "one entry cannot show a drift: {m1:?}");

    // The options answers must agree too — and they are the pair whose *shapes*
    // legitimately differ, so agreeing on the parsed content is the only thing
    // that can be asserted across them.
    for token in ["AEC_1", "AEC_2"] {
        let rows = |o: oxvif::AudioEncoderConfigurationOptions| {
            o.options
                .into_iter()
                .map(|r| {
                    (
                        r.encoding.as_str().to_string(),
                        r.bitrate_list,
                        r.sample_rate_list,
                    )
                })
                .collect::<Vec<_>>()
        };
        let a = rows(
            client
                .get_audio_encoder_configuration_options(&media_url, token)
                .await
                .expect("Media1 options"),
        );
        let b = rows(
            client
                .get_audio_encoder_configuration_options_media2(&media2_url, token)
                .await
                .expect("Media2 options"),
        );
        assert_eq!(b, a, "the two services disagree about {token}'s options");
        assert!(
            !a.is_empty() && !a[0].1.is_empty(),
            "{token}: an empty options answer is what the old parser produced \
             from a *correct* Media1 response, so it cannot be the expected \
             one here: {a:?}"
        );
    }

    // …and the profiles must name the same audio configurations.
    //
    // Added after a perturbation: emptying `ProfileEntry`'s audio slots in the
    // seed reddened **nothing**. `MediaProfile::audio_source_token` and
    // `audio_encoder_token` were parsed by both services and asserted by
    // neither, which is the same hole the PTZ binding was in.
    let bindings1: Vec<_> = client
        .get_profiles(&media_url)
        .await
        .expect("Media1 GetProfiles")
        .into_iter()
        .map(|p| (p.token, p.audio_source_token, p.audio_encoder_token))
        .collect();
    let bindings2: Vec<_> = client
        .get_profiles_media2(&media2_url)
        .await
        .expect("Media2 GetProfiles")
        .into_iter()
        .map(|p| (p.token, p.audio_source_token, p.audio_encoder_token))
        .collect();
    assert_eq!(
        bindings2, bindings1,
        "the two services disagree about each profile's audio bindings.\n  \
         Media1: {bindings1:?}\n  Media2: {bindings2:?}",
    );
    assert!(
        bindings1.iter().any(|(_, s, e)| s.is_some() && e.is_some()),
        "no profile carries audio, so two renderers emitting nothing would \
         agree perfectly: {bindings1:?}"
    );
}

/// A Media2 `CreateProfile` must actually create. It used to answer with a
/// literal token and write nothing — a success the caller could not act on.
#[tokio::test]
async fn a_media2_create_is_visible_to_both_services() {
    let server = start(state_with_profiles(3)).await;
    let client = OnvifClient::new(server.device_url());
    let media2_url = format!("{}/onvif/media2", server.base_url());

    let token = client
        .create_profile_media2(&media2_url, "created-via-media2")
        .await
        .expect("Media2 CreateProfile");

    let (m1, m2) = both_profile_token_sets(&server).await;
    assert!(
        m2.contains(&token),
        "the profile Media2 said it created ({token}) is not in its own list: {m2:?}",
    );
    assert!(
        m1.contains(&token),
        "a Media2 create must be visible to Media1 — one device: {m1:?}",
    );
    assert_eq!(m1.len(), 4, "3 seeded + 1 created");
}

/// A Media2 `DeleteProfile` must actually delete. The dispatcher used to answer
/// it with an unconditional empty success that removed nothing.
#[tokio::test]
async fn a_media2_delete_is_visible_to_both_services() {
    let server = start(state_with_profiles(3)).await;
    let client = OnvifClient::new(server.device_url());
    let media2_url = format!("{}/onvif/media2", server.base_url());

    // Seeded entries are `fixed: true`; create a deletable one first.
    let token = client
        .create_profile_media2(&media2_url, "doomed")
        .await
        .expect("Media2 CreateProfile");
    client
        .delete_profile_media2(&media2_url, &token)
        .await
        .expect("Media2 DeleteProfile");

    let (m1, m2) = both_profile_token_sets(&server).await;
    assert!(
        !m2.contains(&token),
        "Media2 reported the delete as successful but still lists {token}: {m2:?}",
    );
    assert!(
        !m1.contains(&token),
        "a Media2 delete must be visible to Media1: {m1:?}",
    );
    assert_eq!(m1.len(), 3, "back to the seeded three");
}

/// The same defect class as the reported one, in the encoder family and
/// pointing the other way: `SetVideoEncoderConfiguration` **wrote state on
/// Media2 and was a no-op on Media1**, so the identical call succeeded on both
/// services and only one of them changed the device.
///
/// Found by auditing every operation present in both dispatchers for whether it
/// takes `state` — the reported bug was an instance, not a one-off.
#[tokio::test]
async fn a_media1_encoder_write_actually_writes() {
    let server = MockServer::start().await.unwrap();
    let client = OnvifClient::new(server.device_url());
    let media_url = format!("{}/onvif/media", server.base_url());

    let before = client
        .get_video_encoder_configuration(&media_url, "VEC_1")
        .await
        .expect("read VEC_1");
    assert_ne!(
        before.resolution.width, 640,
        "fixture must not already be at the value we write"
    );

    let mut cfg = before.clone();
    cfg.name = "renamed-via-media1".into();
    cfg.resolution.width = 640;
    cfg.resolution.height = 360;
    client
        .set_video_encoder_configuration(&media_url, &cfg)
        .await
        .expect("Media1 SetVideoEncoderConfiguration");

    let after = client
        .get_video_encoder_configuration(&media_url, "VEC_1")
        .await
        .expect("re-read VEC_1");
    assert_eq!(
        (
            after.name.as_str(),
            after.resolution.width,
            after.resolution.height
        ),
        ("renamed-via-media1", 640, 360),
        "Media1 reported success and changed nothing — Set → Get must round-trip",
    );
}

/// …and that write must be the *same* device Media2 sees.
#[tokio::test]
async fn an_encoder_write_on_either_service_is_visible_to_the_other() {
    let server = MockServer::start().await.unwrap();
    let client = OnvifClient::new(server.device_url());
    let media_url = format!("{}/onvif/media", server.base_url());
    let media2_url = format!("{}/onvif/media2", server.base_url());

    let mut cfg = client
        .get_video_encoder_configuration(&media_url, "VEC_1")
        .await
        .unwrap();
    cfg.resolution.width = 800;
    cfg.resolution.height = 600;
    client
        .set_video_encoder_configuration(&media_url, &cfg)
        .await
        .unwrap();

    let via_media2 = client
        .get_video_encoder_configurations_media2(&media2_url)
        .await
        .expect("Media2 GetVideoEncoderConfigurations")
        .into_iter()
        .find(|c| c.token == "VEC_1")
        .expect("VEC_1 present on Media2");

    assert_eq!(
        (via_media2.resolution.width, via_media2.resolution.height),
        (800, 600),
        "a Media1 encoder write must be visible to Media2 — one device",
    );
}

/// Media1's four named binding operations and Media2's one generic
/// `AddConfiguration` write the same four `ProfileEntry` slots, so a binding
/// made through one service must be visible through the other.
///
/// Until the Tier 1 wiring (audit §3 items 1.4–1.7) **all five were
/// `resp_empty`** — every one reported success and bound nothing, so the two
/// services agreed by both being wrong. That is why this test asserts the
/// binding is *present* on both sides rather than merely equal on both sides.
#[tokio::test]
async fn a_media1_configuration_binding_is_visible_to_media2() {
    let server = start(state_with_profiles(2)).await;
    let client = OnvifClient::new(server.device_url());
    let media_url = format!("{}/onvif/media", server.base_url());
    let media2_url = format!("{}/onvif/media2", server.base_url());

    let created = client
        .create_profile(&media_url, "assembled-on-media1", Some("M1_BIND"))
        .await
        .expect("Media1 CreateProfile");
    assert_eq!(
        created.video_encoder_token, None,
        "a fresh profile starts with nothing bound"
    );

    client
        .add_video_encoder_configuration(&media_url, &created.token, "VEC_2")
        .await
        .expect("Media1 AddVideoEncoderConfiguration");

    let via_media2 = client
        .get_profiles_media2(&media2_url)
        .await
        .expect("Media2 GetProfiles")
        .into_iter()
        .find(|p| p.token == "M1_BIND")
        .expect("the created profile is on Media2 too");
    assert_eq!(
        via_media2.video_encoder_token.as_deref(),
        Some("VEC_2"),
        "a Media1 binding must be visible to Media2 — one profile list, one set of slots",
    );
}

/// …and the reverse, through Media2's generic `AddConfiguration`.
#[tokio::test]
async fn a_media2_configuration_binding_is_visible_to_media1() {
    let server = start(state_with_profiles(2)).await;
    let client = OnvifClient::new(server.device_url());
    let media_url = format!("{}/onvif/media", server.base_url());
    let media2_url = format!("{}/onvif/media2", server.base_url());

    let token = client
        .create_profile_media2(&media2_url, "assembled-on-media2")
        .await
        .expect("Media2 CreateProfile");
    client
        .add_configuration_media2(&media2_url, &token, "VideoSource", "VSC_2")
        .await
        .expect("Media2 AddConfiguration");

    let via_media1 = client
        .get_profile(&media_url, &token)
        .await
        .expect("Media1 GetProfile");
    assert_eq!(
        via_media1.video_source_config_token.as_deref(),
        Some("VSC_2"),
        "a Media2 binding must be visible to Media1",
    );
    // Media1 inlines the whole configuration, so it also resolves the *source*
    // behind the config — the token reference Media2 sends does not.
    assert_eq!(
        via_media1.video_source_token.as_deref(),
        Some("VS_2"),
        "Media1 resolves the bound config to its physical source",
    );
}

/// A `SetVideoSourceConfiguration` on either service edits the one catalogue.
/// Both arms were `resp_empty` before the Tier 1 wiring — audit §3 items 1.1
/// and 1.2, the same shape as the encoder divergence one commit earlier.
#[tokio::test]
async fn a_source_config_write_on_either_service_is_visible_to_the_other() {
    let server = MockServer::start().await.unwrap();
    let client = OnvifClient::new(server.device_url());
    let media_url = format!("{}/onvif/media", server.base_url());
    let media2_url = format!("{}/onvif/media2", server.base_url());

    let mut cfg = client
        .get_video_source_configuration(&media_url, "VSC_1")
        .await
        .expect("read VSC_1");
    cfg.name = "renamed-via-media1".into();
    client
        .set_video_source_configuration(&media_url, &cfg)
        .await
        .expect("Media1 SetVideoSourceConfiguration");

    let via_media2 = client
        .get_video_source_configurations_media2(&media2_url)
        .await
        .expect("Media2 GetVideoSourceConfigurations")
        .into_iter()
        .find(|c| c.token == "VSC_1")
        .expect("VSC_1 present on Media2");
    assert_eq!(via_media2.name, "renamed-via-media1");

    // …and back the other way, so neither direction can be the only one wired.
    let mut cfg2 = via_media2;
    cfg2.name = "renamed-via-media2".into();
    client
        .set_video_source_configuration_media2(&media2_url, &cfg2)
        .await
        .expect("Media2 SetVideoSourceConfiguration");

    let via_media1 = client
        .get_video_source_configuration(&media_url, "VSC_1")
        .await
        .expect("re-read VSC_1 on Media1");
    assert_eq!(via_media1.name, "renamed-via-media2");
}

/// The other direction: a Media1 write must reach Media2. This is the half that
/// a Media2-only fix would still leave broken.
#[tokio::test]
async fn a_media1_create_is_visible_to_media2() {
    let server = start(state_with_profiles(2)).await;
    let client = OnvifClient::new(server.device_url());
    let media_url = format!("{}/onvif/media", server.base_url());

    let created = client
        .create_profile(&media_url, "created-via-media1", Some("M1_NEW"))
        .await
        .expect("Media1 CreateProfile");

    let (m1, m2) = both_profile_token_sets(&server).await;
    assert!(m1.contains(&created.token));
    assert!(
        m2.contains(&created.token),
        "a Media1 create must be visible to Media2 — the two never converged \
         before this: {m2:?}",
    );
}
