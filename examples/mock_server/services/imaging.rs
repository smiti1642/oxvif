use crate::helpers::{resp_empty, soap};
use crate::state::SharedState;
use crate::xml_parse::extract_tag;

const NS: &str = r#"xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl""#;

pub fn resp_imaging_settings(state: &SharedState) -> String {
    let s = state.read();
    let img = &s.imaging;
    soap(
        NS,
        &format!(
            r#"<timg:GetImagingSettingsResponse>
          <timg:ImagingSettings>
            <tt:Brightness>{}</tt:Brightness>
            <tt:ColorSaturation>{}</tt:ColorSaturation>
            <tt:Contrast>{}</tt:Contrast>
            <tt:Sharpness>{}</tt:Sharpness>
            <tt:IrCutFilter>{}</tt:IrCutFilter>
            <tt:WhiteBalance><tt:Mode>{}</tt:Mode></tt:WhiteBalance>
            <tt:Exposure><tt:Mode>{}</tt:Mode></tt:Exposure>
            <tt:BacklightCompensation><tt:Mode>{}</tt:Mode></tt:BacklightCompensation>
            <tt:WideDynamicRange><tt:Mode>{}</tt:Mode><tt:Level>{}</tt:Level></tt:WideDynamicRange>
            <tt:Focus><tt:AutoFocusMode>{}</tt:AutoFocusMode></tt:Focus>
          </timg:ImagingSettings>
        </timg:GetImagingSettingsResponse>"#,
            img.brightness,
            img.color_saturation,
            img.contrast,
            img.sharpness,
            img.ir_cut_filter,
            img.white_balance_mode,
            img.exposure_mode,
            img.backlight_compensation,
            img.wide_dynamic_range_mode,
            img.wide_dynamic_range_level,
            img.focus_mode,
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
