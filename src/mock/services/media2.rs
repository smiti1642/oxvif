use crate::mock::helpers::{resp_empty, resp_soap_fault, soap};
use crate::mock::services::media;
use crate::mock::state::{AudioEncoderEntry, ProfileEntry, SharedState, VideoEncoderState};

const NS: &str = r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#;

/// One profile in the Media2 shape.
///
/// **Not a prefix swap on `media::render_profile`.** The two services wrap their
/// configurations differently — Media1 lists them as siblings of `Name`, Media2
/// groups them under a single `<tr2:Configurations>` — the member *names* differ
/// (`VideoSource` against `VideoSourceConfiguration`), the *sequence* is a
/// different declaration in a different schema, and two of the *types* differ
/// (`VideoEncoder2Configuration`, `AudioEncoder2Configuration`). Two genuinely
/// different shapes over the same [`ProfileEntry`], which is why the state is
/// shared and the renderers are not.
///
/// **Each member carries the whole configuration, not a token reference.**
/// `tr2:ConfigurationSet` types every member as the full configuration —
/// `VideoSource` is `tt:VideoSourceConfiguration`, the same type `tt:Profile`
/// inlines — so a conformant Media2 device sends the body, and this renderer
/// now does too. It emitted `<tr2:VideoSource token="…"/>` and nothing else
/// until 0.15, which was five distinct required-member violations at once and
/// left `MediaProfile2::video_source_token` permanently `None`, because that
/// field is read from a `SourceToken` *inside* the configuration.
///
/// The `token` attribute stays: it is `use="required"` on every one of these
/// types, and the five `*_token` fields on [`MediaProfile2`] read it.
///
/// Every body comes from the same helper the corresponding list getter uses, so
/// a profile can never disagree with `GetVideoEncoderConfigurations` about a
/// configuration they both name.
///
/// [`MediaProfile2`]: crate::MediaProfile2
fn render_profile_media2(
    p: &ProfileEntry,
    tag: &str,
    cat: &media::Catalogues,
) -> Result<String, String> {
    // Declaration order of `tr2:ConfigurationSet`: VideoSource, AudioSource,
    // VideoEncoder, AudioEncoder, Analytics, PTZ, … — audio source sits
    // *between* the two video members. This emitted the two video members
    // together until 0.15.
    let vsc = p
        .video_source_config_token
        .as_deref()
        .and_then(|t| cat.vscs.iter().find(|c| c.token == t))
        .map(|c| media::render_vsc_body(c, "tr2:VideoSource"))
        .unwrap_or_default();
    let asc = p
        .audio_source_config_token
        .as_deref()
        .and_then(|t| cat.ascs.iter().find(|c| c.token == t))
        .map(|c| media::render_audio_source_config(c, "tr2:AudioSource"))
        .unwrap_or_default();
    let vec = p
        .video_encoder_config_token
        .as_deref()
        .and_then(|t| cat.vecs.iter().find(|c| c.token == t))
        .map(|c| render_video_encoder(c, "tr2:VideoEncoder"))
        .transpose()?
        .unwrap_or_default();
    let aec = p
        .audio_encoder_config_token
        .as_deref()
        .and_then(|t| cat.aecs.iter().find(|c| c.token == t))
        .map(|c| render_audio_encoder_media2(c, "tr2:AudioEncoder"))
        .transpose()?
        .unwrap_or_default();
    // `MediaProfile2::ptz_config_token` reads `Configurations/PTZ@token` and
    // nothing ever fed it: neither profile renderer emitted a PTZ element, and
    // `ProfileEntry` had no slot to emit from. `PTZ` is `tt:PTZConfiguration`
    // here and in `tt:Profile`, so both services render it from
    // `ptz::render_config`.
    let ptz = p
        .ptz_config_token
        .as_deref()
        .and_then(|t| cat.ptzs.iter().find(|c| c.token == t))
        .map(|c| super::ptz::render_config(c, "tr2:PTZ"))
        .unwrap_or_default();
    // A profile with nothing bound omits the wrapper rather than sending an
    // empty one — a freshly created profile is exactly that case.
    let configurations =
        if vsc.is_empty() && asc.is_empty() && vec.is_empty() && aec.is_empty() && ptz.is_empty() {
            String::new()
        } else {
            format!("<tr2:Configurations>{vsc}{asc}{vec}{aec}{ptz}</tr2:Configurations>")
        };
    // `tr2:Name`, not `tt:Name`: `tr2:MediaProfile` declares `Name` locally and
    // `media2.wsdl` sets `elementFormDefault="qualified"`, so it is in the
    // Media2 namespace. Media1's `tt:Profile` declares its own `Name` in
    // `onvif.xsd`, which is why the same-looking element differs by service.
    Ok(format!(
        r#"<tr2:{tag} token="{token}" fixed="{fixed}">
          <tr2:Name>{name}</tr2:Name>
          {configurations}
        </tr2:{tag}>"#,
        token = crate::types::xml_escape(&p.token),
        fixed = p.fixed,
        name = crate::types::xml_escape(&p.name),
    ))
}

/// The device's profiles, in the Media2 shape.
///
/// This used to be a string literal with no `state` parameter, so a caller who
/// seeded `DeviceState.profiles` got their list from Media1 and four hardcoded
/// `Profile_A`…`Profile_D` from Media2 — same device, same process, same state,
/// no error, no overlap in the token sets. Reported by a C++ ONVIF test suite
/// whose harness seeded 20 profiles and whose DLL, negotiating Media2, received
/// 4.
///
/// The parsed request selects profile identities and projects requested modeled
/// configuration kinds on clones. It never removes bindings from shared state.
/// This is not yet complete field/attribute or nested-renderer validation.
pub fn resp_profiles_media2(state: &SharedState, operation: &crate::mock::request::Node) -> String {
    use crate::mock::fault::{Code, Fault, INVALID_ARG_VAL, NO_PROFILE};
    use crate::mock::request::RequestError;
    let namespace = "http://www.onvif.org/ver20/media/wsdl";
    let selectors = || -> Result<_, RequestError> {
        operation.check_child_sequence(namespace, &["Token", "Type"])?;
        let token = operation
            .child(namespace, "Token")?
            .map(|node| node.scalar_text())
            .transpose()?;
        let types = operation
            .children_named(namespace, "Type")
            .map(|node| node.scalar_text())
            .collect::<Result<Vec<_>, _>>()?;
        Ok((token, types))
    };
    let (token, types) = match selectors() {
        Ok(selectors) => selectors,
        Err(error) => return error.to_fault(),
    };
    if token == Some("") {
        return media::empty_profile_token_fault(Code::Sender);
    }
    let (snapshot, cat) = media::profile_snapshot(state);
    if token.is_none() && snapshot.iter().any(|profile| profile.token.is_empty()) {
        return media::empty_profile_token_fault(Code::Receiver);
    }
    if let Some(token) = token
        && !snapshot.iter().any(|profile| profile.token == token)
    {
        return Fault::new(
            Code::Sender,
            &[INVALID_ARG_VAL, NO_PROFILE],
            &format!("Profile not found: {token}"),
        )
        .to_xml();
    }
    // Type selects configuration content, never the profile set. Only a single
    // All expands to every associated configuration; other lists match literally.
    // Project onto clones so reads cannot unbind shared profiles.
    let selected = |kind: &str| types.as_slice() == ["All"] || types.contains(&kind);
    let items = snapshot
        .into_iter()
        .filter(|profile| token.is_none_or(|token| profile.token == token))
        .map(|mut profile| {
            for (kind, slot) in [
                ("VideoSource", &mut profile.video_source_config_token),
                ("VideoEncoder", &mut profile.video_encoder_config_token),
                ("AudioSource", &mut profile.audio_source_config_token),
                ("AudioEncoder", &mut profile.audio_encoder_config_token),
                ("PTZ", &mut profile.ptz_config_token),
            ] {
                if !selected(kind) {
                    *slot = None;
                }
            }
            render_profile_media2(&profile, "Profiles", &cat)
        })
        .collect::<Result<String, String>>();
    let items = match items {
        Ok(items) => items,
        Err(fault) => return fault,
    };
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

pub fn resp_video_source_configurations_media2(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::video_source::get(state, operation, true)
}

/// Per-channel — the bounds ceiling is the addressed sensor's own resolution.
///
/// The response type is `tt:VideoSourceConfigurationOptions`, the same one
/// Media1 returns, so `MaximumNumberOfProfiles` is an `xs:attribute` here for
/// the same reason and with the same value — see
/// [`super::media::resp_video_source_configuration_options`].
pub fn resp_video_source_configuration_options_media2(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::video_source::options(state, operation, true)
}

pub fn resp_video_encoder_configuration_options_media2(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::video_encoder::options(state, operation, true)
}

pub fn resp_video_encoder_configurations(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::video_encoder::get(state, operation, true, false)
}

/// Atomically apply the complete modeled source configuration and publish its
/// effect only after success. Unsupported source settings are explicitly refused.
pub fn handle_set_video_source_configuration_media2(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    super::video_source::set(state, operation, true, effect)
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
pub fn handle_add_configuration_media2(
    state: &SharedState,
    body: &str,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    match apply_media2_configuration(state, body, operation, true) {
        Ok(()) => {
            *effect = Some(crate::mock::effect::Effect::ProfilesChanged);
            resp_empty("tr2", "AddConfigurationResponse")
        }
        Err(fault) => fault,
    }
}

pub fn handle_remove_configuration_media2(
    state: &SharedState,
    body: &str,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    match apply_media2_configuration(state, body, operation, false) {
        Ok(()) => {
            *effect = Some(crate::mock::effect::Effect::ProfilesChanged);
            resp_empty("tr2", "RemoveConfigurationResponse")
        }
        Err(fault) => fault,
    }
}

/// Both directions of the Media2 configuration binding.
///
/// The request may carry several `<tr2:Configuration>` children; every one is
/// planned, and all supported kinds and required tokens are checked before any
/// slot is written. The shared value-based plan commits under one write lock.
fn apply_media2_configuration(
    state: &SharedState,
    _body: &str,
    operation: &crate::mock::request::Node,
    add: bool,
) -> Result<(), String> {
    let profile = operation
        .optional_child_text("http://www.onvif.org/ver20/media/wsdl", "ProfileToken")
        .map_err(|error| error.to_fault())?
        .unwrap_or_default();
    let ns = "http://www.onvif.org/ver20/media/wsdl";
    let name = if add {
        operation
            .optional_child_text(ns, "Name")
            .map_err(|error| error.to_fault())?
    } else {
        None
    };
    let planned = configuration_plan(operation, add)?;
    if !add
        && operation
            .children_named(ns, "Configuration")
            .next()
            .is_none()
    {
        return Err(resp_soap_fault(
            "env:Sender",
            "NoConfiguration-CFG2-5541: at least one tr2:Configuration is required",
        ));
    }

    let tag = if add { "ADDCFG2-5543" } else { "RMCFG2-5544" };
    media::apply_configuration_bindings(state, profile, &planned, add, name, tag)
}

/// Project all direct references without fragment extraction. All is a no-op
/// during add/create; on remove it clears every modeled slot. Token values are
/// ignored on remove, but malformed scalar/duplicate fields are still refused.
fn configuration_plan(
    operation: &crate::mock::request::Node,
    add: bool,
) -> Result<Vec<(media::ConfigKind, String)>, String> {
    let ns = "http://www.onvif.org/ver20/media/wsdl";
    let mut planned = Vec::new();
    for entry in operation.children_named(ns, "Configuration") {
        entry
            .check_child_sequence(ns, &["Type", "Token"])
            .map_err(|error| error.to_fault())?;
        let type_ = entry
            .required_child_text(ns, "Type")
            .map_err(|error| error.to_fault())?;
        let token = entry
            .optional_child_text(ns, "Token")
            .map_err(|error| error.to_fault())?;
        if type_ == "All" {
            if !add {
                planned.extend(
                    media::ConfigKind::ALL
                        .into_iter()
                        .map(|kind| (kind, String::new())),
                );
            }
            continue;
        }
        let Some(kind) = media::ConfigKind::from_media2_type(type_) else {
            return Err(resp_soap_fault(
                "ter:ConfigurationConflict",
                &format!(
                    "UnmodelledConfigType-CFG2-5542: the mock's ProfileEntry has no slot for \
                     {type_}, and MediaProfile2 exposes none, so a success here could never be \
                     observed"
                ),
            ));
        };
        planned.push((
            kind,
            if add {
                token.unwrap_or_default().to_owned()
            } else {
                String::new()
            },
        ));
    }
    Ok(planned)
}

pub fn handle_set_video_encoder_configuration(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    super::video_encoder::set(state, operation, true, effect)
}

/// One `tt:VideoEncoder2Configuration`, in the flat Media2 shape
/// `VideoEncoderConfiguration2::from_xml` expects.
///
/// `qname` differs by context — `tr2:Configurations` in the list getter,
/// `tr2:VideoEncoder` inlined in a profile. One body for both, because
/// `tr2:ConfigurationSet/VideoEncoder` and
/// `tr2:GetVideoEncoderConfigurationsResponse/Configurations` are the *same*
/// type; a second copy could drift and nothing here would notice.
///
/// **`GovLength` and `Profile` are attributes.** `tt:VideoEncoder2Configuration`
/// declares both as `xs:attribute` and only `Name`, `UseCount`, `Encoding`,
/// `Resolution`, `RateControl`, `Multicast` and `Quality` as child elements.
/// This rendered them as elements until 0.15, agreeing with the client bug it
/// was written beside — and the type carries an `xs:any`, so the shape checker's
/// `UNKNOWN-CHILD` rule could not report `GovLength` at all. Its
/// `ATTR-AS-ELEMENT` rule, added later in 0.15, can: reverting this line opens
/// two rows, one here and one on the profile that inlines the same helper.
pub(super) fn render_video_encoder(ve: &VideoEncoderState, qname: &str) -> Result<String, String> {
    super::video_rate::view(ve, true)?;
    let codec = if ve.encoding == "JPEG" {
        String::new()
    } else {
        format!(
            " GovLength=\"{}\" Profile=\"{}\"",
            ve.gov_length,
            crate::types::xml_escape(&ve.profile)
        )
    };
    Ok(format!(
        r#"<{qname} token="{token}"{codec}>
            <tt:Name>{name}</tt:Name>
            <tt:UseCount>{use_count}</tt:UseCount>
            <tt:Encoding>{encoding}</tt:Encoding>
            <tt:Resolution><tt:Width>{width}</tt:Width><tt:Height>{height}</tt:Height></tt:Resolution>
            <tt:RateControl>
              <tt:FrameRateLimit>{fr}</tt:FrameRateLimit>
              <tt:BitrateLimit>{br}</tt:BitrateLimit>
            </tt:RateControl>
            <tt:Quality>{quality}</tt:Quality>
          </{qname}>"#,
        token = crate::types::xml_escape(&ve.token),
        name = crate::types::xml_escape(&ve.name),
        use_count = ve.use_count,
        encoding = crate::types::xml_escape(&ve.encoding),
        width = ve.width,
        height = ve.height,
        fr = ve.frame_rate_limit,
        br = ve.bitrate_limit,
        quality = ve.quality,
    ))
}

pub fn resp_video_encoder_instances(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::video_encoder::instances(state, operation)
}

/// Create a profile — in the shared list, not in a string literal.
///
/// It used to answer with a hardcoded `Profile_New_M2` and take no `state`, so
/// it reported a success the caller could not then act on: the token it named
/// appeared in no subsequent `GetProfiles`, from either service. The write now
/// uses the shared allocator with initial configuration bindings committed
/// atomically. Media2 returns the bare token; Media1 returns the whole profile.
pub fn handle_create_profile_media2(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    let name = match media::profile_name(operation, "http://www.onvif.org/ver20/media/wsdl") {
        Ok(name) => name,
        Err(error) => return error.to_fault(),
    };
    // `tr2:CreateProfile` carries `Name` and an optional `Configuration` list —
    // and, unlike `trt:CreateProfile`, **no caller-supplied token**. The device
    // always assigns.
    let planned = match configuration_plan(operation, true) {
        Ok(planned) => planned,
        Err(fault) => return fault,
    };
    match media::create_profile_with_bindings(state, name, None, &planned) {
        media::CreateOutcome::Created(entry) => {
            *effect = Some(crate::mock::effect::Effect::ProfilesChanged);
            soap(
                NS,
                &format!(
                    "<tr2:CreateProfileResponse><tr2:Token>{}</tr2:Token></tr2:CreateProfileResponse>",
                    crate::types::xml_escape(&entry.token)
                ),
            )
        }
        media::CreateOutcome::Duplicate(t) => resp_soap_fault(
            "ter:ProfileExists",
            &format!("Profile token already in use: {t}"),
        ),
        media::CreateOutcome::EmptyToken => {
            media::empty_profile_token_fault(crate::mock::fault::Code::Sender)
        }
        media::CreateOutcome::Rejected(fault) => fault,
    }
}

/// Delete a profile — and actually delete it.
///
/// The dispatcher used to answer this with `resp_empty`, an unconditional
/// success that removed nothing and reported nothing about a token that did not
/// exist or a fixed profile that cannot be removed.
pub fn handle_delete_profile_media2(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    use crate::mock::fault::{
        ACTION, Code, DELETION_OF_FIXED_PROFILE, Fault, INVALID_ARG_VAL, NO_PROFILE,
    };
    // **`Token`, not `ProfileToken`.** `tr2:DeleteProfile` names it `Token`
    // where `trt:DeleteProfile` says `ProfileToken`. Reusing Media1's handler
    // wholesale would have read the wrong element and faulted on every valid
    // request — the reason these are two handlers over one state rather than
    // one handler with a prefix argument.
    let token =
        match operation.required_child_text("http://www.onvif.org/ver20/media/wsdl", "Token") {
            Ok(token) => token,
            Err(error) => {
                return resp_soap_fault(
                    "env:Sender",
                    &format!("InvalidRequest-DELETEPROFILE: {}", error.message()),
                );
            }
        };

    match media::delete_profile_in_state(state, token) {
        media::DeleteOutcome::Deleted => {
            *effect = Some(crate::mock::effect::Effect::ProfilesChanged);
            resp_empty("tr2", "DeleteProfileResponse")
        }
        // Media2 service 26.06 §5.1.5: both refusals are Sender faults.
        media::DeleteOutcome::NotFound => Fault::new(
            Code::Sender,
            &[INVALID_ARG_VAL, NO_PROFILE],
            &format!("Profile not found: {token}"),
        )
        .to_xml(),
        media::DeleteOutcome::Fixed => Fault::new(
            Code::Sender,
            &[ACTION, DELETION_OF_FIXED_PROFILE],
            &format!("Cannot delete fixed profile: {token}"),
        )
        .to_xml(),
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

// AM1 uses the complete stored multicast and session-timeout value.

pub fn resp_metadata_configurations(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::audio_metadata::get(state, operation, true, "GetMetadataConfigurations")
}

pub fn resp_metadata_configuration_options(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::audio_metadata::options(state, operation, true, true)
}

pub fn handle_set_metadata_configuration(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    super::audio_metadata::set_metadata(state, operation, effect)
}

pub fn resp_audio_source_configurations_media2(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::audio_metadata::get(state, operation, true, "GetAudioSourceConfigurations")
}

fn render_audio_encoder_media2(c: &AudioEncoderEntry, qname: &str) -> Result<String, String> {
    super::audio_metadata::audio_render(c, qname, true)
}

pub fn resp_audio_encoder_configurations_media2(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::audio_metadata::get(state, operation, true, "GetAudioEncoderConfigurations")
}

pub fn resp_audio_encoder_configuration_options_media2(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::audio_metadata::options(state, operation, true, false)
}

pub fn handle_set_audio_encoder_configuration_media2(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    media::apply_audio_encoder_write(state, operation, true, effect)
}

pub fn resp_audio_output_configurations(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::audio_metadata::get(state, operation, true, "GetAudioOutputConfigurations")
}

pub fn resp_audio_decoder_configurations(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::audio_metadata::get(state, operation, true, "GetAudioDecoderConfigurations")
}

/// All four members are `tr2:` — `tr2:VideoSourceMode` declares them locally
/// and `media2.wsdl` is `elementFormDefault="qualified"`.
///
/// **`MaxResolution`'s children are not.** The element name follows its
/// declaration; its *content* follows its type, and `MaxResolution` is typed
/// `tt:VideoResolution`, whose `Width` and `Height` are declared in
/// `onvif.xsd`. Moving a subtree wholesale is the mistake to avoid here.
pub fn resp_video_source_modes() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetVideoSourceModesResponse>
          <tr2:VideoSourceModes token="Mode_1">
            <tr2:MaxFramerate>30</tr2:MaxFramerate>
            <tr2:MaxResolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tr2:MaxResolution>
            <tr2:Encodings>H264 H265</tr2:Encodings>
            <tr2:Reboot>false</tr2:Reboot>
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
        &format!(
            r#"<tr2:GetServiceCapabilitiesResponse>
          <tr2:Capabilities SnapshotUri="true"
                            Rotation="false"
                            VideoSourceMode="true"
                            OSD="false"
                            Mask="false"
                            SourceMask="false"
                            WebRTC="0">
            <tr2:ProfileCapabilities MaximumNumberOfProfiles="{limit}"
                                     ConfigurationsSupported="VideoSource VideoEncoder AudioSource AudioEncoder PTZ"/>
            <tr2:StreamingCapabilities RTSPStreaming="true"
                                       RTPMulticast="false"
                                       RTP_RTSP_TCP="true"
                                       NonAggregateControl="false"
                                       AutoStartMulticast="false"/>
          </tr2:Capabilities>
        </tr2:GetServiceCapabilitiesResponse>"#,
            limit = media::PROFILE_LIMIT
        ),
    )
}
