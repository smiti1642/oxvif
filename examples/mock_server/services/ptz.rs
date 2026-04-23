use crate::helpers::{resp_empty, soap};
use crate::state::{PtzPreset, SharedState};
use crate::xml_parse::{extract_attr, extract_tag};

const NS: &str = r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#;
const POS_SPACE: &str = "http://www.onvif.org/ver10/tptz/PanTiltSpaces/PositionGenericSpace";
const ZOOM_SPACE: &str = "http://www.onvif.org/ver10/tptz/ZoomSpaces/PositionGenericSpace";

fn clamp(v: f32, min: f32, max: f32) -> f32 {
    v.max(min).min(max)
}

pub fn resp_ptz_status(state: &SharedState) -> String {
    let p = &state.read().ptz;
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
            <tt:UtcTime>2026-04-23T00:00:00Z</tt:UtcTime>
          </tptz:PTZStatus>
        </tptz:GetStatusResponse>"#,
            pan = p.pan,
            tilt = p.tilt,
            zoom = p.zoom,
        ),
    )
}

pub fn resp_ptz_presets(state: &SharedState) -> String {
    let presets = &state.read().ptz.presets;
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
    let inner = extract_tag(body, "SetPreset").unwrap_or_default();
    let name = extract_tag(&inner, "PresetName");
    let token_in = extract_tag(&inner, "PresetToken");

    let token = state.modify_returning(|s| {
        let pos = (s.ptz.pan, s.ptz.tilt, s.ptz.zoom);
        if let Some(t) = token_in {
            if let Some(p) = s.ptz.presets.iter_mut().find(|p| p.token == t) {
                if let Some(n) = name {
                    p.name = n;
                }
                p.pan = pos.0;
                p.tilt = pos.1;
                p.zoom = pos.2;
                eprintln!("    [STATE] preset updated: {t}");
                return t;
            }
            // Token specified but not found — fall through to create with that token.
            eprintln!("    [STATE] preset created with client-supplied token: {t}");
            s.ptz.presets.push(PtzPreset {
                token: t.clone(),
                name: name.unwrap_or_else(|| t.clone()),
                pan: pos.0,
                tilt: pos.1,
                zoom: pos.2,
            });
            return t;
        }
        let new_token = next_preset_token(&s.ptz.presets);
        eprintln!("    [STATE] preset created: {new_token}");
        s.ptz.presets.push(PtzPreset {
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
    let inner = extract_tag(body, "RemovePreset").unwrap_or_default();
    if let Some(token) = extract_tag(&inner, "PresetToken") {
        state.modify(|s| {
            s.ptz.presets.retain(|p| p.token != token);
            eprintln!("    [STATE] preset removed: {token}");
        });
    }
    resp_empty("tptz", "RemovePresetResponse")
}

pub fn handle_ptz_goto_preset(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "GotoPreset").unwrap_or_default();
    if let Some(token) = extract_tag(&inner, "PresetToken") {
        state.modify(|s| {
            if let Some(p) = s.ptz.presets.iter().find(|p| p.token == token) {
                s.ptz.pan = p.pan;
                s.ptz.tilt = p.tilt;
                s.ptz.zoom = p.zoom;
                eprintln!("    [STATE] goto preset: {token}");
            }
        });
    }
    resp_empty("tptz", "GotoPresetResponse")
}

pub fn handle_ptz_absolute_move(state: &SharedState, body: &str) -> String {
    // <tptz:AbsoluteMove><tptz:Position><tt:PanTilt x=.. y=../><tt:Zoom x=../></tptz:Position>...
    let inner = extract_tag(body, "Position").unwrap_or_default();
    let pan = extract_attr(&inner, "PanTilt", "x").and_then(|v| v.parse::<f32>().ok());
    let tilt = extract_attr(&inner, "PanTilt", "y").and_then(|v| v.parse::<f32>().ok());
    let zoom = extract_attr(&inner, "Zoom", "x").and_then(|v| v.parse::<f32>().ok());
    state.modify(|s| {
        if let Some(v) = pan {
            s.ptz.pan = clamp(v, -1.0, 1.0);
        }
        if let Some(v) = tilt {
            s.ptz.tilt = clamp(v, -1.0, 1.0);
        }
        if let Some(v) = zoom {
            s.ptz.zoom = clamp(v, 0.0, 1.0);
        }
        eprintln!(
            "    [STATE] PTZ absolute → ({:.2}, {:.2}, {:.2})",
            s.ptz.pan, s.ptz.tilt, s.ptz.zoom
        );
    });
    resp_empty("tptz", "AbsoluteMoveResponse")
}

pub fn handle_ptz_relative_move(state: &SharedState, body: &str) -> String {
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
        s.ptz.pan = clamp(s.ptz.pan + dpan, -1.0, 1.0);
        s.ptz.tilt = clamp(s.ptz.tilt + dtilt, -1.0, 1.0);
        s.ptz.zoom = clamp(s.ptz.zoom + dzoom, 0.0, 1.0);
        eprintln!(
            "    [STATE] PTZ relative → ({:.2}, {:.2}, {:.2})",
            s.ptz.pan, s.ptz.tilt, s.ptz.zoom
        );
    });
    resp_empty("tptz", "RelativeMoveResponse")
}

/// ContinuousMove updates state by a small step in the velocity direction
/// — enough that GetStatus right after Move shows movement, without
/// requiring the mock to actually run a timer.
pub fn handle_ptz_continuous_move(state: &SharedState, body: &str) -> String {
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
        s.ptz.pan = clamp(s.ptz.pan + vpan * step, -1.0, 1.0);
        s.ptz.tilt = clamp(s.ptz.tilt + vtilt * step, -1.0, 1.0);
        s.ptz.zoom = clamp(s.ptz.zoom + vzoom * step, 0.0, 1.0);
    });
    resp_empty("tptz", "ContinuousMoveResponse")
}

pub fn handle_ptz_stop() -> String {
    resp_empty("tptz", "StopResponse")
}

pub fn handle_ptz_goto_home_position(state: &SharedState) -> String {
    state.modify(|s| {
        s.ptz.pan = s.ptz.home_pan;
        s.ptz.tilt = s.ptz.home_tilt;
        s.ptz.zoom = s.ptz.home_zoom;
        eprintln!("    [STATE] PTZ goto home");
    });
    resp_empty("tptz", "GotoHomePositionResponse")
}

pub fn handle_ptz_set_home_position(state: &SharedState) -> String {
    state.modify(|s| {
        s.ptz.home_pan = s.ptz.pan;
        s.ptz.home_tilt = s.ptz.tilt;
        s.ptz.home_zoom = s.ptz.zoom;
        eprintln!(
            "    [STATE] PTZ set home → ({:.2}, {:.2}, {:.2})",
            s.ptz.home_pan, s.ptz.home_tilt, s.ptz.home_zoom
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
