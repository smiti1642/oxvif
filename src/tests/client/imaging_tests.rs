//! Unit tests for the Imaging methods on `OnvifClient`
//! (`src/client/imaging.rs`).

use super::*;
use crate::tests::common::*;

// ── Imaging service fixtures ──────────────────────────────────────────────

fn imaging_settings_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <timg:GetImagingSettingsResponse>
              <timg:ImagingSettings>
                <tt:Brightness>60</tt:Brightness>
                <tt:ColorSaturation>50</tt:ColorSaturation>
                <tt:Contrast>45</tt:Contrast>
                <tt:Sharpness>30</tt:Sharpness>
                <tt:IrCutFilter>AUTO</tt:IrCutFilter>
                <tt:WhiteBalance><tt:Mode>AUTO</tt:Mode></tt:WhiteBalance>
                <tt:Exposure><tt:Mode>MANUAL</tt:Mode></tt:Exposure>
              </timg:ImagingSettings>
            </timg:GetImagingSettingsResponse>
          </s:Body>
        </s:Envelope>"#
}

fn imaging_options_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <timg:GetOptionsResponse>
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
            </timg:GetOptionsResponse>
          </s:Body>
        </s:Envelope>"#
}

// ── get_imaging_settings ──────────────────────────────────────────────────

#[tokio::test]
async fn test_get_imaging_settings_parses_all_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(imaging_settings_xml()));

    let s = client
        .get_imaging_settings("http://192.168.1.1/onvif/imaging_service", "VS_1")
        .await
        .unwrap();

    assert!((s.brightness.unwrap() - 60.0).abs() < 1e-5);
    assert!((s.color_saturation.unwrap() - 50.0).abs() < 1e-5);
    assert!((s.contrast.unwrap() - 45.0).abs() < 1e-5);
    assert!((s.sharpness.unwrap() - 30.0).abs() < 1e-5);
    assert_eq!(s.ir_cut_filter.as_deref(), Some("AUTO"));
    assert_eq!(s.white_balance_mode.as_deref(), Some("AUTO"));
    assert_eq!(s.exposure_mode.as_deref(), Some("MANUAL"));
}

#[tokio::test]
async fn test_get_imaging_settings_sends_source_token() {
    let (transport, captured) = RecordingTransport::new(imaging_settings_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .get_imaging_settings("http://192.168.1.1/onvif/imaging_service", "VS_1")
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("VS_1"));
    assert_eq!(
        captured.lock().unwrap().action,
        "http://www.onvif.org/ver20/imaging/wsdl/GetImagingSettings"
    );
}

// ── set_imaging_settings ──────────────────────────────────────────────────

#[tokio::test]
async fn test_set_imaging_settings_serialises_fields() {
    let set_xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl">
          <s:Body><timg:SetImagingSettingsResponse/></s:Body>
        </s:Envelope>"#;

    let (transport, captured) = RecordingTransport::new(set_xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    let settings = crate::types::ImagingSettings {
        brightness: Some(70.0),
        ir_cut_filter: Some("OFF".into()),
        ..Default::default()
    };

    client
        .set_imaging_settings(
            "http://192.168.1.1/onvif/imaging_service",
            "VS_1",
            &settings,
        )
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("<tt:Brightness>70</tt:Brightness>"));
    assert!(body.contains("<tt:IrCutFilter>OFF</tt:IrCutFilter>"));
    assert!(body.contains("VS_1"));
    assert!(body.contains("ForcePersistence"));
}

// ── get_imaging_options ───────────────────────────────────────────────────

#[tokio::test]
async fn test_get_imaging_options_parses_ranges_and_modes() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(imaging_options_xml()));

    let opts = client
        .get_imaging_options("http://192.168.1.1/onvif/imaging_service", "VS_1")
        .await
        .unwrap();

    let br = opts.brightness.unwrap();
    assert!((br.min - 0.0).abs() < 1e-5);
    assert!((br.max - 100.0).abs() < 1e-5);
    assert_eq!(opts.ir_cut_filter_modes, ["ON", "OFF", "AUTO"]);
    assert_eq!(opts.white_balance_modes, ["AUTO", "MANUAL"]);
    assert_eq!(opts.exposure_modes, ["AUTO", "MANUAL"]);
}

// ── imaging_move / imaging_stop ───────────────────────────────────────────────

fn imaging_get_status_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl">
          <s:Body>
            <timg:GetStatusResponse>
              <timg:Status>
                <tt:FocusStatus20 xmlns:tt="http://www.onvif.org/ver10/schema">
                  <tt:Position>0.5</tt:Position>
                  <tt:MoveStatus>IDLE</tt:MoveStatus>
                </tt:FocusStatus20>
              </timg:Status>
            </timg:GetStatusResponse>
          </s:Body>
        </s:Envelope>"#
}

fn imaging_move_options_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl">
          <s:Body>
            <timg:GetMoveOptionsResponse>
              <timg:MoveOptions>
                <tt:Absolute xmlns:tt="http://www.onvif.org/ver10/schema">
                  <tt:PositionSpace><tt:Min>0.0</tt:Min><tt:Max>1.0</tt:Max></tt:PositionSpace>
                  <tt:SpeedSpace><tt:Min>0.0</tt:Min><tt:Max>1.0</tt:Max></tt:SpeedSpace>
                </tt:Absolute>
                <tt:Continuous xmlns:tt="http://www.onvif.org/ver10/schema">
                  <tt:SpeedSpace><tt:Min>-1.0</tt:Min><tt:Max>1.0</tt:Max></tt:SpeedSpace>
                </tt:Continuous>
              </timg:MoveOptions>
            </timg:GetMoveOptionsResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_imaging_get_status_parses_focus() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(imaging_get_status_xml()));

    let status = client
        .imaging_get_status("http://192.168.1.1/onvif/imaging", "video_source")
        .await
        .unwrap();

    assert!((status.focus_position.unwrap() - 0.5).abs() < 0.001);
    assert_eq!(status.focus_move_status, "IDLE");
}

#[tokio::test]
async fn test_imaging_get_status_missing_status_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
                   <s:Body><timg:GetStatusResponse/></s:Body>
                 </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let err = client
        .imaging_get_status("http://192.168.1.1/onvif/imaging", "video_source")
        .await
        .unwrap_err();

    assert_missing_field(err, "Status");
}

#[tokio::test]
async fn test_imaging_move_sends_absolute_body() {
    let xml = empty_response_xml("MoveResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .imaging_move(
            "http://192.168.1.1/onvif/imaging",
            "video_source",
            &crate::FocusMove::Absolute {
                position: 0.8,
                speed: None,
            },
        )
        .await
        .unwrap();

    assert!(captured.lock().unwrap().body.contains("0.8"));
}

#[tokio::test]
async fn test_imaging_move_soap_fault_returns_err() {
    let xml = make_soap_fault_xml("s:Sender", "ter:NoFocus");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));

    let result = client
        .imaging_move(
            "http://192.168.1.1/onvif/imaging",
            "video_source",
            &crate::FocusMove::Continuous { speed: 0.5 },
        )
        .await;

    assert!(
        matches!(
            result,
            Err(OnvifError::Soap(crate::soap::SoapError::Fault { ref code, ref reason, .. }))
            if code == "s:Sender" && reason == "ter:NoFocus"
        ),
        "expected SOAP Fault error, got: {result:?}"
    );
}

#[tokio::test]
async fn test_imaging_stop_sends_source_token_and_action() {
    let xml = empty_response_xml("StopResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .imaging_stop("http://192.168.1.1/onvif/imaging", "video_source")
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert!(
        c.body
            .contains("<timg:VideoSourceToken>video_source</timg:VideoSourceToken>")
    );
    assert_eq!(c.action, "http://www.onvif.org/ver20/imaging/wsdl/Stop");
}

#[tokio::test]
async fn test_imaging_stop_soap_fault_returns_err() {
    let xml = make_soap_fault_xml("s:Sender", "ter:NoSource");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));

    let result = client
        .imaging_stop("http://192.168.1.1/onvif/imaging", "video_source")
        .await;

    assert!(
        matches!(
            result,
            Err(OnvifError::Soap(crate::soap::SoapError::Fault { ref code, ref reason, .. }))
            if code == "s:Sender" && reason == "ter:NoSource"
        ),
        "expected SOAP Fault error, got: {result:?}"
    );
}

// Move and Stop used to share one made-up response tag, `<timg:ImagingResponse/>`,
// which exists in no ONVIF WSDL — so both failed with UnexpectedResponse. Each
// must now get its own per-operation response element.

#[cfg(feature = "mock")]
#[tokio::test]
async fn mock_imaging_move_response_parses_via_client() {
    let client = OnvifClient::new("http://mock/onvif/device")
        .with_transport(std::sync::Arc::new(crate::mock::MockTransport::new()));
    client
        .imaging_move(
            "http://mock/onvif/imaging",
            "VideoSource_1",
            &crate::FocusMove::Continuous { speed: 0.5 },
        )
        .await
        .expect("mock must answer <timg:MoveResponse/>");
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn mock_imaging_stop_response_parses_via_client() {
    let client = OnvifClient::new("http://mock/onvif/device")
        .with_transport(std::sync::Arc::new(crate::mock::MockTransport::new()));
    client
        .imaging_stop("http://mock/onvif/imaging", "VideoSource_1")
        .await
        .expect("mock must answer <timg:StopResponse/>");
}

#[tokio::test]
async fn test_imaging_get_move_options_parses_ranges() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(imaging_move_options_xml()));

    let opts = client
        .imaging_get_move_options("http://192.168.1.1/onvif/imaging", "video_source")
        .await
        .unwrap();

    let abs = opts.absolute_position_range.unwrap();
    assert!((abs.min - 0.0).abs() < 0.001);
    assert!((abs.max - 1.0).abs() < 0.001);
    let cont = opts.continuous_speed_range.unwrap();
    assert!((cont.min - -1.0).abs() < 0.001);
}

#[tokio::test]
async fn test_imaging_get_move_options_missing_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
                   <s:Body><timg:GetMoveOptionsResponse/></s:Body>
                 </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let err = client
        .imaging_get_move_options("http://192.168.1.1/onvif/imaging", "video_source")
        .await
        .unwrap_err();

    assert_missing_field(err, "MoveOptions");
}

// ImagingSettings backlight_compensation

#[tokio::test]
async fn test_get_imaging_settings_parses_backlight_compensation() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <timg:GetImagingSettingsResponse>
             <timg:ImagingSettings>
               <tt:Brightness>50</tt:Brightness>
               <tt:BacklightCompensation>
                 <tt:Mode>ON</tt:Mode>
               </tt:BacklightCompensation>
             </timg:ImagingSettings>
           </timg:GetImagingSettingsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let s = client
        .get_imaging_settings("http://192.168.1.1/onvif/imaging_service", "VS_1")
        .await
        .unwrap();
    assert_eq!(s.backlight_compensation.as_deref(), Some("ON"));
}

#[tokio::test]
async fn test_get_imaging_settings_parses_focus_wdr() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <timg:GetImagingSettingsResponse>
             <timg:ImagingSettings>
               <tt:Focus>
                 <tt:AutoFocusMode>AUTO</tt:AutoFocusMode>
                 <tt:DefaultSpeed>0.5</tt:DefaultSpeed>
               </tt:Focus>
               <tt:WideDynamicRange>
                 <tt:Mode>ON</tt:Mode>
                 <tt:Level>50</tt:Level>
               </tt:WideDynamicRange>
               <tt:ImageStabilization>
                 <tt:Mode>ON</tt:Mode>
               </tt:ImageStabilization>
               <tt:ToneCompensation>
                 <tt:Mode>Auto</tt:Mode>
               </tt:ToneCompensation>
             </timg:ImagingSettings>
           </timg:GetImagingSettingsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let s = client
        .get_imaging_settings("http://192.168.1.1/onvif/imaging_service", "VS_1")
        .await
        .unwrap();
    assert_eq!(s.focus_mode.as_deref(), Some("AUTO"));
    assert!((s.focus_default_speed.unwrap() - 0.5).abs() < 1e-5);
    assert_eq!(s.wide_dynamic_range_mode.as_deref(), Some("ON"));
    assert!((s.wide_dynamic_range_level.unwrap() - 50.0).abs() < 1e-5);
    assert_eq!(s.image_stabilization_mode.as_deref(), Some("ON"));
    assert_eq!(s.tone_compensation_mode.as_deref(), Some("Auto"));
}

#[tokio::test]
async fn test_get_imaging_options_parses_exposure_ranges() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <timg:GetOptionsResponse>
             <timg:ImagingOptions>
               <tt:Brightness><tt:Min>0</tt:Min><tt:Max>100</tt:Max></tt:Brightness>
               <tt:Exposure>
                 <tt:Mode>AUTO</tt:Mode>
                 <tt:Mode>MANUAL</tt:Mode>
                 <tt:ExposureTime><tt:Min>0.0001</tt:Min><tt:Max>0.1</tt:Max></tt:ExposureTime>
                 <tt:Gain><tt:Min>0</tt:Min><tt:Max>40</tt:Max></tt:Gain>
                 <tt:Iris><tt:Min>1.4</tt:Min><tt:Max>22</tt:Max></tt:Iris>
               </tt:Exposure>
               <tt:Focus>
                 <tt:AutoFocusModes>AUTO</tt:AutoFocusModes>
                 <tt:AutoFocusModes>MANUAL</tt:AutoFocusModes>
                 <tt:DefaultSpeed><tt:Min>0</tt:Min><tt:Max>1</tt:Max></tt:DefaultSpeed>
               </tt:Focus>
               <tt:WideDynamicRange>
                 <tt:Mode>ON</tt:Mode>
                 <tt:Mode>OFF</tt:Mode>
                 <tt:Level><tt:Min>0</tt:Min><tt:Max>100</tt:Max></tt:Level>
               </tt:WideDynamicRange>
               <tt:BacklightCompensation>
                 <tt:Mode>ON</tt:Mode>
                 <tt:Mode>OFF</tt:Mode>
               </tt:BacklightCompensation>
             </timg:ImagingOptions>
           </timg:GetOptionsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let opts = client
        .get_imaging_options("http://192.168.1.1/onvif/imaging_service", "VS_1")
        .await
        .unwrap();
    let et = opts.exposure_time_range.expect("exposure_time_range");
    assert!((et.min - 0.0001).abs() < 1e-7);
    assert!((et.max - 0.1).abs() < 1e-7);
    let gain = opts.gain_range.expect("gain_range");
    assert!((gain.max - 40.0).abs() < 1e-5);
    let iris = opts.iris_range.expect("iris_range");
    assert!((iris.min - 1.4).abs() < 1e-5);
    assert_eq!(opts.focus_af_modes, ["AUTO", "MANUAL"]);
    let fs = opts.focus_speed_range.expect("focus_speed_range");
    assert!((fs.max - 1.0).abs() < 1e-5);
    let wdr = opts.wdr_level_range.expect("wdr_level_range");
    assert!((wdr.max - 100.0).abs() < 1e-5);
    assert_eq!(opts.wdr_modes, ["ON", "OFF"]);
    assert_eq!(opts.backlight_compensation_modes, ["ON", "OFF"]);
}

#[tokio::test]
async fn test_get_imaging_options_missing_optional_ranges_are_none() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <timg:GetOptionsResponse>
             <timg:ImagingOptions>
               <tt:Brightness><tt:Min>0</tt:Min><tt:Max>100</tt:Max></tt:Brightness>
             </timg:ImagingOptions>
           </timg:GetOptionsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let opts = client
        .get_imaging_options("http://192.168.1.1/onvif/imaging_service", "VS_1")
        .await
        .unwrap();
    assert!(opts.exposure_time_range.is_none());
    assert!(opts.gain_range.is_none());
    assert!(opts.iris_range.is_none());
    assert!(opts.focus_speed_range.is_none());
    assert!(opts.wdr_level_range.is_none());
    assert!(opts.focus_af_modes.is_empty());
    assert!(opts.wdr_modes.is_empty());
    assert!(opts.backlight_compensation_modes.is_empty());
}

// ── Real-camera regression: Exposure20 Min*/Max* options form ─────────────────
//
// Scrubbed captures (bodies are PII-free; the device IP lived only in the
// header, dropped here). Spec-compliant cameras report exposure bounds as
// `Min{X}`/`Max{X}` pairs, NOT a single legacy `{X}` — oxvif previously read the
// legacy name and returned `None` on these. Envelope = [Min{X}.Min, Max{X}.Max].

#[tokio::test]
async fn test_imaging_options_geovision_minmax_envelope() {
    // GeoVision GV-GBLF4813 (ONVIF v25.6).
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <timg:GetOptionsResponse><timg:ImagingOptions>
             <tt:Exposure>
               <tt:Mode>AUTO</tt:Mode>
               <tt:MinExposureTime><tt:Min>10</tt:Min><tt:Max>1000000</tt:Max></tt:MinExposureTime>
               <tt:MaxExposureTime><tt:Min>10</tt:Min><tt:Max>1000000</tt:Max></tt:MaxExposureTime>
               <tt:MinGain><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:MinGain>
               <tt:MaxGain><tt:Min>0</tt:Min><tt:Max>100</tt:Max></tt:MaxGain>
               <tt:MinIris><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:MinIris>
               <tt:MaxIris><tt:Min>13</tt:Min><tt:Max>13</tt:Max></tt:MaxIris>
             </tt:Exposure>
           </timg:ImagingOptions></timg:GetOptionsResponse>
         </s:Body></s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.0.2.10/onvif/device_service").with_transport(mock(xml));
    let opts = client
        .get_imaging_options("http://192.0.2.10/onvif/imaging_service", "VS_1")
        .await
        .unwrap();
    let et = opts.exposure_time_range.expect("exposure_time_range");
    assert_eq!((et.min, et.max), (10.0, 1_000_000.0));
    let gain = opts.gain_range.expect("gain_range");
    assert_eq!((gain.min, gain.max), (0.0, 100.0));
    let iris = opts.iris_range.expect("iris_range");
    assert_eq!((iris.min, iris.max), (0.0, 13.0));
}

#[tokio::test]
async fn test_imaging_options_hikvision_no_iris() {
    // Hikvision iDS-2CD7A26G0-IZHS: Min*/Max* exposure + gain, NO iris.
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <timg:GetOptionsResponse><timg:ImagingOptions>
             <tt:Exposure>
               <tt:Mode>AUTO</tt:Mode>
               <tt:MinExposureTime><tt:Min>10</tt:Min><tt:Max>10</tt:Max></tt:MinExposureTime>
               <tt:MaxExposureTime><tt:Min>10</tt:Min><tt:Max>1000000</tt:Max></tt:MaxExposureTime>
               <tt:MinGain><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:MinGain>
               <tt:MaxGain><tt:Min>0</tt:Min><tt:Max>100</tt:Max></tt:MaxGain>
             </tt:Exposure>
           </timg:ImagingOptions></timg:GetOptionsResponse>
         </s:Body></s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.0.2.11/onvif/device_service").with_transport(mock(xml));
    let opts = client
        .get_imaging_options("http://192.0.2.11/onvif/imaging_service", "VS_1")
        .await
        .unwrap();
    let et = opts.exposure_time_range.expect("exposure_time_range");
    assert_eq!((et.min, et.max), (10.0, 1_000_000.0));
    assert_eq!(opts.gain_range.expect("gain_range").max, 100.0);
    // No iris ranges at all (no Min*/Max* and no legacy) → None, correctly.
    assert!(opts.iris_range.is_none());
}

// ── imaging_get_service_capabilities ──────────────────────────────────────────
//
// `<timg:Capabilities>` copied verbatim from
// `crate::mock::services::imaging::resp_imaging_service_capabilities`
// (src/mock/services/imaging.rs) — feature-gated there, so keep in step by hand.

fn imaging_service_capabilities_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl">
          <s:Body>
            <timg:GetServiceCapabilitiesResponse>
              <timg:Capabilities ImageStabilization="false"
                                 Presets="false"
                                 AdaptablePreset="false"/>
            </timg:GetServiceCapabilitiesResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn imaging_service_capabilities_parses_three_flags() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(imaging_service_capabilities_xml()));

    let caps = client
        .imaging_get_service_capabilities("http://192.168.1.1/onvif/imaging")
        .await
        .unwrap();

    // Three attributes is the complete schema, and all three are present, so
    // this service has no `None` row of its own. `AdaptablePreset` is the
    // verified spelling — singular, "Adaptable". The plausible-looking
    // `AdaptivePresets` would make this `None` and only this assertion says so.
    assert_eq!(caps.adaptable_preset, Some(false));
    assert_eq!(caps.presets, Some(false));
    assert_eq!(caps.image_stabilization, Some(false));
}

#[tokio::test]
async fn imaging_service_capabilities_fault() {
    let xml = make_soap_fault_xml("ter:NotAuthorized", "ImagingCapsDenied-5527");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));

    let err = client
        .imaging_get_service_capabilities("http://192.168.1.1/onvif/imaging")
        .await
        .unwrap_err();
    assert_fault(err, "ter:NotAuthorized", "ImagingCapsDenied-5527");
}
