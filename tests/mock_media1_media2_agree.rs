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
