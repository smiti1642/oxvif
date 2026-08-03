use crate::mock::helpers::{resp_empty, resp_soap_fault, soap};
use crate::mock::state::{
    AudioEncoderEntry, AudioOptionEntry, AudioSourceConfigEntry, MulticastEntry, OSD_QUOTA_DATE,
    OSD_QUOTA_DATE_AND_TIME, OSD_QUOTA_PLAIN, OSD_QUOTA_TIME, OSD_QUOTA_TOTAL, OsdColorEntry,
    OsdEntry, OsdTextEntry, ProfileEntry, PtzConfigEntry, SharedState, VideoEncoderState,
    VideoSourceConfigEntry,
};
use crate::mock::xml_parse::{extract_all_tags, extract_attr, extract_tag};

pub fn resp_profiles(state: &SharedState) -> String {
    let snapshot = state.read().profiles.profiles.clone();
    let cat = catalogues(state);
    let items: String = snapshot
        .iter()
        .map(|p| render_profile(p, "Profiles", &cat))
        .collect();
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!("<trt:GetProfilesResponse>{items}</trt:GetProfilesResponse>"),
    )
}

pub fn resp_profile(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "GetProfile").unwrap_or_default();
    let want = extract_tag(&inner, "ProfileToken").unwrap_or_default();
    let snapshot = state.read().profiles.profiles.clone();
    let cat = catalogues(state);
    match snapshot.iter().find(|p| p.token == want) {
        Some(p) => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            &format!(
                "<trt:GetProfileResponse>{}</trt:GetProfileResponse>",
                render_profile(p, "Profile", &cat)
            ),
        ),
        None => resp_soap_fault("ter:NoProfile", &format!("Profile not found: {want}")),
    }
}

pub fn resp_stream_uri() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetStreamUriResponse>
          <trt:MediaUri>
            <tt:Uri>rtsp://127.0.0.1:554/mock/stream</tt:Uri>
            <tt:InvalidAfterConnect>false</tt:InvalidAfterConnect>
            <tt:InvalidAfterReboot>false</tt:InvalidAfterReboot>
            <tt:Timeout>PT0S</tt:Timeout>
          </trt:MediaUri>
        </trt:GetStreamUriResponse>"#,
    )
}

pub fn resp_snapshot_uri(base: &str) -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            r#"<trt:GetSnapshotUriResponse>
          <trt:MediaUri>
            <tt:Uri>{base}/mock/snapshot.jpg</tt:Uri>
            <tt:InvalidAfterConnect>false</tt:InvalidAfterConnect>
            <tt:InvalidAfterReboot>false</tt:InvalidAfterReboot>
            <tt:Timeout>PT0S</tt:Timeout>
          </trt:MediaUri>
        </trt:GetSnapshotUriResponse>"#
        ),
    )
}

pub fn handle_set_video_encoder_configuration(state: &SharedState, body: &str) -> String {
    match apply_video_encoder_write(
        state,
        body,
        "NoConfigToken-SETVEC-5517",
        "NoSuchConfig-SETVEC-5518",
    ) {
        Ok(()) => resp_empty("trt", "SetVideoEncoderConfigurationResponse"),
        Err(fault) => fault,
    }
}

pub fn handle_set_video_source_configuration(state: &SharedState, body: &str) -> String {
    match apply_video_source_write(
        state,
        body,
        "NoConfigToken-SETVSC-5521",
        "NoSuchConfig-SETVSC-5522",
    ) {
        Ok(()) => resp_empty("trt", "SetVideoSourceConfigurationResponse"),
        Err(fault) => fault,
    }
}

pub fn handle_add_video_encoder_configuration(state: &SharedState, body: &str) -> String {
    match bind_configuration(state, body, ConfigKind::VideoEncoder, "ADDVEC-5531") {
        Ok(()) => resp_empty("trt", "AddVideoEncoderConfigurationResponse"),
        Err(fault) => fault,
    }
}

pub fn handle_remove_video_encoder_configuration(state: &SharedState, body: &str) -> String {
    match unbind_configuration(state, body, ConfigKind::VideoEncoder, "RMVEC-5532") {
        Ok(()) => resp_empty("trt", "RemoveVideoEncoderConfigurationResponse"),
        Err(fault) => fault,
    }
}

pub fn handle_add_video_source_configuration(state: &SharedState, body: &str) -> String {
    match bind_configuration(state, body, ConfigKind::VideoSource, "ADDVSC-5533") {
        Ok(()) => resp_empty("trt", "AddVideoSourceConfigurationResponse"),
        Err(fault) => fault,
    }
}

pub fn handle_remove_video_source_configuration(state: &SharedState, body: &str) -> String {
    match unbind_configuration(state, body, ConfigKind::VideoSource, "RMVSC-5534") {
        Ok(()) => resp_empty("trt", "RemoveVideoSourceConfigurationResponse"),
        Err(fault) => fault,
    }
}

pub fn handle_create_profile(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "CreateProfile").unwrap_or_default();
    let name = extract_tag(&inner, "Name").unwrap_or_else(|| "Profile".to_string());
    // Caller may supply an explicit token (rare — most cameras assign).
    let supplied_token = extract_tag(&inner, "Token");

    let entry = match create_profile_in_state(state, &name, supplied_token) {
        CreateOutcome::Created(e) => e,
        CreateOutcome::Duplicate(t) => {
            return resp_soap_fault(
                "ter:ProfileExists",
                &format!("Profile token already in use: {t}"),
            );
        }
    };

    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            "<trt:CreateProfileResponse>{}</trt:CreateProfileResponse>",
            // A freshly created profile carries no configurations yet, so the
            // catalogues are never consulted.
            render_profile(&entry, "Profile", &catalogues(state))
        ),
    )
}

pub fn handle_delete_profile(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "DeleteProfile").unwrap_or_default();
    let token = extract_tag(&inner, "ProfileToken").unwrap_or_default();
    if token.is_empty() {
        return resp_soap_fault("ter:InvalidArgs", "ProfileToken missing");
    }

    match delete_profile_in_state(state, &token) {
        DeleteOutcome::Deleted => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            "<trt:DeleteProfileResponse/>",
        ),
        DeleteOutcome::NotFound => {
            resp_soap_fault("ter:NoProfile", &format!("Profile not found: {token}"))
        }
        DeleteOutcome::Fixed => resp_soap_fault(
            "ter:DeletionOfFixedProfile",
            &format!("Cannot delete fixed profile: {token}"),
        ),
    }
}

pub(crate) enum DeleteOutcome {
    Deleted,
    NotFound,
    /// Per ONVIF spec, fixed profiles can't be removed.
    Fixed,
}

pub(crate) enum CreateOutcome {
    Created(ProfileEntry),
    /// The caller supplied a token that is already in use.
    Duplicate(String),
}

// ── Shared profile-list operations ──────────────────────────────────────────
//
// The device has **one** profile list, and Media1 and Media2 are two views of
// it. What a create or a delete *does* lives here; each service renders its own
// envelope around the outcome.
//
// Split out in 0.15 after a report from a C++ test suite: Media2's profile
// family took no `state` at all — `GetProfiles` returned a string literal,
// `CreateProfile` returned a literal token and wrote nothing, and the
// dispatcher answered `DeleteProfile` with an unconditional empty success. A
// harness that seeded 20 profiles got 20 from Media1 and 4 from Media2, with no
// error and no overlap in the token sets. Two renderers over one state is the
// shape that cannot drift back.

/// Create a profile in the shared list. `supplied_token` is honoured verbatim
/// when the caller gives one (rare — most cameras assign); otherwise a token is
/// generated from `next_token_id`.
pub(crate) fn create_profile_in_state(
    state: &SharedState,
    name: &str,
    supplied_token: Option<String>,
) -> CreateOutcome {
    if let Some(t) = supplied_token.as_ref()
        && state.read().profiles.profiles.iter().any(|p| &p.token == t)
    {
        return CreateOutcome::Duplicate(t.clone());
    }

    let entry = state.modify_returning(|s| {
        let token = supplied_token.unwrap_or_else(|| {
            let id = s.profiles.next_token_id;
            s.profiles.next_token_id += 1;
            format!("Profile_{id}")
        });
        let entry = ProfileEntry {
            token: token.clone(),
            name: name.to_string(),
            fixed: false,
            video_source_config_token: None,
            video_encoder_config_token: None,
            audio_source_config_token: None,
            audio_encoder_config_token: None,
            ptz_config_token: None,
        };
        eprintln!("    [STATE] profile created: {token} ({name})");
        s.profiles.profiles.push(entry.clone());
        entry
    });
    CreateOutcome::Created(entry)
}

/// Apply a `SetVideoEncoderConfiguration` body to the addressed channel.
///
/// Shared by Media1 and Media2 for the same reason as the profile operations
/// above: one encoder catalogue, two request shapes. Until 0.15 **Media1's Set
/// was `resp_empty`** — it reported success and wrote nothing — while Media2's
/// wrote state, so the identical call changed the device on one service only.
/// Same class as the reported profile divergence, pointing the other way.
///
/// The two bodies differ in exactly one place: Media2's encoder config is flat
/// and carries `<tt:Profile>`, Media1 nests it as `<tt:H264Profile>` /
/// `<tt:H265Profile>`. All three are read here — a given body contains at most
/// one, so there is nothing to disambiguate and no parameter to get wrong.
///
/// `Err` is a rendered SOAP fault. The reasons are per-service so an assertion
/// can tell *which* service refused.
pub(crate) fn apply_video_encoder_write(
    state: &SharedState,
    body: &str,
    missing_reason: &str,
    unknown_prefix: &str,
) -> Result<(), String> {
    // The token *selects* which of the channels to write. An absent or unknown
    // token is a fault: with more than one encoder, writing to a guessed channel
    // is the same silent-wrong-answer failure the getters avoid.
    let Some(want) = extract_attr(body, "Configuration", "token").filter(|t| !t.is_empty()) else {
        return Err(resp_soap_fault("env:Sender", missing_reason));
    };
    if !state.read().video_encoders.iter().any(|c| c.token == want) {
        return Err(resp_soap_fault(
            "env:Sender",
            &format!("{unknown_prefix}: {want}"),
        ));
    }
    state.modify(|s| {
        let Some(ve) = s.video_encoders.iter_mut().find(|c| c.token == want) else {
            return;
        };
        if let Some(v) = extract_tag(body, "Name") {
            ve.name = v;
        }
        if let Some(v) = extract_tag(body, "Encoding") {
            ve.encoding = v;
        }
        if let Some(v) = extract_tag(body, "Width").and_then(|x| x.parse().ok()) {
            ve.width = v;
        }
        if let Some(v) = extract_tag(body, "Height").and_then(|x| x.parse().ok()) {
            ve.height = v;
        }
        if let Some(v) = extract_tag(body, "Quality").and_then(|x| x.parse().ok()) {
            ve.quality = v;
        }
        if let Some(v) = extract_tag(body, "FrameRateLimit").and_then(|x| x.parse().ok()) {
            ve.frame_rate_limit = v;
        }
        if let Some(v) = extract_tag(body, "BitrateLimit").and_then(|x| x.parse().ok()) {
            ve.bitrate_limit = v;
        }
        if let Some(v) = extract_tag(body, "GovLength").and_then(|x| x.parse().ok()) {
            ve.gov_length = v;
        }
        if let Some(v) = extract_tag(body, "Profile")
            .or_else(|| extract_tag(body, "H264Profile"))
            .or_else(|| extract_tag(body, "H265Profile"))
        {
            ve.profile = v;
        }
    });
    Ok(())
}

/// Apply a `SetVideoSourceConfiguration` body to the addressed channel.
///
/// Shared by Media1 and Media2 for the same reason as the encoder write above,
/// and it is the same defect: both dispatchers answered this with `resp_empty`
/// until 0.15 — a success that wrote nothing, over a getter that *is*
/// state-driven. Audit §3 items 1.1 and 1.2.
///
/// The two request bodies are identical apart from the prefix
/// (`<trt:Configuration>` vs `<tr2:Configuration>`), and `extract_attr` /
/// `extract_tag` both match on the local name, so one reader serves both.
///
/// `Bounds/@x` and `@y` are read from the wire and dropped: `VideoSourceConfigEntry`
/// models a size, not an offset, and every renderer emits `x="0" y="0"`. Writing
/// them into a field that does not exist is not possible; **saying so here is
/// what keeps it from looking like the `MTU` case in item 1.8.**
pub(crate) fn apply_video_source_write(
    state: &SharedState,
    body: &str,
    missing_reason: &str,
    unknown_prefix: &str,
) -> Result<(), String> {
    let Some(want) = extract_attr(body, "Configuration", "token").filter(|t| !t.is_empty()) else {
        return Err(resp_soap_fault("env:Sender", missing_reason));
    };
    if !state
        .read()
        .video_source_configs
        .iter()
        .any(|c| c.token == want)
    {
        return Err(resp_soap_fault(
            "env:Sender",
            &format!("{unknown_prefix}: {want}"),
        ));
    }
    state.modify(|s| {
        let Some(vsc) = s.video_source_configs.iter_mut().find(|c| c.token == want) else {
            return;
        };
        if let Some(v) = extract_tag(body, "Name") {
            vsc.name = v;
        }
        if let Some(v) = extract_tag(body, "SourceToken") {
            vsc.source_token = v;
        }
        if let Some(v) = extract_attr(body, "Bounds", "width").and_then(|x| x.parse().ok()) {
            vsc.width = v;
        }
        if let Some(v) = extract_attr(body, "Bounds", "height").and_then(|x| x.parse().ok()) {
            vsc.height = v;
        }
        eprintln!("    [STATE] video source config updated: {want}");
    });
    Ok(())
}

/// Which slot of a [`ProfileEntry`] a configuration binding writes to.
///
/// Media1 encodes the kind in the *operation name*
/// (`AddVideoEncoderConfiguration`); Media2 encodes it in a `<tr2:Type>` element
/// of one generic `AddConfiguration`. Same slots either way, so the kind is
/// resolved at the edge and the state operation below is shared.
///
/// `Ptz` is Media2-only: oxvif has no `AddPTZConfiguration`
/// (`docs/reference/media1.md`), so there is no Media1 arm to keep in step with
/// it and no divergence for `tests/mock_media1_media2_agree.rs` to audit.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigKind {
    VideoSource,
    VideoEncoder,
    AudioSource,
    AudioEncoder,
    Ptz,
}

impl ConfigKind {
    /// Media2's `<tr2:Type>` spelling. `None` for a type the mock does not model
    /// — see `media2::handle_add_configuration_media2` for why that faults.
    pub(crate) fn from_media2_type(t: &str) -> Option<Self> {
        match t {
            "VideoSource" => Some(Self::VideoSource),
            "VideoEncoder" => Some(Self::VideoEncoder),
            "AudioSource" => Some(Self::AudioSource),
            "AudioEncoder" => Some(Self::AudioEncoder),
            "PTZ" => Some(Self::Ptz),
            _ => None,
        }
    }

    fn slot(self, p: &mut ProfileEntry) -> &mut Option<String> {
        match self {
            Self::VideoSource => &mut p.video_source_config_token,
            Self::VideoEncoder => &mut p.video_encoder_config_token,
            Self::AudioSource => &mut p.audio_source_config_token,
            Self::AudioEncoder => &mut p.audio_encoder_config_token,
            Self::Ptz => &mut p.ptz_config_token,
        }
    }

    /// `true` if `DeviceState` carries a catalogue this token can be checked
    /// against. The audio families are static fixtures (audit §5), so there is
    /// nothing to validate a binding against and none is attempted.
    fn known_token(self, state: &SharedState, token: &str) -> bool {
        let s = state.read();
        match self {
            Self::VideoSource => s.video_source_configs.iter().any(|c| c.token == token),
            Self::VideoEncoder => s.video_encoders.iter().any(|c| c.token == token),
            Self::Ptz => s.ptz_configs.iter().any(|c| c.token == token),
            Self::AudioSource | Self::AudioEncoder => true,
        }
    }
}

/// Bind a configuration to a profile in the shared list.
///
/// Audit §3 items 1.4, 1.6 and 1.7: the whole Add/Remove family was `resp_empty`
/// in the dispatcher, which meant **a profile could not be assembled on the
/// mock at all** — create one, add an encoder, read it back, still empty. Any
/// test of profile-assembly logic passed without exercising anything.
///
/// A *fixed* profile is deliberately still bindable. Real devices refuse, but
/// the mock's four seeded profiles are all fixed, so refusing would leave only
/// freshly created profiles reachable and would flip
/// `tests/mock_action_snapshot.rs` from `ok` to `fault` on two operations.
/// Enforcing it belongs with the rest of the fidelity work, not here.
pub(crate) fn bind_configuration(
    state: &SharedState,
    body: &str,
    kind: ConfigKind,
    tag: &str,
) -> Result<(), String> {
    let profile = extract_tag(body, "ProfileToken").unwrap_or_default();
    let config = extract_tag(body, "ConfigurationToken")
        .or_else(|| extract_tag(body, "Token"))
        .unwrap_or_default();
    if profile.is_empty() || config.is_empty() {
        return Err(resp_soap_fault(
            "env:Sender",
            &format!("NoToken-{tag}: ProfileToken and ConfigurationToken are both required"),
        ));
    }
    if !state
        .read()
        .profiles
        .profiles
        .iter()
        .any(|p| p.token == profile)
    {
        return Err(resp_soap_fault(
            "ter:NoProfile",
            &format!("NoSuchProfile-{tag}: {profile}"),
        ));
    }
    if !kind.known_token(state, &config) {
        return Err(resp_soap_fault(
            "ter:NoConfig",
            &format!("NoSuchConfig-{tag}: {config}"),
        ));
    }
    state.modify(|s| {
        if let Some(p) = s.profiles.profiles.iter_mut().find(|p| p.token == profile) {
            *kind.slot(p) = Some(config.clone());
            eprintln!("    [STATE] profile {profile}: bound {config}");
        }
    });
    Ok(())
}

/// Clear a configuration slot on a profile. Audit §3 items 1.5, 1.6 and 1.7.
///
/// Removing a slot that is already empty is **not** a fault — the operation is
/// idempotent and ONVIF does not require a device to complain.
pub(crate) fn unbind_configuration(
    state: &SharedState,
    body: &str,
    kind: ConfigKind,
    tag: &str,
) -> Result<(), String> {
    let profile = extract_tag(body, "ProfileToken").unwrap_or_default();
    if profile.is_empty() {
        return Err(resp_soap_fault(
            "env:Sender",
            &format!("NoToken-{tag}: ProfileToken is required"),
        ));
    }
    if !state
        .read()
        .profiles
        .profiles
        .iter()
        .any(|p| p.token == profile)
    {
        return Err(resp_soap_fault(
            "ter:NoProfile",
            &format!("NoSuchProfile-{tag}: {profile}"),
        ));
    }
    state.modify(|s| {
        if let Some(p) = s.profiles.profiles.iter_mut().find(|p| p.token == profile) {
            *kind.slot(p) = None;
            eprintln!("    [STATE] profile {profile}: unbound");
        }
    });
    Ok(())
}

/// Remove a profile from the shared list, refusing a fixed one.
pub(crate) fn delete_profile_in_state(state: &SharedState, token: &str) -> DeleteOutcome {
    state.modify_returning(|s| {
        let Some(idx) = s.profiles.profiles.iter().position(|p| p.token == token) else {
            return DeleteOutcome::NotFound;
        };
        if s.profiles.profiles[idx].fixed {
            return DeleteOutcome::Fixed;
        }
        s.profiles.profiles.remove(idx);
        eprintln!("    [STATE] profile deleted: {token}");
        DeleteOutcome::Deleted
    })
}

// ── Profile render helpers ──────────────────────────────────────────────────
//
// Profiles are rendered with full nested configuration objects (the
// shape real cameras use), and every one of them is read from `DeviceState`
// through `Catalogues` — video source, video encoder, PTZ, audio source and
// audio encoder alike. Until 0.15 the details for VSC_1, VEC_1 and VEC_2 were
// literals here, which is how `VEC_2` came to have three names in three files.

fn render_profile(p: &ProfileEntry, tag: &str, cat: &Catalogues) -> String {
    let vsc = p
        .video_source_config_token
        .as_deref()
        .map(|t| render_vsc_inline(&cat.vscs, t))
        .unwrap_or_default();
    let vec = p
        .video_encoder_config_token
        .as_deref()
        .map(|t| render_vec_inline(&cat.vecs, t))
        .unwrap_or_default();
    // Both were `match token { "ASC_1" => <literal>, _ => "" }`, so a profile
    // bound to any other token rendered **nothing** and said so nowhere.
    let asc = p
        .audio_source_config_token
        .as_deref()
        .and_then(|t| cat.ascs.iter().find(|c| c.token == t))
        .map(|c| render_audio_source_config(c, "tt:AudioSourceConfiguration"))
        .unwrap_or_default();
    let aec = p
        .audio_encoder_config_token
        .as_deref()
        .and_then(|t| cat.aecs.iter().find(|c| c.token == t))
        .map(|c| render_audio_encoder(c, "tt:AudioEncoderConfiguration"))
        .unwrap_or_default();
    // `MediaProfile::ptz_config_token` reads `Profile/PTZConfiguration@token`
    // and nothing ever fed it. Media1 inlines the whole configuration, as it
    // does for every other kind; Media2 emits a token reference. The body comes
    // from `ptz::render_config` so the two services cannot drift.
    let ptz = p
        .ptz_config_token
        .as_deref()
        .and_then(|t| cat.ptzs.iter().find(|c| c.token == t))
        .map(|c| super::ptz::render_config(c, "tt:PTZConfiguration"))
        .unwrap_or_default();
    format!(
        r#"<trt:{tag} token="{token}" fixed="{fixed}">
          <tt:Name>{name}</tt:Name>
          {vsc}{vec}{asc}{aec}{ptz}
        </trt:{tag}>"#,
        token = p.token,
        fixed = p.fixed,
        name = p.name,
    )
}

/// Every catalogue a profile can inline, cloned under a **single** read lock.
///
/// One lock rather than five keeps a responder from rendering a profile whose
/// video and audio configurations came from different moments. It was a
/// three-tuple until the audio catalogue joined it.
struct Catalogues {
    vscs: Vec<VideoSourceConfigEntry>,
    vecs: Vec<VideoEncoderState>,
    ptzs: Vec<PtzConfigEntry>,
    ascs: Vec<AudioSourceConfigEntry>,
    aecs: Vec<AudioEncoderEntry>,
}

fn catalogues(state: &SharedState) -> Catalogues {
    let s = state.read();
    Catalogues {
        vscs: s.video_source_configs.clone(),
        vecs: s.video_encoders.clone(),
        ptzs: s.ptz_configs.clone(),
        ascs: s.audio_source_configs.clone(),
        aecs: s.audio_encoders.clone(),
    }
}

fn render_vsc_inline(vscs: &[VideoSourceConfigEntry], token: &str) -> String {
    match vscs.iter().find(|c| c.token == token) {
        Some(c) => render_vsc_body(c, "tt:VideoSourceConfiguration"),
        None => String::new(),
    }
}

/// The shared `VideoSourceConfiguration` payload. `tag` differs by context —
/// `tt:VideoSourceConfiguration` inline in a profile, `trt:Configurations` in
/// a list, `trt:Configuration` for the singular getter.
fn render_vsc_body(c: &VideoSourceConfigEntry, tag: &str) -> String {
    format!(
        r#"<{tag} token="{token}">
          <tt:Name>{name}</tt:Name>
          <tt:UseCount>{use_count}</tt:UseCount>
          <tt:SourceToken>{source}</tt:SourceToken>
          <tt:Bounds x="0" y="0" width="{width}" height="{height}"/>
        </{tag}>"#,
        token = c.token,
        name = c.name,
        use_count = c.use_count,
        source = c.source_token,
        width = c.width,
        height = c.height,
    )
}

/// The `Multicast` + `SessionTimeout` tail required to close a schema-valid
/// `tt:VideoEncoderConfiguration` (both are `[1]` in the XSD sequence).
const VEC_TAIL: &str = concat!(
    "<tt:Multicast><tt:Address><tt:Type>IPv4</tt:Type>",
    "<tt:IPv4Address>0.0.0.0</tt:IPv4Address></tt:Address>",
    "<tt:Port>0</tt:Port><tt:TTL>1</tt:TTL><tt:AutoStart>false</tt:AutoStart></tt:Multicast>",
    "<tt:SessionTimeout>PT0S</tt:SessionTimeout>",
);

fn render_vec_inline(vecs: &[VideoEncoderState], token: &str) -> String {
    match vecs.iter().find(|c| c.token == token) {
        Some(c) => render_vec_body(c, "tt:VideoEncoderConfiguration"),
        None => String::new(),
    }
}

/// The shared `VideoEncoderConfiguration` payload, rendered from state.
///
/// Before 0.15 there were three hardcoded copies of this element — inline in a
/// profile, in the `GetVideoEncoderConfigurations` list, and in the singular
/// getter — and they disagreed: `VEC_2` was `H264_sub`/H264/640x480 in one and
/// `SubStream`/JPEG/640x480 in another. Rendering all three from one state
/// entry makes that class of drift unrepresentable.
///
/// The `tt:H264` block is emitted only for H264, because the schema element is
/// encoding-specific; a JPEG config carrying `tt:H264` is not something a
/// conformant device sends.
fn render_vec_body(c: &VideoEncoderState, tag: &str) -> String {
    let codec = if c.encoding == "H264" {
        format!(
            "<tt:H264><tt:GovLength>{gov}</tt:GovLength>\
             <tt:H264Profile>{profile}</tt:H264Profile></tt:H264>",
            gov = c.gov_length,
            profile = c.profile,
        )
    } else {
        String::new()
    };
    format!(
        r#"<{tag} token="{token}">
          <tt:Name>{name}</tt:Name>
          <tt:UseCount>{use_count}</tt:UseCount>
          <tt:Encoding>{encoding}</tt:Encoding>
          <tt:Resolution><tt:Width>{width}</tt:Width><tt:Height>{height}</tt:Height></tt:Resolution>
          <tt:Quality>{quality}</tt:Quality>
          <tt:RateControl><tt:FrameRateLimit>{fps}</tt:FrameRateLimit><tt:EncodingInterval>1</tt:EncodingInterval><tt:BitrateLimit>{bitrate}</tt:BitrateLimit></tt:RateControl>
          {codec}{VEC_TAIL}
        </{tag}>"#,
        token = c.token,
        name = c.name,
        use_count = c.use_count,
        encoding = c.encoding,
        width = c.width,
        height = c.height,
        quality = c.quality,
        fps = c.frame_rate_limit,
        bitrate = c.bitrate_limit,
    )
}

/// `<tt:Multicast>` — one shape, shared by every configuration that has one.
pub(crate) fn render_multicast(m: &MulticastEntry) -> String {
    format!(
        "<tt:Multicast>\
           <tt:Address><tt:Type>IPv4</tt:Type><tt:IPv4Address>{addr}</tt:IPv4Address></tt:Address>\
           <tt:Port>{port}</tt:Port>\
           <tt:TTL>{ttl}</tt:TTL>\
           <tt:AutoStart>{auto}</tt:AutoStart>\
         </tt:Multicast>",
        addr = m.address,
        port = m.port,
        ttl = m.ttl,
        auto = m.auto_start,
    )
}

/// An audio source configuration, in whichever element the caller needs.
///
/// Both services render the *same* entry: `ASC_1` was `AudioSourceConfig1`
/// reading `AudioSource_1` on Media1 and `AudioSourceConfig` reading
/// `AudioSrc_1` on Media2 — one token, two answers, and no test could see it.
pub(crate) fn render_audio_source_config(c: &AudioSourceConfigEntry, qname: &str) -> String {
    format!(
        r#"<{qname} token="{token}">
          <tt:Name>{name}</tt:Name>
          <tt:UseCount>{use_count}</tt:UseCount>
          <tt:SourceToken>{source}</tt:SourceToken>
        </{qname}>"#,
        token = c.token,
        name = c.name,
        use_count = c.use_count,
        source = c.source_token,
    )
}

/// An audio encoder configuration **in Media1's sequence**:
/// `Encoding, Bitrate, SampleRate, Multicast, SessionTimeout`, the last two
/// required by `tt:AudioEncoderConfiguration`.
///
/// Media2's `tt:AudioEncoder2Configuration` is a different type with a
/// different order and no `SessionTimeout`; it renders in `services/media2.rs`
/// from this same entry.
pub(crate) fn render_audio_encoder(c: &AudioEncoderEntry, qname: &str) -> String {
    let multicast = c
        .multicast
        .as_ref()
        .map(render_multicast)
        .unwrap_or_default();
    let timeout = c
        .session_timeout
        .as_deref()
        .map(|t| format!("<tt:SessionTimeout>{t}</tt:SessionTimeout>"))
        .unwrap_or_default();
    format!(
        r#"<{qname} token="{token}">
          <tt:Name>{name}</tt:Name>
          <tt:UseCount>{use_count}</tt:UseCount>
          <tt:Encoding>{encoding}</tt:Encoding>
          <tt:Bitrate>{bitrate}</tt:Bitrate>
          <tt:SampleRate>{sample_rate}</tt:SampleRate>
          {multicast}{timeout}
        </{qname}>"#,
        token = c.token,
        name = c.name,
        use_count = c.use_count,
        encoding = c.encoding,
        bitrate = c.bitrate,
        sample_rate = c.sample_rate,
    )
}

pub fn resp_video_sources(state: &SharedState) -> String {
    let sources = state.read().video_sources.clone();
    let items: String = sources
        .iter()
        .map(|s| {
            format!(
                r#"<trt:VideoSources token="{token}">
            <tt:Framerate>{fps}</tt:Framerate>
            <tt:Resolution><tt:Width>{width}</tt:Width><tt:Height>{height}</tt:Height></tt:Resolution>
          </trt:VideoSources>"#,
                token = s.token,
                fps = s.framerate,
                width = s.width,
                height = s.height,
            )
        })
        .collect();
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!("<trt:GetVideoSourcesResponse>{items}</trt:GetVideoSourcesResponse>"),
    )
}

pub fn resp_video_source_configurations(state: &SharedState) -> String {
    let vscs = state.read().video_source_configs.clone();
    let items: String = vscs
        .iter()
        .map(|c| render_vsc_body(c, "trt:Configurations"))
        .collect();
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            "<trt:GetVideoSourceConfigurationsResponse>{items}</trt:GetVideoSourceConfigurationsResponse>"
        ),
    )
}

/// `GetVideoEncoderConfigurations` — the whole catalogue, or one entry when the
/// request carries a `ConfigurationToken`.
///
/// The token is genuinely optional here (the plural getter means "list them"),
/// which is why an absent token returns everything rather than faulting. That
/// is *not* true of the singular and Options getters below.
pub fn resp_video_encoder_configurations(state: &SharedState, body: &str) -> String {
    let vecs = state.read().video_encoders.clone();
    let want = extract_tag(body, "ConfigurationToken").filter(|t| !t.is_empty());
    let items: String = vecs
        .iter()
        .filter(|c| want.as_deref().is_none_or(|t| t == c.token))
        .map(|c| render_vec_body(c, "trt:Configurations"))
        .collect();
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            "<trt:GetVideoEncoderConfigurationsResponse>{items}</trt:GetVideoEncoderConfigurationsResponse>"
        ),
    )
}

pub fn resp_audio_sources(state: &SharedState) -> String {
    let items: String = state
        .read()
        .audio_sources
        .iter()
        .map(|s| {
            format!(
                r#"<trt:AudioSources token="{token}"><tt:Channels>{ch}</tt:Channels></trt:AudioSources>"#,
                token = s.token,
                ch = s.channels,
            )
        })
        .collect();
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!("<trt:GetAudioSourcesResponse>{items}</trt:GetAudioSourcesResponse>"),
    )
}

pub fn resp_audio_encoder_configurations(state: &SharedState) -> String {
    let items: String = state
        .read()
        .audio_encoders
        .iter()
        .map(|c| render_audio_encoder(c, "trt:Configurations"))
        .collect();
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            "<trt:GetAudioEncoderConfigurationsResponse>{items}\
             </trt:GetAudioEncoderConfigurationsResponse>"
        ),
    )
}

pub fn resp_osds(state: &SharedState, body: &str) -> String {
    // Optional <ConfigurationToken> filter — only return OSDs attached
    // to that VSC. Real cameras vary on whether they apply this filter
    // strictly; we honour it when present, return all when absent.
    let inner = extract_tag(body, "GetOSDs").unwrap_or_default();
    let filter = extract_tag(&inner, "ConfigurationToken");

    let snapshot = state.read().osd.osds.clone();
    let items: String = snapshot
        .iter()
        .filter(|o| {
            filter
                .as_deref()
                .is_none_or(|t| o.video_source_config_token == t)
        })
        .map(render_osd_entry)
        .collect();

    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!("<trt:GetOSDsResponse>{items}</trt:GetOSDsResponse>"),
    )
}

/// The `ConfigurationToken` of a **per-channel** request, or a SOAP Fault.
///
/// Absent is an error, not a default. On a multi-sensor device, answering a
/// token-less per-channel query means picking a channel on the caller's behalf,
/// and the pick is invisible — measured on a real two-sensor device
/// (2026-07-28), a token-less `GetVideoEncoderConfigurationOptions` returned
/// lens 0's resolution list, which the caller would then display for lens 1
/// too. Nothing in the response says which lens answered.
///
/// The schema does mark the token optional, so this mock is stricter than the
/// letter of the WSDL. That is the point: the omission is a client bug that a
/// permissive device hides, and the mock exists to make our own bugs loud.
fn require_config_token(body: &str, missing_reason: &str) -> Result<String, String> {
    extract_tag(body, "ConfigurationToken")
        .filter(|t| !t.is_empty())
        .ok_or_else(|| resp_soap_fault("env:Sender", missing_reason))
}

pub fn resp_video_source_configuration(state: &SharedState, body: &str) -> String {
    let want = match require_config_token(body, "NoConfigToken-VSC-5501") {
        Ok(t) => t,
        Err(fault) => return fault,
    };
    let vscs = state.read().video_source_configs.clone();
    match vscs.iter().find(|c| c.token == want) {
        Some(c) => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            &format!(
                "<trt:GetVideoSourceConfigurationResponse>{}</trt:GetVideoSourceConfigurationResponse>",
                render_vsc_body(c, "trt:Configuration")
            ),
        ),
        None => resp_soap_fault("env:Sender", &format!("NoSuchConfig-VSC-5502: {want}")),
    }
}

/// `GetVideoSourceConfigurationOptions` — per-channel.
///
/// `BoundsRange` maxima are the addressed **sensor's** own resolution, so the
/// two channels report different ceilings; `VideoSourceTokensAvailable` names
/// only the sensor this configuration is attached to.
pub fn resp_video_source_configuration_options(state: &SharedState, body: &str) -> String {
    let want = match require_config_token(body, "NoConfigToken-VSCOPT-5503") {
        Ok(t) => t,
        Err(fault) => return fault,
    };
    let vscs = state.read().video_source_configs.clone();
    let Some(c) = vscs.iter().find(|c| c.token == want) else {
        return resp_soap_fault("env:Sender", &format!("NoSuchConfig-VSCOPT-5504: {want}"));
    };
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            r#"<trt:GetVideoSourceConfigurationOptionsResponse>
          <trt:Options>
            <tt:MaximumNumberOfProfiles>5</tt:MaximumNumberOfProfiles>
            <tt:BoundsRange>
              <tt:XRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:XRange>
              <tt:YRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:YRange>
              <tt:WidthRange><tt:Min>160</tt:Min><tt:Max>{width}</tt:Max></tt:WidthRange>
              <tt:HeightRange><tt:Min>90</tt:Min><tt:Max>{height}</tt:Max></tt:HeightRange>
            </tt:BoundsRange>
            <tt:VideoSourceTokensAvailable>{source}</tt:VideoSourceTokensAvailable>
          </trt:Options>
        </trt:GetVideoSourceConfigurationOptionsResponse>"#,
            width = c.width,
            height = c.height,
            source = c.source_token,
        ),
    )
}

pub fn resp_video_encoder_configuration(state: &SharedState, body: &str) -> String {
    let want = match require_config_token(body, "NoConfigToken-VEC-5505") {
        Ok(t) => t,
        Err(fault) => return fault,
    };
    let vecs = state.read().video_encoders.clone();
    match vecs.iter().find(|c| c.token == want) {
        Some(c) => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            &format!(
                "<trt:GetVideoEncoderConfigurationResponse>{}</trt:GetVideoEncoderConfigurationResponse>",
                render_vec_body(c, "trt:Configuration")
            ),
        ),
        None => resp_soap_fault("env:Sender", &format!("NoSuchConfig-VEC-5506: {want}")),
    }
}

/// `trt:GetVideoEncoderConfigurationOptionsResponse` — **per-channel**.
///
/// This is the operation the multi-sensor rule in CLAUDE.md was written for.
/// The resolution list comes from the addressed configuration's own
/// `resolutions`, so `VEC_1` (sensor 1) reports up to 2592x1944 while `VEC_3`
/// (sensor 2) tops out at 1280x720. Until 0.15 this responder took **no
/// arguments at all**: every channel got sensor 1's list, so a parser that
/// dropped the token on the floor passed every test in the tree.
///
/// Shaped after a real device: the top-level `tt:H264` is `tt:H264Options`,
/// which has **no** `BitrateRange` in the schema, and the whole block is
/// repeated under `tt:Extension` as `tt:H264Options2`, which does. Until 0.15
/// this responder put `BitrateRange` at the top level — not a legal
/// `tt:H264Options` — and so taught the parser a shape no conformant device
/// sends. That is why the parser's failure to descend into `Extension` went
/// unnoticed against both the mock and a hand-written fixture.
///
/// The two copies deliberately carry **different** resolution lists: the
/// `Extension` copy is the superset a newer device sends, so a parser that
/// reads only the shallow copy loses the largest entry and an assertion
/// catches it. Nothing else in the tree pins that direction.
///
/// Deliberately still no `H265`: it would live at
/// `Options/Extension/Extension/H265`, and adding it changes what every caller
/// of this responder sees. The parser's two-level descent is covered by unit
/// fixtures in `src/tests/types_tests.rs`.
pub fn resp_video_encoder_configuration_options(state: &SharedState, body: &str) -> String {
    let want = match require_config_token(body, "NoConfigToken-VECOPT-5507") {
        Ok(t) => t,
        Err(fault) => return fault,
    };
    let vecs = state.read().video_encoders.clone();
    let Some(c) = vecs.iter().find(|c| c.token == want) else {
        return resp_soap_fault("env:Sender", &format!("NoSuchConfig-VECOPT-5508: {want}"));
    };

    let render = |list: &[(u32, u32)]| -> String {
        list.iter()
            .map(|(w, h)| {
                format!(
                    "<tt:ResolutionsAvailable><tt:Width>{w}</tt:Width>\
                     <tt:Height>{h}</tt:Height></tt:ResolutionsAvailable>"
                )
            })
            .collect()
    };
    // The shallow copy is what an older device sends: same channel, minus the
    // widest mode the Extension added.
    let shallow = render(c.resolutions.get(1..).unwrap_or_default());
    let extended = render(&c.resolutions);

    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            r#"<trt:GetVideoEncoderConfigurationOptionsResponse>
          <trt:Options>
            <tt:QualityRange><tt:Min>0</tt:Min><tt:Max>10</tt:Max></tt:QualityRange>
            <tt:H264>
              {shallow}
              <tt:GovLengthRange><tt:Min>1</tt:Min><tt:Max>300</tt:Max></tt:GovLengthRange>
              <tt:FrameRateRange><tt:Min>1</tt:Min><tt:Max>30</tt:Max></tt:FrameRateRange>
              <tt:EncodingIntervalRange><tt:Min>1</tt:Min><tt:Max>30</tt:Max></tt:EncodingIntervalRange>
              <tt:H264ProfilesSupported>Baseline</tt:H264ProfilesSupported>
              <tt:H264ProfilesSupported>Main</tt:H264ProfilesSupported>
              <tt:H264ProfilesSupported>High</tt:H264ProfilesSupported>
            </tt:H264>
            <tt:Extension>
              <tt:H264>
                {extended}
                <tt:GovLengthRange><tt:Min>1</tt:Min><tt:Max>300</tt:Max></tt:GovLengthRange>
                <tt:FrameRateRange><tt:Min>1</tt:Min><tt:Max>30</tt:Max></tt:FrameRateRange>
                <tt:EncodingIntervalRange><tt:Min>1</tt:Min><tt:Max>30</tt:Max></tt:EncodingIntervalRange>
                <tt:H264ProfilesSupported>Baseline</tt:H264ProfilesSupported>
                <tt:H264ProfilesSupported>Main</tt:H264ProfilesSupported>
                <tt:H264ProfilesSupported>High</tt:H264ProfilesSupported>
                <tt:BitrateRange><tt:Min>64</tt:Min><tt:Max>16384</tt:Max></tt:BitrateRange>
              </tt:H264>
            </tt:Extension>
          </trt:Options>
        </trt:GetVideoEncoderConfigurationOptionsResponse>"#
        ),
    )
}

pub fn resp_osd(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "GetOSD").unwrap_or_default();
    let want = extract_tag(&inner, "OSDToken").unwrap_or_default();

    let snapshot = state.read().osd.osds.clone();
    match snapshot.iter().find(|o| o.token == want) {
        Some(entry) => {
            // Singular GetOSDResponse wraps the entry as `<trt:OSD>` (the WSDL
            // element name; OSDConfiguration is the schema *type*). The shared
            // renderer emits the plural `<trt:OSDs>`, so rename it here.
            let body = format!(
                "<trt:GetOSDResponse>{}</trt:GetOSDResponse>",
                render_osd_entry(entry)
            )
            .replace("<trt:OSDs ", "<trt:OSD ")
            .replace("</trt:OSDs>", "</trt:OSD>");
            soap(
                r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
                &body,
            )
        }
        None => resp_soap_fault("ter:InvalidArgs", &format!("OSD not found: {want}")),
    }
}

pub fn handle_create_osd(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "OSD").unwrap_or_default();
    let parsed = match parse_osd_payload(&inner) {
        Ok(p) => p,
        Err(e) => return resp_soap_fault("ter:InvalidArgs", &e),
    };

    // Quota enforcement — match what GetOSDOptions advertises so the
    // mock surfaces "DateAndTime full" the same way Genetec does.
    if let Some(text) = parsed.text.as_ref() {
        let snapshot = state.read().osd.osds.clone();
        let used_total = snapshot.len() as u32;
        let used_for_type = snapshot
            .iter()
            .filter(|o| {
                o.text
                    .as_ref()
                    .is_some_and(|t| t.text_type == text.text_type)
            })
            .count() as u32;
        let limit = match text.text_type.as_str() {
            "Plain" => OSD_QUOTA_PLAIN,
            "Date" => OSD_QUOTA_DATE,
            "Time" => OSD_QUOTA_TIME,
            "DateAndTime" => OSD_QUOTA_DATE_AND_TIME,
            _ => OSD_QUOTA_TOTAL,
        };
        if used_for_type >= limit {
            return resp_soap_fault(
                "ter:InvalidArgs",
                &format!(
                    "Per-type OSD quota exceeded: {}={used_for_type}/{limit}",
                    text.text_type
                ),
            );
        }
        if used_total >= OSD_QUOTA_TOTAL {
            return resp_soap_fault(
                "ter:InvalidArgs",
                &format!("Total OSD quota exceeded: {used_total}/{OSD_QUOTA_TOTAL}"),
            );
        }
    }

    let token = state.modify_returning(|s| {
        let id = s.osd.next_token_id;
        s.osd.next_token_id += 1;
        let token = format!("OSD_{id}");
        let mut entry = parsed;
        entry.token = token.clone();
        eprintln!(
            "    [STATE] OSD created: {token} (vsc={}, type={})",
            entry.video_source_config_token, entry.osd_type
        );
        s.osd.osds.push(entry);
        token
    });

    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            "<trt:CreateOSDResponse><trt:OSDToken>{token}</trt:OSDToken></trt:CreateOSDResponse>"
        ),
    )
}

pub fn handle_set_osd(state: &SharedState, body: &str) -> String {
    // Token sits on the outer `<trt:OSD token="...">` tag, not inside
    // its body — so pull from `body`, not the extracted inner.
    let token = extract_attr(body, "OSD", "token").unwrap_or_default();
    let inner = extract_tag(body, "OSD").unwrap_or_default();
    if token.is_empty() {
        return resp_soap_fault("ter:InvalidArgs", "OSD token missing");
    }
    let parsed = match parse_osd_payload(&inner) {
        Ok(p) => p,
        Err(e) => return resp_soap_fault("ter:InvalidArgs", &e),
    };

    let updated = state.modify_returning(|s| {
        if let Some(existing) = s.osd.osds.iter_mut().find(|o| o.token == token) {
            // Token + vsc are immutable on Set; everything else is replaced.
            let vsc = existing.video_source_config_token.clone();
            *existing = OsdEntry {
                token: token.clone(),
                video_source_config_token: vsc,
                ..parsed
            };
            eprintln!("    [STATE] OSD updated: {token}");
            true
        } else {
            false
        }
    });

    if !updated {
        return resp_soap_fault("ter:InvalidArgs", &format!("OSD not found: {token}"));
    }
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        "<trt:SetOSDResponse/>",
    )
}

pub fn handle_delete_osd(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "DeleteOSD").unwrap_or_default();
    let token = extract_tag(&inner, "OSDToken").unwrap_or_default();

    let removed = state.modify_returning(|s| {
        let before = s.osd.osds.len();
        s.osd.osds.retain(|o| o.token != token);
        let removed = before > s.osd.osds.len();
        if removed {
            eprintln!("    [STATE] OSD deleted: {token}");
        }
        removed
    });

    if !removed {
        return resp_soap_fault("ter:InvalidArgs", &format!("OSD not found: {token}"));
    }
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        "<trt:DeleteOSDResponse/>",
    )
}

/// `GetOSDOptions` advertises per-text-type quotas via XML attributes
/// on `<MaximumNumberOfOSDs>`. This is the Genetec/late-Hikvision shape;
/// `oxvif::OnvifSession::get_osd_options` parses the attributes (the
/// strict `OnvifClient` ignores them, by design).
pub fn resp_osd_options() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            r#"<trt:GetOSDOptionsResponse>
          <trt:OSDOptions>
            <tt:MaximumNumberOfOSDs Total="{OSD_QUOTA_TOTAL}" Plain="{OSD_QUOTA_PLAIN}" Date="{OSD_QUOTA_DATE}" Time="{OSD_QUOTA_TIME}" DateAndTime="{OSD_QUOTA_DATE_AND_TIME}"/>
            <tt:Type>Text</tt:Type>
            <tt:Type>Image</tt:Type>
            <tt:PositionOption>
              <tt:Type>UpperLeft</tt:Type>
              <tt:Type>UpperRight</tt:Type>
              <tt:Type>LowerLeft</tt:Type>
              <tt:Type>LowerRight</tt:Type>
              <tt:Type>Custom</tt:Type>
            </tt:PositionOption>
            <tt:TextOption>
              <tt:Type>Plain</tt:Type>
              <tt:Type>Date</tt:Type>
              <tt:Type>Time</tt:Type>
              <tt:Type>DateAndTime</tt:Type>
              <tt:DateFormat>MM/dd/yyyy</tt:DateFormat>
              <tt:DateFormat>yyyy-MM-dd</tt:DateFormat>
              <tt:DateFormat>dd.MM.yyyy</tt:DateFormat>
              <tt:TimeFormat>HH:mm:ss</tt:TimeFormat>
              <tt:TimeFormat>hh:mm:ss tt</tt:TimeFormat>
              <tt:FontSizeRange>
                <tt:Min>8</tt:Min>
                <tt:Max>72</tt:Max>
              </tt:FontSizeRange>
            </tt:TextOption>
          </trt:OSDOptions>
        </trt:GetOSDOptionsResponse>"#
        ),
    )
}

// ── OSD render / parse helpers ──────────────────────────────────────────────

fn render_osd_entry(o: &OsdEntry) -> String {
    let pos_xy = match (o.position_x, o.position_y) {
        (Some(x), Some(y)) => format!(r#"<tt:Pos x="{x}" y="{y}"/>"#),
        _ => String::new(),
    };
    let text_el = o.text.as_ref().map(render_osd_text).unwrap_or_default();
    let image_el = o
        .image_path
        .as_deref()
        .map(|p| format!("<tt:ImgPath>{p}</tt:ImgPath>"))
        .unwrap_or_default();
    format!(
        r#"<trt:OSDs token="{token}">
          <tt:VideoSourceConfigurationToken>{vsc}</tt:VideoSourceConfigurationToken>
          <tt:Type>{ty}</tt:Type>
          <tt:Position>
            <tt:Type>{pos_type}</tt:Type>
            {pos_xy}
          </tt:Position>
          {text_el}{image_el}
        </trt:OSDs>"#,
        token = o.token,
        vsc = o.video_source_config_token,
        ty = o.osd_type,
        pos_type = o.position_type,
    )
}

fn render_osd_text(t: &OsdTextEntry) -> String {
    let plain = t
        .plain_text
        .as_deref()
        .map(|s| format!("<tt:PlainText>{s}</tt:PlainText>"))
        .unwrap_or_default();
    let date = t
        .date_format
        .as_deref()
        .map(|s| format!("<tt:DateFormat>{s}</tt:DateFormat>"))
        .unwrap_or_default();
    let time = t
        .time_format
        .as_deref()
        .map(|s| format!("<tt:TimeFormat>{s}</tt:TimeFormat>"))
        .unwrap_or_default();
    let font = t
        .font_size
        .map(|n| format!("<tt:FontSize>{n}</tt:FontSize>"))
        .unwrap_or_default();
    let color = t
        .font_color
        .as_ref()
        .map(|c| {
            let cs = c
                .colorspace
                .as_deref()
                .map(|s| format!(r#" Colorspace="{s}""#))
                .unwrap_or_default();
            let trans = c
                .transparent
                .map(|v| format!("<tt:Transparent>{v}</tt:Transparent>"))
                .unwrap_or_default();
            format!(
                r#"<tt:FontColor><tt:Color X="{x}" Y="{y}" Z="{z}"{cs}/>{trans}</tt:FontColor>"#,
                x = c.x,
                y = c.y,
                z = c.z,
            )
        })
        .unwrap_or_default();
    format!(
        r#"<tt:TextString><tt:Type>{ty}</tt:Type>{plain}{date}{time}{font}{color}</tt:TextString>"#,
        ty = t.text_type,
    )
}

/// Parse an `<trt:OSD>` payload into an `OsdEntry`. The token is left
/// blank — `handle_create_osd` fills it in from `next_token_id`,
/// `handle_set_osd` keeps the existing token.
fn parse_osd_payload(inner: &str) -> Result<OsdEntry, String> {
    let vsc = extract_tag(inner, "VideoSourceConfigurationToken")
        .ok_or_else(|| "VideoSourceConfigurationToken missing".to_string())?;
    let ty = extract_tag(inner, "Type").unwrap_or_else(|| "Text".to_string());

    let pos = extract_tag(inner, "Position").unwrap_or_default();
    let position_type = extract_tag(&pos, "Type").unwrap_or_else(|| "UpperLeft".to_string());
    let position_x = extract_attr(&pos, "Pos", "x").and_then(|s| s.parse().ok());
    let position_y = extract_attr(&pos, "Pos", "y").and_then(|s| s.parse().ok());

    let text = if ty == "Text" {
        let ts = extract_tag(inner, "TextString").unwrap_or_default();
        let text_type = extract_tag(&ts, "Type").unwrap_or_else(|| "Plain".to_string());
        Some(OsdTextEntry {
            text_type,
            plain_text: extract_tag(&ts, "PlainText"),
            date_format: extract_tag(&ts, "DateFormat"),
            time_format: extract_tag(&ts, "TimeFormat"),
            font_size: extract_tag(&ts, "FontSize").and_then(|s| s.parse().ok()),
            font_color: parse_osd_color(&ts),
        })
    } else {
        None
    };

    let image_path = if ty == "Image" {
        extract_tag(inner, "ImgPath")
    } else {
        None
    };

    Ok(OsdEntry {
        token: String::new(),
        video_source_config_token: vsc,
        osd_type: ty,
        position_type,
        position_x,
        position_y,
        text,
        image_path,
    })
}

fn parse_osd_color(text_string: &str) -> Option<OsdColorEntry> {
    let fc = extract_tag(text_string, "FontColor")?;
    let x: f32 = extract_attr(&fc, "Color", "X")?.parse().ok()?;
    let y: f32 = extract_attr(&fc, "Color", "Y")?.parse().ok()?;
    let z: f32 = extract_attr(&fc, "Color", "Z")?.parse().ok()?;
    let colorspace = extract_attr(&fc, "Color", "Colorspace");
    let transparent = extract_tag(&fc, "Transparent").and_then(|s| s.parse().ok());
    Some(OsdColorEntry {
        x,
        y,
        z,
        colorspace,
        transparent,
    })
}

// `extract_all_tags` is currently unused but reserved for future Image-OSD
// support that may need to read multiple `<ImgPath>` siblings. Suppress the
// warning rather than removing the import — keeping it discoverable in tree.
#[allow(dead_code)]
fn _force_use_extract_all() {
    let _ = extract_all_tags("", "");
}

pub fn resp_audio_source_configurations(state: &SharedState) -> String {
    let items: String = state
        .read()
        .audio_source_configs
        .iter()
        .map(|c| render_audio_source_config(c, "trt:Configurations"))
        .collect();
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            "<trt:GetAudioSourceConfigurationsResponse>{items}\
             </trt:GetAudioSourceConfigurationsResponse>"
        ),
    )
}

pub fn resp_audio_encoder_configuration(state: &SharedState, body: &str) -> String {
    let Some(token) = extract_tag(body, "ConfigurationToken").filter(|t| !t.is_empty()) else {
        return resp_soap_fault(
            "env:Sender",
            "NoConfigToken-GETAEC-5711: GetAudioEncoderConfiguration names one configuration",
        );
    };
    let Some(cfg) = state
        .read()
        .audio_encoders
        .iter()
        .find(|c| c.token == token)
        .cloned()
    else {
        return resp_soap_fault(
            "ter:NoConfig",
            &format!("NoSuchAudioEncoder-GETAEC-5712: {token}"),
        );
    };
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            "<trt:GetAudioEncoderConfigurationResponse>{}\
             </trt:GetAudioEncoderConfigurationResponse>",
            render_audio_encoder(&cfg, "trt:Configuration")
        ),
    )
}

/// `GetAudioEncoderConfigurationOptions` — **per configuration**, and in
/// Media1's nesting.
///
/// ```text
/// Response/Options            tt:AudioEncoderConfigurationOptions   ← a wrapper
///                 /Options    tt:AudioEncoderConfigurationOption    ← repeated, the entry
/// ```
///
/// This response was flat — `trt:Options` repeated with `Encoding` as a direct
/// child — which is *Media2's* shape. `AudioEncoderConfigurationOptions::from_xml`
/// read that same wrong shape, so the mock and the parser agreed with each other
/// and with no Media1 device on earth. Both were fixed in 0.15; this is the half
/// that keeps the parser's Media1 branch exercised.
///
/// It was also one static pair for the whole device, so a caller that passed the
/// wrong `ConfigurationToken` got a plausible answer and no way to notice.
pub fn resp_audio_encoder_configuration_options(state: &SharedState, body: &str) -> String {
    let Some(token) = extract_tag(body, "ConfigurationToken").filter(|t| !t.is_empty()) else {
        return resp_soap_fault(
            "env:Sender",
            "NoConfigToken-AECOPTS-5713: GetAudioEncoderConfigurationOptions is per configuration",
        );
    };
    let Some(cfg) = state
        .read()
        .audio_encoders
        .iter()
        .find(|c| c.token == token)
        .cloned()
    else {
        return resp_soap_fault(
            "ter:NoConfig",
            &format!("NoSuchAudioEncoder-AECOPTS-5714: {token}"),
        );
    };
    let rows: String = cfg
        .options
        .iter()
        .map(|o| render_audio_option(o, "tt:Options"))
        .collect();
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            "<trt:GetAudioEncoderConfigurationOptionsResponse>\
               <trt:Options>{rows}</trt:Options>\
             </trt:GetAudioEncoderConfigurationOptionsResponse>"
        ),
    )
}

/// One options row. Media1 wraps these in a `trt:Options` container and names
/// them `tt:Options`; Media2 emits them directly as repeated `tr2:Options`.
pub(crate) fn render_audio_option(o: &AudioOptionEntry, qname: &str) -> String {
    let list = |v: &[u32]| v.iter().map(u32::to_string).collect::<Vec<_>>().join(" ");
    format!(
        "<{qname}>\
           <tt:Encoding>{enc}</tt:Encoding>\
           <tt:BitrateList><tt:Items>{br}</tt:Items></tt:BitrateList>\
           <tt:SampleRateList><tt:Items>{sr}</tt:Items></tt:SampleRateList>\
         </{qname}>",
        enc = o.encoding,
        br = list(&o.bitrates),
        sr = list(&o.sample_rates),
    )
}

/// Media1 `SetAudioEncoderConfiguration`.
///
/// **Refuses a body without `Multicast` or `SessionTimeout`.** Both are
/// *required* members of `tt:AudioEncoderConfiguration`, so a device validating
/// the request rejects one that omits them — and oxvif omitted both until 0.15.
/// Storing what arrives regardless would make the mock the one device on which
/// the old, invalid body worked, which is the opposite of what it is for.
///
/// Media2's `SetAudioEncoderConfiguration` is deliberately not this strict:
/// `tt:AudioEncoder2Configuration` makes `Multicast` optional and has no
/// `SessionTimeout` member at all.
pub fn handle_set_audio_encoder_configuration(state: &SharedState, body: &str) -> String {
    match apply_audio_encoder_write(state, body, true) {
        Ok(()) => resp_empty("trt", "SetAudioEncoderConfigurationResponse"),
        Err(fault) => fault,
    }
}

/// The shared audio-encoder write. `media1` selects the required-member check
/// and whether `SessionTimeout` may be written.
pub(crate) fn apply_audio_encoder_write(
    state: &SharedState,
    body: &str,
    media1: bool,
) -> Result<(), String> {
    let tag = if media1 {
        "SETAEC-5715"
    } else {
        "SETAEC2-5716"
    };
    let Some(token) = extract_attr(body, "Configuration", "token").filter(|t| !t.is_empty()) else {
        return Err(resp_soap_fault(
            "env:Sender",
            &format!("NoConfigToken-{tag}: the configuration carries the token it replaces"),
        ));
    };
    if !state.read().audio_encoders.iter().any(|c| c.token == token) {
        return Err(resp_soap_fault(
            "ter:NoConfig",
            &format!("NoSuchAudioEncoder-{tag}: {token}"),
        ));
    }
    let cfg = extract_tag(body, "Configuration").unwrap_or_default();
    let multicast = extract_tag(&cfg, "Multicast").map(|m| MulticastEntry {
        address: extract_tag(&m, "IPv4Address").unwrap_or_default(),
        port: extract_tag(&m, "Port")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        ttl: extract_tag(&m, "TTL")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        auto_start: extract_tag(&m, "AutoStart").as_deref() == Some("true"),
    });
    let session_timeout = extract_tag(&cfg, "SessionTimeout");

    if media1 && (multicast.is_none() || session_timeout.is_none()) {
        return Err(resp_soap_fault(
            "ter:ConfigModify",
            &format!(
                "IncompleteAudioEncoder-{tag}: tt:AudioEncoderConfiguration requires both \
                 Multicast and SessionTimeout; this request carried Multicast={} \
                 SessionTimeout={}",
                multicast.is_some(),
                session_timeout.is_some(),
            ),
        ));
    }

    let name = extract_tag(&cfg, "Name").unwrap_or_default();
    let encoding = extract_tag(&cfg, "Encoding").unwrap_or_default();
    let bitrate = extract_tag(&cfg, "Bitrate")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let sample_rate = extract_tag(&cfg, "SampleRate")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    state.modify(|s| {
        if let Some(c) = s.audio_encoders.iter_mut().find(|c| c.token == token) {
            c.name = name.clone();
            c.encoding = encoding.clone();
            c.bitrate = bitrate;
            c.sample_rate = sample_rate;
            c.multicast = multicast.clone();
            // Media2 cannot express `SessionTimeout` — its type has no such
            // member — so a Media2 write leaves the stored one alone rather
            // than destroying a value Media1 requires. `UseCount` and
            // `options` are the device's, not the caller's.
            if media1 {
                c.session_timeout = session_timeout.clone();
            }
            eprintln!("    [STATE] audio encoder updated: {token}");
        }
    });
    Ok(())
}

// ── GetServiceCapabilities ───────────────────────────────────────────────────

/// `trt:Capabilities`.
///
/// Media1's is one of only two service-capability types with required child
/// elements (`ProfileCapabilities`, `StreamingCapabilities`) rather than
/// attributes alone. `VideoSourceMode` is `false` here and `true` in Media2's
/// — the mock dispatches `GetVideoSourceModes` on ver20 only, and the two
/// answers should not agree just because the field name matches.
///
/// `trt:StreamingCapabilities` is **not** the device-level
/// `tt:StreamingCapabilities`: it adds `NonAggregateControl` and
/// `NoRTSPStreaming`.
pub fn resp_service_capabilities() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetServiceCapabilitiesResponse>
          <trt:Capabilities SnapshotUri="true"
                            Rotation="false"
                            VideoSourceMode="false"
                            OSD="true"
                            TemporaryOSDText="false"
                            EXICompression="false">
            <trt:ProfileCapabilities MaximumNumberOfProfiles="8"/>
            <trt:StreamingCapabilities RTPMulticast="false"
                                       RTP_TCP="true"
                                       RTP_RTSP_TCP="true"
                                       NonAggregateControl="false"
                                       NoRTSPStreaming="false"/>
          </trt:Capabilities>
        </trt:GetServiceCapabilitiesResponse>"#,
    )
}
