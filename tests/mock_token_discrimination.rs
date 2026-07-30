//! Every token-taking mock operation, asked twice with two different tokens.
//!
//! ## Why this exists
//!
//! `docs/active/mock-audit-2026-07.md` §8.2, and the companion to
//! `tests/mock_roundtrip.rs`. That table asks *did my write land?*; this one asks
//! **did the token I sent select the answer I got back?**
//!
//! They catch different bugs. A handler can persist state perfectly and still
//! answer for the wrong channel — which is the failure `CLAUDE.md`'s
//! multi-sensor rule is about, and the one that is silent by construction:
//!
//! > A device answering a token-less request is not obliged to say so — it
//! > answers for its *default* channel, and on a single-sensor camera the
//! > result is indistinguishable from correct.
//!
//! Measured on a real two-sensor device (2026-07-28): a token-less
//! `GetVideoEncoderConfigurationOptions` returned lens 0's list, which a caller
//! would then display for lens 1, whose real maximum is half as wide.
//!
//! ## How a row works
//!
//! Each row names **two tokens the fixture deliberately disagrees on** and a
//! probe that reduces the answer to a comparable fingerprint. Then:
//!
//! - [`Expect::Discriminates`] — the two fingerprints must **differ**.
//! - [`Expect::Blind`] — a declared static fixture (audit §5). The two
//!   fingerprints must be **identical**, and the row cites why.
//!
//! Both arms are asserted. Wire a `Blind` row to the state and this test goes
//! red telling you to move it, so the list cannot quietly become a blind spot —
//! the same contract as the round-trip table.
//!
//! A fingerprint includes the *error* when a call fails, because refusing one
//! token and answering the other is itself discrimination — `imaging`'s focus
//! operations on the fixed lens are exactly that.
#![cfg(feature = "mock-server")]

use std::future::Future;
use std::pin::Pin;

use oxvif::OnvifClient;
use oxvif::mock::MockServer;

// ── Expectation ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum Expect {
    /// Two tokens, two answers.
    Discriminates,
    /// A declared static fixture — one answer for every token. The `&str` is
    /// the audit section that says so.
    Blind(&'static str),
}

impl Expect {
    fn label(self) -> String {
        match self {
            Expect::Discriminates => "Discriminates".into(),
            Expect::Blind(why) => format!("Blind ({why})"),
        }
    }
}

/// Reduce a client result to a comparable string. `Err` is kept, not unwrapped:
/// answering one token and refusing the other *is* discrimination.
fn fingerprint<T: std::fmt::Debug, E: std::fmt::Display>(r: Result<T, E>) -> String {
    match r {
        Ok(v) => format!("{v:?}"),
        Err(e) => format!("ERR {e}"),
    }
}

// ── One device per row ───────────────────────────────────────────────────────

struct Dev {
    server: MockServer,
    client: OnvifClient,
}

impl Dev {
    async fn new() -> Self {
        let server = MockServer::start().await.expect("mock server starts");
        let client = OnvifClient::new(server.device_url());
        Self { server, client }
    }

    fn url(&self, service: &str) -> String {
        format!("{}/onvif/{service}", self.server.base_url())
    }
}

// ── The table ────────────────────────────────────────────────────────────────

type Probe = for<'a> fn(&'a Dev, &'a str) -> Pin<Box<dyn Future<Output = String> + 'a>>;

struct Row {
    name: &'static str,
    expect: Expect,
    tokens: (&'static str, &'static str),
    probe: Probe,
}

/// `name => expectation, (token_a, token_b), async fn;`
macro_rules! rows {
    ($($name:literal => $expect:expr, $tokens:expr, $f:ident;)*) => {
        &[$(Row {
            name: $name,
            expect: $expect,
            tokens: $tokens,
            probe: {
                fn wrap<'a>(d: &'a Dev, t: &'a str)
                    -> Pin<Box<dyn Future<Output = String> + 'a>> { Box::pin($f(d, t)) }
                wrap
            },
        }),*]
    };
}

const ROWS: &[Row] = rows![
    // ── Media1 — per (sensor, stream) channel ───────────────────────────────
    "media1/encoder-options"      => Expect::Discriminates, ("VEC_1", "VEC_3"), m1_encoder_options;
    "media1/encoder-config"       => Expect::Discriminates, ("VEC_1", "VEC_3"), m1_encoder_config;
    "media1/source-options"       => Expect::Discriminates, ("VSC_1", "VSC_2"), m1_source_options;
    "media1/source-config"        => Expect::Discriminates, ("VSC_1", "VSC_2"), m1_source_config;
    "media1/profile"              => Expect::Discriminates, ("Profile_1", "Profile_3"), m1_profile;
    "media1/osds"                 => Expect::Discriminates, ("VSC_1", "VSC_2"), m1_osds;
    "media1/stream-uri"           => Expect::Blind("audit §5 — one canned RTSP URL"), ("Profile_1", "Profile_3"), m1_stream_uri;
    "media1/snapshot-uri"         => Expect::Blind("audit §5 — one canned URL"), ("Profile_1", "Profile_3"), m1_snapshot_uri;
    "media1/osd-options"          => Expect::Blind("audit §5 — static"), ("VSC_1", "VSC_2"), m1_osd_options;

    // ── Media2 ──────────────────────────────────────────────────────────────
    "media2/encoder-options"      => Expect::Discriminates, ("VEC_1", "VEC_3"), m2_encoder_options;
    "media2/encoder-config"       => Expect::Discriminates, ("VEC_1", "VEC_3"), m2_encoder_config;
    "media2/source-options"       => Expect::Discriminates, ("VSC_1", "VSC_2"), m2_source_options;
    "media2/encoder-instances"    => Expect::Blind("audit §5 — static"), ("VEC_1", "VEC_3"), m2_encoder_instances;
    "media2/stream-uri"           => Expect::Blind("audit §5 — one canned URL"), ("Profile_1", "Profile_3"), m2_stream_uri;
    "media2/video-source-modes"   => Expect::Blind("audit §5 — static"), ("VS_1", "VS_2"), m2_video_source_modes;

    // ── Imaging — per physical lens ─────────────────────────────────────────
    "imaging/settings"            => Expect::Discriminates, ("VS_1", "VS_2"), img_settings;
    "imaging/options"             => Expect::Discriminates, ("VS_1", "VS_2"), img_options;
    "imaging/status"              => Expect::Discriminates, ("VS_1", "VS_2"), img_status;
    "imaging/move-options"        => Expect::Discriminates, ("VS_1", "VS_2"), img_move_options;

    // ── PTZ — per head ──────────────────────────────────────────────────────
    "ptz/status"                  => Expect::Discriminates, ("Profile_1", "Profile_3"), ptz_status;
    "ptz/presets"                 => Expect::Discriminates, ("Profile_1", "Profile_3"), ptz_presets;
    "ptz/preset-tours"            => Expect::Discriminates, ("Profile_1", "Profile_2"), ptz_preset_tours;
    "ptz/preset-tour-options"     => Expect::Discriminates, ("Profile_1", "Profile_3"), ptz_preset_tour_options;
    "ptz/compatible-configs"      => Expect::Blind("audit §5 — PTZ configurations are static"), ("Profile_1", "Profile_3"), ptz_compatible_configs;

    // ── Recording — audit §4.2, no state at all ─────────────────────────────
    "recording/replay-uri"        => Expect::Discriminates, ("Rec_001", "Rec_002"), rec_replay_uri;
    "recording/job-state"         => Expect::Discriminates, ("Job_001", "Job_002"), rec_job_state;
];

// ── Media1 probes ────────────────────────────────────────────────────────────

async fn m1_encoder_options(d: &Dev, t: &str) -> String {
    fingerprint(
        d.client
            .get_video_encoder_configuration_options(&d.url("media"), t)
            .await,
    )
}

async fn m1_encoder_config(d: &Dev, t: &str) -> String {
    fingerprint(
        d.client
            .get_video_encoder_configuration(&d.url("media"), t)
            .await,
    )
}

async fn m1_source_options(d: &Dev, t: &str) -> String {
    fingerprint(
        d.client
            .get_video_source_configuration_options(&d.url("media"), t)
            .await,
    )
}

async fn m1_source_config(d: &Dev, t: &str) -> String {
    fingerprint(
        d.client
            .get_video_source_configuration(&d.url("media"), t)
            .await,
    )
}

async fn m1_profile(d: &Dev, t: &str) -> String {
    fingerprint(d.client.get_profile(&d.url("media"), t).await)
}

async fn m1_osds(d: &Dev, t: &str) -> String {
    fingerprint(d.client.get_osds(&d.url("media"), Some(t)).await)
}

async fn m1_stream_uri(d: &Dev, t: &str) -> String {
    fingerprint(d.client.get_stream_uri(&d.url("media"), t).await)
}

async fn m1_snapshot_uri(d: &Dev, t: &str) -> String {
    fingerprint(d.client.get_snapshot_uri(&d.url("media"), t).await)
}

async fn m1_osd_options(d: &Dev, t: &str) -> String {
    fingerprint(d.client.get_osd_options(&d.url("media"), t).await)
}

// ── Media2 probes ────────────────────────────────────────────────────────────

async fn m2_encoder_options(d: &Dev, t: &str) -> String {
    fingerprint(
        d.client
            .get_video_encoder_configuration_options_media2(&d.url("media2"), t)
            .await,
    )
}

async fn m2_encoder_config(d: &Dev, t: &str) -> String {
    fingerprint(
        d.client
            .get_video_encoder_configuration_media2(&d.url("media2"), t)
            .await,
    )
}

async fn m2_source_options(d: &Dev, t: &str) -> String {
    fingerprint(
        d.client
            .get_video_source_configuration_options_media2(&d.url("media2"), t)
            .await,
    )
}

async fn m2_encoder_instances(d: &Dev, t: &str) -> String {
    fingerprint(
        d.client
            .get_video_encoder_instances_media2(&d.url("media2"), t)
            .await,
    )
}

async fn m2_stream_uri(d: &Dev, t: &str) -> String {
    fingerprint(d.client.get_stream_uri_media2(&d.url("media2"), t).await)
}

async fn m2_video_source_modes(d: &Dev, t: &str) -> String {
    fingerprint(
        d.client
            .get_video_source_modes_media2(&d.url("media2"), t)
            .await,
    )
}

// ── Imaging probes ───────────────────────────────────────────────────────────

async fn img_settings(d: &Dev, t: &str) -> String {
    fingerprint(d.client.get_imaging_settings(&d.url("imaging"), t).await)
}

async fn img_options(d: &Dev, t: &str) -> String {
    fingerprint(d.client.get_imaging_options(&d.url("imaging"), t).await)
}

async fn img_status(d: &Dev, t: &str) -> String {
    fingerprint(d.client.imaging_get_status(&d.url("imaging"), t).await)
}

/// `VS_2` has no focus motor and refuses. Refusing one token and answering the
/// other is discrimination — the fingerprint keeps the error for that reason.
async fn img_move_options(d: &Dev, t: &str) -> String {
    fingerprint(
        d.client
            .imaging_get_move_options(&d.url("imaging"), t)
            .await,
    )
}

// ── PTZ probes ───────────────────────────────────────────────────────────────

async fn ptz_status(d: &Dev, t: &str) -> String {
    fingerprint(d.client.ptz_get_status(&d.url("ptz"), t).await)
}

async fn ptz_presets(d: &Dev, t: &str) -> String {
    fingerprint(d.client.ptz_get_presets(&d.url("ptz"), t).await)
}

async fn ptz_preset_tours(d: &Dev, t: &str) -> String {
    fingerprint(d.client.ptz_get_preset_tours(&d.url("ptz"), t).await)
}

async fn ptz_preset_tour_options(d: &Dev, t: &str) -> String {
    fingerprint(
        d.client
            .ptz_get_preset_tour_options(&d.url("ptz"), t, None)
            .await,
    )
}

async fn ptz_compatible_configs(d: &Dev, t: &str) -> String {
    fingerprint(
        d.client
            .ptz_get_compatible_configurations(&d.url("ptz"), t)
            .await,
    )
}

// ── Recording probes ─────────────────────────────────────────────────────────

async fn rec_replay_uri(d: &Dev, t: &str) -> String {
    fingerprint(
        d.client
            .get_replay_uri(&d.url("replay"), t, "RTP-Unicast", "RTSP")
            .await,
    )
}

async fn rec_job_state(d: &Dev, t: &str) -> String {
    fingerprint(
        d.client
            .get_recording_job_state(&d.url("recording"), t)
            .await,
    )
}

// ── The test ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn every_token_taking_operation_matches_its_declared_expectation() {
    // Guard on the guard: a gutted table makes every assertion below vacuous.
    // 26 rows at 0.15.0.
    assert!(
        ROWS.len() >= 22,
        "the table has only {} rows — it was gutted, which would make this test \
         pass for the wrong reason",
        ROWS.len()
    );

    let mut mismatches = Vec::new();
    let mut discriminating = 0usize;

    for row in ROWS {
        let dev = Dev::new().await;
        let (ta, tb) = row.tokens;
        let a = (row.probe)(&dev, ta).await;
        let b = (row.probe)(&dev, tb).await;

        match (a == b, row.expect) {
            (false, Expect::Discriminates) => discriminating += 1,
            (true, Expect::Blind(_)) => {}
            (true, Expect::Discriminates) => mismatches.push(format!(
                "{}: declared Discriminates but {ta} and {tb} got the same answer.\n      \
                 Either the handler ignores the token, or the fixture does not \
                 disagree about anything this probe reads.\n      answer: {a}",
                row.name,
            )),
            (false, Expect::Blind(_)) => mismatches.push(format!(
                "{}: declared {} but {ta} and {tb} now differ.\n      \
                 If you just wired this up, move the row to \
                 `Expect::Discriminates` (and strike it from \
                 docs/active/mock-audit-2026-07.md).\n      {ta}: {a}\n      {tb}: {b}",
                row.name,
                row.expect.label(),
            )),
        }
    }

    assert!(
        mismatches.is_empty(),
        "{} of {} token probes disagree with the table:\n  {}",
        mismatches.len(),
        ROWS.len(),
        mismatches.join("\n  "),
    );

    // And the positive side is not vacuous: most of the table really does
    // answer per token.
    assert!(
        discriminating >= 17,
        "only {discriminating} operations discriminated — the mock is far more \
         token-blind than the table claims",
    );
}
