//! High-level ONVIF client.
//!
//! [`OnvifClient`] is the primary entry point for the oxvif library. It is
//! intentionally **stateless**: the service URLs discovered via
//! `get_capabilities()` are returned to the caller rather than cached
//! internally. This design makes the client cheaply cloneable and safe to
//! share across threads behind an `Arc`.
//!
//! ## Authentication
//!
//! When credentials are supplied via [`with_credentials`], every request
//! includes a WS-Security `UsernameToken` with a freshly generated nonce.
//! If the device clock differs from the local clock, call [`with_utc_offset`]
//! after `GetSystemDateAndTime` to keep timestamps in sync.
//!
//! ## Testing
//!
//! Inject a custom [`Transport`](crate::transport::Transport) via
//! [`with_transport`] to unit-test without a real device.
//!
//! [`with_credentials`]: OnvifClient::with_credentials
//! [`with_utc_offset`]: OnvifClient::with_utc_offset
//! [`with_transport`]: OnvifClient::with_transport

use std::sync::Arc;

use crate::error::OnvifError;
use crate::soap::{SoapEnvelope, WsSecurityToken, find_response, parse_soap_body};
use crate::transport::{HttpTransport, Transport};
use crate::types::{
    Capabilities, DeviceInfo, MediaProfile, PtzPreset, SnapshotUri, StreamUri, SystemDateTime,
};

// ── OnvifClient ───────────────────────────────────────────────────────────────

/// Async ONVIF device client.
///
/// # Quick start
///
/// ```no_run
/// use oxvif::{OnvifClient, OnvifError};
///
/// async fn run() -> Result<(), OnvifError> {
///     let client = OnvifClient::new("http://192.168.1.100/onvif/device_service")
///         .with_credentials("admin", "password");
///
///     let caps     = client.get_capabilities().await?;
///     let media    = caps.media.url.as_deref().unwrap();
///     let profiles = client.get_profiles(media).await?;
///     let uri      = client.get_stream_uri(media, &profiles[0].token).await?;
///
///     println!("RTSP: {}", uri.uri);
///     Ok(())
/// }
/// ```
pub struct OnvifClient {
    device_url: String,
    credentials: Option<(String, String)>,
    /// Seconds to add to local UTC when generating WS-Security timestamps.
    /// Set via [`with_utc_offset`](Self::with_utc_offset) after calling
    /// `GetSystemDateAndTime` if the device clock differs from local UTC.
    utc_offset: i64,
    transport: Arc<dyn Transport>,
}

impl OnvifClient {
    /// Create a client targeting the ONVIF device service at `device_url`.
    ///
    /// `device_url` is the endpoint returned by WS-Discovery or entered
    /// manually (e.g. `http://192.168.1.100/onvif/device_service`).
    pub fn new(device_url: impl Into<String>) -> Self {
        Self {
            device_url: device_url.into(),
            credentials: None,
            utc_offset: 0,
            transport: Arc::new(HttpTransport::new()),
        }
    }

    /// Set the credentials used for WS-Security `UsernameToken` authentication.
    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.credentials = Some((username.into(), password.into()));
        self
    }

    /// Adjust the `<wsu:Created>` timestamp by `offset_secs` seconds.
    ///
    /// Obtain the offset by subtracting local UTC from the value returned by
    /// `GetSystemDateAndTime`. Ignored when no credentials are set.
    pub fn with_utc_offset(mut self, offset_secs: i64) -> Self {
        self.utc_offset = offset_secs;
        self
    }

    /// Replace the default [`HttpTransport`] with a custom implementation.
    ///
    /// Primarily used in tests to inject a mock transport without a live device.
    pub fn with_transport(mut self, transport: Arc<dyn Transport>) -> Self {
        self.transport = transport;
        self
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn security_token(&self) -> Option<WsSecurityToken> {
        self.credentials
            .as_ref()
            .map(|(user, pass)| WsSecurityToken::generate(user, pass, self.utc_offset))
    }

    /// Build a SOAP envelope, attach a WS-Security header if credentials are
    /// set, serialise to XML, and POST to `url`.
    async fn call(&self, url: &str, action: &str, body: &str) -> Result<String, OnvifError> {
        let mut envelope = SoapEnvelope::new(body.to_string());
        if let Some(token) = self.security_token() {
            envelope = envelope.with_security(token);
        }
        Ok(self
            .transport
            .soap_post(url, action, envelope.build())
            .await?)
    }

    // ── Device Service ────────────────────────────────────────────────────────

    /// Retrieve service endpoint URLs from the device.
    ///
    /// This is typically the first call made after constructing a client. The
    /// returned [`Capabilities`] provides the URLs needed for all subsequent
    /// media, PTZ, events, and imaging operations.
    pub async fn get_capabilities(&self) -> Result<Capabilities, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetCapabilities";
        const BODY: &str =
            "<tds:GetCapabilities><tds:Category>All</tds:Category></tds:GetCapabilities>";

        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body = parse_soap_body(&xml)?;
        let resp = find_response(&body, "GetCapabilitiesResponse")?;
        Capabilities::from_xml(resp)
    }

    /// Retrieve the device clock and compute the UTC offset for WS-Security.
    ///
    /// Call this before [`with_utc_offset`](Self::with_utc_offset) when the
    /// device clock may differ from local UTC:
    ///
    /// ```no_run
    /// # use oxvif::{OnvifClient, OnvifError};
    /// # async fn run() -> Result<(), OnvifError> {
    /// let client = OnvifClient::new("http://192.168.1.1/onvif/device_service");
    /// let dt     = client.get_system_date_and_time().await?;
    /// let client = client.with_credentials("admin", "pass")
    ///                    .with_utc_offset(dt.utc_offset_secs());
    /// # Ok(()) }
    /// ```
    pub async fn get_system_date_and_time(&self) -> Result<SystemDateTime, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetSystemDateAndTime";
        const BODY: &str = "<tds:GetSystemDateAndTime/>";

        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body = parse_soap_body(&xml)?;
        let resp = find_response(&body, "GetSystemDateAndTimeResponse")?;
        SystemDateTime::from_xml(resp)
    }

    /// Retrieve manufacturer, model, firmware version, and serial number.
    pub async fn get_device_info(&self) -> Result<DeviceInfo, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation";
        const BODY: &str = "<tds:GetDeviceInformation/>";

        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body = parse_soap_body(&xml)?;
        let resp = find_response(&body, "GetDeviceInformationResponse")?;
        DeviceInfo::from_xml(resp)
    }

    // ── Media Service ─────────────────────────────────────────────────────────

    /// List all media profiles available on the device.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    /// Each returned [`MediaProfile`] contains a `token` that can be passed to
    /// [`get_stream_uri`](Self::get_stream_uri).
    pub async fn get_profiles(&self, media_url: &str) -> Result<Vec<MediaProfile>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetProfiles";
        const BODY: &str = "<trt:GetProfiles/>";

        let xml = self.call(media_url, ACTION, BODY).await?;
        let body = parse_soap_body(&xml)?;
        let resp = find_response(&body, "GetProfilesResponse")?;
        MediaProfile::vec_from_xml(resp)
    }

    /// Retrieve an RTSP stream URI for the given media profile.
    ///
    /// `media_url` comes from [`get_capabilities`](Self::get_capabilities);
    /// `profile_token` comes from a [`MediaProfile`] returned by
    /// [`get_profiles`](Self::get_profiles).
    pub async fn get_stream_uri(
        &self,
        media_url: &str,
        profile_token: &str,
    ) -> Result<StreamUri, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetStreamUri";

        let body = format!(
            "<trt:GetStreamUri>\
               <trt:StreamSetup>\
                 <tt:Stream>RTP-Unicast</tt:Stream>\
                 <tt:Transport><tt:Protocol>RTSP</tt:Protocol></tt:Transport>\
               </trt:StreamSetup>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
             </trt:GetStreamUri>"
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetStreamUriResponse")?;
        StreamUri::from_xml(resp)
    }

    /// Retrieve an HTTP snapshot URI for the given media profile.
    ///
    /// `media_url` comes from [`get_capabilities`](Self::get_capabilities);
    /// `profile_token` comes from a [`MediaProfile`] returned by
    /// [`get_profiles`](Self::get_profiles).
    pub async fn get_snapshot_uri(
        &self,
        media_url: &str,
        profile_token: &str,
    ) -> Result<SnapshotUri, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetSnapshotUri";

        let body = format!(
            "<trt:GetSnapshotUri>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
             </trt:GetSnapshotUri>"
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetSnapshotUriResponse")?;
        SnapshotUri::from_xml(resp)
    }

    // ── PTZ Service ───────────────────────────────────────────────────────────

    /// Move the camera to an absolute position.
    ///
    /// Coordinates are in the normalised range `[-1.0, 1.0]` for pan/tilt
    /// and `[0.0, 1.0]` for zoom. `ptz_url` comes from
    /// [`get_capabilities`](Self::get_capabilities).
    pub async fn ptz_absolute_move(
        &self,
        ptz_url: &str,
        profile_token: &str,
        pan: f32,
        tilt: f32,
        zoom: f32,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/AbsoluteMove";
        let body = format!(
            "<tptz:AbsoluteMove>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
               <tptz:Position>\
                 <tt:PanTilt x=\"{pan}\" y=\"{tilt}\"/>\
                 <tt:Zoom x=\"{zoom}\"/>\
               </tptz:Position>\
             </tptz:AbsoluteMove>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "AbsoluteMoveResponse")?;
        Ok(())
    }

    /// Move the camera by a relative offset from the current position.
    ///
    /// Values are in the normalised range `[-1.0, 1.0]` for pan/tilt
    /// and `[-1.0, 1.0]` for zoom.
    pub async fn ptz_relative_move(
        &self,
        ptz_url: &str,
        profile_token: &str,
        pan: f32,
        tilt: f32,
        zoom: f32,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/RelativeMove";
        let body = format!(
            "<tptz:RelativeMove>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
               <tptz:Translation>\
                 <tt:PanTilt x=\"{pan}\" y=\"{tilt}\"/>\
                 <tt:Zoom x=\"{zoom}\"/>\
               </tptz:Translation>\
             </tptz:RelativeMove>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "RelativeMoveResponse")?;
        Ok(())
    }

    /// Start continuous pan/tilt/zoom movement at the given velocities.
    ///
    /// Values are in the normalised range `[-1.0, 1.0]`. Call
    /// [`ptz_stop`](Self::ptz_stop) to halt movement.
    pub async fn ptz_continuous_move(
        &self,
        ptz_url: &str,
        profile_token: &str,
        pan: f32,
        tilt: f32,
        zoom: f32,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/ContinuousMove";
        let body = format!(
            "<tptz:ContinuousMove>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
               <tptz:Velocity>\
                 <tt:PanTilt x=\"{pan}\" y=\"{tilt}\"/>\
                 <tt:Zoom x=\"{zoom}\"/>\
               </tptz:Velocity>\
             </tptz:ContinuousMove>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "ContinuousMoveResponse")?;
        Ok(())
    }

    /// Stop all ongoing PTZ movement.
    pub async fn ptz_stop(&self, ptz_url: &str, profile_token: &str) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/Stop";
        let body = format!(
            "<tptz:Stop>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
               <tptz:PanTilt>true</tptz:PanTilt>\
               <tptz:Zoom>true</tptz:Zoom>\
             </tptz:Stop>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "StopResponse")?;
        Ok(())
    }

    /// List all saved PTZ presets for the given profile.
    pub async fn ptz_get_presets(
        &self,
        ptz_url: &str,
        profile_token: &str,
    ) -> Result<Vec<PtzPreset>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetPresets";
        let body = format!(
            "<tptz:GetPresets>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
             </tptz:GetPresets>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetPresetsResponse")?;
        PtzPreset::vec_from_xml(resp)
    }

    /// Move the camera to a saved PTZ preset.
    ///
    /// `preset_token` comes from a [`PtzPreset`] returned by
    /// [`ptz_get_presets`](Self::ptz_get_presets).
    pub async fn ptz_goto_preset(
        &self,
        ptz_url: &str,
        profile_token: &str,
        preset_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GotoPreset";
        let body = format!(
            "<tptz:GotoPreset>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
               <tptz:PresetToken>{preset_token}</tptz:PresetToken>\
             </tptz:GotoPreset>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "GotoPresetResponse")?;
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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
}
