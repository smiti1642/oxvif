use super::*;
use async_trait::async_trait;
use futures::StreamExt as _;
use std::sync::{Arc, Mutex};

use crate::transport::TransportError;
use crate::types::RecordingJobConfiguration;

// ── MockTransport: returns a fixed XML string ─────────────────────────────

struct MockTransport {
    response: String,
}

#[async_trait]
impl Transport for MockTransport {
    async fn soap_post(
        &self,
        _url: &str,
        _action: &str,
        _body: String,
    ) -> Result<String, TransportError> {
        Ok(self.response.clone())
    }
}

fn mock(xml: &str) -> Arc<dyn Transport> {
    Arc::new(MockTransport {
        response: xml.to_string(),
    })
}

// ── RecordingTransport: records the last call for assertion ───────────────

#[derive(Default)]
struct Captured {
    url: String,
    action: String,
    body: String,
}

struct RecordingTransport {
    response: String,
    captured: Arc<Mutex<Captured>>,
}

impl RecordingTransport {
    fn new(response: &str) -> (Arc<Self>, Arc<Mutex<Captured>>) {
        let captured = Arc::new(Mutex::new(Captured::default()));
        let t = Arc::new(Self {
            response: response.to_string(),
            captured: captured.clone(),
        });
        (t, captured)
    }
}

#[async_trait]
impl Transport for RecordingTransport {
    async fn soap_post(
        &self,
        url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        let mut c = self.captured.lock().unwrap();
        c.url = url.to_string();
        c.action = action.to_string();
        c.body = body;
        Ok(self.response.clone())
    }
}

// ── ErrorTransport: always fails with a given HTTP status ─────────────────

struct ErrorTransport {
    status: u16,
}

#[async_trait]
impl Transport for ErrorTransport {
    async fn soap_post(
        &self,
        _url: &str,
        _action: &str,
        _body: String,
    ) -> Result<String, TransportError> {
        Err(TransportError::HttpStatus {
            status: self.status,
            body: format!("HTTP {}", self.status),
        })
    }
}

// ── XML response fixtures ─────────────────────────────────────────────────

fn capabilities_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tds:GetCapabilitiesResponse>
              <tds:Capabilities>
                <tt:Device> <tt:XAddr>http://192.168.1.1/onvif/device_service</tt:XAddr> </tt:Device>
                <tt:Media>  <tt:XAddr>http://192.168.1.1/onvif/media_service</tt:XAddr>  </tt:Media>
                <tt:PTZ>    <tt:XAddr>http://192.168.1.1/onvif/ptz_service</tt:XAddr>    </tt:PTZ>
              </tds:Capabilities>
            </tds:GetCapabilitiesResponse>
          </s:Body>
        </s:Envelope>"#
}

fn device_info_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
          <s:Body>
            <tds:GetDeviceInformationResponse>
              <tds:Manufacturer>Hikvision</tds:Manufacturer>
              <tds:Model>DS-2CD2085G1-I</tds:Model>
              <tds:FirmwareVersion>V5.6.1</tds:FirmwareVersion>
              <tds:SerialNumber>SN123456</tds:SerialNumber>
              <tds:HardwareId>0x00</tds:HardwareId>
            </tds:GetDeviceInformationResponse>
          </s:Body>
        </s:Envelope>"#
}

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

fn soap_fault_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
          <s:Body>
            <s:Fault>
              <s:Code><s:Value>s:Sender</s:Value></s:Code>
              <s:Reason><s:Text xml:lang="en">Not Authorized</s:Text></s:Reason>
            </s:Fault>
          </s:Body>
        </s:Envelope>"#
}

// ── get_capabilities ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_capabilities_returns_correct_urls() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(capabilities_xml()));

    let caps = client.get_capabilities().await.unwrap();
    assert_eq!(
        caps.device.url.as_deref(),
        Some("http://192.168.1.1/onvif/device_service")
    );
    assert_eq!(
        caps.media.url.as_deref(),
        Some("http://192.168.1.1/onvif/media_service")
    );
    assert_eq!(
        caps.ptz_url.as_deref(),
        Some("http://192.168.1.1/onvif/ptz_service")
    );
}

#[tokio::test]
async fn test_get_capabilities_soap_fault_returns_error() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(soap_fault_xml()));

    let err = client.get_capabilities().await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::Fault { .. })
    ));
}

#[tokio::test]
async fn test_get_capabilities_transport_error_propagates() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(Arc::new(ErrorTransport { status: 503 }));

    let err = client.get_capabilities().await.unwrap_err();
    assert!(matches!(err, OnvifError::Transport(_)));
}

// ── WS-Security ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_credentials_add_ws_security_header() {
    let (transport, captured) = RecordingTransport::new(capabilities_xml());
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_credentials("admin", "password")
        .with_transport(transport);

    client.get_capabilities().await.unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(
        body.contains("<wsse:Security>"),
        "WS-Security element must be present"
    );
    assert!(body.contains("<wsse:Username>admin</wsse:Username>"));
}

#[tokio::test]
async fn test_no_credentials_omits_security_header() {
    let (transport, captured) = RecordingTransport::new(capabilities_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.get_capabilities().await.unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(
        !body.contains("<wsse:Security>"),
        "no credentials → no security header"
    );
}

// ── get_device_info ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_device_info_returns_correct_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(device_info_xml()));

    let info = client.get_device_info().await.unwrap();
    assert_eq!(info.manufacturer, "Hikvision");
    assert_eq!(info.model, "DS-2CD2085G1-I");
    assert_eq!(info.firmware_version, "V5.6.1");
    assert_eq!(info.serial_number, "SN123456");
    assert_eq!(info.hardware_id, "0x00");
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

fn empty_response_xml(tag: &str) -> String {
    format!(
        r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:trt="http://www.onvif.org/ver10/media/wsdl">
              <s:Body><trt:{tag}/></s:Body>
            </s:Envelope>"#
    )
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

// ── Device management fixtures ────────────────────────────────────────────

fn hostname_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tds:GetHostnameResponse>
              <tds:HostnameInformation>
                <tt:FromDHCP>false</tt:FromDHCP>
                <tt:Name>ONVIF-Camera</tt:Name>
              </tds:HostnameInformation>
            </tds:GetHostnameResponse>
          </s:Body>
        </s:Envelope>"#
}

fn hostname_dhcp_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tds:GetHostnameResponse>
              <tds:HostnameInformation>
                <tt:FromDHCP>true</tt:FromDHCP>
              </tds:HostnameInformation>
            </tds:GetHostnameResponse>
          </s:Body>
        </s:Envelope>"#
}

fn ntp_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tds:GetNTPResponse>
              <tds:NTPInformation>
                <tt:FromDHCP>false</tt:FromDHCP>
                <tt:NTPManual>
                  <tt:Type>DNS</tt:Type>
                  <tt:DNSname>pool.ntp.org</tt:DNSname>
                </tt:NTPManual>
                <tt:NTPManual>
                  <tt:Type>IPv4</tt:Type>
                  <tt:IPv4Address>192.168.1.1</tt:IPv4Address>
                </tt:NTPManual>
              </tds:NTPInformation>
            </tds:GetNTPResponse>
          </s:Body>
        </s:Envelope>"#
}

fn system_reboot_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
          <s:Body>
            <tds:SystemRebootResponse>
              <tds:Message>Rebooting in 30 seconds</tds:Message>
            </tds:SystemRebootResponse>
          </s:Body>
        </s:Envelope>"#
}

// ── get_hostname ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_hostname_returns_name_and_flag() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(hostname_xml()));

    let h = client.get_hostname().await.unwrap();
    assert!(!h.from_dhcp);
    assert_eq!(h.name.as_deref(), Some("ONVIF-Camera"));
}

#[tokio::test]
async fn test_get_hostname_dhcp_no_name() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(hostname_dhcp_xml()));

    let h = client.get_hostname().await.unwrap();
    assert!(h.from_dhcp);
    assert!(h.name.is_none());
}

#[tokio::test]
async fn test_get_hostname_uses_device_url() {
    let (transport, captured) = RecordingTransport::new(hostname_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.get_hostname().await.unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.url, "http://192.168.1.1/onvif/device_service");
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/GetHostname"
    );
}

// ── set_hostname ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_set_hostname_sends_name() {
    let set_xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
          <s:Body><tds:SetHostnameResponse/></s:Body>
        </s:Envelope>"#;

    let (transport, captured) = RecordingTransport::new(set_xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.set_hostname("NewName").await.unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("NewName"));
}

// ── get_ntp ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_ntp_returns_servers() {
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(ntp_xml()));

    let ntp = client.get_ntp().await.unwrap();
    assert!(!ntp.from_dhcp);
    assert_eq!(ntp.servers.len(), 2);
    assert_eq!(ntp.servers[0], "pool.ntp.org");
    assert_eq!(ntp.servers[1], "192.168.1.1");
}

// ── set_ntp ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_set_ntp_sends_from_dhcp_false_and_servers() {
    let set_xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
          <s:Body><tds:SetNTPResponse/></s:Body>
        </s:Envelope>"#;

    let (transport, captured) = RecordingTransport::new(set_xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_ntp(false, &["pool.ntp.org", "time.google.com"])
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("<tds:FromDHCP>false</tds:FromDHCP>"));
    assert!(body.contains("pool.ntp.org"));
    assert!(body.contains("time.google.com"));
}

#[tokio::test]
async fn test_set_ntp_from_dhcp_true_sends_no_servers() {
    let set_xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
          <s:Body><tds:SetNTPResponse/></s:Body>
        </s:Envelope>"#;

    let (transport, captured) = RecordingTransport::new(set_xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.set_ntp(true, &[]).await.unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("<tds:FromDHCP>true</tds:FromDHCP>"));
    assert!(!body.contains("NTPManual"));
}

// ── system_reboot ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_system_reboot_returns_message() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(system_reboot_xml()));

    let msg = client.system_reboot().await.unwrap();
    assert_eq!(msg, "Rebooting in 30 seconds");
}

#[tokio::test]
async fn test_system_reboot_uses_device_url() {
    let (transport, captured) = RecordingTransport::new(system_reboot_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.system_reboot().await.unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.url, "http://192.168.1.1/onvif/device_service");
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SystemReboot"
    );
}

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

// ── Events service fixtures ───────────────────────────────────────────────

fn event_properties_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tev="http://www.onvif.org/ver10/events/wsdl">
          <s:Body>
            <tev:GetEventPropertiesResponse>
              <tev:TopicSet>
                <tns1:VideoSource xmlns:tns1="http://www.onvif.org/ver10/topics">
                  <tns1:MotionAlarm/>
                  <tns1:ImageTooBlurry/>
                </tns1:VideoSource>
                <tns1:RuleEngine xmlns:tns1="http://www.onvif.org/ver10/topics">
                  <tns1:Cell>
                    <tns1:Motion/>
                  </tns1:Cell>
                </tns1:RuleEngine>
              </tev:TopicSet>
            </tev:GetEventPropertiesResponse>
          </s:Body>
        </s:Envelope>"#
}

fn create_pull_point_subscription_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tev="http://www.onvif.org/ver10/events/wsdl"
                      xmlns:wsa="http://www.w3.org/2005/08/addressing">
          <s:Body>
            <tev:CreatePullPointSubscriptionResponse>
              <tev:SubscriptionReference>
                <wsa:Address>http://192.168.1.1/onvif/events/subscription_1</wsa:Address>
              </tev:SubscriptionReference>
              <tev:CurrentTime>2024-01-01T00:00:00Z</tev:CurrentTime>
              <tev:TerminationTime>2024-01-01T00:01:00Z</tev:TerminationTime>
            </tev:CreatePullPointSubscriptionResponse>
          </s:Body>
        </s:Envelope>"#
}

fn pull_messages_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tev="http://www.onvif.org/ver10/events/wsdl"
                      xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tev:PullMessagesResponse>
              <tev:CurrentTime>2024-01-01T00:00:10Z</tev:CurrentTime>
              <tev:TerminationTime>2024-01-01T00:01:00Z</tev:TerminationTime>
              <wsnt:NotificationMessage>
                <wsnt:Topic>tns1:VideoSource/MotionAlarm</wsnt:Topic>
                <wsnt:Message>
                  <tt:Message UtcTime="2024-01-01T00:00:09Z">
                    <tt:Source>
                      <tt:SimpleItem Name="VideoSourceToken" Value="VideoSource_1"/>
                    </tt:Source>
                    <tt:Data>
                      <tt:SimpleItem Name="IsMotion" Value="true"/>
                    </tt:Data>
                  </tt:Message>
                </wsnt:Message>
              </wsnt:NotificationMessage>
            </tev:PullMessagesResponse>
          </s:Body>
        </s:Envelope>"#
}

fn pull_messages_empty_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tev="http://www.onvif.org/ver10/events/wsdl">
          <s:Body>
            <tev:PullMessagesResponse>
              <tev:CurrentTime>2024-01-01T00:00:10Z</tev:CurrentTime>
              <tev:TerminationTime>2024-01-01T00:01:00Z</tev:TerminationTime>
            </tev:PullMessagesResponse>
          </s:Body>
        </s:Envelope>"#
}

fn renew_subscription_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2">
          <s:Body>
            <wsnt:RenewResponse>
              <wsnt:TerminationTime>2024-01-01T00:02:00Z</wsnt:TerminationTime>
            </wsnt:RenewResponse>
          </s:Body>
        </s:Envelope>"#
}

// ── get_event_properties ──────────────────────────────────────────────────

#[tokio::test]
async fn test_get_event_properties_flattens_topics() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(event_properties_xml()));

    let props = client
        .get_event_properties("http://192.168.1.1/onvif/events_service")
        .await
        .unwrap();

    assert!(
        props.topics.iter().any(|t| t.contains("MotionAlarm")),
        "topics should contain MotionAlarm"
    );
    assert!(
        props.topics.iter().any(|t| t.contains("Motion")),
        "topics should contain nested Motion topic"
    );
}

// ── create_pull_point_subscription ────────────────────────────────────────

#[tokio::test]
async fn test_create_pull_point_subscription_returns_reference_url() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(create_pull_point_subscription_xml()));

    let sub = client
        .create_pull_point_subscription(
            "http://192.168.1.1/onvif/events_service",
            None,
            Some("PT60S"),
        )
        .await
        .unwrap();

    assert_eq!(
        sub.reference_url,
        "http://192.168.1.1/onvif/events/subscription_1"
    );
    assert_eq!(sub.termination_time, "2024-01-01T00:01:00Z");
}

#[tokio::test]
async fn test_create_pull_point_subscription_with_filter() {
    let (transport, captured) = RecordingTransport::new(create_pull_point_subscription_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .create_pull_point_subscription(
            "http://192.168.1.1/onvif/events_service",
            Some("tns1:VideoSource/MotionAlarm"),
            None,
        )
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("tns1:VideoSource/MotionAlarm"));
    assert!(body.contains("TopicExpression"));
}

#[tokio::test]
async fn test_create_pull_point_subscription_without_filter_omits_filter_el() {
    let (transport, captured) = RecordingTransport::new(create_pull_point_subscription_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .create_pull_point_subscription("http://192.168.1.1/onvif/events_service", None, None)
        .await
        .unwrap();

    assert!(!captured.lock().unwrap().body.contains("Filter"));
}

// ── pull_messages ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_pull_messages_parses_notification() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(pull_messages_xml()));

    let msgs = client
        .pull_messages(
            "http://192.168.1.1/onvif/events/subscription_1",
            "PT5S",
            100,
        )
        .await
        .unwrap();

    assert_eq!(msgs.len(), 1);
    assert!(msgs[0].topic.contains("MotionAlarm"));
    assert_eq!(msgs[0].utc_time, "2024-01-01T00:00:09Z");
    assert_eq!(
        msgs[0].source.get("VideoSourceToken").map(String::as_str),
        Some("VideoSource_1")
    );
    assert_eq!(
        msgs[0].data.get("IsMotion").map(String::as_str),
        Some("true")
    );
}

#[tokio::test]
async fn test_pull_messages_empty_returns_empty_vec() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(pull_messages_empty_xml()));

    let msgs = client
        .pull_messages(
            "http://192.168.1.1/onvif/events/subscription_1",
            "PT5S",
            100,
        )
        .await
        .unwrap();

    assert!(msgs.is_empty());
}

#[tokio::test]
async fn test_pull_messages_sends_timeout_and_limit() {
    let (transport, captured) = RecordingTransport::new(pull_messages_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .pull_messages(
            "http://192.168.1.1/onvif/events/subscription_1",
            "PT10S",
            50,
        )
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("PT10S"));
    assert!(body.contains("50"));
}

// ── renew_subscription ────────────────────────────────────────────────────

#[tokio::test]
async fn test_renew_subscription_returns_new_termination_time() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(renew_subscription_xml()));

    let new_time = client
        .renew_subscription("http://192.168.1.1/onvif/events/subscription_1", "PT60S")
        .await
        .unwrap();

    assert_eq!(new_time, "2024-01-01T00:02:00Z");
}

#[tokio::test]
async fn test_renew_subscription_sends_termination_time() {
    let (transport, captured) = RecordingTransport::new(renew_subscription_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .renew_subscription("http://192.168.1.1/onvif/events/subscription_1", "PT120S")
        .await
        .unwrap();

    assert!(captured.lock().unwrap().body.contains("PT120S"));
}

// ── unsubscribe ───────────────────────────────────────────────────────────

// ── Negative / error-path tests ───────────────────────────────────────────────

// ── Malformed XML ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_capabilities_malformed_xml_returns_err() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock("this is not xml at all"));
    let result = client.get_capabilities().await;
    assert!(result.is_err(), "expected Err on malformed XML");
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

// ── SOAP Fault ────────────────────────────────────────────────────────────

fn make_soap_fault_xml(code: &str, reason: &str) -> String {
    format!(
        r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
             <s:Body>
               <s:Fault>
                 <s:Code><s:Value>{code}</s:Value></s:Code>
                 <s:Reason><s:Text xml:lang="en">{reason}</s:Text></s:Reason>
               </s:Fault>
             </s:Body>
           </s:Envelope>"#
    )
}

#[tokio::test]
async fn test_get_capabilities_soap_fault_returns_err() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(
        &make_soap_fault_xml("s:Sender", "Sender not Authorized"),
    ));
    let result = client.get_capabilities().await;
    assert!(
        matches!(
            result,
            Err(OnvifError::Soap(crate::soap::SoapError::Fault { ref code, .. }))
            if code == "s:Sender"
        ),
        "expected SOAP Fault error, got: {result:?}"
    );
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

// ── HTTP transport error ──────────────────────────────────────────────────

#[tokio::test]
async fn test_get_capabilities_http_error_returns_err() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(Arc::new(ErrorTransport { status: 401 }));
    let result = client.get_capabilities().await;
    assert!(
        matches!(
            result,
            Err(OnvifError::Transport(
                crate::transport::TransportError::HttpStatus { status: 401, .. }
            ))
        ),
        "expected HTTP 401 transport error"
    );
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
    assert!(result.is_err());
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
    assert!(result.is_err());
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

    assert!(matches!(err, crate::error::OnvifError::Soap(_)));
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

    assert!(matches!(err, crate::error::OnvifError::Soap(_)));
}

// ── OSD ───────────────────────────────────────────────────────────────────────

fn get_osds_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trt:GetOSDsResponse>
              <trt:OSDConfiguration token="osd_1">
                <tt:VideoSourceConfigurationToken>vsc_1</tt:VideoSourceConfigurationToken>
                <tt:Type>Text</tt:Type>
                <tt:Position>
                  <tt:Type>UpperLeft</tt:Type>
                </tt:Position>
                <tt:TextString>
                  <tt:Type>DateAndTime</tt:Type>
                  <tt:DateFormat>MM/DD/YYYY</tt:DateFormat>
                </tt:TextString>
              </trt:OSDConfiguration>
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
              <trt:OSDConfiguration>
                <tt:Type xmlns:tt="http://www.onvif.org/ver10/schema">Text</tt:Type>
              </trt:OSDConfiguration>
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

// ── get_scopes ────────────────────────────────────────────────────────────────

fn get_scopes_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tds:GetScopesResponse>
              <tds:Scopes>
                <tt:ScopeAttribute>Fixed</tt:ScopeAttribute>
                <tt:ScopeItem>onvif://www.onvif.org/name/Camera1</tt:ScopeItem>
              </tds:Scopes>
              <tds:Scopes>
                <tt:ScopeAttribute>Fixed</tt:ScopeAttribute>
                <tt:ScopeItem>onvif://www.onvif.org/location/country/taiwan</tt:ScopeItem>
              </tds:Scopes>
            </tds:GetScopesResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_get_scopes_returns_uris() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_scopes_xml()));

    let scopes = client.get_scopes().await.unwrap();

    assert_eq!(scopes.len(), 2);
    assert!(scopes[0].contains("name/Camera1"));
    assert!(scopes[1].contains("country/taiwan"));
}

// ── get_recordings ────────────────────────────────────────────────────────────

fn get_recordings_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trc="http://www.onvif.org/ver10/recording/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trc:GetRecordingsResponse>
              <trc:RecordingItems Token="rec_001">
                <trc:RecordingInformation>
                  <tt:Source>
                    <tt:SourceId>urn:uuid:source-1</tt:SourceId>
                    <tt:Name>Channel 1</tt:Name>
                    <tt:Location>Entrance</tt:Location>
                    <tt:Description>Front door camera</tt:Description>
                  </tt:Source>
                  <tt:EarliestRecording>2026-01-01T00:00:00Z</tt:EarliestRecording>
                  <tt:LatestRecording>2026-01-02T00:00:00Z</tt:LatestRecording>
                  <tt:Content>Motion event</tt:Content>
                  <tt:RecordingStatus>Stopped</tt:RecordingStatus>
                </trc:RecordingInformation>
              </trc:RecordingItems>
            </trc:GetRecordingsResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_get_recordings_parses_item() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_recordings_xml()));

    let recs = client
        .get_recordings("http://192.168.1.1/onvif/recording")
        .await
        .unwrap();

    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0].token, "rec_001");
    assert_eq!(recs[0].source.name, "Channel 1");
    assert_eq!(recs[0].recording_status, "Stopped");
    assert_eq!(
        recs[0].earliest_recording.as_deref(),
        Some("2026-01-01T00:00:00Z")
    );
}

#[tokio::test]
async fn test_get_recordings_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
          <s:Body>
            <trc:GetRecordingsResponse>
              <trc:RecordingItems>
              </trc:RecordingItems>
            </trc:GetRecordingsResponse>
          </s:Body>
        </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let err = client
        .get_recordings("http://192.168.1.1/onvif/recording")
        .await
        .unwrap_err();

    assert!(matches!(err, crate::error::OnvifError::Soap(_)));
}

// ── find_recordings / get_recording_search_results / end_search ───────────────

fn find_recordings_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tse="http://www.onvif.org/ver10/search/wsdl">
          <s:Body>
            <tse:FindRecordingsResponse>
              <tse:SearchToken>search_abc123</tse:SearchToken>
            </tse:FindRecordingsResponse>
          </s:Body>
        </s:Envelope>"#
}

fn recording_search_results_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tse="http://www.onvif.org/ver10/search/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tse:GetRecordingSearchResultsResponse>
              <tse:SearchState>Completed</tse:SearchState>
              <tse:RecordingInformation>
                <tt:RecordingToken>rec_001</tt:RecordingToken>
                <tt:Source>
                  <tt:Name>Channel 1</tt:Name>
                </tt:Source>
                <tt:EarliestRecording>2026-01-01T00:00:00Z</tt:EarliestRecording>
                <tt:LatestRecording>2026-01-02T00:00:00Z</tt:LatestRecording>
                <tt:Content>Motion event</tt:Content>
                <tt:RecordingStatus>Stopped</tt:RecordingStatus>
              </tse:RecordingInformation>
            </tse:GetRecordingSearchResultsResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_find_recordings_returns_token() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(find_recordings_xml()));

    let token = client
        .find_recordings("http://192.168.1.1/onvif/search", None, "PT60S")
        .await
        .unwrap();

    assert_eq!(token, "search_abc123");
}

#[tokio::test]
async fn test_find_recordings_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
                   <s:Body><tse:FindRecordingsResponse/></s:Body>
                 </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let err = client
        .find_recordings("http://192.168.1.1/onvif/search", None, "PT60S")
        .await
        .unwrap_err();

    assert!(matches!(err, crate::error::OnvifError::Soap(_)));
}

#[tokio::test]
async fn test_get_recording_search_results_parses_completed() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(recording_search_results_xml()));

    let results = client
        .get_recording_search_results(
            "http://192.168.1.1/onvif/search",
            "search_abc123",
            100,
            "PT5S",
        )
        .await
        .unwrap();

    assert_eq!(results.search_state, "Completed");
    assert_eq!(results.recording_information.len(), 1);
    assert_eq!(results.recording_information[0].recording_token, "rec_001");
    assert_eq!(results.recording_information[0].source_name, "Channel 1");
}

#[tokio::test]
async fn test_end_search_ok() {
    let xml = empty_response_xml("EndSearchResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .end_search("http://192.168.1.1/onvif/search", "search_abc123")
        .await
        .unwrap();

    assert!(captured.lock().unwrap().body.contains("search_abc123"));
}

// ── get_replay_uri ────────────────────────────────────────────────────────────

fn get_replay_uri_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trp="http://www.onvif.org/ver10/replay/wsdl">
          <s:Body>
            <trp:GetReplayUriResponse>
              <trp:Uri>rtsp://192.168.1.1/replay/rec_001</trp:Uri>
            </trp:GetReplayUriResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_get_replay_uri_returns_rtsp() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_replay_uri_xml()));

    let uri = client
        .get_replay_uri(
            "http://192.168.1.1/onvif/replay",
            "rec_001",
            "RTP-Unicast",
            "RTSP",
        )
        .await
        .unwrap();

    assert_eq!(uri, "rtsp://192.168.1.1/replay/rec_001");
}

#[tokio::test]
async fn test_get_replay_uri_missing_uri_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
                   <s:Body><trp:GetReplayUriResponse/></s:Body>
                 </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let err = client
        .get_replay_uri(
            "http://192.168.1.1/onvif/replay",
            "rec_001",
            "RTP-Unicast",
            "RTSP",
        )
        .await
        .unwrap_err();

    assert!(matches!(err, crate::error::OnvifError::Soap(_)));
}

// ── get_users ─────────────────────────────────────────────────────────────────

fn get_users_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetUsersResponse>
             <tds:User>
               <tt:Username>admin</tt:Username>
               <tt:UserLevel>Administrator</tt:UserLevel>
             </tds:User>
             <tds:User>
               <tt:Username>operator</tt:Username>
               <tt:UserLevel>Operator</tt:UserLevel>
             </tds:User>
           </tds:GetUsersResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_users_returns_list() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_users_xml()));

    let users = client.get_users().await.unwrap();
    assert_eq!(users.len(), 2);
    assert_eq!(users[0].username, "admin");
    assert_eq!(users[0].user_level, "Administrator");
    assert_eq!(users[1].username, "operator");
}

// ── create_users ──────────────────────────────────────────────────────────────

fn create_users_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:CreateUsersResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_create_users_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(create_users_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .create_users(&[("newuser", "pass123", "Operator")])
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/CreateUsers"
    );
    assert!(c.body.contains("<tt:Username>newuser</tt:Username>"));
    assert!(c.body.contains("<tt:UserLevel>Operator</tt:UserLevel>"));
}

// ── delete_users ──────────────────────────────────────────────────────────────

fn delete_users_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:DeleteUsersResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_delete_users_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(delete_users_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.delete_users(&["operator"]).await.unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/DeleteUsers"
    );
    assert!(c.body.contains("<tds:Username>operator</tds:Username>"));
}

#[tokio::test]
async fn test_delete_users_transport_error() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(Arc::new(ErrorTransport { status: 500 }));
    let err = client.delete_users(&["operator"]).await.unwrap_err();
    assert!(matches!(err, OnvifError::Transport(_)));
}

// ── set_user ─────────────────────────────────────────────────────────────────

fn set_user_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetUserResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_user_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_user_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_user("admin", Some("newpass"), "Administrator")
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.action, "http://www.onvif.org/ver10/device/wsdl/SetUser");
    assert!(c.body.contains("<tt:Username>admin</tt:Username>"));
    assert!(c.body.contains("<tt:Password>newpass</tt:Password>"));
    assert!(
        c.body
            .contains("<tt:UserLevel>Administrator</tt:UserLevel>")
    );
}

// ── get_network_interfaces ────────────────────────────────────────────────────

fn get_network_interfaces_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetNetworkInterfacesResponse>
             <tds:NetworkInterfaces token="eth0">
               <tt:Enabled>true</tt:Enabled>
               <tt:Info>
                 <tt:Name>eth0</tt:Name>
                 <tt:HwAddress>00:11:22:33:44:55</tt:HwAddress>
                 <tt:MTU>1500</tt:MTU>
               </tt:Info>
               <tt:IPv4>
                 <tt:Enabled>true</tt:Enabled>
                 <tt:Config>
                   <tt:FromDHCP>false</tt:FromDHCP>
                   <tt:Manual>
                     <tt:Address>192.168.1.100</tt:Address>
                     <tt:PrefixLength>24</tt:PrefixLength>
                   </tt:Manual>
                 </tt:Config>
               </tt:IPv4>
             </tds:NetworkInterfaces>
           </tds:GetNetworkInterfacesResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_network_interfaces_returns_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_network_interfaces_xml()));

    let ifaces = client.get_network_interfaces().await.unwrap();
    assert_eq!(ifaces.len(), 1);
    let iface = &ifaces[0];
    assert_eq!(iface.token, "eth0");
    assert!(iface.enabled);
    assert_eq!(iface.name, "eth0");
    assert_eq!(iface.hw_address, "00:11:22:33:44:55");
    assert_eq!(iface.mtu, 1500);
    assert_eq!(iface.ipv4_address, "192.168.1.100");
    assert_eq!(iface.ipv4_prefix_length, 24);
    assert!(!iface.ipv4_from_dhcp);
}

#[tokio::test]
async fn test_get_network_interfaces_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetNetworkInterfacesResponse>
             <tds:NetworkInterfaces>
               <tt:Enabled>true</tt:Enabled>
             </tds:NetworkInterfaces>
           </tds:GetNetworkInterfacesResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client.get_network_interfaces().await.unwrap_err();
    assert!(matches!(err, OnvifError::Soap(_)));
}

// ── set_network_interfaces ────────────────────────────────────────────────────

fn set_network_interfaces_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:SetNetworkInterfacesResponse>
             <tds:RebootNeeded>false</tds:RebootNeeded>
           </tds:SetNetworkInterfacesResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_network_interfaces_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_network_interfaces_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    let reboot = client
        .set_network_interfaces("eth0", true, "192.168.1.200", 24, false)
        .await
        .unwrap();

    assert!(!reboot);
    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SetNetworkInterfaces"
    );
    assert!(
        c.body
            .contains("<tds:InterfaceToken>eth0</tds:InterfaceToken>")
    );
    assert!(c.body.contains("192.168.1.200"));
}

#[tokio::test]
async fn test_set_network_interfaces_reboot_needed() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body>
           <tds:SetNetworkInterfacesResponse>
             <tds:RebootNeeded>true</tds:RebootNeeded>
           </tds:SetNetworkInterfacesResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let reboot = client
        .set_network_interfaces("eth0", true, "10.0.0.1", 8, false)
        .await
        .unwrap();
    assert!(reboot);
}

// ── get_network_protocols ─────────────────────────────────────────────────────

fn get_network_protocols_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetNetworkProtocolsResponse>
             <tds:NetworkProtocols>
               <tt:Name>HTTP</tt:Name>
               <tt:Enabled>true</tt:Enabled>
               <tt:Port>80</tt:Port>
             </tds:NetworkProtocols>
             <tds:NetworkProtocols>
               <tt:Name>RTSP</tt:Name>
               <tt:Enabled>true</tt:Enabled>
               <tt:Port>554</tt:Port>
             </tds:NetworkProtocols>
           </tds:GetNetworkProtocolsResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_network_protocols_returns_list() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_network_protocols_xml()));

    let protos = client.get_network_protocols().await.unwrap();
    assert_eq!(protos.len(), 2);
    assert_eq!(protos[0].name, "HTTP");
    assert!(protos[0].enabled);
    assert_eq!(protos[0].ports, vec![80]);
    assert_eq!(protos[1].name, "RTSP");
    assert_eq!(protos[1].ports, vec![554]);
}

// ── get_dns ───────────────────────────────────────────────────────────────────

fn get_dns_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetDNSResponse>
             <tds:DNSInformation>
               <tt:FromDHCP>false</tt:FromDHCP>
               <tt:DNSManual>
                 <tt:Type>IPv4</tt:Type>
                 <tt:IPv4Address>8.8.8.8</tt:IPv4Address>
               </tt:DNSManual>
               <tt:DNSManual>
                 <tt:Type>IPv4</tt:Type>
                 <tt:IPv4Address>8.8.4.4</tt:IPv4Address>
               </tt:DNSManual>
             </tds:DNSInformation>
           </tds:GetDNSResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_dns_returns_servers() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_dns_xml()));

    let dns = client.get_dns().await.unwrap();
    assert!(!dns.from_dhcp);
    assert_eq!(dns.servers, vec!["8.8.8.8", "8.8.4.4"]);
}

#[tokio::test]
async fn test_get_dns_missing_dns_information_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:GetDNSResponse/></s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client.get_dns().await.unwrap_err();
    assert!(matches!(err, OnvifError::Soap(_)));
}

// ── set_dns ───────────────────────────────────────────────────────────────────

fn set_dns_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetDNSResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_dns_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_dns_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_dns(false, &["1.1.1.1", "9.9.9.9"])
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.action, "http://www.onvif.org/ver10/device/wsdl/SetDNS");
    assert!(c.body.contains("<tds:FromDHCP>false</tds:FromDHCP>"));
    assert!(c.body.contains("<tt:IPv4Address>1.1.1.1</tt:IPv4Address>"));
    assert!(c.body.contains("<tt:IPv4Address>9.9.9.9</tt:IPv4Address>"));
}

// ── get_network_default_gateway ───────────────────────────────────────────────

fn get_network_default_gateway_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetNetworkDefaultGatewayResponse>
             <tds:NetworkGateway>
               <tt:IPv4Address>192.168.1.1</tt:IPv4Address>
             </tds:NetworkGateway>
           </tds:GetNetworkDefaultGatewayResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_network_default_gateway_returns_address() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_network_default_gateway_xml()));

    let gw = client.get_network_default_gateway().await.unwrap();
    assert_eq!(gw.ipv4_addresses, vec!["192.168.1.1"]);
    assert!(gw.ipv6_addresses.is_empty());
}

#[tokio::test]
async fn test_get_network_default_gateway_missing_node_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:GetNetworkDefaultGatewayResponse/></s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client.get_network_default_gateway().await.unwrap_err();
    assert!(matches!(err, OnvifError::Soap(_)));
}

// ── get_system_log ────────────────────────────────────────────────────────────

fn get_system_log_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetSystemLogResponse>
             <tds:SystemLog>
               <tt:String>2026-04-03 12:00:00 system started</tt:String>
             </tds:SystemLog>
           </tds:GetSystemLogResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_system_log_returns_string() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_system_log_xml()));

    let log = client.get_system_log("System").await.unwrap();
    assert_eq!(
        log.string.as_deref(),
        Some("2026-04-03 12:00:00 system started")
    );
}

#[tokio::test]
async fn test_get_system_log_missing_system_log_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:GetSystemLogResponse/></s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client.get_system_log("System").await.unwrap_err();
    assert!(matches!(err, OnvifError::Soap(_)));
}

// ── get_relay_outputs ─────────────────────────────────────────────────────────

fn get_relay_outputs_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetRelayOutputsResponse>
             <tds:RelayOutputs token="RelayOutput_1">
               <tt:Properties>
                 <tt:Mode>Bistable</tt:Mode>
                 <tt:DelayTime>PT0S</tt:DelayTime>
                 <tt:IdleState>open</tt:IdleState>
               </tt:Properties>
             </tds:RelayOutputs>
           </tds:GetRelayOutputsResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_relay_outputs_returns_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_relay_outputs_xml()));

    let relays = client.get_relay_outputs().await.unwrap();
    assert_eq!(relays.len(), 1);
    assert_eq!(relays[0].token, "RelayOutput_1");
    assert_eq!(relays[0].mode, "Bistable");
    assert_eq!(relays[0].idle_state, "open");
}

#[tokio::test]
async fn test_get_relay_outputs_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetRelayOutputsResponse>
             <tds:RelayOutputs>
               <tt:Properties><tt:Mode>Bistable</tt:Mode></tt:Properties>
             </tds:RelayOutputs>
           </tds:GetRelayOutputsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client.get_relay_outputs().await.unwrap_err();
    assert!(matches!(err, OnvifError::Soap(_)));
}

// ── set_relay_output_state ────────────────────────────────────────────────────

fn set_relay_output_state_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetRelayOutputStateResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_relay_output_state_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_relay_output_state_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_relay_output_state("RelayOutput_1", "active")
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SetRelayOutputState"
    );
    assert!(
        c.body
            .contains("<tds:RelayOutputToken>RelayOutput_1</tds:RelayOutputToken>")
    );
    assert!(
        c.body
            .contains("<tds:LogicalState>active</tds:LogicalState>")
    );
}

// ── set_relay_output_settings ─────────────────────────────────────────────────

fn set_relay_output_settings_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetRelayOutputSettingsResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_relay_output_settings_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_relay_output_settings_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_relay_output_settings("Relay_1", "Monostable", "PT2S", "open")
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SetRelayOutputSettings"
    );
    assert!(
        c.body
            .contains("<tds:RelayOutputToken>Relay_1</tds:RelayOutputToken>")
    );
    assert!(c.body.contains("<tt:Mode>Monostable</tt:Mode>"));
    assert!(c.body.contains("<tt:DelayTime>PT2S</tt:DelayTime>"));
    assert!(c.body.contains("<tt:IdleState>open</tt:IdleState>"));
}

// ── set_network_protocols ─────────────────────────────────────────────────────

fn set_network_protocols_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetNetworkProtocolsResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_network_protocols_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_network_protocols_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_network_protocols(&[("HTTP", true, &[80u32]), ("RTSP", true, &[554u32])])
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SetNetworkProtocols"
    );
    assert!(c.body.contains("<tt:Name>HTTP</tt:Name>"));
    assert!(c.body.contains("<tt:Name>RTSP</tt:Name>"));
    assert!(c.body.contains("<tt:Port>554</tt:Port>"));
}

// ── set_system_factory_default ────────────────────────────────────────────────

fn set_system_factory_default_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetSystemFactoryDefaultResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_system_factory_default_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_system_factory_default_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.set_system_factory_default("Soft").await.unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SetSystemFactoryDefault"
    );
    assert!(
        c.body
            .contains("<tds:FactoryDefault>Soft</tds:FactoryDefault>")
    );
}

// ── get_storage_configurations ────────────────────────────────────────────────

fn get_storage_configurations_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetStorageConfigurationsResponse>
             <tds:StorageConfigurations token="SD_01">
               <tt:StorageType>LocalStorage</tt:StorageType>
               <tt:LocalPath>/mnt/sd</tt:LocalPath>
               <tt:StorageUri></tt:StorageUri>
               <tt:UserInfo>
                 <tt:Username></tt:Username>
                 <tt:UseAnonymous>true</tt:UseAnonymous>
               </tt:UserInfo>
             </tds:StorageConfigurations>
           </tds:GetStorageConfigurationsResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_storage_configurations_returns_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_storage_configurations_xml()));
    let configs = client.get_storage_configurations().await.unwrap();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].token, "SD_01");
    assert_eq!(configs[0].storage_type, "LocalStorage");
    assert_eq!(configs[0].local_path, "/mnt/sd");
    assert!(configs[0].use_anonymous);
}

#[tokio::test]
async fn test_get_storage_configurations_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetStorageConfigurationsResponse>
             <tds:StorageConfigurations>
               <tt:StorageType>LocalStorage</tt:StorageType>
             </tds:StorageConfigurations>
           </tds:GetStorageConfigurationsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client.get_storage_configurations().await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

// ── set_storage_configuration ─────────────────────────────────────────────────

fn set_storage_configuration_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetStorageConfigurationResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_storage_configuration_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_storage_configuration_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_storage_configuration("SD_01", "LocalStorage", "/mnt/sd", "", "", true)
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SetStorageConfiguration"
    );
    assert!(
        c.body
            .contains("<tt:StorageType>LocalStorage</tt:StorageType>")
    );
    assert!(c.body.contains("<tt:LocalPath>/mnt/sd</tt:LocalPath>"));
    assert!(c.body.contains("<tt:UseAnonymous>true</tt:UseAnonymous>"));
}

// ── get_system_uris ───────────────────────────────────────────────────────────

fn get_system_uris_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body>
           <tds:GetSystemUrisResponse>
             <tds:FirmwareUpgrade>http://192.168.1.1/firmware</tds:FirmwareUpgrade>
             <tds:SystemLog>http://192.168.1.1/log</tds:SystemLog>
             <tds:SupportInfo>http://192.168.1.1/support</tds:SupportInfo>
           </tds:GetSystemUrisResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_system_uris_returns_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_system_uris_xml()));
    let uris = client.get_system_uris().await.unwrap();
    assert_eq!(
        uris.firmware_upgrade_uri.as_deref(),
        Some("http://192.168.1.1/firmware")
    );
    assert_eq!(
        uris.system_log_uri.as_deref(),
        Some("http://192.168.1.1/log")
    );
    assert_eq!(
        uris.support_info_uri.as_deref(),
        Some("http://192.168.1.1/support")
    );
}

// ── get_discovery_mode ────────────────────────────────────────────────────────

fn get_discovery_mode_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body>
           <tds:GetDiscoveryModeResponse>
             <tds:DiscoveryMode>Discoverable</tds:DiscoveryMode>
           </tds:GetDiscoveryModeResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_discovery_mode_returns_value() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_discovery_mode_xml()));
    let mode = client.get_discovery_mode().await.unwrap();
    assert_eq!(mode, "Discoverable");
}

// ── set_discovery_mode ────────────────────────────────────────────────────────

fn set_discovery_mode_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetDiscoveryModeResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_discovery_mode_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_discovery_mode_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.set_discovery_mode("NonDiscoverable").await.unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SetDiscoveryMode"
    );
    assert!(
        c.body
            .contains("<tds:DiscoveryMode>NonDiscoverable</tds:DiscoveryMode>")
    );
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
               <tt:VideoSourceConfiguration token="VideoSrc_1"/>
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
    assert_eq!(p.video_source_token.as_deref(), Some("VideoSrc_1"));
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
    assert!(p.video_source_token.is_none());
    assert!(p.video_encoder_token.is_none());
    assert!(p.audio_source_token.is_none());
    assert!(p.audio_encoder_token.is_none());
    assert!(p.ptz_config_token.is_none());
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

// DnsInformation search_domains

#[tokio::test]
async fn test_get_dns_parses_search_domains() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetDNSResponse>
             <tds:DNSInformation>
               <tt:FromDHCP>false</tt:FromDHCP>
               <tt:SearchDomain>example.com</tt:SearchDomain>
               <tt:SearchDomain>local</tt:SearchDomain>
               <tt:DNSManual>
                 <tt:Type>IPv4</tt:Type>
                 <tt:IPv4Address>1.1.1.1</tt:IPv4Address>
               </tt:DNSManual>
             </tds:DNSInformation>
           </tds:GetDNSResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let dns = client.get_dns().await.unwrap();
    assert_eq!(dns.search_domains, vec!["example.com", "local"]);
    assert_eq!(dns.servers, vec!["1.1.1.1"]);
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

// NetworkInterface IPv6

#[tokio::test]
async fn test_get_network_interfaces_parses_ipv6() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetNetworkInterfacesResponse>
             <tds:NetworkInterfaces token="eth0">
               <tt:Enabled>true</tt:Enabled>
               <tt:Info>
                 <tt:Name>eth0</tt:Name>
                 <tt:HwAddress>AA:BB:CC:DD:EE:FF</tt:HwAddress>
                 <tt:MTU>1500</tt:MTU>
               </tt:Info>
               <tt:IPv4>
                 <tt:Enabled>true</tt:Enabled>
                 <tt:Config>
                   <tt:FromDHCP>false</tt:FromDHCP>
                   <tt:Manual>
                     <tt:Address>10.0.0.1</tt:Address>
                     <tt:PrefixLength>8</tt:PrefixLength>
                   </tt:Manual>
                 </tt:Config>
               </tt:IPv4>
               <tt:IPv6>
                 <tt:Enabled>true</tt:Enabled>
                 <tt:Config>
                   <tt:DHCP>Stateful</tt:DHCP>
                   <tt:Manual>
                     <tt:Address>2001:db8::1</tt:Address>
                     <tt:PrefixLength>64</tt:PrefixLength>
                   </tt:Manual>
                 </tt:Config>
               </tt:IPv6>
             </tds:NetworkInterfaces>
           </tds:GetNetworkInterfacesResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let ifaces = client.get_network_interfaces().await.unwrap();
    let iface = &ifaces[0];
    assert!(iface.ipv6_enabled);
    assert!(iface.ipv6_from_dhcp);
    assert_eq!(iface.ipv6_address.as_deref(), Some("2001:db8::1"));
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
async fn test_get_recordings_parses_track_times_and_address() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trc:GetRecordingsResponse>
             <trc:RecordingItems Token="Rec_001">
               <tt:RecordingInformation>
                 <tt:Source>
                   <tt:SourceId>urn:uuid:camera-001</tt:SourceId>
                   <tt:Name>Camera 1</tt:Name>
                   <tt:Location>Entrance</tt:Location>
                   <tt:Description>Front door</tt:Description>
                   <tt:Address>rtsp://192.168.1.50/stream</tt:Address>
                 </tt:Source>
                 <tt:EarliestRecording>2024-01-01T00:00:00Z</tt:EarliestRecording>
                 <tt:LatestRecording>2024-01-02T00:00:00Z</tt:LatestRecording>
                 <tt:Content>Normal</tt:Content>
                 <tt:RecordingStatus>Recording</tt:RecordingStatus>
               </tt:RecordingInformation>
               <tt:Tracks>
                 <tt:Track token="Track_V1">
                   <tt:TrackType>Video</tt:TrackType>
                   <tt:Description>Main video</tt:Description>
                   <tt:DataFrom>2024-01-01T00:00:00Z</tt:DataFrom>
                   <tt:DataTo>2024-01-02T00:00:00Z</tt:DataTo>
                 </tt:Track>
               </tt:Tracks>
             </trc:RecordingItems>
           </trc:GetRecordingsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let recs = client
        .get_recordings("http://192.168.1.1/onvif/recording_service")
        .await
        .unwrap();
    assert_eq!(
        recs[0].source.address.as_deref(),
        Some("rtsp://192.168.1.50/stream")
    );
    let track = &recs[0].tracks[0];
    assert_eq!(track.data_from.as_deref(), Some("2024-01-01T00:00:00Z"));
    assert_eq!(track.data_to.as_deref(), Some("2024-01-02T00:00:00Z"));
}

#[tokio::test]
async fn test_get_osd_parses_colors_and_persistence() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trt:GetOSDResponse>
             <trt:OSDConfiguration token="OSD_1">
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
             </trt:OSDConfiguration>
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

#[tokio::test]
async fn test_get_storage_configurations_parses_storage_status() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetStorageConfigurationsResponse>
             <tds:StorageConfigurations token="SD_1">
               <tt:StorageType>LocalStorage</tt:StorageType>
               <tt:LocalPath>/mnt/sd</tt:LocalPath>
               <tt:StorageStatus>Connected</tt:StorageStatus>
             </tds:StorageConfigurations>
           </tds:GetStorageConfigurationsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let cfgs = client.get_storage_configurations().await.unwrap();
    assert_eq!(cfgs[0].storage_status.as_deref(), Some("Connected"));
}

#[tokio::test]
async fn test_get_storage_configurations_no_status_is_none() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetStorageConfigurationsResponse>
             <tds:StorageConfigurations token="SD_1">
               <tt:StorageType>LocalStorage</tt:StorageType>
               <tt:LocalPath>/mnt/sd</tt:LocalPath>
             </tds:StorageConfigurations>
           </tds:GetStorageConfigurationsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let cfgs = client.get_storage_configurations().await.unwrap();
    assert!(cfgs[0].storage_status.is_none());
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
                 <tt:AFModes>AUTO</tt:AFModes>
                 <tt:AFModes>MANUAL</tt:AFModes>
                 <tt:AutoFocusSpeed><tt:Min>0</tt:Min><tt:Max>1</tt:Max></tt:AutoFocusSpeed>
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

// ── Direction-1: Profile G recording write operations ─────────────────────────

#[tokio::test]
async fn test_create_recording_returns_token() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:CreateRecordingResponse>
             <trc:RecordingToken>Rec_007</trc:RecordingToken>
           </trc:CreateRecordingResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let token = client
        .create_recording(
            "http://192.168.1.1/onvif/recording_service",
            "Camera A",
            "urn:uuid:cam-a",
            "Entrance",
            "Front door cam",
            "Normal",
        )
        .await
        .unwrap();
    assert_eq!(token, "Rec_007");
}

#[tokio::test]
async fn test_create_recording_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:CreateRecordingResponse/>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let res = client
        .create_recording(
            "http://192.168.1.1/onvif/recording_service",
            "Camera A",
            "urn:uuid:cam-a",
            "",
            "",
            "",
        )
        .await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_create_track_returns_token() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:CreateTrackResponse>
             <trc:TrackToken>Track_V2</trc:TrackToken>
           </trc:CreateTrackResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let token = client
        .create_track(
            "http://192.168.1.1/onvif/recording_service",
            "Rec_001",
            "Video",
            "Main video track",
        )
        .await
        .unwrap();
    assert_eq!(token, "Track_V2");
}

#[tokio::test]
async fn test_create_track_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:CreateTrackResponse/>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let res = client
        .create_track(
            "http://192.168.1.1/onvif/recording_service",
            "Rec_001",
            "Video",
            "",
        )
        .await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_get_recording_jobs_parses_fields() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trc:GetRecordingJobsResponse>
             <trc:JobItem>
               <trc:JobToken>Job_001</trc:JobToken>
               <trc:JobConfiguration>
                 <tt:RecordingToken>Rec_001</tt:RecordingToken>
                 <tt:Mode>Active</tt:Mode>
                 <tt:Priority>2</tt:Priority>
                 <tt:Source>
                   <tt:SourceToken>
                     <tt:Token>Profile_1</tt:Token>
                   </tt:SourceToken>
                 </tt:Source>
               </trc:JobConfiguration>
             </trc:JobItem>
           </trc:GetRecordingJobsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let jobs = client
        .get_recording_jobs("http://192.168.1.1/onvif/recording_service")
        .await
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].token, "Job_001");
    assert_eq!(jobs[0].recording_token, "Rec_001");
    assert_eq!(jobs[0].mode, "Active");
    assert_eq!(jobs[0].priority, 2);
    assert_eq!(jobs[0].source_token, "Profile_1");
}

#[tokio::test]
async fn test_get_recording_jobs_missing_job_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:GetRecordingJobsResponse>
             <trc:JobItem/>
           </trc:GetRecordingJobsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let res = client
        .get_recording_jobs("http://192.168.1.1/onvif/recording_service")
        .await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_create_recording_job_returns_token() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:CreateRecordingJobResponse>
             <trc:JobToken>Job_new</trc:JobToken>
           </trc:CreateRecordingJobResponse>
         </s:Body>
       </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    let config = RecordingJobConfiguration {
        recording_token: "Rec_001".into(),
        mode: "Active".into(),
        priority: 1,
        source_token: "Profile_1".into(),
    };
    let token = client
        .create_recording_job("http://192.168.1.1/onvif/recording_service", &config)
        .await
        .unwrap();
    assert_eq!(token, "Job_new");
    let c = captured.lock().unwrap();
    assert!(c.body.contains("Rec_001"));
    assert!(c.body.contains("Active"));
    assert!(c.body.contains("Profile_1"));
}

#[tokio::test]
async fn test_set_recording_job_mode_sends_correct_body() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:SetRecordingJobModeResponse/>
         </s:Body>
       </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    client
        .set_recording_job_mode(
            "http://192.168.1.1/onvif/recording_service",
            "Job_001",
            "Idle",
        )
        .await
        .unwrap();
    let c = captured.lock().unwrap();
    assert!(c.body.contains("Job_001"));
    assert!(c.body.contains("Idle"));
}

#[tokio::test]
async fn test_get_recording_job_state_parses_active_state() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trc:GetRecordingJobStateResponse>
             <trc:JobToken>Job_001</trc:JobToken>
             <trc:State>
               <tt:ActiveState>Active</tt:ActiveState>
             </trc:State>
           </trc:GetRecordingJobStateResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let state = client
        .get_recording_job_state("http://192.168.1.1/onvif/recording_service", "Job_001")
        .await
        .unwrap();
    assert_eq!(state.token, "Job_001");
    assert_eq!(state.active_state, "Active");
}

#[tokio::test]
async fn test_get_recording_job_state_missing_state_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:GetRecordingJobStateResponse>
             <trc:JobToken>Job_001</trc:JobToken>
           </trc:GetRecordingJobStateResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let res = client
        .get_recording_job_state("http://192.168.1.1/onvif/recording_service", "Job_001")
        .await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_create_recording_job_xml_escapes_token() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:CreateRecordingJobResponse>
             <trc:JobToken>Job_safe</trc:JobToken>
           </trc:CreateRecordingJobResponse>
         </s:Body>
       </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    let config = RecordingJobConfiguration {
        recording_token: "Rec<&>".into(),
        mode: "Active".into(),
        priority: 1,
        source_token: "Profile_1".into(),
    };
    client
        .create_recording_job("http://192.168.1.1/onvif/recording_service", &config)
        .await
        .unwrap();
    let c = captured.lock().unwrap();
    assert!(c.body.contains("Rec&lt;&amp;&gt;"));
}

// ── Direction-3: event_stream ─────────────────────────────────────────────────

#[tokio::test]
async fn test_event_stream_yields_notification_messages() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(pull_messages_xml()));
    let mut stream = client.event_stream("http://192.168.1.1/onvif/subscription_1", "PT5S", 10);
    let msg = stream.next().await.expect("stream should yield").unwrap();
    assert!(msg.topic.contains("MotionAlarm"));
}

#[tokio::test]
async fn test_event_stream_error_on_bad_response() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
         <s:Body>
           <s:Fault>
             <s:Code><s:Value>s:Receiver</s:Value></s:Code>
             <s:Reason><s:Text>Subscription expired</s:Text></s:Reason>
           </s:Fault>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let mut stream = client.event_stream("http://192.168.1.1/onvif/subscription_1", "PT5S", 10);
    let result = stream.next().await.expect("stream should yield an error");
    assert!(result.is_err());
}
