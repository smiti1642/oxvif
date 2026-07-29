//! Imaging service responders.
//!
//! **Every operation here is per-`VideoSourceToken`.** On a multi-sensor device
//! there is no such thing as "the device's brightness" or "the device's focus
//! range" — there is one answer per lens, and a responder that ignores the
//! token answers for the wrong one without saying so.
//!
//! The mock's two sensors differ on purpose (see `default_imaging_sources` in
//! `src/mock/state.rs`): `VS_1` is a motorised 5MP lens reporting levels on
//! 0–100, `VS_2` is a fixed-focus 720p lens on 0–255 that **faults** on the
//! focus operations. Both halves of that are things a single-sensor fixture
//! cannot express, and both are what the tests assert on.

use crate::mock::helpers::{resp_empty, resp_soap_fault, soap};
use crate::mock::state::{ImagingState, SharedState};
use crate::mock::xml_parse::extract_tag;

const NS: &str = r#"xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl""#;

/// Resolve the request's `VideoSourceToken` to one sensor's imaging state.
///
/// Absent is a fault, not a default channel — the same reasoning as
/// `media::require_config_token`. Unknown is a fault whose reason names the
/// token, so an assertion can tell a rejected token from a missing one.
fn lookup(
    state: &SharedState,
    body: &str,
    missing_reason: &str,
    unknown_prefix: &str,
) -> Result<ImagingState, String> {
    let Some(want) = extract_tag(body, "VideoSourceToken").filter(|t| !t.is_empty()) else {
        return Err(resp_soap_fault("env:Sender", missing_reason));
    };
    state
        .read()
        .imaging_sources
        .iter()
        .find(|i| i.source_token == want)
        .cloned()
        .ok_or_else(|| resp_soap_fault("env:Sender", &format!("{unknown_prefix}: {want}")))
}

/// A lens with no motorised focus refuses the focus operations outright rather
/// than reporting a range it cannot honour.
fn require_focus(img: &ImagingState, reason_prefix: &str) -> Result<(), String> {
    if img.focus_supported {
        Ok(())
    } else {
        Err(resp_soap_fault(
            "env:Sender",
            &format!("{reason_prefix}: {}", img.source_token),
        ))
    }
}

pub fn resp_imaging_settings(state: &SharedState, body: &str) -> String {
    let img = match lookup(
        state,
        body,
        "NoVideoSourceToken-IMGSET-5601",
        "NoSuchVideoSource-IMGSET-5602",
    ) {
        Ok(i) => i,
        Err(fault) => return fault,
    };

    // `tt:Focus` is [0..1] in `tt:ImagingSettings20`; a fixed lens omits it
    // rather than reporting a mode it cannot change.
    let focus = if img.focus_supported {
        format!(
            "<tt:Focus><tt:AutoFocusMode>{}</tt:AutoFocusMode></tt:Focus>",
            img.focus_mode
        )
    } else {
        String::new()
    };

    soap(
        NS,
        &format!(
            // `tt:ImagingSettings20` is an xs:sequence — children in schema order:
            // BacklightCompensation, Brightness, ColorSaturation, Contrast,
            // Exposure, Focus, IrCutFilter, Sharpness, WideDynamicRange,
            // WhiteBalance, Extension.
            r#"<timg:GetImagingSettingsResponse>
          <timg:ImagingSettings>
            <tt:BacklightCompensation><tt:Mode>{backlight}</tt:Mode></tt:BacklightCompensation>
            <tt:Brightness>{brightness}</tt:Brightness>
            <tt:ColorSaturation>{saturation}</tt:ColorSaturation>
            <tt:Contrast>{contrast}</tt:Contrast>
            <tt:Exposure><tt:Mode>{exposure}</tt:Mode></tt:Exposure>
            {focus}
            <tt:IrCutFilter>{ir}</tt:IrCutFilter>
            <tt:Sharpness>{sharpness}</tt:Sharpness>
            <tt:WideDynamicRange><tt:Mode>{wdr_mode}</tt:Mode><tt:Level>{wdr_level}</tt:Level></tt:WideDynamicRange>
            <tt:WhiteBalance><tt:Mode>{wb}</tt:Mode></tt:WhiteBalance>
          </timg:ImagingSettings>
        </timg:GetImagingSettingsResponse>"#,
            backlight = img.backlight_compensation,
            brightness = img.brightness,
            saturation = img.color_saturation,
            contrast = img.contrast,
            exposure = img.exposure_mode,
            ir = img.ir_cut_filter,
            sharpness = img.sharpness,
            wdr_mode = img.wide_dynamic_range_mode,
            wdr_level = img.wide_dynamic_range_level,
            wb = img.white_balance_mode,
        ),
    )
}

pub fn handle_set_imaging_settings(state: &SharedState, body: &str) -> String {
    // Resolve first so an absent or unknown token faults *before* anything is
    // written — a Set that half-applies to a guessed channel is worse than one
    // that refuses.
    let target = match lookup(
        state,
        body,
        "NoVideoSourceToken-IMGSETW-5603",
        "NoSuchVideoSource-IMGSETW-5604",
    ) {
        Ok(i) => i.source_token,
        Err(fault) => return fault,
    };

    state.modify(|s| {
        let Some(img) = s
            .imaging_sources
            .iter_mut()
            .find(|i| i.source_token == target)
        else {
            return;
        };
        if let Some(v) = extract_tag(body, "Brightness").and_then(|v| v.parse().ok()) {
            img.brightness = v;
        }
        if let Some(v) = extract_tag(body, "ColorSaturation").and_then(|v| v.parse().ok()) {
            img.color_saturation = v;
        }
        if let Some(v) = extract_tag(body, "Contrast").and_then(|v| v.parse().ok()) {
            img.contrast = v;
        }
        if let Some(v) = extract_tag(body, "Sharpness").and_then(|v| v.parse().ok()) {
            img.sharpness = v;
        }
        if let Some(v) = extract_tag(body, "IrCutFilter") {
            img.ir_cut_filter = v;
        }
        // oxvif sends each mode as a flat XML field, extract by context
        if let Some(v) = extract_tag(body, "WhiteBalanceMode") {
            img.white_balance_mode = v;
        }
        if let Some(v) = extract_tag(body, "ExposureMode") {
            img.exposure_mode = v;
        }
        if let Some(v) = extract_tag(body, "BacklightCompensationMode") {
            img.backlight_compensation = v;
        }
        if let Some(v) = extract_tag(body, "WideDynamicRangeMode") {
            img.wide_dynamic_range_mode = v;
        }
        if let Some(v) = extract_tag(body, "WideDynamicRangeLevel").and_then(|v| v.parse().ok()) {
            img.wide_dynamic_range_level = v;
        }
        // A fixed lens has no focus mode to set; silently ignoring it here
        // matches what such a device does with the field.
        if img.focus_supported
            && let Some(v) = extract_tag(body, "AutoFocusMode")
        {
            img.focus_mode = v;
        }
        eprintln!("    [STATE] imaging settings updated for {target}");
    });
    resp_empty("timg", "SetImagingSettingsResponse")
}

pub fn resp_imaging_options(state: &SharedState, body: &str) -> String {
    let img = match lookup(
        state,
        body,
        "NoVideoSourceToken-IMGOPT-5605",
        "NoSuchVideoSource-IMGOPT-5606",
    ) {
        Ok(i) => i,
        Err(fault) => return fault,
    };

    // `tt:Focus` is [0..1] in `tt:ImagingOptions20`. A fixed lens offers no
    // auto-focus modes at all, so the block is absent rather than empty.
    //
    // The element is `tt:AutoFocusModes`, not `AFModes`. The mock sent
    // `AFModes` until 0.15 — a name that appears nowhere in
    // `tt:FocusOptions20` — so `ImagingOptions::focus_af_modes` came back
    // empty from the mock forever and nothing noticed: the hand-written unit
    // fixture in `imaging_tests.rs` spelled it correctly, and the two were
    // never compared. Exactly the failure `tests/mock_service_capabilities.rs`
    // was written to prevent for a different service.
    let focus = if img.focus_supported {
        "<tt:Focus>\
           <tt:AutoFocusModes>AUTO</tt:AutoFocusModes>\
           <tt:AutoFocusModes>MANUAL</tt:AutoFocusModes>\
         </tt:Focus>"
    } else {
        ""
    };

    soap(
        NS,
        &format!(
            r#"<timg:GetOptionsResponse>
          <timg:ImagingOptions>
            <tt:Brightness><tt:Min>0</tt:Min><tt:Max>{max}</tt:Max></tt:Brightness>
            <tt:ColorSaturation><tt:Min>0</tt:Min><tt:Max>{max}</tt:Max></tt:ColorSaturation>
            <tt:Contrast><tt:Min>0</tt:Min><tt:Max>{max}</tt:Max></tt:Contrast>
            <tt:Sharpness><tt:Min>0</tt:Min><tt:Max>{max}</tt:Max></tt:Sharpness>
            <tt:IrCutFilterModes>ON</tt:IrCutFilterModes>
            <tt:IrCutFilterModes>OFF</tt:IrCutFilterModes>
            <tt:IrCutFilterModes>AUTO</tt:IrCutFilterModes>
            <tt:WhiteBalance>
              <tt:Mode>AUTO</tt:Mode>
              <tt:Mode>MANUAL</tt:Mode>
            </tt:WhiteBalance>
            <tt:Exposure>
              <tt:Mode>AUTO</tt:Mode>
              <tt:Mode>MANUAL</tt:Mode>
            </tt:Exposure>
            {focus}
            <tt:WideDynamicRange>
              <tt:Mode>OFF</tt:Mode>
              <tt:Mode>ON</tt:Mode>
              <tt:Level><tt:Min>0</tt:Min><tt:Max>{max}</tt:Max></tt:Level>
            </tt:WideDynamicRange>
            <tt:BacklightCompensation>
              <tt:Mode>OFF</tt:Mode>
              <tt:Mode>ON</tt:Mode>
            </tt:BacklightCompensation>
          </timg:ImagingOptions>
        </timg:GetOptionsResponse>"#,
            max = img.level_max,
        ),
    )
}

pub fn resp_imaging_status(state: &SharedState, body: &str) -> String {
    let img = match lookup(
        state,
        body,
        "NoVideoSourceToken-IMGSTAT-5607",
        "NoSuchVideoSource-IMGSTAT-5608",
    ) {
        Ok(i) => i,
        Err(fault) => return fault,
    };

    // `tt:FocusStatus20` is [0..1] in `tt:ImagingStatus20`, and it is the only
    // content the type has — so a fixed lens returns an empty `Status`, which
    // is a legal response and one a caller has to survive.
    let focus = if img.focus_supported {
        "<tt:FocusStatus20>\
           <tt:Position>0.5</tt:Position>\
           <tt:MoveStatus>IDLE</tt:MoveStatus>\
         </tt:FocusStatus20>"
    } else {
        ""
    };

    soap(
        NS,
        &format!(
            "<timg:GetStatusResponse><timg:Status>{focus}</timg:Status></timg:GetStatusResponse>"
        ),
    )
}

pub fn resp_imaging_move_options(state: &SharedState, body: &str) -> String {
    let img = match lookup(
        state,
        body,
        "NoVideoSourceToken-IMGMOVEOPT-5609",
        "NoSuchVideoSource-IMGMOVEOPT-5610",
    ) {
        Ok(i) => i,
        Err(fault) => return fault,
    };
    if let Err(fault) = require_focus(&img, "NoFocusSupport-IMGMOVEOPT-5611") {
        return fault;
    }

    soap(
        NS,
        r#"<timg:GetMoveOptionsResponse>
          <timg:MoveOptions>
            <tt:Absolute>
              <tt:PositionSpace><tt:Min>0.0</tt:Min><tt:Max>1.0</tt:Max></tt:PositionSpace>
              <tt:SpeedSpace><tt:Min>0.0</tt:Min><tt:Max>1.0</tt:Max></tt:SpeedSpace>
            </tt:Absolute>
            <tt:Continuous>
              <tt:SpeedSpace><tt:Min>-1.0</tt:Min><tt:Max>1.0</tt:Max></tt:SpeedSpace>
            </tt:Continuous>
          </timg:MoveOptions>
        </timg:GetMoveOptionsResponse>"#,
    )
}

/// `timg:Move` — focus move on one lens.
///
/// Used to be an unconditional empty response, so it accepted a move on a
/// channel that has no focus motor and on tokens that do not exist.
pub fn handle_imaging_move(state: &SharedState, body: &str) -> String {
    let img = match lookup(
        state,
        body,
        "NoVideoSourceToken-IMGMOVE-5612",
        "NoSuchVideoSource-IMGMOVE-5613",
    ) {
        Ok(i) => i,
        Err(fault) => return fault,
    };
    if let Err(fault) = require_focus(&img, "NoFocusSupport-IMGMOVE-5614") {
        return fault;
    }
    resp_empty("timg", "MoveResponse")
}

/// `timg:Stop` — halt a focus move on one lens.
///
/// Stop is accepted on any focusable lens whether or not a move is running;
/// that is what devices do, and the mock keeps no move state to check against.
pub fn handle_imaging_stop(state: &SharedState, body: &str) -> String {
    let img = match lookup(
        state,
        body,
        "NoVideoSourceToken-IMGSTOP-5615",
        "NoSuchVideoSource-IMGSTOP-5616",
    ) {
        Ok(i) => i,
        Err(fault) => return fault,
    };
    if let Err(fault) = require_focus(&img, "NoFocusSupport-IMGSTOP-5617") {
        return fault;
    }
    resp_empty("timg", "StopResponse")
}

// ── GetServiceCapabilities ───────────────────────────────────────────────────

/// `timg:Capabilities`.
///
/// `AdaptablePreset` is spelled exactly as the schema has it — singular,
/// "Adaptable". The mock implements no imaging presets, so `Presets` is a
/// real `false` here rather than an omission.
///
/// This is the one Imaging operation that is **not** per-source: it describes
/// the service, not a lens, and carries no `VideoSourceToken` in the schema.
pub fn resp_imaging_service_capabilities() -> String {
    soap(
        NS,
        r#"<timg:GetServiceCapabilitiesResponse>
          <timg:Capabilities ImageStabilization="false"
                             Presets="false"
                             AdaptablePreset="false"/>
        </timg:GetServiceCapabilitiesResponse>"#,
    )
}
