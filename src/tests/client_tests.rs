use super::*;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use crate::transport::TransportError;

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
async fn test_get_capabilities_sends_correct_action_and_url() {
    let (transport, captured) = RecordingTransport::new(capabilities_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.get_capabilities().await.unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.url, "http://192.168.1.1/onvif/device_service");
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/GetCapabilities"
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

#[tokio::test]
async fn test_get_device_info_uses_device_url() {
    let (transport, captured) = RecordingTransport::new(device_info_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.get_device_info().await.unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.url, "http://192.168.1.1/onvif/device_service");
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation"
    );
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

#[tokio::test]
async fn test_get_profiles_uses_media_url() {
    let (transport, captured) = RecordingTransport::new(profiles_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    let media_url = "http://192.168.1.1/onvif/media_service";
    client.get_profiles(media_url).await.unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.url, media_url);
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/media/wsdl/GetProfiles"
    );
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

#[tokio::test]
async fn test_get_stream_uri_uses_media_url_and_correct_action() {
    let (transport, captured) = RecordingTransport::new(stream_uri_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    let media_url = "http://192.168.1.1/onvif/media_service";
    client.get_stream_uri(media_url, "tok").await.unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.url, media_url);
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/media/wsdl/GetStreamUri"
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

#[tokio::test]
async fn test_ptz_set_preset_uses_correct_action() {
    let (transport, captured) = RecordingTransport::new(ptz_set_preset_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .ptz_set_preset("http://192.168.1.1/onvif/ptz_service", "P", None, None)
        .await
        .unwrap();

    assert_eq!(
        captured.lock().unwrap().action,
        "http://www.onvif.org/ver20/ptz/wsdl/SetPreset"
    );
}

// ── ptz_remove_preset ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_ptz_remove_preset_ok() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(ptz_remove_preset_xml()));

    client
        .ptz_remove_preset(
            "http://192.168.1.1/onvif/ptz_service",
            "Profile_1",
            "Preset_3",
        )
        .await
        .unwrap();
}

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

#[tokio::test]
async fn test_create_profile_uses_correct_action() {
    let (transport, captured) = RecordingTransport::new(create_profile_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .create_profile("http://192.168.1.1/onvif/media_service", "P", None)
        .await
        .unwrap();

    assert_eq!(
        captured.lock().unwrap().action,
        "http://www.onvif.org/ver10/media/wsdl/CreateProfile"
    );
}

// ── delete_profile ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_delete_profile_ok() {
    let xml = empty_response_xml("DeleteProfileResponse");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));

    client
        .delete_profile("http://192.168.1.1/onvif/media_service", "Profile_1")
        .await
        .unwrap();
}

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

fn unsubscribe_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2">
          <s:Body>
            <wsnt:UnsubscribeResponse/>
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

#[tokio::test]
async fn test_get_event_properties_uses_correct_action() {
    let (transport, captured) = RecordingTransport::new(event_properties_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .get_event_properties("http://192.168.1.1/onvif/events_service")
        .await
        .unwrap();

    assert_eq!(
        captured.lock().unwrap().action,
        "http://www.onvif.org/ver10/events/wsdl/EventPortType/GetEventPropertiesRequest"
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

#[tokio::test]
async fn test_create_pull_point_subscription_uses_correct_action() {
    let (transport, captured) = RecordingTransport::new(create_pull_point_subscription_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .create_pull_point_subscription("http://192.168.1.1/onvif/events_service", None, None)
        .await
        .unwrap();

    assert_eq!(
        captured.lock().unwrap().action,
        "http://www.onvif.org/ver10/events/wsdl/EventPortType/CreatePullPointSubscriptionRequest"
    );
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

#[tokio::test]
async fn test_pull_messages_posts_to_subscription_url() {
    let (transport, captured) = RecordingTransport::new(pull_messages_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .pull_messages(
            "http://192.168.1.1/onvif/events/subscription_1",
            "PT5S",
            100,
        )
        .await
        .unwrap();

    assert_eq!(
        captured.lock().unwrap().url,
        "http://192.168.1.1/onvif/events/subscription_1"
    );
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

#[tokio::test]
async fn test_unsubscribe_ok() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(unsubscribe_xml()));

    client
        .unsubscribe("http://192.168.1.1/onvif/events/subscription_1")
        .await
        .unwrap();
}

#[tokio::test]
async fn test_unsubscribe_posts_to_subscription_url_with_correct_action() {
    let (transport, captured) = RecordingTransport::new(unsubscribe_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .unsubscribe("http://192.168.1.1/onvif/events/subscription_1")
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.url, "http://192.168.1.1/onvif/events/subscription_1");
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/events/wsdl/SubscriptionManager/UnsubscribeRequest"
    );
}

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

#[tokio::test]
async fn test_get_device_info_soap_fault_returns_err() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(
        &make_soap_fault_xml("s:Sender", "Action not supported"),
    ));
    let result = client.get_device_info().await;
    assert!(result.is_err());
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
