//! In-memory mock device state.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::RwLock;

// ── Device State ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceState {
    #[serde(default = "default_device_info")]
    pub info: DeviceInfo,
    #[serde(default = "default_hostname")]
    pub hostname: String,
    #[serde(default)]
    pub hostname_from_dhcp: bool,
    #[serde(default = "default_users")]
    pub users: Vec<MockUser>,
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,
    #[serde(default = "default_tz")]
    pub timezone: String,
    #[serde(default)]
    pub daylight_savings: bool,
    #[serde(default = "default_dns")]
    pub dns: DnsState,
    #[serde(default = "default_ntp")]
    pub ntp: NtpState,
    #[serde(default = "default_gateway")]
    pub gateway_ipv4: Vec<String>,
    #[serde(default = "default_discovery_mode")]
    pub discovery_mode: String,
    #[serde(default = "default_imaging_sources")]
    pub imaging_sources: Vec<ImagingState>,
    #[serde(default = "default_ptz")]
    pub ptz: PtzState,
    #[serde(default = "default_interface")]
    pub interface: NetworkInterfaceState,
    #[serde(default = "default_protocols")]
    pub protocols: Vec<NetworkProtocolState>,
    #[serde(default = "default_osd")]
    pub osd: OsdState,
    #[serde(default = "default_profiles")]
    pub profiles: ProfilesState,
    #[serde(default = "default_recording")]
    pub recording: RecordingState,
    #[serde(default = "default_video_sources")]
    pub video_sources: Vec<VideoSourceEntry>,
    #[serde(default = "default_video_source_configs")]
    pub video_source_configs: Vec<VideoSourceConfigEntry>,
    #[serde(default = "default_video_encoders")]
    pub video_encoders: Vec<VideoEncoderState>,
    #[serde(default = "default_relay_outputs")]
    pub relay_outputs: Vec<RelayOutputState>,
    #[serde(default = "default_digital_inputs")]
    pub digital_inputs: Vec<DigitalInputState>,
    #[serde(default = "default_storage")]
    pub storage: Vec<StorageEntry>,
    /// Monotonic event counter for the pull-point stream (per-instance,
    /// not persisted). Replaces the former process-global `EVENT_SEQ`.
    #[serde(skip)]
    pub event_seq: u64,
    /// Active pull-point topic filter, set by CreatePullPointSubscription
    /// (per-instance, not persisted). `None` = emit every topic.
    #[serde(skip)]
    pub event_filter: Option<Vec<String>>,
    /// Pending events emitted out-of-band (e.g. by the
    /// `/mock/digital-input/...` simulator endpoint) and surfaced on the
    /// next `PullMessages` call. Per-instance, not persisted.
    #[serde(skip)]
    pub pending_io_events: Vec<PendingIoEvent>,
}

// ── Relay output / Digital input ──────────────────────────────────────────────
//
// Real cameras expose 0–2 of each on the Device service. The mock seeds two
// of each so callers exercise both single-port and multi-port flows. The
// `logical_state` field on `RelayOutputState` is what `SetRelayOutputState`
// writes (active/inactive); `GetRelayOutputs` by spec doesn't return it,
// but the mock keeps it for event emission and for tests that want to
// assert the latched state after a Set call.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayOutputState {
    pub token: String,
    /// `"Bistable"` (latching) or `"Monostable"` (timed pulse).
    pub mode: String,
    /// ISO 8601 duration for monostable mode, e.g. `"PT1S"`.
    pub delay_time: String,
    /// Idle electrical state: `"closed"` or `"open"`.
    pub idle_state: String,
    /// Current logical state — `"active"` or `"inactive"`. Updated by
    /// `SetRelayOutputState`. The mock's monostable handler does NOT
    /// auto-revert (a real device would, but that needs a timer the
    /// mock doesn't run); callers wanting that flow should use the
    /// `/mock/relay-output/.../pulse` REST hook.
    #[serde(default = "default_logical_inactive")]
    pub logical_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigitalInputState {
    pub token: String,
    /// Idle electrical state: `"closed"` or `"open"`. Empty when the
    /// firmware omits the attribute.
    pub idle_state: String,
    /// Current logical state — `"active"` or `"inactive"`. Flipped by
    /// the `/mock/digital-input/.../pulse|set` REST endpoint to simulate
    /// sensor signals. Drives PullPoint event emission.
    #[serde(default = "default_logical_inactive")]
    pub logical_state: String,
}

// ── Storage ───────────────────────────────────────────────────────────────────

/// One storage location, as returned by `GetStorageConfigurations` and
/// written by `SetStorageConfiguration`.
///
/// The field set is exactly what `crate::types::StorageConfiguration` parses,
/// so every field the client can read is a field the mock can store. Before
/// 0.15 this whole family was a single static fixture that emitted `token`,
/// `Data/@type` and `LocalPath` only — `storage_uri` and `user` were the
/// "storage credential fields" of the audit's Tier 4.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageEntry {
    pub token: String,
    /// `"LocalStorage"`, `"NFS"`, `"CIFS"` — the `type` attribute of `Data`.
    pub storage_type: String,
    /// Mount path on the device. Empty for a share addressed only by URI.
    pub local_path: String,
    /// Network URI for NFS / CIFS shares. Empty for local storage.
    pub storage_uri: String,
    /// Username for an authenticated share. Empty when none is configured.
    ///
    /// The password is **deliberately not modelled**. `tt:UserCredential` has a
    /// `Password` element, but `StorageConfiguration` does not parse it and no
    /// getter could return it, so storing it would be write-only state that
    /// nothing can observe — and a mock that holds a password it never uses is
    /// a worse default than one that visibly does not.
    pub user: String,
}

/// One-shot event emitted by the IO simulator endpoint and consumed by
/// the next `PullMessages` call. Distinct from the regular periodic
/// event stream (`event_seq`) — these are demand-driven.
#[derive(Debug, Clone)]
pub struct PendingIoEvent {
    /// `"DigitalInput"` or `"RelayOutput"`. Maps to the
    /// `tns1:Device/Trigger/{this}` PullPoint topic.
    pub kind: &'static str,
    pub token: String,
    /// `"active"` or `"inactive"`.
    pub logical_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterfaceState {
    pub token: String,
    pub name: String,
    pub mac: String,
    pub mtu: u32,
    pub enabled: bool,
    pub ipv4_from_dhcp: bool,
    pub ipv4_address: String,
    pub ipv4_prefix_length: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkProtocolState {
    pub name: String,
    pub enabled: bool,
    pub ports: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub firmware_version: String,
    pub serial_number: String,
    pub hardware_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockUser {
    pub username: String,
    pub level: String,
    /// Plaintext password used to validate WS-Security digests.
    /// `#[serde(default)]` keeps older state files (pre-per-user-auth)
    /// loadable; those users get a blank password until re-set.
    #[serde(default)]
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsState {
    pub from_dhcp: bool,
    pub servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NtpState {
    pub from_dhcp: bool,
    pub servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzPreset {
    pub token: String,
    pub name: String,
    pub pan: f32,
    pub tilt: f32,
    pub zoom: f32,
}

/// One stop on a mock preset tour. `preset_token` is the only stop kind the
/// mock stores — `Home` and explicit positions parse fine on the client side
/// but are not something this device offers, and inventing them here would
/// make the mock claim a capability it does not honour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzTourSpot {
    pub preset_token: String,
    pub stay_time: String,
}

/// A stored preset tour. Created empty by `CreatePresetTour`, filled in by
/// `ModifyPresetTour`, and walked by `OperatePresetTour` — which moves `state`
/// only, since the mock has no clock to tour on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzTour {
    pub token: String,
    pub name: String,
    /// `Idle`, `Touring` or `Paused`.
    pub state: String,
    pub auto_start: bool,
    pub random_preset_order: bool,
    pub recurring_time: Option<u32>,
    pub direction: String,
    pub spots: Vec<PtzTourSpot>,
}

/// PTZ state for **one** media profile.
///
/// Every PTZ operation that moves or reads the head takes a `ProfileToken`, so
/// on a multi-head device there is one of these per profile and none of them is
/// "the device's" position. Same reasoning as [`ImagingState`] and the video
/// encoder catalogue: with a single global copy, a handler that ignores the
/// token is indistinguishable from one that reads it, and every test of
/// "does my code address the right head?" passes against a mock that never
/// looked.
///
/// Until 0.15 this *was* the whole of `PtzState` — one position, one preset
/// list, one tour list for the entire device — and **26 of 27 PTZ dispatch arms
/// never received the request body at all**, while the client sent
/// `ProfileToken` at 20 call sites. `docs/active/mock-audit-2026-07.md` §4.1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzChannel {
    pub pan: f32,
    pub tilt: f32,
    pub zoom: f32,
    pub home_pan: f32,
    pub home_tilt: f32,
    pub home_zoom: f32,
    pub presets: Vec<PtzPreset>,
    #[serde(default)]
    pub tours: Vec<PtzTour>,
}

impl Default for PtzChannel {
    /// A head parked at the origin with nothing stored. This is what a profile
    /// with no seeded channel gets on first use.
    fn default() -> Self {
        Self {
            pan: 0.0,
            tilt: 0.0,
            zoom: 0.0,
            home_pan: 0.0,
            home_tilt: 0.0,
            home_zoom: 0.0,
            presets: Vec::new(),
            tours: Vec::new(),
        }
    }
}

/// The device's PTZ heads, keyed by media profile token.
///
/// `BTreeMap` rather than `HashMap` so a serialised snapshot is stable.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PtzState {
    #[serde(default = "default_ptz_channels")]
    pub channels: BTreeMap<String, PtzChannel>,
}

impl PtzState {
    /// The channel for `profile`, or `None` if the profile has never been
    /// touched and was not seeded. Read paths render an empty head rather than
    /// faulting — a profile with no presets is a legitimate device state.
    pub fn channel(&self, profile: &str) -> Option<&PtzChannel> {
        self.channels.get(profile)
    }

    /// The channel for `profile`, created empty if absent. Write paths use this
    /// so a seeded state that lists profiles but no PTZ channels still works.
    pub fn channel_mut(&mut self, profile: &str) -> &mut PtzChannel {
        self.channels.entry(profile.to_string()).or_default()
    }
}

/// Imaging state for **one** video source.
///
/// Every operation in the Imaging service takes a `VideoSourceToken`, so on a
/// multi-sensor device there is one of these per lens and none of them is the
/// device's answer. The mock holds a `Vec` for exactly the reason the video
/// encoder catalogue does: with a single entry, a responder that ignores the
/// token is indistinguishable from one that reads it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagingState {
    /// Which sensor these settings belong to (`VS_1` / `VS_2`).
    #[serde(default = "default_source_token")]
    pub source_token: String,
    pub brightness: f32,
    pub color_saturation: f32,
    pub contrast: f32,
    pub sharpness: f32,
    pub exposure_mode: String,
    pub white_balance_mode: String,
    pub backlight_compensation: String,
    pub wide_dynamic_range_mode: String,
    pub wide_dynamic_range_level: f32,
    pub ir_cut_filter: String,
    pub focus_mode: String,
    /// Whether this lens has a motorised focus.
    ///
    /// A dual-sensor camera commonly pairs one motorised lens with one fixed
    /// one, and the fixed one **faults** on `GetMoveOptions` / `Move` / `Stop`
    /// rather than reporting a focus range it does not have. That asymmetry is
    /// what lets a test tell "the device supports focus" apart from "*this
    /// channel* supports focus" — a distinction a single-sensor fixture cannot
    /// express at all.
    #[serde(default = "default_true")]
    pub focus_supported: bool,
    /// Upper bound this sensor reports for brightness/saturation/contrast/
    /// sharpness in `GetOptions`.
    ///
    /// Vendors genuinely differ per sensor here (0–100 on one, 0–255 on
    /// another), and it gives the per-channel options test a single number to
    /// assert rather than a structural difference.
    #[serde(default = "default_level_max")]
    pub level_max: f32,
}

// ── OSD state ───────────────────────────────────────────────────────────────
//
// OSDs are persisted by `(token, video_source_config_token)`. The mock
// advertises per-type quotas in `GetOSDOptions` (Genetec/late-Hikvision
// shape) and enforces them in `CreateOSD` — over-limit returns
// `ter:InvalidArgs`. This lets clients exercise their quota-gate UI
// against the mock instead of waiting for real-camera failures.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsdState {
    pub osds: Vec<OsdEntry>,
    /// Counter for tokens. Persists across restarts so deleted tokens
    /// don't get reused (matches what real cameras do).
    #[serde(default)]
    pub next_token_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsdEntry {
    pub token: String,
    pub video_source_config_token: String,
    /// `"Text"` or `"Image"`.
    pub osd_type: String,
    /// `"UpperLeft"`, `"UpperRight"`, `"LowerLeft"`, `"LowerRight"`,
    /// or `"Custom"` (uses `position_x`/`position_y`).
    pub position_type: String,
    #[serde(default)]
    pub position_x: Option<f32>,
    #[serde(default)]
    pub position_y: Option<f32>,
    /// Text-OSD payload — `Some` when `osd_type == "Text"`.
    #[serde(default)]
    pub text: Option<OsdTextEntry>,
    /// Image-OSD URL — `Some` when `osd_type == "Image"`.
    #[serde(default)]
    pub image_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsdTextEntry {
    /// `"Plain"`, `"Date"`, `"Time"`, or `"DateAndTime"`.
    pub text_type: String,
    #[serde(default)]
    pub plain_text: Option<String>,
    #[serde(default)]
    pub date_format: Option<String>,
    #[serde(default)]
    pub time_format: Option<String>,
    #[serde(default)]
    pub font_size: Option<u32>,
    #[serde(default)]
    pub font_color: Option<OsdColorEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsdColorEntry {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    #[serde(default)]
    pub colorspace: Option<String>,
    #[serde(default)]
    pub transparent: Option<f32>,
}

/// Per-text-type OSD quotas. Matches what the mock advertises in
/// `GetOSDOptionsResponse`. `CreateOSD` enforces these — over-limit
/// returns a `ter:InvalidArgs` SOAP fault, mirroring Genetec's
/// behaviour.
pub const OSD_QUOTA_TOTAL: u32 = 8;
pub const OSD_QUOTA_PLAIN: u32 = 7;
pub const OSD_QUOTA_DATE: u32 = 1;
pub const OSD_QUOTA_TIME: u32 = 1;
pub const OSD_QUOTA_DATE_AND_TIME: u32 = 1;

// ── Media profile state ─────────────────────────────────────────────────────
//
// Tracks the camera's media profile list. Real cameras seed two or
// three "fixed" profiles (mainStream / subStream / sometimes thirdStream)
// that can't be deleted, plus any user-created ones. The actual
// configuration objects (VSC, VEC, etc.) referenced by the profile
// stay hardcoded in `services/media.rs`'s render helpers — only the
// attachment (which token is bound to which profile) is mutable.
//
// `CreateProfile` adds an entry with no configurations attached, matching
// real-camera behaviour where the caller follows up with
// `AddVideoSourceConfiguration` etc. to fill it in. `DeleteProfile`
// refuses to remove `fixed=true` profiles (per ONVIF spec — returns
// `ter:DeletionOfFixedProfile`).

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilesState {
    pub profiles: Vec<ProfileEntry>,
    /// Counter for generated tokens. Persists so deleted profile
    /// tokens don't get reused.
    #[serde(default)]
    pub next_token_id: u32,
}

// ── Recording / Search / Replay ─────────────────────────────────────────────
//
// Profile G, modelled on `ProfilesState` above for the same reason.
//
// Until 0.15 `grep -c recording src/mock/state.rs` was **0**: every one of the
// eleven Recording operations was a static fixture, so `CreateRecording`
// answered `Rec_new` and `GetRecordings` never listed it — the identical shape
// to the reported Media2 `CreateProfile` bug, in a different service.
// `docs/active/mock-audit-2026-07.md` §4.2.
//
// The consequence went past the mock — see the header of
// `src/mock/services/recording.rs` for what the health check's Profile G
// liveness chain was and was not proving against a fixture.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingState {
    pub recordings: Vec<RecordingEntry>,
    pub jobs: Vec<RecordingJobEntry>,
    /// Counters for generated tokens, so a deleted token is never reused.
    #[serde(default)]
    pub next_recording_id: u32,
    #[serde(default)]
    pub next_track_id: u32,
    #[serde(default)]
    pub next_job_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingEntry {
    pub token: String,
    pub source_id: String,
    pub source_name: String,
    pub location: String,
    pub description: String,
    pub content: String,
    pub maximum_retention_time: String,
    #[serde(default)]
    pub tracks: Vec<RecordingTrackEntry>,
    /// Bounds reported by `FindRecordings` / `GetRecordingSearchResults`.
    pub earliest: String,
    pub latest: String,
    /// `Initiated`, `Recording` or `Stopped`.
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingTrackEntry {
    pub token: String,
    /// `Video`, `Audio` or `Metadata`.
    pub track_type: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingJobEntry {
    pub token: String,
    pub recording_token: String,
    /// `Active` or `Idle`.
    pub mode: String,
    pub priority: u32,
    /// Media profile token this job records from.
    pub source_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileEntry {
    pub token: String,
    pub name: String,
    /// `true` for factory-baked profiles that can't be deleted.
    pub fixed: bool,
    #[serde(default)]
    pub video_source_config_token: Option<String>,
    #[serde(default)]
    pub video_encoder_config_token: Option<String>,
    #[serde(default)]
    pub audio_source_config_token: Option<String>,
    #[serde(default)]
    pub audio_encoder_config_token: Option<String>,
}

// ── Video source / encoder state ──────────────────────────────────────────────
//
// The mock is a **two-sensor** device: one ONVIF endpoint, two physical lenses,
// two streams each. That is not decoration. Per the multi-sensor rule in
// CLAUDE.md, every `Get…Options` / `Get…Configuration` answer depends on which
// channel was asked about, and a single-sensor fixture cannot tell a parser
// that reads the token from one that ignores it — both pass.
//
// The numbers below are transcribed from a real two-sensor device measured
// 2026-07-28. The load-bearing property is that the sensors **disagree**:
// VS_2 tops out at 1280x720, while VS_1 goes to 2592x1944. So VEC_1's current
// 1920x1080 does not appear anywhere in VEC_3's option list, and a responder
// that returns lens 0's list for a lens 1 query fails an assertion instead of
// silently answering for the wrong channel.

/// One physical sensor (`trt:GetVideoSources`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoSourceEntry {
    pub token: String,
    pub framerate: u32,
    pub width: u32,
    pub height: u32,
}

/// A video *source* configuration — the crop/bounds view onto one sensor.
/// `source_token` is what ties it back to a [`VideoSourceEntry`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoSourceConfigEntry {
    pub token: String,
    pub name: String,
    pub use_count: u32,
    pub source_token: String,
    pub width: u32,
    pub height: u32,
}

/// One video encoder configuration — a (sensor, stream) pair.
///
/// `GetVideoEncoderConfigurations` renders from here and
/// `SetVideoEncoderConfiguration` persists into it, so a Set → Get roundtrip
/// reflects the change. Uses Media2's flat, H.265-capable shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoEncoderState {
    pub token: String,
    pub name: String,
    pub use_count: u32,
    /// `"H264"`, `"H265"`, or `"JPEG"`.
    pub encoding: String,
    pub width: u32,
    pub height: u32,
    pub quality: f32,
    pub frame_rate_limit: u32,
    pub bitrate_limit: u32,
    pub gov_length: u32,
    pub profile: String,
    /// Which sensor this encoder draws from (`VS_1` / `VS_2`).
    #[serde(default = "default_source_token")]
    pub source_token: String,
    /// The resolutions `GetVideoEncoderConfigurationOptions` reports **for this
    /// channel**. `(width, height)`, widest first.
    ///
    /// The invariant that makes this list worth having: `(width, height)` above
    /// must appear in it. A device that offers a resolution it is not running,
    /// or runs one it does not offer, is a bug the mock should not model by
    /// accident — `default_video_encoders_are_self_consistent` asserts it.
    #[serde(default)]
    pub resolutions: Vec<(u32, u32)>,
}

// ── Defaults ────────────────────────────────────────────────────────────────

fn default_device_info() -> DeviceInfo {
    DeviceInfo {
        manufacturer: "oxvif-mock".into(),
        model: "MockCam-1080p".into(),
        firmware_version: "1.0.0".into(),
        serial_number: "MOCK-0001".into(),
        hardware_id: "1.0".into(),
    }
}
fn default_hostname() -> String {
    "mock-camera".into()
}
fn default_users() -> Vec<MockUser> {
    vec![
        MockUser {
            username: "admin".into(),
            level: "Administrator".into(),
            password: "admin".into(),
        },
        MockUser {
            username: "operator".into(),
            level: "Operator".into(),
            password: "operator".into(),
        },
    ]
}
fn default_scopes() -> Vec<String> {
    vec![
        "onvif://www.onvif.org/name/MockCamera".into(),
        "onvif://www.onvif.org/type/video_encoder".into(),
        "onvif://www.onvif.org/location/country/taiwan".into(),
    ]
}
fn default_tz() -> String {
    "UTC".into()
}
fn default_dns() -> DnsState {
    DnsState {
        from_dhcp: false,
        servers: vec!["8.8.8.8".into(), "8.8.4.4".into()],
    }
}
fn default_ntp() -> NtpState {
    NtpState {
        from_dhcp: false,
        servers: vec!["pool.ntp.org".into()],
    }
}
fn default_gateway() -> Vec<String> {
    vec!["192.168.1.1".into()]
}
fn default_interface() -> NetworkInterfaceState {
    NetworkInterfaceState {
        token: "eth0".into(),
        name: "eth0".into(),
        mac: "00:11:22:33:44:55".into(),
        mtu: 1500,
        enabled: true,
        ipv4_from_dhcp: false,
        ipv4_address: "192.168.1.100".into(),
        ipv4_prefix_length: 24,
    }
}
fn default_protocols() -> Vec<NetworkProtocolState> {
    vec![
        NetworkProtocolState {
            name: "HTTP".into(),
            enabled: true,
            ports: vec![80],
        },
        NetworkProtocolState {
            name: "HTTPS".into(),
            enabled: true,
            ports: vec![443],
        },
        NetworkProtocolState {
            name: "RTSP".into(),
            enabled: true,
            ports: vec![554],
        },
    ]
}
fn default_discovery_mode() -> String {
    "Discoverable".into()
}
fn default_ptz() -> PtzState {
    PtzState {
        channels: default_ptz_channels(),
    }
}

fn preset(token: &str, name: &str, pan: f32, tilt: f32, zoom: f32) -> PtzPreset {
    PtzPreset {
        token: token.into(),
        name: name.into(),
        pan,
        tilt,
        zoom,
    }
}

/// **The four heads deliberately disagree.**
///
/// `CLAUDE.md` — "a single-sensor fixture cannot cover a per-channel feature":
/// a fixture whose channels give the same answer is passed just as well by a
/// handler that ignores the token entirely. So these differ in *position*, in
/// *preset count*, in *preset names*, and in *whether tours exist at all* —
/// every one of those is something an assertion can read.
///
/// Measured before this existed: `ptz_get_status(Profile_1 vs Profile_3)` gave
/// `pan Some(0.77)` both times, and `ptz_get_presets` gave `2 vs 2`.
///
/// `Profile_4` is left completely empty on purpose. An empty preset list is a
/// legitimate device state and the only fixture that can catch a renderer which
/// substitutes a default when it finds nothing.
fn default_ptz_channels() -> BTreeMap<String, PtzChannel> {
    BTreeMap::from([
        (
            "Profile_1".to_string(),
            PtzChannel {
                presets: vec![
                    preset("Preset_1", "Home", 0.0, 0.0, 0.0),
                    preset("Preset_2", "Door", 0.5, 0.2, 0.0),
                ],
                tours: default_tours(),
                ..PtzChannel::default()
            },
        ),
        (
            "Profile_2".to_string(),
            PtzChannel {
                pan: 0.25,
                tilt: -0.10,
                zoom: 0.40,
                presets: vec![preset("Preset_1", "Gate", 0.25, -0.10, 0.40)],
                ..PtzChannel::default()
            },
        ),
        (
            "Profile_3".to_string(),
            PtzChannel {
                pan: -0.60,
                tilt: 0.35,
                zoom: 0.80,
                presets: vec![
                    preset("Preset_1", "Lobby", -0.60, 0.35, 0.80),
                    preset("Preset_2", "Dock", -0.20, 0.05, 0.10),
                    preset("Preset_3", "Roof", 0.90, -0.45, 1.0),
                ],
                ..PtzChannel::default()
            },
        ),
        ("Profile_4".to_string(), PtzChannel::default()),
    ])
}

/// One tour, with **two** stops. A single-stop fixture cannot tell a parser
/// that returns the first spot and drops the rest from one that collects them
/// all, which is the specific defect the `vec_from_xml` rule exists to catch.
fn default_tours() -> Vec<PtzTour> {
    vec![PtzTour {
        token: "Tour_1".into(),
        name: "Perimeter".into(),
        state: "Idle".into(),
        auto_start: false,
        random_preset_order: false,
        recurring_time: Some(3),
        direction: "Forward".into(),
        spots: vec![
            PtzTourSpot {
                preset_token: "Preset_1".into(),
                stay_time: "PT10S".into(),
            },
            PtzTourSpot {
                preset_token: "Preset_2".into(),
                stay_time: "PT20S".into(),
            },
        ],
    }]
}
fn default_osd() -> OsdState {
    OsdState {
        osds: vec![OsdEntry {
            token: "OSD_1".into(),
            video_source_config_token: "VSC_1".into(),
            osd_type: "Text".into(),
            position_type: "UpperLeft".into(),
            position_x: None,
            position_y: None,
            text: Some(OsdTextEntry {
                text_type: "DateAndTime".into(),
                plain_text: None,
                date_format: Some("MM/dd/yyyy".into()),
                time_format: Some("HH:mm:ss".into()),
                font_size: Some(20),
                font_color: None,
            }),
            image_path: None,
        }],
        next_token_id: 2,
    }
}

/// Two recordings and **two** jobs, deliberately disagreeing.
///
/// `Rec_001` carries a track and `Rec_002` does not, so a renderer that emits a
/// fixed track list cannot satisfy both. `Job_001` is `Active` and `Job_002` is
/// `Idle`, which is what lets `GetRecordingJobState` be a per-token question at
/// all — with one job it is indistinguishable from a constant.
fn default_recording() -> RecordingState {
    RecordingState {
        recordings: vec![
            RecordingEntry {
                token: "Rec_001".into(),
                source_id: "rtsp://mock/live".into(),
                source_name: "MockCamera".into(),
                location: "Lab".into(),
                description: "Mock recording".into(),
                content: "Normal".into(),
                maximum_retention_time: "PT0S".into(),
                tracks: vec![RecordingTrackEntry {
                    token: "VIDEO001".into(),
                    track_type: "Video".into(),
                    description: "videoTrack".into(),
                }],
                earliest: "2026-01-01T00:00:00Z".into(),
                latest: "2026-04-01T00:00:00Z".into(),
                status: "Stopped".into(),
            },
            RecordingEntry {
                token: "Rec_002".into(),
                source_id: String::new(),
                source_name: "MockCamera".into(),
                location: String::new(),
                description: String::new(),
                content: String::new(),
                maximum_retention_time: "PT0S".into(),
                tracks: Vec::new(),
                earliest: "2026-05-01T00:00:00Z".into(),
                latest: "2026-06-01T00:00:00Z".into(),
                status: "Recording".into(),
            },
        ],
        jobs: vec![
            RecordingJobEntry {
                token: "Job_001".into(),
                recording_token: "Rec_001".into(),
                mode: "Active".into(),
                priority: 1,
                source_token: "Profile_1".into(),
            },
            RecordingJobEntry {
                token: "Job_002".into(),
                recording_token: "Rec_002".into(),
                mode: "Idle".into(),
                priority: 2,
                source_token: "Profile_3".into(),
            },
        ],
        next_recording_id: 3,
        next_track_id: 2,
        next_job_id: 3,
    }
}

fn default_profiles() -> ProfilesState {
    ProfilesState {
        profiles: vec![
            ProfileEntry {
                token: "Profile_1".into(),
                name: "mainStream".into(),
                fixed: true,
                video_source_config_token: Some("VSC_1".into()),
                video_encoder_config_token: Some("VEC_1".into()),
                audio_source_config_token: None,
                audio_encoder_config_token: None,
            },
            ProfileEntry {
                token: "Profile_2".into(),
                name: "subStream".into(),
                fixed: false,
                video_source_config_token: Some("VSC_1".into()),
                video_encoder_config_token: Some("VEC_2".into()),
                audio_source_config_token: None,
                audio_encoder_config_token: None,
            },
            // Sensor 2. Present so the second lens is reachable the way a
            // client actually reaches one — via a profile — and not only by
            // guessing configuration tokens.
            ProfileEntry {
                token: "Profile_3".into(),
                name: "mainStream2".into(),
                fixed: true,
                video_source_config_token: Some("VSC_2".into()),
                video_encoder_config_token: Some("VEC_3".into()),
                audio_source_config_token: None,
                audio_encoder_config_token: None,
            },
            ProfileEntry {
                token: "Profile_4".into(),
                name: "subStream2".into(),
                fixed: false,
                video_source_config_token: Some("VSC_2".into()),
                video_encoder_config_token: Some("VEC_4".into()),
                audio_source_config_token: None,
                audio_encoder_config_token: None,
            },
        ],
        next_token_id: 5,
    }
}

fn default_source_token() -> String {
    "VS_1".into()
}

fn default_video_sources() -> Vec<VideoSourceEntry> {
    vec![
        VideoSourceEntry {
            token: "VS_1".into(),
            framerate: 25,
            width: 2592,
            height: 1944,
        },
        VideoSourceEntry {
            token: "VS_2".into(),
            framerate: 25,
            width: 1280,
            height: 720,
        },
    ]
}

fn default_video_source_configs() -> Vec<VideoSourceConfigEntry> {
    vec![
        VideoSourceConfigEntry {
            token: "VSC_1".into(),
            name: "VSConfig1".into(),
            use_count: 2,
            source_token: "VS_1".into(),
            width: 2592,
            height: 1944,
        },
        VideoSourceConfigEntry {
            token: "VSC_2".into(),
            name: "VSConfig2".into(),
            use_count: 2,
            source_token: "VS_2".into(),
            width: 1280,
            height: 720,
        },
    ]
}

/// Four encoder configs: two sensors x (main, sub).
///
/// See the section comment above [`VideoEncoderState`] for why the two sensors
/// deliberately disagree about what they can do.
fn default_video_encoders() -> Vec<VideoEncoderState> {
    vec![
        // ── Sensor 1 (VS_1, 5MP) ────────────────────────────────────────────
        VideoEncoderState {
            token: "VEC_1".into(),
            name: "MainStream".into(),
            use_count: 1,
            encoding: "H264".into(),
            width: 1920,
            height: 1080,
            quality: 5.0,
            frame_rate_limit: 25,
            bitrate_limit: 4096,
            gov_length: 25,
            profile: "Main".into(),
            source_token: "VS_1".into(),
            resolutions: vec![
                (2592, 1944),
                (2592, 1520),
                (2560, 1440),
                (2304, 1296),
                (1920, 1080),
                (1280, 720),
            ],
        },
        VideoEncoderState {
            token: "VEC_2".into(),
            name: "SubStream".into(),
            use_count: 1,
            encoding: "H264".into(),
            width: 704,
            height: 480,
            quality: 4.0,
            frame_rate_limit: 15,
            bitrate_limit: 1024,
            gov_length: 25,
            profile: "Main".into(),
            source_token: "VS_1".into(),
            resolutions: vec![(1280, 720), (704, 480), (352, 240)],
        },
        // ── Sensor 2 (VS_2, 720p) ───────────────────────────────────────────
        VideoEncoderState {
            token: "VEC_3".into(),
            name: "MainStream2".into(),
            use_count: 1,
            encoding: "H264".into(),
            width: 1280,
            height: 720,
            quality: 5.0,
            frame_rate_limit: 25,
            bitrate_limit: 2048,
            gov_length: 25,
            profile: "High".into(),
            source_token: "VS_2".into(),
            resolutions: vec![(1280, 720), (704, 480), (480, 240), (352, 240)],
        },
        VideoEncoderState {
            token: "VEC_4".into(),
            name: "SubStream2".into(),
            use_count: 1,
            encoding: "JPEG".into(),
            width: 704,
            height: 480,
            quality: 3.0,
            frame_rate_limit: 10,
            bitrate_limit: 512,
            gov_length: 25,
            profile: "Baseline".into(),
            source_token: "VS_2".into(),
            resolutions: vec![(704, 480), (480, 240), (352, 240)],
        },
    ]
}

fn default_logical_inactive() -> String {
    "inactive".into()
}

fn default_relay_outputs() -> Vec<RelayOutputState> {
    vec![
        RelayOutputState {
            token: "RelayOutput_1".into(),
            mode: "Bistable".into(),
            delay_time: "PT0S".into(),
            idle_state: "closed".into(),
            logical_state: "inactive".into(),
        },
        RelayOutputState {
            token: "RelayOutput_2".into(),
            mode: "Monostable".into(),
            delay_time: "PT1S".into(),
            idle_state: "open".into(),
            logical_state: "inactive".into(),
        },
    ]
}

fn default_digital_inputs() -> Vec<DigitalInputState> {
    vec![
        DigitalInputState {
            token: "DigitalInput_1".into(),
            idle_state: "closed".into(),
            logical_state: "inactive".into(),
        },
        DigitalInputState {
            token: "DigitalInput_2".into(),
            idle_state: "open".into(),
            logical_state: "inactive".into(),
        },
    ]
}

/// Three storage locations that **disagree on every field independently**.
///
/// A single entry would let a renderer that hard-codes `LocalPath` and omits
/// `StorageUri`/`User` pass just as well as a correct one — the same
/// single-fixture blindness the multi-sensor rule in `CLAUDE.md` describes.
/// Here `local_path`, `storage_uri` and `user` are each present on some
/// entries and empty on others, and no two entries share a `storage_type`,
/// so an assertion on any one of them can fail on its own.
///
/// Note what this does *not* buy: `StorageConfiguration` parses these as
/// `String` with `unwrap_or_default()`, so a client cannot tell an omitted
/// element from an empty one. The empty fields here prove the renderer reads
/// state rather than hard-coding the seed; they do not pin the wire shape.
fn default_storage() -> Vec<StorageEntry> {
    vec![
        // Local card: a path, no network URI, no credentials.
        StorageEntry {
            token: "SD_01".into(),
            storage_type: "LocalStorage".into(),
            local_path: "/mnt/sd".into(),
            storage_uri: String::new(),
            user: String::new(),
        },
        // Authenticated NFS share: every field populated.
        StorageEntry {
            token: "NAS_01".into(),
            storage_type: "NFS".into(),
            local_path: "/mnt/nas".into(),
            storage_uri: "nfs://192.168.1.50/records".into(),
            user: "recorder".into(),
        },
        // Anonymous CIFS share: a URI and nothing else. The entry that keeps
        // "the device did not say" distinguishable from "the device said
        // empty" on both `local_path` and `user`.
        StorageEntry {
            token: "CIFS_01".into(),
            storage_type: "CIFS".into(),
            local_path: String::new(),
            storage_uri: "smb://192.168.1.60/cam".into(),
            user: String::new(),
        },
    ]
}

fn default_true() -> bool {
    true
}
fn default_level_max() -> f32 {
    100.0
}

/// One imaging entry per sensor, and they deliberately differ.
///
/// `VS_1` is the motorised 5MP lens; `VS_2` is a fixed-focus 720p lens that
/// refuses the focus operations outright and reports levels on a 0–255 scale.
/// Every value below that differs between the two is one a per-channel test can
/// assert on; if they were equal, the tests would pass against a mock that
/// threw the `VideoSourceToken` away.
fn default_imaging_sources() -> Vec<ImagingState> {
    vec![
        ImagingState {
            source_token: "VS_1".into(),
            brightness: 60.0,
            color_saturation: 50.0,
            contrast: 50.0,
            sharpness: 50.0,
            exposure_mode: "AUTO".into(),
            white_balance_mode: "AUTO".into(),
            backlight_compensation: "OFF".into(),
            wide_dynamic_range_mode: "OFF".into(),
            wide_dynamic_range_level: 50.0,
            ir_cut_filter: "AUTO".into(),
            focus_mode: "AUTO".into(),
            focus_supported: true,
            level_max: 100.0,
        },
        ImagingState {
            source_token: "VS_2".into(),
            brightness: 45.0,
            color_saturation: 128.0,
            contrast: 130.0,
            sharpness: 96.0,
            exposure_mode: "MANUAL".into(),
            white_balance_mode: "MANUAL".into(),
            backlight_compensation: "ON".into(),
            wide_dynamic_range_mode: "ON".into(),
            wide_dynamic_range_level: 70.0,
            ir_cut_filter: "ON".into(),
            // No motorised focus, so the mode is the only one such a lens can
            // report — and `GetMoveOptions` on it is a fault, not a range.
            focus_mode: "MANUAL".into(),
            focus_supported: false,
            level_max: 255.0,
        },
    ]
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            info: default_device_info(),
            hostname: default_hostname(),
            hostname_from_dhcp: false,
            users: default_users(),
            scopes: default_scopes(),
            timezone: default_tz(),
            daylight_savings: false,
            dns: default_dns(),
            ntp: default_ntp(),
            gateway_ipv4: default_gateway(),
            discovery_mode: default_discovery_mode(),
            imaging_sources: default_imaging_sources(),
            ptz: default_ptz(),
            interface: default_interface(),
            protocols: default_protocols(),
            osd: default_osd(),
            profiles: default_profiles(),
            recording: default_recording(),
            video_sources: default_video_sources(),
            video_source_configs: default_video_source_configs(),
            video_encoders: default_video_encoders(),
            relay_outputs: default_relay_outputs(),
            digital_inputs: default_digital_inputs(),
            storage: default_storage(),
            event_seq: 0,
            event_filter: None,
            pending_io_events: Vec::new(),
        }
    }
}

// ── In-memory shared state ──────────────────────────────────────────────────
//
// The mock holds its `DeviceState` purely in memory and never touches the
// filesystem. Persistence is opt-in and owned by the caller: register an
// `on_change` hook (the bundled example writes TOML) and it fires after every
// mutation with a snapshot of the new state.

/// Callback fired after each state mutation — the seam for caller-owned
/// persistence without the library doing any file I/O.
pub type ChangeHook = std::sync::Arc<dyn Fn(&DeviceState) + Send + Sync>;

/// Thread-safe in-memory device state shared by `MockTransport` / `MockServer`.
pub struct MockState {
    state: RwLock<DeviceState>,
    on_change: Option<ChangeHook>,
}

/// Internal alias so service handlers keep reading `&SharedState` unchanged.
pub type SharedState = MockState;

impl MockState {
    /// Fresh state seeded with factory defaults and no persistence hook.
    pub fn new() -> Self {
        Self {
            state: RwLock::new(DeviceState::default()),
            on_change: None,
        }
    }

    /// Seed with a caller-supplied state (e.g. loaded from disk by the caller).
    pub fn with_state(state: DeviceState) -> Self {
        Self {
            state: RwLock::new(state),
            on_change: None,
        }
    }

    /// Register a hook invoked after every mutation with a snapshot of the new
    /// state. This is how opt-in persistence is wired — the library performs no
    /// file I/O itself.
    pub fn set_on_change(&mut self, hook: ChangeHook) {
        self.on_change = Some(hook);
    }

    /// Read access.
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, DeviceState> {
        self.state.read().unwrap()
    }

    /// Mutate the state, then fire the change hook (if any).
    pub fn modify(&self, f: impl FnOnce(&mut DeviceState)) {
        {
            let mut guard = self.state.write().unwrap();
            f(&mut guard);
        }
        self.notify();
    }

    /// Like [`modify`](Self::modify) but the closure returns a value
    /// (e.g. a freshly-generated token).
    pub fn modify_returning<R>(&self, f: impl FnOnce(&mut DeviceState) -> R) -> R {
        let result = {
            let mut guard = self.state.write().unwrap();
            f(&mut guard)
        };
        self.notify();
        result
    }

    fn notify(&self) {
        if let Some(hook) = &self.on_change {
            hook(&self.state.read().unwrap());
        }
    }

    /// In-memory instance for tests.
    #[cfg(test)]
    pub fn for_tests() -> Self {
        Self::new()
    }
}

impl Default for MockState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::services::device;

    fn new_state() -> MockState {
        MockState::new()
    }

    /// The mock's clock must be the **real** clock, all six components.
    ///
    /// It used to compute the time of day from `SystemTime::now()` and hardcode
    /// `<tt:Year>2026</tt:Year><tt:Month>4</tt:Month><tt:Day>15</tt:Day>`, so
    /// the reported timestamp drifted a day further into the past every day. It
    /// broke nothing and failed nothing — but `HealthCheck` compares the device
    /// clock against the local one, so a healthy mock warned about clock skew,
    /// and that warning was the first thing anyone saw when trying the health
    /// check without a camera.
    ///
    /// Asserted against the parser rather than the string, and as a skew bound
    /// rather than an expected date: a literal date would need editing every
    /// day, which is how the bug got there.
    #[test]
    fn system_date_and_time_reports_the_real_clock_not_a_frozen_date() {
        let xml = device::resp_system_date_and_time(&new_state());
        let body = crate::soap::parse_soap_body(&xml).expect("mock response is a SOAP envelope");
        let resp = body
            .child("GetSystemDateAndTimeResponse")
            .expect("response element present");
        let dt = crate::types::SystemDateTime::from_xml(resp)
            .expect("the mock's own response must parse");

        let skew = dt.utc_offset_secs();
        assert!(
            skew.abs() < 120,
            "the mock's clock is {skew}s off the local clock; a frozen date or a \
             time-of-day-only computation is back. Response was:\n{xml}"
        );
    }

    #[test]
    fn get_hostname_returns_default() {
        let s = new_state();
        let xml = device::resp_hostname(&s);
        assert!(xml.contains("mock-camera"), "default hostname");
    }

    #[test]
    fn set_hostname_then_get() {
        let s = new_state();
        let body = r#"<tds:SetHostname><tt:Name>new-host</tt:Name></tds:SetHostname>"#;
        device::handle_set_hostname(&s, body);
        let xml = device::resp_hostname(&s);
        assert!(xml.contains("new-host"));
        assert!(!xml.contains("mock-camera"));
    }

    #[test]
    fn get_users_returns_defaults() {
        let s = new_state();
        let xml = device::resp_users(&s);
        assert!(xml.contains("admin"));
        assert!(xml.contains("operator"));
    }

    #[test]
    fn create_user_then_get() {
        let s = new_state();
        let body = r#"<tds:CreateUsers><tds:User><tt:Username>viewer</tt:Username><tt:Password>pass</tt:Password><tt:UserLevel>User</tt:UserLevel></tds:User></tds:CreateUsers>"#;
        device::handle_create_users(&s, body);
        let xml = device::resp_users(&s);
        assert!(xml.contains("viewer"));
    }

    #[test]
    fn delete_user_then_get() {
        let s = new_state();
        let body = r#"<tds:DeleteUsers><tt:Username>operator</tt:Username></tds:DeleteUsers>"#;
        device::handle_delete_users(&s, body);
        let xml = device::resp_users(&s);
        assert!(xml.contains("admin"));
        assert!(!xml.contains("operator"));
    }

    #[test]
    fn set_user_level_then_get() {
        let s = new_state();
        let body = r#"<tds:SetUser><tds:User><tt:Username>operator</tt:Username><tt:UserLevel>Administrator</tt:UserLevel></tds:User></tds:SetUser>"#;
        device::handle_set_user(&s, body);
        let xml = device::resp_users(&s);
        assert_eq!(xml.matches("Administrator").count(), 2);
    }

    #[test]
    fn set_dns_then_get() {
        let s = new_state();
        let body = r#"<tds:SetDNS><tt:FromDHCP>false</tt:FromDHCP><tt:DNSManual><tt:Type>IPv4</tt:Type><tt:IPv4Address>1.1.1.1</tt:IPv4Address></tt:DNSManual></tds:SetDNS>"#;
        device::handle_set_dns(&s, body);
        let xml = device::resp_dns(&s);
        assert!(xml.contains("1.1.1.1"));
        assert!(!xml.contains("8.8.8.8"));
    }

    #[test]
    fn set_ntp_then_get() {
        let s = new_state();
        let body = r#"<tds:SetNTP><tt:FromDHCP>false</tt:FromDHCP><tt:NTPManual><tt:Type>DNS</tt:Type><tt:DNSname>time.google.com</tt:DNSname></tt:NTPManual></tds:SetNTP>"#;
        device::handle_set_ntp(&s, body);
        let xml = device::resp_ntp(&s);
        assert!(xml.contains("time.google.com"));
        assert!(!xml.contains("pool.ntp.org"));
    }

    #[test]
    fn set_scopes_then_get() {
        let s = new_state();
        // ONVIF SetScopes: each URI is sent as a bare <Scopes>URI</Scopes>
        // element — NOT wrapped in <ScopeItem>. The old test was matching
        // a broken parser that looked for the wrong tag; fixed along with
        // `handle_set_scopes` itself.
        let body = r#"<tds:SetScopes><tds:Scopes>onvif://www.onvif.org/name/NewCam</tds:Scopes></tds:SetScopes>"#;
        device::handle_set_scopes(&s, body);
        let xml = device::resp_scopes(&s);
        assert!(xml.contains("NewCam"));
        assert!(!xml.contains("MockCamera"));
    }

    #[test]
    fn set_timezone_then_get() {
        let s = new_state();
        let body = r#"<tds:SetSystemDateAndTime><tt:TimeZone><tt:TZ>CST-8</tt:TZ></tt:TimeZone><tt:DaylightSavings>true</tt:DaylightSavings></tds:SetSystemDateAndTime>"#;
        device::handle_set_system_date_and_time(&s, body);
        let xml = device::resp_system_date_and_time(&s);
        assert!(xml.contains("CST-8"));
        assert!(xml.contains("<tt:DaylightSavings>true</tt:DaylightSavings>"));
    }

    #[test]
    fn device_info_reads_from_state() {
        let s = new_state();
        let xml = device::resp_device_info(&s);
        assert!(xml.contains("oxvif-mock"));
        assert!(xml.contains("MockCam-1080p"));
    }

    #[test]
    fn set_network_interfaces_updates_ip_and_dhcp() {
        let s = new_state();
        // SetNetworkInterfaces body shape per oxvif's `set_network_interfaces`.
        let body = r#"<tds:SetNetworkInterfaces>
            <tds:InterfaceToken>eth0</tds:InterfaceToken>
            <tds:NetworkInterface>
              <tt:Enabled>true</tt:Enabled>
              <tt:IPv4>
                <tt:Enabled>true</tt:Enabled>
                <tt:DHCP>false</tt:DHCP>
                <tt:Manual>
                  <tt:Address>10.0.0.5</tt:Address>
                  <tt:PrefixLength>16</tt:PrefixLength>
                </tt:Manual>
              </tt:IPv4>
            </tds:NetworkInterface>
          </tds:SetNetworkInterfaces>"#;
        let resp = device::handle_set_network_interfaces(&s, body);
        // Response wraps RebootNeeded — sanity check that the handler ran.
        assert!(resp.contains("SetNetworkInterfacesResponse"));
        assert!(resp.contains("RebootNeeded"));
        let xml = device::resp_network_interfaces(&s);
        assert!(xml.contains("10.0.0.5"));
        assert!(xml.contains("<tt:PrefixLength>16</tt:PrefixLength>"));
        assert!(!xml.contains("192.168.1.100"));
    }

    #[test]
    fn set_network_protocols_updates_and_inserts() {
        let s = new_state();
        // Flip HTTP port, add a brand-new "FTP" entry the mock didn't have.
        let body = r#"<tds:SetNetworkProtocols>
            <tds:NetworkProtocols><tt:Name>HTTP</tt:Name><tt:Enabled>false</tt:Enabled><tt:Port>8080</tt:Port></tds:NetworkProtocols>
            <tds:NetworkProtocols><tt:Name>FTP</tt:Name><tt:Enabled>true</tt:Enabled><tt:Port>21</tt:Port></tds:NetworkProtocols>
          </tds:SetNetworkProtocols>"#;
        device::handle_set_network_protocols(&s, body);
        let xml = device::resp_network_protocols(&s);
        // HTTP should still be there but disabled + new port.
        assert!(xml.contains("<tt:Name>HTTP</tt:Name>"));
        assert!(xml.contains("<tt:Port>8080</tt:Port>"));
        // FTP newly inserted.
        assert!(xml.contains("<tt:Name>FTP</tt:Name>"));
        assert!(xml.contains("<tt:Port>21</tt:Port>"));
    }

    #[test]
    fn set_network_default_gateway_replaces_list() {
        let s = new_state();
        let body = r#"<tds:SetNetworkDefaultGateway>
            <tds:IPv4Address>10.0.0.1</tds:IPv4Address>
            <tds:IPv4Address>10.0.0.254</tds:IPv4Address>
          </tds:SetNetworkDefaultGateway>"#;
        device::handle_set_network_default_gateway(&s, body);
        let xml = device::resp_network_default_gateway(&s);
        assert!(xml.contains("10.0.0.1"));
        assert!(xml.contains("10.0.0.254"));
        // Default was 192.168.1.1 — must be gone after replacement.
        assert!(!xml.contains("192.168.1.1"));
    }

    /// Every PTZ request names a profile. Wrapping it here keeps each test
    /// below reading like the operation it exercises rather than like XML.
    fn ptz_req(profile: &str, op: &str, inner: &str) -> String {
        format!("<tptz:{op}><tptz:ProfileToken>{profile}</tptz:ProfileToken>{inner}</tptz:{op}>")
    }

    fn ptz_ask(profile: &str, op: &str) -> String {
        ptz_req(profile, op, "")
    }

    const MOVE_TO: &str = r#"<tptz:Position>
        <tt:PanTilt x="{X}" y="{Y}"/><tt:Zoom x="{Z}"/>
      </tptz:Position>"#;

    fn move_to(profile: &str, x: &str, y: &str, z: &str) -> String {
        ptz_req(
            profile,
            "AbsoluteMove",
            &MOVE_TO
                .replace("{X}", x)
                .replace("{Y}", y)
                .replace("{Z}", z),
        )
    }

    #[test]
    fn ptz_absolute_move_updates_position() {
        use crate::mock::services::ptz;
        let s = new_state();
        ptz::handle_ptz_absolute_move(&s, &move_to("Profile_1", "0.5", "-0.3", "0.7"));
        let xml = ptz::resp_ptz_status(&s, &ptz_ask("Profile_1", "GetStatus"));
        assert!(xml.contains(r#"x="0.5""#));
        assert!(xml.contains(r#"y="-0.3""#));
        assert!(xml.contains(r#"x="0.7""#));
    }

    /// The whole point of `PtzChannel`: two profiles are two heads.
    ///
    /// Before 0.15 this could not be written — `PtzState` held one position for
    /// the entire device, so moving "Profile_1" moved everything. The measured
    /// symptom was `ptz_get_status(Profile_1 vs Profile_3)` returning the same
    /// pan for both.
    #[test]
    fn ptz_move_on_one_profile_does_not_move_another() {
        use crate::mock::services::ptz;
        let s = new_state();
        let before = ptz::resp_ptz_status(&s, &ptz_ask("Profile_3", "GetStatus"));
        // Profile_3's seeded position, deliberately not Profile_1's.
        assert!(before.contains(r#"x="-0.6""#), "got {before}");

        ptz::handle_ptz_absolute_move(&s, &move_to("Profile_1", "0.5", "-0.3", "0.7"));

        let after = ptz::resp_ptz_status(&s, &ptz_ask("Profile_3", "GetStatus"));
        assert_eq!(
            before, after,
            "moving Profile_1 must not move Profile_3 — they are separate heads"
        );
    }

    /// …and the same for the preset list, which is the other half of the state
    /// that used to be global.
    #[test]
    fn ptz_presets_are_per_profile() {
        use crate::mock::services::ptz;
        let s = new_state();
        let p1 = ptz::resp_ptz_presets(&s, &ptz_ask("Profile_1", "GetPresets"));
        let p3 = ptz::resp_ptz_presets(&s, &ptz_ask("Profile_3", "GetPresets"));
        let p4 = ptz::resp_ptz_presets(&s, &ptz_ask("Profile_4", "GetPresets"));

        // Counts differ, so a handler that ignores the token cannot be right.
        assert_eq!(p1.matches("<tptz:Preset ").count(), 2, "got {p1}");
        assert_eq!(p3.matches("<tptz:Preset ").count(), 3, "got {p3}");
        assert_eq!(p4.matches("<tptz:Preset ").count(), 0, "got {p4}");

        // …and so do the names, so a count-only assertion is not the only guard.
        assert!(p1.contains("Door") && !p1.contains("Lobby"), "got {p1}");
        assert!(p3.contains("Lobby") && !p3.contains("Door"), "got {p3}");
    }

    /// A PTZ request with no `ProfileToken` faults rather than answering for
    /// some default head. That fallback is exactly what made a token-less
    /// handler indistinguishable from a correct one.
    #[test]
    fn ptz_without_a_profile_token_faults() {
        use crate::mock::services::ptz;
        let s = new_state();
        let xml = ptz::resp_ptz_status(&s, "<tptz:GetStatus/>");
        assert!(xml.contains("NoProfileToken-STATUS-5601"), "got {xml}");

        let unknown = ptz::resp_ptz_status(&s, &ptz_ask("Profile_nope", "GetStatus"));
        assert!(
            unknown.contains("NoSuchProfile-STATUS-5601"),
            "got {unknown}"
        );
        assert!(unknown.contains("Profile_nope"), "got {unknown}");
    }

    #[test]
    fn ptz_set_preset_uses_current_position_and_returns_token() {
        use crate::mock::services::ptz;
        let s = new_state();
        // Move first so SetPreset captures a non-zero position.
        ptz::handle_ptz_absolute_move(&s, &move_to("Profile_1", "0.4", "0.1", "0.2"));

        let body = ptz_req(
            "Profile_1",
            "SetPreset",
            "<tptz:PresetName>Garden</tptz:PresetName>",
        );
        let resp = ptz::handle_ptz_set_preset(&s, &body);
        // Profile_1 already has Preset_1 and Preset_2, so the new one is Preset_3.
        assert!(resp.contains("Preset_3"), "got {resp}");

        let presets = ptz::resp_ptz_presets(&s, &ptz_ask("Profile_1", "GetPresets"));
        assert!(presets.contains("Garden"));
        assert!(presets.contains(r#"x="0.4""#));

        // Profile_2 has its own list and must be untouched.
        let other = ptz::resp_ptz_presets(&s, &ptz_ask("Profile_2", "GetPresets"));
        assert!(!other.contains("Garden"), "got {other}");
    }

    #[test]
    fn ptz_remove_preset_then_get() {
        use crate::mock::services::ptz;
        let s = new_state();
        let body = ptz_req(
            "Profile_1",
            "RemovePreset",
            "<tptz:PresetToken>Preset_2</tptz:PresetToken>",
        );
        ptz::handle_ptz_remove_preset(&s, &body);
        let xml = ptz::resp_ptz_presets(&s, &ptz_ask("Profile_1", "GetPresets"));
        assert!(xml.contains("Preset_1"));
        assert!(!xml.contains(r#"token="Preset_2""#));

        // Profile_3 also has a Preset_2 — removing Profile_1's must not take it.
        let other = ptz::resp_ptz_presets(&s, &ptz_ask("Profile_3", "GetPresets"));
        assert!(other.contains(r#"token="Preset_2""#), "got {other}");
    }

    #[test]
    fn ptz_goto_preset_jumps_position() {
        use crate::mock::services::ptz;
        let s = new_state();
        // Profile_1's Preset_2 ("Door"): pan=0.5 tilt=0.2 zoom=0.0
        let body = ptz_req(
            "Profile_1",
            "GotoPreset",
            "<tptz:PresetToken>Preset_2</tptz:PresetToken>",
        );
        ptz::handle_ptz_goto_preset(&s, &body);
        let xml = ptz::resp_ptz_status(&s, &ptz_ask("Profile_1", "GetStatus"));
        assert!(xml.contains(r#"x="0.5""#));
        assert!(xml.contains(r#"y="0.2""#));
    }

    #[test]
    fn ptz_set_home_then_goto_home() {
        use crate::mock::services::ptz;
        let s = new_state();
        // Move, set home, move away, goto home → position should reset to setpoint.
        ptz::handle_ptz_absolute_move(&s, &move_to("Profile_1", "0.8", "-0.4", "0.3"));
        ptz::handle_ptz_set_home_position(&s, &ptz_ask("Profile_1", "SetHomePosition"));

        ptz::handle_ptz_absolute_move(&s, &move_to("Profile_1", "-0.5", "0.5", "0.0"));
        ptz::handle_ptz_goto_home_position(&s, &ptz_ask("Profile_1", "GotoHomePosition"));

        let xml = ptz::resp_ptz_status(&s, &ptz_ask("Profile_1", "GetStatus"));
        assert!(xml.contains(r#"x="0.8""#));
        assert!(xml.contains(r#"y="-0.4""#));
    }

    /// Preset *tours* were global too. Profile_1 ships one and Profile_2 ships
    /// none, so a tour handler that ignores the profile cannot answer both.
    #[test]
    fn ptz_preset_tours_are_per_profile() {
        use crate::mock::services::ptz;
        let s = new_state();
        let p1 = ptz::resp_ptz_preset_tours(&s, &ptz_ask("Profile_1", "GetPresetTours"));
        let p2 = ptz::resp_ptz_preset_tours(&s, &ptz_ask("Profile_2", "GetPresetTours"));
        assert!(p1.contains("Tour_1"), "got {p1}");
        assert!(!p2.contains("Tour_1"), "got {p2}");

        // A tour created on Profile_2 is Profile_2's alone.
        let created =
            ptz::handle_ptz_create_preset_tour(&s, &ptz_ask("Profile_2", "CreatePresetTour"));
        assert!(
            created.contains("Tour_1"),
            "first tour on this head: {created}"
        );
        let p1_after = ptz::resp_ptz_preset_tours(&s, &ptz_ask("Profile_1", "GetPresetTours"));
        assert_eq!(
            p1_after.matches("<tptz:PresetTour ").count(),
            1,
            "Profile_1 still has exactly its own one tour: {p1_after}"
        );
    }

    // ── OSD CRUD + quota ─────────────────────────────────────────────────

    #[test]
    fn osd_default_state_has_one_datetime_entry() {
        let s = new_state();
        let snap = s.read().osd.clone();
        assert_eq!(snap.osds.len(), 1);
        assert_eq!(snap.osds[0].token, "OSD_1");
        assert_eq!(snap.osds[0].text.as_ref().unwrap().text_type, "DateAndTime");
    }

    #[test]
    fn create_osd_then_appears_in_get() {
        use crate::mock::services::media;
        let s = new_state();
        // Create a Plain text OSD — DateAndTime is at quota (1/1) by default.
        let body = r#"<trt:CreateOSD><trt:OSD>
            <tt:VideoSourceConfigurationToken>VSC_1</tt:VideoSourceConfigurationToken>
            <tt:Type>Text</tt:Type>
            <tt:Position><tt:Type>UpperRight</tt:Type></tt:Position>
            <tt:TextString>
              <tt:Type>Plain</tt:Type>
              <tt:PlainText>Hello camera</tt:PlainText>
              <tt:FontSize>24</tt:FontSize>
            </tt:TextString>
          </trt:OSD></trt:CreateOSD>"#;
        let resp = media::handle_create_osd(&s, body);
        assert!(resp.contains("CreateOSDResponse"));
        assert!(resp.contains("OSD_2"), "new token should be OSD_2");

        let listed = media::resp_osds(&s, "<trt:GetOSDs/>");
        assert!(listed.contains("OSD_1"));
        assert!(listed.contains("OSD_2"));
        assert!(listed.contains("Hello camera"));
    }

    #[test]
    fn create_osd_rejects_when_per_type_quota_full() {
        use crate::mock::services::media;
        let s = new_state();
        // Default already has one DateAndTime — DateAndTime quota is 1.
        // A second one must be rejected.
        let body = r#"<trt:CreateOSD><trt:OSD>
            <tt:VideoSourceConfigurationToken>VSC_1</tt:VideoSourceConfigurationToken>
            <tt:Type>Text</tt:Type>
            <tt:Position><tt:Type>LowerRight</tt:Type></tt:Position>
            <tt:TextString><tt:Type>DateAndTime</tt:Type></tt:TextString>
          </trt:OSD></trt:CreateOSD>"#;
        let resp = media::handle_create_osd(&s, body);
        assert!(resp.contains("Fault"), "should be SOAP fault");
        assert!(resp.contains("InvalidArgs"));
        assert!(resp.contains("DateAndTime"));
        // State unchanged — still just the default one.
        assert_eq!(s.read().osd.osds.len(), 1);
    }

    #[test]
    fn set_osd_updates_existing() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:SetOSD><trt:OSD token="OSD_1">
            <tt:VideoSourceConfigurationToken>VSC_1</tt:VideoSourceConfigurationToken>
            <tt:Type>Text</tt:Type>
            <tt:Position><tt:Type>LowerLeft</tt:Type></tt:Position>
            <tt:TextString>
              <tt:Type>DateAndTime</tt:Type>
              <tt:DateFormat>yyyy-MM-dd</tt:DateFormat>
            </tt:TextString>
          </trt:OSD></trt:SetOSD>"#;
        let resp = media::handle_set_osd(&s, body);
        assert!(resp.contains("SetOSDResponse"));
        assert!(!resp.contains("Fault"));

        let listed = media::resp_osds(&s, "<trt:GetOSDs/>");
        assert!(listed.contains("LowerLeft"));
        assert!(listed.contains("yyyy-MM-dd"));
        // VSC token must be preserved across SetOSD.
        assert!(listed.contains("VSC_1"));
    }

    #[test]
    fn delete_osd_removes_entry() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:DeleteOSD><trt:OSDToken>OSD_1</trt:OSDToken></trt:DeleteOSD>"#;
        let resp = media::handle_delete_osd(&s, body);
        assert!(resp.contains("DeleteOSDResponse"));
        assert_eq!(s.read().osd.osds.len(), 0);
    }

    #[test]
    fn delete_osd_unknown_token_returns_fault() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:DeleteOSD><trt:OSDToken>OSD_99</trt:OSDToken></trt:DeleteOSD>"#;
        let resp = media::handle_delete_osd(&s, body);
        assert!(resp.contains("Fault"));
        assert!(resp.contains("OSD_99"));
        // State untouched.
        assert_eq!(s.read().osd.osds.len(), 1);
    }

    #[test]
    fn get_osds_filters_by_configuration_token() {
        use crate::mock::services::media;
        let s = new_state();
        // Create one OSD on a different VSC.
        let create = r#"<trt:CreateOSD><trt:OSD>
            <tt:VideoSourceConfigurationToken>VSC_OTHER</tt:VideoSourceConfigurationToken>
            <tt:Type>Text</tt:Type>
            <tt:Position><tt:Type>UpperLeft</tt:Type></tt:Position>
            <tt:TextString><tt:Type>Plain</tt:Type><tt:PlainText>Other</tt:PlainText></tt:TextString>
          </trt:OSD></trt:CreateOSD>"#;
        media::handle_create_osd(&s, create);
        assert_eq!(s.read().osd.osds.len(), 2);

        // Filter by VSC_1 — should NOT include the VSC_OTHER one.
        let only_vsc1 = media::resp_osds(
            &s,
            r#"<trt:GetOSDs><trt:ConfigurationToken>VSC_1</trt:ConfigurationToken></trt:GetOSDs>"#,
        );
        assert!(only_vsc1.contains("OSD_1"));
        assert!(!only_vsc1.contains("Other"));
    }

    #[test]
    fn osd_options_advertises_per_type_quotas_via_attributes() {
        use crate::mock::services::media;
        let xml = media::resp_osd_options();
        // Genetec/late-Hikvision shape — attributes on <MaximumNumberOfOSDs>,
        // not element text. oxvif::OnvifSession parses these.
        assert!(xml.contains(r#"Total="8""#));
        assert!(xml.contains(r#"DateAndTime="1""#));
        assert!(xml.contains(r#"Plain="7""#));
    }

    // ── Profile CRUD ─────────────────────────────────────────────────────

    #[test]
    fn profiles_default_state_has_two() {
        use crate::mock::services::media;
        let s = new_state();
        let xml = media::resp_profiles(&s);
        assert!(xml.contains(r#"token="Profile_1" fixed="true""#));
        assert!(xml.contains(r#"token="Profile_2" fixed="false""#));
        assert!(xml.contains("mainStream"));
        assert!(xml.contains("subStream"));
        // Default profiles have video configs attached.
        assert!(xml.contains("VSC_1"));
        assert!(xml.contains("VEC_1"));
        assert!(xml.contains("VEC_2"));
        // Sensor 2's pair, so a caller enumerating profiles reaches both lenses.
        assert!(xml.contains(r#"token="Profile_3" fixed="true""#));
        assert!(xml.contains(r#"token="Profile_4" fixed="false""#));
        assert!(xml.contains("VSC_2"));
        assert!(xml.contains("VEC_3"));
        assert!(xml.contains("VEC_4"));
        // Each profile carries its *own* source config, not a shared one: the
        // sensor-2 profiles must not be rendered with VS_1's bounds.
        assert!(xml.contains("<tt:SourceToken>VS_2</tt:SourceToken>"));
    }

    #[test]
    fn create_profile_then_appears_in_get_profiles() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:CreateProfile><trt:Name>customStream</trt:Name></trt:CreateProfile>"#;
        let resp = media::handle_create_profile(&s, body);
        assert!(resp.contains("CreateProfileResponse"));
        // Four default profiles, so the counter starts at 5.
        assert!(resp.contains("Profile_5"));
        assert!(resp.contains("customStream"));
        assert!(resp.contains(r#"fixed="false""#));

        let listed = media::resp_profiles(&s);
        assert!(listed.contains("Profile_5"));
        assert!(listed.contains("customStream"));
        // New profiles have no configurations attached.
        let new_p_block = listed
            .find("Profile_5")
            .and_then(|i| {
                listed[i..]
                    .find("</trt:Profiles>")
                    .map(|j| &listed[i..i + j])
            })
            .unwrap_or("");
        assert!(!new_p_block.contains("VideoSourceConfiguration"));
        assert!(!new_p_block.contains("VideoEncoderConfiguration"));
    }

    #[test]
    fn create_profile_with_explicit_token_honoured() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:CreateProfile>
            <trt:Name>specialName</trt:Name>
            <trt:Token>MyProfile</trt:Token>
          </trt:CreateProfile>"#;
        let resp = media::handle_create_profile(&s, body);
        assert!(resp.contains("MyProfile"));
        // Counter should NOT have been bumped — explicit token, no generation.
        assert_eq!(s.read().profiles.next_token_id, 5);
    }

    #[test]
    fn create_profile_rejects_duplicate_token() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:CreateProfile>
            <trt:Name>dup</trt:Name>
            <trt:Token>Profile_1</trt:Token>
          </trt:CreateProfile>"#;
        let resp = media::handle_create_profile(&s, body);
        assert!(resp.contains("Fault"));
        assert!(resp.contains("ProfileExists"));
        // No new entry, no counter change.
        assert_eq!(s.read().profiles.profiles.len(), 4);
    }

    #[test]
    fn delete_profile_removes_non_fixed() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:DeleteProfile><trt:ProfileToken>Profile_2</trt:ProfileToken></trt:DeleteProfile>"#;
        let resp = media::handle_delete_profile(&s, body);
        assert!(resp.contains("DeleteProfileResponse"));
        assert_eq!(s.read().profiles.profiles.len(), 3);
        // Only Profile_2 went; the other three are untouched and in order.
        let tokens: Vec<String> = s
            .read()
            .profiles
            .profiles
            .iter()
            .map(|p| p.token.clone())
            .collect();
        assert_eq!(tokens, ["Profile_1", "Profile_3", "Profile_4"]);
    }

    #[test]
    fn delete_profile_refuses_fixed() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:DeleteProfile><trt:ProfileToken>Profile_1</trt:ProfileToken></trt:DeleteProfile>"#;
        let resp = media::handle_delete_profile(&s, body);
        assert!(resp.contains("Fault"));
        assert!(resp.contains("DeletionOfFixedProfile"));
        // State untouched.
        assert_eq!(s.read().profiles.profiles.len(), 4);
    }

    #[test]
    fn delete_profile_unknown_token_returns_fault() {
        use crate::mock::services::media;
        let s = new_state();
        let body =
            r#"<trt:DeleteProfile><trt:ProfileToken>NoSuch</trt:ProfileToken></trt:DeleteProfile>"#;
        let resp = media::handle_delete_profile(&s, body);
        assert!(resp.contains("Fault"));
        assert!(resp.contains("NoProfile"));
        assert_eq!(s.read().profiles.profiles.len(), 4);
    }

    #[test]
    fn get_profile_by_token() {
        use crate::mock::services::media;
        let s = new_state();
        let body =
            r#"<trt:GetProfile><trt:ProfileToken>Profile_2</trt:ProfileToken></trt:GetProfile>"#;
        let resp = media::resp_profile(&s, body);
        assert!(resp.contains("GetProfileResponse"));
        assert!(resp.contains("subStream"));
        assert!(!resp.contains("mainStream"));
    }

    #[test]
    fn get_profile_unknown_token_returns_fault() {
        use crate::mock::services::media;
        let s = new_state();
        let body = r#"<trt:GetProfile><trt:ProfileToken>Bogus</trt:ProfileToken></trt:GetProfile>"#;
        let resp = media::resp_profile(&s, body);
        assert!(resp.contains("Fault"));
        assert!(resp.contains("NoProfile"));
    }

    // ── Media2 video encoder config (stateful get/set) ───────────────────

    #[test]
    fn media2_get_video_encoder_configurations_returns_default() {
        use crate::mock::services::media2;
        let s = new_state();
        let xml =
            media2::resp_video_encoder_configurations(&s, "<tr2:GetVideoEncoderConfigurations/>");
        assert!(xml.contains("GetVideoEncoderConfigurationsResponse"));
        // All four channels, not just one: the token-less plural getter lists
        // everything, which is what makes the filtering test below meaningful.
        for tok in ["VEC_1", "VEC_2", "VEC_3", "VEC_4"] {
            assert!(xml.contains(&format!(r#"token="{tok}""#)), "missing {tok}");
        }
        assert!(xml.contains("<tt:Width>1920</tt:Width>"));
        // Sensor 2's main stream is 720p — a value sensor 1 never reports, so
        // this also pins that the list is not four copies of one channel.
        assert!(xml.contains("<tt:Width>1280</tt:Width>"));
        assert!(xml.contains("<tt:Encoding>JPEG</tt:Encoding>"));
    }

    #[test]
    fn media2_get_video_encoder_configurations_filters_by_token() {
        use crate::mock::services::media2;
        let s = new_state();
        let xml = media2::resp_video_encoder_configurations(
            &s,
            r#"<tr2:GetVideoEncoderConfigurations><tr2:ConfigurationToken>OTHER</tr2:ConfigurationToken></tr2:GetVideoEncoderConfigurations>"#,
        );
        // Unknown token → response present but no configuration element.
        assert!(xml.contains("GetVideoEncoderConfigurationsResponse"));
        assert!(!xml.contains(r#"token="VEC_1""#));
    }

    // ── Relay output / Digital input (stateful) ───────────────────────────

    #[test]
    fn get_relay_outputs_returns_two_defaults() {
        let s = new_state();
        let xml = device::resp_relay_outputs(&s);
        assert!(xml.contains(r#"token="RelayOutput_1""#));
        assert!(xml.contains(r#"token="RelayOutput_2""#));
        assert!(xml.contains("<tt:Mode>Bistable</tt:Mode>"));
        assert!(xml.contains("<tt:Mode>Monostable</tt:Mode>"));
    }

    #[test]
    fn set_relay_output_state_updates_logical_and_queues_event() {
        let s = new_state();
        let body = r#"<tds:SetRelayOutputState>
            <tds:RelayOutputToken>RelayOutput_1</tds:RelayOutputToken>
            <tds:LogicalState>active</tds:LogicalState>
          </tds:SetRelayOutputState>"#;
        let resp = device::handle_set_relay_output_state(&s, body);
        assert!(resp.contains("SetRelayOutputStateResponse"));
        assert!(!resp.contains("Fault"));

        let snap = s.read();
        let r1 = snap
            .relay_outputs
            .iter()
            .find(|r| r.token == "RelayOutput_1");
        assert_eq!(r1.unwrap().logical_state, "active");
        assert_eq!(snap.pending_io_events.len(), 1);
        assert_eq!(snap.pending_io_events[0].kind, "RelayOutput");
        assert_eq!(snap.pending_io_events[0].logical_state, "active");
    }

    #[test]
    fn set_relay_output_state_unknown_token_returns_fault() {
        let s = new_state();
        let body = r#"<tds:SetRelayOutputState>
            <tds:RelayOutputToken>NoSuch</tds:RelayOutputToken>
            <tds:LogicalState>active</tds:LogicalState>
          </tds:SetRelayOutputState>"#;
        let resp = device::handle_set_relay_output_state(&s, body);
        assert!(resp.contains("Fault"));
        assert!(resp.contains("NoSuch"));
        // State untouched.
        assert_eq!(s.read().pending_io_events.len(), 0);
    }

    #[test]
    fn set_relay_output_settings_updates_mode_and_delay() {
        let s = new_state();
        let body = r#"<tds:SetRelayOutputSettings>
            <tds:RelayOutputToken>RelayOutput_1</tds:RelayOutputToken>
            <tds:Properties>
              <tt:Mode>Monostable</tt:Mode>
              <tt:DelayTime>PT5S</tt:DelayTime>
              <tt:IdleState>open</tt:IdleState>
            </tds:Properties>
          </tds:SetRelayOutputSettings>"#;
        let resp = device::handle_set_relay_output_settings(&s, body);
        assert!(resp.contains("SetRelayOutputSettingsResponse"));

        let snap = s.read();
        let r1 = snap
            .relay_outputs
            .iter()
            .find(|r| r.token == "RelayOutput_1");
        let r1 = r1.unwrap();
        assert_eq!(r1.mode, "Monostable");
        assert_eq!(r1.delay_time, "PT5S");
        assert_eq!(r1.idle_state, "open");
    }

    #[test]
    fn get_digital_inputs_returns_two_defaults() {
        let s = new_state();
        let xml = device::resp_digital_inputs(&s);
        assert!(xml.contains(r#"token="DigitalInput_1""#));
        assert!(xml.contains(r#"token="DigitalInput_2""#));
        assert!(xml.contains(r#"IdleState="closed""#));
        assert!(xml.contains(r#"IdleState="open""#));
    }

    #[test]
    fn pull_messages_drains_pending_io_event_first() {
        use crate::mock::services::events;
        let s = new_state();
        // Seed an IO event as if the pulse endpoint fired.
        s.modify(|st| {
            st.pending_io_events.push(PendingIoEvent {
                kind: "DigitalInput",
                token: "DigitalInput_1".into(),
                logical_state: "active".into(),
            });
        });
        let xml = events::resp_pull_messages(&s);
        assert!(xml.contains("tns1:Device/Trigger/DigitalInput"));
        assert!(xml.contains(r#"Name="InputToken" Value="DigitalInput_1""#));
        assert!(xml.contains(r#"Name="LogicalState" Value="true""#));
        // Queue drained; next call falls through to the synthetic stream.
        assert_eq!(s.read().pending_io_events.len(), 0);
        let xml2 = events::resp_pull_messages(&s);
        assert!(xml2.contains("MotionAlarm") || xml2.contains("RuleEngine"));
    }

    #[test]
    fn pull_messages_io_event_relay_uses_relay_token_source() {
        use crate::mock::services::events;
        let s = new_state();
        s.modify(|st| {
            st.pending_io_events.push(PendingIoEvent {
                kind: "RelayOutput",
                token: "RelayOutput_1".into(),
                logical_state: "inactive".into(),
            });
        });
        let xml = events::resp_pull_messages(&s);
        assert!(xml.contains("tns1:Device/Trigger/RelayOutput"));
        assert!(xml.contains(r#"Name="RelayToken" Value="RelayOutput_1""#));
        assert!(xml.contains(r#"Name="LogicalState" Value="false""#));
    }

    // ── Multi-sensor: per-channel answers ─────────────────────────────────
    //
    // The rule these cover (CLAUDE.md, "Multi-sensor devices"): a single-sensor
    // fixture cannot cover a per-channel feature, because it passes just as well
    // against a responder that ignores the token entirely. Every assertion below
    // is written so that it fails if the token stops being read — most of them
    // by naming a value that exists on *one* channel only.

    /// Pull the `<tt:Width>` values out of a rendered response, in order.
    fn widths(xml: &str) -> Vec<u32> {
        xml.split("<tt:Width>")
            .skip(1)
            .filter_map(|s| s.split("</tt:Width>").next())
            .filter_map(|s| s.trim().parse().ok())
            .collect()
    }

    /// Every default encoder must run a resolution it also offers, draw from a
    /// sensor that exists, and every source config must point at a real sensor.
    ///
    /// Without this, the fixture could drift into a shape no camera produces —
    /// a channel advertising 720p while running 1080p — and the per-channel
    /// tests below would still pass while asserting nonsense.
    #[test]
    fn default_video_catalogue_is_self_consistent() {
        let s = new_state();
        let snap = s.read();
        let sensors: Vec<&str> = snap
            .video_sources
            .iter()
            .map(|v| v.token.as_str())
            .collect();
        assert_eq!(sensors, ["VS_1", "VS_2"], "two sensors");

        for c in &snap.video_encoders {
            assert!(
                c.resolutions.contains(&(c.width, c.height)),
                "{} runs {}x{} but does not offer it: {:?}",
                c.token,
                c.width,
                c.height,
                c.resolutions
            );
            assert!(
                sensors.contains(&c.source_token.as_str()),
                "{} draws from unknown sensor {}",
                c.token,
                c.source_token
            );
        }
        for c in &snap.video_source_configs {
            assert!(
                sensors.contains(&c.source_token.as_str()),
                "{} points at unknown sensor {}",
                c.token,
                c.source_token
            );
        }
        // The property every per-channel test leans on: the two sensors do not
        // agree about their maximum. If this ever becomes false the tests below
        // stop distinguishing a token-aware responder from a token-blind one.
        let max_of = |tok: &str| -> u32 {
            snap.video_encoders
                .iter()
                .filter(|c| c.source_token == tok)
                .flat_map(|c| c.resolutions.iter().map(|(w, _)| *w))
                .max()
                .unwrap()
        };
        assert!(
            max_of("VS_1") > max_of("VS_2"),
            "the sensors must disagree: VS_1 max {} vs VS_2 max {}",
            max_of("VS_1"),
            max_of("VS_2")
        );
    }

    #[test]
    fn get_video_sources_lists_both_sensors() {
        use crate::mock::services::media;
        let s = new_state();
        let xml = media::resp_video_sources(&s);
        assert!(xml.contains(r#"token="VS_1""#));
        assert!(xml.contains(r#"token="VS_2""#));
        // Their resolutions differ, so this is not one sensor listed twice.
        assert_eq!(widths(&xml), vec![2592, 1280]);
    }

    // ── Media1 video encoder options ──────────────────────────────────────

    fn vec_options_body(token: &str) -> String {
        format!(
            "<trt:GetVideoEncoderConfigurationOptions>\
               <trt:ConfigurationToken>{token}</trt:ConfigurationToken>\
             </trt:GetVideoEncoderConfigurationOptions>"
        )
    }

    /// The regression this whole change exists for.
    ///
    /// Before 0.15 `resp_video_encoder_configuration_options` took no arguments,
    /// so both halves of this test saw sensor 1's list and the assertion below
    /// could not have failed.
    #[test]
    fn video_encoder_options_are_per_channel() {
        use crate::mock::services::media;
        let s = new_state();

        let lens1 = media::resp_video_encoder_configuration_options(&s, &vec_options_body("VEC_1"));
        let lens2 = media::resp_video_encoder_configuration_options(&s, &vec_options_body("VEC_3"));

        // Sensor 1 offers 5MP; sensor 2 cannot and must not claim to.
        assert!(lens1.contains("<tt:Width>2592</tt:Width>"), "VEC_1 max");
        assert!(
            !lens2.contains("<tt:Width>2592</tt:Width>"),
            "VEC_3 must not report sensor 1's 2592-wide mode — token ignored?"
        );
        assert_eq!(
            widths(&lens2).into_iter().max(),
            Some(1280),
            "VEC_3 tops out at 720p"
        );
        // And the two responses are genuinely different documents.
        assert_ne!(lens1, lens2);
    }

    /// The `Extension` copy is the superset — a parser reading only the shallow
    /// `Options/H264` loses the widest mode. Pinned here because the two copies
    /// are generated from one list and could silently become identical.
    #[test]
    fn video_encoder_options_extension_copy_is_a_superset() {
        use crate::mock::services::media;
        let s = new_state();
        let xml = media::resp_video_encoder_configuration_options(&s, &vec_options_body("VEC_1"));

        // Match the full pair, not the width alone — VEC_1 offers both
        // 2592x1944 and 2592x1520, so a width-only check cannot tell the two
        // copies apart. (It didn't: this assertion was written that way first
        // and passed against a response where the widest mode *was* present.)
        const WIDEST: &str = "<tt:Width>2592</tt:Width><tt:Height>1944</tt:Height>";
        let (shallow, extension) = xml.split_once("<tt:Extension>").expect("Extension block");
        assert!(
            !shallow.contains(WIDEST),
            "the shallow copy is the older device's smaller list"
        );
        assert!(
            extension.contains(WIDEST),
            "the Extension copy adds the widest mode"
        );
        // Same channel either way — the Extension is a superset, not a different
        // lens's list.
        assert!(shallow.contains("<tt:Width>1280</tt:Width>"));
        assert!(extension.contains("<tt:Width>1280</tt:Width>"));
    }

    #[test]
    fn video_encoder_options_without_token_faults() {
        use crate::mock::services::media;
        let s = new_state();
        let xml = media::resp_video_encoder_configuration_options(
            &s,
            "<trt:GetVideoEncoderConfigurationOptions/>",
        );
        assert!(xml.contains("NoConfigToken-VECOPT-5507"), "got: {xml}");
        // Nothing resembling an answer came back with it.
        assert!(!xml.contains("ResolutionsAvailable"));
    }

    #[test]
    fn video_encoder_options_unknown_token_faults() {
        use crate::mock::services::media;
        let s = new_state();
        let xml = media::resp_video_encoder_configuration_options(&s, &vec_options_body("VEC_99"));
        assert!(
            xml.contains("NoSuchConfig-VECOPT-5508: VEC_99"),
            "the fault must name the token that was rejected; got: {xml}"
        );
        assert!(!xml.contains("ResolutionsAvailable"));
    }

    // ── Media1 video source options ───────────────────────────────────────

    fn vsc_options_body(token: &str) -> String {
        format!(
            "<trt:GetVideoSourceConfigurationOptions>\
               <trt:ConfigurationToken>{token}</trt:ConfigurationToken>\
             </trt:GetVideoSourceConfigurationOptions>"
        )
    }

    #[test]
    fn video_source_options_are_per_channel() {
        use crate::mock::services::media;
        let s = new_state();
        let lens1 = media::resp_video_source_configuration_options(&s, &vsc_options_body("VSC_1"));
        let lens2 = media::resp_video_source_configuration_options(&s, &vsc_options_body("VSC_2"));

        assert!(lens1.contains("<tt:Max>2592</tt:Max>"), "VSC_1 bounds");
        assert!(lens2.contains("<tt:Max>1280</tt:Max>"), "VSC_2 bounds");
        assert!(!lens2.contains("<tt:Max>2592</tt:Max>"));
        // Each names only its own sensor.
        assert!(lens1.contains("<tt:VideoSourceTokensAvailable>VS_1<"));
        assert!(lens2.contains("<tt:VideoSourceTokensAvailable>VS_2<"));
    }

    #[test]
    fn video_source_options_without_token_faults() {
        use crate::mock::services::media;
        let s = new_state();
        let xml = media::resp_video_source_configuration_options(
            &s,
            "<trt:GetVideoSourceConfigurationOptions/>",
        );
        assert!(xml.contains("NoConfigToken-VSCOPT-5503"), "got: {xml}");
        assert!(!xml.contains("BoundsRange"));
    }

    #[test]
    fn video_source_options_unknown_token_faults() {
        use crate::mock::services::media;
        let s = new_state();
        let xml = media::resp_video_source_configuration_options(&s, &vsc_options_body("VSC_9"));
        assert!(
            xml.contains("NoSuchConfig-VSCOPT-5504: VSC_9"),
            "got: {xml}"
        );
        assert!(!xml.contains("BoundsRange"));
    }

    // ── Media1 singular getters ───────────────────────────────────────────

    #[test]
    fn get_video_encoder_configuration_is_per_channel() {
        use crate::mock::services::media;
        let s = new_state();
        let body = |t: &str| {
            format!(
                "<trt:GetVideoEncoderConfiguration>\
                   <trt:ConfigurationToken>{t}</trt:ConfigurationToken>\
                 </trt:GetVideoEncoderConfiguration>"
            )
        };
        let one = media::resp_video_encoder_configuration(&s, &body("VEC_1"));
        let three = media::resp_video_encoder_configuration(&s, &body("VEC_3"));
        assert!(one.contains(r#"token="VEC_1""#) && one.contains("<tt:Width>1920</tt:Width>"));
        assert!(three.contains(r#"token="VEC_3""#) && three.contains("<tt:Width>1280</tt:Width>"));
        assert!(!three.contains(r#"token="VEC_1""#));

        // JPEG channels carry no tt:H264 block — it is an encoding-specific
        // element and a conformant device does not send it here.
        let four = media::resp_video_encoder_configuration(&s, &body("VEC_4"));
        assert!(four.contains("<tt:Encoding>JPEG</tt:Encoding>"));
        assert!(!four.contains("<tt:H264>"));
    }

    #[test]
    fn get_video_encoder_configuration_unknown_token_faults() {
        use crate::mock::services::media;
        let s = new_state();
        let xml = media::resp_video_encoder_configuration(
            &s,
            "<trt:GetVideoEncoderConfiguration>\
               <trt:ConfigurationToken>VEC_77</trt:ConfigurationToken>\
             </trt:GetVideoEncoderConfiguration>",
        );
        assert!(xml.contains("NoSuchConfig-VEC-5506: VEC_77"), "got: {xml}");
    }

    #[test]
    fn get_video_source_configuration_is_per_channel() {
        use crate::mock::services::media;
        let s = new_state();
        let body = |t: &str| {
            format!(
                "<trt:GetVideoSourceConfiguration>\
                   <trt:ConfigurationToken>{t}</trt:ConfigurationToken>\
                 </trt:GetVideoSourceConfiguration>"
            )
        };
        let one = media::resp_video_source_configuration(&s, &body("VSC_1"));
        let two = media::resp_video_source_configuration(&s, &body("VSC_2"));
        assert!(one.contains("<tt:SourceToken>VS_1</tt:SourceToken>"));
        assert!(two.contains("<tt:SourceToken>VS_2</tt:SourceToken>"));
        assert!(!two.contains("<tt:SourceToken>VS_1</tt:SourceToken>"));
    }

    #[test]
    fn get_video_source_configuration_unknown_token_faults() {
        use crate::mock::services::media;
        let s = new_state();
        let xml = media::resp_video_source_configuration(
            &s,
            "<trt:GetVideoSourceConfiguration>\
               <trt:ConfigurationToken>VSC_77</trt:ConfigurationToken>\
             </trt:GetVideoSourceConfiguration>",
        );
        assert!(xml.contains("NoSuchConfig-VSC-5502: VSC_77"), "got: {xml}");
    }

    // ── Media2 ────────────────────────────────────────────────────────────

    #[test]
    fn media2_video_encoder_options_are_per_channel() {
        use crate::mock::services::media2;
        let s = new_state();
        let body = |t: &str| {
            format!(
                "<tr2:GetVideoEncoderConfigurationOptions>\
                   <tr2:ConfigurationToken>{t}</tr2:ConfigurationToken>\
                 </tr2:GetVideoEncoderConfigurationOptions>"
            )
        };
        let lens1 = media2::resp_video_encoder_configuration_options_media2(&s, &body("VEC_1"));
        let lens2 = media2::resp_video_encoder_configuration_options_media2(&s, &body("VEC_3"));

        assert!(lens1.contains("<tt:Width>2592</tt:Width>"));
        assert!(!lens2.contains("<tt:Width>2592</tt:Width>"));
        // Only the 5MP sensor advertises H.265, so codec support is per-channel
        // too, not a device-wide fact.
        assert!(lens1.contains("<tt:Encoding>H265</tt:Encoding>"));
        assert!(!lens2.contains("<tt:Encoding>H265</tt:Encoding>"));
        assert!(lens2.contains("<tt:Encoding>H264</tt:Encoding>"));
    }

    #[test]
    fn media2_video_encoder_options_without_token_faults() {
        use crate::mock::services::media2;
        let s = new_state();
        let xml = media2::resp_video_encoder_configuration_options_media2(
            &s,
            "<tr2:GetVideoEncoderConfigurationOptions/>",
        );
        assert!(xml.contains("NoConfigToken-VECOPT2-5513"), "got: {xml}");
        assert!(!xml.contains("ResolutionsAvailable"));
    }

    #[test]
    fn media2_video_encoder_options_unknown_token_faults() {
        use crate::mock::services::media2;
        let s = new_state();
        let xml = media2::resp_video_encoder_configuration_options_media2(
            &s,
            "<tr2:GetVideoEncoderConfigurationOptions>\
               <tr2:ConfigurationToken>VEC_42</tr2:ConfigurationToken>\
             </tr2:GetVideoEncoderConfigurationOptions>",
        );
        assert!(
            xml.contains("NoSuchConfig-VECOPT2-5514: VEC_42"),
            "got: {xml}"
        );
    }

    #[test]
    fn media2_video_source_options_are_per_channel() {
        use crate::mock::services::media2;
        let s = new_state();
        let body = |t: &str| {
            format!(
                "<tr2:GetVideoSourceConfigurationOptions>\
                   <tr2:ConfigurationToken>{t}</tr2:ConfigurationToken>\
                 </tr2:GetVideoSourceConfigurationOptions>"
            )
        };
        let one = media2::resp_video_source_configuration_options_media2(&s, &body("VSC_1"));
        let two = media2::resp_video_source_configuration_options_media2(&s, &body("VSC_2"));
        assert!(one.contains("<tt:VideoSourceTokensAvailable>VS_1<"));
        assert!(two.contains("<tt:VideoSourceTokensAvailable>VS_2<"));
        assert!(one.contains("<tt:Max>2592</tt:Max>"));
        assert!(!two.contains("<tt:Max>2592</tt:Max>"));
    }

    #[test]
    fn media2_video_source_options_unknown_token_faults() {
        use crate::mock::services::media2;
        let s = new_state();
        let xml = media2::resp_video_source_configuration_options_media2(
            &s,
            "<tr2:GetVideoSourceConfigurationOptions>\
               <tr2:ConfigurationToken>VSC_42</tr2:ConfigurationToken>\
             </tr2:GetVideoSourceConfigurationOptions>",
        );
        assert!(
            xml.contains("NoSuchConfig-VSCOPT2-5512: VSC_42"),
            "got: {xml}"
        );
    }

    /// A Set must write the channel it names and leave the other three alone.
    ///
    /// The old version of this test asserted `!xml.contains("H265")` after
    /// setting the single global config to H264 — with four channels and no
    /// H265 among them that assertion is true no matter what the handler does,
    /// so it is replaced by one that reads the sibling channels.
    #[test]
    fn media2_set_video_encoder_writes_only_the_named_channel() {
        use crate::mock::services::media2;
        let s = new_state();
        let body = r#"<tr2:SetVideoEncoderConfiguration><tr2:Configuration token="VEC_3">
            <tt:Name>Retuned</tt:Name>
            <tt:Encoding>H264</tt:Encoding>
            <tt:Resolution><tt:Width>704</tt:Width><tt:Height>480</tt:Height></tt:Resolution>
            <tt:RateControl><tt:FrameRateLimit>12</tt:FrameRateLimit><tt:BitrateLimit>777</tt:BitrateLimit></tt:RateControl>
            <tt:GovLength>60</tt:GovLength>
            <tt:Profile>High</tt:Profile>
            <tt:Quality>6</tt:Quality>
          </tr2:Configuration></tr2:SetVideoEncoderConfiguration>"#;
        let resp = media2::handle_set_video_encoder_configuration(&s, body);
        assert!(resp.contains("SetVideoEncoderConfigurationResponse"));
        assert!(!resp.contains("Fault"));

        let snap = s.read();
        let by = |t: &str| snap.video_encoders.iter().find(|c| c.token == t).unwrap();
        let three = by("VEC_3");
        assert_eq!(three.name, "Retuned");
        assert_eq!((three.width, three.height), (704, 480));
        assert_eq!(three.bitrate_limit, 777);
        assert_eq!(three.gov_length, 60);
        // The other three keep their factory values — in particular VEC_1 must
        // not have picked up VEC_3's bitrate.
        assert_eq!(by("VEC_1").bitrate_limit, 4096);
        assert_eq!(by("VEC_1").name, "MainStream");
        assert_eq!(by("VEC_2").bitrate_limit, 1024);
        assert_eq!(by("VEC_4").bitrate_limit, 512);
    }

    #[test]
    fn media2_set_video_encoder_without_token_faults() {
        use crate::mock::services::media2;
        let s = new_state();
        let resp = media2::handle_set_video_encoder_configuration(
            &s,
            "<tr2:SetVideoEncoderConfiguration><tr2:Configuration>\
               <tt:Name>Nope</tt:Name>\
             </tr2:Configuration></tr2:SetVideoEncoderConfiguration>",
        );
        assert!(resp.contains("NoConfigToken-SETVEC2-5515"), "got: {resp}");
        // And no channel was renamed on the way past.
        assert!(s.read().video_encoders.iter().all(|c| c.name != "Nope"));
    }

    #[test]
    fn media2_set_video_encoder_unknown_token_faults() {
        use crate::mock::services::media2;
        let s = new_state();
        let resp = media2::handle_set_video_encoder_configuration(
            &s,
            r#"<tr2:SetVideoEncoderConfiguration><tr2:Configuration token="VEC_88">
               <tt:Name>Nope</tt:Name>
             </tr2:Configuration></tr2:SetVideoEncoderConfiguration>"#,
        );
        assert!(
            resp.contains("NoSuchConfig-SETVEC2-5516: VEC_88"),
            "got: {resp}"
        );
        assert!(s.read().video_encoders.iter().all(|c| c.name != "Nope"));
    }

    // ── Imaging: every operation is per-VideoSourceToken ──────────────────
    //
    // Same rule as the video encoder options above, applied to the service
    // where *every* method carries the token. The two lenses differ in three
    // independent ways — level scale, focus support, and current values — so
    // an assertion that survives a token being ignored has to be trying.

    fn img_body(op: &str, token: &str) -> String {
        format!("<timg:{op}><timg:VideoSourceToken>{token}</timg:VideoSourceToken></timg:{op}>")
    }

    #[test]
    fn default_imaging_sources_cover_both_sensors_and_disagree() {
        let s = new_state();
        let snap = s.read();
        let tokens: Vec<&str> = snap
            .imaging_sources
            .iter()
            .map(|i| i.source_token.as_str())
            .collect();
        assert_eq!(tokens, ["VS_1", "VS_2"]);

        // Each imaging entry must name a sensor that exists.
        for i in &snap.imaging_sources {
            assert!(
                snap.video_sources.iter().any(|v| v.token == i.source_token),
                "imaging entry for unknown sensor {}",
                i.source_token
            );
        }
        let vs1 = &snap.imaging_sources[0];
        let vs2 = &snap.imaging_sources[1];
        // The three properties the per-channel tests lean on. If any of these
        // becomes an equality the corresponding test stops discriminating.
        assert_ne!(vs1.brightness, vs2.brightness);
        assert_ne!(vs1.level_max, vs2.level_max);
        assert!(vs1.focus_supported && !vs2.focus_supported);
    }

    #[test]
    fn imaging_settings_are_per_source() {
        use crate::mock::services::imaging;
        let s = new_state();
        let one = imaging::resp_imaging_settings(&s, &img_body("GetImagingSettings", "VS_1"));
        let two = imaging::resp_imaging_settings(&s, &img_body("GetImagingSettings", "VS_2"));

        assert!(one.contains("<tt:Brightness>60</tt:Brightness>"), "{one}");
        assert!(two.contains("<tt:Brightness>45</tt:Brightness>"), "{two}");
        assert!(one.contains("<tt:IrCutFilter>AUTO</tt:IrCutFilter>"));
        assert!(two.contains("<tt:IrCutFilter>ON</tt:IrCutFilter>"));
        // The fixed lens omits tt:Focus entirely — it is [0..1] in the schema
        // and a lens with no motor has no auto-focus mode to report.
        assert!(one.contains("<tt:AutoFocusMode>AUTO</tt:AutoFocusMode>"));
        assert!(!two.contains("<tt:Focus>"));
    }

    #[test]
    fn imaging_settings_without_token_faults() {
        use crate::mock::services::imaging;
        let s = new_state();
        let xml = imaging::resp_imaging_settings(&s, "<timg:GetImagingSettings/>");
        assert!(xml.contains("NoVideoSourceToken-IMGSET-5601"), "got: {xml}");
        assert!(!xml.contains("<tt:Brightness>"));
    }

    #[test]
    fn imaging_settings_unknown_source_faults() {
        use crate::mock::services::imaging;
        let s = new_state();
        let xml =
            imaging::resp_imaging_settings(&s, &img_body("GetImagingSettings", "VideoSource_1"));
        assert!(
            xml.contains("NoSuchVideoSource-IMGSET-5602: VideoSource_1"),
            "got: {xml}"
        );
    }

    #[test]
    fn imaging_options_are_per_source() {
        use crate::mock::services::imaging;
        let s = new_state();
        let one = imaging::resp_imaging_options(&s, &img_body("GetOptions", "VS_1"));
        let two = imaging::resp_imaging_options(&s, &img_body("GetOptions", "VS_2"));

        // Different level scales — the single number that separates the two.
        assert!(one.contains("<tt:Max>100</tt:Max>"), "{one}");
        assert!(two.contains("<tt:Max>255</tt:Max>"), "{two}");
        assert!(!two.contains("<tt:Max>100</tt:Max>"));
        // And the fixed lens offers no auto-focus modes at all.
        // Spelled as the schema has it — `tt:FocusOptions20/AutoFocusModes`.
        assert!(one.contains("<tt:AutoFocusModes>AUTO</tt:AutoFocusModes>"));
        assert!(!two.contains("<tt:AutoFocusModes>"));
    }

    #[test]
    fn imaging_options_unknown_source_faults() {
        use crate::mock::services::imaging;
        let s = new_state();
        let xml = imaging::resp_imaging_options(&s, &img_body("GetOptions", "VS_7"));
        assert!(
            xml.contains("NoSuchVideoSource-IMGOPT-5606: VS_7"),
            "got: {xml}"
        );
    }

    #[test]
    fn imaging_status_reports_focus_only_where_there_is_one() {
        use crate::mock::services::imaging;
        let s = new_state();
        let one = imaging::resp_imaging_status(&s, &img_body("GetStatus", "VS_1"));
        let two = imaging::resp_imaging_status(&s, &img_body("GetStatus", "VS_2"));

        assert!(one.contains("<tt:FocusStatus20>"));
        assert!(one.contains("<tt:Position>0.5</tt:Position>"));
        // Legal empty Status — FocusStatus20 is the type's only content and it
        // is [0..1], so a caller has to survive its absence.
        assert!(!two.contains("<tt:FocusStatus20>"));
        assert!(two.contains("<timg:Status></timg:Status>"), "{two}");
    }

    #[test]
    fn imaging_move_options_fault_on_a_fixed_lens() {
        use crate::mock::services::imaging;
        let s = new_state();
        let one = imaging::resp_imaging_move_options(&s, &img_body("GetMoveOptions", "VS_1"));
        assert!(one.contains("<tt:PositionSpace>"), "{one}");

        let two = imaging::resp_imaging_move_options(&s, &img_body("GetMoveOptions", "VS_2"));
        assert!(
            two.contains("NoFocusSupport-IMGMOVEOPT-5611: VS_2"),
            "got: {two}"
        );
        assert!(!two.contains("<tt:PositionSpace>"));
    }

    #[test]
    fn imaging_move_and_stop_fault_on_a_fixed_lens() {
        use crate::mock::services::imaging;
        let s = new_state();
        assert!(
            imaging::handle_imaging_move(&s, &img_body("Move", "VS_1")).contains("MoveResponse")
        );
        assert!(
            imaging::handle_imaging_stop(&s, &img_body("Stop", "VS_1")).contains("StopResponse")
        );

        let m = imaging::handle_imaging_move(&s, &img_body("Move", "VS_2"));
        assert!(m.contains("NoFocusSupport-IMGMOVE-5614: VS_2"), "got: {m}");
        let st = imaging::handle_imaging_stop(&s, &img_body("Stop", "VS_2"));
        assert!(
            st.contains("NoFocusSupport-IMGSTOP-5617: VS_2"),
            "got: {st}"
        );
    }

    #[test]
    fn imaging_move_without_token_faults() {
        use crate::mock::services::imaging;
        let s = new_state();
        let xml = imaging::handle_imaging_move(&s, "<timg:Move/>");
        assert!(
            xml.contains("NoVideoSourceToken-IMGMOVE-5612"),
            "got: {xml}"
        );
    }

    /// A Set writes the lens it names and leaves the other alone.
    #[test]
    fn set_imaging_settings_writes_only_the_named_source() {
        use crate::mock::services::imaging;
        let s = new_state();
        let body = r#"<timg:SetImagingSettings>
            <timg:VideoSourceToken>VS_2</timg:VideoSourceToken>
            <timg:ImagingSettings>
              <tt:Brightness>7</tt:Brightness>
              <tt:IrCutFilter>OFF</tt:IrCutFilter>
            </timg:ImagingSettings>
          </timg:SetImagingSettings>"#;
        let resp = imaging::handle_set_imaging_settings(&s, body);
        assert!(resp.contains("SetImagingSettingsResponse"));
        assert!(!resp.contains("Fault"));

        let snap = s.read();
        let by = |t: &str| {
            snap.imaging_sources
                .iter()
                .find(|i| i.source_token == t)
                .unwrap()
        };
        assert_eq!(by("VS_2").brightness, 7.0);
        assert_eq!(by("VS_2").ir_cut_filter, "OFF");
        // VS_1 untouched — the value it would have picked up is distinctive.
        assert_eq!(by("VS_1").brightness, 60.0);
        assert_eq!(by("VS_1").ir_cut_filter, "AUTO");
    }

    #[test]
    fn set_imaging_settings_unknown_source_writes_nothing() {
        use crate::mock::services::imaging;
        let s = new_state();
        let body = r#"<timg:SetImagingSettings>
            <timg:VideoSourceToken>VS_9</timg:VideoSourceToken>
            <timg:ImagingSettings><tt:Brightness>7</tt:Brightness></timg:ImagingSettings>
          </timg:SetImagingSettings>"#;
        let resp = imaging::handle_set_imaging_settings(&s, body);
        assert!(
            resp.contains("NoSuchVideoSource-IMGSETW-5604: VS_9"),
            "got: {resp}"
        );
        // Neither lens moved.
        assert!(s.read().imaging_sources.iter().all(|i| i.brightness != 7.0));
    }

    #[test]
    fn set_imaging_settings_without_token_writes_nothing() {
        use crate::mock::services::imaging;
        let s = new_state();
        let resp = imaging::handle_set_imaging_settings(
            &s,
            "<timg:SetImagingSettings><timg:ImagingSettings>\
               <tt:Brightness>7</tt:Brightness>\
             </timg:ImagingSettings></timg:SetImagingSettings>",
        );
        assert!(
            resp.contains("NoVideoSourceToken-IMGSETW-5603"),
            "got: {resp}"
        );
        assert!(s.read().imaging_sources.iter().all(|i| i.brightness != 7.0));
    }
}
