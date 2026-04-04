use super::*;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use crate::transport::TransportError;

// ── SequenceTransport: returns responses in order ─────────────────────────

struct SequenceTransport {
    responses: Mutex<Vec<String>>,
}

impl SequenceTransport {
    fn new(responses: &[&str]) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(responses.iter().map(|s| s.to_string()).rev().collect()),
        })
    }
}

#[async_trait]
impl Transport for SequenceTransport {
    async fn soap_post(
        &self,
        _url: &str,
        _action: &str,
        _body: String,
    ) -> Result<String, TransportError> {
        let mut stack = self.responses.lock().unwrap();
        stack.pop().map(Ok).unwrap_or_else(|| {
            Err(TransportError::HttpStatus {
                status: 503,
                body: "no more responses".into(),
            })
        })
    }
}

// ── ErrorTransport: always fails ─────────────────────────────────────────

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

// ── XML fixtures ──────────────────────────────────────────────────────────

/// Full capabilities response including all service URLs (Media, PTZ, Imaging,
/// Events, Recording, Search, Replay, Media2).
fn caps_full_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
      <s:Body>
        <tds:GetCapabilitiesResponse>
          <tds:Capabilities>
            <tt:Device>   <tt:XAddr>http://cam/onvif/device</tt:XAddr>    </tt:Device>
            <tt:Media>    <tt:XAddr>http://cam/onvif/media</tt:XAddr>     </tt:Media>
            <tt:PTZ>      <tt:XAddr>http://cam/onvif/ptz</tt:XAddr>       </tt:PTZ>
            <tt:Imaging>  <tt:XAddr>http://cam/onvif/imaging</tt:XAddr>   </tt:Imaging>
            <tt:Events>   <tt:XAddr>http://cam/onvif/events</tt:XAddr>    </tt:Events>
            <tt:Extension>
              <tt:Recording> <tt:XAddr>http://cam/onvif/recording</tt:XAddr> </tt:Recording>
              <tt:Search>    <tt:XAddr>http://cam/onvif/search</tt:XAddr>    </tt:Search>
              <tt:Replay>    <tt:XAddr>http://cam/onvif/replay</tt:XAddr>    </tt:Replay>
              <tt:Media2>    <tt:XAddr>http://cam/onvif/media2</tt:XAddr>    </tt:Media2>
            </tt:Extension>
          </tds:Capabilities>
        </tds:GetCapabilitiesResponse>
      </s:Body>
    </s:Envelope>"#
}

/// Minimal capabilities response — only a Device URL, no service URLs.
fn caps_device_only_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
      <s:Body>
        <tds:GetCapabilitiesResponse>
          <tds:Capabilities>
            <tt:Device><tt:XAddr>http://cam/onvif/device</tt:XAddr></tt:Device>
          </tds:Capabilities>
        </tds:GetCapabilitiesResponse>
      </s:Body>
    </s:Envelope>"#
}

fn system_date_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
      <s:Body>
        <tds:GetSystemDateAndTimeResponse>
          <tds:SystemDateAndTime>
            <tt:DateTimeType>NTP</tt:DateTimeType>
            <tt:DaylightSavings>false</tt:DaylightSavings>
            <tt:TimeZone><tt:TZ>UTC</tt:TZ></tt:TimeZone>
            <tt:UTCDateTime>
              <tt:Time><tt:Hour>12</tt:Hour><tt:Minute>0</tt:Minute><tt:Second>0</tt:Second></tt:Time>
              <tt:Date><tt:Year>2026</tt:Year><tt:Month>4</tt:Month><tt:Day>2</tt:Day></tt:Date>
            </tt:UTCDateTime>
          </tds:SystemDateAndTime>
        </tds:GetSystemDateAndTimeResponse>
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
            <tt:Uri>rtsp://cam:554/stream1</tt:Uri>
            <tt:InvalidAfterConnect>false</tt:InvalidAfterConnect>
            <tt:InvalidAfterReboot>false</tt:InvalidAfterReboot>
            <tt:Timeout>PT0S</tt:Timeout>
          </trt:MediaUri>
        </trt:GetStreamUriResponse>
      </s:Body>
    </s:Envelope>"#
}

fn ptz_stop_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl">
      <s:Body><tptz:StopResponse/></s:Body>
    </s:Envelope>"#
}

fn imaging_settings_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
      <s:Body>
        <timg:GetImagingSettingsResponse>
          <timg:ImagingSettings>
            <tt:Brightness>50</tt:Brightness>
            <tt:Contrast>50</tt:Contrast>
          </timg:ImagingSettings>
        </timg:GetImagingSettingsResponse>
      </s:Body>
    </s:Envelope>"#
}

fn recordings_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:trc="http://www.onvif.org/ver10/recording/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
      <s:Body>
        <trc:GetRecordingsResponse>
          <trc:RecordingItems Token="rec1">
            <trc:RecordingInformation>
              <tt:RecordingStatus>Recording</tt:RecordingStatus>
            </trc:RecordingInformation>
          </trc:RecordingItems>
        </trc:GetRecordingsResponse>
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

// ── Helper: build a session from a sequence of SOAP responses ─────────────
// The first response is always for GetCapabilities (caps_full_xml).

async fn session_with(method_responses: &[&str]) -> OnvifSession {
    let mut responses = vec![caps_full_xml()];
    responses.extend_from_slice(method_responses);
    OnvifSession::builder("http://cam/onvif/device")
        .with_transport(SequenceTransport::new(&responses))
        .build()
        .await
        .expect("session build failed")
}

// ── Builder tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_builder_stores_capabilities() {
    let session = session_with(&[]).await;
    let caps = session.capabilities();
    assert_eq!(caps.media.url.as_deref(), Some("http://cam/onvif/media"));
    assert_eq!(caps.ptz.url.as_deref(), Some("http://cam/onvif/ptz"));
    assert_eq!(
        caps.imaging.url.as_deref(),
        Some("http://cam/onvif/imaging")
    );
    assert_eq!(caps.events.url.as_deref(), Some("http://cam/onvif/events"));
    assert_eq!(
        caps.recording.url.as_deref(),
        Some("http://cam/onvif/recording")
    );
    assert_eq!(caps.search.url.as_deref(), Some("http://cam/onvif/search"));
    assert_eq!(caps.replay.url.as_deref(), Some("http://cam/onvif/replay"));
    assert_eq!(caps.media2.url.as_deref(), Some("http://cam/onvif/media2"));
}

#[tokio::test]
async fn test_builder_with_clock_sync_calls_date_time_first() {
    // clock sync → GetCapabilities: two responses required, in that order
    let transport = SequenceTransport::new(&[system_date_xml(), caps_full_xml()]);
    let session = OnvifSession::builder("http://cam/onvif/device")
        .with_clock_sync()
        .with_transport(transport)
        .build()
        .await
        .expect("session with clock sync failed");

    // Session must be functional after the two-call init sequence
    assert!(session.capabilities().media.url.is_some());
}

#[tokio::test]
async fn test_builder_without_clock_sync_uses_one_call() {
    // Without clock sync only GetCapabilities is called
    let transport = SequenceTransport::new(&[caps_full_xml()]);
    OnvifSession::builder("http://cam/onvif/device")
        .with_transport(transport)
        .build()
        .await
        .expect("build without clock sync failed");
}

#[tokio::test]
async fn test_builder_transport_error_propagates() {
    let result = OnvifSession::builder("http://cam/onvif/device")
        .with_transport(Arc::new(ErrorTransport { status: 503 }))
        .build()
        .await;

    assert!(matches!(result, Err(OnvifError::Transport(_))));
}

#[tokio::test]
async fn test_builder_soap_fault_propagates() {
    let transport = SequenceTransport::new(&[soap_fault_xml()]);
    let result = OnvifSession::builder("http://cam/onvif/device")
        .with_transport(transport)
        .build()
        .await;

    assert!(matches!(
        result,
        Err(OnvifError::Soap(crate::soap::SoapError::Fault { .. }))
    ));
}

// ── client() / capabilities() accessors ──────────────────────────────────

#[tokio::test]
async fn test_client_accessor_returns_underlying_client() {
    let session = session_with(&[]).await;
    // Verify the client is callable (it will return transport error since no
    // more responses, but the accessor itself must work)
    let _client_ref = session.client();
}

#[tokio::test]
async fn test_capabilities_accessor_is_the_cached_value() {
    let session = session_with(&[]).await;
    let caps = session.capabilities();
    // Smoke check that the cached caps are the parsed full caps fixture
    assert_eq!(caps.device.url.as_deref(), Some("http://cam/onvif/device"));
}

// ── Missing URL errors ────────────────────────────────────────────────────
// Each test builds a session with only a Device URL so the relevant
// service URL resolver returns Err rather than Ok.

async fn session_device_only() -> OnvifSession {
    OnvifSession::builder("http://cam/onvif/device")
        .with_transport(SequenceTransport::new(&[caps_device_only_xml()]))
        .build()
        .await
        .expect("device-only session build failed")
}

#[tokio::test]
async fn test_missing_media_url_returns_error() {
    let session = session_device_only().await;
    let err = session.get_profiles().await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

#[tokio::test]
async fn test_missing_ptz_url_returns_error() {
    let session = session_device_only().await;
    let err = session.ptz_stop("tok").await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

#[tokio::test]
async fn test_missing_imaging_url_returns_error() {
    let session = session_device_only().await;
    let err = session.get_imaging_settings("src_tok").await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

#[tokio::test]
async fn test_missing_events_url_returns_error() {
    let session = session_device_only().await;
    let err = session.get_event_properties().await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

#[tokio::test]
async fn test_missing_recording_url_returns_error() {
    let session = session_device_only().await;
    let err = session.get_recordings().await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

#[tokio::test]
async fn test_missing_search_url_returns_error() {
    let session = session_device_only().await;
    let err = session.find_recordings(None, "PT60S").await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

#[tokio::test]
async fn test_missing_replay_url_returns_error() {
    let session = session_device_only().await;
    let err = session
        .get_replay_uri("tok", "RTP-Unicast", "RTSP")
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

#[tokio::test]
async fn test_missing_media2_url_returns_error() {
    let session = session_device_only().await;
    let err = session.get_profiles_media2().await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

// ── Delegate method tests ─────────────────────────────────────────────────

#[tokio::test]
async fn test_get_profiles_delegates_and_returns_results() {
    let session = session_with(&[profiles_xml()]).await;
    let profiles = session.get_profiles().await.unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].token, "Profile_1");
    assert_eq!(profiles[0].name, "mainStream");
}

#[tokio::test]
async fn test_get_stream_uri_delegates_and_returns_uri() {
    let session = session_with(&[stream_uri_xml()]).await;
    let uri = session.get_stream_uri("Profile_1").await.unwrap();
    assert_eq!(uri.uri, "rtsp://cam:554/stream1");
}

#[tokio::test]
async fn test_ptz_stop_delegates_ok() {
    let session = session_with(&[ptz_stop_xml()]).await;
    session.ptz_stop("Profile_1").await.unwrap();
}

#[tokio::test]
async fn test_get_imaging_settings_delegates_ok() {
    let session = session_with(&[imaging_settings_xml()]).await;
    let settings = session.get_imaging_settings("VideoSource_1").await.unwrap();
    assert_eq!(settings.brightness, Some(50.0));
}

#[tokio::test]
async fn test_get_recordings_delegates_and_returns_items() {
    let session = session_with(&[recordings_xml()]).await;
    let recs = session.get_recordings().await.unwrap();
    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0].token, "rec1");
    assert_eq!(recs[0].recording_status, "Recording");
}

#[tokio::test]
async fn test_delegate_soap_fault_propagates() {
    let session = session_with(&[soap_fault_xml()]).await;
    let err = session.get_profiles().await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::Fault { .. })
    ));
}

#[tokio::test]
async fn test_get_device_info_uses_device_url() {
    // Device methods bypass service URL caching and go directly to device_url
    let device_info_xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
      <s:Body>
        <tds:GetDeviceInformationResponse>
          <tds:Manufacturer>TestCorp</tds:Manufacturer>
          <tds:Model>Cam-X</tds:Model>
          <tds:FirmwareVersion>1.0</tds:FirmwareVersion>
          <tds:SerialNumber>SN001</tds:SerialNumber>
          <tds:HardwareId>0x01</tds:HardwareId>
        </tds:GetDeviceInformationResponse>
      </s:Body>
    </s:Envelope>"#;

    let session = session_with(&[device_info_xml]).await;
    let info = session.get_device_info().await.unwrap();
    assert_eq!(info.manufacturer, "TestCorp");
    assert_eq!(info.model, "Cam-X");
}
