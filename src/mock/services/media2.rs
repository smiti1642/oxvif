use crate::mock::helpers::{resp_empty, resp_soap_fault, soap};
use crate::mock::services::media;
use crate::mock::state::{ProfileEntry, SharedState, VideoEncoderState};
use crate::mock::xml_parse::{extract_all_tags, extract_tag};

const NS: &str = r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#;

/// The `ConfigurationToken` of a per-channel Media2 request, or a SOAP Fault.
/// Same reasoning as `media::require_config_token` — see that doc comment.
fn require_config_token(body: &str, missing_reason: &str) -> Result<String, String> {
    extract_tag(body, "ConfigurationToken")
        .filter(|t| !t.is_empty())
        .ok_or_else(|| resp_soap_fault("env:Sender", missing_reason))
}

/// One profile in the Media2 shape.
///
/// **Not a prefix swap on `media::render_profile`.** Media1 inlines the whole
/// configuration (`<tt:VideoSourceConfiguration token="VSC_1"><tt:Name>…
/// <tt:UseCount>…`); Media2 emits token *references* only, inside a single
/// `<tr2:Configurations>` wrapper. Two genuinely different shapes over the same
/// [`ProfileEntry`] — which is why the state is shared and the renderers are not.
///
/// Element names match what `MediaProfile2::vec_from_xml` reads: `VideoSource`,
/// `VideoEncoder`, `AudioSource`, and `Audio` — note the audio encoder is
/// `<tr2:Audio>`, not `<tr2:AudioEncoder>`.
fn render_profile_media2(p: &ProfileEntry, tag: &str) -> String {
    let mut cfgs = String::new();
    if let Some(t) = &p.video_source_config_token {
        cfgs.push_str(&format!("<tr2:VideoSource token=\"{t}\"/>"));
    }
    if let Some(t) = &p.video_encoder_config_token {
        cfgs.push_str(&format!("<tr2:VideoEncoder token=\"{t}\"/>"));
    }
    if let Some(t) = &p.audio_source_config_token {
        cfgs.push_str(&format!("<tr2:AudioSource token=\"{t}\"/>"));
    }
    if let Some(t) = &p.audio_encoder_config_token {
        cfgs.push_str(&format!("<tr2:Audio token=\"{t}\"/>"));
    }
    // `MediaProfile2::ptz_config_token` reads `Configurations/PTZ@token` and
    // nothing ever fed it: neither profile renderer emitted a PTZ element, and
    // `ProfileEntry` had no slot to emit from.
    if let Some(t) = &p.ptz_config_token {
        cfgs.push_str(&format!("<tr2:PTZ token=\"{t}\"/>"));
    }
    // A profile with nothing bound omits the wrapper rather than sending an
    // empty one — a freshly created profile is exactly that case.
    let configurations = if cfgs.is_empty() {
        String::new()
    } else {
        format!("<tr2:Configurations>{cfgs}</tr2:Configurations>")
    };
    format!(
        r#"<tr2:{tag} token="{token}" fixed="{fixed}">
          <tt:Name>{name}</tt:Name>
          {configurations}
        </tr2:{tag}>"#,
        token = p.token,
        fixed = p.fixed,
        name = p.name,
    )
}

/// The device's profiles, in the Media2 shape.
///
/// This used to be a string literal with no `state` parameter, so a caller who
/// seeded `DeviceState.profiles` got their list from Media1 and four hardcoded
/// `Profile_A`…`Profile_D` from Media2 — same device, same process, same state,
/// no error, no overlap in the token sets. Reported by a C++ ONVIF test suite
/// whose harness seeded 20 profiles and whose DLL, negotiating Media2, received
/// 4.
pub fn resp_profiles_media2(state: &SharedState) -> String {
    let snapshot = state.read().profiles.profiles.clone();
    let items: String = snapshot
        .iter()
        .map(|p| render_profile_media2(p, "Profiles"))
        .collect();
    soap(
        NS,
        &format!("<tr2:GetProfilesResponse>{items}</tr2:GetProfilesResponse>"),
    )
}

pub fn resp_stream_uri_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetStreamUriResponse>
          <tr2:Uri>rtsp://127.0.0.1:554/mock/h265</tr2:Uri>
        </tr2:GetStreamUriResponse>"#,
    )
}

pub fn resp_snapshot_uri_media2(base: &str) -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        &format!(
            r#"<tr2:GetSnapshotUriResponse>
          <tr2:Uri>{base}/mock/snapshot.jpg</tr2:Uri>
        </tr2:GetSnapshotUriResponse>"#
        ),
    )
}

pub fn resp_video_source_configurations_media2(state: &SharedState) -> String {
    let vscs = state.read().video_source_configs.clone();
    let items: String = vscs
        .iter()
        .map(|c| {
            format!(
                r#"<tr2:Configurations token="{token}">
            <tt:Name>{name}</tt:Name>
            <tt:UseCount>{use_count}</tt:UseCount>
            <tt:SourceToken>{source}</tt:SourceToken>
            <tt:Bounds x="0" y="0" width="{width}" height="{height}"/>
          </tr2:Configurations>"#,
                token = c.token,
                name = c.name,
                use_count = c.use_count,
                source = c.source_token,
                width = c.width,
                height = c.height,
            )
        })
        .collect();
    soap(
        NS,
        &format!(
            "<tr2:GetVideoSourceConfigurationsResponse>{items}</tr2:GetVideoSourceConfigurationsResponse>"
        ),
    )
}

/// Per-channel — the bounds ceiling is the addressed sensor's own resolution.
pub fn resp_video_source_configuration_options_media2(state: &SharedState, body: &str) -> String {
    let want = match require_config_token(body, "NoConfigToken-VSCOPT2-5511") {
        Ok(t) => t,
        Err(fault) => return fault,
    };
    let vscs = state.read().video_source_configs.clone();
    let Some(c) = vscs.iter().find(|c| c.token == want) else {
        return resp_soap_fault("env:Sender", &format!("NoSuchConfig-VSCOPT2-5512: {want}"));
    };
    soap(
        NS,
        &format!(
            r#"<tr2:GetVideoSourceConfigurationOptionsResponse>
          <tr2:Options>
            <tt:MaximumNumberOfProfiles>5</tt:MaximumNumberOfProfiles>
            <tt:BoundsRange>
              <tt:XRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:XRange>
              <tt:YRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:YRange>
              <tt:WidthRange><tt:Min>160</tt:Min><tt:Max>{width}</tt:Max></tt:WidthRange>
              <tt:HeightRange><tt:Min>90</tt:Min><tt:Max>{height}</tt:Max></tt:HeightRange>
            </tt:BoundsRange>
            <tt:VideoSourceTokensAvailable>{source}</tt:VideoSourceTokensAvailable>
          </tr2:Options>
        </tr2:GetVideoSourceConfigurationOptionsResponse>"#,
            width = c.width,
            height = c.height,
            source = c.source_token,
        ),
    )
}

/// Per-channel. Media2 returns one `Options` block **per encoding**, so the
/// H.265 block is offered only where the sensor can actually do it — sensor 1.
/// Sensor 2 gets H.264 alone, at its own smaller resolution list.
pub fn resp_video_encoder_configuration_options_media2(state: &SharedState, body: &str) -> String {
    let want = match require_config_token(body, "NoConfigToken-VECOPT2-5513") {
        Ok(t) => t,
        Err(fault) => return fault,
    };
    let vecs = state.read().video_encoders.clone();
    let Some(c) = vecs.iter().find(|c| c.token == want) else {
        return resp_soap_fault("env:Sender", &format!("NoSuchConfig-VECOPT2-5514: {want}"));
    };

    let resolutions: String = c
        .resolutions
        .iter()
        .map(|(w, h)| {
            format!(
                "<tt:ResolutionsAvailable><tt:Width>{w}</tt:Width>\
                 <tt:Height>{h}</tt:Height></tt:ResolutionsAvailable>"
            )
        })
        .collect();

    // Only the 5MP sensor advertises H.265. Nothing else in the mock lets a
    // test tell "this device supports H265" from "this *channel* supports it".
    let h265 = if c.source_token == "VS_1" {
        format!(
            r#"<tr2:Options>
            <tt:Encoding>H265</tt:Encoding>
            <tt:QualityRange><tt:Min>0</tt:Min><tt:Max>10</tt:Max></tt:QualityRange>
            {resolutions}
            <tt:BitrateRange><tt:Min>64</tt:Min><tt:Max>32768</tt:Max></tt:BitrateRange>
            <tt:FrameRateRange><tt:Min>1</tt:Min><tt:Max>60</tt:Max></tt:FrameRateRange>
            <tt:GovLengthRange><tt:Min>1</tt:Min><tt:Max>600</tt:Max></tt:GovLengthRange>
            <tt:ProfilesSupported>Main</tt:ProfilesSupported>
          </tr2:Options>"#
        )
    } else {
        String::new()
    };

    soap(
        NS,
        &format!(
            r#"<tr2:GetVideoEncoderConfigurationOptionsResponse>
          <tr2:Options>
            <tt:Encoding>H264</tt:Encoding>
            <tt:QualityRange><tt:Min>0</tt:Min><tt:Max>10</tt:Max></tt:QualityRange>
            {resolutions}
            <tt:BitrateRange><tt:Min>64</tt:Min><tt:Max>16384</tt:Max></tt:BitrateRange>
            <tt:FrameRateRange><tt:Min>1</tt:Min><tt:Max>30</tt:Max></tt:FrameRateRange>
            <tt:GovLengthRange><tt:Min>1</tt:Min><tt:Max>300</tt:Max></tt:GovLengthRange>
            <tt:ProfilesSupported>Baseline</tt:ProfilesSupported>
            <tt:ProfilesSupported>Main</tt:ProfilesSupported>
            <tt:ProfilesSupported>High</tt:ProfilesSupported>
          </tr2:Options>
          {h265}
        </tr2:GetVideoEncoderConfigurationOptionsResponse>"#
        ),
    )
}

/// `GetVideoEncoderConfigurations` (Media2) — renders the encoder config from
/// state. If the request carries a `ConfigurationToken`, only the matching
/// config is returned (empty list otherwise), mirroring ONVIF token filtering.
/// Pairs with [`handle_set_video_encoder_configuration`] for Set → Get roundtrips.
pub fn resp_video_encoder_configurations(state: &SharedState, body: &str) -> String {
    let vecs = state.read().video_encoders.clone();
    let want = extract_tag(body, "ConfigurationToken").filter(|t| !t.is_empty());
    let items: String = vecs
        .iter()
        .filter(|c| want.as_deref().is_none_or(|t| t == c.token))
        .map(render_video_encoder)
        .collect();
    soap(
        NS,
        &format!(
            "<tr2:GetVideoEncoderConfigurationsResponse>{items}</tr2:GetVideoEncoderConfigurationsResponse>"
        ),
    )
}

/// `SetVideoEncoderConfiguration` (Media2) — persists the posted fields into
/// state so a following `GetVideoEncoderConfigurations` reflects them. Only the
/// fields present in the request body are updated.
pub fn handle_set_video_source_configuration_media2(state: &SharedState, body: &str) -> String {
    match media::apply_video_source_write(
        state,
        body,
        "NoConfigToken-SETVSC2-5523",
        "NoSuchConfig-SETVSC2-5524",
    ) {
        Ok(()) => resp_empty("tr2", "SetVideoSourceConfigurationResponse"),
        Err(fault) => fault,
    }
}

/// `tr2:AddConfiguration` — one generic operation carrying `<tr2:Type>`, where
/// Media1 has four named ones. It writes the *same* profile slots; only the way
/// the caller names the slot differs.
///
/// A `Type` the mock does not model (`Metadata`, `Analytics`, `AudioOutput`,
/// `AudioDecoder`) **faults** rather than reporting success. `ProfileEntry` has
/// no slot for those and `MediaProfile2` exposes none, so there is no state to
/// write and no getter that could ever show the result — answering
/// `AddConfigurationResponse` to it would be the audit's LIE cell (§1),
/// reintroduced by the very commit that removes it. The fault names the type, so
/// a caller learns *why* instead of being told nothing happened.
///
/// **`PTZ` was on that list until the PTZ family was wired.** The moment
/// `ProfileEntry` grew `ptz_config_token` and both profile renderers started
/// emitting it, the justification above stopped being true of `PTZ` — there was
/// now a slot to write and two getters that show it. A fault whose stated reason
/// has quietly become false is worse than no fault, so `ConfigKind::Ptz` binds
/// like the other four.
pub fn handle_add_configuration_media2(state: &SharedState, body: &str) -> String {
    match apply_media2_configuration(state, body, true) {
        Ok(()) => resp_empty("tr2", "AddConfigurationResponse"),
        Err(fault) => fault,
    }
}

pub fn handle_remove_configuration_media2(state: &SharedState, body: &str) -> String {
    match apply_media2_configuration(state, body, false) {
        Ok(()) => resp_empty("tr2", "RemoveConfigurationResponse"),
        Err(fault) => fault,
    }
}

/// Both directions of the Media2 configuration binding.
///
/// The request may carry several `<tr2:Configuration>` children; every one is
/// applied, and the first unmodelled `Type` aborts before anything is written so
/// a partial application cannot be mistaken for a whole one.
fn apply_media2_configuration(state: &SharedState, body: &str, add: bool) -> Result<(), String> {
    let profile = extract_tag(body, "ProfileToken").unwrap_or_default();
    let entries = extract_all_tags(body, "Configuration");
    if entries.is_empty() {
        return Err(resp_soap_fault(
            "env:Sender",
            "NoConfiguration-CFG2-5541: at least one tr2:Configuration is required",
        ));
    }

    // Resolve every kind first — see the doc comment above.
    let mut planned = Vec::new();
    for entry in &entries {
        let type_ = extract_tag(entry, "Type").unwrap_or_default();
        let Some(kind) = media::ConfigKind::from_media2_type(&type_) else {
            return Err(resp_soap_fault(
                "ter:ConfigurationConflict",
                &format!(
                    "UnmodelledConfigType-CFG2-5542: the mock's ProfileEntry has no slot for \
                     {type_}, and MediaProfile2 exposes none, so a success here could never be \
                     observed"
                ),
            ));
        };
        planned.push((kind, extract_tag(entry, "Token").unwrap_or_default()));
    }

    for (kind, token) in planned {
        // The state operations read `ProfileToken` out of the body themselves,
        // so hand each one a minimal body naming this single binding.
        let one = format!(
            "<ProfileToken>{profile}</ProfileToken><ConfigurationToken>{token}</ConfigurationToken>"
        );
        if add {
            media::bind_configuration(state, &one, kind, "ADDCFG2-5543")?;
        } else {
            media::unbind_configuration(state, &one, kind, "RMCFG2-5544")?;
        }
    }
    Ok(())
}

pub fn handle_set_video_encoder_configuration(state: &SharedState, body: &str) -> String {
    match media::apply_video_encoder_write(
        state,
        body,
        "NoConfigToken-SETVEC2-5515",
        "NoSuchConfig-SETVEC2-5516",
    ) {
        Ok(()) => resp_empty("tr2", "SetVideoEncoderConfigurationResponse"),
        Err(fault) => fault,
    }
}

/// Render one `<tr2:Configurations>` element from encoder state, in the flat
/// Media2 shape `VideoEncoderConfiguration2::from_xml` expects.
fn render_video_encoder(ve: &VideoEncoderState) -> String {
    format!(
        r#"<tr2:Configurations token="{token}">
            <tt:Name>{name}</tt:Name>
            <tt:UseCount>{use_count}</tt:UseCount>
            <tt:Encoding>{encoding}</tt:Encoding>
            <tt:Resolution><tt:Width>{width}</tt:Width><tt:Height>{height}</tt:Height></tt:Resolution>
            <tt:RateControl>
              <tt:FrameRateLimit>{fr}</tt:FrameRateLimit>
              <tt:BitrateLimit>{br}</tt:BitrateLimit>
            </tt:RateControl>
            <tt:GovLength>{gov}</tt:GovLength>
            <tt:Profile>{profile}</tt:Profile>
            <tt:Quality>{quality}</tt:Quality>
          </tr2:Configurations>"#,
        token = ve.token,
        name = ve.name,
        use_count = ve.use_count,
        encoding = ve.encoding,
        width = ve.width,
        height = ve.height,
        fr = ve.frame_rate_limit,
        br = ve.bitrate_limit,
        gov = ve.gov_length,
        profile = ve.profile,
        quality = ve.quality,
    )
}

pub fn resp_video_encoder_instances() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetVideoEncoderInstancesResponse>
          <tr2:Info>
            <tt:Total>4</tt:Total>
            <tt:Encoding>
              <tt:Encoding>H264</tt:Encoding>
              <tt:Number>2</tt:Number>
            </tt:Encoding>
            <tt:Encoding>
              <tt:Encoding>H265</tt:Encoding>
              <tt:Number>2</tt:Number>
            </tt:Encoding>
          </tr2:Info>
        </tr2:GetVideoEncoderInstancesResponse>"#,
    )
}

/// Create a profile — in the shared list, not in a string literal.
///
/// It used to answer with a hardcoded `Profile_New_M2` and take no `state`, so
/// it reported a success the caller could not then act on: the token it named
/// appeared in no subsequent `GetProfiles`, from either service. The write now
/// goes through [`media::create_profile_in_state`], the same call Media1 makes;
/// only the response envelope differs (Media2 returns the bare token, Media1 the
/// whole profile).
pub fn handle_create_profile_media2(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "CreateProfile").unwrap_or_default();
    let name = extract_tag(&inner, "Name").unwrap_or_else(|| "Profile".to_string());
    // `tr2:CreateProfile` carries `Name` and an optional `Configuration` list —
    // and, unlike `trt:CreateProfile`, **no caller-supplied token**. The device
    // always assigns.
    match media::create_profile_in_state(state, &name, None) {
        media::CreateOutcome::Created(entry) => soap(
            NS,
            &format!(
                "<tr2:CreateProfileResponse><tr2:Token>{}</tr2:Token></tr2:CreateProfileResponse>",
                entry.token
            ),
        ),
        media::CreateOutcome::Duplicate(t) => resp_soap_fault(
            "ter:ProfileExists",
            &format!("Profile token already in use: {t}"),
        ),
    }
}

/// Delete a profile — and actually delete it.
///
/// The dispatcher used to answer this with `resp_empty`, an unconditional
/// success that removed nothing and reported nothing about a token that did not
/// exist or a fixed profile that cannot be removed.
pub fn handle_delete_profile_media2(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "DeleteProfile").unwrap_or_default();
    // **`Token`, not `ProfileToken`.** `tr2:DeleteProfile` names it `Token`
    // where `trt:DeleteProfile` says `ProfileToken`. Reusing Media1's handler
    // wholesale would have read the wrong element and faulted on every valid
    // request — the reason these are two handlers over one state rather than
    // one handler with a prefix argument.
    let token = extract_tag(&inner, "Token").unwrap_or_default();
    if token.is_empty() {
        return resp_soap_fault("ter:InvalidArgs", "Token missing");
    }

    match media::delete_profile_in_state(state, &token) {
        media::DeleteOutcome::Deleted => resp_empty("tr2", "DeleteProfileResponse"),
        media::DeleteOutcome::NotFound => {
            resp_soap_fault("ter:NoProfile", &format!("Profile not found: {token}"))
        }
        media::DeleteOutcome::Fixed => resp_soap_fault(
            "ter:DeletionOfFixedProfile",
            &format!("Cannot delete fixed profile: {token}"),
        ),
    }
}

// ── Metadata configurations ───────────────────────────────────────────────────
//
// Audit §5 (Tier 3): the getter, the options getter and `SetMetadataConfiguration`
// were all static — a consistent stub, so `Get` never claimed to reflect the
// write, but a family a caller would reasonably expect to work.
//
// All three are addressed by the same `ConfigurationToken`, so making only the
// configurations getter state-driven would leave the options getter answering
// for the wrong configuration — the exact per-channel failure the multi-sensor
// rule in `CLAUDE.md` describes.

fn render_metadata(e: &crate::mock::state::MetadataEntry) -> String {
    // Multicast is genuinely optional in `tt:MetadataConfiguration`, and
    // `MetadataConfiguration` parses both fields as `Option`. Unlike the
    // Storage case, omitting the block here **is** observable from a client.
    let multicast = match (&e.multicast_address, e.multicast_port) {
        (Some(addr), Some(port)) => format!(
            "<tt:Multicast>\
               <tt:Address><tt:Type>IPv4</tt:Type><tt:IPv4Address>{addr}</tt:IPv4Address></tt:Address>\
               <tt:Port>{port}</tt:Port>\
             </tt:Multicast>"
        ),
        _ => String::new(),
    };
    format!(
        "<tr2:Configurations token=\"{token}\">\
           <tt:Name>{name}</tt:Name>\
           <tt:UseCount>{use_count}</tt:UseCount>\
           <tt:Analytics>{analytics}</tt:Analytics>\
           <tt:PTZStatus>\
             <tt:Status>{status}</tt:Status>\
             <tt:Position>{position}</tt:Position>\
           </tt:PTZStatus>\
           {multicast}\
         </tr2:Configurations>",
        token = e.token,
        name = e.name,
        use_count = e.use_count,
        analytics = e.analytics,
        status = e.ptz_status,
        position = e.ptz_position,
    )
}

/// `GetMetadataConfigurations`. The `ConfigurationToken` filter is optional in
/// the WSDL: absent means "all", present means exactly that one. A token that
/// names nothing yields an empty list rather than a fault, matching the
/// filter semantics — it is a query, not an addressed read.
pub fn resp_metadata_configurations(state: &SharedState, body: &str) -> String {
    let want = extract_tag(body, "ConfigurationToken").filter(|t| !t.is_empty());
    let s = state.read();
    let items: String = s
        .metadata
        .iter()
        .filter(|e| want.as_deref().is_none_or(|w| w == e.token))
        .map(render_metadata)
        .collect();
    soap(
        NS,
        &format!(
            "<tr2:GetMetadataConfigurationsResponse>{items}</tr2:GetMetadataConfigurationsResponse>"
        ),
    )
}

/// `GetMetadataConfigurationOptions` — answers for the addressed
/// configuration. `AnalyticsSupported` lives under `Options/Extension`, which
/// is where `MetadataConfigurationOptions::from_xml` looks for it; the old
/// static fixture omitted it entirely, so that parser branch was never fed and
/// every caller saw `analytics_supported: false`.
pub fn resp_metadata_configuration_options(state: &SharedState, body: &str) -> String {
    let want = extract_tag(body, "ConfigurationToken").filter(|t| !t.is_empty());
    let s = state.read();
    let entry = match &want {
        Some(w) => s.metadata.iter().find(|e| &e.token == w),
        None => s.metadata.first(),
    };
    let Some(entry) = entry else {
        return resp_soap_fault(
            "ter:NoConfig",
            &format!(
                "NoSuchMetadataConfig-METAOPT-5812: {}",
                want.unwrap_or_default()
            ),
        );
    };
    let analytics = entry.analytics_supported;
    soap(
        NS,
        &format!(
            "<tr2:GetMetadataConfigurationOptionsResponse>\
               <tr2:Options>\
                 <tt:PTZStatusFilterOptions/>\
                 <tt:Extension><tt:AnalyticsSupported>{analytics}</tt:AnalyticsSupported></tt:Extension>\
               </tr2:Options>\
             </tr2:GetMetadataConfigurationOptionsResponse>"
        ),
    )
}

/// `SetMetadataConfiguration`. Updates in place; an unknown token faults
/// rather than being silently created, for the same reason as Storage — a typo
/// must not be indistinguishable from a successful update.
///
/// `analytics_supported` is deliberately **not** writable: it is a device
/// capability reported by the options getter, not part of
/// `tt:MetadataConfiguration`, and the client never sends it.
pub fn handle_set_metadata_configuration(state: &SharedState, body: &str) -> String {
    let Some(token) = crate::mock::xml_parse::extract_attr(body, "Configuration", "token")
        .filter(|t| !t.is_empty())
    else {
        return resp_soap_fault(
            "env:Sender",
            "NoMetadataToken-SETMETA-5810: Configuration/@token is required",
        );
    };
    if !state.read().metadata.iter().any(|e| e.token == token) {
        return resp_soap_fault(
            "ter:NoConfig",
            &format!("NoSuchMetadataConfig-SETMETA-5811: {token}"),
        );
    }
    let name = extract_tag(body, "Name").unwrap_or_default();
    let analytics = extract_tag(body, "Analytics").as_deref() == Some("true");
    // `Status` and `Position` are both inside `tt:PTZStatus`; read that subtree
    // so a `Status` element elsewhere in the body cannot be mistaken for it.
    let ptz = extract_tag(body, "PTZStatus").unwrap_or_default();
    let ptz_status = extract_tag(&ptz, "Status").as_deref() == Some("true");
    let ptz_position = extract_tag(&ptz, "Position").as_deref() == Some("true");
    state.modify(|s| {
        if let Some(e) = s.metadata.iter_mut().find(|e| e.token == token) {
            e.name = name.clone();
            e.analytics = analytics;
            e.ptz_status = ptz_status;
            e.ptz_position = ptz_position;
            eprintln!("    [STATE] metadata config updated: {token}");
        }
    });
    resp_empty("tr2", "SetMetadataConfigurationResponse")
}

pub fn resp_audio_source_configurations_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetAudioSourceConfigurationsResponse>
          <tr2:Configurations token="ASC_1">
            <tt:Name>AudioSourceConfig</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:SourceToken>AudioSrc_1</tt:SourceToken>
          </tr2:Configurations>
        </tr2:GetAudioSourceConfigurationsResponse>"#,
    )
}

pub fn resp_audio_encoder_configurations_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetAudioEncoderConfigurationsResponse>
          <tr2:Configurations token="AEC_1">
            <tt:Name>AudioEncoderConfig</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>G711</tt:Encoding>
            <tt:Bitrate>64</tt:Bitrate>
            <tt:SampleRate>8</tt:SampleRate>
          </tr2:Configurations>
        </tr2:GetAudioEncoderConfigurationsResponse>"#,
    )
}

pub fn resp_audio_encoder_configuration_options_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetAudioEncoderConfigurationOptionsResponse>
          <tr2:Options>
            <tt:Options>
              <tt:Encoding>G711</tt:Encoding>
              <tt:BitrateList><tt:Items>64</tt:Items></tt:BitrateList>
              <tt:SampleRateList><tt:Items>8</tt:Items></tt:SampleRateList>
            </tt:Options>
          </tr2:Options>
        </tr2:GetAudioEncoderConfigurationOptionsResponse>"#,
    )
}

pub fn resp_audio_output_configurations() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetAudioOutputConfigurationsResponse>
          <tr2:Configurations token="AOC_1">
            <tt:Name>AudioOutput</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:OutputToken>AudioOut_1</tt:OutputToken>
            <tt:OutputLevel>50</tt:OutputLevel>
          </tr2:Configurations>
        </tr2:GetAudioOutputConfigurationsResponse>"#,
    )
}

pub fn resp_audio_decoder_configurations() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetAudioDecoderConfigurationsResponse>
          <tr2:Configurations token="ADC_1">
            <tt:Name>AudioDecoder</tt:Name>
            <tt:UseCount>1</tt:UseCount>
          </tr2:Configurations>
        </tr2:GetAudioDecoderConfigurationsResponse>"#,
    )
}

pub fn resp_video_source_modes() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetVideoSourceModesResponse>
          <tr2:VideoSourceModes token="Mode_1">
            <tt:MaxFramerate>30</tt:MaxFramerate>
            <tt:MaxResolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:MaxResolution>
            <tt:Encodings>H264 H265</tt:Encodings>
            <tt:Reboot>false</tt:Reboot>
          </tr2:VideoSourceModes>
        </tr2:GetVideoSourceModesResponse>"#,
    )
}

/// `SetVideoSourceMode` — **faults; the mock does not model sensor modes.**
///
/// Until 0.15 this answered `<tr2:Reboot>false</tr2:Reboot>`, i.e. "the mode was
/// switched and no reboot is needed". Nothing was stored, and nothing could
/// have been: `resp_video_source_modes` is a static one-mode list, and oxvif's
/// [`VideoSourceMode`](crate::VideoSourceMode) carries no active-mode field, so
/// **no getter in this crate could ever contradict the claim.** That is the
/// worst combination a mock can offer — an unfalsifiable success — and it is why
/// `CLAUDE.md` step 5c says to prefer a fault when a write has no getter that
/// could show it: the caller then learns the mock does not model the operation
/// instead of being told, wrongly, that it worked.
///
/// This is *not* the same situation as `SetRelayOutputState`, which the same
/// paragraph once grouped with it: that one really does write
/// `RelayOutputState::logical_state` and emit an event, so it is observable —
/// just not through `GetRelayOutputs`, which by spec does not return the live
/// state.
///
/// Wiring it for real means adding `Enabled` to `VideoSourceMode` (the ONVIF
/// schema has it; oxvif's type does not) and a mode catalogue in `DeviceState`.
/// That is a public-API change, so it is left as a deliberate gap rather than
/// smuggled in behind a mock fix — recorded in `docs/mock-server.md` §13.
pub fn resp_set_video_source_mode() -> String {
    resp_soap_fault(
        "ter:ActionNotSupported",
        "NotModelled-VSMODE-5813: the mock does not switch video source modes; \
         nothing was stored, and no getter could show it if it had been",
    )
}

// ── GetServiceCapabilities ───────────────────────────────────────────────────

/// `tr2:Capabilities2` — note the `2` in the type name; ver20 media.wsdl
/// defines no type called `Capabilities`.
///
/// Claims track what this mock dispatches: `GetSnapshotUri`,
/// `GetVideoSourceModes`/`SetVideoSourceMode`, and the `AddConfiguration`
/// kinds. Privacy masks and WebRTC are not implemented, so `Mask` /
/// `SourceMask` are `false` and `WebRTC` is `0` — an **`xs:int` session
/// count**, not a boolean.
pub fn resp_service_capabilities_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetServiceCapabilitiesResponse>
          <tr2:Capabilities SnapshotUri="true"
                            Rotation="false"
                            VideoSourceMode="true"
                            OSD="false"
                            Mask="false"
                            SourceMask="false"
                            WebRTC="0">
            <tr2:ProfileCapabilities MaximumNumberOfProfiles="8"
                                     ConfigurationsSupported="VideoSource VideoEncoder AudioSource AudioEncoder Metadata"/>
            <tr2:StreamingCapabilities RTSPStreaming="true"
                                       RTPMulticast="false"
                                       RTP_RTSP_TCP="true"
                                       NonAggregateControl="false"
                                       AutoStartMulticast="false"/>
          </tr2:Capabilities>
        </tr2:GetServiceCapabilitiesResponse>"#,
    )
}
