use crate::mock::helpers::{resp_empty, resp_soap_fault, soap};
use crate::mock::state::{
    OSD_QUOTA_DATE, OSD_QUOTA_DATE_AND_TIME, OSD_QUOTA_PLAIN, OSD_QUOTA_TIME, OSD_QUOTA_TOTAL,
    OsdColorEntry, OsdEntry, OsdTextEntry, ProfileEntry, SharedState, VideoEncoderState,
    VideoSourceConfigEntry,
};
use crate::mock::xml_parse::{extract_all_tags, extract_attr, extract_tag};

pub fn resp_profiles(state: &SharedState) -> String {
    let snapshot = state.read().profiles.profiles.clone();
    let (vscs, vecs) = catalogues(state);
    let items: String = snapshot
        .iter()
        .map(|p| render_profile(p, "Profiles", &vscs, &vecs))
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
    let (vscs, vecs) = catalogues(state);
    match snapshot.iter().find(|p| p.token == want) {
        Some(p) => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            &format!(
                "<trt:GetProfileResponse>{}</trt:GetProfileResponse>",
                render_profile(p, "Profile", &vscs, &vecs)
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
            render_profile(&entry, "Profile", &[], &[])
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
// shape real cameras use). The configuration details for VSC_1, VEC_1,
// VEC_2 are hardcoded here rather than stored in state — only the
// *attachment* (which token a profile is bound to) is mutable, which
// matches what `CreateProfile` / `AddVideoEncoderConfiguration` etc.
// actually mutate on a real camera.

fn render_profile(
    p: &ProfileEntry,
    tag: &str,
    vscs: &[VideoSourceConfigEntry],
    vecs: &[VideoEncoderState],
) -> String {
    let vsc = p
        .video_source_config_token
        .as_deref()
        .map(|t| render_vsc_inline(vscs, t))
        .unwrap_or_default();
    let vec = p
        .video_encoder_config_token
        .as_deref()
        .map(|t| render_vec_inline(vecs, t))
        .unwrap_or_default();
    let asc = p
        .audio_source_config_token
        .as_deref()
        .map(render_asc_inline)
        .unwrap_or_default();
    let aec = p
        .audio_encoder_config_token
        .as_deref()
        .map(render_aec_inline)
        .unwrap_or_default();
    format!(
        r#"<trt:{tag} token="{token}" fixed="{fixed}">
          <tt:Name>{name}</tt:Name>
          {vsc}{vec}{asc}{aec}
        </trt:{tag}>"#,
        token = p.token,
        fixed = p.fixed,
        name = p.name,
    )
}

/// Clone both per-channel catalogues out under a single read lock.
///
/// Every caller below needs the pair, and taking one lock rather than two
/// keeps a responder from rendering a profile whose source config and encoder
/// config came from different moments.
fn catalogues(state: &SharedState) -> (Vec<VideoSourceConfigEntry>, Vec<VideoEncoderState>) {
    let s = state.read();
    (s.video_source_configs.clone(), s.video_encoders.clone())
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

fn render_asc_inline(token: &str) -> String {
    match token {
        "ASC_1" => r#"<tt:AudioSourceConfiguration token="ASC_1">
          <tt:Name>AudioSourceConfig1</tt:Name>
          <tt:UseCount>1</tt:UseCount>
          <tt:SourceToken>AudioSource_1</tt:SourceToken>
        </tt:AudioSourceConfiguration>"#
            .to_string(),
        _ => String::new(),
    }
}

fn render_aec_inline(token: &str) -> String {
    match token {
        "AEC_1" => r#"<tt:AudioEncoderConfiguration token="AEC_1">
          <tt:Name>AudioEncoder</tt:Name>
          <tt:UseCount>1</tt:UseCount>
          <tt:Encoding>G711</tt:Encoding>
          <tt:Bitrate>64</tt:Bitrate>
          <tt:SampleRate>8</tt:SampleRate>
        </tt:AudioEncoderConfiguration>"#
            .to_string(),
        _ => String::new(),
    }
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

pub fn resp_audio_sources() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioSourcesResponse>
          <trt:AudioSources token="AudioSource_1">
            <tt:Channels>1</tt:Channels>
          </trt:AudioSources>
        </trt:GetAudioSourcesResponse>"#,
    )
}

pub fn resp_audio_encoder_configurations() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioEncoderConfigurationsResponse>
          <trt:Configurations token="AEC_1">
            <tt:Name>AudioEncoder</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>G711</tt:Encoding>
            <tt:Bitrate>64</tt:Bitrate>
            <tt:SampleRate>8</tt:SampleRate>
          </trt:Configurations>
        </trt:GetAudioEncoderConfigurationsResponse>"#,
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

pub fn resp_audio_source_configurations() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioSourceConfigurationsResponse>
          <trt:Configurations token="ASC_1">
            <tt:Name>AudioSourceConfig1</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:SourceToken>AudioSource_1</tt:SourceToken>
          </trt:Configurations>
        </trt:GetAudioSourceConfigurationsResponse>"#,
    )
}

pub fn resp_audio_encoder_configuration() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioEncoderConfigurationResponse>
          <trt:Configuration token="AEC_1">
            <tt:Name>AudioEncoder</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>G711</tt:Encoding>
            <tt:Bitrate>64</tt:Bitrate>
            <tt:SampleRate>8</tt:SampleRate>
          </trt:Configuration>
        </trt:GetAudioEncoderConfigurationResponse>"#,
    )
}

pub fn resp_audio_encoder_configuration_options() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioEncoderConfigurationOptionsResponse>
          <trt:Options>
            <tt:Encoding>G711</tt:Encoding>
            <tt:BitrateList><tt:Items>64</tt:Items></tt:BitrateList>
            <tt:SampleRateList><tt:Items>8</tt:Items></tt:SampleRateList>
          </trt:Options>
          <trt:Options>
            <tt:Encoding>AAC</tt:Encoding>
            <tt:BitrateList><tt:Items>64 128 256</tt:Items></tt:BitrateList>
            <tt:SampleRateList><tt:Items>16 32 44</tt:Items></tt:SampleRateList>
          </trt:Options>
        </trt:GetAudioEncoderConfigurationOptionsResponse>"#,
    )
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
