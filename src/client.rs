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
    Capabilities, DeviceInfo, EventProperties, Hostname, ImagingOptions, ImagingSettings,
    MediaProfile, MediaProfile2, NotificationMessage, NtpInfo, OnvifService, PtzPreset, PtzStatus,
    PullPointSubscription, SnapshotUri, StreamUri, SystemDateTime, VideoEncoderConfiguration,
    VideoEncoderConfiguration2, VideoEncoderConfigurationOptions,
    VideoEncoderConfigurationOptions2, VideoEncoderInstances, VideoSource,
    VideoSourceConfiguration, VideoSourceConfigurationOptions,
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
#[derive(Clone)]
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

    /// Return the device service URL this client was constructed with.
    pub fn device_url(&self) -> &str {
        &self.device_url
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

    /// Retrieve the device hostname and whether it is assigned by DHCP.
    pub async fn get_hostname(&self) -> Result<Hostname, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetHostname";
        const BODY: &str = "<tds:GetHostname/>";
        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetHostnameResponse")?;
        Hostname::from_xml(resp)
    }

    /// Set the device hostname.
    ///
    /// Most devices require a reboot for the change to take effect.
    pub async fn set_hostname(&self, name: &str) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SetHostname";
        let body = format!("<tds:SetHostname><tds:Name>{name}</tds:Name></tds:SetHostname>");
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetHostnameResponse")?;
        Ok(())
    }

    /// Retrieve the NTP server configuration.
    ///
    /// Returns whether servers come from DHCP and the list of manually
    /// configured server addresses.
    pub async fn get_ntp(&self) -> Result<NtpInfo, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetNTP";
        const BODY: &str = "<tds:GetNTP/>";
        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetNTPResponse")?;
        NtpInfo::from_xml(resp)
    }

    /// Set the NTP server configuration.
    ///
    /// When `from_dhcp` is `true`, `servers` is ignored; DHCP provides the
    /// NTP servers. When `false`, each entry in `servers` is sent as a
    /// `NTPManual` element (accepted as either a DNS hostname or an IP
    /// address string).
    pub async fn set_ntp(&self, from_dhcp: bool, servers: &[&str]) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SetNTP";
        let from_dhcp_str = if from_dhcp { "true" } else { "false" };
        let server_els: String = servers
            .iter()
            .map(|s| {
                format!(
                    "<tds:NTPManual>\
                       <tt:Type>DNS</tt:Type>\
                       <tt:DNSname>{s}</tt:DNSname>\
                     </tds:NTPManual>"
                )
            })
            .collect();
        let body = format!(
            "<tds:SetNTP>\
               <tds:FromDHCP>{from_dhcp_str}</tds:FromDHCP>\
               {server_els}\
             </tds:SetNTP>"
        );
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetNTPResponse")?;
        Ok(())
    }

    /// Initiate a device reboot.
    ///
    /// Returns the device's informational reboot message (e.g.
    /// `"Rebooting in 30 seconds"`). The connection will drop shortly after.
    pub async fn system_reboot(&self) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SystemReboot";
        const BODY: &str = "<tds:SystemReboot/>";
        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "SystemRebootResponse")?;
        Ok(resp
            .child("Message")
            .map(|n| n.text().to_string())
            .unwrap_or_default())
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

    /// Create a new, initially empty media profile.
    ///
    /// `token` is optional; if omitted the device assigns one. Returns the
    /// newly created [`MediaProfile`] including the assigned token.
    pub async fn create_profile(
        &self,
        media_url: &str,
        name: &str,
        token: Option<&str>,
    ) -> Result<MediaProfile, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/CreateProfile";
        let token_el = token
            .map(|t| format!("<trt:Token>{t}</trt:Token>"))
            .unwrap_or_default();
        let body = format!(
            "<trt:CreateProfile>\
               <trt:Name>{name}</trt:Name>\
               {token_el}\
             </trt:CreateProfile>"
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "CreateProfileResponse")?;
        let p = resp
            .child("Profile")
            .ok_or_else(|| crate::soap::SoapError::missing("Profile"))?;
        Ok(MediaProfile::from_xml(p))
    }

    /// Delete a non-fixed media profile.
    ///
    /// Fixed profiles (where `profile.fixed == true`) cannot be deleted.
    pub async fn delete_profile(
        &self,
        media_url: &str,
        profile_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/DeleteProfile";
        let body = format!(
            "<trt:DeleteProfile>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
             </trt:DeleteProfile>"
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "DeleteProfileResponse")?;
        Ok(())
    }

    /// Retrieve a single media profile by token.
    pub async fn get_profile(
        &self,
        media_url: &str,
        profile_token: &str,
    ) -> Result<MediaProfile, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetProfile";
        let body = format!(
            "<trt:GetProfile>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
             </trt:GetProfile>"
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetProfileResponse")?;
        let p = resp
            .child("Profile")
            .ok_or_else(|| crate::soap::SoapError::missing("Profile"))?;
        Ok(MediaProfile::from_xml(p))
    }

    /// Bind a video encoder configuration to a media profile.
    ///
    /// `config_token` comes from a [`VideoEncoderConfiguration`] returned by
    /// [`get_video_encoder_configurations`](Self::get_video_encoder_configurations).
    pub async fn add_video_encoder_configuration(
        &self,
        media_url: &str,
        profile_token: &str,
        config_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/AddVideoEncoderConfiguration";
        let body = format!(
            "<trt:AddVideoEncoderConfiguration>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
               <trt:ConfigurationToken>{config_token}</trt:ConfigurationToken>\
             </trt:AddVideoEncoderConfiguration>"
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "AddVideoEncoderConfigurationResponse")?;
        Ok(())
    }

    /// Remove the video encoder configuration from a media profile.
    pub async fn remove_video_encoder_configuration(
        &self,
        media_url: &str,
        profile_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver10/media/wsdl/RemoveVideoEncoderConfiguration";
        let body = format!(
            "<trt:RemoveVideoEncoderConfiguration>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
             </trt:RemoveVideoEncoderConfiguration>"
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "RemoveVideoEncoderConfigurationResponse")?;
        Ok(())
    }

    /// Bind a video source configuration to a media profile.
    ///
    /// `config_token` comes from a [`VideoSourceConfiguration`] returned by
    /// [`get_video_source_configurations`](Self::get_video_source_configurations).
    pub async fn add_video_source_configuration(
        &self,
        media_url: &str,
        profile_token: &str,
        config_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/AddVideoSourceConfiguration";
        let body = format!(
            "<trt:AddVideoSourceConfiguration>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
               <trt:ConfigurationToken>{config_token}</trt:ConfigurationToken>\
             </trt:AddVideoSourceConfiguration>"
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "AddVideoSourceConfigurationResponse")?;
        Ok(())
    }

    /// Remove the video source configuration from a media profile.
    pub async fn remove_video_source_configuration(
        &self,
        media_url: &str,
        profile_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/RemoveVideoSourceConfiguration";
        let body = format!(
            "<trt:RemoveVideoSourceConfiguration>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
             </trt:RemoveVideoSourceConfiguration>"
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "RemoveVideoSourceConfigurationResponse")?;
        Ok(())
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

    /// Save the current camera position as a named preset.
    ///
    /// Pass `preset_name` to label the preset and `preset_token` to overwrite
    /// an existing preset rather than create a new one. Returns the token of
    /// the saved (or updated) preset.
    pub async fn ptz_set_preset(
        &self,
        ptz_url: &str,
        profile_token: &str,
        preset_name: Option<&str>,
        preset_token: Option<&str>,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/SetPreset";
        let name_el = preset_name
            .map(|n| format!("<tptz:PresetName>{n}</tptz:PresetName>"))
            .unwrap_or_default();
        let token_el = preset_token
            .map(|t| format!("<tptz:PresetToken>{t}</tptz:PresetToken>"))
            .unwrap_or_default();
        let body = format!(
            "<tptz:SetPreset>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
               {name_el}{token_el}\
             </tptz:SetPreset>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "SetPresetResponse")?;
        resp.child("PresetToken")
            .map(|n| n.text().to_string())
            .ok_or_else(|| crate::soap::SoapError::missing("PresetToken").into())
    }

    /// Delete a saved PTZ preset.
    ///
    /// `preset_token` comes from a [`PtzPreset`] returned by
    /// [`ptz_get_presets`](Self::ptz_get_presets).
    pub async fn ptz_remove_preset(
        &self,
        ptz_url: &str,
        profile_token: &str,
        preset_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/RemovePreset";
        let body = format!(
            "<tptz:RemovePreset>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
               <tptz:PresetToken>{preset_token}</tptz:PresetToken>\
             </tptz:RemovePreset>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "RemovePresetResponse")?;
        Ok(())
    }

    /// Query the current PTZ position and movement state.
    ///
    /// Returns a [`PtzStatus`] with the normalised pan, tilt, and zoom
    /// positions, and a movement state string (`"IDLE"` or `"MOVING"`).
    pub async fn ptz_get_status(
        &self,
        ptz_url: &str,
        profile_token: &str,
    ) -> Result<PtzStatus, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetStatus";
        let body = format!(
            "<tptz:GetStatus>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
             </tptz:GetStatus>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetStatusResponse")?;
        PtzStatus::from_xml(resp)
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

    // ── Imaging Service ───────────────────────────────────────────────────────

    /// Retrieve the current image quality settings for a video source.
    ///
    /// `imaging_url` is obtained from
    /// [`get_capabilities`](Self::get_capabilities) via `caps.imaging_url`.
    /// `video_source_token` comes from a [`VideoSource`] returned by
    /// [`get_video_sources`](Self::get_video_sources).
    pub async fn get_imaging_settings(
        &self,
        imaging_url: &str,
        video_source_token: &str,
    ) -> Result<ImagingSettings, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/imaging/wsdl/GetImagingSettings";
        let body = format!(
            "<timg:GetImagingSettings>\
               <timg:VideoSourceToken>{video_source_token}</timg:VideoSourceToken>\
             </timg:GetImagingSettings>"
        );
        let xml = self.call(imaging_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetImagingSettingsResponse")?;
        ImagingSettings::from_xml(resp)
    }

    /// Apply modified image quality settings to a video source.
    ///
    /// Obtain the current settings with
    /// [`get_imaging_settings`](Self::get_imaging_settings), modify the
    /// fields you want to change, then pass the result here.
    pub async fn set_imaging_settings(
        &self,
        imaging_url: &str,
        video_source_token: &str,
        settings: &ImagingSettings,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/imaging/wsdl/SetImagingSettings";
        let body = format!(
            "<timg:SetImagingSettings>\
               <timg:VideoSourceToken>{video_source_token}</timg:VideoSourceToken>\
               {settings_xml}\
               <timg:ForcePersistence>true</timg:ForcePersistence>\
             </timg:SetImagingSettings>",
            settings_xml = settings.to_xml_body()
        );
        let xml = self.call(imaging_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetImagingSettingsResponse")?;
        Ok(())
    }

    /// Retrieve the valid parameter ranges for `set_imaging_settings`.
    ///
    /// Use the returned [`ImagingOptions`] to validate or clamp values before
    /// calling [`set_imaging_settings`](Self::set_imaging_settings).
    pub async fn get_imaging_options(
        &self,
        imaging_url: &str,
        video_source_token: &str,
    ) -> Result<ImagingOptions, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/imaging/wsdl/GetOptions";
        let body = format!(
            "<timg:GetOptions>\
               <timg:VideoSourceToken>{video_source_token}</timg:VideoSourceToken>\
             </timg:GetOptions>"
        );
        let xml = self.call(imaging_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetOptionsResponse")?;
        ImagingOptions::from_xml(resp)
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

    // ── Events Service ────────────────────────────────────────────────────────

    /// Retrieve all event topics advertised by the device.
    ///
    /// `events_url` is obtained from [`get_capabilities`](Self::get_capabilities)
    /// via `caps.events.url`.
    pub async fn get_event_properties(
        &self,
        events_url: &str,
    ) -> Result<EventProperties, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver10/events/wsdl/EventPortType/GetEventPropertiesRequest";
        const BODY: &str = "<tev:GetEventProperties/>";

        let xml = self.call(events_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetEventPropertiesResponse")?;
        EventProperties::from_xml(resp)
    }

    /// Subscribe to device events using a pull-point endpoint.
    ///
    /// - `filter` — optional topic filter expression (e.g.
    ///   `"tns1:VideoSource/MotionAlarm"`); pass `None` to subscribe to all topics.
    /// - `initial_termination_time` — ISO 8601 duration or absolute time
    ///   (e.g. `"PT60S"`); pass `None` to use the device default.
    ///
    /// Returns a [`PullPointSubscription`] whose `reference_url` must be passed
    /// to [`pull_messages`](Self::pull_messages),
    /// [`renew_subscription`](Self::renew_subscription), and
    /// [`unsubscribe`](Self::unsubscribe).
    pub async fn create_pull_point_subscription(
        &self,
        events_url: &str,
        filter: Option<&str>,
        initial_termination_time: Option<&str>,
    ) -> Result<PullPointSubscription, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/events/wsdl/EventPortType/CreatePullPointSubscriptionRequest";

        let filter_el = filter
            .map(|f| {
                format!(
                    "<tev:Filter>\
                       <wsnt:TopicExpression \
                         Dialect=\"http://www.onvif.org/ver10/tev/topicExpression/ConcreteSet\"\
                       >{f}</wsnt:TopicExpression>\
                     </tev:Filter>"
                )
            })
            .unwrap_or_default();

        let termination_el = initial_termination_time
            .map(|t| format!("<tev:InitialTerminationTime>{t}</tev:InitialTerminationTime>"))
            .unwrap_or_default();

        let body = format!(
            "<tev:CreatePullPointSubscription>\
               {filter_el}{termination_el}\
             </tev:CreatePullPointSubscription>"
        );

        let xml = self.call(events_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "CreatePullPointSubscriptionResponse")?;
        PullPointSubscription::from_xml(resp)
    }

    /// Pull pending event messages from a subscription.
    ///
    /// - `subscription_url` — the `reference_url` from [`PullPointSubscription`].
    /// - `timeout` — ISO 8601 duration to long-poll for events (e.g. `"PT5S"`).
    /// - `max_messages` — maximum number of messages to return per call.
    pub async fn pull_messages(
        &self,
        subscription_url: &str,
        timeout: &str,
        max_messages: u32,
    ) -> Result<Vec<NotificationMessage>, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/PullMessagesRequest";

        let body = format!(
            "<tev:PullMessages>\
               <tev:Timeout>{timeout}</tev:Timeout>\
               <tev:MessageLimit>{max_messages}</tev:MessageLimit>\
             </tev:PullMessages>"
        );

        let xml = self.call(subscription_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "PullMessagesResponse")?;
        Ok(NotificationMessage::vec_from_xml(resp))
    }

    /// Extend the lifetime of an active pull-point subscription.
    ///
    /// `subscription_url` is the `reference_url` from [`PullPointSubscription`].
    /// `termination_time` is an ISO 8601 duration or absolute timestamp
    /// (e.g. `"PT60S"`).
    ///
    /// Returns the new termination timestamp set by the device.
    pub async fn renew_subscription(
        &self,
        subscription_url: &str,
        termination_time: &str,
    ) -> Result<String, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver10/events/wsdl/SubscriptionManager/RenewRequest";

        let body = format!(
            "<wsnt:Renew>\
               <wsnt:TerminationTime>{termination_time}</wsnt:TerminationTime>\
             </wsnt:Renew>"
        );

        let xml = self.call(subscription_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "RenewResponse")?;
        Ok(resp
            .child("TerminationTime")
            .map(|n| n.text().to_string())
            .unwrap_or_default())
    }

    /// Cancel an active pull-point subscription.
    ///
    /// `subscription_url` is the `reference_url` from [`PullPointSubscription`].
    pub async fn unsubscribe(&self, subscription_url: &str) -> Result<(), OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver10/events/wsdl/SubscriptionManager/UnsubscribeRequest";
        const BODY: &str = "<wsnt:Unsubscribe/>";

        let xml = self.call(subscription_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "UnsubscribeResponse")?;
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "tests/client_tests.rs"]
mod tests;
