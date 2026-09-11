use crate::mock::helpers::{resp_empty, resp_soap_fault, soap};
use crate::mock::state::{
    AudioEncoderEntry, AudioOptionEntry, AudioSourceConfigEntry, OSD_QUOTA_DATE,
    OSD_QUOTA_DATE_AND_TIME, OSD_QUOTA_PLAIN, OSD_QUOTA_TIME, OSD_QUOTA_TOTAL, OsdColorEntry,
    OsdEntry, OsdTextEntry, ProfileEntry, PtzConfigEntry, SharedState, VideoEncoderState,
    VideoSourceConfigEntry,
};
use crate::mock::xml_parse::{extract_all_tags, extract_attr, extract_tag};

/// Existing advertised synthetic capacity; imported fixtures are never truncated.
pub(crate) const PROFILE_LIMIT: usize = 8;

/// B16 acknowledgment only. Dispatch policy refuses by default; opting in does
/// not bypass selector validation and never produces a committed stream effect.
pub fn handle_set_synchronization_point(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    media2: bool,
) -> String {
    use crate::mock::{
        fault::{Code, Fault, INVALID_ARG_VAL, MOCK_REQUEST_POLICY, NO_PROFILE},
        request::RequestError,
    };
    let ns = if media2 {
        "http://www.onvif.org/ver20/media/wsdl"
    } else {
        "http://www.onvif.org/ver10/media/wsdl"
    };
    let token = (|| {
        if operation.expanded_attributes().next().is_some() {
            return Err(RequestError::OperationIdentity);
        }
        operation.check_child_sequence(ns, &["ProfileToken"])?;
        let field = operation
            .child(ns, "ProfileToken")?
            .ok_or(RequestError::MissingField)?;
        if field.expanded_attributes().next().is_some() {
            return Err(RequestError::OperationIdentity);
        }
        let token = operation.required_child_text(ns, "ProfileToken")?;
        if token.chars().count() > 64 {
            return Err(RequestError::EmptyField);
        }
        Ok(token)
    })();
    let token = match token {
        Ok(token) => token,
        Err(error) => return error.to_fault(),
    };
    let count = state
        .read()
        .profiles
        .profiles
        .iter()
        .filter(|p| p.token == token)
        .count();
    match count {
        0 => Fault::new(
            Code::Sender,
            &[INVALID_ARG_VAL, NO_PROFILE],
            &format!("Profile not found: {token}"),
        )
        .to_xml(),
        1 => resp_empty(
            if media2 { "tr2" } else { "trt" },
            "SetSynchronizationPointResponse",
        ),
        _ => Fault::new(
            Code::Receiver,
            &[MOCK_REQUEST_POLICY],
            "Ambiguous mock profile identity",
        )
        .to_xml(),
    }
}

pub fn resp_profiles(state: &SharedState) -> String {
    let (snapshot, cat) = profile_snapshot(state);
    if snapshot.iter().any(|profile| profile.token.is_empty()) {
        return empty_profile_token_fault(crate::mock::fault::Code::Receiver);
    }
    let items = snapshot
        .iter()
        .map(|p| render_profile(p, "Profiles", &cat))
        .collect::<Result<String, String>>();
    let items = match items {
        Ok(items) => items,
        Err(fault) => return fault,
    };
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!("<trt:GetProfilesResponse>{items}</trt:GetProfilesResponse>"),
    )
}

pub fn resp_profile(state: &SharedState, operation: &crate::mock::request::Node) -> String {
    let want = match operation
        .optional_child_text("http://www.onvif.org/ver10/media/wsdl", "ProfileToken")
    {
        Ok(Some("")) => return empty_profile_token_fault(crate::mock::fault::Code::Sender),
        Ok(token) => token.unwrap_or_default(),
        Err(error) => return error.to_fault(),
    };
    let (snapshot, cat) = profile_snapshot(state);
    match snapshot
        .iter()
        .find(|p| !want.is_empty() && p.token == want)
    {
        Some(p) => {
            let profile = match render_profile(p, "Profile", &cat) {
                Ok(profile) => profile,
                Err(fault) => return fault,
            };
            soap(
                r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
                &format!(
                    "<trt:GetProfileResponse>{}</trt:GetProfileResponse>",
                    profile
                ),
            )
        }
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

pub fn handle_set_video_encoder_configuration(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    super::video_encoder::set(state, operation, false, effect)
}

pub fn handle_set_video_source_configuration(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    super::video_source::set(state, operation, false, effect)
}

pub fn handle_add_video_encoder_configuration(
    state: &SharedState,
    body: &str,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    match bind_configuration(
        state,
        body,
        operation,
        ConfigKind::VideoEncoder,
        "ADDVEC-5531",
    ) {
        Ok(()) => {
            *effect = Some(crate::mock::effect::Effect::ProfilesChanged);
            resp_empty("trt", "AddVideoEncoderConfigurationResponse")
        }
        Err(fault) => fault,
    }
}

pub fn handle_remove_video_encoder_configuration(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    match unbind_configuration(state, operation, ConfigKind::VideoEncoder, "RMVEC-5532") {
        Ok(()) => {
            *effect = Some(crate::mock::effect::Effect::ProfilesChanged);
            resp_empty("trt", "RemoveVideoEncoderConfigurationResponse")
        }
        Err(fault) => fault,
    }
}

pub fn handle_add_video_source_configuration(
    state: &SharedState,
    body: &str,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    match bind_configuration(
        state,
        body,
        operation,
        ConfigKind::VideoSource,
        "ADDVSC-5533",
    ) {
        Ok(()) => {
            *effect = Some(crate::mock::effect::Effect::ProfilesChanged);
            resp_empty("trt", "AddVideoSourceConfigurationResponse")
        }
        Err(fault) => fault,
    }
}

pub fn handle_remove_video_source_configuration(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    match unbind_configuration(state, operation, ConfigKind::VideoSource, "RMVSC-5534") {
        Ok(()) => {
            *effect = Some(crate::mock::effect::Effect::ProfilesChanged);
            resp_empty("trt", "RemoveVideoSourceConfigurationResponse")
        }
        Err(fault) => fault,
    }
}

pub fn handle_create_profile(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    let name = match profile_name(operation, "http://www.onvif.org/ver10/media/wsdl") {
        Ok(name) => name,
        Err(error) => return error.to_fault(),
    };
    // Caller may supply an explicit token (rare — most cameras assign).
    let supplied_token =
        match operation.optional_child_text("http://www.onvif.org/ver10/media/wsdl", "Token") {
            Ok(token) => token.map(str::to_owned),
            Err(error) => return error.to_fault(),
        };

    let entry = match create_profile_in_state(state, name, supplied_token) {
        CreateOutcome::Created(e) => {
            *effect = Some(crate::mock::effect::Effect::ProfilesChanged);
            e
        }
        CreateOutcome::Duplicate(t) => {
            use crate::mock::fault::{Code, Fault, INVALID_ARG_VAL, PROFILE_EXISTS};
            return Fault::new(
                Code::Sender,
                &[INVALID_ARG_VAL, PROFILE_EXISTS],
                &format!("Profile token already in use: {t}"),
            )
            .to_xml();
        }
        CreateOutcome::EmptyToken => {
            return empty_profile_token_fault(crate::mock::fault::Code::Sender);
        }
        CreateOutcome::Rejected(fault) => return fault,
    };

    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            "<trt:CreateProfileResponse>{}</trt:CreateProfileResponse>",
            // A freshly created profile carries no configurations yet, so the
            // catalogues are never consulted.
            match render_profile(&entry, "Profile", &catalogues(state)) {
                Ok(profile) => profile,
                Err(fault) => return fault,
            }
        ),
    )
}

/// Profile names are decoded state text, not serialized XML. An explicit empty
/// name is distinct from omission; field-length validation is separate.
pub(crate) fn profile_name<'a>(
    operation: &'a crate::mock::request::Node,
    namespace: &str,
) -> Result<&'a str, crate::mock::request::RequestError> {
    operation
        .child(namespace, "Name")?
        .ok_or(crate::mock::request::RequestError::MissingField)?
        .scalar_text()
}

pub fn handle_delete_profile(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    use crate::mock::fault::{
        ACTION, Code, DELETION_OF_FIXED_PROFILE, Fault, INVALID_ARG_VAL, NO_PROFILE,
    };
    let token = match operation
        .required_child_text("http://www.onvif.org/ver10/media/wsdl", "ProfileToken")
    {
        Ok(token) => token,
        Err(error) => {
            return resp_soap_fault(
                "env:Sender",
                &format!("InvalidRequest-DELETEPROFILE: {}", error.message()),
            );
        }
    };

    match delete_profile_in_state(state, token) {
        DeleteOutcome::Deleted => {
            *effect = Some(crate::mock::effect::Effect::ProfilesChanged);
            soap(
                r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
                "<trt:DeleteProfileResponse/>",
            )
        }
        // Media service 24.12 §5.2.22: both refusals are Sender faults.
        DeleteOutcome::NotFound => Fault::new(
            Code::Sender,
            &[INVALID_ARG_VAL, NO_PROFILE],
            &format!("Profile not found: {token}"),
        )
        .to_xml(),
        DeleteOutcome::Fixed => Fault::new(
            Code::Sender,
            &[ACTION, DELETION_OF_FIXED_PROFILE],
            &format!("Cannot delete fixed profile: {token}"),
        )
        .to_xml(),
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
    /// The synthetic model cannot expose a usable typed profile with this identity.
    EmptyToken,
    /// Capacity or initial binding validation refused before any mutation.
    Rejected(String),
}

/// Explicit model limitation, not a claim about the ONVIF xs:string lexical space.
pub(crate) fn empty_profile_token_fault(code: crate::mock::fault::Code) -> String {
    use crate::mock::fault::{Fault, MOCK_REQUEST_POLICY};
    Fault::new(
        code,
        &[MOCK_REQUEST_POLICY],
        "The mock does not support empty profile tokens",
    )
    .to_xml()
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
/// when the caller gives a nonempty one; an empty one is refused by mock policy.
/// Otherwise a token is
/// generated from `next_token_id`, skipping occupied identities. Validation and
/// insertion share one write lock; a refusal does not notify the state hook.
pub(crate) fn create_profile_in_state(
    state: &SharedState,
    name: &str,
    supplied_token: Option<String>,
) -> CreateOutcome {
    create_profile_with_bindings(state, name, supplied_token, &[])
}

pub(crate) fn create_profile_with_bindings(
    state: &SharedState,
    name: &str,
    supplied_token: Option<String>,
    planned: &[(ConfigKind, String)],
) -> CreateOutcome {
    if supplied_token.as_deref() == Some("") {
        return CreateOutcome::EmptyToken;
    }
    state.modify_returning_if(
        |s| {
            // Duplicate identity has precedence over capacity and does not advance
            // the allocation hint. Initial bindings are checked in this same lock.
            if let Some(token) = supplied_token.as_ref()
                && s.profiles.profiles.iter().any(|p| p.token == *token)
            {
                return CreateOutcome::Duplicate(token.clone());
            }
            if s.profiles.profiles.len() >= PROFILE_LIMIT {
                use crate::mock::fault::{ACTION, Code, Fault, MAX_PROFILES};
                return CreateOutcome::Rejected(
                    Fault::new(
                        Code::Receiver,
                        &[ACTION, MAX_PROFILES],
                        "Mock profile capacity reached",
                    )
                    .to_xml(),
                );
            }
            if let Err(fault) = validate_configuration_plan(s, planned, "CREATECFG2") {
                return CreateOutcome::Rejected(fault);
            }
            let token = if let Some(token) = supplied_token {
                if s.profiles.profiles.iter().any(|p| p.token == token) {
                    return CreateOutcome::Duplicate(token);
                }
                token
            } else {
                // There are at most profiles.len() occupied candidates. The wider
                // temporary cannot overflow while probing that many Vec entries,
                // even when a persisted u32 hint starts at its maximum value.
                let mut id = u128::from(s.profiles.next_token_id);
                let token = loop {
                    let candidate = format!("Profile_{id}");
                    if !s.profiles.profiles.iter().any(|p| p.token == candidate) {
                        break candidate;
                    }
                    id += 1;
                };
                s.profiles.next_token_id = (id as u32).wrapping_add(1);
                token
            };
            let mut entry = ProfileEntry {
                token: token.clone(),
                name: name.to_string(),
                fixed: false,
                video_source_config_token: None,
                video_encoder_config_token: None,
                audio_source_config_token: None,
                audio_encoder_config_token: None,
                ptz_config_token: None,
            };
            for (kind, token) in planned {
                *kind.slot(&mut entry) = Some(token.clone());
            }
            eprintln!("    [STATE] profile created: {token} ({name})");
            s.profiles.profiles.push(entry.clone());
            refresh_reference_counts(
                s,
                planned.iter().map(|(kind, token)| (*kind, token.clone())),
            );
            CreateOutcome::Created(entry)
        },
        |outcome| matches!(outcome, CreateOutcome::Created(_)),
    )
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
    pub(crate) const ALL: [Self; 5] = [
        Self::VideoSource,
        Self::VideoEncoder,
        Self::AudioSource,
        Self::AudioEncoder,
        Self::Ptz,
    ];

    fn token(self, p: &ProfileEntry) -> Option<&str> {
        match self {
            Self::VideoSource => p.video_source_config_token.as_deref(),
            Self::VideoEncoder => p.video_encoder_config_token.as_deref(),
            Self::AudioSource => p.audio_source_config_token.as_deref(),
            Self::AudioEncoder => p.audio_encoder_config_token.as_deref(),
            Self::Ptz => p.ptz_config_token.as_deref(),
        }
    }
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

    /// `true` if `DeviceState` carries an entry this token names.
    ///
    /// The audio arms returned `true` unconditionally while the audio families
    /// were static fixtures with no catalogue to check against. `ee7e0b3` gave
    /// them one, and until this was corrected a bogus `AudioSource` or
    /// `AudioEncoder` token bound **silently** where a bogus video or PTZ token
    /// faulted — the profile then rendered no audio at all, with nothing to say
    /// why. A stale justification outliving its premise, which is the class the
    /// 0.15.0 audit was looking for.
    fn known_token(self, s: &crate::mock::state::DeviceState, token: &str) -> bool {
        match self {
            Self::VideoSource => s.video_source_configs.iter().any(|c| c.token == token),
            Self::VideoEncoder => s.video_encoders.iter().any(|c| c.token == token),
            Self::AudioSource => s.audio_source_configs.iter().any(|c| c.token == token),
            Self::AudioEncoder => s.audio_encoders.iter().any(|c| c.token == token),
            Self::Ptz => s.ptz_configs.iter().any(|c| c.token == token),
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
/// A *fixed* profile remains bindable: fixed controls deletion, not configuration
/// immutability (Media1 v24.12 and Media2 v26.06, section 4.1). This is not a mock
/// exception. Preserve it when tightening request validation; compatibility and
/// configuration-conflict checks are separate from the profile's fixed flag.
pub(crate) fn bind_configuration(
    state: &SharedState,
    _body: &str,
    operation: &crate::mock::request::Node,
    kind: ConfigKind,
    tag: &str,
) -> Result<(), String> {
    let profile = operation
        .optional_child_text("http://www.onvif.org/ver10/media/wsdl", "ProfileToken")
        .map_err(|error| error.to_fault())?
        .unwrap_or_default();
    let config = operation
        .optional_child_text(
            "http://www.onvif.org/ver10/media/wsdl",
            "ConfigurationToken",
        )
        .map_err(|error| error.to_fault())?
        .unwrap_or_default();
    apply_configuration_bindings(
        state,
        profile,
        &[(kind, config.to_owned())],
        true,
        None,
        tag,
    )
}

/// Clear a configuration slot on a profile. Audit §3 items 1.5, 1.6 and 1.7.
///
/// Removing a slot that is already empty is **not** a fault — the operation is
/// idempotent and ONVIF does not require a device to complain.
pub(crate) fn unbind_configuration(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    kind: ConfigKind,
    tag: &str,
) -> Result<(), String> {
    let profile = operation
        .optional_child_text("http://www.onvif.org/ver10/media/wsdl", "ProfileToken")
        .map_err(|error| error.to_fault())?
        .unwrap_or_default();
    apply_configuration_bindings(state, profile, &[(kind, String::new())], false, None, tag)
}

/// Validate a complete value-based plan and commit its slots under one lock.
/// All refusals precede mutation. Successful plans notify once, including a
/// successful idempotent removal; this is not a generic rollback mechanism.
pub(crate) fn apply_configuration_bindings(
    state: &SharedState,
    profile: &str,
    planned: &[(ConfigKind, String)],
    add: bool,
    name: Option<&str>,
    tag: &str,
) -> Result<(), String> {
    if profile.is_empty() || (add && planned.iter().any(|(_, token)| token.is_empty())) {
        let required = if add {
            "ProfileToken and ConfigurationToken are both required"
        } else {
            "ProfileToken is required"
        };
        return Err(resp_soap_fault(
            "env:Sender",
            &format!("NoToken-{tag}: {required}"),
        ));
    }
    state.modify_returning_if(
        |s| {
            let Some(index) = s.profiles.profiles.iter().position(|p| p.token == profile) else {
                use crate::mock::fault::{Code, Fault, INVALID_ARG_VAL, NO_PROFILE};
                return Err(Fault::new(
                    Code::Sender,
                    &[INVALID_ARG_VAL, NO_PROFILE],
                    &format!("NoSuchProfile-{tag}: {profile}"),
                )
                .to_xml());
            };
            if add {
                validate_configuration_plan(s, planned, tag)?;
            }
            let mut affected: Vec<_> = planned
                .iter()
                .filter_map(|(kind, _)| {
                    kind.token(&s.profiles.profiles[index])
                        .map(|token| (*kind, token.to_owned()))
                })
                .collect();
            if add {
                affected.extend_from_slice(planned);
            }
            let p = &mut s.profiles.profiles[index];
            if let Some(name) = name {
                p.name = name.to_owned();
            }
            for (kind, token) in planned {
                *kind.slot(p) = add.then(|| token.clone());
            }
            refresh_reference_counts(s, affected);
            eprintln!("    [STATE] profile {profile}: configuration plan committed");
            Ok(())
        },
        Result::is_ok,
    )
}

/// Validate every reference, including overwritten/repeated entries, before commit.
fn validate_configuration_plan(
    state: &crate::mock::state::DeviceState,
    planned: &[(ConfigKind, String)],
    tag: &str,
) -> Result<(), String> {
    use crate::mock::fault::{
        ACTION, CONFIGURATION_CONFLICT, Code, Fault, INVALID_ARG_VAL, NO_CONFIG,
    };
    for (index, (kind, token)) in planned.iter().enumerate() {
        if !kind.known_token(state, token) {
            return Err(Fault::new(
                Code::Sender,
                &[INVALID_ARG_VAL, NO_CONFIG],
                &format!("NoSuchConfig-{tag}: {token}"),
            )
            .to_xml());
        }
        if planned[..index]
            .iter()
            .any(|(previous_kind, previous_token)| previous_kind == kind && previous_token != token)
        {
            return Err(Fault::new(
                Code::Receiver,
                &[ACTION, CONFIGURATION_CONFLICT],
                "Conflicting configurations for one mock profile slot",
            )
            .to_xml());
        }
    }
    Ok(())
}

/// Recount only touched references, preserving unrelated caller-authored fixture
/// fields. Count in the mutation lock so hooks and all catalogue readers agree.
fn refresh_reference_counts(
    state: &mut crate::mock::state::DeviceState,
    affected: impl IntoIterator<Item = (ConfigKind, String)>,
) {
    for (kind, token) in affected {
        let count = state
            .profiles
            .profiles
            .iter()
            .filter(|p| kind.token(p) == Some(token.as_str()))
            .count();
        // The public snapshot field is u32; arbitrary oversized caller state is
        // not normalized. Saturation avoids wrapping its diagnostic count.
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        macro_rules! update {
            ($catalogue:expr) => {
                for entry in $catalogue.iter_mut().filter(|entry| entry.token == token) {
                    entry.use_count = count;
                }
            };
        }
        match kind {
            ConfigKind::VideoSource => update!(state.video_source_configs),
            ConfigKind::VideoEncoder => update!(state.video_encoders),
            ConfigKind::AudioSource => update!(state.audio_source_configs),
            ConfigKind::AudioEncoder => update!(state.audio_encoders),
            ConfigKind::Ptz => update!(state.ptz_configs),
        }
    }
}

/// Remove a profile from the shared list, refusing a fixed one.
pub(crate) fn delete_profile_in_state(state: &SharedState, token: &str) -> DeleteOutcome {
    state.modify_returning_if(
        |s| {
            let Some(idx) = s.profiles.profiles.iter().position(|p| p.token == token) else {
                return DeleteOutcome::NotFound;
            };
            if s.profiles.profiles[idx].fixed {
                return DeleteOutcome::Fixed;
            }
            let removed = s.profiles.profiles.remove(idx);
            let affected = ConfigKind::ALL
                .into_iter()
                .filter_map(|kind| kind.token(&removed).map(|token| (kind, token.to_owned())))
                .collect::<Vec<_>>();
            refresh_reference_counts(s, affected);
            eprintln!("    [STATE] profile deleted: {token}");
            DeleteOutcome::Deleted
        },
        |outcome| matches!(outcome, DeleteOutcome::Deleted),
    )
}

// ── Profile render helpers ──────────────────────────────────────────────────
//
// Profiles are rendered with full nested configuration objects (the
// shape real cameras use), and every one of them is read from `DeviceState`
// through `Catalogues` — video source, video encoder, PTZ, audio source and
// audio encoder alike. Until 0.15 the details for VSC_1, VEC_1 and VEC_2 were
// literals here, which is how `VEC_2` came to have three names in three files.

fn render_profile(p: &ProfileEntry, tag: &str, cat: &Catalogues) -> Result<String, String> {
    let vsc = p
        .video_source_config_token
        .as_deref()
        .map(|t| render_vsc_inline(&cat.vscs, t))
        .unwrap_or_default();
    let vec = p
        .video_encoder_config_token
        .as_deref()
        .map(|t| render_vec_inline(&cat.vecs, t))
        .transpose()?
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
        .transpose()?
        .unwrap_or_default();
    // `MediaProfile::ptz_config_token` reads `Profile/PTZConfiguration@token`
    // and nothing ever fed it. Media1 inlines the whole configuration, as it
    // does for every other kind; Media2 also inlines it. The body comes
    // from `ptz::render_config` so the two services cannot drift.
    let ptz = p
        .ptz_config_token
        .as_deref()
        .and_then(|t| cat.ptzs.iter().find(|c| c.token == t))
        .map(|c| super::ptz::render_config(c, "tt:PTZConfiguration"))
        .unwrap_or_default();
    // **Audio source before video encoder.** `tt:Profile`'s sequence is
    // `Name, VideoSourceConfiguration, AudioSourceConfiguration,
    // VideoEncoderConfiguration, AudioEncoderConfiguration, …` — video and
    // audio are *not* grouped by medium, which is the order this emitted until
    // 0.15 and the order every reader assumes. `tr2:ConfigurationSet` happens
    // to declare the same interleaving, but it is a different type in a
    // different schema and each order is derived on its own.
    Ok(format!(
        r#"<trt:{tag} token="{token}" fixed="{fixed}">
          <tt:Name>{name}</tt:Name>
          {vsc}{asc}{vec}{aec}{ptz}
        </trt:{tag}>"#,
        token = crate::types::xml_escape(&p.token),
        fixed = p.fixed,
        name = crate::types::xml_escape(&p.name),
    ))
}

/// Every catalogue a profile can inline, cloned under a **single** read lock.
///
/// One lock rather than five keeps a responder from rendering a profile whose
/// video and audio configurations came from different moments. It was a
/// three-tuple until the audio catalogue joined it.
pub(crate) struct Catalogues {
    pub(crate) vscs: Vec<VideoSourceConfigEntry>,
    pub(crate) vecs: Vec<VideoEncoderState>,
    pub(crate) ptzs: Vec<PtzConfigEntry>,
    pub(crate) ascs: Vec<AudioSourceConfigEntry>,
    pub(crate) aecs: Vec<AudioEncoderEntry>,
}

pub(crate) fn catalogues(state: &SharedState) -> Catalogues {
    let s = state.read();
    catalogues_from_state(&s)
}

/// Capture profile identities and every catalogue they can inline under one
/// read guard. Separate snapshots could join two different committed states.
pub(crate) fn profile_snapshot(state: &SharedState) -> (Vec<ProfileEntry>, Catalogues) {
    let state = state.read();
    (
        state.profiles.profiles.clone(),
        catalogues_from_state(&state),
    )
}

fn catalogues_from_state(s: &crate::mock::state::DeviceState) -> Catalogues {
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
/// `tt:VideoSourceConfiguration` inline in a Media1 profile, `tr2:VideoSource`
/// inline in a Media2 one, `trt:Configurations` in a list, `trt:Configuration`
/// for the singular getter. The *body* is the same in all four because the type
/// is the same: `tr2:ConfigurationSet/VideoSource` is typed
/// `tt:VideoSourceConfiguration`, exactly as `tt:Profile`'s member is.
pub(crate) fn render_vsc_body(c: &VideoSourceConfigEntry, tag: &str) -> String {
    format!(
        r#"<{tag} token="{token}">
          <tt:Name>{name}</tt:Name>
          <tt:UseCount>{use_count}</tt:UseCount>
          <tt:SourceToken>{source}</tt:SourceToken>
          <tt:Bounds x="0" y="0" width="{width}" height="{height}"/>
        </{tag}>"#,
        token = crate::types::xml_escape(&c.token),
        name = crate::types::xml_escape(&c.name),
        use_count = c.use_count,
        source = crate::types::xml_escape(&c.source_token),
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

fn render_vec_inline(vecs: &[VideoEncoderState], token: &str) -> Result<String, String> {
    match vecs.iter().find(|c| c.token == token) {
        Some(c) => render_vec_body(c, "tt:VideoEncoderConfiguration"),
        None => Ok(String::new()),
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
pub(super) fn render_vec_body(c: &VideoEncoderState, tag: &str) -> Result<String, String> {
    super::video_rate::view(c, false)?;
    let codec = if c.encoding == "H264" {
        format!(
            "<tt:H264><tt:GovLength>{gov}</tt:GovLength>\
             <tt:H264Profile>{profile}</tt:H264Profile></tt:H264>",
            gov = c.gov_length,
            profile = crate::types::xml_escape(&c.profile),
        )
    } else {
        String::new()
    };
    Ok(format!(
        r#"<{tag} token="{token}">
          <tt:Name>{name}</tt:Name>
          <tt:UseCount>{use_count}</tt:UseCount>
          <tt:Encoding>{encoding}</tt:Encoding>
          <tt:Resolution><tt:Width>{width}</tt:Width><tt:Height>{height}</tt:Height></tt:Resolution>
          <tt:Quality>{quality}</tt:Quality>
          <tt:RateControl><tt:FrameRateLimit>{fps}</tt:FrameRateLimit><tt:EncodingInterval>1</tt:EncodingInterval><tt:BitrateLimit>{bitrate}</tt:BitrateLimit></tt:RateControl>
          {codec}{VEC_TAIL}
        </{tag}>"#,
        token = crate::types::xml_escape(&c.token),
        name = crate::types::xml_escape(&c.name),
        use_count = c.use_count,
        encoding = crate::types::xml_escape(&c.encoding),
        width = c.width,
        height = c.height,
        quality = c.quality,
        fps = c.frame_rate_limit,
        bitrate = c.bitrate_limit,
    ))
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
        token = crate::types::xml_escape(&c.token),
        name = crate::types::xml_escape(&c.name),
        use_count = c.use_count,
        source = crate::types::xml_escape(&c.source_token),
    )
}

pub(crate) fn render_audio_encoder(c: &AudioEncoderEntry, qname: &str) -> Result<String, String> {
    super::audio_metadata::audio_render(c, qname, false)
}

pub fn resp_video_sources(state: &SharedState, operation: &crate::mock::request::Node) -> String {
    if let Err(fault) = super::video_source::empty(operation, false) {
        return fault;
    }
    let sources = state.read().video_sources.clone();
    let items: String = sources
        .iter()
        .map(|s| {
            format!(
                r#"<trt:VideoSources token="{token}">
            <tt:Framerate>{fps}</tt:Framerate>
            <tt:Resolution><tt:Width>{width}</tt:Width><tt:Height>{height}</tt:Height></tt:Resolution>
          </trt:VideoSources>"#,
                token = crate::types::xml_escape(&s.token),
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

pub fn resp_video_source_configurations(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    if let Err(fault) = super::video_source::empty(operation, false) {
        return fault;
    }
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

pub fn resp_video_encoder_configurations(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::video_encoder::get(state, operation, false, false)
}

pub fn resp_audio_sources(state: &SharedState, operation: &crate::mock::request::Node) -> String {
    super::audio_metadata::get(state, operation, false, "GetAudioSources")
}

pub fn resp_audio_encoder_configurations(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::audio_metadata::get(state, operation, false, "GetAudioEncoderConfigurations")
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

pub fn resp_video_source_configuration(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::video_source::get(state, operation, false)
}

/// `GetVideoSourceConfigurationOptions` — per-channel.
///
/// `BoundsRange` maxima are the addressed **sensor's** own resolution, so the
/// two channels report different ceilings even after cropping. An omitted
/// configuration selector returns conservative generic options, not a guessed
/// channel. Profile context is validated; physical routing conflicts are not modeled.
///
/// **`MaximumNumberOfProfiles` is an attribute.**
/// `tt:VideoSourceConfigurationOptions` declares it as `xs:attribute` and only
/// `BoundsRange`, `VideoSourceTokensAvailable` and `Extension` as child
/// elements. This emitted it as an element until 0.15, agreeing with the client
/// bug it was written beside. Media1 and Media2 return the *same* type here, so
/// `resp_video_source_configuration_options_media2` carries the identical
/// correction.
pub fn resp_video_source_configuration_options(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::video_source::options(state, operation, false)
}

pub fn resp_video_encoder_configuration(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::video_encoder::get(state, operation, false, true)
}

pub fn resp_video_encoder_configuration_options(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::video_encoder::options(state, operation, false)
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
///
/// **`PositionOption` is a repeated plain string, not a wrapper.**
/// `tt:OSDConfigurationOptions` declares it `type="xs:string"
/// maxOccurs="unbounded"`, so a conformant device sends one element per
/// position. This emitted a single `<tt:PositionOption>` holding `<tt:Type>`
/// children until 0.15 — a shape no device produces — because the parser it
/// was written beside read that wrapper and a doc comment called the real
/// shape a Genetec deviation. The shape checker could not see it either: it
/// stops the walk at any child whose type is not a complex type, so the whole
/// invented subtree was skipped rather than judged. Its `SIMPLE-TYPE-KIDS`
/// rule is what reports it now.
pub fn resp_osd_options() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            r#"<trt:GetOSDOptionsResponse>
          <trt:OSDOptions>
            <tt:MaximumNumberOfOSDs Total="{OSD_QUOTA_TOTAL}" Plain="{OSD_QUOTA_PLAIN}" Date="{OSD_QUOTA_DATE}" Time="{OSD_QUOTA_TIME}" DateAndTime="{OSD_QUOTA_DATE_AND_TIME}"/>
            <tt:Type>Text</tt:Type>
            <tt:Type>Image</tt:Type>
            <tt:PositionOption>UpperLeft</tt:PositionOption>
            <tt:PositionOption>UpperRight</tt:PositionOption>
            <tt:PositionOption>LowerLeft</tt:PositionOption>
            <tt:PositionOption>LowerRight</tt:PositionOption>
            <tt:PositionOption>Custom</tt:PositionOption>
            <tt:TextOption>
              <tt:Type>Plain</tt:Type>
              <tt:Type>Date</tt:Type>
              <tt:Type>Time</tt:Type>
              <tt:Type>DateAndTime</tt:Type>
              <tt:FontSizeRange>
                <tt:Min>8</tt:Min>
                <tt:Max>72</tt:Max>
              </tt:FontSizeRange>
              <tt:DateFormat>MM/dd/yyyy</tt:DateFormat>
              <tt:DateFormat>yyyy-MM-dd</tt:DateFormat>
              <tt:DateFormat>dd.MM.yyyy</tt:DateFormat>
              <tt:TimeFormat>HH:mm:ss</tt:TimeFormat>
              <tt:TimeFormat>hh:mm:ss tt</tt:TimeFormat>
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

pub fn resp_audio_source_configurations(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::audio_metadata::get(state, operation, false, "GetAudioSourceConfigurations")
}

pub fn resp_audio_encoder_configuration(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::audio_metadata::get(state, operation, false, "GetAudioEncoderConfiguration")
}

pub fn resp_audio_encoder_configuration_options(
    state: &SharedState,
    operation: &crate::mock::request::Node,
) -> String {
    super::audio_metadata::options(state, operation, false, false)
}

/// One options row. Media1 wraps these in a `trt:Options` container and names
/// them `tt:Options`; Media2 emits them directly as repeated `tr2:Options`.
pub(crate) fn render_audio_option(o: &AudioOptionEntry, qname: &str) -> String {
    let list = |v: &[u32]| {
        v.iter()
            .map(|n| format!("<tt:Items>{n}</tt:Items>"))
            .collect::<String>()
    };
    format!(
        "<{qname}>\
           <tt:Encoding>{enc}</tt:Encoding>\
           <tt:BitrateList>{br}</tt:BitrateList>\
           <tt:SampleRateList>{sr}</tt:SampleRateList>\
         </{qname}>",
        enc = crate::types::xml_escape(&o.encoding),
        br = list(&o.bitrates),
        sr = list(&o.sample_rates),
    )
}

pub fn handle_set_audio_encoder_configuration(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    apply_audio_encoder_write(state, operation, false, effect)
}

pub(crate) fn apply_audio_encoder_write(
    state: &SharedState,
    operation: &crate::mock::request::Node,
    media2: bool,
    effect: &mut Option<crate::mock::effect::Effect>,
) -> String {
    super::audio_metadata::set_audio(state, operation, media2, effect)
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
        &format!(
            r#"<trt:GetServiceCapabilitiesResponse>
          <trt:Capabilities SnapshotUri="true"
                            Rotation="false"
                            VideoSourceMode="false"
                            OSD="true"
                            TemporaryOSDText="false"
                            EXICompression="false">
            <trt:ProfileCapabilities MaximumNumberOfProfiles="{PROFILE_LIMIT}"/>
            <trt:StreamingCapabilities RTPMulticast="false"
                                       RTP_TCP="true"
                                       RTP_RTSP_TCP="true"
                                       NonAggregateControl="false"
                                       NoRTSPStreaming="false"/>
          </trt:Capabilities>
        </trt:GetServiceCapabilitiesResponse>"#
        ),
    )
}

#[cfg(test)]
mod profile_allocation_tests {
    use super::*;
    use crate::mock::state::MockState;
    use std::sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn allocation_crosses_counter_boundary_without_collision_or_panic() {
        let state = MockState::new();
        state.modify(|s| {
            s.profiles.next_token_id = u32::MAX;
            s.profiles.profiles[0].token = "Profile_4294967295".into();
            s.profiles.profiles[1].token = "Profile_4294967296".into();
        });
        let before = state.read().clone();
        let CreateOutcome::Created(entry) = create_profile_in_state(&state, "boundary-821", None)
        else {
            panic!("a free generated identity must be allocated");
        };
        assert_eq!(entry.token, "Profile_4294967297");
        assert_eq!(entry.name, "boundary-821");
        let mut expected = before;
        expected.profiles.next_token_id = 2;
        expected.profiles.profiles.push(entry);
        assert_eq!(
            serde_json::to_value(&*state.read()).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }

    #[test]
    fn duplicate_creation_preserves_every_state_field_and_skips_notification() {
        let mut state = MockState::new();
        let notifications = Arc::new(AtomicUsize::new(0));
        let capture = notifications.clone();
        state.set_on_change(Arc::new(move |_| {
            capture.fetch_add(1, Ordering::SeqCst);
        }));
        let before = serde_json::to_value(&*state.read()).unwrap();
        let token = state.read().profiles.profiles[0].token.clone();
        match create_profile_in_state(&state, "must-not-overwrite-822", Some(token.clone())) {
            CreateOutcome::Duplicate(actual) => assert_eq!(actual, token),
            CreateOutcome::Created(entry) => panic!("duplicate created: {}", entry.token),
            CreateOutcome::EmptyToken => panic!("nonempty duplicate rejected as empty"),
            CreateOutcome::Rejected(fault) => panic!("unexpected refusal: {fault}"),
        }
        assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
        assert_eq!(notifications.load(Ordering::SeqCst), 0);
        let CreateOutcome::Created(entry) =
            create_profile_in_state(&state, "new-823", Some("free-823".into()))
        else {
            panic!("fresh explicit token was refused");
        };
        assert_eq!(entry.token, "free-823");
        assert_eq!(entry.name, "new-823");
        assert_eq!(notifications.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn concurrent_creation_serializes_explicit_and_generated_identity_checks() {
        for supplied in [None, Some("shared-824".to_owned())] {
            let mut state = MockState::new();
            let notifications = Arc::new(AtomicUsize::new(0));
            let capture = notifications.clone();
            state.set_on_change(Arc::new(move |_| {
                capture.fetch_add(1, Ordering::SeqCst);
            }));
            let initial = state.read().profiles.profiles.len();
            let barrier = Arc::new(Barrier::new(8));
            let results = std::thread::scope(|scope| {
                let handles: Vec<_> = (0..8)
                    .map(|_| {
                        let state = &state;
                        let barrier = barrier.clone();
                        let supplied = supplied.clone();
                        scope.spawn(move || {
                            barrier.wait();
                            create_profile_in_state(state, "concurrent-824", supplied)
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|h| h.join().unwrap())
                    .collect::<Vec<_>>()
            });
            let mut created = std::collections::BTreeSet::new();
            let mut duplicates = 0;
            let mut full = 0;
            for result in results {
                match result {
                    CreateOutcome::Created(entry) => {
                        assert_eq!(entry.name, "concurrent-824");
                        assert!(created.insert(entry.token), "duplicate success identity");
                    }
                    CreateOutcome::Duplicate(token) => {
                        assert_eq!(Some(token), supplied);
                        duplicates += 1;
                    }
                    CreateOutcome::EmptyToken => {
                        panic!("nonempty/generated token rejected as empty")
                    }
                    CreateOutcome::Rejected(fault) => {
                        assert!(fault.contains("ter:MaxNVTProfiles"), "{fault}");
                        full += 1;
                    }
                }
            }
            let expected = if supplied.is_some() {
                1
            } else {
                PROFILE_LIMIT - initial
            };
            assert_eq!(created.len(), expected);
            assert_eq!(duplicates, if supplied.is_some() { 7 } else { 0 });
            assert_eq!(full, if supplied.is_some() { 0 } else { 8 - expected });
            assert_eq!(notifications.load(Ordering::SeqCst), expected);
            let snapshot = state.read();
            assert_eq!(snapshot.profiles.profiles.len(), initial + expected);
            for token in created {
                assert_eq!(
                    snapshot
                        .profiles
                        .profiles
                        .iter()
                        .filter(|p| p.token == token)
                        .count(),
                    1
                );
            }
        }
    }
}
