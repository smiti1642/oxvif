//! Every `Set → Get` pair on the mock, in one table.
//!
//! ## Why this exists
//!
//! `docs/active/mock-audit-2026-07.md` §8 names the one structural cause behind
//! every mock defect found so far:
//!
//! > **Nothing distinguishes "deliberately static" from "not wired up yet"** —
//! > not the type system, not the dispatch table, not the tests.
//!
//! `resp_profiles_media2()` (a bug, fixed in `fa1cd91`) and
//! `resp_audio_sources()` (a perfectly fine stub) have the same signature, live
//! in the same match block, and are indistinguishable at every level. That is
//! why five instances of the class were reported from *outside* the project
//! rather than caught in review.
//!
//! The table below is where that distinction is finally recorded. Each pair
//! declares an [`Expect`]:
//!
//! - [`Expect::Works`] — the write must land and the getter must show it.
//! - [`Expect::Broken`] — a **real defect**, with its audit citation. The write
//!   is discarded today and the test asserts that it still is.
//! - [`Expect::Static`] — a **deliberate stub**. The whole family is fixture
//!   data; nothing pretends otherwise.
//!
//! Both non-`Works` arms are asserted, not skipped. Wire one of them up and this
//! test goes red telling you to move the row — so the list cannot rot into a
//! permanent blind spot, which is the usual fate of an xfail list.
//!
//! ## What it is not
//!
//! This is not a fidelity check. It asks one question per pair — *did the value
//! I wrote come back?* — over the **public API only** (`oxvif::mock` plus the
//! ordinary client), over real HTTP, against a fresh server per pair.
#![cfg(feature = "mock-server")]

use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

use oxvif::OnvifClient;
use oxvif::mock::MockServer;
use oxvif::{
    ImagingSettings, OsdConfiguration, OsdPosition, OsdTextString, RecordingConfiguration,
    RecordingJobConfiguration, SetDateTimeRequest, UtcDateTime,
};

// ── Outcome and expectation ──────────────────────────────────────────────────

#[derive(Debug)]
enum Outcome {
    /// The value written came back out of the getter.
    RoundTripped,
    /// The call succeeded and the getter does not reflect it.
    Discarded(String),
    /// The call itself errored. Never expected, for any row — see the second
    /// assertion in the test.
    Failed(String),
}

#[derive(Clone, Copy)]
enum Expect {
    /// The write must land.
    Works,
    /// A real defect. The `&str` is the audit section that catalogues it.
    ///
    /// **No row uses this today, and that is the point** — every `Broken` row
    /// the audit found has been wired up. Kept rather than deleted because it is
    /// half the contract: the next defect gets a row here with its citation
    /// instead of a comment or nothing, and the test then asserts the write is
    /// *still* discarded until someone fixes it.
    #[allow(dead_code)]
    Broken(&'static str),
    /// Deliberately fixture data on both sides. `docs/active/mock-audit-2026-07.md` §5.
    ///
    /// **No row uses this today either**, as of the audio catalogue — the last
    /// two `Static` rows were `media1/audio-encoder-config` and
    /// `media2/audio-encoder-config`, and Tier 3 is now empty. Kept for the same
    /// reason as `Broken`: the next family that is genuinely fixture data gets a
    /// row here with its citation rather than silence, and the test then asserts
    /// the write really is discarded. Deleting the arm would delete the only
    /// place that distinction can be written down.
    #[allow(dead_code)]
    Static(&'static str),
}

impl Expect {
    fn should_round_trip(self) -> bool {
        matches!(self, Expect::Works)
    }

    fn label(self) -> String {
        match self {
            Expect::Works => "Works".into(),
            Expect::Broken(why) => format!("Broken ({why})"),
            Expect::Static(why) => format!("Static ({why})"),
        }
    }
}

/// Compare what was written against what came back.
fn cmp<T: PartialEq + Debug>(wrote: T, read: T) -> Outcome {
    if wrote == read {
        Outcome::RoundTripped
    } else {
        Outcome::Discarded(format!("wrote {wrote:?}, read back {read:?}"))
    }
}

/// Await a client call, turning any error into [`Outcome::Failed`] rather than
/// letting it masquerade as a discarded write.
macro_rules! call {
    ($what:literal, $e:expr) => {
        match $e.await {
            Ok(v) => v,
            Err(e) => return Outcome::Failed(format!("{}: {e}", $what)),
        }
    };
}

// ── One device per pair ──────────────────────────────────────────────────────

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

type Run = for<'a> fn(&'a Dev) -> Pin<Box<dyn Future<Output = Outcome> + 'a>>;

struct Pair {
    name: &'static str,
    expect: Expect,
    run: Run,
}

/// `name => expectation, async fn;`
macro_rules! pairs {
    ($($name:literal => $expect:expr, $f:ident;)*) => {
        &[$(Pair {
            name: $name,
            expect: $expect,
            run: {
                fn wrap<'a>(d: &'a Dev) -> Pin<Box<dyn Future<Output = Outcome> + 'a>> {
                    Box::pin($f(d))
                }
                wrap
            },
        }),*]
    };
}

const PAIRS: &[Pair] = pairs![
    // ── Device ──────────────────────────────────────────────────────────────
    "device/hostname"                  => Expect::Works, hostname;
    "device/ntp"                       => Expect::Works, ntp;
    "device/dns"                       => Expect::Works, dns;
    "device/scopes"                    => Expect::Works, scopes;
    "device/users-create"              => Expect::Works, users_create;
    "device/users-delete"              => Expect::Works, users_delete;
    "device/user-level"                => Expect::Works, user_level;
    "device/network-interfaces"        => Expect::Works                     , network_interfaces;
    "device/network-protocols"         => Expect::Works, network_protocols;
    "device/default-gateway"           => Expect::Works, default_gateway;
    "device/relay-output-settings"     => Expect::Works, relay_output_settings;
    "device/system-date-and-time"      => Expect::Works, system_date_and_time;
    "device/discovery-mode"            => Expect::Works                     , discovery_mode;
    "device/storage-configuration"     => Expect::Works, storage_configuration;
    "device/storage-create"            => Expect::Works, storage_create;

    // ── Media1 ──────────────────────────────────────────────────────────────
    "media1/profile-create"            => Expect::Works, m1_profile_create;
    "media1/profile-delete"            => Expect::Works, m1_profile_delete;
    "media1/video-encoder-config"      => Expect::Works, m1_video_encoder_config;
    "media1/osd-create"                => Expect::Works, m1_osd_create;
    "media1/osd-set"                   => Expect::Works, m1_osd_set;
    "media1/osd-delete"                => Expect::Works, m1_osd_delete;
    "media1/video-source-config"       => Expect::Works                     , m1_video_source_config;
    "media1/add-video-encoder-config"  => Expect::Works                     , m1_add_video_encoder;
    "media1/remove-video-encoder-cfg"  => Expect::Works                     , m1_remove_video_encoder;
    "media1/add-video-source-config"   => Expect::Works                     , m1_add_video_source;
    "media1/remove-video-source-cfg"   => Expect::Works                     , m1_remove_video_source;
    "media1/audio-encoder-config"      => Expect::Works, m1_audio_encoder_config;

    // ── Media2 ──────────────────────────────────────────────────────────────
    "media2/profile-create"            => Expect::Works, m2_profile_create;
    "media2/profile-delete"            => Expect::Works, m2_profile_delete;
    "media2/video-encoder-config"      => Expect::Works, m2_video_encoder_config;
    "media2/video-source-config"       => Expect::Works                     , m2_video_source_config;
    "media2/add-configuration"         => Expect::Works                     , m2_add_configuration;
    "media2/add-ptz-configuration"     => Expect::Works                     , m2_add_ptz_configuration;
    "media2/remove-configuration"      => Expect::Works                     , m2_remove_configuration;
    "media2/metadata-config"           => Expect::Works, m2_metadata_config;
    "media2/audio-encoder-config"      => Expect::Works, m2_audio_encoder_config;

    // ── PTZ ─────────────────────────────────────────────────────────────────
    "ptz/preset-set"                   => Expect::Works, ptz_preset_set;
    "ptz/preset-remove"                => Expect::Works, ptz_preset_remove;
    "ptz/absolute-move"                => Expect::Works, ptz_absolute_move;
    "ptz/home-position"                => Expect::Works, ptz_home_position;
    "ptz/preset-tour-create"           => Expect::Works, ptz_preset_tour_create;
    "ptz/preset-tour-remove"           => Expect::Works, ptz_preset_tour_remove;
    "ptz/configuration"                => Expect::Works, ptz_configuration;

    // ── Imaging ─────────────────────────────────────────────────────────────
    "imaging/settings"                 => Expect::Works, imaging_settings;

    // ── Recording ───────────────────────────────────────────────────────────
    "recording/create"                 => Expect::Works                 , recording_create;
    "recording/delete"                 => Expect::Works                 , recording_delete;
    "recording/track-create"           => Expect::Works                 , recording_track_create;
    "recording/job-create"             => Expect::Works                 , recording_job_create;
    "recording/job-mode"               => Expect::Works                 , recording_job_mode;
];

// ── Device checks ────────────────────────────────────────────────────────────

async fn hostname(d: &Dev) -> Outcome {
    let want = "roundtrip-host-4417";
    call!("SetHostname", d.client.set_hostname(want));
    let got = call!("GetHostname", d.client.get_hostname());
    cmp(want.to_string(), got.name.unwrap_or_default())
}

async fn ntp(d: &Dev) -> Outcome {
    let want = "ntp-4418.example.invalid";
    call!("SetNTP", d.client.set_ntp(false, &[want]));
    let got = call!("GetNTP", d.client.get_ntp());
    cmp(vec![want.to_string()], got.servers)
}

async fn dns(d: &Dev) -> Outcome {
    let want = "10.44.19.53";
    call!("SetDNS", d.client.set_dns(false, &[want]));
    let got = call!("GetDNS", d.client.get_dns());
    cmp(vec![want.to_string()], got.servers)
}

async fn scopes(d: &Dev) -> Outcome {
    let want = "onvif://www.onvif.org/name/roundtrip-4420";
    call!("SetScopes", d.client.set_scopes(&[want]));
    let got = call!("GetScopes", d.client.get_scopes());
    cmp(true, got.iter().any(|s| s == want))
}

async fn users_create(d: &Dev) -> Outcome {
    call!(
        "CreateUsers",
        d.client.create_users(&[("rt4421", "Pw-4421!", "Operator")])
    );
    let got = call!("GetUsers", d.client.get_users());
    cmp(true, got.iter().any(|u| u.username == "rt4421"))
}

async fn users_delete(d: &Dev) -> Outcome {
    call!(
        "CreateUsers",
        d.client.create_users(&[("rt4422", "Pw-4422!", "User")])
    );
    call!("DeleteUsers", d.client.delete_users(&["rt4422"]));
    let got = call!("GetUsers", d.client.get_users());
    cmp(false, got.iter().any(|u| u.username == "rt4422"))
}

async fn user_level(d: &Dev) -> Outcome {
    call!(
        "CreateUsers",
        d.client.create_users(&[("rt4423", "Pw-4423!", "User")])
    );
    call!(
        "SetUser",
        d.client.set_user("rt4423", None, "Administrator")
    );
    let got = call!("GetUsers", d.client.get_users());
    let level = got
        .iter()
        .find(|u| u.username == "rt4423")
        .map(|u| u.user_level.clone())
        .unwrap_or_default();
    cmp("Administrator".to_string(), level)
}

/// The **partial** write, and the row this table found on its own: the handler
/// reads `Enabled`, `FromDHCP`, `Address` and `PrefixLength` out of the body and
/// silently drops `MTU`, which the client does send and `GetNetworkInterfaces`
/// does report. Three of five fields landing is what makes it look wired.
async fn network_interfaces(d: &Dev) -> Outcome {
    use oxvif::NetworkInterfaceConfig;

    let want_mtu = 1420;
    let cfg = NetworkInterfaceConfig {
        enabled: true,
        mtu: Some(want_mtu),
        ipv4: None,
        ipv6: None,
    };
    call!(
        "SetNetworkInterfaces",
        d.client.set_network_interfaces("eth0", &cfg)
    );
    let got = call!("GetNetworkInterfaces", d.client.get_network_interfaces());
    let mtu = got.iter().find(|i| i.token == "eth0").map(|i| i.mtu);
    cmp(Some(want_mtu), mtu)
}

async fn network_protocols(d: &Dev) -> Outcome {
    let want_port = 8899u32;
    call!(
        "SetNetworkProtocols",
        d.client
            .set_network_protocols(&[("HTTP", true, &[want_port])])
    );
    let got = call!("GetNetworkProtocols", d.client.get_network_protocols());
    let ports = got
        .iter()
        .find(|p| p.name == "HTTP")
        .map(|p| p.ports.clone())
        .unwrap_or_default();
    cmp(vec![want_port], ports)
}

async fn default_gateway(d: &Dev) -> Outcome {
    let want = "10.44.24.254";
    call!(
        "SetNetworkDefaultGateway",
        d.client.set_network_default_gateway(&[want])
    );
    let got = call!(
        "GetNetworkDefaultGateway",
        d.client.get_network_default_gateway()
    );
    cmp(vec![want.to_string()], got.ipv4_addresses)
}

async fn relay_output_settings(d: &Dev) -> Outcome {
    let want_delay = "PT7S";
    call!(
        "SetRelayOutputSettings",
        d.client
            .set_relay_output_settings("RelayOutput_1", "Monostable", want_delay, "closed")
    );
    let got = call!("GetRelayOutputs", d.client.get_relay_outputs());
    let delay = got
        .iter()
        .find(|r| r.token == "RelayOutput_1")
        .map(|r| r.delay_time.clone())
        .unwrap_or_default();
    cmp(want_delay.to_string(), delay)
}

async fn system_date_and_time(d: &Dev) -> Outcome {
    let want_tz = "GMT-11";
    let req = SetDateTimeRequest {
        datetime_type: "Manual".into(),
        daylight_savings: false,
        timezone: want_tz.into(),
        utc_datetime: Some(UtcDateTime {
            year: 2031,
            month: 3,
            day: 14,
            hour: 15,
            minute: 9,
            second: 26,
        }),
    };
    call!(
        "SetSystemDateAndTime",
        d.client.set_system_date_and_time(&req)
    );
    let got = call!("GetSystemDateAndTime", d.client.get_system_date_and_time());
    cmp(want_tz.to_string(), got.timezone)
}

/// `GetDiscoveryMode` **is** state-driven; `SetDiscoveryMode` is `resp_empty`.
/// The exact shape of the reported Media2 bug: a live getter over a discarded
/// write.
async fn discovery_mode(d: &Dev) -> Outcome {
    let want = "NonDiscoverable";
    call!("SetDiscoveryMode", d.client.set_discovery_mode(want));
    let got = call!("GetDiscoveryMode", d.client.get_discovery_mode());
    cmp(want.to_string(), got)
}

/// Updates every field the client can send, not just `local_path`.
///
/// `storage_uri` and `user` are the audit's Tier 4 "storage credential fields"
/// — parsed by `StorageConfiguration` and never emitted by the mock before
/// 0.15. Asserting only the path would leave that fix unproved, since the old
/// static fixture already carried a `LocalPath`.
async fn storage_configuration(d: &Dev) -> Outcome {
    let want = ("NFS", "/mnt/roundtrip-4424", "nfs://10.9.8.7/rt", "rt-user");
    call!(
        "SetStorageConfiguration",
        d.client
            .set_storage_configuration("SD_01", want.0, want.1, want.2, want.3)
    );
    let got = call!(
        "GetStorageConfigurations",
        d.client.get_storage_configurations()
    );
    let e = got.iter().find(|s| s.token == "SD_01");
    // The trailing element is `NAS_01`'s untouched user. A handler that wrote
    // the update into every entry rather than the addressed one would satisfy
    // the first four fields and fail here.
    let untouched = got
        .iter()
        .find(|s| s.token == "NAS_01")
        .map(|s| s.user.clone())
        .unwrap_or_default();
    cmp(
        format!("{want:?} + NAS_01 user \"recorder\""),
        format!(
            "{:?} + NAS_01 user {untouched:?}",
            e.map(|s| (
                s.storage_type.as_str(),
                s.local_path.as_str(),
                s.storage_uri.as_str(),
                s.user.as_str()
            ))
            .unwrap_or(("", "", "", ""))
        ),
    )
}

/// A token-less `SetStorageConfiguration` creates a new entry that the getter
/// then lists. The create path is separate from the update path in the
/// handler, so one row cannot cover both.
async fn storage_create(d: &Dev) -> Outcome {
    let before = call!(
        "GetStorageConfigurations",
        d.client.get_storage_configurations()
    )
    .len();
    call!(
        "SetStorageConfiguration",
        d.client
            .set_storage_configuration("", "CIFS", "", "smb://10.1.1.1/new-4425", "")
    );
    let after = call!(
        "GetStorageConfigurations",
        d.client.get_storage_configurations()
    );
    let created = after
        .iter()
        .find(|s| s.storage_uri == "smb://10.1.1.1/new-4425");
    cmp(
        format!("{} entries, CIFS created", before + 1),
        format!(
            "{} entries, {} created",
            after.len(),
            created
                .map(|s| s.storage_type.as_str())
                .unwrap_or("nothing")
        ),
    )
}

// ── Media1 checks ────────────────────────────────────────────────────────────

async fn m1_profile_create(d: &Dev) -> Outcome {
    let url = d.url("media");
    let created = call!(
        "CreateProfile",
        d.client.create_profile(&url, "rt-4430", Some("RT_M1_4430"))
    );
    let got = call!("GetProfiles", d.client.get_profiles(&url));
    cmp(true, got.iter().any(|p| p.token == created.token))
}

async fn m1_profile_delete(d: &Dev) -> Outcome {
    let url = d.url("media");
    let created = call!(
        "CreateProfile",
        d.client.create_profile(&url, "rt-4431", Some("RT_M1_4431"))
    );
    call!(
        "DeleteProfile",
        d.client.delete_profile(&url, &created.token)
    );
    let got = call!("GetProfiles", d.client.get_profiles(&url));
    cmp(false, got.iter().any(|p| p.token == created.token))
}

async fn m1_video_encoder_config(d: &Dev) -> Outcome {
    let url = d.url("media");
    let mut cfg = call!(
        "GetVideoEncoderConfiguration",
        d.client.get_video_encoder_configuration(&url, "VEC_1")
    );
    cfg.name = "rt-encoder-4432".into();
    call!(
        "SetVideoEncoderConfiguration",
        d.client.set_video_encoder_configuration(&url, &cfg)
    );
    let got = call!(
        "GetVideoEncoderConfiguration",
        d.client.get_video_encoder_configuration(&url, "VEC_1")
    );
    cmp("rt-encoder-4432".to_string(), got.name)
}

fn rt_osd(token: &str, text: &str) -> OsdConfiguration {
    OsdConfiguration {
        token: token.into(),
        video_source_config_token: "VSC_1".into(),
        type_: "Text".into(),
        position: OsdPosition {
            type_: "UpperLeft".into(),
            x: None,
            y: None,
        },
        text_string: Some(OsdTextString {
            type_: "Plain".into(),
            plain_text: Some(text.into()),
            date_format: None,
            time_format: None,
            font_size: Some(32),
            font_color: None,
            background_color: None,
            is_persistent_text: None,
        }),
        image_path: None,
    }
}

async fn m1_osd_create(d: &Dev) -> Outcome {
    let url = d.url("media");
    let token = call!(
        "CreateOSD",
        d.client.create_osd(&url, &rt_osd("", "rt-osd-4433"))
    );
    let got = call!("GetOSDs", d.client.get_osds(&url, Some("VSC_1")));
    cmp(true, got.iter().any(|o| o.token == token))
}

async fn m1_osd_set(d: &Dev) -> Outcome {
    let url = d.url("media");
    call!(
        "SetOSD",
        d.client.set_osd(&url, &rt_osd("OSD_1", "rt-osd-4434"))
    );
    let got = call!("GetOSD", d.client.get_osd(&url, "OSD_1"));
    let text = got
        .text_string
        .and_then(|t| t.plain_text)
        .unwrap_or_default();
    cmp("rt-osd-4434".to_string(), text)
}

async fn m1_osd_delete(d: &Dev) -> Outcome {
    let url = d.url("media");
    let token = call!(
        "CreateOSD",
        d.client.create_osd(&url, &rt_osd("", "rt-osd-4435"))
    );
    call!("DeleteOSD", d.client.delete_osd(&url, &token));
    let got = call!("GetOSDs", d.client.get_osds(&url, Some("VSC_1")));
    cmp(false, got.iter().any(|o| o.token == token))
}

async fn m1_video_source_config(d: &Dev) -> Outcome {
    let url = d.url("media");
    let mut cfg = call!(
        "GetVideoSourceConfiguration",
        d.client.get_video_source_configuration(&url, "VSC_1")
    );
    cfg.name = "rt-vsc-4436".into();
    call!(
        "SetVideoSourceConfiguration",
        d.client.set_video_source_configuration(&url, &cfg)
    );
    let got = call!(
        "GetVideoSourceConfiguration",
        d.client.get_video_source_configuration(&url, "VSC_1")
    );
    cmp("rt-vsc-4436".to_string(), got.name)
}

/// A freshly created profile has nothing bound. Binding an encoder to it and
/// reading the profile back is the minimal "can I assemble a profile on this
/// mock?" question — and today the answer is no.
async fn m1_add_video_encoder(d: &Dev) -> Outcome {
    let url = d.url("media");
    let created = call!(
        "CreateProfile",
        d.client.create_profile(&url, "rt-4437", Some("RT_M1_4437"))
    );
    call!(
        "AddVideoEncoderConfiguration",
        d.client
            .add_video_encoder_configuration(&url, &created.token, "VEC_2")
    );
    let got = call!("GetProfile", d.client.get_profile(&url, &created.token));
    cmp(Some("VEC_2".to_string()), got.video_encoder_token)
}

async fn m1_remove_video_encoder(d: &Dev) -> Outcome {
    let url = d.url("media");
    call!(
        "RemoveVideoEncoderConfiguration",
        d.client
            .remove_video_encoder_configuration(&url, "Profile_1")
    );
    let got = call!("GetProfile", d.client.get_profile(&url, "Profile_1"));
    cmp(None, got.video_encoder_token)
}

async fn m1_add_video_source(d: &Dev) -> Outcome {
    let url = d.url("media");
    let created = call!(
        "CreateProfile",
        d.client.create_profile(&url, "rt-4439", Some("RT_M1_4439"))
    );
    call!(
        "AddVideoSourceConfiguration",
        d.client
            .add_video_source_configuration(&url, &created.token, "VSC_2")
    );
    let got = call!("GetProfile", d.client.get_profile(&url, &created.token));
    cmp(Some("VSC_2".to_string()), got.video_source_config_token)
}

async fn m1_remove_video_source(d: &Dev) -> Outcome {
    let url = d.url("media");
    call!(
        "RemoveVideoSourceConfiguration",
        d.client
            .remove_video_source_configuration(&url, "Profile_1")
    );
    let got = call!("GetProfile", d.client.get_profile(&url, "Profile_1"));
    cmp(None, got.video_source_config_token)
}

/// Writes every member the Media1 type carries, and checks the *other*
/// configuration is untouched.
///
/// `bitrate` alone — all this asserted while the row was `Static` — passes
/// against a handler that stores the bitrate and drops the encoding, the
/// multicast group and the session timeout: the `MTU` shape from `CLAUDE.md`
/// step 5c.
async fn m1_audio_encoder_config(d: &Dev) -> Outcome {
    let url = d.url("media");
    let mut cfg = call!(
        "GetAudioEncoderConfiguration",
        d.client.get_audio_encoder_configuration(&url, "AEC_1")
    );
    // Seed: G711 / 64 / 8, multicast 239.0.0.5:40002 ttl 5, PT60S.
    cfg.name = "rt-aec-4460".into();
    cfg.encoding = oxvif::AudioEncoding::G726;
    cfg.bitrate = 44;
    cfg.sample_rate = 16;
    cfg.session_timeout = Some("PT15S".into());
    if let Some(m) = cfg.multicast.as_mut() {
        m.address = "239.0.0.9".into();
        m.port = 41000;
        m.auto_start = true;
    }
    call!(
        "SetAudioEncoderConfiguration",
        d.client.set_audio_encoder_configuration(&url, &cfg)
    );
    let got = call!(
        "GetAudioEncoderConfiguration",
        d.client.get_audio_encoder_configuration(&url, "AEC_1")
    );
    let other = call!(
        "GetAudioEncoderConfiguration",
        d.client.get_audio_encoder_configuration(&url, "AEC_2")
    );
    cmp(
        (
            "rt-aec-4460".to_string(),
            "G726".to_string(),
            44,
            16,
            Some(("239.0.0.9".to_string(), 41000, true)),
            Some("PT15S".to_string()),
            "AudioEncoder2".to_string(),
        ),
        (
            got.name,
            got.encoding.as_str().to_string(),
            got.bitrate,
            got.sample_rate,
            got.multicast.map(|m| (m.address, m.port, m.auto_start)),
            got.session_timeout,
            other.name,
        ),
    )
}

// ── Media2 checks ────────────────────────────────────────────────────────────

async fn m2_profile_create(d: &Dev) -> Outcome {
    let url = d.url("media2");
    let token = call!(
        "CreateProfile",
        d.client.create_profile_media2(&url, "rt-4440")
    );
    let got = call!("GetProfiles", d.client.get_profiles_media2(&url));
    cmp(true, got.iter().any(|p| p.token == token))
}

async fn m2_profile_delete(d: &Dev) -> Outcome {
    let url = d.url("media2");
    let token = call!(
        "CreateProfile",
        d.client.create_profile_media2(&url, "rt-4441")
    );
    call!(
        "DeleteProfile",
        d.client.delete_profile_media2(&url, &token)
    );
    let got = call!("GetProfiles", d.client.get_profiles_media2(&url));
    cmp(false, got.iter().any(|p| p.token == token))
}

async fn m2_video_encoder_config(d: &Dev) -> Outcome {
    let url = d.url("media2");
    let mut cfg = call!(
        "GetVideoEncoderConfiguration",
        d.client
            .get_video_encoder_configuration_media2(&url, "VEC_1")
    );
    cfg.name = "rt-encoder2-4442".into();
    call!(
        "SetVideoEncoderConfiguration",
        d.client.set_video_encoder_configuration_media2(&url, &cfg)
    );
    let got = call!(
        "GetVideoEncoderConfiguration",
        d.client
            .get_video_encoder_configuration_media2(&url, "VEC_1")
    );
    cmp("rt-encoder2-4442".to_string(), got.name)
}

async fn m2_video_source_config(d: &Dev) -> Outcome {
    let url = d.url("media2");
    let mut cfg = call!(
        "GetVideoSourceConfigurations",
        d.client.get_video_source_configurations_media2(&url)
    )
    .into_iter()
    .find(|c| c.token == "VSC_1")
    .expect("VSC_1 present on Media2");
    cfg.name = "rt-vsc2-4443".into();
    call!(
        "SetVideoSourceConfiguration",
        d.client.set_video_source_configuration_media2(&url, &cfg)
    );
    let name = call!(
        "GetVideoSourceConfigurations",
        d.client.get_video_source_configurations_media2(&url)
    )
    .into_iter()
    .find(|c| c.token == "VSC_1")
    .map(|c| c.name)
    .unwrap_or_default();
    cmp("rt-vsc2-4443".to_string(), name)
}

async fn m2_add_configuration(d: &Dev) -> Outcome {
    let url = d.url("media2");
    let token = call!(
        "CreateProfile",
        d.client.create_profile_media2(&url, "rt-4444")
    );
    call!(
        "AddConfiguration",
        d.client
            .add_configuration_media2(&url, &token, "VideoEncoder", "VEC_2")
    );
    let bound = call!("GetProfiles", d.client.get_profiles_media2(&url))
        .into_iter()
        .find(|p| p.token == token)
        .and_then(|p| p.video_encoder_token);
    cmp(Some("VEC_2".to_string()), bound)
}

/// `AddConfiguration(Type="PTZ")` on the one seeded profile that binds none.
///
/// It faulted with `UnmodelledConfigType-CFG2-5542` until the PTZ family was
/// wired, on the grounds that nothing could ever show the result. `Profile_4`
/// is the fixture for a profile that is deliberately **not** PTZ-capable, so
/// binding it here also proves the unbound state was a slot and not an absence.
async fn m2_add_ptz_configuration(d: &Dev) -> Outcome {
    let url = d.url("media2");
    call!(
        "AddConfiguration",
        d.client
            .add_configuration_media2(&url, "Profile_4", "PTZ", "PTZConfig_2")
    );
    let bound = call!("GetProfiles", d.client.get_profiles_media2(&url))
        .into_iter()
        .find(|p| p.token == "Profile_4")
        .and_then(|p| p.ptz_config_token);
    cmp(Some("PTZConfig_2".to_string()), bound)
}

async fn m2_remove_configuration(d: &Dev) -> Outcome {
    let url = d.url("media2");
    call!(
        "RemoveConfiguration",
        d.client
            .remove_configuration_media2(&url, "Profile_1", "VideoEncoder", "VEC_1")
    );
    let bound = call!("GetProfiles", d.client.get_profiles_media2(&url))
        .into_iter()
        .find(|p| p.token == "Profile_1")
        .and_then(|p| p.video_encoder_token);
    cmp(None, bound)
}

/// Writes `name` **and all three booleans**, each flipped away from what the
/// seed holds, and reads back the addressed configuration by token.
///
/// Asserting `name` alone would pass against a handler that stored the name
/// and dropped the filter flags — the `MTU` shape from the audit's §5c note,
/// where a partial write is worse than no write.
async fn m2_metadata_config(d: &Dev) -> Outcome {
    let url = d.url("media2");
    let mut cfg = call!(
        "GetMetadataConfigurations",
        d.client
            .get_metadata_configurations_media2(&url, Some("MetaConf_1"), None)
    )
    .into_iter()
    .next()
    .expect("MetaConf_1 exists");

    // Seed is analytics=true, ptz_status=false, ptz_position=true. Invert all
    // three so no field can be satisfied by the value already stored.
    cfg.name = "rt-meta-4446".into();
    cfg.analytics = false;
    cfg.ptz_status = true;
    cfg.ptz_position = false;
    call!(
        "SetMetadataConfiguration",
        d.client.set_metadata_configuration_media2(&url, &cfg)
    );

    let got = call!(
        "GetMetadataConfigurations",
        d.client
            .get_metadata_configurations_media2(&url, Some("MetaConf_1"), None)
    )
    .into_iter()
    .next();
    cmp(
        ("rt-meta-4446".to_string(), false, true, false),
        got.map(|c| (c.name, c.analytics, c.ptz_status, c.ptz_position))
            .unwrap_or_default(),
    )
}

/// The Media2 write, addressed at the *other* configuration — and the one
/// assertion only a cross-service read can make.
///
/// `tt:AudioEncoder2Configuration` has **no `SessionTimeout` member**, so a
/// Media2 write cannot express it. It must therefore leave the stored one
/// alone rather than clearing a value the Media1 type requires — and only
/// Media1's getter can show that, because Media2's response has nowhere to put
/// it.
async fn m2_audio_encoder_config(d: &Dev) -> Outcome {
    let url = d.url("media2");
    let mut cfg = call!(
        "GetAudioEncoderConfigurations",
        d.client.get_audio_encoder_configurations_media2(&url)
    )
    .into_iter()
    .find(|c| c.token == "AEC_2")
    .expect("AEC_2 exists");
    // Seed: AAC / 128 / 48, PT30S (which Media2 never sees).
    cfg.name = "rt-aec2-4461".into();
    cfg.bitrate = 47;
    cfg.sample_rate = 32;
    call!(
        "SetAudioEncoderConfiguration",
        d.client.set_audio_encoder_configuration_media2(&url, &cfg)
    );
    let got = call!(
        "GetAudioEncoderConfigurations",
        d.client.get_audio_encoder_configurations_media2(&url)
    )
    .into_iter()
    .find(|c| c.token == "AEC_2")
    .expect("AEC_2 still exists");
    let through_media1 = call!(
        "GetAudioEncoderConfiguration",
        d.client
            .get_audio_encoder_configuration(&d.url("media"), "AEC_2")
    );
    cmp(
        (
            "rt-aec2-4461".to_string(),
            47,
            32,
            "rt-aec2-4461".to_string(),
            Some("PT30S".to_string()),
        ),
        (
            got.name,
            got.bitrate,
            got.sample_rate,
            // Media1 must show the same write — one catalogue, two views.
            through_media1.name,
            through_media1.session_timeout,
        ),
    )
}

// ── PTZ checks ───────────────────────────────────────────────────────────────

async fn ptz_preset_set(d: &Dev) -> Outcome {
    let url = d.url("ptz");
    let token = call!(
        "SetPreset",
        d.client
            .ptz_set_preset(&url, "Profile_1", Some("rt-preset-4450"), None)
    );
    let got = call!("GetPresets", d.client.ptz_get_presets(&url, "Profile_1"));
    cmp(
        true,
        got.iter()
            .any(|p| p.token == token && p.name == "rt-preset-4450"),
    )
}

async fn ptz_preset_remove(d: &Dev) -> Outcome {
    let url = d.url("ptz");
    let token = call!(
        "SetPreset",
        d.client
            .ptz_set_preset(&url, "Profile_1", Some("rt-preset-4451"), None)
    );
    call!(
        "RemovePreset",
        d.client.ptz_remove_preset(&url, "Profile_1", &token)
    );
    let got = call!("GetPresets", d.client.ptz_get_presets(&url, "Profile_1"));
    cmp(false, got.iter().any(|p| p.token == token))
}

async fn ptz_absolute_move(d: &Dev) -> Outcome {
    let url = d.url("ptz");
    call!(
        "AbsoluteMove",
        d.client
            .ptz_absolute_move(&url, "Profile_1", 0.42, -0.17, 0.33)
    );
    let got = call!("GetStatus", d.client.ptz_get_status(&url, "Profile_1"));
    cmp(
        (Some(0.42), Some(-0.17), Some(0.33)),
        (got.pan, got.tilt, got.zoom),
    )
}

/// `SetHomePosition` stores wherever the device currently is, so this drives a
/// move first, stores it, moves away, and asks to go home.
async fn ptz_home_position(d: &Dev) -> Outcome {
    let url = d.url("ptz");
    call!(
        "AbsoluteMove",
        d.client
            .ptz_absolute_move(&url, "Profile_1", -0.55, 0.25, 0.61)
    );
    call!(
        "SetHomePosition",
        d.client.ptz_set_home_position(&url, "Profile_1")
    );
    call!(
        "AbsoluteMove",
        d.client.ptz_absolute_move(&url, "Profile_1", 0.0, 0.0, 0.0)
    );
    call!(
        "GotoHomePosition",
        d.client.ptz_goto_home_position(&url, "Profile_1", None)
    );
    let got = call!("GetStatus", d.client.ptz_get_status(&url, "Profile_1"));
    cmp(
        (Some(-0.55), Some(0.25), Some(0.61)),
        (got.pan, got.tilt, got.zoom),
    )
}

async fn ptz_preset_tour_create(d: &Dev) -> Outcome {
    let url = d.url("ptz");
    let token = call!(
        "CreatePresetTour",
        d.client.ptz_create_preset_tour(&url, "Profile_1")
    );
    let got = call!(
        "GetPresetTours",
        d.client.ptz_get_preset_tours(&url, "Profile_1")
    );
    cmp(
        true,
        got.iter()
            .any(|t| t.token.as_deref() == Some(token.as_str())),
    )
}

async fn ptz_preset_tour_remove(d: &Dev) -> Outcome {
    let url = d.url("ptz");
    let token = call!(
        "CreatePresetTour",
        d.client.ptz_create_preset_tour(&url, "Profile_1")
    );
    call!(
        "RemovePresetTour",
        d.client.ptz_remove_preset_tour(&url, "Profile_1", &token)
    );
    let got = call!(
        "GetPresetTours",
        d.client.ptz_get_preset_tours(&url, "Profile_1")
    );
    cmp(
        false,
        got.iter()
            .any(|t| t.token.as_deref() == Some(token.as_str())),
    )
}

/// Writes **every field `PtzConfiguration` can carry**, each moved away from
/// what the seed holds, plus one optional cleared to `None` and one assertion on
/// the configuration that was *not* addressed.
///
/// Asserting `name` alone — which is all this did while the row was `Static` —
/// would pass against a handler that stored the name and dropped the spaces, the
/// speed and the limits: the `MTU` shape from `CLAUDE.md` step 5c, where a
/// partial write is worse than no write. The cleared space covers the other
/// half: `SetConfiguration` *replaces* a configuration, so an element the
/// request omits must come back absent, not preserved. And `PTZConfig_2`'s
/// untouched name is what separates "the write landed" from "the write landed on
/// the configuration I addressed".
async fn ptz_configuration(d: &Dev) -> Outcome {
    let url = d.url("ptz");
    let mut cfg = call!(
        "GetConfiguration",
        d.client.ptz_get_configuration(&url, "PTZConfig_1")
    );
    // Seed: PT10S, speed (0.5, 0.5)/0.5, PanTiltLimits ±0.9 × ±0.7,
    // ZoomLimits 0.0–1.0, all six spaces set.
    cfg.name = "rt-ptzcfg-4456".into();
    // Re-point the configuration at the other head. `node_token` is the one
    // required child of `PTZConfiguration`, so leaving it at its seeded value
    // would let a handler that drops it entirely pass this probe — measured:
    // deleting the `node_token` write reddened nothing until this line changed.
    cfg.node_token = "PTZNode_2".into();
    cfg.default_ptz_timeout = Some("PT7S".into());
    cfg.default_ptz_speed = Some(oxvif::PtzSpeed {
        pan_tilt: Some((0.25, 0.75)),
        zoom: Some(0.9),
    });
    cfg.default_rel_zoom_space = None;
    if let Some(r) = cfg.pan_tilt_limits.as_mut() {
        r.x_range = (-0.8, 0.8);
        r.y_range = Some((-0.6, 0.6));
    }
    if let Some(r) = cfg.zoom_limits.as_mut() {
        r.x_range = (0.2, 0.9);
    }
    call!(
        "SetConfiguration",
        d.client.ptz_set_configuration(&url, &cfg, true)
    );
    let got = call!(
        "GetConfiguration",
        d.client.ptz_get_configuration(&url, "PTZConfig_1")
    );
    let other = call!(
        "GetConfiguration",
        d.client.ptz_get_configuration(&url, "PTZConfig_2")
    );
    cmp(
        (
            "rt-ptzcfg-4456".to_string(),
            "PTZNode_2".to_string(),
            Some("PT7S".to_string()),
            Some((0.25_f32, 0.75_f32)),
            Some(0.9_f32),
            None,
            Some(((-0.8_f32, 0.8_f32), Some((-0.6_f32, 0.6_f32)))),
            Some(((0.2_f32, 0.9_f32), None)),
            "PTZConfig_2".to_string(),
        ),
        (
            got.name,
            got.node_token,
            got.default_ptz_timeout,
            got.default_ptz_speed.as_ref().and_then(|s| s.pan_tilt),
            got.default_ptz_speed.as_ref().and_then(|s| s.zoom),
            got.default_rel_zoom_space,
            got.pan_tilt_limits.map(|r| (r.x_range, r.y_range)),
            got.zoom_limits.map(|r| (r.x_range, r.y_range)),
            other.name,
        ),
    )
}

// ── Imaging check ────────────────────────────────────────────────────────────

async fn imaging_settings(d: &Dev) -> Outcome {
    let url = d.url("imaging");
    let mut settings = call!(
        "GetImagingSettings",
        d.client.get_imaging_settings(&url, "VS_1")
    );
    settings.brightness = Some(44.0);
    settings.contrast = Some(61.0);
    call!(
        "SetImagingSettings",
        d.client.set_imaging_settings(&url, "VS_1", &settings)
    );
    let got: ImagingSettings = call!(
        "GetImagingSettings",
        d.client.get_imaging_settings(&url, "VS_1")
    );
    cmp((Some(44.0), Some(61.0)), (got.brightness, got.contrast))
}

// ── Recording checks ─────────────────────────────────────────────────────────

fn rt_recording_config(name: &str) -> RecordingConfiguration {
    RecordingConfiguration {
        source_name: name.into(),
        source_id: format!("urn:uuid:{name}"),
        location: "roundtrip".into(),
        description: "created by mock_roundtrip".into(),
        content: "Normal".into(),
        maximum_retention_time: "PT0S".into(),
    }
}

async fn recording_create(d: &Dev) -> Outcome {
    let url = d.url("recording");
    let token = call!(
        "CreateRecording",
        d.client
            .create_recording(&url, &rt_recording_config("rt-4460"))
    );
    let got = call!("GetRecordings", d.client.get_recordings(&url));
    cmp(true, got.iter().any(|r| r.token == token))
}

async fn recording_delete(d: &Dev) -> Outcome {
    let url = d.url("recording");
    call!(
        "DeleteRecording",
        d.client.delete_recording(&url, "Rec_001")
    );
    let got = call!("GetRecordings", d.client.get_recordings(&url));
    cmp(false, got.iter().any(|r| r.token == "Rec_001"))
}

async fn recording_track_create(d: &Dev) -> Outcome {
    let url = d.url("recording");
    let token = call!(
        "CreateTrack",
        d.client
            .create_track(&url, "Rec_001", "Audio", "rt-track-4462")
    );
    let got = call!("GetRecordings", d.client.get_recordings(&url));
    let present = got
        .iter()
        .find(|r| r.token == "Rec_001")
        .is_some_and(|r| r.tracks.iter().any(|t| t.token == token));
    cmp(true, present)
}

async fn recording_job_create(d: &Dev) -> Outcome {
    let url = d.url("recording");
    let cfg = RecordingJobConfiguration {
        recording_token: "Rec_001".into(),
        mode: "Idle".into(),
        priority: 3,
        source_token: "Profile_1".into(),
    };
    let token = call!(
        "CreateRecordingJob",
        d.client.create_recording_job(&url, &cfg)
    );
    let got = call!("GetRecordingJobs", d.client.get_recording_jobs(&url));
    cmp(true, got.iter().any(|j| j.token == token))
}

async fn recording_job_mode(d: &Dev) -> Outcome {
    let url = d.url("recording");
    call!(
        "SetRecordingJobMode",
        d.client.set_recording_job_mode(&url, "Job_001", "Idle")
    );
    let got = call!("GetRecordingJobs", d.client.get_recording_jobs(&url));
    let mode = got
        .iter()
        .find(|j| j.token == "Job_001")
        .map(|j| j.mode.clone())
        .unwrap_or_default();
    cmp("Idle".to_string(), mode)
}

// ── The test ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn every_get_set_pair_matches_its_declared_expectation() {
    // Guard on the guard: an empty or gutted table makes every assertion below
    // vacuously true.
    //
    // **This is an exact pin, not a floor, and that is deliberate.** The floor it
    // replaced (`>= 45`) let the true split drift away from every prose copy of
    // it — twice: the audit's §2 said "40 round-trip … 5 stubs" against an actual
    // 42, and after the metadata fix moved a row to `Works` the docs still said
    // "44 Works, 4 Static" against an actual 45/3. Nothing failed either time,
    // because a floor cannot notice a number that grew. Now it can.
    let declared_works = PAIRS
        .iter()
        .filter(|p| p.expect.should_round_trip())
        .count();
    assert_eq!(
        (PAIRS.len(), declared_works),
        (49, 49),
        "the pair table's shape changed (rows, declared-Works). If that was \
         deliberate, update this expectation **and** the counts in \
         docs/mock-server.md §12 and docs/active/mock-audit-2026-07.md §2 in the \
         same commit — they are the two places that quote it.",
    );

    let mut mismatches = Vec::new();
    let mut failures = Vec::new();
    let mut round_tripped = 0usize;

    for pair in PAIRS {
        let dev = Dev::new().await;
        let outcome = (pair.run)(&dev).await;

        match (&outcome, pair.expect.should_round_trip()) {
            (Outcome::RoundTripped, true) => round_tripped += 1,
            (Outcome::Discarded(_), false) => {}
            (Outcome::RoundTripped, false) => mismatches.push(format!(
                "{}: declared {} but the write now round-trips.\n      \
                 If you just wired this up, move the row to `Expect::Works` \
                 (and strike it from docs/active/mock-audit-2026-07.md).",
                pair.name,
                pair.expect.label(),
            )),
            (Outcome::Discarded(detail), true) => mismatches.push(format!(
                "{}: declared Works but the write was discarded — {detail}",
                pair.name,
            )),
            (Outcome::Failed(detail), _) => failures.push(format!("{}: {detail}", pair.name)),
        }
    }

    // A row whose *call* errors proves nothing about whether the write landed,
    // so it can never satisfy a `Broken` or `Static` expectation by accident.
    assert!(
        failures.is_empty(),
        "{} pair(s) errored rather than answering the round-trip question:\n  {}",
        failures.len(),
        failures.join("\n  "),
    );
    assert!(
        mismatches.is_empty(),
        "{} of {} Set/Get pairs disagree with the table:\n  {}",
        mismatches.len(),
        PAIRS.len(),
        mismatches.join("\n  "),
    );

    // And the positive side is not vacuous either: every row declared `Works`
    // actually round-tripped. (`mismatches` already covers this row by row; this
    // catches a counting bug in the loop itself.)
    assert_eq!(
        round_tripped, declared_works,
        "{round_tripped} pairs round-tripped but {declared_works} rows declare \
         Works — the tally and the table disagree",
    );
}
