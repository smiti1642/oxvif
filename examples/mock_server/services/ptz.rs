use crate::helpers::soap;

pub fn resp_ptz_status() -> String {
    soap(
        r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
        r#"<tptz:GetStatusResponse>
          <tptz:PTZStatus>
            <tt:Position>
              <tt:PanTilt x="0.1" y="-0.2" space="http://www.onvif.org/ver10/tptz/PanTiltSpaces/PositionGenericSpace"/>
              <tt:Zoom x="0.0" space="http://www.onvif.org/ver10/tptz/ZoomSpaces/PositionGenericSpace"/>
            </tt:Position>
            <tt:MoveStatus>
              <tt:PanTilt>IDLE</tt:PanTilt>
              <tt:Zoom>IDLE</tt:Zoom>
            </tt:MoveStatus>
          </tptz:PTZStatus>
        </tptz:GetStatusResponse>"#,
    )
}

pub fn resp_ptz_presets() -> String {
    soap(
        r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
        r#"<tptz:GetPresetsResponse>
          <tptz:Preset token="Preset_1">
            <tt:Name>Home</tt:Name>
            <tt:PTZPosition>
              <tt:PanTilt x="0.0" y="0.0" space="http://www.onvif.org/ver10/tptz/PanTiltSpaces/PositionGenericSpace"/>
              <tt:Zoom x="0.0" space="http://www.onvif.org/ver10/tptz/ZoomSpaces/PositionGenericSpace"/>
            </tt:PTZPosition>
          </tptz:Preset>
          <tptz:Preset token="Preset_2">
            <tt:Name>Door</tt:Name>
            <tt:PTZPosition>
              <tt:PanTilt x="0.5" y="0.2" space="http://www.onvif.org/ver10/tptz/PanTiltSpaces/PositionGenericSpace"/>
            </tt:PTZPosition>
          </tptz:Preset>
        </tptz:GetPresetsResponse>"#,
    )
}

pub fn resp_ptz_set_preset() -> String {
    soap(
        r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
        r#"<tptz:SetPresetResponse>
          <tptz:PresetToken>Preset_3</tptz:PresetToken>
        </tptz:SetPresetResponse>"#,
    )
}

pub fn resp_ptz_nodes() -> String {
    soap(
        r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
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
        r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
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
        r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
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
        r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
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
        r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
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
