//! Unit tests for the Media2 (ver20) methods on `OnvifClient`
//! (`src/client/media2.rs`).

use super::*;
use crate::tests::common::*;

// ── Media2 fixtures ───────────────────────────────────────────────────────

fn profiles_media2_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tr2:GetProfilesResponse>
              <tr2:Profiles token="Profile_A" fixed="true">
                <tt:Name>mainStream</tt:Name>
                <tr2:Configurations>
                  <tr2:VideoSource token="VSC_1"/>
                  <tr2:VideoEncoder token="VEC_1"/>
                </tr2:Configurations>
              </tr2:Profiles>
              <tr2:Profiles token="Profile_B" fixed="false">
                <tt:Name>subStream</tt:Name>
              </tr2:Profiles>
            </tr2:GetProfilesResponse>
          </s:Body>
        </s:Envelope>"#
}

fn stream_uri_media2_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tr2="http://www.onvif.org/ver20/media/wsdl">
          <s:Body>
            <tr2:GetStreamUriResponse>
              <tr2:Uri>rtsp://192.168.1.1:554/h265/ch1</tr2:Uri>
            </tr2:GetStreamUriResponse>
          </s:Body>
        </s:Envelope>"#
}

fn video_encoder_configurations_media2_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tr2:GetVideoEncoderConfigurationsResponse>
              <tr2:Configurations token="VEC_H265">
                <tt:Name>H265Stream</tt:Name>
                <tt:UseCount>1</tt:UseCount>
                <tt:Encoding>H265</tt:Encoding>
                <tt:Resolution><tt:Width>3840</tt:Width><tt:Height>2160</tt:Height></tt:Resolution>
                <tt:Quality>7</tt:Quality>
                <tt:RateControl>
                  <tt:FrameRateLimit>30</tt:FrameRateLimit>
                  <tt:BitrateLimit>8192</tt:BitrateLimit>
                </tt:RateControl>
                <tt:GovLength>60</tt:GovLength>
                <tt:Profile>Main</tt:Profile>
              </tr2:Configurations>
            </tr2:GetVideoEncoderConfigurationsResponse>
          </s:Body>
        </s:Envelope>"#
}

fn video_encoder_configuration_options_media2_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tr2:GetVideoEncoderConfigurationOptionsResponse>
              <tr2:Options>
                <tt:Encoding>H264</tt:Encoding>
                <tt:QualityRange><tt:Min>1</tt:Min><tt:Max>10</tt:Max></tt:QualityRange>
                <tt:ResolutionsAvailable><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:ResolutionsAvailable>
                <tt:BitrateRange><tt:Min>32</tt:Min><tt:Max>16384</tt:Max></tt:BitrateRange>
                <tt:ProfilesSupported>Main</tt:ProfilesSupported>
              </tr2:Options>
              <tr2:Options>
                <tt:Encoding>H265</tt:Encoding>
                <tt:QualityRange><tt:Min>1</tt:Min><tt:Max>10</tt:Max></tt:QualityRange>
                <tt:ResolutionsAvailable><tt:Width>3840</tt:Width><tt:Height>2160</tt:Height></tt:ResolutionsAvailable>
                <tt:BitrateRange><tt:Min>64</tt:Min><tt:Max>32768</tt:Max></tt:BitrateRange>
                <tt:ProfilesSupported>Main</tt:ProfilesSupported>
                <tt:ProfilesSupported>Main10</tt:ProfilesSupported>
              </tr2:Options>
            </tr2:GetVideoEncoderConfigurationOptionsResponse>
          </s:Body>
        </s:Envelope>"#
}

fn video_encoder_instances_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tr2:GetVideoEncoderInstancesResponse>
              <tr2:Info>
                <tt:Total>4</tt:Total>
                <tt:Encoding>
                  <tt:Encoding>H264</tt:Encoding>
                  <tt:Number>2</tt:Number>
                </tt:Encoding>
                <tt:Encoding>
                  <tt:Encoding>H265</tt:Encoding>
                  <tt:Number>2</tt:Number>
                </tt:Encoding>
              </tr2:Info>
            </tr2:GetVideoEncoderInstancesResponse>
          </s:Body>
        </s:Envelope>"#
}

// ── Media2 tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_profiles_media2_returns_correct_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(profiles_media2_xml()));

    let profiles = client
        .get_profiles_media2("http://192.168.1.1/onvif/media2_service")
        .await
        .unwrap();

    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0].token, "Profile_A");
    assert_eq!(profiles[0].name, "mainStream");
    assert!(profiles[0].fixed);
    assert_eq!(profiles[1].token, "Profile_B");
    assert!(!profiles[1].fixed);
}

#[tokio::test]
async fn test_get_stream_uri_media2_returns_string() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(stream_uri_media2_xml()));

    let uri = client
        .get_stream_uri_media2("http://192.168.1.1/onvif/media2_service", "Profile_A")
        .await
        .unwrap();

    assert_eq!(uri, "rtsp://192.168.1.1:554/h265/ch1");
}

#[tokio::test]
async fn test_get_video_encoder_configurations_media2_parses_h265() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(video_encoder_configurations_media2_xml()));

    let cfgs = client
        .get_video_encoder_configurations_media2("http://192.168.1.1/onvif/media2_service")
        .await
        .unwrap();

    assert_eq!(cfgs.len(), 1);
    assert_eq!(cfgs[0].token, "VEC_H265");
    assert_eq!(cfgs[0].encoding, crate::types::VideoEncoding::H265);
    assert_eq!(cfgs[0].gov_length, Some(60));
    assert_eq!(cfgs[0].profile.as_deref(), Some("Main"));
    let rc = cfgs[0].rate_control.as_ref().unwrap();
    assert_eq!(rc.frame_rate_limit, 30);
    assert_eq!(rc.bitrate_limit, 8192);
}

#[tokio::test]
async fn test_get_video_encoder_configuration_options_media2_parses_options() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(video_encoder_configuration_options_media2_xml()));

    let opts = client
        .get_video_encoder_configuration_options_media2(
            "http://192.168.1.1/onvif/media2_service",
            None,
        )
        .await
        .unwrap();

    assert_eq!(opts.options.len(), 2);
    assert_eq!(opts.options[0].encoding, crate::types::VideoEncoding::H264);
    assert_eq!(opts.options[1].encoding, crate::types::VideoEncoding::H265);
    assert_eq!(opts.options[1].profiles.len(), 2);
}

#[tokio::test]
async fn test_get_video_encoder_instances_parses_total() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(video_encoder_instances_xml()));

    let inst = client
        .get_video_encoder_instances_media2("http://192.168.1.1/onvif/media2_service", "VSC_1")
        .await
        .unwrap();

    assert_eq!(inst.total, 4);
    assert_eq!(inst.encodings.len(), 2);
    assert_eq!(
        inst.encodings[0].encoding,
        crate::types::VideoEncoding::H264
    );
    assert_eq!(inst.encodings[0].number, 2);
}

// ── Round 2 new-field coverage tests ─────────────────────────────────────────

#[tokio::test]
async fn test_get_profiles_media2_parses_audio_ptz_tokens() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tr2:GetProfilesResponse>
             <tr2:Profiles token="Profile_1" fixed="false">
               <tt:Name>main</tt:Name>
               <tt:Configurations>
                 <tt:VideoSource token="VideoSrc_1"/>
                 <tt:VideoEncoder token="VideoEnc_1"/>
                 <tt:AudioSource token="AudioSrc_1"/>
                 <tt:Audio token="AudioEnc_1"/>
                 <tt:PTZ token="PTZConfig_1"/>
               </tt:Configurations>
             </tr2:Profiles>
           </tr2:GetProfilesResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let profiles = client
        .get_profiles_media2("http://192.168.1.1/onvif/media2_service")
        .await
        .unwrap();
    let p = &profiles[0];
    assert_eq!(p.audio_source_token.as_deref(), Some("AudioSrc_1"));
    assert_eq!(p.audio_encoder_token.as_deref(), Some("AudioEnc_1"));
    assert_eq!(p.ptz_config_token.as_deref(), Some("PTZConfig_1"));
}

// ── Media2 AddConfiguration / RemoveConfiguration ─────────────────────────

#[tokio::test]
async fn test_add_configuration_media2_sends_type_and_token() {
    let xml = empty_response_xml("AddConfigurationResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .add_configuration_media2(
            "http://192.168.1.1/onvif/media2",
            "Profile_1",
            "VideoEncoder",
            "VEC_1",
        )
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("Profile_1"));
    assert!(body.contains("VideoEncoder"));
    assert!(body.contains("VEC_1"));
}

#[tokio::test]
async fn test_remove_configuration_media2_sends_type_and_token() {
    let xml = empty_response_xml("RemoveConfigurationResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .remove_configuration_media2(
            "http://192.168.1.1/onvif/media2",
            "Profile_1",
            "Metadata",
            "MetaConf_1",
        )
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("Metadata"));
    assert!(body.contains("MetaConf_1"));
}

// ── Media2 Metadata configurations ────────────────────────────────────────

fn metadata_configs_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                    xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tr2:GetMetadataConfigurationsResponse>
              <tr2:Configurations token="MetaConf_1">
                <tt:Name>MetadataConfig</tt:Name>
                <tt:UseCount>1</tt:UseCount>
                <tt:Analytics>true</tt:Analytics>
                <tt:PTZStatus>
                  <tt:Status>false</tt:Status>
                  <tt:Position>true</tt:Position>
                </tt:PTZStatus>
              </tr2:Configurations>
            </tr2:GetMetadataConfigurationsResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_get_metadata_configurations_parses_response() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(metadata_configs_xml()));

    let configs = client
        .get_metadata_configurations_media2("http://192.168.1.1/onvif/media2", None, None)
        .await
        .unwrap();

    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].token, "MetaConf_1");
    assert!(configs[0].analytics);
    assert!(configs[0].ptz_position);
    assert!(!configs[0].ptz_status);
}

#[tokio::test]
async fn test_get_metadata_configurations_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tr2="http://www.onvif.org/ver20/media/wsdl">
          <s:Body>
            <tr2:GetMetadataConfigurationsResponse>
              <tr2:Configurations>
                <tt:Name xmlns:tt="http://www.onvif.org/ver10/schema">NoToken</tt:Name>
              </tr2:Configurations>
            </tr2:GetMetadataConfigurationsResponse>
          </s:Body>
        </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let err = client
        .get_metadata_configurations_media2("http://192.168.1.1/onvif/media2", None, None)
        .await
        .unwrap_err();
    assert!(matches!(err, crate::error::OnvifError::Soap(_)));
}

// ── Media2 Audio decoder / output configurations ──────────────────────────

fn audio_decoder_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                    xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tr2:GetAudioDecoderConfigurationsResponse>
              <tr2:Configurations token="ADC_1">
                <tt:Name>AudioDecoder</tt:Name>
                <tt:UseCount>1</tt:UseCount>
              </tr2:Configurations>
            </tr2:GetAudioDecoderConfigurationsResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_get_audio_decoder_configurations_parses_response() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(audio_decoder_xml()));

    let configs = client
        .get_audio_decoder_configurations_media2("http://192.168.1.1/onvif/media2")
        .await
        .unwrap();

    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].token, "ADC_1");
    assert_eq!(configs[0].name, "AudioDecoder");
}

fn audio_output_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                    xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tr2:GetAudioOutputConfigurationsResponse>
              <tr2:Configurations token="AOC_1">
                <tt:Name>AudioOutput</tt:Name>
                <tt:UseCount>1</tt:UseCount>
                <tt:OutputToken>AudioOut_1</tt:OutputToken>
                <tt:OutputLevel>50</tt:OutputLevel>
              </tr2:Configurations>
            </tr2:GetAudioOutputConfigurationsResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_get_audio_output_configurations_parses_response() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(audio_output_xml()));

    let configs = client
        .get_audio_output_configurations_media2("http://192.168.1.1/onvif/media2")
        .await
        .unwrap();

    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].token, "AOC_1");
    assert_eq!(configs[0].output_token, "AudioOut_1");
    assert_eq!(configs[0].output_level, Some(50));
}

// ── Media2 Video source modes ─────────────────────────────────────────────

fn video_source_modes_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                    xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tr2:GetVideoSourceModesResponse>
              <tr2:VideoSourceModes token="Mode_1">
                <tt:MaxFramerate>30</tt:MaxFramerate>
                <tt:MaxResolution>
                  <tt:Width>1920</tt:Width>
                  <tt:Height>1080</tt:Height>
                </tt:MaxResolution>
                <tt:Encodings>H264 H265</tt:Encodings>
                <tt:Reboot>false</tt:Reboot>
              </tr2:VideoSourceModes>
            </tr2:GetVideoSourceModesResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_get_video_source_modes_parses_response() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(video_source_modes_xml()));

    let modes = client
        .get_video_source_modes_media2("http://192.168.1.1/onvif/media2", "VS_1")
        .await
        .unwrap();

    assert_eq!(modes.len(), 1);
    assert_eq!(modes[0].token, "Mode_1");
    assert_eq!(modes[0].max_framerate, 30.0);
    assert_eq!(modes[0].max_resolution_width, 1920);
    assert_eq!(modes[0].max_resolution_height, 1080);
    assert_eq!(modes[0].encodings, ["H264", "H265"]);
    assert!(!modes[0].reboot);
}

#[tokio::test]
async fn test_set_video_source_mode_sends_tokens() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tr2="http://www.onvif.org/ver20/media/wsdl">
          <s:Body>
            <tr2:SetVideoSourceModeResponse>
              <tr2:Reboot>true</tr2:Reboot>
            </tr2:SetVideoSourceModeResponse>
          </s:Body>
        </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    let reboot = client
        .set_video_source_mode_media2("http://192.168.1.1/onvif/media2", "VS_1", "Mode_1")
        .await
        .unwrap();

    assert!(reboot);
    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("VS_1"));
    assert!(body.contains("Mode_1"));
}

// ── Stage 4: positives for the Media2 methods that had none ───────────────
//
// Read methods assert the parsed field values the fixture chose; write methods
// assert the SOAPAction URI *and* the exact operation fragment put on the wire.

#[tokio::test]
async fn test_create_profile_media2_returns_token_and_sends_name() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tr2="http://www.onvif.org/ver20/media/wsdl">
          <s:Body>
            <tr2:CreateProfileResponse>
              <tr2:Token>Profile_new_1</tr2:Token>
            </tr2:CreateProfileResponse>
          </s:Body>
        </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    let token = client
        .create_profile_media2("http://192.168.1.1/onvif/media2", "NightProfile")
        .await
        .unwrap();

    assert_eq!(token, "Profile_new_1");
    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver20/media/wsdl/CreateProfile"
    );
    assert!(
        c.body
            .contains("<tr2:CreateProfile><tr2:Name>NightProfile</tr2:Name></tr2:CreateProfile>"),
        "CreateProfile body drifted: {}",
        c.body
    );
}

#[tokio::test]
async fn test_delete_profile_media2_sends_action_and_token_element() {
    let xml = empty_response_xml("DeleteProfileResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .delete_profile_media2("http://192.168.1.1/onvif/media2", "Profile_9")
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver20/media/wsdl/DeleteProfile"
    );
    assert!(
        c.body
            .contains("<tr2:DeleteProfile><tr2:Token>Profile_9</tr2:Token></tr2:DeleteProfile>"),
        "DeleteProfile body drifted: {}",
        c.body
    );
}

#[tokio::test]
async fn test_get_snapshot_uri_media2_returns_string() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tr2="http://www.onvif.org/ver20/media/wsdl">
          <s:Body>
            <tr2:GetSnapshotUriResponse>
              <tr2:Uri>http://192.168.1.1/snapshot/media2?Profile_B</tr2:Uri>
            </tr2:GetSnapshotUriResponse>
          </s:Body>
        </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let uri = client
        .get_snapshot_uri_media2("http://192.168.1.1/onvif/media2", "Profile_B")
        .await
        .unwrap();

    assert_eq!(uri, "http://192.168.1.1/snapshot/media2?Profile_B");
}

fn video_source_configurations_media2_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                    xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tr2:GetVideoSourceConfigurationsResponse>
              <tr2:Configurations token="VSC_M2_1">
                <tt:Name>Media2Source</tt:Name>
                <tt:UseCount>4</tt:UseCount>
                <tt:SourceToken>VS_M2</tt:SourceToken>
                <tt:Bounds x="8" y="4" width="3840" height="2160"/>
              </tr2:Configurations>
            </tr2:GetVideoSourceConfigurationsResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_get_video_source_configurations_media2_returns_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(video_source_configurations_media2_xml()));

    let cfgs = client
        .get_video_source_configurations_media2("http://192.168.1.1/onvif/media2")
        .await
        .unwrap();

    assert_eq!(cfgs.len(), 1);
    assert_eq!(cfgs[0].token, "VSC_M2_1");
    assert_eq!(cfgs[0].name, "Media2Source");
    assert_eq!(cfgs[0].use_count, 4);
    assert_eq!(cfgs[0].source_token, "VS_M2");
    assert_eq!(cfgs[0].bounds.width, 3840);
    assert_eq!(cfgs[0].bounds.height, 2160);
}

fn video_source_configuration_options_media2_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                    xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tr2:GetVideoSourceConfigurationOptionsResponse>
              <tr2:Options>
                <tt:MaximumNumberOfProfiles>9</tt:MaximumNumberOfProfiles>
                <tt:BoundsRange>
                  <tt:XRange><tt:Min>0</tt:Min><tt:Max>1920</tt:Max></tt:XRange>
                  <tt:YRange><tt:Min>0</tt:Min><tt:Max>1080</tt:Max></tt:YRange>
                  <tt:WidthRange><tt:Min>640</tt:Min><tt:Max>3840</tt:Max></tt:WidthRange>
                  <tt:HeightRange><tt:Min>360</tt:Min><tt:Max>2160</tt:Max></tt:HeightRange>
                </tt:BoundsRange>
                <tt:VideoSourceTokensAvailable>VS_M2</tt:VideoSourceTokensAvailable>
              </tr2:Options>
            </tr2:GetVideoSourceConfigurationOptionsResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_get_video_source_configuration_options_media2_returns_ranges() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(video_source_configuration_options_media2_xml()));

    let opts = client
        .get_video_source_configuration_options_media2("http://192.168.1.1/onvif/media2", None)
        .await
        .unwrap();

    assert_eq!(opts.max_limit, Some(9));
    assert_eq!(opts.source_tokens, vec!["VS_M2"]);
    let br = opts.bounds_range.expect("BoundsRange must be parsed");
    assert_eq!(br.x_range.max, 1920);
    assert_eq!(br.y_range.max, 1080);
    assert_eq!(br.width_range.min, 640);
    assert_eq!(br.width_range.max, 3840);
    assert_eq!(br.height_range.min, 360);
    assert_eq!(br.height_range.max, 2160);
}

#[tokio::test]
async fn test_get_video_encoder_configuration_media2_returns_first_configuration() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                    xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tr2:GetVideoEncoderConfigurationsResponse>
              <tr2:Configurations token="VEC_M2_1">
                <tt:Name>Media2Encoder</tt:Name>
                <tt:UseCount>2</tt:UseCount>
                <tt:Encoding>H264</tt:Encoding>
                <tt:Resolution><tt:Width>1280</tt:Width><tt:Height>720</tt:Height></tt:Resolution>
                <tt:Quality>4</tt:Quality>
                <tt:RateControl>
                  <tt:FrameRateLimit>15</tt:FrameRateLimit>
                  <tt:BitrateLimit>2048</tt:BitrateLimit>
                </tt:RateControl>
                <tt:GovLength>45</tt:GovLength>
                <tt:Profile>High</tt:Profile>
              </tr2:Configurations>
            </tr2:GetVideoEncoderConfigurationsResponse>
          </s:Body>
        </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let cfg = client
        .get_video_encoder_configuration_media2("http://192.168.1.1/onvif/media2", "VEC_M2_1")
        .await
        .unwrap();

    assert_eq!(cfg.token, "VEC_M2_1");
    assert_eq!(cfg.name, "Media2Encoder");
    assert_eq!(cfg.use_count, 2);
    assert_eq!(cfg.encoding, crate::types::VideoEncoding::H264);
    assert_eq!(cfg.resolution.width, 1280);
    assert_eq!(cfg.resolution.height, 720);
    assert_eq!(cfg.gov_length, Some(45));
    assert_eq!(cfg.profile.as_deref(), Some("High"));
    let rc = cfg
        .rate_control
        .as_ref()
        .expect("RateControl must be parsed");
    assert_eq!(rc.frame_rate_limit, 15);
    assert_eq!(rc.bitrate_limit, 2048);
}

#[tokio::test]
async fn test_get_audio_source_configurations_media2_returns_fields() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                    xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tr2:GetAudioSourceConfigurationsResponse>
              <tr2:Configurations token="ASC_M2">
                <tt:Name>Media2AudioSource</tt:Name>
                <tt:UseCount>2</tt:UseCount>
                <tt:SourceToken>AudioSource_2</tt:SourceToken>
              </tr2:Configurations>
            </tr2:GetAudioSourceConfigurationsResponse>
          </s:Body>
        </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let cfgs = client
        .get_audio_source_configurations_media2("http://192.168.1.1/onvif/media2")
        .await
        .unwrap();

    assert_eq!(cfgs.len(), 1);
    assert_eq!(cfgs[0].token, "ASC_M2");
    assert_eq!(cfgs[0].name, "Media2AudioSource");
    assert_eq!(cfgs[0].use_count, 2);
    assert_eq!(cfgs[0].source_token, "AudioSource_2");
}

#[tokio::test]
async fn test_get_audio_encoder_configurations_media2_returns_fields() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                    xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tr2:GetAudioEncoderConfigurationsResponse>
              <tr2:Configurations token="AEC_M2">
                <tt:Name>Media2AudioEncoder</tt:Name>
                <tt:UseCount>1</tt:UseCount>
                <tt:Encoding>AAC</tt:Encoding>
                <tt:Bitrate>128</tt:Bitrate>
                <tt:SampleRate>48</tt:SampleRate>
                <tt:Channels>2</tt:Channels>
              </tr2:Configurations>
            </tr2:GetAudioEncoderConfigurationsResponse>
          </s:Body>
        </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let cfgs = client
        .get_audio_encoder_configurations_media2("http://192.168.1.1/onvif/media2")
        .await
        .unwrap();

    assert_eq!(cfgs.len(), 1);
    assert_eq!(cfgs[0].token, "AEC_M2");
    assert_eq!(cfgs[0].encoding.as_str(), "AAC");
    assert_eq!(cfgs[0].bitrate, 128);
    assert_eq!(cfgs[0].sample_rate, 48);
    assert_eq!(cfgs[0].channels, 2);
}

#[tokio::test]
async fn test_get_audio_encoder_configuration_options_media2_returns_lists() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                    xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tr2:GetAudioEncoderConfigurationOptionsResponse>
              <tr2:Options>
                <tt:Encoding>G726</tt:Encoding>
                <tt:BitrateList><tt:Items>16 32</tt:Items></tt:BitrateList>
                <tt:SampleRateList><tt:Items>8</tt:Items></tt:SampleRateList>
              </tr2:Options>
              <tr2:Options>
                <tt:Encoding>AAC</tt:Encoding>
                <tt:BitrateList><tt:Items>64 128 192</tt:Items></tt:BitrateList>
                <tt:SampleRateList><tt:Items>16 48</tt:Items></tt:SampleRateList>
              </tr2:Options>
            </tr2:GetAudioEncoderConfigurationOptionsResponse>
          </s:Body>
        </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let opts = client
        .get_audio_encoder_configuration_options_media2("http://192.168.1.1/onvif/media2", None)
        .await
        .unwrap();

    assert_eq!(opts.options.len(), 2);
    assert_eq!(opts.options[0].encoding.as_str(), "G726");
    assert_eq!(opts.options[0].bitrate_list, vec![16, 32]);
    assert_eq!(opts.options[0].sample_rate_list, vec![8]);
    assert_eq!(opts.options[1].encoding.as_str(), "AAC");
    assert_eq!(opts.options[1].bitrate_list, vec![64, 128, 192]);
    assert_eq!(opts.options[1].sample_rate_list, vec![16, 48]);
}

#[tokio::test]
async fn test_get_metadata_configuration_options_media2_returns_flags() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tr2="http://www.onvif.org/ver20/media/wsdl"
                    xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tr2:GetMetadataConfigurationOptionsResponse>
              <tr2:Options>
                <tt:PTZStatusFilterOptions>
                  <tt:PanTiltStatusSupported>true</tt:PanTiltStatusSupported>
                </tt:PTZStatusFilterOptions>
                <tt:Extension>
                  <tt:AnalyticsSupported>true</tt:AnalyticsSupported>
                </tt:Extension>
              </tr2:Options>
            </tr2:GetMetadataConfigurationOptionsResponse>
          </s:Body>
        </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let opts = client
        .get_metadata_configuration_options_media2("http://192.168.1.1/onvif/media2", None, None)
        .await
        .unwrap();

    assert!(opts.ptz_status_filter_supported);
    assert!(opts.analytics_supported);
}

#[tokio::test]
async fn test_set_metadata_configuration_media2_sends_action_and_exact_body() {
    let xml = empty_response_xml("SetMetadataConfigurationResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    let cfg = crate::types::MetadataConfiguration {
        token: "MetaConf_9".to_string(),
        name: "MetaCfg".to_string(),
        use_count: 2,
        analytics: true,
        ptz_status: false,
        ptz_position: true,
        multicast_address: None,
        multicast_port: None,
    };

    client
        .set_metadata_configuration_media2("http://192.168.1.1/onvif/media2", &cfg)
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver20/media/wsdl/SetMetadataConfiguration"
    );
    let expected = concat!(
        "<tr2:SetMetadataConfiguration>",
        r#"<tr2:Configuration token="MetaConf_9">"#,
        "<tt:Name>MetaCfg</tt:Name>",
        "<tt:UseCount>2</tt:UseCount>",
        "<tt:Analytics>true</tt:Analytics>",
        "<tt:PTZStatus><tt:Status>false</tt:Status><tt:Position>true</tt:Position></tt:PTZStatus>",
        "</tr2:Configuration>",
        "</tr2:SetMetadataConfiguration>",
    );
    assert!(
        c.body.contains(expected),
        "SetMetadataConfiguration body drifted.\nexpected fragment:\n{expected}\nactual body:\n{}",
        c.body
    );
}

// ── NET 2: emitted request-body shapes ────────────────────────────────────────
//
// Stage 1b rewrites `src/types/audio.rs`, `src/types/video.rs` and
// `src/client/media2.rs`. These tests pin the *exact* fragment each setter puts
// on the wire today, so any unintended change to element names, element order,
// namespace prefixes or values fails loudly.
//
// Assertions are on complete operation fragments, not on incidental substrings:
// a fragment like `<trt:SetAudioEncoderConfiguration>…</trt:SetAudioEncoder…>`
// cannot appear by accident.
//
// This module pairs each Media1 setter with its Media2 counterpart, so it spans
// two services; it lives with Media2 because `src/client/media2.rs` is the file
// Stage 1b rewrites and the pinned bugs are all on the Media2 side.

mod request_body_shapes {
    use super::*;
    use crate::types::{
        AudioEncoderConfiguration, AudioEncoding, H264Configuration, Resolution, SourceBounds,
        VideoEncoderConfiguration, VideoEncoderConfiguration2, VideoEncoding, VideoRateControl,
        VideoRateControl2, VideoSourceConfiguration,
    };

    const MEDIA: &str = "http://192.168.1.1/onvif/media_service";
    const MEDIA2: &str = "http://192.168.1.1/onvif/media2_service";

    fn envelope(inner: &str) -> String {
        format!(
            r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                            xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                            xmlns:tr2="http://www.onvif.org/ver20/media/wsdl">
                 <s:Body>{inner}</s:Body>
               </s:Envelope>"#
        )
    }

    fn audio_cfg() -> AudioEncoderConfiguration {
        AudioEncoderConfiguration {
            token: "AEC_1".into(),
            name: "AudioEncoder".into(),
            use_count: 1,
            encoding: AudioEncoding::G711,
            bitrate: 64,
            sample_rate: 8,
            channels: 1,
        }
    }

    /// The serialised audio-encoder fragment Media1 emits — `trt:`-prefixed.
    const AUDIO_CFG_FRAGMENT: &str = concat!(
        r#"<trt:Configuration token="AEC_1">"#,
        "<tt:Name>AudioEncoder</tt:Name>",
        "<tt:UseCount>1</tt:UseCount>",
        "<tt:Encoding>G711</tt:Encoding>",
        "<tt:Bitrate>64</tt:Bitrate>",
        "<tt:SampleRate>8</tt:SampleRate>",
        "<tt:Channels>1</tt:Channels>",
        "</trt:Configuration>",
    );

    /// The same fragment as Media2 must emit it — identical apart from the
    /// wrapper prefix.
    const AUDIO_CFG_FRAGMENT_MEDIA2: &str = concat!(
        r#"<tr2:Configuration token="AEC_1">"#,
        "<tt:Name>AudioEncoder</tt:Name>",
        "<tt:UseCount>1</tt:UseCount>",
        "<tt:Encoding>G711</tt:Encoding>",
        "<tt:Bitrate>64</tt:Bitrate>",
        "<tt:SampleRate>8</tt:SampleRate>",
        "<tt:Channels>1</tt:Channels>",
        "</tr2:Configuration>",
    );

    fn video_enc_cfg() -> VideoEncoderConfiguration {
        VideoEncoderConfiguration {
            token: "VEC_1".into(),
            name: "MainStream".into(),
            use_count: 1,
            encoding: VideoEncoding::H264,
            resolution: Resolution {
                width: 1920,
                height: 1080,
            },
            quality: 5.0,
            rate_control: Some(VideoRateControl {
                frame_rate_limit: 25,
                encoding_interval: 1,
                bitrate_limit: 4096,
            }),
            h264: Some(H264Configuration {
                gov_length: 25,
                profile: "Main".into(),
            }),
            h265: None,
            multicast: None,
            session_timeout: Some("PT60S".into()),
            guaranteed_frame_rate: None,
        }
    }

    fn video_enc_cfg2() -> VideoEncoderConfiguration2 {
        VideoEncoderConfiguration2 {
            token: "VEC_1".into(),
            name: "MainStream".into(),
            use_count: 1,
            encoding: VideoEncoding::H265,
            resolution: Resolution {
                width: 1920,
                height: 1080,
            },
            quality: 5.0,
            rate_control: Some(VideoRateControl2 {
                frame_rate_limit: 25,
                bitrate_limit: 4096,
            }),
            gov_length: Some(50),
            profile: Some("Main".into()),
        }
    }

    fn video_src_cfg() -> VideoSourceConfiguration {
        VideoSourceConfiguration {
            token: "VSC_1".into(),
            name: "VSConfig1".into(),
            use_count: 2,
            source_token: "VS_1".into(),
            bounds: SourceBounds {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
        }
    }

    /// Media1 `SetAudioEncoderConfiguration` emits a `trt:`-prefixed
    /// `<Configuration>` wrapped in a `trt:` operation element, plus
    /// `ForcePersistence` (which Media2 does not have). This is the correct
    /// Media1 shape and Stage 1b must not disturb it.
    #[tokio::test]
    async fn set_audio_encoder_configuration_media1_emits_trt_configuration() {
        let (transport, captured) =
            RecordingTransport::new(&envelope("<trt:SetAudioEncoderConfigurationResponse/>"));
        let client =
            OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

        client
            .set_audio_encoder_configuration(MEDIA, &audio_cfg())
            .await
            .unwrap();

        let c = captured.lock().unwrap();
        assert_eq!(
            c.action,
            "http://www.onvif.org/ver10/media/wsdl/SetAudioEncoderConfiguration"
        );
        let expected = format!(
            "<trt:SetAudioEncoderConfiguration>{AUDIO_CFG_FRAGMENT}\
             <trt:ForcePersistence>true</trt:ForcePersistence>\
             </trt:SetAudioEncoderConfiguration>"
        );
        assert!(
            c.body.contains(&expected),
            "Media1 audio body drifted.\nexpected fragment:\n{expected}\nactual body:\n{}",
            c.body
        );
        assert!(
            !c.body.contains("<tr2:Configuration"),
            "Media1 must never emit a tr2:-prefixed Configuration: {}",
            c.body
        );
    }

    /// D3 target: Media2 `SetAudioEncoderConfiguration` must wrap a
    /// `tr2:`-prefixed `<Configuration>`, not the Media1 `trt:` one.
    ///
    /// (This test previously pinned the bug and expected `trt:`; Stage 1b flips
    /// it, which is why it is listed as the stage's red-before-green target.)
    #[tokio::test]
    async fn set_audio_encoder_configuration_media2_emits_tr2_configuration() {
        let (transport, captured) =
            RecordingTransport::new(&envelope("<tr2:SetAudioEncoderConfigurationResponse/>"));
        let client =
            OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

        client
            .set_audio_encoder_configuration_media2(MEDIA2, &audio_cfg())
            .await
            .unwrap();

        let c = captured.lock().unwrap();
        assert_eq!(
            c.action,
            "http://www.onvif.org/ver20/media/wsdl/SetAudioEncoderConfiguration"
        );
        let expected = format!(
            "<tr2:SetAudioEncoderConfiguration>{AUDIO_CFG_FRAGMENT_MEDIA2}\
             </tr2:SetAudioEncoderConfiguration>"
        );
        assert!(
            c.body.contains(&expected),
            "Media2 audio body drifted.\nexpected fragment:\n{expected}\nactual body:\n{}",
            c.body
        );
        assert!(
            !c.body.contains("trt:"),
            "Media2 must not carry any Media1 trt: prefix: {}",
            c.body
        );
        assert!(
            !c.body.contains("ForcePersistence"),
            "Media2 must not send ForcePersistence: {}",
            c.body
        );
    }

    /// Negative for `set_audio_encoder_configuration_media2`: a device Fault
    /// must surface as `SoapError::Fault` carrying the device's code and
    /// reason — not as `UnexpectedResponse` or `MissingField`.
    #[tokio::test]
    async fn set_audio_encoder_configuration_media2_soap_fault_returns_fault() {
        let client =
            OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(
                &make_soap_fault_xml("s:Sender", "Media2 audio encoder rejected"),
            ));

        let err = client
            .set_audio_encoder_configuration_media2(MEDIA2, &audio_cfg())
            .await
            .unwrap_err();

        match err {
            OnvifError::Soap(crate::soap::SoapError::Fault { code, reason, .. }) => {
                assert_eq!(code, "s:Sender");
                assert_eq!(reason, "Media2 audio encoder rejected");
            }
            other => panic!("expected SoapError::Fault, got {other:?}"),
        }
    }

    /// Negative for the Media1 `set_audio_encoder_configuration` twin.
    #[tokio::test]
    async fn set_audio_encoder_configuration_media1_soap_fault_returns_fault() {
        let client =
            OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(
                &make_soap_fault_xml("s:Receiver", "Media1 audio encoder rejected"),
            ));

        let err = client
            .set_audio_encoder_configuration(MEDIA, &audio_cfg())
            .await
            .unwrap_err();

        match err {
            OnvifError::Soap(crate::soap::SoapError::Fault { code, reason, .. }) => {
                assert_eq!(code, "s:Receiver");
                assert_eq!(reason, "Media1 audio encoder rejected");
            }
            other => panic!("expected SoapError::Fault, got {other:?}"),
        }
    }

    /// Media1 `SetVideoEncoderConfiguration`: full element sequence, including
    /// the schema-mandated `Quality` before `RateControl`.
    #[tokio::test]
    async fn set_video_encoder_configuration_media1_body_is_exact() {
        let (transport, captured) =
            RecordingTransport::new(&envelope("<trt:SetVideoEncoderConfigurationResponse/>"));
        let client =
            OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

        client
            .set_video_encoder_configuration(MEDIA, &video_enc_cfg())
            .await
            .unwrap();

        let c = captured.lock().unwrap();
        assert_eq!(
            c.action,
            "http://www.onvif.org/ver10/media/wsdl/SetVideoEncoderConfiguration"
        );
        let expected = concat!(
            "<trt:SetVideoEncoderConfiguration>",
            r#"<trt:Configuration token="VEC_1">"#,
            "<tt:Name>MainStream</tt:Name>",
            "<tt:UseCount>1</tt:UseCount>",
            "<tt:Encoding>H264</tt:Encoding>",
            "<tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>",
            "<tt:Quality>5</tt:Quality>",
            "<tt:RateControl>",
            "<tt:FrameRateLimit>25</tt:FrameRateLimit>",
            "<tt:EncodingInterval>1</tt:EncodingInterval>",
            "<tt:BitrateLimit>4096</tt:BitrateLimit>",
            "</tt:RateControl>",
            "<tt:H264><tt:GovLength>25</tt:GovLength><tt:H264Profile>Main</tt:H264Profile></tt:H264>",
            "<tt:SessionTimeout>PT60S</tt:SessionTimeout>",
            "</trt:Configuration>",
            "<trt:ForcePersistence>true</trt:ForcePersistence>",
            "</trt:SetVideoEncoderConfiguration>",
        );
        assert!(
            c.body.contains(expected),
            "Media1 video encoder body drifted.\nexpected fragment:\n{expected}\nactual body:\n{}",
            c.body
        );
        assert!(
            !c.body.contains("<tr2:"),
            "Media1 must not emit tr2: elements: {}",
            c.body
        );
    }

    /// Media2 `SetVideoEncoderConfiguration`: `tr2:`-prefixed Configuration,
    /// `GovLength` + `Profile` between RateControl and Quality, no
    /// `EncodingInterval`, no `ForcePersistence`.
    #[tokio::test]
    async fn set_video_encoder_configuration_media2_body_is_exact() {
        let (transport, captured) =
            RecordingTransport::new(&envelope("<tr2:SetVideoEncoderConfigurationResponse/>"));
        let client =
            OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

        client
            .set_video_encoder_configuration_media2(MEDIA2, &video_enc_cfg2())
            .await
            .unwrap();

        let c = captured.lock().unwrap();
        assert_eq!(
            c.action,
            "http://www.onvif.org/ver20/media/wsdl/SetVideoEncoderConfiguration"
        );
        let expected = concat!(
            "<tr2:SetVideoEncoderConfiguration>",
            r#"<tr2:Configuration token="VEC_1">"#,
            "<tt:Name>MainStream</tt:Name>",
            "<tt:UseCount>1</tt:UseCount>",
            "<tt:Encoding>H265</tt:Encoding>",
            "<tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>",
            "<tt:RateControl>",
            "<tt:FrameRateLimit>25</tt:FrameRateLimit>",
            "<tt:BitrateLimit>4096</tt:BitrateLimit>",
            "</tt:RateControl>",
            "<tt:GovLength>50</tt:GovLength>",
            "<tt:Profile>Main</tt:Profile>",
            "<tt:Quality>5</tt:Quality>",
            "</tr2:Configuration>",
            "</tr2:SetVideoEncoderConfiguration>",
        );
        assert!(
            c.body.contains(expected),
            "Media2 video encoder body drifted.\nexpected fragment:\n{expected}\nactual body:\n{}",
            c.body
        );
        assert!(
            !c.body.contains("ForcePersistence"),
            "Media2 must not send ForcePersistence: {}",
            c.body
        );
    }

    /// Media1 `SetVideoSourceConfiguration`: `trt:` Configuration + Bounds
    /// attributes + ForcePersistence.
    #[tokio::test]
    async fn set_video_source_configuration_media1_body_is_exact() {
        let (transport, captured) =
            RecordingTransport::new(&envelope("<trt:SetVideoSourceConfigurationResponse/>"));
        let client =
            OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

        client
            .set_video_source_configuration(MEDIA, &video_src_cfg())
            .await
            .unwrap();

        let c = captured.lock().unwrap();
        assert_eq!(
            c.action,
            "http://www.onvif.org/ver10/media/wsdl/SetVideoSourceConfiguration"
        );
        let expected = concat!(
            "<trt:SetVideoSourceConfiguration>",
            r#"<trt:Configuration token="VSC_1">"#,
            "<tt:Name>VSConfig1</tt:Name>",
            "<tt:UseCount>2</tt:UseCount>",
            "<tt:SourceToken>VS_1</tt:SourceToken>",
            r#"<tt:Bounds x="0" y="0" width="1920" height="1080"/>"#,
            "</trt:Configuration>",
            "<trt:ForcePersistence>true</trt:ForcePersistence>",
            "</trt:SetVideoSourceConfiguration>",
        );
        assert!(
            c.body.contains(expected),
            "Media1 video source body drifted.\nexpected fragment:\n{expected}\nactual body:\n{}",
            c.body
        );
        assert!(
            !c.body.contains("<tr2:Configuration"),
            "Media1 must never emit a tr2:-prefixed Configuration: {}",
            c.body
        );
    }

    /// Media2 `SetVideoSourceConfiguration`: identical children, `tr2:` prefix
    /// on the Configuration element, and no ForcePersistence.
    #[tokio::test]
    async fn set_video_source_configuration_media2_body_is_exact() {
        let (transport, captured) =
            RecordingTransport::new(&envelope("<tr2:SetVideoSourceConfigurationResponse/>"));
        let client =
            OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

        client
            .set_video_source_configuration_media2(MEDIA2, &video_src_cfg())
            .await
            .unwrap();

        let c = captured.lock().unwrap();
        assert_eq!(
            c.action,
            "http://www.onvif.org/ver20/media/wsdl/SetVideoSourceConfiguration"
        );
        let expected = concat!(
            "<tr2:SetVideoSourceConfiguration>",
            r#"<tr2:Configuration token="VSC_1">"#,
            "<tt:Name>VSConfig1</tt:Name>",
            "<tt:UseCount>2</tt:UseCount>",
            "<tt:SourceToken>VS_1</tt:SourceToken>",
            r#"<tt:Bounds x="0" y="0" width="1920" height="1080"/>"#,
            "</tr2:Configuration>",
            "</tr2:SetVideoSourceConfiguration>",
        );
        assert!(
            c.body.contains(expected),
            "Media2 video source body drifted.\nexpected fragment:\n{expected}\nactual body:\n{}",
            c.body
        );
        assert!(
            !c.body.contains("ForcePersistence"),
            "Media2 must not send ForcePersistence: {}",
            c.body
        );
    }
}
