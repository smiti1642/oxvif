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
