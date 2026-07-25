//! Unit tests for the Media (ver10) methods on `OnvifClient`
//! (`src/client/media.rs`).

use super::*;
use crate::tests::common::*;
use std::sync::Arc;

fn profiles_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trt:GetProfilesResponse>
              <trt:Profiles token="Profile_1" fixed="true">
                <tt:Name>mainStream</tt:Name>
              </trt:Profiles>
              <trt:Profiles token="Profile_2" fixed="false">
                <tt:Name>subStream</tt:Name>
              </trt:Profiles>
            </trt:GetProfilesResponse>
          </s:Body>
        </s:Envelope>"#
}

fn stream_uri_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trt:GetStreamUriResponse>
              <trt:MediaUri>
                <tt:Uri>rtsp://192.168.1.1:554/Streaming/Channels/101</tt:Uri>
                <tt:InvalidAfterConnect>false</tt:InvalidAfterConnect>
                <tt:InvalidAfterReboot>false</tt:InvalidAfterReboot>
                <tt:Timeout>PT0S</tt:Timeout>
              </trt:MediaUri>
            </trt:GetStreamUriResponse>
          </s:Body>
        </s:Envelope>"#
}

// ── get_profiles ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_profiles_returns_all_profiles() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(profiles_xml()));

    let profiles = client
        .get_profiles("http://192.168.1.1/onvif/media_service")
        .await
        .unwrap();

    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0].token, "Profile_1");
    assert_eq!(profiles[0].name, "mainStream");
    assert!(profiles[0].fixed);
    assert_eq!(profiles[1].token, "Profile_2");
    assert!(!profiles[1].fixed);
}

// ── get_stream_uri ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_stream_uri_returns_rtsp_url() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(stream_uri_xml()));

    let uri = client
        .get_stream_uri("http://192.168.1.1/onvif/media_service", "Profile_1")
        .await
        .unwrap();

    assert_eq!(uri.uri, "rtsp://192.168.1.1:554/Streaming/Channels/101");
    assert_eq!(uri.timeout, "PT0S");
    assert!(!uri.invalid_after_connect);
    assert!(!uri.invalid_after_reboot);
}

#[tokio::test]
async fn test_get_stream_uri_embeds_profile_token_in_body() {
    let (transport, captured) = RecordingTransport::new(stream_uri_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .get_stream_uri("http://192.168.1.1/onvif/media_service", "Profile_1")
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(
        body.contains("Profile_1"),
        "profile token must appear in request body"
    );
}

// ── video source / encoder fixtures ──────────────────────────────────────

fn video_sources_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trt:GetVideoSourcesResponse>
              <trt:VideoSources token="VS_1">
                <tt:Framerate>25</tt:Framerate>
                <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
              </trt:VideoSources>
              <trt:VideoSources token="VS_2">
                <tt:Framerate>15</tt:Framerate>
                <tt:Resolution><tt:Width>1280</tt:Width><tt:Height>720</tt:Height></tt:Resolution>
              </trt:VideoSources>
            </trt:GetVideoSourcesResponse>
          </s:Body>
        </s:Envelope>"#
}

fn video_source_configurations_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trt:GetVideoSourceConfigurationsResponse>
              <trt:Configurations token="VSC_1">
                <tt:Name>VSConfig1</tt:Name>
                <tt:UseCount>2</tt:UseCount>
                <tt:SourceToken>VS_1</tt:SourceToken>
                <tt:Bounds x="0" y="0" width="1920" height="1080"/>
              </trt:Configurations>
              <trt:Configurations token="VSC_2">
                <tt:Name>VSConfig2</tt:Name>
                <tt:UseCount>1</tt:UseCount>
                <tt:SourceToken>VS_2</tt:SourceToken>
                <tt:Bounds x="0" y="0" width="1280" height="720"/>
              </trt:Configurations>
            </trt:GetVideoSourceConfigurationsResponse>
          </s:Body>
        </s:Envelope>"#
}

fn video_encoder_configurations_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trt:GetVideoEncoderConfigurationsResponse>
              <trt:Configurations token="VEC_1">
                <tt:Name>MainStream</tt:Name>
                <tt:UseCount>1</tt:UseCount>
                <tt:Encoding>H264</tt:Encoding>
                <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
                <tt:Quality>5</tt:Quality>
              </trt:Configurations>
              <trt:Configurations token="VEC_2">
                <tt:Name>SubStream</tt:Name>
                <tt:UseCount>1</tt:UseCount>
                <tt:Encoding>JPEG</tt:Encoding>
                <tt:Resolution><tt:Width>640</tt:Width><tt:Height>480</tt:Height></tt:Resolution>
                <tt:Quality>3</tt:Quality>
              </trt:Configurations>
            </trt:GetVideoEncoderConfigurationsResponse>
          </s:Body>
        </s:Envelope>"#
}

fn video_encoder_configuration_single_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trt:GetVideoEncoderConfigurationResponse>
              <trt:Configuration token="VEC_1">
                <tt:Name>MainStream</tt:Name>
                <tt:UseCount>1</tt:UseCount>
                <tt:Encoding>H264</tt:Encoding>
                <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
                <tt:Quality>5</tt:Quality>
                <tt:RateControl>
                  <tt:FrameRateLimit>25</tt:FrameRateLimit>
                  <tt:EncodingInterval>1</tt:EncodingInterval>
                  <tt:BitrateLimit>4096</tt:BitrateLimit>
                </tt:RateControl>
                <tt:H264>
                  <tt:GovLength>30</tt:GovLength>
                  <tt:H264Profile>Main</tt:H264Profile>
                </tt:H264>
              </trt:Configuration>
            </trt:GetVideoEncoderConfigurationResponse>
          </s:Body>
        </s:Envelope>"#
}

fn video_encoder_configuration_options_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trt:GetVideoEncoderConfigurationOptionsResponse>
              <trt:Options>
                <tt:QualityRange><tt:Min>1</tt:Min><tt:Max>10</tt:Max></tt:QualityRange>
                <tt:H264>
                  <tt:ResolutionsAvailable><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:ResolutionsAvailable>
                  <tt:GovLengthRange><tt:Min>1</tt:Min><tt:Max>150</tt:Max></tt:GovLengthRange>
                  <tt:FrameRateRange><tt:Min>1</tt:Min><tt:Max>30</tt:Max></tt:FrameRateRange>
                  <tt:EncodingIntervalRange><tt:Min>1</tt:Min><tt:Max>1</tt:Max></tt:EncodingIntervalRange>
                  <tt:BitrateRange><tt:Min>32</tt:Min><tt:Max>16384</tt:Max></tt:BitrateRange>
                  <tt:H264ProfilesSupported>Baseline</tt:H264ProfilesSupported>
                  <tt:H264ProfilesSupported>Main</tt:H264ProfilesSupported>
                  <tt:H264ProfilesSupported>High</tt:H264ProfilesSupported>
                </tt:H264>
              </trt:Options>
            </trt:GetVideoEncoderConfigurationOptionsResponse>
          </s:Body>
        </s:Envelope>"#
}

// ── get_video_sources ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_video_sources_returns_correct_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(video_sources_xml()));

    let sources = client
        .get_video_sources("http://192.168.1.1/onvif/media_service")
        .await
        .unwrap();

    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0].token, "VS_1");
    assert!((sources[0].framerate - 25.0).abs() < 1e-5);
    assert_eq!(
        sources[0].resolution,
        crate::types::Resolution {
            width: 1920,
            height: 1080
        }
    );
    assert_eq!(sources[1].token, "VS_2");
}

// ── get_video_source_configurations ──────────────────────────────────────

#[tokio::test]
async fn test_get_video_source_configurations_returns_all() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(video_source_configurations_xml()));

    let cfgs = client
        .get_video_source_configurations("http://192.168.1.1/onvif/media_service")
        .await
        .unwrap();

    assert_eq!(cfgs.len(), 2);
    assert_eq!(cfgs[0].token, "VSC_1");
    assert_eq!(cfgs[0].source_token, "VS_1");
    assert_eq!(cfgs[1].token, "VSC_2");
}

// ── get_video_encoder_configurations ─────────────────────────────────────

#[tokio::test]
async fn test_get_video_encoder_configurations_returns_all() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(video_encoder_configurations_xml()));

    let cfgs = client
        .get_video_encoder_configurations("http://192.168.1.1/onvif/media_service")
        .await
        .unwrap();

    assert_eq!(cfgs.len(), 2);
    assert_eq!(cfgs[0].token, "VEC_1");
    assert_eq!(cfgs[0].encoding, crate::types::VideoEncoding::H264);
    assert_eq!(cfgs[1].encoding, crate::types::VideoEncoding::Jpeg);
}

// ── get_video_encoder_configuration (single) ──────────────────────────────

#[tokio::test]
async fn test_get_video_encoder_configuration_single() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(video_encoder_configuration_single_xml()));

    let cfg = client
        .get_video_encoder_configuration("http://192.168.1.1/onvif/media_service", "VEC_1")
        .await
        .unwrap();

    assert_eq!(cfg.token, "VEC_1");
    assert_eq!(cfg.encoding, crate::types::VideoEncoding::H264);
    let rc = cfg.rate_control.unwrap();
    assert_eq!(rc.frame_rate_limit, 25);
    assert_eq!(rc.bitrate_limit, 4096);
    let h264 = cfg.h264.unwrap();
    assert_eq!(h264.gov_length, 30);
    assert_eq!(h264.profile, "Main");
}

// ── get_video_encoder_configuration_options ───────────────────────────────

#[tokio::test]
async fn test_get_video_encoder_configuration_options_parses_h264() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(video_encoder_configuration_options_xml()));

    let opts = client
        .get_video_encoder_configuration_options("http://192.168.1.1/onvif/media_service", None)
        .await
        .unwrap();

    let qr = opts.quality_range.unwrap();
    assert!((qr.min - 1.0).abs() < 1e-5);
    assert!((qr.max - 10.0).abs() < 1e-5);
    let h264 = opts.h264.unwrap();
    assert_eq!(h264.profiles.len(), 3);
    assert_eq!(h264.profiles[1], "Main");
    let br = h264.bitrate_range.unwrap();
    assert_eq!(br.max, 16384);
}

// ── set_video_encoder_configuration H265 gate ─────────────────────────────

#[tokio::test]
async fn test_set_video_encoder_configuration_rejects_h265_via_media1() {
    use crate::types::{H265Configuration, Resolution, VideoEncoderConfiguration, VideoEncoding};

    // Transport explodes if reached — proves the gate fires before any send.
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(Arc::new(ErrorTransport { status: 999 }));

    let cfg = VideoEncoderConfiguration {
        token: "VEC_1".into(),
        name: "Main".into(),
        use_count: 1,
        encoding: VideoEncoding::H265,
        resolution: Resolution {
            width: 1920,
            height: 1080,
        },
        quality: 4.0,
        rate_control: None,
        h264: None,
        h265: Some(H265Configuration {
            gov_length: 25,
            profile: "Main".into(),
        }),
        multicast: None,
        session_timeout: Some("PT60S".into()),
        guaranteed_frame_rate: None,
    };

    let err = client
        .set_video_encoder_configuration("http://192.168.1.1/onvif/media_service", &cfg)
        .await
        .unwrap_err();

    match err {
        OnvifError::InvalidArgument(msg) => {
            assert!(
                msg.contains("H265"),
                "expected H265 mention in error: {msg}"
            );
            assert!(
                msg.contains("Media2") || msg.contains("media2"),
                "expected Media2 hint in error: {msg}"
            );
        }
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

// ── Media1 profile management fixtures ───────────────────────────────────

fn create_profile_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trt:CreateProfileResponse>
              <trt:Profile token="NewToken" fixed="false">
                <tt:Name>MyProfile</tt:Name>
              </trt:Profile>
            </trt:CreateProfileResponse>
          </s:Body>
        </s:Envelope>"#
}

fn get_profile_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trt:GetProfileResponse>
              <trt:Profile token="Profile_1" fixed="true">
                <tt:Name>mainStream</tt:Name>
              </trt:Profile>
            </trt:GetProfileResponse>
          </s:Body>
        </s:Envelope>"#
}

// ── create_profile ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_create_profile_returns_profile() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(create_profile_xml()));

    let profile = client
        .create_profile("http://192.168.1.1/onvif/media_service", "MyProfile", None)
        .await
        .unwrap();

    assert_eq!(profile.token, "NewToken");
    assert_eq!(profile.name, "MyProfile");
    assert!(!profile.fixed);
}

#[tokio::test]
async fn test_create_profile_with_token_sends_token() {
    let (transport, captured) = RecordingTransport::new(create_profile_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .create_profile(
            "http://192.168.1.1/onvif/media_service",
            "MyProfile",
            Some("NewToken"),
        )
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(
        body.contains("NewToken"),
        "explicit token must appear in request"
    );
}

// ── delete_profile ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_delete_profile_sends_token() {
    let xml = empty_response_xml("DeleteProfileResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .delete_profile("http://192.168.1.1/onvif/media_service", "Profile_1")
        .await
        .unwrap();

    assert!(captured.lock().unwrap().body.contains("Profile_1"));
}

// ── get_profile ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_profile_returns_correct_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_profile_xml()));

    let profile = client
        .get_profile("http://192.168.1.1/onvif/media_service", "Profile_1")
        .await
        .unwrap();

    assert_eq!(profile.token, "Profile_1");
    assert_eq!(profile.name, "mainStream");
    assert!(profile.fixed);
}

#[tokio::test]
async fn test_get_profile_sends_token_in_body() {
    let (transport, captured) = RecordingTransport::new(get_profile_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .get_profile("http://192.168.1.1/onvif/media_service", "Profile_1")
        .await
        .unwrap();

    assert!(captured.lock().unwrap().body.contains("Profile_1"));
}

// ── add/remove video encoder configuration ────────────────────────────────

#[tokio::test]
async fn test_add_video_encoder_configuration_ok() {
    let xml = empty_response_xml("AddVideoEncoderConfigurationResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .add_video_encoder_configuration(
            "http://192.168.1.1/onvif/media_service",
            "Profile_1",
            "VEC_1",
        )
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("Profile_1"));
    assert!(body.contains("VEC_1"));
}

#[tokio::test]
async fn test_remove_video_encoder_configuration_ok() {
    let xml = empty_response_xml("RemoveVideoEncoderConfigurationResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .remove_video_encoder_configuration("http://192.168.1.1/onvif/media_service", "Profile_1")
        .await
        .unwrap();

    assert!(captured.lock().unwrap().body.contains("Profile_1"));
}

// ── add/remove video source configuration ────────────────────────────────

#[tokio::test]
async fn test_add_video_source_configuration_ok() {
    let xml = empty_response_xml("AddVideoSourceConfigurationResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .add_video_source_configuration(
            "http://192.168.1.1/onvif/media_service",
            "Profile_1",
            "VSC_1",
        )
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("Profile_1"));
    assert!(body.contains("VSC_1"));
}

#[tokio::test]
async fn test_remove_video_source_configuration_ok() {
    let xml = empty_response_xml("RemoveVideoSourceConfigurationResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .remove_video_source_configuration("http://192.168.1.1/onvif/media_service", "Profile_1")
        .await
        .unwrap();

    assert!(captured.lock().unwrap().body.contains("Profile_1"));
}

#[tokio::test]
async fn test_get_profiles_malformed_xml_returns_err() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock("<unclosed"));
    let result = client
        .get_profiles("http://192.168.1.1/onvif/media_service")
        .await;
    assert!(result.is_err(), "expected Err on malformed XML");
}

// ── Missing required fields ───────────────────────────────────────────────

fn get_profiles_response_missing_token() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trt:GetProfilesResponse>
             <trt:Profiles>
               <tt:Name>MainStream</tt:Name>
             </trt:Profiles>
           </trt:GetProfilesResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_profiles_missing_token_returns_err() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_profiles_response_missing_token()));
    let result = client
        .get_profiles("http://192.168.1.1/onvif/media_service")
        .await;
    assert!(
        result.is_err(),
        "expected Err when profile token is missing"
    );
}

fn get_profile_response_missing_token() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trt:GetProfileResponse>
             <trt:Profile>
               <tt:Name>MainStream</tt:Name>
             </trt:Profile>
           </trt:GetProfileResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_profile_missing_token_returns_err() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_profile_response_missing_token()));
    let result = client
        .get_profile("http://192.168.1.1/onvif/media_service", "Profile_1")
        .await;
    assert!(
        result.is_err(),
        "expected Err when profile token attribute is absent"
    );
}

fn get_stream_uri_missing_uri() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:trt="http://www.onvif.org/ver10/media/wsdl">
         <s:Body>
           <trt:GetStreamUriResponse>
             <trt:MediaUri/>
           </trt:GetStreamUriResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_stream_uri_missing_uri_returns_err() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_stream_uri_missing_uri()));
    let result = client
        .get_stream_uri("http://192.168.1.1/onvif/media_service", "Profile_1")
        .await;
    assert!(result.is_err(), "expected Err when Uri element is missing");
}

// ── Audio Service tests ───────────────────────────────────────────────────────

fn get_audio_sources_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trt:GetAudioSourcesResponse>
             <trt:AudioSources token="AudioSource_1">
               <tt:Channels>1</tt:Channels>
             </trt:AudioSources>
           </trt:GetAudioSourcesResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_audio_sources_ok() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_audio_sources_xml()));
    let sources = client
        .get_audio_sources("http://192.168.1.1/onvif/media_service")
        .await
        .unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].token, "AudioSource_1");
    assert_eq!(sources[0].channels, 1);
}

fn get_audio_source_configurations_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trt:GetAudioSourceConfigurationsResponse>
             <trt:Configurations token="AudioSourceConfig_1">
               <tt:Name>AudioSourceConfiguration_1</tt:Name>
               <tt:UseCount>1</tt:UseCount>
               <tt:SourceToken>AudioSource_1</tt:SourceToken>
             </trt:Configurations>
           </trt:GetAudioSourceConfigurationsResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_audio_source_configurations_ok() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_audio_source_configurations_xml()));
    let cfgs = client
        .get_audio_source_configurations("http://192.168.1.1/onvif/media_service")
        .await
        .unwrap();
    assert_eq!(cfgs.len(), 1);
    assert_eq!(cfgs[0].token, "AudioSourceConfig_1");
    assert_eq!(cfgs[0].source_token, "AudioSource_1");
}

fn get_audio_encoder_configurations_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trt:GetAudioEncoderConfigurationsResponse>
             <trt:Configurations token="AudioEncoderConfig_1">
               <tt:Name>AudioEncoderConfiguration_1</tt:Name>
               <tt:UseCount>1</tt:UseCount>
               <tt:Encoding>G711</tt:Encoding>
               <tt:Bitrate>64</tt:Bitrate>
               <tt:SampleRate>8</tt:SampleRate>
             </trt:Configurations>
           </trt:GetAudioEncoderConfigurationsResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_audio_encoder_configurations_ok() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_audio_encoder_configurations_xml()));
    let cfgs = client
        .get_audio_encoder_configurations("http://192.168.1.1/onvif/media_service")
        .await
        .unwrap();
    assert_eq!(cfgs.len(), 1);
    assert_eq!(cfgs[0].token, "AudioEncoderConfig_1");
    assert_eq!(cfgs[0].encoding.as_str(), "G711");
    assert_eq!(cfgs[0].bitrate, 64);
    assert_eq!(cfgs[0].sample_rate, 8);
}

fn get_audio_encoder_configuration_options_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trt:GetAudioEncoderConfigurationOptionsResponse>
             <trt:Options>
               <tt:Encoding>G711</tt:Encoding>
               <tt:BitrateList><tt:Items>64</tt:Items></tt:BitrateList>
               <tt:SampleRateList><tt:Items>8</tt:Items></tt:SampleRateList>
             </trt:Options>
             <trt:Options>
               <tt:Encoding>AAC</tt:Encoding>
               <tt:BitrateList><tt:Items>32 64 128</tt:Items></tt:BitrateList>
               <tt:SampleRateList><tt:Items>8 16 44</tt:Items></tt:SampleRateList>
             </trt:Options>
           </trt:GetAudioEncoderConfigurationOptionsResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_audio_encoder_configuration_options_ok() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_audio_encoder_configuration_options_xml()));
    let opts = client
        .get_audio_encoder_configuration_options(
            "http://192.168.1.1/onvif/media_service",
            "AudioEncoderConfig_1",
        )
        .await
        .unwrap();
    assert_eq!(opts.options.len(), 2);
    assert_eq!(opts.options[0].encoding.as_str(), "G711");
    assert_eq!(opts.options[0].bitrate_list, vec![64]);
    assert_eq!(opts.options[1].encoding.as_str(), "AAC");
    assert_eq!(opts.options[1].sample_rate_list, vec![8, 16, 44]);
}

// ── OSD ───────────────────────────────────────────────────────────────────────

fn get_osds_xml() -> &'static str {
    // Real cameras wrap each entry in `<trt:OSDs>` (the WSDL element
    // name is "OSDs" with type tt:OSDConfiguration). Earlier fixture
    // used `<trt:OSDConfiguration>` — that was wrong and masked a
    // parser bug.
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trt:GetOSDsResponse>
              <trt:OSDs token="osd_1">
                <tt:VideoSourceConfigurationToken>vsc_1</tt:VideoSourceConfigurationToken>
                <tt:Type>Text</tt:Type>
                <tt:Position>
                  <tt:Type>UpperLeft</tt:Type>
                </tt:Position>
                <tt:TextString>
                  <tt:Type>DateAndTime</tt:Type>
                  <tt:DateFormat>MM/DD/YYYY</tt:DateFormat>
                </tt:TextString>
              </trt:OSDs>
            </trt:GetOSDsResponse>
          </s:Body>
        </s:Envelope>"#
}

fn get_osd_options_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trt:GetOSDOptionsResponse>
              <trt:OSDOptions>
                <tt:MaximumNumberOfOSDs>4</tt:MaximumNumberOfOSDs>
                <tt:Type>Text</tt:Type>
                <tt:Type>Image</tt:Type>
              </trt:OSDOptions>
            </trt:GetOSDOptionsResponse>
          </s:Body>
        </s:Envelope>"#
}

fn create_osd_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trt="http://www.onvif.org/ver10/media/wsdl">
          <s:Body>
            <trt:CreateOSDResponse>
              <trt:OSDToken>osd_new_1</trt:OSDToken>
            </trt:CreateOSDResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_get_osds_parses_configuration() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_osds_xml()));

    let osds = client
        .get_osds("http://192.168.1.1/onvif/media", None)
        .await
        .unwrap();

    assert_eq!(osds.len(), 1);
    assert_eq!(osds[0].token, "osd_1");
    assert_eq!(osds[0].video_source_config_token, "vsc_1");
    assert_eq!(osds[0].type_, "Text");
    assert_eq!(osds[0].position.type_, "UpperLeft");
    let ts = osds[0].text_string.as_ref().unwrap();
    assert_eq!(ts.type_, "DateAndTime");
    assert_eq!(ts.date_format.as_deref(), Some("MM/DD/YYYY"));
}

#[tokio::test]
async fn test_get_osds_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trt="http://www.onvif.org/ver10/media/wsdl">
          <s:Body>
            <trt:GetOSDsResponse>
              <trt:OSDs>
                <tt:Type xmlns:tt="http://www.onvif.org/ver10/schema">Text</tt:Type>
              </trt:OSDs>
            </trt:GetOSDsResponse>
          </s:Body>
        </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let err = client
        .get_osds("http://192.168.1.1/onvif/media", None)
        .await
        .unwrap_err();

    assert!(matches!(err, crate::error::OnvifError::Soap(_)));
}

#[tokio::test]
async fn test_create_osd_returns_token() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(create_osd_xml()));

    let osd = crate::OsdConfiguration {
        token: String::new(),
        video_source_config_token: "vsc_1".to_string(),
        type_: "Text".to_string(),
        position: crate::OsdPosition {
            type_: "UpperLeft".to_string(),
            x: None,
            y: None,
        },
        text_string: None,
        image_path: None,
    };

    let token = client
        .create_osd("http://192.168.1.1/onvif/media", &osd)
        .await
        .unwrap();

    assert_eq!(token, "osd_new_1");
}

#[tokio::test]
async fn test_get_osd_options_parses_max_and_types() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_osd_options_xml()));

    let opts = client
        .get_osd_options("http://192.168.1.1/onvif/media", "vsc_1")
        .await
        .unwrap();

    assert_eq!(opts.max_osd, 4);
    assert_eq!(opts.types, vec!["Text", "Image"]);
}

#[tokio::test]
async fn test_get_osd_options_missing_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
                   <s:Body><trt:GetOSDOptionsResponse/></s:Body>
                 </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let err = client
        .get_osd_options("http://192.168.1.1/onvif/media", "vsc_1")
        .await
        .unwrap_err();

    assert!(matches!(err, crate::error::OnvifError::Soap(_)));
}

// ── New-field coverage tests ──────────────────────────────────────────────────

// MediaProfile config tokens

#[tokio::test]
async fn test_get_profiles_parses_config_tokens() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trt:GetProfilesResponse>
             <trt:Profiles token="Profile_1" fixed="false">
               <tt:Name>main</tt:Name>
               <tt:VideoSourceConfiguration token="VSC_1">
                 <tt:SourceToken>VS_1</tt:SourceToken>
               </tt:VideoSourceConfiguration>
               <tt:VideoEncoderConfiguration token="VideoEnc_1"/>
               <tt:AudioSourceConfiguration token="AudioSrc_1"/>
               <tt:AudioEncoderConfiguration token="AudioEnc_1"/>
               <tt:PTZConfiguration token="PTZConfig_1"/>
             </trt:Profiles>
           </trt:GetProfilesResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let profiles = client
        .get_profiles("http://192.168.1.1/onvif/media_service")
        .await
        .unwrap();
    assert_eq!(profiles.len(), 1);
    let p = &profiles[0];
    assert_eq!(p.video_source_config_token.as_deref(), Some("VSC_1"));
    assert_eq!(p.video_source_token.as_deref(), Some("VS_1"));
    assert_eq!(p.video_encoder_token.as_deref(), Some("VideoEnc_1"));
    assert_eq!(p.audio_source_token.as_deref(), Some("AudioSrc_1"));
    assert_eq!(p.audio_encoder_token.as_deref(), Some("AudioEnc_1"));
    assert_eq!(p.ptz_config_token.as_deref(), Some("PTZConfig_1"));
}

#[tokio::test]
async fn test_get_profiles_missing_configs_are_none() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trt="http://www.onvif.org/ver10/media/wsdl">
         <s:Body>
           <trt:GetProfilesResponse>
             <trt:Profiles token="Profile_2" fixed="true">
               <tt:Name>sub</tt:Name>
             </trt:Profiles>
           </trt:GetProfilesResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let profiles = client
        .get_profiles("http://192.168.1.1/onvif/media_service")
        .await
        .unwrap();
    let p = &profiles[0];
    assert!(p.video_source_config_token.is_none());
    assert!(p.video_source_token.is_none());
    assert!(p.video_encoder_token.is_none());
    assert!(p.audio_source_token.is_none());
    assert!(p.audio_encoder_token.is_none());
    assert!(p.ptz_config_token.is_none());
}

// AudioEncoderConfiguration channels

#[tokio::test]
async fn test_get_audio_encoder_configuration_parses_channels() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trt:GetAudioEncoderConfigurationResponse>
             <trt:Configuration token="AudioEnc_1">
               <tt:Name>Audio</tt:Name>
               <tt:UseCount>1</tt:UseCount>
               <tt:Encoding>AAC</tt:Encoding>
               <tt:Bitrate>128</tt:Bitrate>
               <tt:SampleRate>44</tt:SampleRate>
               <tt:Channels>2</tt:Channels>
             </trt:Configuration>
           </trt:GetAudioEncoderConfigurationResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let cfg = client
        .get_audio_encoder_configuration("http://192.168.1.1/onvif/media_service", "AudioEnc_1")
        .await
        .unwrap();
    assert_eq!(cfg.channels, 2);
    assert_eq!(cfg.encoding.as_str(), "AAC");
}

// VideoEncoderConfiguration multicast

#[tokio::test]
async fn test_get_video_encoder_configuration_parses_multicast() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trt:GetVideoEncoderConfigurationResponse>
             <trt:Configuration token="VideoEnc_1">
               <tt:Name>Main</tt:Name>
               <tt:UseCount>1</tt:UseCount>
               <tt:Encoding>H264</tt:Encoding>
               <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
               <tt:Quality>4.0</tt:Quality>
               <tt:Multicast>
                 <tt:Address>
                   <tt:Type>IPv4</tt:Type>
                   <tt:IPv4Address>239.255.0.1</tt:IPv4Address>
                 </tt:Address>
                 <tt:Port>5000</tt:Port>
                 <tt:TTL>5</tt:TTL>
                 <tt:AutoStart>false</tt:AutoStart>
               </tt:Multicast>
             </trt:Configuration>
           </trt:GetVideoEncoderConfigurationResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let cfg = client
        .get_video_encoder_configuration("http://192.168.1.1/onvif/media_service", "VideoEnc_1")
        .await
        .unwrap();
    let mc = cfg.multicast.expect("multicast should be present");
    assert_eq!(mc.address, "239.255.0.1");
    assert_eq!(mc.port, 5000);
    assert_eq!(mc.ttl, 5);
    assert!(!mc.auto_start);
}

#[tokio::test]
async fn test_get_osd_parses_colors_and_persistence() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trt:GetOSDResponse>
             <trt:OSD token="OSD_1">
               <tt:VideoSourceConfigurationToken>VideoSrc_1</tt:VideoSourceConfigurationToken>
               <tt:Type>Text</tt:Type>
               <tt:Position><tt:Type>UpperLeft</tt:Type></tt:Position>
               <tt:TextString>
                 <tt:Type>Plain</tt:Type>
                 <tt:PlainText>Hello</tt:PlainText>
                 <tt:FontColor>
                   <tt:Color X="1.0" Y="0.5" Z="0.5" Colorspace="http://www.onvif.org/ver10/colorspace/YCbCr"/>
                   <tt:Transparent>0</tt:Transparent>
                 </tt:FontColor>
                 <tt:BackgroundColor>
                   <tt:Color X="0.0" Y="0.5" Z="0.5"/>
                 </tt:BackgroundColor>
                 <tt:IsPersistentText>true</tt:IsPersistentText>
               </tt:TextString>
             </trt:OSD>
           </trt:GetOSDResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let osd = client
        .get_osd("http://192.168.1.1/onvif/media_service", "OSD_1")
        .await
        .unwrap();
    let ts = osd.text_string.expect("text_string should be present");
    let fc = ts.font_color.expect("font_color should be present");
    assert!((fc.x - 1.0).abs() < 1e-5);
    assert!(fc.colorspace.as_deref().unwrap().contains("YCbCr"));
    assert_eq!(fc.transparent, Some(0.0));
    assert!(ts.background_color.is_some());
    assert_eq!(ts.is_persistent_text, Some(true));
}

#[tokio::test]
async fn test_get_video_encoder_configuration_parses_guaranteed_frame_rate() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trt:GetVideoEncoderConfigurationResponse>
             <trt:Configuration token="VideoEnc_1">
               <tt:Name>Main</tt:Name>
               <tt:UseCount>1</tt:UseCount>
               <tt:Encoding>H264</tt:Encoding>
               <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
               <tt:Quality>4.0</tt:Quality>
               <tt:GuaranteedFrameRate>true</tt:GuaranteedFrameRate>
             </trt:Configuration>
           </trt:GetVideoEncoderConfigurationResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let cfg = client
        .get_video_encoder_configuration("http://192.168.1.1/onvif/media_service", "VideoEnc_1")
        .await
        .unwrap();
    assert_eq!(cfg.guaranteed_frame_rate, Some(true));
}

#[tokio::test]
async fn test_get_video_encoder_configuration_no_guaranteed_frame_rate_is_none() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trt:GetVideoEncoderConfigurationResponse>
             <trt:Configuration token="VideoEnc_1">
               <tt:Name>Main</tt:Name>
               <tt:UseCount>1</tt:UseCount>
               <tt:Encoding>H264</tt:Encoding>
               <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
               <tt:Quality>4.0</tt:Quality>
             </trt:Configuration>
           </trt:GetVideoEncoderConfigurationResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let cfg = client
        .get_video_encoder_configuration("http://192.168.1.1/onvif/media_service", "VideoEnc_1")
        .await
        .unwrap();
    assert!(cfg.guaranteed_frame_rate.is_none());
}

// ── XML escape security tests ─────────────────────────────────────────────

#[tokio::test]
async fn test_get_stream_uri_escapes_profile_token() {
    let (transport, captured) = RecordingTransport::new(stream_uri_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .get_stream_uri("http://192.168.1.1/onvif/media_service", "tok<&>en")
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(
        body.contains("tok&lt;&amp;&gt;en"),
        "XML special chars in profile_token must be escaped: {body}"
    );
    assert!(
        !body.contains("tok<&>en"),
        "raw special chars must not appear in XML body"
    );
}

// ── get_osds sends ConfigurationToken (not OSDToken) ──────────────────────

#[tokio::test]
async fn test_get_osds_sends_configuration_token_element() {
    let (transport, captured) = RecordingTransport::new(get_osds_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .get_osds("http://192.168.1.1/onvif/media", Some("VSC_1"))
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(
        body.contains("<trt:ConfigurationToken>VSC_1</trt:ConfigurationToken>"),
        "get_osds must use ConfigurationToken per ONVIF spec, not OSDToken: {body}"
    );
    assert!(
        !body.contains("OSDToken"),
        "OSDToken element must not appear in GetOSDs request"
    );
}

#[tokio::test]
async fn test_get_osds_without_filter_sends_no_token() {
    let (transport, captured) = RecordingTransport::new(get_osds_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .get_osds("http://192.168.1.1/onvif/media", None)
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(
        !body.contains("ConfigurationToken"),
        "no filter token should be sent when config_token is None: {body}"
    );
}

// ── Mock self-consistency: the mock's own responses parse via the client ──
// These round-trip through the real dispatching mock (crate::mock::MockTransport)
// and re-parse with the client, guarding against mock/parser drift that the
// hand-written fixtures above cannot catch (they test the parser against
// known-good XML, never the mock's actual output).

#[cfg(feature = "mock")]
#[tokio::test]
async fn mock_get_osd_response_parses_via_client() {
    let client = OnvifClient::new("http://mock/onvif/device")
        .with_transport(Arc::new(crate::mock::MockTransport::new()));
    // The mock's GetOSDResponse must wrap the entry as <trt:OSD> (WSDL element
    // name), not <trt:OSDConfiguration> (the schema type) — else get_osd's
    // `resp.child("OSD")` misses and the parse fails.
    let osd = client
        .get_osd("http://mock/onvif/media", "OSD_1")
        .await
        .unwrap();
    assert_eq!(osd.token, "OSD_1");
}
