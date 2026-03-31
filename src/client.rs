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
}
