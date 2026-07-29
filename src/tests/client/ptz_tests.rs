//! Unit tests for the PTZ methods on `OnvifClient` (`src/client/ptz.rs`).

use super::*;
use crate::tests::common::*;
// The preset-tour types the tour tests construct. `use super::*` only reaches
// what `src/client/ptz.rs` itself imports, which is the response types and not
// the members they are built from.
use crate::types::{
    PtzPresetTourDirection, PtzPresetTourPresetDetail, PtzPresetTourSpot,
    PtzPresetTourStartingCondition, PtzPresetTourState, PtzPresetTourStatus,
};
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

// ── Preset tours ──────────────────────────────────────────────────────────────
//
// The `<tptz:PresetTour>` shape below matches what
// `crate::mock::services::ptz::tour_xml` emits (src/mock/services/ptz.rs) —
// feature-gated there, so keep the two in step by hand.

fn preset_tours_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tptz:GetPresetToursResponse>
              <tptz:PresetTour token="Tour_1">
                <tt:Name>Perimeter</tt:Name>
                <tt:Status>
                  <tt:State>Idle</tt:State>
                </tt:Status>
                <tt:AutoStart>false</tt:AutoStart>
                <tt:StartingCondition RandomPresetOrder="false">
                  <tt:RecurringTime>3</tt:RecurringTime>
                  <tt:Direction>Forward</tt:Direction>
                </tt:StartingCondition>
                <tt:TourSpot>
                  <tt:PresetDetail>
                    <tt:PresetToken>Preset_1</tt:PresetToken>
                  </tt:PresetDetail>
                  <tt:StayTime>PT10S</tt:StayTime>
                </tt:TourSpot>
                <tt:TourSpot>
                  <tt:PresetDetail>
                    <tt:PresetToken>Preset_2</tt:PresetToken>
                  </tt:PresetDetail>
                  <tt:StayTime>PT20S</tt:StayTime>
                </tt:TourSpot>
              </tptz:PresetTour>
              <tptz:PresetTour>
                <tt:Status>
                  <tt:State>Touring</tt:State>
                  <tt:CurrentTourSpot>
                    <tt:PresetDetail>
                      <tt:Home>true</tt:Home>
                    </tt:PresetDetail>
                  </tt:CurrentTourSpot>
                </tt:Status>
                <tt:AutoStart>true</tt:AutoStart>
                <tt:StartingCondition>
                  <tt:Direction>Panoramic</tt:Direction>
                </tt:StartingCondition>
                <tt:TourSpot>
                  <tt:PresetDetail>
                    <tt:PTZPosition>
                      <tt:PanTilt x="0.5" y="-0.25"/>
                      <tt:Zoom x="0.75"/>
                    </tt:PTZPosition>
                  </tt:PresetDetail>
                  <tt:Speed>
                    <tt:PanTilt x="0.4" y="0.4"/>
                  </tt:Speed>
                </tt:TourSpot>
              </tptz:PresetTour>
            </tptz:GetPresetToursResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn ptz_get_preset_tours_parses_both_tours_and_all_spots() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(preset_tours_xml()));

    let tours = client
        .ptz_get_preset_tours("http://192.168.1.1/onvif/ptz", "Profile_1")
        .await
        .unwrap();

    assert_eq!(tours.len(), 2);

    let t0 = &tours[0];
    assert_eq!(t0.token.as_deref(), Some("Tour_1"));
    assert_eq!(t0.name.as_deref(), Some("Perimeter"));
    assert_eq!(t0.status.state, PtzPresetTourState::Idle);
    assert!(t0.status.current_tour_spot.is_none());
    assert!(!t0.auto_start);

    // `RandomPresetOrder` is an attribute, not a child element. A parser
    // reading it as a child sees `None` here and still passes every other
    // line, so this is the assertion that pins it.
    assert_eq!(t0.starting_condition.random_preset_order, Some(false));
    assert_eq!(t0.starting_condition.recurring_time, Some(3));
    assert_eq!(
        t0.starting_condition.direction,
        Some(PtzPresetTourDirection::Forward)
    );

    // Two spots, and the second one is what proves the whole list is read.
    assert_eq!(t0.tour_spots.len(), 2);
    match &t0.tour_spots[1].preset_detail {
        PtzPresetTourPresetDetail::PresetToken(t) => assert_eq!(t, "Preset_2"),
        other => panic!("expected PresetToken, got {other:?}"),
    }
    assert_eq!(t0.tour_spots[1].stay_time.as_deref(), Some("PT20S"));

    let t1 = &tours[1];
    // `@token` is `[0..1]` on `tt:PresetTour`, unlike `tt:PTZPreset/@token`.
    // A tour with no token is schema-valid and must not be an error.
    assert_eq!(t1.token, None);
    assert_eq!(t1.name, None);
    assert_eq!(t1.status.state, PtzPresetTourState::Touring);

    // An unrecognised direction is carried, not rejected — a vendor string
    // must not turn GetPresetTours into an Err.
    assert_eq!(
        t1.starting_condition.direction,
        Some(PtzPresetTourDirection::Unknown("Panoramic".into()))
    );

    // The other two arms of the `xs:choice`.
    let current = t1
        .status
        .current_tour_spot
        .as_ref()
        .expect("CurrentTourSpot");
    assert!(matches!(
        current.preset_detail,
        PtzPresetTourPresetDetail::Home
    ));
    match &t1.tour_spots[0].preset_detail {
        PtzPresetTourPresetDetail::Position { pan_tilt, zoom } => {
            assert_eq!(*pan_tilt, Some((0.5, -0.25)));
            assert_eq!(*zoom, Some(0.75));
        }
        other => panic!("expected Position, got {other:?}"),
    }
    let speed = t1.tour_spots[0].speed.as_ref().expect("Speed");
    assert_eq!(speed.pan_tilt, Some((0.4, 0.4)));
    assert_eq!(speed.zoom, None);
}

#[tokio::test]
async fn ptz_get_preset_tours_propagates_a_bad_second_tour() {
    // One valid tour and one missing `Status`. The whole call must fail — a
    // `vec_from_xml` that returned the first tour and dropped the second would
    // pass a one-element fixture and this is what catches it.
    //
    // `Status` rather than `@token` on purpose: `@token` is optional here, so
    // omitting it is *valid* and would prove nothing.
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tptz:GetPresetToursResponse>
              <tptz:PresetTour token="Tour_1">
                <tt:Status><tt:State>Idle</tt:State></tt:Status>
                <tt:AutoStart>false</tt:AutoStart>
                <tt:StartingCondition/>
              </tptz:PresetTour>
              <tptz:PresetTour token="Tour_2">
                <tt:AutoStart>false</tt:AutoStart>
                <tt:StartingCondition/>
              </tptz:PresetTour>
            </tptz:GetPresetToursResponse>
          </s:Body>
        </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let err = client
        .ptz_get_preset_tours("http://192.168.1.1/onvif/ptz", "Profile_1")
        .await
        .unwrap_err();
    assert_missing_field(err, "PresetTour/Status");
}

#[tokio::test]
async fn ptz_get_preset_tour_fault() {
    let xml = make_soap_fault_xml("ter:InvalidArgVal", "NoSuchPresetTour-tour-4471");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));

    let err = client
        .ptz_get_preset_tour("http://192.168.1.1/onvif/ptz", "Profile_1", "Tour_9")
        .await
        .unwrap_err();
    assert_fault(err, "ter:InvalidArgVal", "NoSuchPresetTour-tour-4471");
}

fn preset_tour_options_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tptz:GetPresetTourOptionsResponse>
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
                    <tt:PresetToken>Preset_1</tt:PresetToken>
                    <tt:PresetToken>Preset_2</tt:PresetToken>
                    <tt:Home>true</tt:Home>
                  </tt:PresetDetail>
                  <tt:StayTime>
                    <tt:Min>PT5S</tt:Min>
                    <tt:Max>PT10M</tt:Max>
                  </tt:StayTime>
                </tt:TourSpot>
              </tptz:Options>
            </tptz:GetPresetTourOptionsResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn ptz_get_preset_tour_options_reads_the_direction_list() {
    let (transport, captured) = RecordingTransport::new(preset_tour_options_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    let opts = client
        .ptz_get_preset_tour_options("http://192.168.1.1/onvif/ptz", "Profile_1", None)
        .await
        .unwrap();

    assert!(opts.auto_start);

    // The cardinality asymmetry: `Direction` is `[0..*]` here and a single
    // value inside a concrete `StartingCondition`. A parser using `child()`
    // instead of `children_named()` returns one item and passes everything
    // else.
    assert_eq!(
        opts.starting_condition.directions,
        [
            PtzPresetTourDirection::Forward,
            PtzPresetTourDirection::Backward
        ]
    );
    let rt = opts
        .starting_condition
        .recurring_time
        .expect("RecurringTime");
    assert_eq!((rt.min, rt.max), (1, 10));
    assert_eq!(
        opts.starting_condition.recurring_duration,
        Some(("PT1M".to_string(), "PT8H".to_string()))
    );

    assert_eq!(
        opts.tour_spot.preset_detail.preset_tokens,
        ["Preset_1", "Preset_2"]
    );
    assert_eq!(opts.tour_spot.preset_detail.home, Some(true));
    assert!(
        opts.tour_spot
            .preset_detail
            .pan_tilt_position_space
            .is_none()
    );
    assert_eq!(
        opts.tour_spot.stay_time,
        ("PT5S".to_string(), "PT10M".to_string())
    );

    // Passing `None` for the tour token must omit the element entirely, not
    // send an empty one — a device reads an empty token as "this tour".
    let c = captured.lock().unwrap();
    assert!(!c.body.contains("PresetTourToken"), "body was: {}", c.body);
}

#[tokio::test]
async fn ptz_get_preset_tour_options_missing_stay_time() {
    // `StayTime` is `minOccurs="1"` on `tt:PTZPresetTourSpotOptions`.
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tptz:GetPresetTourOptionsResponse>
              <tptz:Options>
                <tt:AutoStart>true</tt:AutoStart>
                <tt:StartingCondition/>
                <tt:TourSpot>
                  <tt:PresetDetail>
                    <tt:PresetToken>Preset_1</tt:PresetToken>
                  </tt:PresetDetail>
                </tt:TourSpot>
              </tptz:Options>
            </tptz:GetPresetTourOptionsResponse>
          </s:Body>
        </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let err = client
        .ptz_get_preset_tour_options("http://192.168.1.1/onvif/ptz", "Profile_1", Some("Tour_1"))
        .await
        .unwrap_err();
    assert_missing_field(err, "Options/TourSpot/StayTime");
}

#[tokio::test]
async fn ptz_create_preset_tour_returns_the_new_token() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl">
          <s:Body>
            <tptz:CreatePresetTourResponse>
              <tptz:PresetTourToken>Tour_2</tptz:PresetTourToken>
            </tptz:CreatePresetTourResponse>
          </s:Body>
        </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    let token = client
        .ptz_create_preset_tour("http://192.168.1.1/onvif/ptz", "Profile_1")
        .await
        .unwrap();
    assert_eq!(token, "Tour_2");

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver20/ptz/wsdl/CreatePresetTour"
    );
    assert!(
        c.body
            .contains("<tptz:ProfileToken>Profile_1</tptz:ProfileToken>"),
        "body was: {}",
        c.body
    );
}

#[tokio::test]
async fn ptz_create_preset_tour_missing_token() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl">
          <s:Body>
            <tptz:CreatePresetTourResponse/>
          </s:Body>
        </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let err = client
        .ptz_create_preset_tour("http://192.168.1.1/onvif/ptz", "Profile_1")
        .await
        .unwrap_err();
    assert_missing_field(err, "PresetTourToken");
}

#[tokio::test]
async fn ptz_modify_preset_tour_escapes_and_serialises_every_spot() {
    let (transport, captured) = RecordingTransport::new(&empty_response_xml("dummy"));
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    let tour = PtzPresetTour {
        token: Some("Tour_1".into()),
        // `ModifyPresetTour` is the only Tier 1 operation that writes
        // structured user data, so the name is where escaping has teeth.
        name: Some("Lobby & <Gate>".into()),
        status: PtzPresetTourStatus {
            state: PtzPresetTourState::Idle,
            current_tour_spot: None,
        },
        auto_start: true,
        starting_condition: PtzPresetTourStartingCondition {
            random_preset_order: Some(true),
            recurring_time: Some(4),
            recurring_duration: Some("PT30M".into()),
            direction: Some(PtzPresetTourDirection::Backward),
        },
        tour_spots: vec![
            PtzPresetTourSpot {
                preset_detail: PtzPresetTourPresetDetail::PresetToken("A&B".into()),
                speed: None,
                stay_time: Some("PT10S".into()),
            },
            PtzPresetTourSpot {
                preset_detail: PtzPresetTourPresetDetail::Home,
                speed: None,
                stay_time: None,
            },
        ],
    };

    let res = client
        .ptz_modify_preset_tour("http://192.168.1.1/onvif/ptz", "Profile_1", &tour)
        .await;
    // The fixture is an unrelated response element, so the call must report
    // that rather than silently succeeding.
    assert!(
        matches!(
            res,
            Err(crate::error::OnvifError::Soap(
                crate::soap::SoapError::UnexpectedResponse(ref t)
            )) if t == "ModifyPresetTourResponse"
        ),
        "got {res:?}"
    );

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver20/ptz/wsdl/ModifyPresetTour"
    );
    assert!(
        c.body
            .contains("<tt:Name>Lobby &amp; &lt;Gate&gt;</tt:Name>"),
        "name must be escaped: {}",
        c.body
    );
    assert!(
        c.body.contains("<tt:PresetToken>A&amp;B</tt:PresetToken>"),
        "preset token must be escaped: {}",
        c.body
    );
    assert!(
        c.body.contains(r#"<tptz:PresetTour token="Tour_1">"#),
        "body was: {}",
        c.body
    );
    assert!(
        c.body
            .contains(r#"<tt:StartingCondition RandomPresetOrder="true">"#),
        "RandomPresetOrder is an attribute: {}",
        c.body
    );
    assert!(
        c.body.contains("<tt:Direction>Backward</tt:Direction>"),
        "body was: {}",
        c.body
    );
    // Both spots, and the `Home` arm of the choice serialised on its own —
    // a `PresetToken` alongside it would be schema-invalid.
    assert_eq!(c.body.matches("<tt:TourSpot>").count(), 2);
    assert!(
        c.body.contains("<tt:Home>true</tt:Home>"),
        "body: {}",
        c.body
    );
}

#[tokio::test]
async fn ptz_operate_preset_tour_sends_the_operation_verb() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl">
          <s:Body><tptz:OperatePresetTourResponse/></s:Body>
        </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .ptz_operate_preset_tour(
            "http://192.168.1.1/onvif/ptz",
            "Profile_1",
            "Tour_1",
            PtzPresetTourOperation::Pause,
        )
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver20/ptz/wsdl/OperatePresetTour"
    );
    assert!(
        c.body.contains("<tptz:Operation>Pause</tptz:Operation>"),
        "body was: {}",
        c.body
    );
    assert!(
        c.body
            .contains("<tptz:PresetTourToken>Tour_1</tptz:PresetTourToken>"),
        "body was: {}",
        c.body
    );
}

#[tokio::test]
async fn ptz_operate_preset_tour_fault() {
    let xml = make_soap_fault_xml("env:Sender", "TourNotRunning-5106");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));

    let err = client
        .ptz_operate_preset_tour(
            "http://192.168.1.1/onvif/ptz",
            "Profile_1",
            "Tour_1",
            PtzPresetTourOperation::Start,
        )
        .await
        .unwrap_err();
    assert_fault(err, "env:Sender", "TourNotRunning-5106");
}

#[tokio::test]
async fn ptz_remove_preset_tour_sends_both_tokens() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl">
          <s:Body><tptz:RemovePresetTourResponse/></s:Body>
        </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .ptz_remove_preset_tour("http://192.168.1.1/onvif/ptz", "Profile_1", "Tour_1")
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver20/ptz/wsdl/RemovePresetTour"
    );
    assert!(
        c.body
            .contains("<tptz:ProfileToken>Profile_1</tptz:ProfileToken>"),
        "body was: {}",
        c.body
    );
    assert!(
        c.body
            .contains("<tptz:PresetTourToken>Tour_1</tptz:PresetTourToken>"),
        "body was: {}",
        c.body
    );
}

#[tokio::test]
async fn ptz_remove_preset_tour_fault() {
    let xml = make_soap_fault_xml("ter:NotAuthorized", "TourDeleteDenied-2884");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));

    let err = client
        .ptz_remove_preset_tour("http://192.168.1.1/onvif/ptz", "Profile_1", "Tour_1")
        .await
        .unwrap_err();
    assert_fault(err, "ter:NotAuthorized", "TourDeleteDenied-2884");
}

// ── ptz_send_auxiliary_command ────────────────────────────────────────────────
//
// The PTZ operation, not the Device one exercised in `device_tests.rs`. The two
// share a name and nothing else: different endpoint, different request element
// (`AuxiliaryData` vs `AuxiliaryCommand`), different response element
// (`AuxiliaryResponse` vs `AuxiliaryCommandResponse`). Wiring either one to the
// other's element names parses to an empty string rather than failing, so the
// assertions below name the exact elements.

#[tokio::test]
async fn ptz_send_auxiliary_command_returns_the_device_answer() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl">
          <s:Body>
            <tptz:SendAuxiliaryCommandResponse>
              <tptz:AuxiliaryResponse>tt:Wiper|On accepted</tptz:AuxiliaryResponse>
            </tptz:SendAuxiliaryCommandResponse>
          </s:Body>
        </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    let answer = client
        .ptz_send_auxiliary_command("http://192.168.1.1/onvif/ptz", "Profile_1", "tt:Wiper|On")
        .await
        .unwrap();
    assert_eq!(answer, "tt:Wiper|On accepted");

    let c = captured.lock().unwrap();
    // ver20/ptz, not ver10/device — the whole point of the second method.
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver20/ptz/wsdl/SendAuxiliaryCommand"
    );
    assert!(
        c.body
            .contains("<tptz:ProfileToken>Profile_1</tptz:ProfileToken>"),
        "the PTZ operation is per-profile: {}",
        c.body
    );
    assert!(
        c.body
            .contains("<tptz:AuxiliaryData>tt:Wiper|On</tptz:AuxiliaryData>"),
        "element is AuxiliaryData, not AuxiliaryCommand: {}",
        c.body
    );
}

#[tokio::test]
async fn ptz_send_auxiliary_command_escapes_the_command() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl">
          <s:Body>
            <tptz:SendAuxiliaryCommandResponse>
              <tptz:AuxiliaryResponse>OK</tptz:AuxiliaryResponse>
            </tptz:SendAuxiliaryCommandResponse>
          </s:Body>
        </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .ptz_send_auxiliary_command(
            "http://192.168.1.1/onvif/ptz",
            "Profile&1",
            "tt:Wiper|On&<Off>",
        )
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert!(
        c.body
            .contains("<tptz:AuxiliaryData>tt:Wiper|On&amp;&lt;Off&gt;</tptz:AuxiliaryData>"),
        "command must be escaped: {}",
        c.body
    );
    assert!(
        c.body
            .contains("<tptz:ProfileToken>Profile&amp;1</tptz:ProfileToken>"),
        "profile token must be escaped: {}",
        c.body
    );
}

#[tokio::test]
async fn ptz_send_auxiliary_command_missing_response_element() {
    // `AuxiliaryResponse` is `minOccurs="1"`. Reporting it as missing beats
    // handing back an empty string that reads as "the device said nothing".
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl">
          <s:Body>
            <tptz:SendAuxiliaryCommandResponse/>
          </s:Body>
        </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let err = client
        .ptz_send_auxiliary_command("http://192.168.1.1/onvif/ptz", "Profile_1", "tt:Wiper|On")
        .await
        .unwrap_err();
    assert_missing_field(err, "AuxiliaryResponse");
}

#[tokio::test]
async fn ptz_send_auxiliary_command_fault() {
    let xml = make_soap_fault_xml("ter:InvalidArgVal", "NoAuxiliaryCommand-ptz-6620");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));

    let err = client
        .ptz_send_auxiliary_command("http://192.168.1.1/onvif/ptz", "Profile_1", "tt:Nope|On")
        .await
        .unwrap_err();
    assert_fault(err, "ter:InvalidArgVal", "NoAuxiliaryCommand-ptz-6620");
}
