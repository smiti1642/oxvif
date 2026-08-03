use crate::mock::helpers::{resp_empty, resp_soap_fault, soap};
use crate::mock::state::{
    PtzConfigEntry, PtzNodeEntry, PtzPreset, PtzTour, PtzTourSpot, SharedState, SpaceEntry,
    SpaceKind,
};
use crate::mock::xml_parse::{extract_all_tags, extract_attr, extract_tag};

const NS: &str = r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#;
const POS_SPACE: &str = "http://www.onvif.org/ver10/tptz/PanTiltSpaces/PositionGenericSpace";
const ZOOM_SPACE: &str = "http://www.onvif.org/ver10/tptz/ZoomSpaces/PositionGenericSpace";

fn clamp(v: f32, min: f32, max: f32) -> f32 {
    v.max(min).min(max)
}

/// The `ProfileToken` of a PTZ request, checked against the device's profile
/// list, or a rendered SOAP Fault.
///
/// **Every PTZ operation that moves or reads a head is per-profile.** Until 0.15
/// none of them looked: 26 of the 27 dispatch arms did not even receive the
/// request body, while the client sent `ProfileToken` at 20 call sites. A test
/// asserting "my code addressed the right head" passed against a mock that
/// could not tell one head from another. `docs/active/mock-audit-2026-07.md` §4.1.
///
/// An absent or unknown token faults rather than falling back to a default
/// profile, for the reason `CLAUDE.md` gives about token-less per-channel
/// queries: an answer for *some* head is indistinguishable from the right one on
/// a single-head device, and wrong on every other.
fn require_profile(state: &SharedState, body: &str, tag: &str) -> Result<String, String> {
    let Some(token) = extract_tag(body, "ProfileToken").filter(|t| !t.is_empty()) else {
        return Err(resp_soap_fault(
            "env:Sender",
            &format!("NoProfileToken-{tag}: every PTZ operation is per-profile"),
        ));
    };
    if !state
        .read()
        .profiles
        .profiles
        .iter()
        .any(|p| p.token == token)
    {
        return Err(resp_soap_fault(
            "ter:NoProfile",
            &format!("NoSuchProfile-{tag}: {token}"),
        ));
    }
    Ok(token)
}

/// The **PTZ node** a request's `ProfileToken` addresses, or a rendered fault.
///
/// ```text
/// ProfileToken → ProfileEntry.ptz_config_token → PtzConfigEntry.node_token
/// ```
///
/// This chain is why `PtzState` is keyed by node. A profile does not own a
/// head; it *references* one through a PTZ configuration, and two profiles over
/// one lens — a main and a sub stream — reference the same one. Keying the
/// state by profile made them two independent heads, so moving the main stream
/// left the sub stream's reported position where it was. No camera does that,
/// and every test written against it was asserting the mock's mistake.
///
/// Three ways to fail, each with its own fault:
///
/// | Failure | Code |
/// |---|---|
/// | no `ProfileToken` in the request | `env:Sender` / `NoProfileToken-…` |
/// | token names no profile | `ter:NoProfile` / `NoSuchProfile-…` |
/// | the profile has no PTZ configuration | `ter:NoConfig` / `NoPTZConfig-…-5619` |
///
/// The third is the new one. A profile with no PTZ configuration is not
/// PTZ-capable at all, so there is no head to answer for; the previous
/// behaviour — answering for an empty channel invented on the spot — told a
/// caller their profile supports PTZ when it does not.
fn require_head(state: &SharedState, body: &str, tag: &str) -> Result<String, String> {
    let token = require_profile(state, body, tag)?;
    let s = state.read();
    let Some(profile) = s.profiles.profiles.iter().find(|p| p.token == token) else {
        // `require_profile` just checked this; unreachable unless the state
        // changed between the two reads.
        return Err(resp_soap_fault(
            "ter:NoProfile",
            &format!("NoSuchProfile-{tag}: {token}"),
        ));
    };
    let Some(config_token) = profile.ptz_config_token.clone() else {
        return Err(resp_soap_fault(
            "ter:NoConfig",
            &format!(
                "NoPTZConfig-{tag}-5619: profile {token} has no PTZ configuration, \
                 so it addresses no PTZ node"
            ),
        ));
    };
    match s.ptz_configs.iter().find(|c| c.token == config_token) {
        Some(c) => Ok(c.node_token.clone()),
        None => Err(resp_soap_fault(
            "ter:NoConfig",
            &format!(
                "NoPTZConfig-{tag}-5619: profile {token} names PTZ configuration \
                 {config_token}, which this device does not have"
            ),
        )),
    }
}

/// `require_head`, but returning early from the handler with the fault.
macro_rules! head {
    ($state:expr, $body:expr, $tag:literal) => {
        match require_head($state, $body, $tag) {
            Ok(t) => t,
            Err(fault) => return fault,
        }
    };
}

/// `PTZStatus/UtcTime` — **the real current time**, not a fixed date.
///
/// It was the literal `2026-04-23T00:00:00Z` until 0.15: the same defect as the
/// hardcoded `2026-04-15` in `GetSystemDateAndTime` (see
/// `device::resp_system_date_and_time`), and missed by the same fix because the
/// two clocks were written in different files. A caller that trusts
/// `PTZStatus/UtcTime` — to age a position sample, or to check it against the
/// device's own clock — got an answer that drifted a day further into the past
/// with every day that passed, and nothing failed.
///
/// The conversion is `soap::security::unix_secs_to_iso8601`, the same one
/// `GetSystemDateAndTime` and the WS-Security `Created` header use, so the two
/// clocks now agree by construction rather than by coincidence.
fn now_iso8601() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    crate::soap::security::unix_secs_to_iso8601(now as i64)
}

pub fn resp_ptz_status(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "STATUS-5601");
    let snapshot = state.read().ptz.channel(&head).cloned().unwrap_or_default();
    let p = &snapshot;
    let utc = now_iso8601();
    soap(
        NS,
        &format!(
            r#"<tptz:GetStatusResponse>
          <tptz:PTZStatus>
            <tt:Position>
              <tt:PanTilt x="{pan}" y="{tilt}" space="{POS_SPACE}"/>
              <tt:Zoom x="{zoom}" space="{ZOOM_SPACE}"/>
            </tt:Position>
            <tt:MoveStatus>
              <tt:PanTilt>IDLE</tt:PanTilt>
              <tt:Zoom>IDLE</tt:Zoom>
            </tt:MoveStatus>
            <tt:UtcTime>{utc}</tt:UtcTime>
          </tptz:PTZStatus>
        </tptz:GetStatusResponse>"#,
            pan = p.pan,
            tilt = p.tilt,
            zoom = p.zoom,
        ),
    )
}

pub fn resp_ptz_presets(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "PRESETS-5602");
    let presets = state
        .read()
        .ptz
        .channel(&head)
        .map(|c| c.presets.clone())
        .unwrap_or_default();
    let items: String = presets
        .iter()
        .map(|p| {
            format!(
                r#"<tptz:Preset token="{token}">
              <tt:Name>{name}</tt:Name>
              <tt:PTZPosition>
                <tt:PanTilt x="{pan}" y="{tilt}" space="{POS_SPACE}"/>
                <tt:Zoom x="{zoom}" space="{ZOOM_SPACE}"/>
              </tt:PTZPosition>
            </tptz:Preset>"#,
                token = p.token,
                name = p.name,
                pan = p.pan,
                tilt = p.tilt,
                zoom = p.zoom,
            )
        })
        .collect();
    soap(
        NS,
        &format!("<tptz:GetPresetsResponse>{items}</tptz:GetPresetsResponse>"),
    )
}

/// Pick the next free `Preset_<n>` token.
fn next_preset_token(presets: &[PtzPreset]) -> String {
    let used: std::collections::HashSet<u32> = presets
        .iter()
        .filter_map(|p| p.token.strip_prefix("Preset_").and_then(|n| n.parse().ok()))
        .collect();
    (1..)
        .find(|n| !used.contains(n))
        .map(|n| format!("Preset_{n}"))
        .unwrap()
}

pub fn handle_ptz_set_preset(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "SETPRESET-5603");
    let inner = extract_tag(body, "SetPreset").unwrap_or_default();
    let name = extract_tag(&inner, "PresetName");
    let token_in = extract_tag(&inner, "PresetToken");

    let token = state.modify_returning(|s| {
        let ch = s.ptz.channel_mut(&head);
        let pos = (ch.pan, ch.tilt, ch.zoom);
        if let Some(t) = token_in {
            if let Some(p) = ch.presets.iter_mut().find(|p| p.token == t) {
                if let Some(n) = name {
                    p.name = n;
                }
                p.pan = pos.0;
                p.tilt = pos.1;
                p.zoom = pos.2;
                eprintln!("    [STATE] {head}: preset updated: {t}");
                return t;
            }
            // Token specified but not found — fall through to create with that token.
            eprintln!("    [STATE] {head}: preset created with client-supplied token: {t}");
            ch.presets.push(PtzPreset {
                token: t.clone(),
                name: name.unwrap_or_else(|| t.clone()),
                pan: pos.0,
                tilt: pos.1,
                zoom: pos.2,
            });
            return t;
        }
        let new_token = next_preset_token(&ch.presets);
        eprintln!("    [STATE] {head}: preset created: {new_token}");
        ch.presets.push(PtzPreset {
            token: new_token.clone(),
            name: name.unwrap_or_else(|| new_token.clone()),
            pan: pos.0,
            tilt: pos.1,
            zoom: pos.2,
        });
        new_token
    });

    soap(
        NS,
        &format!(
            r#"<tptz:SetPresetResponse>
              <tptz:PresetToken>{token}</tptz:PresetToken>
            </tptz:SetPresetResponse>"#
        ),
    )
}

pub fn handle_ptz_remove_preset(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "RMPRESET-5604");
    let inner = extract_tag(body, "RemovePreset").unwrap_or_default();
    if let Some(token) = extract_tag(&inner, "PresetToken") {
        state.modify(|s| {
            s.ptz
                .channel_mut(&head)
                .presets
                .retain(|p| p.token != token);
            eprintln!("    [STATE] {head}: preset removed: {token}");
        });
    }
    resp_empty("tptz", "RemovePresetResponse")
}

pub fn handle_ptz_goto_preset(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "GOTOPRESET-5605");
    let inner = extract_tag(body, "GotoPreset").unwrap_or_default();
    if let Some(token) = extract_tag(&inner, "PresetToken") {
        state.modify(|s| {
            let ch = s.ptz.channel_mut(&head);
            if let Some(p) = ch.presets.iter().find(|p| p.token == token) {
                let (pan, tilt, zoom) = (p.pan, p.tilt, p.zoom);
                ch.pan = pan;
                ch.tilt = tilt;
                ch.zoom = zoom;
                eprintln!("    [STATE] {head}: goto preset: {token}");
            }
        });
    }
    resp_empty("tptz", "GotoPresetResponse")
}

/// Does `head` declare the space slot this move needs?
///
/// A head with no `AbsolutePanTiltPositionSpace` cannot honour an
/// `AbsoluteMove` carrying a `PanTilt` vector — it is a zoom-only head. The
/// mock refuses rather than storing a pan and tilt it has just told the caller,
/// through `GetNodes`, that it does not have. Answering `Ok` would let a caller
/// ship code that works here and silently does nothing on the hardware.
///
/// **The test is whether the vector is present, not whether it is zero.**
/// `<tt:PanTilt x="0" y="0"/>` is still a request to point the head, and a
/// device that cannot point is not entitled to call that a no-op success.
fn reject_pan_tilt(state: &SharedState, head: &str, kind: SpaceKind, tag: &str) -> Option<String> {
    let supported = state
        .read()
        .ptz_nodes
        .iter()
        .find(|n| n.token == head)
        .is_some_and(|n| n.supports(kind));
    if supported {
        return None;
    }
    Some(resp_soap_fault(
        "ter:InvalidArgVal",
        &format!(
            "NoPanTiltSpace-{tag}: node {head} declares no {}; it is a zoom-only \
             head and cannot honour a PanTilt vector",
            kind.element()
        ),
    ))
}

/// Was a `PanTilt` vector present at all? Either attribute is enough — a
/// `PanTilt` element carrying only one of them is malformed, not absent.
fn has_pan_tilt(inner: &str) -> bool {
    extract_attr(inner, "PanTilt", "x").is_some() || extract_attr(inner, "PanTilt", "y").is_some()
}

pub fn handle_ptz_absolute_move(state: &SharedState, body: &str) -> String {
    // <tptz:AbsoluteMove><tptz:Position><tt:PanTilt x=.. y=../><tt:Zoom x=../></tptz:Position>...
    let head = head!(state, body, "ABSMOVE-5606");
    let inner = extract_tag(body, "Position").unwrap_or_default();
    if has_pan_tilt(&inner)
        && let Some(fault) = reject_pan_tilt(
            state,
            &head,
            SpaceKind::AbsolutePanTiltPosition,
            "ABSMOVE-5620",
        )
    {
        return fault;
    }
    let pan = extract_attr(&inner, "PanTilt", "x").and_then(|v| v.parse::<f32>().ok());
    let tilt = extract_attr(&inner, "PanTilt", "y").and_then(|v| v.parse::<f32>().ok());
    let zoom = extract_attr(&inner, "Zoom", "x").and_then(|v| v.parse::<f32>().ok());
    state.modify(|s| {
        let ch = s.ptz.channel_mut(&head);
        if let Some(v) = pan {
            ch.pan = clamp(v, -1.0, 1.0);
        }
        if let Some(v) = tilt {
            ch.tilt = clamp(v, -1.0, 1.0);
        }
        if let Some(v) = zoom {
            ch.zoom = clamp(v, 0.0, 1.0);
        }
        eprintln!(
            "    [STATE] {head}: PTZ absolute → ({:.2}, {:.2}, {:.2})",
            ch.pan, ch.tilt, ch.zoom
        );
    });
    resp_empty("tptz", "AbsoluteMoveResponse")
}

pub fn handle_ptz_relative_move(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "RELMOVE-5607");
    let inner = extract_tag(body, "Translation").unwrap_or_default();
    if has_pan_tilt(&inner)
        && let Some(fault) = reject_pan_tilt(
            state,
            &head,
            SpaceKind::RelativePanTiltTranslation,
            "RELMOVE-5621",
        )
    {
        return fault;
    }
    let dpan = extract_attr(&inner, "PanTilt", "x")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.0);
    let dtilt = extract_attr(&inner, "PanTilt", "y")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.0);
    let dzoom = extract_attr(&inner, "Zoom", "x")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.0);
    state.modify(|s| {
        let ch = s.ptz.channel_mut(&head);
        ch.pan = clamp(ch.pan + dpan, -1.0, 1.0);
        ch.tilt = clamp(ch.tilt + dtilt, -1.0, 1.0);
        ch.zoom = clamp(ch.zoom + dzoom, 0.0, 1.0);
        eprintln!(
            "    [STATE] {head}: PTZ relative → ({:.2}, {:.2}, {:.2})",
            ch.pan, ch.tilt, ch.zoom
        );
    });
    resp_empty("tptz", "RelativeMoveResponse")
}

/// ContinuousMove updates state by a small step in the velocity direction
/// — enough that GetStatus right after Move shows movement, without
/// requiring the mock to actually run a timer.
pub fn handle_ptz_continuous_move(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "CONTMOVE-5608");
    let inner = extract_tag(body, "Velocity").unwrap_or_default();
    if has_pan_tilt(&inner)
        && let Some(fault) = reject_pan_tilt(
            state,
            &head,
            SpaceKind::ContinuousPanTiltVelocity,
            "CONTMOVE-5622",
        )
    {
        return fault;
    }
    let vpan = extract_attr(&inner, "PanTilt", "x")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.0);
    let vtilt = extract_attr(&inner, "PanTilt", "y")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.0);
    let vzoom = extract_attr(&inner, "Zoom", "x")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.0);
    let step = 0.05;
    state.modify(|s| {
        let ch = s.ptz.channel_mut(&head);
        ch.pan = clamp(ch.pan + vpan * step, -1.0, 1.0);
        ch.tilt = clamp(ch.tilt + vtilt * step, -1.0, 1.0);
        ch.zoom = clamp(ch.zoom + vzoom * step, 0.0, 1.0);
    });
    resp_empty("tptz", "ContinuousMoveResponse")
}

/// Nothing is moving in the mock, so there is no motion to stop and no state to
/// write. The profile token is still validated: `Stop` names a head like every
/// other PTZ operation, and a mock that accepts a token it does not have lets a
/// caller ship code that only works here.
pub fn handle_ptz_stop(state: &SharedState, body: &str) -> String {
    let _head = head!(state, body, "STOP-5609");
    resp_empty("tptz", "StopResponse")
}

pub fn handle_ptz_goto_home_position(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "GOTOHOME-5610");
    state.modify(|s| {
        let ch = s.ptz.channel_mut(&head);
        ch.pan = ch.home_pan;
        ch.tilt = ch.home_tilt;
        ch.zoom = ch.home_zoom;
        eprintln!("    [STATE] {head}: PTZ goto home");
    });
    resp_empty("tptz", "GotoHomePositionResponse")
}

pub fn handle_ptz_set_home_position(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "SETHOME-5611");
    state.modify(|s| {
        let ch = s.ptz.channel_mut(&head);
        ch.home_pan = ch.pan;
        ch.home_tilt = ch.tilt;
        ch.home_zoom = ch.zoom;
        eprintln!(
            "    [STATE] {head}: PTZ set home → ({:.2}, {:.2}, {:.2})",
            ch.home_pan, ch.home_tilt, ch.home_zoom
        );
    });
    resp_empty("tptz", "SetHomePositionResponse")
}

// ── Nodes, configurations and coordinate spaces ─────────────────────────────
//
// All six responders below were string literals whose handlers did not receive
// the request body, so `GetNode` and `GetConfiguration` answered for the same
// single node and configuration whatever token was asked about, and
// `SupportedPTZSpaces` was sent as an empty element — schema-valid, and a claim
// that the head supports no coordinate space at all, which contradicted the
// same node's `HomeSupported=true` and the mock accepting `AbsoluteMove`.
// `docs/active/ptz-wiring-plan-2026-07.md` §2.1.

/// One `tt:PTZSpaces` slot. `Space2DDescription` when `y_range` is set,
/// `Space1DDescription` when it is not — the schema fixes which per slot.
fn render_space(s: &SpaceEntry) -> String {
    let y = match s.y_range {
        Some((min, max)) => {
            format!("<tt:YRange><tt:Min>{min}</tt:Min><tt:Max>{max}</tt:Max></tt:YRange>")
        }
        None => String::new(),
    };
    format!(
        "<tt:{el}>\
           <tt:URI>{uri}</tt:URI>\
           <tt:XRange><tt:Min>{xmin}</tt:Min><tt:Max>{xmax}</tt:Max></tt:XRange>\
           {y}\
         </tt:{el}>",
        el = s.kind.element(),
        uri = s.uri,
        xmin = s.x_range.0,
        xmax = s.x_range.1,
    )
}

/// `PanTiltLimits` / `ZoomLimits` — a `tt:Range` wrapper, not a spaces slot, so
/// the element name comes from the caller rather than from `SpaceKind`.
fn render_limits(s: &SpaceEntry, tag: &str) -> String {
    let y = match s.y_range {
        Some((min, max)) => {
            format!("<tt:YRange><tt:Min>{min}</tt:Min><tt:Max>{max}</tt:Max></tt:YRange>")
        }
        None => String::new(),
    };
    format!(
        "<tt:{tag}><tt:Range>\
           <tt:URI>{uri}</tt:URI>\
           <tt:XRange><tt:Min>{xmin}</tt:Min><tt:Max>{xmax}</tt:Max></tt:XRange>\
           {y}\
         </tt:Range></tt:{tag}>",
        uri = s.uri,
        xmin = s.x_range.0,
        xmax = s.x_range.1,
    )
}

fn render_node(n: &PtzNodeEntry, tag: &str) -> String {
    // `tt:PTZSpaces` is an xs:sequence, so the slots go out in schema order
    // rather than in whatever order the state happens to hold them.
    let spaces: String = SpaceKind::ALL
        .iter()
        .flat_map(|k| {
            n.pan_tilt_spaces
                .iter()
                .chain(n.zoom_spaces.iter())
                .filter(move |s| s.kind == *k)
        })
        .map(render_space)
        .collect();
    let aux: String = n
        .aux_commands
        .iter()
        .map(|c| format!("<tt:AuxiliaryCommands>{c}</tt:AuxiliaryCommands>"))
        .collect();
    format!(
        r#"<tptz:{tag} token="{token}" FixedHomePosition="{fixed}">
          <tt:Name>{name}</tt:Name>
          <tt:SupportedPTZSpaces>{spaces}</tt:SupportedPTZSpaces>
          <tt:MaximumNumberOfPresets>{presets}</tt:MaximumNumberOfPresets>
          <tt:HomeSupported>{home}</tt:HomeSupported>
          {aux}
        </tptz:{tag}>"#,
        token = n.token,
        fixed = n.fixed_home_position,
        name = n.name,
        presets = n.max_presets,
        home = n.home_supported,
    )
}

/// A `tt:PTZConfiguration` body, in `onvif.xsd` sequence order.
///
/// The absolute pan/tilt space element is `DefaultAbsolutePantTiltPositionSpace`
/// — `Pant`, double `t`. That is ONVIF's own typo and it is normative; oxvif's
/// parser reads both spellings and writes this one.
pub(crate) fn render_config(c: &PtzConfigEntry, qname: &str) -> String {
    let opt = |v: &Option<String>, tag: &str| match v {
        Some(s) => format!("<tt:{tag}>{s}</tt:{tag}>"),
        None => String::new(),
    };
    let speed = match (c.default_speed_pan_tilt, c.default_speed_zoom) {
        (None, None) => String::new(),
        (pt, z) => {
            let pt = pt
                .map(|(x, y)| format!("<tt:PanTilt x=\"{x}\" y=\"{y}\"/>"))
                .unwrap_or_default();
            let z = z
                .map(|x| format!("<tt:Zoom x=\"{x}\"/>"))
                .unwrap_or_default();
            format!("<tt:DefaultPTZSpeed>{pt}{z}</tt:DefaultPTZSpeed>")
        }
    };
    format!(
        r#"<{qname} token="{token}">
          <tt:Name>{name}</tt:Name>
          <tt:UseCount>{use_count}</tt:UseCount>
          <tt:NodeToken>{node}</tt:NodeToken>
          {abs_pt}{abs_z}{rel_pt}{rel_z}{cont_pt}{cont_z}{speed}{timeout}{pt_lim}{z_lim}
        </{qname}>"#,
        token = c.token,
        name = c.name,
        use_count = c.use_count,
        node = c.node_token,
        abs_pt = opt(
            &c.abs_pan_tilt_space,
            "DefaultAbsolutePantTiltPositionSpace"
        ),
        abs_z = opt(&c.abs_zoom_space, "DefaultAbsoluteZoomPositionSpace"),
        rel_pt = opt(
            &c.rel_pan_tilt_space,
            "DefaultRelativePanTiltTranslationSpace"
        ),
        rel_z = opt(&c.rel_zoom_space, "DefaultRelativeZoomTranslationSpace"),
        cont_pt = opt(
            &c.cont_pan_tilt_space,
            "DefaultContinuousPanTiltVelocitySpace"
        ),
        cont_z = opt(&c.cont_zoom_space, "DefaultContinuousZoomVelocitySpace"),
        timeout = opt(&c.default_ptz_timeout, "DefaultPTZTimeout"),
        pt_lim = c
            .pan_tilt_limits
            .as_ref()
            .map(|s| render_limits(s, "PanTiltLimits"))
            .unwrap_or_default(),
        z_lim = c
            .zoom_limits
            .as_ref()
            .map(|s| render_limits(s, "ZoomLimits"))
            .unwrap_or_default(),
    )
}

pub fn resp_ptz_nodes(state: &SharedState) -> String {
    let nodes = state.read().ptz_nodes.clone();
    let items: String = nodes.iter().map(|n| render_node(n, "PTZNode")).collect();
    soap(
        NS,
        &format!("<tptz:GetNodesResponse>{items}</tptz:GetNodesResponse>"),
    )
}

pub fn resp_ptz_node(state: &SharedState, body: &str) -> String {
    let Some(token) = extract_tag(body, "NodeToken").filter(|t| !t.is_empty()) else {
        return resp_soap_fault(
            "env:Sender",
            "NoNodeToken-GETNODE-5615: GetNode names one head",
        );
    };
    let Some(node) = state
        .read()
        .ptz_nodes
        .iter()
        .find(|n| n.token == token)
        .cloned()
    else {
        return resp_soap_fault("ter:NoEntity", &format!("NoSuchNode-GETNODE-5616: {token}"));
    };
    soap(
        NS,
        &format!(
            "<tptz:GetNodeResponse>{}</tptz:GetNodeResponse>",
            render_node(&node, "PTZNode")
        ),
    )
}

pub fn resp_ptz_configurations(state: &SharedState) -> String {
    let configs = state.read().ptz_configs.clone();
    let items: String = configs
        .iter()
        .map(|c| render_config(c, "tptz:PTZConfiguration"))
        .collect();
    soap(
        NS,
        &format!("<tptz:GetConfigurationsResponse>{items}</tptz:GetConfigurationsResponse>"),
    )
}

pub fn resp_ptz_configuration(state: &SharedState, body: &str) -> String {
    let Some(token) = extract_tag(body, "PTZConfigurationToken").filter(|t| !t.is_empty()) else {
        return resp_soap_fault(
            "env:Sender",
            "NoConfigToken-GETCFG-5617: GetConfiguration names one configuration",
        );
    };
    let Some(cfg) = state
        .read()
        .ptz_configs
        .iter()
        .find(|c| c.token == token)
        .cloned()
    else {
        return resp_soap_fault(
            "ter:NoConfig",
            &format!("NoSuchPTZConfig-GETCFG-5618: {token}"),
        );
    };
    soap(
        NS,
        &format!(
            "<tptz:GetConfigurationResponse>{}</tptz:GetConfigurationResponse>",
            render_config(&cfg, "tptz:PTZConfiguration")
        ),
    )
}

/// `GetCompatibleConfigurations` — the configurations that can be added to the
/// named profile.
///
/// **This is the one per-profile PTZ operation that must not fault on a profile
/// with no PTZ configuration.** It asks "what is compatible", and an empty
/// answer is exactly how a client learns a profile is not PTZ-capable; faulting
/// would force every caller to treat a normal condition as an error. Same
/// distinction `docs/mock-server.md` §7.3 draws between a token that *filters*
/// a list and a token that *addresses* an entity. An absent or unknown
/// `ProfileToken` still faults — that is a malformed request either way.
pub fn resp_ptz_compatible_configurations(state: &SharedState, body: &str) -> String {
    let profile = match require_profile(state, body, "COMPATCFG-5614") {
        Ok(t) => t,
        Err(fault) => return fault,
    };
    let s = state.read();
    let bound = s
        .profiles
        .profiles
        .iter()
        .find(|p| p.token == profile)
        .and_then(|p| p.ptz_config_token.clone());
    let items: String = match bound {
        Some(t) => s
            .ptz_configs
            .iter()
            .filter(|c| c.token == t)
            .map(|c| render_config(c, "tptz:PTZConfiguration"))
            .collect(),
        None => String::new(),
    };
    soap(
        NS,
        &format!(
            "<tptz:GetCompatibleConfigurationsResponse>{items}\
             </tptz:GetCompatibleConfigurationsResponse>"
        ),
    )
}

/// `GetConfigurationOptions` is **per configuration**, and the two
/// configurations answer differently (`PT1S`–`PT60S` against `PT5S`–`PT30S`).
/// It was one static pair for the whole device, so a caller that passed the
/// wrong token got a plausible answer and no way to notice.
pub fn resp_ptz_configuration_options(state: &SharedState, body: &str) -> String {
    let Some(token) = extract_tag(body, "ConfigurationToken").filter(|t| !t.is_empty()) else {
        return resp_soap_fault(
            "env:Sender",
            "NoConfigToken-CFGOPTS-5612: GetConfigurationOptions is per configuration",
        );
    };
    let Some(cfg) = state
        .read()
        .ptz_configs
        .iter()
        .find(|c| c.token == token)
        .cloned()
    else {
        return resp_soap_fault(
            "ter:NoConfig",
            &format!("NoSuchPTZConfig-CFGOPTS-5613: {token}"),
        );
    };
    soap(
        NS,
        &format!(
            r#"<tptz:GetConfigurationOptionsResponse>
          <tptz:PTZConfigurationOptions>
            <tt:PTZTimeout>
              <tt:Min>{min}</tt:Min>
              <tt:Max>{max}</tt:Max>
            </tt:PTZTimeout>
          </tptz:PTZConfigurationOptions>
        </tptz:GetConfigurationOptionsResponse>"#,
            min = cfg.timeout_min,
            max = cfg.timeout_max,
        ),
    )
}

// ── SetConfiguration ─────────────────────────────────────────────────────────

/// The `<tt:Min>` / `<tt:Max>` pair inside an `XRange` or a `YRange`.
fn min_max(inner: &str) -> Option<(f32, f32)> {
    let min = extract_tag(inner, "Min")?.parse().ok()?;
    let max = extract_tag(inner, "Max")?.parse().ok()?;
    Some((min, max))
}

/// A `PanTiltLimits` / `ZoomLimits` element, or `None` if the request omitted it.
///
/// `kind` is not on the wire: a limit wraps a bare `tt:Range`, and which space it
/// constrains is implied by which limit it is. It is stored anyway so a
/// [`SpaceEntry`] read back out of state is still self-describing.
fn parse_limits(cfg: &str, tag: &str, kind: SpaceKind) -> Option<SpaceEntry> {
    let range = extract_tag(&extract_tag(cfg, tag)?, "Range")?;
    Some(SpaceEntry {
        kind,
        uri: extract_tag(&range, "URI")?,
        x_range: min_max(&extract_tag(&range, "XRange")?)?,
        y_range: extract_tag(&range, "YRange").and_then(|y| min_max(&y)),
    })
}

/// The `x` / `y` attributes of a `tt:PanTilt` vector.
fn pan_tilt_attrs(xml: &str) -> Option<(f32, f32)> {
    let x = extract_attr(xml, "PanTilt", "x")?.parse().ok()?;
    let y = extract_attr(xml, "PanTilt", "y")?.parse().ok()?;
    Some((x, y))
}

/// `tptz:SetConfiguration` — persist what the client sent.
///
/// The body was discarded entirely until now (`resp_empty` in the dispatcher):
/// the call reported success and `GetConfiguration` went on answering the
/// fixture, so a get → modify → set → get round trip returned the old values and
/// nothing failed. That is the audit's §1 LIE cell, and it is the last one in the
/// PTZ family.
///
/// **The request is read with ONVIF's spelling only** —
/// `DefaultAbsolutePantTiltPositionSpace`, `Pant`, double `t`. A device parses
/// what its own schema declares, and accepting both spellings here would make
/// `tests/mock_roundtrip.rs` blind to the client regressing to the corrected one.
/// Stage 1 closed that hole in the render direction after a perturbation came
/// back green; this is the same hole in the parse direction.
///
/// **Three things are deliberately not written.** `CLAUDE.md` step 5c: a
/// documented omission is a design decision, an undocumented one is the `MTU`
/// bug.
///
/// - `UseCount` is the device's count of profiles referencing the configuration,
///   not the caller's to set. The mock does not maintain it for any
///   configuration family — the video encoder and source counts are fixture
///   numbers too — so recomputing it here alone would be the odd one out.
/// - `timeout_min` / `timeout_max` belong to `GetConfigurationOptions`, not to a
///   configuration. `crate::types::PtzConfiguration` has no such fields, so the
///   request cannot carry them at all.
/// - `ForcePersistence` is ignored. Real devices differ too widely on `false`
///   for a pretend model to beat none — `docs/mock-server.md` §13.3.
///
/// Everything else the client can send is written, **including clearing an
/// optional element the request omits**: `SetConfiguration` replaces the
/// configuration, so an absent `DefaultPTZSpeed` means "this configuration has
/// no default speed", not "leave the old one alone".
pub fn handle_ptz_set_configuration(state: &SharedState, body: &str) -> String {
    match apply_ptz_configuration(state, body) {
        Ok(()) => resp_empty("tptz", "SetConfigurationResponse"),
        Err(fault) => fault,
    }
}

fn apply_ptz_configuration(state: &SharedState, body: &str) -> Result<(), String> {
    let Some(token) = extract_attr(body, "PTZConfiguration", "token").filter(|t| !t.is_empty())
    else {
        return Err(resp_soap_fault(
            "env:Sender",
            "NoConfigToken-SETCFG-5623: SetConfiguration needs a tptz:PTZConfiguration \
             carrying the token of the configuration it replaces",
        ));
    };
    if !state.read().ptz_configs.iter().any(|c| c.token == token) {
        return Err(resp_soap_fault(
            "ter:NoConfig",
            &format!("NoSuchPTZConfig-SETCFG-5624: {token}"),
        ));
    }
    let cfg = extract_tag(body, "PTZConfiguration").unwrap_or_default();

    // `NodeToken` is the one required child of `PTZConfiguration` in onvif.xsd,
    // and a configuration that names a head this device does not have is worse
    // than a rejected write: every later `GetStatus` through a profile bound to
    // it would resolve to nothing.
    let Some(node) = extract_tag(&cfg, "NodeToken").filter(|t| !t.is_empty()) else {
        return Err(resp_soap_fault(
            "env:Sender",
            "NoNodeToken-SETCFG-5625: NodeToken is required — a PTZ configuration \
             that drives no node is not a configuration",
        ));
    };
    if !state.read().ptz_nodes.iter().any(|n| n.token == node) {
        return Err(resp_soap_fault(
            "ter:NoEntity",
            &format!("NoSuchNode-SETCFG-5626: {node}"),
        ));
    }

    let name = extract_tag(&cfg, "Name").unwrap_or_default();
    let timeout = extract_tag(&cfg, "DefaultPTZTimeout");
    let abs_pt = extract_tag(&cfg, "DefaultAbsolutePantTiltPositionSpace");
    let abs_z = extract_tag(&cfg, "DefaultAbsoluteZoomPositionSpace");
    let rel_pt = extract_tag(&cfg, "DefaultRelativePanTiltTranslationSpace");
    let rel_z = extract_tag(&cfg, "DefaultRelativeZoomTranslationSpace");
    let cont_pt = extract_tag(&cfg, "DefaultContinuousPanTiltVelocitySpace");
    let cont_z = extract_tag(&cfg, "DefaultContinuousZoomVelocitySpace");
    let (speed_pt, speed_z) = match extract_tag(&cfg, "DefaultPTZSpeed") {
        Some(s) => (
            pan_tilt_attrs(&s),
            extract_attr(&s, "Zoom", "x").and_then(|v| v.parse().ok()),
        ),
        None => (None, None),
    };
    let pt_limits = parse_limits(&cfg, "PanTiltLimits", SpaceKind::AbsolutePanTiltPosition);
    let z_limits = parse_limits(&cfg, "ZoomLimits", SpaceKind::AbsoluteZoomPosition);

    state.modify(|s| {
        if let Some(c) = s.ptz_configs.iter_mut().find(|c| c.token == token) {
            c.name = name.clone();
            c.node_token = node.clone();
            c.default_ptz_timeout = timeout.clone();
            c.abs_pan_tilt_space = abs_pt.clone();
            c.abs_zoom_space = abs_z.clone();
            c.rel_pan_tilt_space = rel_pt.clone();
            c.rel_zoom_space = rel_z.clone();
            c.cont_pan_tilt_space = cont_pt.clone();
            c.cont_zoom_space = cont_z.clone();
            c.default_speed_pan_tilt = speed_pt;
            c.default_speed_zoom = speed_z;
            c.pan_tilt_limits = pt_limits.clone();
            c.zoom_limits = z_limits.clone();
            eprintln!("    [STATE] PTZ configuration updated: {token} → node {node}");
        }
    });
    Ok(())
}

// ── GetServiceCapabilities ───────────────────────────────────────────────────

/// `tptz:Capabilities`.
///
/// The claims here are the ones this mock actually honours: `resp_ptz_status`
/// emits both `MoveStatus` and `Position`, and `GetCompatibleConfigurations`
/// is dispatched. `EFlip` and `Reverse` are deliberately **absent** rather
/// than `false` — the mock is the only device a test can rely on to exercise
/// the "attribute omitted" branch, which is distinct from "attribute said no".
pub fn resp_ptz_service_capabilities() -> String {
    soap(
        NS,
        r#"<tptz:GetServiceCapabilitiesResponse>
          <tptz:Capabilities GetCompatibleConfigurations="true"
                             MoveStatus="true"
                             StatusPosition="true"
                             MoveAndTrack="PresetToken PTZVector"/>
        </tptz:GetServiceCapabilitiesResponse>"#,
    )
}

// ── Preset tours ─────────────────────────────────────────────────────────────
//
// Unlike the capability responders, tours are **stateful**: a tour created by
// `CreatePresetTour` must come back from a later `GetPresetTours` or the mock
// is not an integration harness for the feature, only a fixture printer.
//
// The default fixture carries **two** tour spots on purpose. A one-spot tour
// passes just as well against a parser that returns the first `TourSpot` and
// drops the rest.

fn tour_xml(t: &PtzTour) -> String {
    let spots: String = t
        .spots
        .iter()
        .map(|s| {
            format!(
                r#"<tt:TourSpot>
              <tt:PresetDetail>
                <tt:PresetToken>{preset}</tt:PresetToken>
              </tt:PresetDetail>
              <tt:StayTime>{stay}</tt:StayTime>
            </tt:TourSpot>"#,
                preset = s.preset_token,
                stay = s.stay_time,
            )
        })
        .collect();
    let recurring = t
        .recurring_time
        .map(|n| format!("<tt:RecurringTime>{n}</tt:RecurringTime>"))
        .unwrap_or_default();
    format!(
        r#"<tptz:PresetTour token="{token}">
          <tt:Name>{name}</tt:Name>
          <tt:Status>
            <tt:State>{state}</tt:State>
          </tt:Status>
          <tt:AutoStart>{auto}</tt:AutoStart>
          <tt:StartingCondition RandomPresetOrder="{random}">
            {recurring}
            <tt:Direction>{direction}</tt:Direction>
          </tt:StartingCondition>
          {spots}
        </tptz:PresetTour>"#,
        token = t.token,
        name = t.name,
        state = t.state,
        auto = t.auto_start,
        random = t.random_preset_order,
        direction = t.direction,
    )
}

pub fn resp_ptz_preset_tours(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "TOURS-5612");
    let snapshot = state
        .read()
        .ptz
        .channel(&head)
        .map(|c| c.tours.clone())
        .unwrap_or_default();
    let tours: String = snapshot.iter().map(tour_xml).collect();
    soap(
        NS,
        &format!("<tptz:GetPresetToursResponse>{tours}</tptz:GetPresetToursResponse>"),
    )
}

pub fn resp_ptz_preset_tour(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "TOUR-5613");
    let inner = extract_tag(body, "GetPresetTour").unwrap_or_default();
    let token = extract_tag(&inner, "PresetTourToken").unwrap_or_default();
    let found = state
        .read()
        .ptz
        .channel(&head)
        .map(|c| c.tours.clone())
        .unwrap_or_default()
        .iter()
        .find(|t| t.token == token)
        .map(tour_xml);
    match found {
        Some(xml) => soap(
            NS,
            &format!("<tptz:GetPresetTourResponse>{xml}</tptz:GetPresetTourResponse>"),
        ),
        None => resp_soap_fault("ter:InvalidArgVal", &format!("NoSuchPresetTour: {token}")),
    }
}

/// `tt:PTZPresetTourOptions`. The three members are all `minOccurs="1"`.
///
/// `Direction` appears **twice** here, because in
/// `PTZPresetTourStartingConditionOptions` it is `[0..*]` — a list of what the
/// device supports — where the same element name inside a concrete
/// `StartingCondition` is a single value. A one-element list here would let a
/// parser that reads only the first child pass.
pub fn resp_ptz_preset_tour_options(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "TOUROPT-5614");
    let tokens: String = state
        .read()
        .ptz
        .channel(&head)
        .map(|c| c.presets.clone())
        .unwrap_or_default()
        .iter()
        .map(|p| format!("<tt:PresetToken>{}</tt:PresetToken>", p.token))
        .collect();
    soap(
        NS,
        &format!(
            r#"<tptz:GetPresetTourOptionsResponse>
          <tptz:Options>
            <tt:AutoStart>true</tt:AutoStart>
            <tt:StartingCondition>
              <tt:RecurringTime>
                <tt:Min>1</tt:Min>
                <tt:Max>10</tt:Max>
              </tt:RecurringTime>
              <tt:RecurringDuration>
                <tt:Min>PT1M</tt:Min>
                <tt:Max>PT8H</tt:Max>
              </tt:RecurringDuration>
              <tt:Direction>Forward</tt:Direction>
              <tt:Direction>Backward</tt:Direction>
            </tt:StartingCondition>
            <tt:TourSpot>
              <tt:PresetDetail>
                {tokens}
                <tt:Home>true</tt:Home>
              </tt:PresetDetail>
              <tt:StayTime>
                <tt:Min>PT5S</tt:Min>
                <tt:Max>PT10M</tt:Max>
              </tt:StayTime>
            </tt:TourSpot>
          </tptz:Options>
        </tptz:GetPresetTourOptionsResponse>"#
        ),
    )
}

/// Pick the next free `Tour_<n>` token, the same way presets are numbered.
fn next_tour_token(tours: &[PtzTour]) -> String {
    let used: std::collections::HashSet<u32> = tours
        .iter()
        .filter_map(|t| t.token.strip_prefix("Tour_").and_then(|n| n.parse().ok()))
        .collect();
    (1..)
        .find(|n| !used.contains(n))
        .map(|n| format!("Tour_{n}"))
        .unwrap_or_else(|| "Tour_1".to_string())
}

pub fn handle_ptz_create_preset_tour(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "CREATETOUR-5615");
    let token = state.modify_returning(|s| {
        let ch = s.ptz.channel_mut(&head);
        let token = next_tour_token(&ch.tours);
        ch.tours.push(PtzTour {
            token: token.clone(),
            name: String::new(),
            state: "Idle".into(),
            auto_start: false,
            random_preset_order: false,
            recurring_time: None,
            direction: "Forward".into(),
            spots: Vec::new(),
        });
        eprintln!("    [STATE] {head}: preset tour created: {token}");
        token
    });
    soap(
        NS,
        &format!(
            r#"<tptz:CreatePresetTourResponse>
              <tptz:PresetTourToken>{token}</tptz:PresetTourToken>
            </tptz:CreatePresetTourResponse>"#
        ),
    )
}

pub fn handle_ptz_modify_preset_tour(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "MODIFYTOUR-5616");
    let inner = extract_tag(body, "ModifyPresetTour").unwrap_or_default();
    let tour_xml = extract_tag(&inner, "PresetTour").unwrap_or_default();
    let token = extract_attr(&inner, "PresetTour", "token").unwrap_or_default();

    let name = extract_tag(&tour_xml, "Name").unwrap_or_default();
    let auto_start = extract_tag(&tour_xml, "AutoStart").as_deref() == Some("true");
    let random = extract_attr(&tour_xml, "StartingCondition", "RandomPresetOrder").as_deref()
        == Some("true");
    let recurring = extract_tag(&tour_xml, "RecurringTime").and_then(|v| v.parse().ok());
    let direction = extract_tag(&tour_xml, "Direction").unwrap_or_else(|| "Forward".into());
    let spots: Vec<PtzTourSpot> = extract_all_tags(&tour_xml, "TourSpot")
        .iter()
        .map(|s| PtzTourSpot {
            preset_token: extract_tag(s, "PresetToken").unwrap_or_default(),
            stay_time: extract_tag(s, "StayTime").unwrap_or_default(),
        })
        .collect();

    let found = state.modify_returning(|s| {
        let ch = s.ptz.channel_mut(&head);
        if let Some(t) = ch.tours.iter_mut().find(|t| t.token == token) {
            t.name = name;
            t.auto_start = auto_start;
            t.random_preset_order = random;
            t.recurring_time = recurring;
            t.direction = direction;
            t.spots = spots;
            eprintln!("    [STATE] {head}: preset tour modified: {token}");
            true
        } else {
            false
        }
    });

    if found {
        resp_empty("tptz", "ModifyPresetTourResponse")
    } else {
        resp_soap_fault("ter:InvalidArgVal", &format!("NoSuchPresetTour: {token}"))
    }
}

/// `Start` / `Stop` / `Pause` move the stored `state` string, mirroring the
/// existing `SetRecordingJobMode` handler. There is no clock here, so nothing
/// actually tours — but a client can observe that its operation took effect.
pub fn handle_ptz_operate_preset_tour(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "OPERATETOUR-5617");
    let inner = extract_tag(body, "OperatePresetTour").unwrap_or_default();
    let token = extract_tag(&inner, "PresetTourToken").unwrap_or_default();
    let op = extract_tag(&inner, "Operation").unwrap_or_default();

    let new_state = match op.as_str() {
        "Start" => "Touring",
        "Stop" => "Idle",
        "Pause" => "Paused",
        _ => return resp_soap_fault("ter:InvalidArgVal", &format!("BadTourOperation: {op}")),
    };

    let found = state.modify_returning(|s| {
        let ch = s.ptz.channel_mut(&head);
        if let Some(t) = ch.tours.iter_mut().find(|t| t.token == token) {
            t.state = new_state.to_string();
            eprintln!("    [STATE] {head}: preset tour {token} -> {new_state}");
            true
        } else {
            false
        }
    });

    if found {
        resp_empty("tptz", "OperatePresetTourResponse")
    } else {
        resp_soap_fault("ter:InvalidArgVal", &format!("NoSuchPresetTour: {token}"))
    }
}

pub fn handle_ptz_remove_preset_tour(state: &SharedState, body: &str) -> String {
    let head = head!(state, body, "RMTOUR-5618");
    let inner = extract_tag(body, "RemovePresetTour").unwrap_or_default();
    if let Some(token) = extract_tag(&inner, "PresetTourToken") {
        state.modify(|s| {
            s.ptz.channel_mut(&head).tours.retain(|t| t.token != token);
            eprintln!("    [STATE] {head}: preset tour removed: {token}");
        });
    }
    resp_empty("tptz", "RemovePresetTourResponse")
}

// ── SendAuxiliaryCommand ─────────────────────────────────────────────────────

/// The **PTZ** `SendAuxiliaryCommand`, not the Device one in `device.rs`. Two
/// different operations, two endpoints, and this one returns a payload where
/// the Device one returns a bare acknowledgement.
///
/// The accepted values are exactly those advertised by
/// `resp_service_capabilities` in `device.rs` as `Misc/@AuxiliaryCommands` —
/// a mock that accepted anything would let a client ship code that only works
/// against the mock.
pub fn handle_ptz_send_auxiliary_command(body: &str) -> String {
    const ACCEPTED: &[&str] = &[
        "tt:Wiper|On",
        "tt:Wiper|Off",
        "tt:IRLamp|On",
        "tt:IRLamp|Off",
        "tt:IRLamp|Auto",
    ];
    let inner = extract_tag(body, "SendAuxiliaryCommand").unwrap_or_default();
    let data = extract_tag(&inner, "AuxiliaryData").unwrap_or_default();
    if !ACCEPTED.contains(&data.as_str()) {
        return resp_soap_fault("ter:InvalidArgVal", &format!("NoAuxiliaryCommand: {data}"));
    }
    soap(
        NS,
        &format!(
            r#"<tptz:SendAuxiliaryCommandResponse>
              <tptz:AuxiliaryResponse>{data} accepted</tptz:AuxiliaryResponse>
            </tptz:SendAuxiliaryCommandResponse>"#
        ),
    )
}
