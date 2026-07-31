use crate::mock::helpers::{resp_empty, resp_soap_fault, soap};
use crate::mock::state::{PtzPreset, PtzTour, PtzTourSpot, SharedState};
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

/// `require_profile`, but returning early from the handler with the fault.
macro_rules! profile {
    ($state:expr, $body:expr, $tag:literal) => {
        match require_profile($state, $body, $tag) {
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
    let profile = profile!(state, body, "STATUS-5601");
    let snapshot = state
        .read()
        .ptz
        .channel(&profile)
        .cloned()
        .unwrap_or_default();
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
    let profile = profile!(state, body, "PRESETS-5602");
    let presets = state
        .read()
        .ptz
        .channel(&profile)
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
    let profile = profile!(state, body, "SETPRESET-5603");
    let inner = extract_tag(body, "SetPreset").unwrap_or_default();
    let name = extract_tag(&inner, "PresetName");
    let token_in = extract_tag(&inner, "PresetToken");

    let token = state.modify_returning(|s| {
        let ch = s.ptz.channel_mut(&profile);
        let pos = (ch.pan, ch.tilt, ch.zoom);
        if let Some(t) = token_in {
            if let Some(p) = ch.presets.iter_mut().find(|p| p.token == t) {
                if let Some(n) = name {
                    p.name = n;
                }
                p.pan = pos.0;
                p.tilt = pos.1;
                p.zoom = pos.2;
                eprintln!("    [STATE] {profile}: preset updated: {t}");
                return t;
            }
            // Token specified but not found — fall through to create with that token.
            eprintln!("    [STATE] {profile}: preset created with client-supplied token: {t}");
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
        eprintln!("    [STATE] {profile}: preset created: {new_token}");
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
    let profile = profile!(state, body, "RMPRESET-5604");
    let inner = extract_tag(body, "RemovePreset").unwrap_or_default();
    if let Some(token) = extract_tag(&inner, "PresetToken") {
        state.modify(|s| {
            s.ptz
                .channel_mut(&profile)
                .presets
                .retain(|p| p.token != token);
            eprintln!("    [STATE] {profile}: preset removed: {token}");
        });
    }
    resp_empty("tptz", "RemovePresetResponse")
}

pub fn handle_ptz_goto_preset(state: &SharedState, body: &str) -> String {
    let profile = profile!(state, body, "GOTOPRESET-5605");
    let inner = extract_tag(body, "GotoPreset").unwrap_or_default();
    if let Some(token) = extract_tag(&inner, "PresetToken") {
        state.modify(|s| {
            let ch = s.ptz.channel_mut(&profile);
            if let Some(p) = ch.presets.iter().find(|p| p.token == token) {
                let (pan, tilt, zoom) = (p.pan, p.tilt, p.zoom);
                ch.pan = pan;
                ch.tilt = tilt;
                ch.zoom = zoom;
                eprintln!("    [STATE] {profile}: goto preset: {token}");
            }
        });
    }
    resp_empty("tptz", "GotoPresetResponse")
}

pub fn handle_ptz_absolute_move(state: &SharedState, body: &str) -> String {
    // <tptz:AbsoluteMove><tptz:Position><tt:PanTilt x=.. y=../><tt:Zoom x=../></tptz:Position>...
    let profile = profile!(state, body, "ABSMOVE-5606");
    let inner = extract_tag(body, "Position").unwrap_or_default();
    let pan = extract_attr(&inner, "PanTilt", "x").and_then(|v| v.parse::<f32>().ok());
    let tilt = extract_attr(&inner, "PanTilt", "y").and_then(|v| v.parse::<f32>().ok());
    let zoom = extract_attr(&inner, "Zoom", "x").and_then(|v| v.parse::<f32>().ok());
    state.modify(|s| {
        let ch = s.ptz.channel_mut(&profile);
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
            "    [STATE] {profile}: PTZ absolute → ({:.2}, {:.2}, {:.2})",
            ch.pan, ch.tilt, ch.zoom
        );
    });
    resp_empty("tptz", "AbsoluteMoveResponse")
}

pub fn handle_ptz_relative_move(state: &SharedState, body: &str) -> String {
    let profile = profile!(state, body, "RELMOVE-5607");
    let inner = extract_tag(body, "Translation").unwrap_or_default();
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
        let ch = s.ptz.channel_mut(&profile);
        ch.pan = clamp(ch.pan + dpan, -1.0, 1.0);
        ch.tilt = clamp(ch.tilt + dtilt, -1.0, 1.0);
        ch.zoom = clamp(ch.zoom + dzoom, 0.0, 1.0);
        eprintln!(
            "    [STATE] {profile}: PTZ relative → ({:.2}, {:.2}, {:.2})",
            ch.pan, ch.tilt, ch.zoom
        );
    });
    resp_empty("tptz", "RelativeMoveResponse")
}

/// ContinuousMove updates state by a small step in the velocity direction
/// — enough that GetStatus right after Move shows movement, without
/// requiring the mock to actually run a timer.
pub fn handle_ptz_continuous_move(state: &SharedState, body: &str) -> String {
    let profile = profile!(state, body, "CONTMOVE-5608");
    let inner = extract_tag(body, "Velocity").unwrap_or_default();
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
        let ch = s.ptz.channel_mut(&profile);
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
    let _profile = profile!(state, body, "STOP-5609");
    resp_empty("tptz", "StopResponse")
}

pub fn handle_ptz_goto_home_position(state: &SharedState, body: &str) -> String {
    let profile = profile!(state, body, "GOTOHOME-5610");
    state.modify(|s| {
        let ch = s.ptz.channel_mut(&profile);
        ch.pan = ch.home_pan;
        ch.tilt = ch.home_tilt;
        ch.zoom = ch.home_zoom;
        eprintln!("    [STATE] {profile}: PTZ goto home");
    });
    resp_empty("tptz", "GotoHomePositionResponse")
}

pub fn handle_ptz_set_home_position(state: &SharedState, body: &str) -> String {
    let profile = profile!(state, body, "SETHOME-5611");
    state.modify(|s| {
        let ch = s.ptz.channel_mut(&profile);
        ch.home_pan = ch.pan;
        ch.home_tilt = ch.tilt;
        ch.home_zoom = ch.zoom;
        eprintln!(
            "    [STATE] {profile}: PTZ set home → ({:.2}, {:.2}, {:.2})",
            ch.home_pan, ch.home_tilt, ch.home_zoom
        );
    });
    resp_empty("tptz", "SetHomePositionResponse")
}

pub fn resp_ptz_nodes() -> String {
    soap(
        NS,
        r#"<tptz:GetNodesResponse>
          <tptz:PTZNode token="PTZNode_1" FixedHomePosition="false">
            <tt:Name>PTZNode</tt:Name>
            <tt:SupportedPTZSpaces/>
            <tt:MaximumNumberOfPresets>100</tt:MaximumNumberOfPresets>
            <tt:HomeSupported>true</tt:HomeSupported>
          </tptz:PTZNode>
        </tptz:GetNodesResponse>"#,
    )
}

pub fn resp_ptz_node() -> String {
    soap(
        NS,
        r#"<tptz:GetNodeResponse>
          <tptz:PTZNode token="PTZNode_1" FixedHomePosition="false">
            <tt:Name>PTZNode</tt:Name>
            <tt:SupportedPTZSpaces/>
            <tt:MaximumNumberOfPresets>100</tt:MaximumNumberOfPresets>
            <tt:HomeSupported>true</tt:HomeSupported>
          </tptz:PTZNode>
        </tptz:GetNodeResponse>"#,
    )
}

pub fn resp_ptz_configurations() -> String {
    soap(
        NS,
        r#"<tptz:GetConfigurationsResponse>
          <tptz:PTZConfiguration token="PTZConfig_1">
            <tt:Name>PTZConfig</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:NodeToken>PTZNode_1</tt:NodeToken>
            <tt:DefaultPTZTimeout>PT10S</tt:DefaultPTZTimeout>
          </tptz:PTZConfiguration>
        </tptz:GetConfigurationsResponse>"#,
    )
}

/// `GetCompatibleConfigurations` — same PTZConfiguration content as
/// `GetConfigurations`, but wrapped in its own response element (the client
/// parser matches on `GetCompatibleConfigurationsResponse`, so it cannot share
/// `resp_ptz_configurations`'s `GetConfigurationsResponse`).
pub fn resp_ptz_compatible_configurations() -> String {
    soap(
        NS,
        r#"<tptz:GetCompatibleConfigurationsResponse>
          <tptz:PTZConfiguration token="PTZConfig_1">
            <tt:Name>PTZConfig</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:NodeToken>PTZNode_1</tt:NodeToken>
            <tt:DefaultPTZTimeout>PT10S</tt:DefaultPTZTimeout>
          </tptz:PTZConfiguration>
        </tptz:GetCompatibleConfigurationsResponse>"#,
    )
}

pub fn resp_ptz_configuration() -> String {
    soap(
        NS,
        r#"<tptz:GetConfigurationResponse>
          <tptz:PTZConfiguration token="PTZConfig_1">
            <tt:Name>PTZConfig</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:NodeToken>PTZNode_1</tt:NodeToken>
            <tt:DefaultPTZTimeout>PT10S</tt:DefaultPTZTimeout>
          </tptz:PTZConfiguration>
        </tptz:GetConfigurationResponse>"#,
    )
}

pub fn resp_ptz_configuration_options() -> String {
    soap(
        NS,
        r#"<tptz:GetConfigurationOptionsResponse>
          <tptz:PTZConfigurationOptions>
            <tt:PTZTimeout>
              <tt:Min>PT1S</tt:Min>
              <tt:Max>PT60S</tt:Max>
            </tt:PTZTimeout>
          </tptz:PTZConfigurationOptions>
        </tptz:GetConfigurationOptionsResponse>"#,
    )
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
    let profile = profile!(state, body, "TOURS-5612");
    let snapshot = state
        .read()
        .ptz
        .channel(&profile)
        .map(|c| c.tours.clone())
        .unwrap_or_default();
    let tours: String = snapshot.iter().map(tour_xml).collect();
    soap(
        NS,
        &format!("<tptz:GetPresetToursResponse>{tours}</tptz:GetPresetToursResponse>"),
    )
}

pub fn resp_ptz_preset_tour(state: &SharedState, body: &str) -> String {
    let profile = profile!(state, body, "TOUR-5613");
    let inner = extract_tag(body, "GetPresetTour").unwrap_or_default();
    let token = extract_tag(&inner, "PresetTourToken").unwrap_or_default();
    let found = state
        .read()
        .ptz
        .channel(&profile)
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
    let profile = profile!(state, body, "TOUROPT-5614");
    let tokens: String = state
        .read()
        .ptz
        .channel(&profile)
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
    let profile = profile!(state, body, "CREATETOUR-5615");
    let token = state.modify_returning(|s| {
        let ch = s.ptz.channel_mut(&profile);
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
        eprintln!("    [STATE] {profile}: preset tour created: {token}");
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
    let profile = profile!(state, body, "MODIFYTOUR-5616");
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
        let ch = s.ptz.channel_mut(&profile);
        if let Some(t) = ch.tours.iter_mut().find(|t| t.token == token) {
            t.name = name;
            t.auto_start = auto_start;
            t.random_preset_order = random;
            t.recurring_time = recurring;
            t.direction = direction;
            t.spots = spots;
            eprintln!("    [STATE] {profile}: preset tour modified: {token}");
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
    let profile = profile!(state, body, "OPERATETOUR-5617");
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
        let ch = s.ptz.channel_mut(&profile);
        if let Some(t) = ch.tours.iter_mut().find(|t| t.token == token) {
            t.state = new_state.to_string();
            eprintln!("    [STATE] {profile}: preset tour {token} -> {new_state}");
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
    let profile = profile!(state, body, "RMTOUR-5618");
    let inner = extract_tag(body, "RemovePresetTour").unwrap_or_default();
    if let Some(token) = extract_tag(&inner, "PresetTourToken") {
        state.modify(|s| {
            s.ptz
                .channel_mut(&profile)
                .tours
                .retain(|t| t.token != token);
            eprintln!("    [STATE] {profile}: preset tour removed: {token}");
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
