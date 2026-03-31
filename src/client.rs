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
    Capabilities, DeviceInfo, MediaProfile, MediaProfile2, OnvifService, PtzPreset, SnapshotUri,
    StreamUri, SystemDateTime, VideoEncoderConfiguration, VideoEncoderConfiguration2,
    VideoEncoderConfigurationOptions, VideoEncoderConfigurationOptions2, VideoEncoderInstances,
    VideoSource, VideoSourceConfiguration, VideoSourceConfigurationOptions,
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

    /// Retrieve all service endpoints advertised by the device.
    ///
    /// `GetServices` is the correct ONVIF mechanism for discovering every
    /// service URL, including Media2. Many devices do not include the Media2
    /// URL in `GetCapabilities` — call this as a fallback:
    ///
    /// ```no_run
    /// # use oxvif::{OnvifClient, OnvifError};
    /// # async fn run() -> Result<(), OnvifError> {
    /// let client = OnvifClient::new("http://192.168.1.1/onvif/device_service");
    /// let caps   = client.get_capabilities().await?;
    /// let media2_url = match caps.media2_url {
    ///     Some(u) => u,
    ///     None => client.get_services().await?
    ///         .into_iter()
    ///         .find(|s| s.is_media2())
    ///         .map(|s| s.url)
    ///         .expect("device does not support Media2"),
    /// };
    /// # Ok(()) }
    /// ```
    pub async fn get_services(&self) -> Result<Vec<OnvifService>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetServices";
        const BODY: &str = "<tds:GetServices><tds:IncludeCapability>false</tds:IncludeCapability></tds:GetServices>";

        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body = parse_soap_body(&xml)?;
        let resp = find_response(&body, "GetServicesResponse")?;
        OnvifService::vec_from_xml(resp)
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

    // ── Video Source Service ──────────────────────────────────────────────────

    /// List all physical video sources available on the device.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    pub async fn get_video_sources(&self, media_url: &str) -> Result<Vec<VideoSource>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetVideoSources";
        const BODY: &str = "<trt:GetVideoSources/>";

        let xml = self.call(media_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoSourcesResponse")?;
        VideoSource::vec_from_xml(resp)
    }

    /// List all video source configurations on the device.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    pub async fn get_video_source_configurations(
        &self,
        media_url: &str,
    ) -> Result<Vec<VideoSourceConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfigurations";
        const BODY: &str = "<trt:GetVideoSourceConfigurations/>";

        let xml = self.call(media_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoSourceConfigurationsResponse")?;
        VideoSourceConfiguration::vec_from_xml(resp)
    }

    /// Retrieve a single video source configuration by token.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    pub async fn get_video_source_configuration(
        &self,
        media_url: &str,
        token: &str,
    ) -> Result<VideoSourceConfiguration, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfiguration";
        let body = format!(
            "<trt:GetVideoSourceConfiguration>\
               <trt:ConfigurationToken>{token}</trt:ConfigurationToken>\
             </trt:GetVideoSourceConfiguration>"
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoSourceConfigurationResponse")?;
        let node = resp
            .child("Configuration")
            .ok_or_else(|| crate::soap::SoapError::missing("Configuration"))?;
        VideoSourceConfiguration::from_xml(node)
    }

    /// Apply a modified video source configuration to the device.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    pub async fn set_video_source_configuration(
        &self,
        media_url: &str,
        config: &VideoSourceConfiguration,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/SetVideoSourceConfiguration";
        let body = format!(
            "<trt:SetVideoSourceConfiguration>\
               {cfg}\
               <trt:ForcePersistence>true</trt:ForcePersistence>\
             </trt:SetVideoSourceConfiguration>",
            cfg = config.to_xml_body()
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetVideoSourceConfigurationResponse")?;
        Ok(())
    }

    /// Retrieve the valid parameter ranges for video source configuration.
    ///
    /// Pass `config_token` to narrow the options to a specific configuration,
    /// or `None` to retrieve options valid for all configurations.
    pub async fn get_video_source_configuration_options(
        &self,
        media_url: &str,
        config_token: Option<&str>,
    ) -> Result<VideoSourceConfigurationOptions, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfigurationOptions";
        let inner = match config_token {
            Some(tok) => format!("<trt:ConfigurationToken>{tok}</trt:ConfigurationToken>"),
            None => String::new(),
        };
        let body = format!(
            "<trt:GetVideoSourceConfigurationOptions>{inner}</trt:GetVideoSourceConfigurationOptions>"
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoSourceConfigurationOptionsResponse")?;
        VideoSourceConfigurationOptions::from_xml(resp)
    }

    // ── Video Encoder Service ─────────────────────────────────────────────────

    /// List all video encoder configurations on the device.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    pub async fn get_video_encoder_configurations(
        &self,
        media_url: &str,
    ) -> Result<Vec<VideoEncoderConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfigurations";
        const BODY: &str = "<trt:GetVideoEncoderConfigurations/>";

        let xml = self.call(media_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoEncoderConfigurationsResponse")?;
        VideoEncoderConfiguration::vec_from_xml(resp)
    }

    /// Retrieve a single video encoder configuration by token.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    pub async fn get_video_encoder_configuration(
        &self,
        media_url: &str,
        token: &str,
    ) -> Result<VideoEncoderConfiguration, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfiguration";
        let body = format!(
            "<trt:GetVideoEncoderConfiguration>\
               <trt:ConfigurationToken>{token}</trt:ConfigurationToken>\
             </trt:GetVideoEncoderConfiguration>"
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoEncoderConfigurationResponse")?;
        let node = resp
            .child("Configuration")
            .ok_or_else(|| crate::soap::SoapError::missing("Configuration"))?;
        VideoEncoderConfiguration::from_xml(node)
    }

    /// Apply a modified video encoder configuration to the device.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    pub async fn set_video_encoder_configuration(
        &self,
        media_url: &str,
        config: &VideoEncoderConfiguration,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/SetVideoEncoderConfiguration";
        let body = format!(
            "<trt:SetVideoEncoderConfiguration>\
               {cfg}\
               <trt:ForcePersistence>true</trt:ForcePersistence>\
             </trt:SetVideoEncoderConfiguration>",
            cfg = config.to_xml_body()
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetVideoEncoderConfigurationResponse")?;
        Ok(())
    }

    /// Retrieve the valid parameter ranges for video encoder configuration.
    ///
    /// Pass `config_token` to narrow the options to a specific configuration,
    /// or `None` to retrieve options valid for all configurations.
    pub async fn get_video_encoder_configuration_options(
        &self,
        media_url: &str,
        config_token: Option<&str>,
    ) -> Result<VideoEncoderConfigurationOptions, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfigurationOptions";
        let inner = match config_token {
            Some(tok) => format!("<trt:ConfigurationToken>{tok}</trt:ConfigurationToken>"),
            None => String::new(),
        };
        let body = format!(
            "<trt:GetVideoEncoderConfigurationOptions>{inner}</trt:GetVideoEncoderConfigurationOptions>"
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoEncoderConfigurationOptionsResponse")?;
        VideoEncoderConfigurationOptions::from_xml(resp)
    }

    // ── Media2 Service ────────────────────────────────────────────────────────

    /// List all media profiles from the Media2 service.
    ///
    /// `media2_url` is obtained from `caps.media2_url` via
    /// [`get_capabilities`](Self::get_capabilities).
    pub async fn get_profiles_media2(
        &self,
        media2_url: &str,
    ) -> Result<Vec<MediaProfile2>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetProfiles";
        const BODY: &str = "<tr2:GetProfiles/>";

        let xml = self.call(media2_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetProfilesResponse")?;
        MediaProfile2::vec_from_xml(resp)
    }

    /// Retrieve an RTSP stream URI via the Media2 service.
    ///
    /// Returns the URI string directly (no `MediaUri` wrapper, unlike Media1).
    pub async fn get_stream_uri_media2(
        &self,
        media2_url: &str,
        profile_token: &str,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetStreamUri";
        let body = format!(
            "<tr2:GetStreamUri>\
               <tr2:Protocol>RTSP</tr2:Protocol>\
               <tr2:ProfileToken>{profile_token}</tr2:ProfileToken>\
             </tr2:GetStreamUri>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetStreamUriResponse")?;
        resp.child("Uri")
            .map(|n| n.text().to_string())
            .ok_or_else(|| crate::soap::SoapError::missing("Uri").into())
    }

    /// Retrieve an HTTP snapshot URI via the Media2 service.
    ///
    /// Returns the URI string directly (no `MediaUri` wrapper, unlike Media1).
    pub async fn get_snapshot_uri_media2(
        &self,
        media2_url: &str,
        profile_token: &str,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetSnapshotUri";
        let body = format!(
            "<tr2:GetSnapshotUri>\
               <tr2:ProfileToken>{profile_token}</tr2:ProfileToken>\
             </tr2:GetSnapshotUri>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetSnapshotUriResponse")?;
        resp.child("Uri")
            .map(|n| n.text().to_string())
            .ok_or_else(|| crate::soap::SoapError::missing("Uri").into())
    }

    /// List all video source configurations via the Media2 service.
    pub async fn get_video_source_configurations_media2(
        &self,
        media2_url: &str,
    ) -> Result<Vec<VideoSourceConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetVideoSourceConfigurations";
        const BODY: &str = "<tr2:GetVideoSourceConfigurations/>";

        let xml = self.call(media2_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoSourceConfigurationsResponse")?;
        VideoSourceConfiguration::vec_from_xml(resp)
    }

    /// Apply a modified video source configuration via the Media2 service.
    ///
    /// Note: Media2 does not use `ForcePersistence`.
    pub async fn set_video_source_configuration_media2(
        &self,
        media2_url: &str,
        config: &VideoSourceConfiguration,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/SetVideoSourceConfiguration";
        let body = format!(
            "<tr2:SetVideoSourceConfiguration>{cfg}</tr2:SetVideoSourceConfiguration>",
            cfg = config.to_xml_body_media2()
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetVideoSourceConfigurationResponse")?;
        Ok(())
    }

    /// Retrieve the valid parameter ranges for video source configuration via Media2.
    pub async fn get_video_source_configuration_options_media2(
        &self,
        media2_url: &str,
        config_token: Option<&str>,
    ) -> Result<VideoSourceConfigurationOptions, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver20/media/wsdl/GetVideoSourceConfigurationOptions";
        let inner = match config_token {
            Some(tok) => format!("<tr2:ConfigurationToken>{tok}</tr2:ConfigurationToken>"),
            None => String::new(),
        };
        let body = format!(
            "<tr2:GetVideoSourceConfigurationOptions>{inner}</tr2:GetVideoSourceConfigurationOptions>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoSourceConfigurationOptionsResponse")?;
        VideoSourceConfigurationOptions::from_xml(resp)
    }

    /// List all video encoder configurations from the Media2 service.
    ///
    /// Returns [`VideoEncoderConfiguration2`] which uses a flat layout with
    /// native H.265 support.
    pub async fn get_video_encoder_configurations_media2(
        &self,
        media2_url: &str,
    ) -> Result<Vec<VideoEncoderConfiguration2>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurations";
        const BODY: &str = "<tr2:GetVideoEncoderConfigurations/>";

        let xml = self.call(media2_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoEncoderConfigurationsResponse")?;
        VideoEncoderConfiguration2::vec_from_xml(resp)
    }

    /// Retrieve a single video encoder configuration by token from the Media2 service.
    pub async fn get_video_encoder_configuration_media2(
        &self,
        media2_url: &str,
        token: &str,
    ) -> Result<VideoEncoderConfiguration2, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurations";
        let body = format!(
            "<tr2:GetVideoEncoderConfigurations>\
               <tr2:ConfigurationToken>{token}</tr2:ConfigurationToken>\
             </tr2:GetVideoEncoderConfigurations>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoEncoderConfigurationsResponse")?;
        let configs = VideoEncoderConfiguration2::vec_from_xml(resp)?;
        configs
            .into_iter()
            .next()
            .ok_or_else(|| crate::soap::SoapError::missing("Configurations").into())
    }

    /// Apply a modified video encoder configuration via the Media2 service.
    ///
    /// Note: Media2 does not use `ForcePersistence`.
    pub async fn set_video_encoder_configuration_media2(
        &self,
        media2_url: &str,
        config: &VideoEncoderConfiguration2,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/SetVideoEncoderConfiguration";
        let body = format!(
            "<tr2:SetVideoEncoderConfiguration>{cfg}</tr2:SetVideoEncoderConfiguration>",
            cfg = config.to_xml_body()
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetVideoEncoderConfigurationResponse")?;
        Ok(())
    }

    /// Retrieve the valid parameter ranges for video encoder configuration via Media2.
    ///
    /// Media2 returns one options entry per supported encoding type.
    pub async fn get_video_encoder_configuration_options_media2(
        &self,
        media2_url: &str,
        config_token: Option<&str>,
    ) -> Result<VideoEncoderConfigurationOptions2, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurationOptions";
        let inner = match config_token {
            Some(tok) => format!("<tr2:ConfigurationToken>{tok}</tr2:ConfigurationToken>"),
            None => String::new(),
        };
        let body = format!(
            "<tr2:GetVideoEncoderConfigurationOptions>{inner}</tr2:GetVideoEncoderConfigurationOptions>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoEncoderConfigurationOptionsResponse")?;
        VideoEncoderConfigurationOptions2::from_xml(resp)
    }

    /// Retrieve encoder instance capacity info for a video source configuration (Media2).
    pub async fn get_video_encoder_instances_media2(
        &self,
        media2_url: &str,
        config_token: &str,
    ) -> Result<VideoEncoderInstances, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderInstances";
        let body = format!(
            "<tr2:GetVideoEncoderInstances>\
               <tr2:ConfigurationToken>{config_token}</tr2:ConfigurationToken>\
             </tr2:GetVideoEncoderInstances>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoEncoderInstancesResponse")?;
        VideoEncoderInstances::from_xml(resp)
    }

    /// Create a new media profile via the Media2 service.
    ///
    /// Returns the token of the newly created profile.
    pub async fn create_profile_media2(
        &self,
        media2_url: &str,
        name: &str,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/CreateProfile";
        let body = format!(
            "<tr2:CreateProfile>\
               <tr2:Name>{name}</tr2:Name>\
             </tr2:CreateProfile>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "CreateProfileResponse")?;
        resp.child("Token")
            .map(|n| n.text().to_string())
            .ok_or_else(|| crate::soap::SoapError::missing("Token").into())
    }

    /// Delete a media profile via the Media2 service.
    pub async fn delete_profile_media2(
        &self,
        media2_url: &str,
        token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/DeleteProfile";
        let body = format!(
            "<tr2:DeleteProfile>\
               <tr2:Token>{token}</tr2:Token>\
             </tr2:DeleteProfile>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "DeleteProfileResponse")?;
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "tests/client_tests.rs"]
mod tests;
