//! Unit tests for the PTZ methods on `OnvifClient` (`src/client/ptz.rs`).

use super::*;
use crate::tests::common::*;
use std::sync::Arc;

// ── PTZ preset / status fixtures ──────────────────────────────────────────

fn ptz_set_preset_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl">
          <s:Body>
            <tptz:SetPresetResponse>
              <tptz:PresetToken>Preset_3</tptz:PresetToken>
            </tptz:SetPresetResponse>
          </s:Body>
        </s:Envelope>"#
}

fn ptz_remove_preset_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl">
          <s:Body>
            <tptz:RemovePresetResponse/>
          </s:Body>
        </s:Envelope>"#
}

fn ptz_get_status_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tptz:GetStatusResponse>
              <tptz:PTZStatus>
                <tt:Position>
                  <tt:PanTilt x="0.5" y="-0.25"/>
                  <tt:Zoom x="0.1"/>
                </tt:Position>
                <tt:MoveStatus>
                  <tt:PanTilt>IDLE</tt:PanTilt>
                  <tt:Zoom>IDLE</tt:Zoom>
                </tt:MoveStatus>
              </tptz:PTZStatus>
            </tptz:GetStatusResponse>
          </s:Body>
        </s:Envelope>"#
}

fn ptz_get_status_no_position_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tptz:GetStatusResponse>
              <tptz:PTZStatus>
                <tt:MoveStatus>
                  <tt:PanTilt>MOVING</tt:PanTilt>
                  <tt:Zoom>IDLE</tt:Zoom>
                </tt:MoveStatus>
              </tptz:PTZStatus>
            </tptz:GetStatusResponse>
          </s:Body>
        </s:Envelope>"#
}

// ── ptz_set_preset ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_ptz_set_preset_returns_token() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(ptz_set_preset_xml()));

    let token = client
        .ptz_set_preset(
            "http://192.168.1.1/onvif/ptz_service",
            "Profile_1",
            Some("Entrance"),
            None,
        )
        .await
        .unwrap();

    assert_eq!(token, "Preset_3");
}

#[tokio::test]
async fn test_ptz_set_preset_embeds_name_and_optional_token() {
    let (transport, captured) = RecordingTransport::new(ptz_set_preset_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .ptz_set_preset(
            "http://192.168.1.1/onvif/ptz_service",
            "Profile_1",
            Some("Entrance"),
            Some("Preset_3"),
        )
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("Entrance"), "preset name must be in request");
    assert!(body.contains("Preset_3"), "preset token must be in request");
}

#[tokio::test]
async fn test_ptz_set_preset_without_name_or_token() {
    let (transport, captured) = RecordingTransport::new(ptz_set_preset_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .ptz_set_preset(
            "http://192.168.1.1/onvif/ptz_service",
            "Profile_1",
            None,
            None,
        )
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(
        !body.contains("PresetName"),
        "optional PresetName must be absent"
    );
    assert!(
        !body.contains("PresetToken"),
        "optional PresetToken must be absent"
    );
}

// ── ptz_remove_preset ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_ptz_remove_preset_embeds_tokens() {
    let (transport, captured) = RecordingTransport::new(ptz_remove_preset_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .ptz_remove_preset(
            "http://192.168.1.1/onvif/ptz_service",
            "Profile_1",
            "Preset_3",
        )
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("Profile_1"));
    assert!(body.contains("Preset_3"));
}

// ── ptz_get_status ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_ptz_get_status_parses_position_and_move_status() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(ptz_get_status_xml()));

    let status = client
        .ptz_get_status("http://192.168.1.1/onvif/ptz_service", "Profile_1")
        .await
        .unwrap();

    assert!((status.pan.unwrap() - 0.5).abs() < 1e-5);
    assert!((status.tilt.unwrap() - (-0.25)).abs() < 1e-5);
    assert!((status.zoom.unwrap() - 0.1).abs() < 1e-5);
    assert_eq!(status.pan_tilt_status, "IDLE");
    assert_eq!(status.zoom_status, "IDLE");
}

#[tokio::test]
async fn test_ptz_get_status_no_position_is_none() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(ptz_get_status_no_position_xml()));

    let status = client
        .ptz_get_status("http://192.168.1.1/onvif/ptz_service", "Profile_1")
        .await
        .unwrap();

    assert!(status.pan.is_none());
    assert!(status.tilt.is_none());
    assert!(status.zoom.is_none());
    assert_eq!(status.pan_tilt_status, "MOVING");
}

// ── PTZ Configuration tests ───────────────────────────────────────────────────

fn get_ptz_configurations_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tptz:GetConfigurationsResponse>
             <tptz:PTZConfiguration token="PTZConfig_1">
               <tt:Name>PTZConfiguration_1</tt:Name>
               <tt:UseCount>1</tt:UseCount>
               <tt:NodeToken>PTZNode_1</tt:NodeToken>
               <tt:DefaultPTZTimeout>PT5S</tt:DefaultPTZTimeout>
             </tptz:PTZConfiguration>
           </tptz:GetConfigurationsResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_ptz_get_configurations_ok() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_ptz_configurations_xml()));
    let cfgs = client
        .ptz_get_configurations("http://192.168.1.1/onvif/ptz_service")
        .await
        .unwrap();
    assert_eq!(cfgs.len(), 1);
    assert_eq!(cfgs[0].token, "PTZConfig_1");
    assert_eq!(cfgs[0].node_token, "PTZNode_1");
    assert_eq!(cfgs[0].default_ptz_timeout.as_deref(), Some("PT5S"));
}

#[tokio::test]
async fn test_ptz_get_configurations_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                              xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                              xmlns:tt="http://www.onvif.org/ver10/schema">
                   <s:Body>
                     <tptz:GetConfigurationsResponse>
                       <tptz:PTZConfiguration>
                         <tt:Name>NoToken</tt:Name>
                       </tptz:PTZConfiguration>
                     </tptz:GetConfigurationsResponse>
                   </s:Body>
                 </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let result = client
        .ptz_get_configurations("http://192.168.1.1/onvif/ptz_service")
        .await;
    assert_missing_field(result.unwrap_err(), "PTZConfiguration/@token");
}

fn set_ptz_configuration_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl">
         <s:Body>
           <tptz:SetConfigurationResponse/>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_ptz_set_configuration_ok() {
    use crate::types::PtzConfiguration;
    let (transport, captured) = RecordingTransport::new(set_ptz_configuration_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    let cfg = PtzConfiguration {
        token: "PTZConfig_1".to_string(),
        name: "PTZConfiguration_1".to_string(),
        use_count: 1,
        node_token: "PTZNode_1".to_string(),
        default_ptz_timeout: Some("PT5S".to_string()),
        default_abs_pan_tilt_space: None,
        default_abs_zoom_space: None,
        default_rel_pan_tilt_space: None,
        default_rel_zoom_space: None,
        default_cont_pan_tilt_space: None,
        default_cont_zoom_space: None,
        default_ptz_speed: None,
        pan_tilt_limits: None,
        zoom_limits: None,
    };
    client
        .ptz_set_configuration("http://192.168.1.1/onvif/ptz_service", &cfg, true)
        .await
        .unwrap();
    let c = captured.lock().unwrap();
    assert!(c.body.contains("PTZConfig_1"));
    assert!(c.body.contains("PTZNode_1"));
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver20/ptz/wsdl/SetConfiguration"
    );
}

fn get_ptz_configuration_options_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tptz:GetConfigurationOptionsResponse>
             <tptz:PTZConfigurationOptions>
               <tt:PTZTimeout>
                 <tt:Min>PT0S</tt:Min>
                 <tt:Max>PT60S</tt:Max>
               </tt:PTZTimeout>
             </tptz:PTZConfigurationOptions>
           </tptz:GetConfigurationOptionsResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_ptz_get_configuration_options_ok() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_ptz_configuration_options_xml()));
    let opts = client
        .ptz_get_configuration_options("http://192.168.1.1/onvif/ptz_service", "PTZConfig_1")
        .await
        .unwrap();
    assert_eq!(opts.ptz_timeout_min.as_deref(), Some("PT0S"));
    assert_eq!(opts.ptz_timeout_max.as_deref(), Some("PT60S"));
}

fn get_ptz_nodes_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tptz:GetNodesResponse>
             <tptz:PTZNode token="PTZNode_1" FixedHomePosition="false">
               <tt:Name>PTZNode_1</tt:Name>
               <tt:MaximumNumberOfPresets>255</tt:MaximumNumberOfPresets>
               <tt:HomeSupported>true</tt:HomeSupported>
             </tptz:PTZNode>
           </tptz:GetNodesResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_ptz_get_nodes_ok() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_ptz_nodes_xml()));
    let nodes = client
        .ptz_get_nodes("http://192.168.1.1/onvif/ptz_service")
        .await
        .unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].token, "PTZNode_1");
    assert_eq!(nodes[0].max_presets, 255);
    assert!(nodes[0].home_supported);
    assert!(!nodes[0].fixed_home_position);
}

#[tokio::test]
async fn test_ptz_get_nodes_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                              xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                              xmlns:tt="http://www.onvif.org/ver10/schema">
                   <s:Body>
                     <tptz:GetNodesResponse>
                       <tptz:PTZNode>
                         <tt:Name>NoToken</tt:Name>
                       </tptz:PTZNode>
                     </tptz:GetNodesResponse>
                   </s:Body>
                 </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let result = client
        .ptz_get_nodes("http://192.168.1.1/onvif/ptz_service")
        .await;
    assert_missing_field(result.unwrap_err(), "PTZNode/@token");
}

// ── ptz_goto_home_position / ptz_set_home_position ────────────────────────────

#[tokio::test]
async fn test_ptz_goto_home_position_ok() {
    let xml = empty_response_xml("GotoHomePositionResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .ptz_goto_home_position("http://192.168.1.1/onvif/ptz", "Profile_1", None)
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver20/ptz/wsdl/GotoHomePosition"
    );
    assert!(c.body.contains("Profile_1"));
}

#[tokio::test]
async fn test_ptz_set_home_position_ok() {
    let xml = empty_response_xml("SetHomePositionResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .ptz_set_home_position("http://192.168.1.1/onvif/ptz", "Profile_1")
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver20/ptz/wsdl/SetHomePosition"
    );
    assert!(c.body.contains("Profile_1"));
}

// PtzNode SupportedPTZSpaces

#[tokio::test]
async fn test_ptz_get_nodes_parses_spaces() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tptz:GetNodesResponse>
             <tptz:PTZNode token="PTZNode_1" FixedHomePosition="false">
               <tt:Name>PTZNode_1</tt:Name>
               <tt:SupportedPTZSpaces>
                 <tt:AbsolutePanTiltPositionSpace>
                   <tt:URI>http://www.onvif.org/ver10/tptz/PanTiltSpaces/PositionGenericSpace</tt:URI>
                   <tt:XRange><tt:Min>-1</tt:Min><tt:Max>1</tt:Max></tt:XRange>
                   <tt:YRange><tt:Min>-1</tt:Min><tt:Max>1</tt:Max></tt:YRange>
                 </tt:AbsolutePanTiltPositionSpace>
                 <tt:AbsoluteZoomPositionSpace>
                   <tt:URI>http://www.onvif.org/ver10/tptz/ZoomSpaces/PositionGenericSpace</tt:URI>
                   <tt:XRange><tt:Min>0</tt:Min><tt:Max>1</tt:Max></tt:XRange>
                 </tt:AbsoluteZoomPositionSpace>
               </tt:SupportedPTZSpaces>
               <tt:MaximumNumberOfPresets>100</tt:MaximumNumberOfPresets>
               <tt:HomeSupported>true</tt:HomeSupported>
             </tptz:PTZNode>
           </tptz:GetNodesResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let nodes = client
        .ptz_get_nodes("http://192.168.1.1/onvif/ptz_service")
        .await
        .unwrap();
    assert_eq!(nodes[0].pan_tilt_spaces.len(), 1);
    assert_eq!(nodes[0].zoom_spaces.len(), 1);
    assert!(nodes[0].pan_tilt_spaces[0].uri.contains("PanTilt"));
    assert_eq!(nodes[0].pan_tilt_spaces[0].x_range, (-1.0, 1.0));
    assert!(nodes[0].pan_tilt_spaces[0].y_range.is_some());
    assert!(nodes[0].zoom_spaces[0].y_range.is_none());
}

// PtzStatus utc_time

#[tokio::test]
async fn test_ptz_get_status_parses_utc_time() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tptz:GetStatusResponse>
             <tptz:PTZStatus>
               <tt:Position>
                 <tt:PanTilt x="0.0" y="0.0"/>
                 <tt:Zoom x="0.0"/>
               </tt:Position>
               <tt:MoveStatus>
                 <tt:PanTilt>IDLE</tt:PanTilt>
                 <tt:Zoom>IDLE</tt:Zoom>
               </tt:MoveStatus>
               <tt:UtcTime>2024-06-15T12:00:00Z</tt:UtcTime>
             </tptz:PTZStatus>
           </tptz:GetStatusResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let status = client
        .ptz_get_status("http://192.168.1.1/onvif/ptz_service", "Profile_1")
        .await
        .unwrap();
    assert_eq!(status.utc_time.as_deref(), Some("2024-06-15T12:00:00Z"));
}

#[tokio::test]
async fn test_ptz_get_configuration_parses_default_spaces() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tptz:GetConfigurationResponse>
             <tptz:PTZConfiguration token="PTZConfig_1">
               <tt:Name>PTZConfig</tt:Name>
               <tt:UseCount>1</tt:UseCount>
               <tt:NodeToken>PTZNode_1</tt:NodeToken>
               <tt:DefaultAbsolutePanTiltPositionSpace>http://www.onvif.org/ver10/tptz/PanTiltSpaces/PositionGenericSpace</tt:DefaultAbsolutePanTiltPositionSpace>
               <tt:DefaultAbsoluteZoomPositionSpace>http://www.onvif.org/ver10/tptz/ZoomSpaces/PositionGenericSpace</tt:DefaultAbsoluteZoomPositionSpace>
               <tt:DefaultRelativePanTiltTranslationSpace>http://www.onvif.org/ver10/tptz/PanTiltSpaces/TranslationGenericSpace</tt:DefaultRelativePanTiltTranslationSpace>
               <tt:DefaultRelativeZoomTranslationSpace>http://www.onvif.org/ver10/tptz/ZoomSpaces/TranslationGenericSpace</tt:DefaultRelativeZoomTranslationSpace>
               <tt:DefaultContinuousPanTiltVelocitySpace>http://www.onvif.org/ver10/tptz/PanTiltSpaces/VelocityGenericSpace</tt:DefaultContinuousPanTiltVelocitySpace>
               <tt:DefaultContinuousZoomVelocitySpace>http://www.onvif.org/ver10/tptz/ZoomSpaces/VelocityGenericSpace</tt:DefaultContinuousZoomVelocitySpace>
               <tt:DefaultPTZSpeed>
                 <tt:PanTilt x="0.5" y="0.5"/>
                 <tt:Zoom x="0.5"/>
               </tt:DefaultPTZSpeed>
             </tptz:PTZConfiguration>
           </tptz:GetConfigurationResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let cfg = client
        .ptz_get_configuration("http://192.168.1.1/onvif/ptz_service", "PTZConfig_1")
        .await
        .unwrap();
    assert!(
        cfg.default_abs_pan_tilt_space
            .as_deref()
            .unwrap()
            .contains("PanTilt")
    );
    assert!(
        cfg.default_abs_zoom_space
            .as_deref()
            .unwrap()
            .contains("Zoom")
    );
    assert!(cfg.default_rel_pan_tilt_space.is_some());
    assert!(cfg.default_cont_pan_tilt_space.is_some());
    let speed = cfg.default_ptz_speed.expect("speed should be present");
    assert_eq!(speed.pan_tilt, Some((0.5, 0.5)));
    assert!((speed.zoom.unwrap() - 0.5).abs() < 1e-5);
}

// ── Direction-4 new-field coverage tests ─────────────────────────────────────

#[tokio::test]
async fn test_ptz_get_status_parses_error() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tptz:GetStatusResponse>
             <tptz:PTZStatus>
               <tt:MoveStatus>
                 <tt:PanTilt>IDLE</tt:PanTilt>
                 <tt:Zoom>IDLE</tt:Zoom>
               </tt:MoveStatus>
               <tt:Error>ObstacleDetected</tt:Error>
             </tptz:PTZStatus>
           </tptz:GetStatusResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let status = client
        .ptz_get_status("http://192.168.1.1/onvif/ptz_service", "Profile_1")
        .await
        .unwrap();
    assert_eq!(status.error.as_deref(), Some("ObstacleDetected"));
    assert!(status.utc_time.is_none());
}

#[tokio::test]
async fn test_ptz_get_status_no_error_is_none() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tptz:GetStatusResponse>
             <tptz:PTZStatus>
               <tt:MoveStatus>
                 <tt:PanTilt>IDLE</tt:PanTilt>
                 <tt:Zoom>IDLE</tt:Zoom>
               </tt:MoveStatus>
             </tptz:PTZStatus>
           </tptz:GetStatusResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let status = client
        .ptz_get_status("http://192.168.1.1/onvif/ptz_service", "Profile_1")
        .await
        .unwrap();
    assert!(status.error.is_none());
}

// ── PTZ GetNode ───────────────────────────────────────────────────────────

fn ptz_node_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                    xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tptz:GetNodeResponse>
              <tptz:PTZNode token="PTZNode_1" FixedHomePosition="false">
                <tt:Name>PTZNode</tt:Name>
                <tt:SupportedPTZSpaces/>
                <tt:MaximumNumberOfPresets>100</tt:MaximumNumberOfPresets>
                <tt:HomeSupported>true</tt:HomeSupported>
              </tptz:PTZNode>
            </tptz:GetNodeResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_ptz_get_node_parses_response() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(ptz_node_xml()));

    let node = client
        .ptz_get_node("http://192.168.1.1/onvif/ptz_service", "PTZNode_1")
        .await
        .unwrap();

    assert_eq!(node.token, "PTZNode_1");
    assert!(node.home_supported);
}

#[tokio::test]
async fn test_ptz_get_node_sends_token() {
    let (transport, captured) = RecordingTransport::new(ptz_node_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .ptz_get_node("http://192.168.1.1/onvif/ptz_service", "PTZNode_1")
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("PTZNode_1"));
}

// ── PTZ GetCompatibleConfigurations ───────────────────────────────────────

#[tokio::test]
async fn test_ptz_get_compatible_configurations_sends_profile_token() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                    xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tptz:GetCompatibleConfigurationsResponse>
              <tptz:PTZConfiguration token="PTZConfig_1">
                <tt:Name>PTZConfig</tt:Name>
                <tt:UseCount>1</tt:UseCount>
                <tt:NodeToken>PTZNode_1</tt:NodeToken>
                <tt:DefaultPTZTimeout>PT10S</tt:DefaultPTZTimeout>
              </tptz:PTZConfiguration>
            </tptz:GetCompatibleConfigurationsResponse>
          </s:Body>
        </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    let configs = client
        .ptz_get_compatible_configurations("http://192.168.1.1/onvif/ptz_service", "Profile_1")
        .await
        .unwrap();

    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].token, "PTZConfig_1");

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("Profile_1"));
}

// ── PTZ movement commands (void writes: pin action + exact body) ──────────

#[tokio::test]
async fn test_ptz_absolute_move_pins_action_and_body() {
    let xml = empty_response_xml("AbsoluteMoveResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .ptz_absolute_move(
            "http://192.168.1.1/onvif/ptz_service",
            "Profile_1",
            0.5,
            -0.25,
            0.125,
        )
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.action, "http://www.onvif.org/ver20/ptz/wsdl/AbsoluteMove");
    assert!(
        c.body.contains(
            "<tptz:AbsoluteMove>\
               <tptz:ProfileToken>Profile_1</tptz:ProfileToken>\
               <tptz:Position>\
                 <tt:PanTilt x=\"0.5\" y=\"-0.25\"/>\
                 <tt:Zoom x=\"0.125\"/>\
               </tptz:Position>\
             </tptz:AbsoluteMove>"
        ),
        "AbsoluteMove body was: {}",
        c.body
    );
}

#[tokio::test]
async fn test_ptz_relative_move_pins_action_and_body() {
    let xml = empty_response_xml("RelativeMoveResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .ptz_relative_move(
            "http://192.168.1.1/onvif/ptz_service",
            "Profile_2",
            -0.75,
            0.375,
            -0.0625,
        )
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.action, "http://www.onvif.org/ver20/ptz/wsdl/RelativeMove");
    assert!(
        c.body.contains(
            "<tptz:RelativeMove>\
               <tptz:ProfileToken>Profile_2</tptz:ProfileToken>\
               <tptz:Translation>\
                 <tt:PanTilt x=\"-0.75\" y=\"0.375\"/>\
                 <tt:Zoom x=\"-0.0625\"/>\
               </tptz:Translation>\
             </tptz:RelativeMove>"
        ),
        "RelativeMove body was: {}",
        c.body
    );
}

#[tokio::test]
async fn test_ptz_continuous_move_pins_action_and_body() {
    let xml = empty_response_xml("ContinuousMoveResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .ptz_continuous_move(
            "http://192.168.1.1/onvif/ptz_service",
            "Profile_3",
            0.25,
            -0.5,
            0.75,
        )
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver20/ptz/wsdl/ContinuousMove"
    );
    assert!(
        c.body.contains(
            "<tptz:ContinuousMove>\
               <tptz:ProfileToken>Profile_3</tptz:ProfileToken>\
               <tptz:Velocity>\
                 <tt:PanTilt x=\"0.25\" y=\"-0.5\"/>\
                 <tt:Zoom x=\"0.75\"/>\
               </tptz:Velocity>\
             </tptz:ContinuousMove>"
        ),
        "ContinuousMove body was: {}",
        c.body
    );
}

/// Client-level positive for `OnvifClient::ptz_stop`. The only pre-existing
/// call site is `session_tests.rs`, whose subject is `OnvifSession`
/// delegation and which asserts nothing about the wire request.
#[tokio::test]
async fn test_ptz_stop_pins_action_and_body() {
    let xml = empty_response_xml("StopResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .ptz_stop("http://192.168.1.1/onvif/ptz_service", "Profile_4")
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.action, "http://www.onvif.org/ver20/ptz/wsdl/Stop");
    assert!(
        c.body.contains(
            "<tptz:Stop>\
               <tptz:ProfileToken>Profile_4</tptz:ProfileToken>\
               <tptz:PanTilt>true</tptz:PanTilt>\
               <tptz:Zoom>true</tptz:Zoom>\
             </tptz:Stop>"
        ),
        "Stop body was: {}",
        c.body
    );
}

#[tokio::test]
async fn test_ptz_goto_preset_pins_action_and_body() {
    let xml = empty_response_xml("GotoPresetResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .ptz_goto_preset(
            "http://192.168.1.1/onvif/ptz_service",
            "Profile_5",
            "Preset_11",
        )
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.action, "http://www.onvif.org/ver20/ptz/wsdl/GotoPreset");
    assert!(
        c.body.contains(
            "<tptz:GotoPreset>\
               <tptz:ProfileToken>Profile_5</tptz:ProfileToken>\
               <tptz:PresetToken>Preset_11</tptz:PresetToken>\
             </tptz:GotoPreset>"
        ),
        "GotoPreset body was: {}",
        c.body
    );
}

// ── ptz_get_presets ───────────────────────────────────────────────────────

fn ptz_get_presets_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                    xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tptz:GetPresetsResponse>
              <tptz:Preset token="Preset_7">
                <tt:Name>Gate-North-7</tt:Name>
                <tt:PTZPosition>
                  <tt:PanTilt x="0.25" y="-0.5"/>
                  <tt:Zoom x="0.75"/>
                </tt:PTZPosition>
              </tptz:Preset>
              <tptz:Preset token="Preset_9">
                <tt:Name>Lobby-9</tt:Name>
              </tptz:Preset>
            </tptz:GetPresetsResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_ptz_get_presets_parses_tokens_names_and_positions() {
    let (transport, captured) = RecordingTransport::new(ptz_get_presets_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    let presets = client
        .ptz_get_presets("http://192.168.1.1/onvif/ptz_service", "Profile_6")
        .await
        .unwrap();

    assert_eq!(presets.len(), 2);

    assert_eq!(presets[0].token, "Preset_7");
    assert_eq!(presets[0].name, "Gate-North-7");
    assert_eq!(presets[0].pan_tilt, Some((0.25, -0.5)));
    assert_eq!(presets[0].zoom, Some(0.75));

    assert_eq!(presets[1].token, "Preset_9");
    assert_eq!(presets[1].name, "Lobby-9");
    assert!(presets[1].pan_tilt.is_none());
    assert!(presets[1].zoom.is_none());

    let c = captured.lock().unwrap();
    assert_eq!(c.action, "http://www.onvif.org/ver20/ptz/wsdl/GetPresets");
    assert!(
        c.body.contains(
            "<tptz:GetPresets>\
               <tptz:ProfileToken>Profile_6</tptz:ProfileToken>\
             </tptz:GetPresets>"
        ),
        "GetPresets body was: {}",
        c.body
    );
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn mock_get_compatible_configurations_response_parses_via_client() {
    let client = OnvifClient::new("http://mock/onvif/device")
        .with_transport(Arc::new(crate::mock::MockTransport::new()));
    // The mock must answer with <GetCompatibleConfigurationsResponse>, not reuse
    // <GetConfigurationsResponse> — the client parser matches the former.
    let configs = client
        .ptz_get_compatible_configurations("http://mock/onvif/ptz", "Profile_1")
        .await
        .unwrap();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].token, "PTZConfig_1");
}

// ── ptz_get_service_capabilities ──────────────────────────────────────────────
//
// `<tptz:Capabilities>` copied verbatim from
// `crate::mock::services::ptz::resp_ptz_service_capabilities`
// (src/mock/services/ptz.rs) — feature-gated there, so keep in step by hand.
// `tests/mock_service_capabilities.rs` is what actually proves the two equal.

fn ptz_service_capabilities_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl">
          <s:Body>
            <tptz:GetServiceCapabilitiesResponse>
              <tptz:Capabilities GetCompatibleConfigurations="true"
                                 MoveStatus="true"
                                 StatusPosition="true"
                                 MoveAndTrack="PresetToken PTZVector"/>
            </tptz:GetServiceCapabilitiesResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn ptz_service_capabilities_parses_move_and_track() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(ptz_service_capabilities_xml()));

    let caps = client
        .ptz_get_service_capabilities("http://192.168.1.1/onvif/ptz")
        .await
        .unwrap();

    // `MoveAndTrack` is the spelling the schema uses. `MoveAndStartTracking`
    // is an operation name, not an attribute, and would parse as an empty
    // list here rather than failing.
    assert_eq!(caps.move_and_track, ["PresetToken", "PTZVector"]);

    // The richest omitted/denied pair in the stage: two `None`s next to three
    // `Some(true)`s in one struct.
    assert_eq!(caps.eflip, None);
    assert_eq!(caps.reverse, None);
    assert_eq!(caps.move_status, Some(true));
    assert_eq!(caps.status_position, Some(true));
    assert_eq!(caps.get_compatible_configurations, Some(true));
}

#[tokio::test]
async fn ptz_service_capabilities_fault() {
    let xml = make_soap_fault_xml("env:Receiver", "PtzCapsUnavailable-9318");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));

    let err = client
        .ptz_get_service_capabilities("http://192.168.1.1/onvif/ptz")
        .await
        .unwrap_err();
    assert_fault(err, "env:Receiver", "PtzCapsUnavailable-9318");
}
