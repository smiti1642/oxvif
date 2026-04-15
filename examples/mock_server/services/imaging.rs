use crate::helpers::soap;

pub fn resp_imaging_settings() -> String {
    soap(
        r#"xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl""#,
        r#"<timg:GetImagingSettingsResponse>
          <timg:ImagingSettings>
            <tt:Brightness>60</tt:Brightness>
            <tt:ColorSaturation>50</tt:ColorSaturation>
            <tt:Contrast>45</tt:Contrast>
            <tt:Sharpness>30</tt:Sharpness>
            <tt:IrCutFilter>AUTO</tt:IrCutFilter>
            <tt:WhiteBalance><tt:Mode>AUTO</tt:Mode></tt:WhiteBalance>
            <tt:Exposure><tt:Mode>MANUAL</tt:Mode></tt:Exposure>
          </timg:ImagingSettings>
        </timg:GetImagingSettingsResponse>"#,
    )
}

pub fn resp_imaging_options() -> String {
    soap(
        r#"xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl""#,
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
          </timg:ImagingOptions>
        </timg:GetOptionsResponse>"#,
    )
}

pub fn resp_imaging_status() -> String {
    soap(
        r#"xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl""#,
        r#"<timg:GetStatusResponse>
          <timg:Status>
            <tt:FocusStatus20 xmlns:tt="http://www.onvif.org/ver10/schema">
              <tt:Position>0.5</tt:Position>
              <tt:MoveStatus>IDLE</tt:MoveStatus>
            </tt:FocusStatus20>
          </timg:Status>
        </timg:GetStatusResponse>"#,
    )
}

pub fn resp_imaging_move_options() -> String {
    soap(
        r#"xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl""#,
        r#"<timg:GetMoveOptionsResponse>
          <timg:MoveOptions>
            <tt:Absolute xmlns:tt="http://www.onvif.org/ver10/schema">
              <tt:PositionSpace><tt:Min>0.0</tt:Min><tt:Max>1.0</tt:Max></tt:PositionSpace>
              <tt:SpeedSpace><tt:Min>0.0</tt:Min><tt:Max>1.0</tt:Max></tt:SpeedSpace>
            </tt:Absolute>
            <tt:Continuous xmlns:tt="http://www.onvif.org/ver10/schema">
              <tt:SpeedSpace><tt:Min>-1.0</tt:Min><tt:Max>1.0</tt:Max></tt:SpeedSpace>
            </tt:Continuous>
          </timg:MoveOptions>
        </timg:GetMoveOptionsResponse>"#,
    )
}
