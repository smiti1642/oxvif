use crate::mock::helpers::{resp_empty, soap};
use crate::mock::state::SharedState;
use crate::mock::xml_parse::extract_tag;

const NS: &str = r#"xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl""#;

pub fn resp_imaging_settings(state: &SharedState) -> String {
    let s = state.read();
    let img = &s.imaging;
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
            <tt:Focus><tt:AutoFocusMode>{focus}</tt:AutoFocusMode></tt:Focus>
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
            focus = img.focus_mode,
            ir = img.ir_cut_filter,
            sharpness = img.sharpness,
            wdr_mode = img.wide_dynamic_range_mode,
            wdr_level = img.wide_dynamic_range_level,
            wb = img.white_balance_mode,
        ),
    )
}

pub fn handle_set_imaging_settings(state: &SharedState, body: &str) -> String {
    state.modify(|s| {
        if let Some(v) = extract_tag(body, "Brightness").and_then(|v| v.parse().ok()) {
            s.imaging.brightness = v;
        }
        if let Some(v) = extract_tag(body, "ColorSaturation").and_then(|v| v.parse().ok()) {
            s.imaging.color_saturation = v;
        }
        if let Some(v) = extract_tag(body, "Contrast").and_then(|v| v.parse().ok()) {
            s.imaging.contrast = v;
        }
        if let Some(v) = extract_tag(body, "Sharpness").and_then(|v| v.parse().ok()) {
            s.imaging.sharpness = v;
        }
        if let Some(v) = extract_tag(body, "IrCutFilter") {
            s.imaging.ir_cut_filter = v;
        }
        // oxvif sends each mode as a flat XML field, extract by context
        if let Some(v) = extract_tag(body, "WhiteBalanceMode") {
            s.imaging.white_balance_mode = v;
        }
        if let Some(v) = extract_tag(body, "ExposureMode") {
            s.imaging.exposure_mode = v;
        }
        if let Some(v) = extract_tag(body, "BacklightCompensationMode") {
            s.imaging.backlight_compensation = v;
        }
        if let Some(v) = extract_tag(body, "WideDynamicRangeMode") {
            s.imaging.wide_dynamic_range_mode = v;
        }
        if let Some(v) = extract_tag(body, "WideDynamicRangeLevel").and_then(|v| v.parse().ok()) {
            s.imaging.wide_dynamic_range_level = v;
        }
        if let Some(v) = extract_tag(body, "AutoFocusMode") {
            s.imaging.focus_mode = v;
        }
        eprintln!("    [STATE] imaging settings updated");
    });
    resp_empty("timg", "SetImagingSettingsResponse")
}

pub fn resp_imaging_options() -> String {
    soap(
        NS,
        r#"<timg:GetOptionsResponse>
          <timg:ImagingOptions>
            <tt:Brightness><tt:Min>0</tt:Min><tt:Max>100</tt:Max></tt:Brightness>
            <tt:ColorSaturation><tt:Min>0</tt:Min><tt:Max>100</tt:Max></tt:ColorSaturation>
            <tt:Contrast><tt:Min>0</tt:Min><tt:Max>100</tt:Max></tt:Contrast>
            <tt:Sharpness><tt:Min>0</tt:Min><tt:Max>100</tt:Max></tt:Sharpness>
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
            <tt:Focus>
              <tt:AFModes>AUTO</tt:AFModes>
              <tt:AFModes>MANUAL</tt:AFModes>
            </tt:Focus>
            <tt:WideDynamicRange>
              <tt:Mode>OFF</tt:Mode>
              <tt:Mode>ON</tt:Mode>
              <tt:Level><tt:Min>0</tt:Min><tt:Max>100</tt:Max></tt:Level>
            </tt:WideDynamicRange>
            <tt:BacklightCompensation>
              <tt:Mode>OFF</tt:Mode>
              <tt:Mode>ON</tt:Mode>
            </tt:BacklightCompensation>
          </timg:ImagingOptions>
        </timg:GetOptionsResponse>"#,
    )
}

pub fn resp_imaging_status() -> String {
    soap(
        NS,
        r#"<timg:GetStatusResponse>
          <timg:Status>
            <tt:FocusStatus20>
              <tt:Position>0.5</tt:Position>
              <tt:MoveStatus>IDLE</tt:MoveStatus>
            </tt:FocusStatus20>
          </timg:Status>
        </timg:GetStatusResponse>"#,
    )
}

pub fn resp_imaging_move_options() -> String {
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

// ── GetServiceCapabilities ───────────────────────────────────────────────────

/// `timg:Capabilities`.
///
/// `AdaptablePreset` is spelled exactly as the schema has it — singular,
/// "Adaptable". The mock implements no imaging presets, so `Presets` is a
/// real `false` here rather than an omission.
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
