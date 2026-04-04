//! High-level session wrapper around [`OnvifClient`].
//!
//! [`OnvifSession`] is a convenience layer built on top of the stateless
//! [`OnvifClient`]. It runs `GetCapabilities` once at construction and caches
//! the discovered service URLs so callers never need to pass endpoint URLs to
//! individual methods.
//!
//! ## `OnvifClient` vs `OnvifSession`
//!
//! | | `OnvifClient` | `OnvifSession` |
//! |---|---|---|
//! | Service URLs | Caller passes every time | Resolved automatically from cached `Capabilities` |
//! | State | Stateless | Holds `Capabilities` |
//! | Use case | Protocol-level control, custom flows | Typical application usage |
//!
//! `OnvifClient` remains the canonical ONVIF protocol implementation.
//! `OnvifSession` is an ergonomics wrapper — nothing it does is impossible
//! with `OnvifClient` directly.
//!
//! ## Quick start
//!
//! ```no_run
//! use oxvif::{OnvifSession, OnvifError};
//!
//! async fn run() -> Result<(), OnvifError> {
//!     let session = OnvifSession::builder("http://192.168.1.100/onvif/device_service")
//!         .with_credentials("admin", "password")
//!         .with_clock_sync()
//!         .build()
//!         .await?;
//!
//!     let profiles = session.get_profiles().await?;
//!     let uri = session.get_stream_uri(&profiles[0].token).await?;
//!     println!("RTSP: {}", uri.uri);
//!     Ok(())
//! }
//! ```

use std::sync::Arc;

use crate::client::OnvifClient;
use crate::error::OnvifError;
use crate::soap::SoapError;
use crate::transport::Transport;
use crate::types::{
    AudioEncoderConfiguration, AudioEncoderConfigurationOptions, AudioSource,
    AudioSourceConfiguration, Capabilities, DeviceInfo, DnsInformation, EventProperties,
    FindRecordingResults, FocusMove, Hostname, ImagingMoveOptions, ImagingOptions, ImagingSettings,
    ImagingStatus, MediaProfile, MediaProfile2, NetworkGateway, NetworkInterface, NetworkProtocol,
    NotificationMessage, NtpInfo, OnvifService, OsdConfiguration, OsdOptions, PtzConfiguration,
    PtzConfigurationOptions, PtzNode, PtzPreset, PtzStatus, PullPointSubscription, RecordingItem,
    RecordingJob, RecordingJobConfiguration, RecordingJobState, RelayOutput, SnapshotUri,
    StorageConfiguration, StreamUri, SystemDateTime, SystemLog, SystemUris, User,
    VideoEncoderConfiguration, VideoEncoderConfiguration2, VideoEncoderConfigurationOptions,
    VideoEncoderConfigurationOptions2, VideoEncoderInstances, VideoSource,
    VideoSourceConfiguration, VideoSourceConfigurationOptions,
};

// ── OnvifSessionBuilder ───────────────────────────────────────────────────────

/// Builder for [`OnvifSession`].
///
/// Obtained via [`OnvifSession::builder`].
pub struct OnvifSessionBuilder {
    device_url: String,
    credentials: Option<(String, String)>,
    sync_clock: bool,
    transport: Option<Arc<dyn Transport>>,
}

impl OnvifSessionBuilder {
    fn new(device_url: impl Into<String>) -> Self {
        Self {
            device_url: device_url.into(),
            credentials: None,
            sync_clock: false,
            transport: None,
        }
    }

    /// Set credentials for WS-Security `UsernameToken` authentication.
    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.credentials = Some((username.into(), password.into()));
        self
    }

    /// Call `GetSystemDateAndTime` before `GetCapabilities` and apply the
    /// resulting UTC offset to keep WS-Security timestamps in sync with the
    /// device clock.
    ///
    /// Recommended when the device clock may differ from local UTC, which
    /// can cause WS-Security authentication to fail.
    pub fn with_clock_sync(mut self) -> Self {
        self.sync_clock = true;
        self
    }

    /// Replace the default HTTP transport (primarily for testing).
    pub fn with_transport(mut self, transport: Arc<dyn Transport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Connect to the device and return an [`OnvifSession`].
    ///
    /// Steps performed:
    /// 1. Build an [`OnvifClient`] with the given settings.
    /// 2. If [`with_clock_sync`](Self::with_clock_sync) was set, call
    ///    `GetSystemDateAndTime` and apply the UTC offset.
    /// 3. Call `GetCapabilities` and cache the service URLs.
    pub async fn build(self) -> Result<OnvifSession, OnvifError> {
        let mut client = OnvifClient::new(&self.device_url);

        if let Some((user, pass)) = self.credentials {
            client = client.with_credentials(user, pass);
        }

        if let Some(transport) = self.transport {
            client = client.with_transport(transport);
        }

        if self.sync_clock {
            let dt = client.get_system_date_and_time().await?;
            client = client.with_utc_offset(dt.utc_offset_secs());
        }

        let caps = client.get_capabilities().await?;

        Ok(OnvifSession { client, caps })
    }
}

// ── OnvifSession ──────────────────────────────────────────────────────────────

/// High-level ONVIF session with cached service URLs.
///
/// Constructed via [`OnvifSession::builder`]. All methods delegate to the
/// underlying [`OnvifClient`] using service URLs discovered during
/// [`build`](OnvifSessionBuilder::build).
///
/// For operations not covered here, or when you need direct protocol control,
/// use [`client`](Self::client) to access the underlying `OnvifClient`.
#[derive(Clone)]
pub struct OnvifSession {
    client: OnvifClient,
    caps: Capabilities,
}

impl OnvifSession {
    /// Create a builder for `OnvifSession`.
    pub fn builder(device_url: impl Into<String>) -> OnvifSessionBuilder {
        OnvifSessionBuilder::new(device_url)
    }

    /// The underlying [`OnvifClient`] for direct protocol access.
    pub fn client(&self) -> &OnvifClient {
        &self.client
    }

    /// The device capabilities discovered at session construction.
    pub fn capabilities(&self) -> &Capabilities {
        &self.caps
    }

    // ── Private URL resolvers ─────────────────────────────────────────────────

    fn media_url(&self) -> Result<&str, OnvifError> {
        self.caps
            .media
            .url
            .as_deref()
            .ok_or_else(|| SoapError::missing("Media service URL").into())
    }

    fn media2_url(&self) -> Result<&str, OnvifError> {
        self.caps
            .media2_url
            .as_deref()
            .ok_or_else(|| SoapError::missing("Media2 service URL").into())
    }

    fn ptz_url(&self) -> Result<&str, OnvifError> {
        self.caps
            .ptz_url
            .as_deref()
            .ok_or_else(|| SoapError::missing("PTZ service URL").into())
    }

    fn imaging_url(&self) -> Result<&str, OnvifError> {
        self.caps
            .imaging_url
            .as_deref()
            .ok_or_else(|| SoapError::missing("Imaging service URL").into())
    }

    fn events_url(&self) -> Result<&str, OnvifError> {
        self.caps
            .events
            .url
            .as_deref()
            .ok_or_else(|| SoapError::missing("Events service URL").into())
    }

    fn recording_url(&self) -> Result<&str, OnvifError> {
        self.caps
            .recording_url
            .as_deref()
            .ok_or_else(|| SoapError::missing("Recording service URL").into())
    }

    fn search_url(&self) -> Result<&str, OnvifError> {
        self.caps
            .search_url
            .as_deref()
            .ok_or_else(|| SoapError::missing("Search service URL").into())
    }

    fn replay_url(&self) -> Result<&str, OnvifError> {
        self.caps
            .replay_url
            .as_deref()
            .ok_or_else(|| SoapError::missing("Replay service URL").into())
    }

    // ── Device Service ────────────────────────────────────────────────────────

    /// Retrieve all service endpoints advertised by the device.
    pub async fn get_services(&self) -> Result<Vec<OnvifService>, OnvifError> {
        self.client.get_services().await
    }

    /// Retrieve the device clock.
    pub async fn get_system_date_and_time(&self) -> Result<SystemDateTime, OnvifError> {
        self.client.get_system_date_and_time().await
    }

    /// Retrieve manufacturer, model, firmware version, and serial number.
    pub async fn get_device_info(&self) -> Result<DeviceInfo, OnvifError> {
        self.client.get_device_info().await
    }

    /// Retrieve the device hostname.
    pub async fn get_hostname(&self) -> Result<Hostname, OnvifError> {
        self.client.get_hostname().await
    }

    /// Set the device hostname.
    pub async fn set_hostname(&self, name: &str) -> Result<(), OnvifError> {
        self.client.set_hostname(name).await
    }

    /// Retrieve the NTP server configuration.
    pub async fn get_ntp(&self) -> Result<NtpInfo, OnvifError> {
        self.client.get_ntp().await
    }

    /// Set the NTP server configuration.
    pub async fn set_ntp(&self, from_dhcp: bool, servers: &[&str]) -> Result<(), OnvifError> {
        self.client.set_ntp(from_dhcp, servers).await
    }

    /// Initiate a device reboot.
    pub async fn system_reboot(&self) -> Result<String, OnvifError> {
        self.client.system_reboot().await
    }

    /// Retrieve the device's scope URIs.
    pub async fn get_scopes(&self) -> Result<Vec<String>, OnvifError> {
        self.client.get_scopes().await
    }

    /// Retrieve user accounts configured on the device.
    pub async fn get_users(&self) -> Result<Vec<User>, OnvifError> {
        self.client.get_users().await
    }

    /// Create one or more user accounts.
    pub async fn create_users(&self, users: &[(&str, &str, &str)]) -> Result<(), OnvifError> {
        self.client.create_users(users).await
    }

    /// Delete user accounts by username.
    pub async fn delete_users(&self, usernames: &[&str]) -> Result<(), OnvifError> {
        self.client.delete_users(usernames).await
    }

    /// Modify an existing user account.
    pub async fn set_user(
        &self,
        username: &str,
        password: Option<&str>,
        user_level: &str,
    ) -> Result<(), OnvifError> {
        self.client.set_user(username, password, user_level).await
    }

    /// Retrieve all network interfaces and their IPv4/IPv6 configuration.
    pub async fn get_network_interfaces(&self) -> Result<Vec<NetworkInterface>, OnvifError> {
        self.client.get_network_interfaces().await
    }

    /// Update the IPv4 configuration of a network interface.
    pub async fn set_network_interfaces(
        &self,
        token: &str,
        enabled: bool,
        ipv4_address: &str,
        prefix_length: u32,
        from_dhcp: bool,
    ) -> Result<bool, OnvifError> {
        self.client
            .set_network_interfaces(token, enabled, ipv4_address, prefix_length, from_dhcp)
            .await
    }

    /// Retrieve the enabled network protocols.
    pub async fn get_network_protocols(&self) -> Result<Vec<NetworkProtocol>, OnvifError> {
        self.client.get_network_protocols().await
    }

    /// Retrieve the DNS server configuration.
    pub async fn get_dns(&self) -> Result<DnsInformation, OnvifError> {
        self.client.get_dns().await
    }

    /// Set the DNS server configuration.
    pub async fn set_dns(&self, from_dhcp: bool, servers: &[&str]) -> Result<(), OnvifError> {
        self.client.set_dns(from_dhcp, servers).await
    }

    /// Retrieve the default IPv4 and IPv6 gateway addresses.
    pub async fn get_network_default_gateway(&self) -> Result<NetworkGateway, OnvifError> {
        self.client.get_network_default_gateway().await
    }

    /// Retrieve the device system log.
    pub async fn get_system_log(&self, log_type: &str) -> Result<SystemLog, OnvifError> {
        self.client.get_system_log(log_type).await
    }

    /// Retrieve all relay output port configurations.
    pub async fn get_relay_outputs(&self) -> Result<Vec<RelayOutput>, OnvifError> {
        self.client.get_relay_outputs().await
    }

    /// Set the electrical state of a relay output port.
    pub async fn set_relay_output_state(
        &self,
        relay_token: &str,
        state: &str,
    ) -> Result<(), OnvifError> {
        self.client.set_relay_output_state(relay_token, state).await
    }

    /// Configure the properties of a relay output port.
    pub async fn set_relay_output_settings(
        &self,
        relay_token: &str,
        mode: &str,
        delay_time: &str,
        idle_state: &str,
    ) -> Result<(), OnvifError> {
        self.client
            .set_relay_output_settings(relay_token, mode, delay_time, idle_state)
            .await
    }

    /// Enable or disable network protocols (HTTP, HTTPS, RTSP, etc.).
    pub async fn set_network_protocols(
        &self,
        protocols: &[(&str, bool, &[u32])],
    ) -> Result<(), OnvifError> {
        self.client.set_network_protocols(protocols).await
    }

    /// Restore the device to factory defaults.
    pub async fn set_system_factory_default(&self, default_type: &str) -> Result<(), OnvifError> {
        self.client.set_system_factory_default(default_type).await
    }

    /// Retrieve all storage locations configured on the device.
    pub async fn get_storage_configurations(
        &self,
    ) -> Result<Vec<StorageConfiguration>, OnvifError> {
        self.client.get_storage_configurations().await
    }

    /// Create or update a storage configuration entry.
    pub async fn set_storage_configuration(
        &self,
        token: &str,
        storage_type: &str,
        local_path: &str,
        storage_uri: &str,
        user: &str,
        use_anonymous: bool,
    ) -> Result<(), OnvifError> {
        self.client
            .set_storage_configuration(
                token,
                storage_type,
                local_path,
                storage_uri,
                user,
                use_anonymous,
            )
            .await
    }

    /// Retrieve HTTP URIs for firmware upgrade, system log, and support-info download.
    pub async fn get_system_uris(&self) -> Result<SystemUris, OnvifError> {
        self.client.get_system_uris().await
    }

    /// Retrieve the current WS-Discovery mode.
    pub async fn get_discovery_mode(&self) -> Result<String, OnvifError> {
        self.client.get_discovery_mode().await
    }

    /// Set the WS-Discovery mode (`"Discoverable"` or `"NonDiscoverable"`).
    pub async fn set_discovery_mode(&self, mode: &str) -> Result<(), OnvifError> {
        self.client.set_discovery_mode(mode).await
    }

    // ── Media1 Service ────────────────────────────────────────────────────────

    /// List all media profiles.
    pub async fn get_profiles(&self) -> Result<Vec<MediaProfile>, OnvifError> {
        self.client.get_profiles(self.media_url()?).await
    }

    /// Retrieve a single media profile by token.
    pub async fn get_profile(&self, profile_token: &str) -> Result<MediaProfile, OnvifError> {
        self.client
            .get_profile(self.media_url()?, profile_token)
            .await
    }

    /// Create a new, initially empty media profile.
    pub async fn create_profile(
        &self,
        name: &str,
        token: Option<&str>,
    ) -> Result<MediaProfile, OnvifError> {
        self.client
            .create_profile(self.media_url()?, name, token)
            .await
    }

    /// Delete a non-fixed media profile.
    pub async fn delete_profile(&self, profile_token: &str) -> Result<(), OnvifError> {
        self.client
            .delete_profile(self.media_url()?, profile_token)
            .await
    }

    /// Retrieve an RTSP stream URI for the given media profile.
    pub async fn get_stream_uri(&self, profile_token: &str) -> Result<StreamUri, OnvifError> {
        self.client
            .get_stream_uri(self.media_url()?, profile_token)
            .await
    }

    /// Retrieve an HTTP snapshot URI for the given media profile.
    pub async fn get_snapshot_uri(&self, profile_token: &str) -> Result<SnapshotUri, OnvifError> {
        self.client
            .get_snapshot_uri(self.media_url()?, profile_token)
            .await
    }

    /// Bind a video encoder configuration to a media profile.
    pub async fn add_video_encoder_configuration(
        &self,
        profile_token: &str,
        config_token: &str,
    ) -> Result<(), OnvifError> {
        self.client
            .add_video_encoder_configuration(self.media_url()?, profile_token, config_token)
            .await
    }

    /// Remove the video encoder configuration from a media profile.
    pub async fn remove_video_encoder_configuration(
        &self,
        profile_token: &str,
    ) -> Result<(), OnvifError> {
        self.client
            .remove_video_encoder_configuration(self.media_url()?, profile_token)
            .await
    }

    /// Bind a video source configuration to a media profile.
    pub async fn add_video_source_configuration(
        &self,
        profile_token: &str,
        config_token: &str,
    ) -> Result<(), OnvifError> {
        self.client
            .add_video_source_configuration(self.media_url()?, profile_token, config_token)
            .await
    }

    /// Remove the video source configuration from a media profile.
    pub async fn remove_video_source_configuration(
        &self,
        profile_token: &str,
    ) -> Result<(), OnvifError> {
        self.client
            .remove_video_source_configuration(self.media_url()?, profile_token)
            .await
    }

    /// List all physical video sources.
    pub async fn get_video_sources(&self) -> Result<Vec<VideoSource>, OnvifError> {
        self.client.get_video_sources(self.media_url()?).await
    }

    /// List all video source configurations.
    pub async fn get_video_source_configurations(
        &self,
    ) -> Result<Vec<VideoSourceConfiguration>, OnvifError> {
        self.client
            .get_video_source_configurations(self.media_url()?)
            .await
    }

    /// Retrieve a single video source configuration by token.
    pub async fn get_video_source_configuration(
        &self,
        token: &str,
    ) -> Result<VideoSourceConfiguration, OnvifError> {
        self.client
            .get_video_source_configuration(self.media_url()?, token)
            .await
    }

    /// Apply a modified video source configuration.
    pub async fn set_video_source_configuration(
        &self,
        config: &VideoSourceConfiguration,
    ) -> Result<(), OnvifError> {
        self.client
            .set_video_source_configuration(self.media_url()?, config)
            .await
    }

    /// Retrieve valid parameter ranges for video source configuration.
    pub async fn get_video_source_configuration_options(
        &self,
        config_token: Option<&str>,
    ) -> Result<VideoSourceConfigurationOptions, OnvifError> {
        self.client
            .get_video_source_configuration_options(self.media_url()?, config_token)
            .await
    }

    /// List all video encoder configurations.
    pub async fn get_video_encoder_configurations(
        &self,
    ) -> Result<Vec<VideoEncoderConfiguration>, OnvifError> {
        self.client
            .get_video_encoder_configurations(self.media_url()?)
            .await
    }

    /// Retrieve a single video encoder configuration by token.
    pub async fn get_video_encoder_configuration(
        &self,
        token: &str,
    ) -> Result<VideoEncoderConfiguration, OnvifError> {
        self.client
            .get_video_encoder_configuration(self.media_url()?, token)
            .await
    }

    /// Apply a modified video encoder configuration.
    pub async fn set_video_encoder_configuration(
        &self,
        config: &VideoEncoderConfiguration,
    ) -> Result<(), OnvifError> {
        self.client
            .set_video_encoder_configuration(self.media_url()?, config)
            .await
    }

    /// Retrieve valid parameter ranges for video encoder configuration.
    pub async fn get_video_encoder_configuration_options(
        &self,
        config_token: Option<&str>,
    ) -> Result<VideoEncoderConfigurationOptions, OnvifError> {
        self.client
            .get_video_encoder_configuration_options(self.media_url()?, config_token)
            .await
    }

    // ── OSD Service ───────────────────────────────────────────────────────────

    /// List all OSD elements.
    pub async fn get_osds(
        &self,
        config_token: Option<&str>,
    ) -> Result<Vec<OsdConfiguration>, OnvifError> {
        self.client.get_osds(self.media_url()?, config_token).await
    }

    /// Retrieve a single OSD element by token.
    pub async fn get_osd(&self, osd_token: &str) -> Result<OsdConfiguration, OnvifError> {
        self.client.get_osd(self.media_url()?, osd_token).await
    }

    /// Update an existing OSD element.
    pub async fn set_osd(&self, osd: &OsdConfiguration) -> Result<(), OnvifError> {
        self.client.set_osd(self.media_url()?, osd).await
    }

    /// Create a new OSD element and return the assigned token.
    pub async fn create_osd(&self, osd: &OsdConfiguration) -> Result<String, OnvifError> {
        self.client.create_osd(self.media_url()?, osd).await
    }

    /// Delete an OSD element.
    pub async fn delete_osd(&self, osd_token: &str) -> Result<(), OnvifError> {
        self.client.delete_osd(self.media_url()?, osd_token).await
    }

    /// Retrieve valid OSD configuration options.
    pub async fn get_osd_options(&self, config_token: &str) -> Result<OsdOptions, OnvifError> {
        self.client
            .get_osd_options(self.media_url()?, config_token)
            .await
    }

    // ── Audio Service ─────────────────────────────────────────────────────────

    /// List all physical audio inputs.
    pub async fn get_audio_sources(&self) -> Result<Vec<AudioSource>, OnvifError> {
        self.client.get_audio_sources(self.media_url()?).await
    }

    /// List all audio source configurations.
    pub async fn get_audio_source_configurations(
        &self,
    ) -> Result<Vec<AudioSourceConfiguration>, OnvifError> {
        self.client
            .get_audio_source_configurations(self.media_url()?)
            .await
    }

    /// List all audio encoder configurations.
    pub async fn get_audio_encoder_configurations(
        &self,
    ) -> Result<Vec<AudioEncoderConfiguration>, OnvifError> {
        self.client
            .get_audio_encoder_configurations(self.media_url()?)
            .await
    }

    /// Retrieve a single audio encoder configuration by token.
    pub async fn get_audio_encoder_configuration(
        &self,
        config_token: &str,
    ) -> Result<AudioEncoderConfiguration, OnvifError> {
        self.client
            .get_audio_encoder_configuration(self.media_url()?, config_token)
            .await
    }

    /// Write an audio encoder configuration back to the device.
    pub async fn set_audio_encoder_configuration(
        &self,
        config: &AudioEncoderConfiguration,
    ) -> Result<(), OnvifError> {
        self.client
            .set_audio_encoder_configuration(self.media_url()?, config)
            .await
    }

    /// Retrieve valid parameter ranges for an audio encoder configuration.
    pub async fn get_audio_encoder_configuration_options(
        &self,
        config_token: &str,
    ) -> Result<AudioEncoderConfigurationOptions, OnvifError> {
        self.client
            .get_audio_encoder_configuration_options(self.media_url()?, config_token)
            .await
    }

    // ── Media2 Service ────────────────────────────────────────────────────────

    /// List all media profiles via the Media2 service.
    pub async fn get_profiles_media2(&self) -> Result<Vec<MediaProfile2>, OnvifError> {
        self.client.get_profiles_media2(self.media2_url()?).await
    }

    /// Retrieve an RTSP stream URI via the Media2 service.
    pub async fn get_stream_uri_media2(&self, profile_token: &str) -> Result<String, OnvifError> {
        self.client
            .get_stream_uri_media2(self.media2_url()?, profile_token)
            .await
    }

    /// Retrieve an HTTP snapshot URI via the Media2 service.
    pub async fn get_snapshot_uri_media2(&self, profile_token: &str) -> Result<String, OnvifError> {
        self.client
            .get_snapshot_uri_media2(self.media2_url()?, profile_token)
            .await
    }

    /// List all video source configurations via the Media2 service.
    pub async fn get_video_source_configurations_media2(
        &self,
    ) -> Result<Vec<VideoSourceConfiguration>, OnvifError> {
        self.client
            .get_video_source_configurations_media2(self.media2_url()?)
            .await
    }

    /// Apply a modified video source configuration via the Media2 service.
    pub async fn set_video_source_configuration_media2(
        &self,
        config: &VideoSourceConfiguration,
    ) -> Result<(), OnvifError> {
        self.client
            .set_video_source_configuration_media2(self.media2_url()?, config)
            .await
    }

    /// Retrieve valid parameter ranges for video source configuration via Media2.
    pub async fn get_video_source_configuration_options_media2(
        &self,
        config_token: Option<&str>,
    ) -> Result<VideoSourceConfigurationOptions, OnvifError> {
        self.client
            .get_video_source_configuration_options_media2(self.media2_url()?, config_token)
            .await
    }

    /// List all video encoder configurations via the Media2 service.
    pub async fn get_video_encoder_configurations_media2(
        &self,
    ) -> Result<Vec<VideoEncoderConfiguration2>, OnvifError> {
        self.client
            .get_video_encoder_configurations_media2(self.media2_url()?)
            .await
    }

    /// Retrieve a single video encoder configuration by token via the Media2 service.
    pub async fn get_video_encoder_configuration_media2(
        &self,
        token: &str,
    ) -> Result<VideoEncoderConfiguration2, OnvifError> {
        self.client
            .get_video_encoder_configuration_media2(self.media2_url()?, token)
            .await
    }

    /// Apply a modified video encoder configuration via the Media2 service.
    pub async fn set_video_encoder_configuration_media2(
        &self,
        config: &VideoEncoderConfiguration2,
    ) -> Result<(), OnvifError> {
        self.client
            .set_video_encoder_configuration_media2(self.media2_url()?, config)
            .await
    }

    /// Retrieve valid parameter ranges for video encoder configuration via Media2.
    pub async fn get_video_encoder_configuration_options_media2(
        &self,
        config_token: Option<&str>,
    ) -> Result<VideoEncoderConfigurationOptions2, OnvifError> {
        self.client
            .get_video_encoder_configuration_options_media2(self.media2_url()?, config_token)
            .await
    }

    /// Retrieve encoder instance capacity info via the Media2 service.
    pub async fn get_video_encoder_instances_media2(
        &self,
        config_token: &str,
    ) -> Result<VideoEncoderInstances, OnvifError> {
        self.client
            .get_video_encoder_instances_media2(self.media2_url()?, config_token)
            .await
    }

    /// Create a new media profile via the Media2 service.
    pub async fn create_profile_media2(&self, name: &str) -> Result<String, OnvifError> {
        self.client
            .create_profile_media2(self.media2_url()?, name)
            .await
    }

    /// Delete a media profile via the Media2 service.
    pub async fn delete_profile_media2(&self, token: &str) -> Result<(), OnvifError> {
        self.client
            .delete_profile_media2(self.media2_url()?, token)
            .await
    }

    // ── PTZ Service ───────────────────────────────────────────────────────────

    /// Move the camera to an absolute position.
    pub async fn ptz_absolute_move(
        &self,
        profile_token: &str,
        pan: f32,
        tilt: f32,
        zoom: f32,
    ) -> Result<(), OnvifError> {
        self.client
            .ptz_absolute_move(self.ptz_url()?, profile_token, pan, tilt, zoom)
            .await
    }

    /// Move the camera by a relative offset.
    pub async fn ptz_relative_move(
        &self,
        profile_token: &str,
        pan: f32,
        tilt: f32,
        zoom: f32,
    ) -> Result<(), OnvifError> {
        self.client
            .ptz_relative_move(self.ptz_url()?, profile_token, pan, tilt, zoom)
            .await
    }

    /// Start continuous pan/tilt/zoom movement.
    pub async fn ptz_continuous_move(
        &self,
        profile_token: &str,
        pan: f32,
        tilt: f32,
        zoom: f32,
    ) -> Result<(), OnvifError> {
        self.client
            .ptz_continuous_move(self.ptz_url()?, profile_token, pan, tilt, zoom)
            .await
    }

    /// Stop all ongoing PTZ movement.
    pub async fn ptz_stop(&self, profile_token: &str) -> Result<(), OnvifError> {
        self.client.ptz_stop(self.ptz_url()?, profile_token).await
    }

    /// List all saved PTZ presets for the given profile.
    pub async fn ptz_get_presets(&self, profile_token: &str) -> Result<Vec<PtzPreset>, OnvifError> {
        self.client
            .ptz_get_presets(self.ptz_url()?, profile_token)
            .await
    }

    /// Move the camera to a saved PTZ preset.
    pub async fn ptz_goto_preset(
        &self,
        profile_token: &str,
        preset_token: &str,
    ) -> Result<(), OnvifError> {
        self.client
            .ptz_goto_preset(self.ptz_url()?, profile_token, preset_token)
            .await
    }

    /// Save the current camera position as a named preset.
    pub async fn ptz_set_preset(
        &self,
        profile_token: &str,
        preset_name: Option<&str>,
        preset_token: Option<&str>,
    ) -> Result<String, OnvifError> {
        self.client
            .ptz_set_preset(self.ptz_url()?, profile_token, preset_name, preset_token)
            .await
    }

    /// Delete a saved PTZ preset.
    pub async fn ptz_remove_preset(
        &self,
        profile_token: &str,
        preset_token: &str,
    ) -> Result<(), OnvifError> {
        self.client
            .ptz_remove_preset(self.ptz_url()?, profile_token, preset_token)
            .await
    }

    /// Query the current PTZ position and movement state.
    pub async fn ptz_get_status(&self, profile_token: &str) -> Result<PtzStatus, OnvifError> {
        self.client
            .ptz_get_status(self.ptz_url()?, profile_token)
            .await
    }

    /// Move the camera to its configured home position.
    pub async fn ptz_goto_home_position(
        &self,
        profile_token: &str,
        speed: Option<f32>,
    ) -> Result<(), OnvifError> {
        self.client
            .ptz_goto_home_position(self.ptz_url()?, profile_token, speed)
            .await
    }

    /// Set the current PTZ position as the home position.
    pub async fn ptz_set_home_position(&self, profile_token: &str) -> Result<(), OnvifError> {
        self.client
            .ptz_set_home_position(self.ptz_url()?, profile_token)
            .await
    }

    /// List all PTZ configurations.
    pub async fn ptz_get_configurations(&self) -> Result<Vec<PtzConfiguration>, OnvifError> {
        self.client.ptz_get_configurations(self.ptz_url()?).await
    }

    /// Retrieve a single PTZ configuration by token.
    pub async fn ptz_get_configuration(
        &self,
        config_token: &str,
    ) -> Result<PtzConfiguration, OnvifError> {
        self.client
            .ptz_get_configuration(self.ptz_url()?, config_token)
            .await
    }

    /// Write a PTZ configuration back to the device.
    pub async fn ptz_set_configuration(
        &self,
        config: &PtzConfiguration,
        force_persist: bool,
    ) -> Result<(), OnvifError> {
        self.client
            .ptz_set_configuration(self.ptz_url()?, config, force_persist)
            .await
    }

    /// Retrieve valid parameter ranges for a PTZ configuration.
    pub async fn ptz_get_configuration_options(
        &self,
        config_token: &str,
    ) -> Result<PtzConfigurationOptions, OnvifError> {
        self.client
            .ptz_get_configuration_options(self.ptz_url()?, config_token)
            .await
    }

    /// List all PTZ nodes.
    pub async fn ptz_get_nodes(&self) -> Result<Vec<PtzNode>, OnvifError> {
        self.client.ptz_get_nodes(self.ptz_url()?).await
    }

    // ── Imaging Service ───────────────────────────────────────────────────────

    /// Retrieve the current image quality settings for a video source.
    pub async fn get_imaging_settings(
        &self,
        video_source_token: &str,
    ) -> Result<ImagingSettings, OnvifError> {
        self.client
            .get_imaging_settings(self.imaging_url()?, video_source_token)
            .await
    }

    /// Apply modified image quality settings to a video source.
    pub async fn set_imaging_settings(
        &self,
        video_source_token: &str,
        settings: &ImagingSettings,
    ) -> Result<(), OnvifError> {
        self.client
            .set_imaging_settings(self.imaging_url()?, video_source_token, settings)
            .await
    }

    /// Retrieve valid parameter ranges for imaging settings.
    pub async fn get_imaging_options(
        &self,
        video_source_token: &str,
    ) -> Result<ImagingOptions, OnvifError> {
        self.client
            .get_imaging_options(self.imaging_url()?, video_source_token)
            .await
    }

    /// Move the focus to an absolute position, relative distance, or start
    /// continuous movement.
    pub async fn imaging_move(
        &self,
        video_source_token: &str,
        focus: &FocusMove,
    ) -> Result<(), OnvifError> {
        self.client
            .imaging_move(self.imaging_url()?, video_source_token, focus)
            .await
    }

    /// Stop any ongoing focus movement.
    pub async fn imaging_stop(&self, video_source_token: &str) -> Result<(), OnvifError> {
        self.client
            .imaging_stop(self.imaging_url()?, video_source_token)
            .await
    }

    /// Retrieve valid focus movement ranges.
    pub async fn imaging_get_move_options(
        &self,
        video_source_token: &str,
    ) -> Result<ImagingMoveOptions, OnvifError> {
        self.client
            .imaging_get_move_options(self.imaging_url()?, video_source_token)
            .await
    }

    /// Retrieve the current focus position and movement state.
    pub async fn imaging_get_status(
        &self,
        video_source_token: &str,
    ) -> Result<ImagingStatus, OnvifError> {
        self.client
            .imaging_get_status(self.imaging_url()?, video_source_token)
            .await
    }

    // ── Events Service ────────────────────────────────────────────────────────

    /// Retrieve all event topics advertised by the device.
    pub async fn get_event_properties(&self) -> Result<EventProperties, OnvifError> {
        self.client.get_event_properties(self.events_url()?).await
    }

    /// Subscribe to device events using a pull-point endpoint.
    pub async fn create_pull_point_subscription(
        &self,
        filter: Option<&str>,
        initial_termination_time: Option<&str>,
    ) -> Result<PullPointSubscription, OnvifError> {
        self.client
            .create_pull_point_subscription(self.events_url()?, filter, initial_termination_time)
            .await
    }

    /// Pull pending event messages from a subscription.
    ///
    /// `subscription_url` comes from [`PullPointSubscription::reference_url`].
    pub async fn pull_messages(
        &self,
        subscription_url: &str,
        timeout: &str,
        max_messages: u32,
    ) -> Result<Vec<NotificationMessage>, OnvifError> {
        self.client
            .pull_messages(subscription_url, timeout, max_messages)
            .await
    }

    /// Extend the lifetime of an active pull-point subscription.
    pub async fn renew_subscription(
        &self,
        subscription_url: &str,
        termination_time: &str,
    ) -> Result<String, OnvifError> {
        self.client
            .renew_subscription(subscription_url, termination_time)
            .await
    }

    /// Cancel an active pull-point subscription.
    pub async fn unsubscribe(&self, subscription_url: &str) -> Result<(), OnvifError> {
        self.client.unsubscribe(subscription_url).await
    }

    /// Wrap `pull_messages` polling into an infinite async stream of notification
    /// messages.
    ///
    /// `subscription_url` comes from [`PullPointSubscription::reference_url`].
    /// `timeout` is an ISO 8601 long-poll duration (e.g. `"PT5S"`).
    /// `max_messages` is the maximum number of events to fetch per poll.
    ///
    /// # Example (requires `futures` in caller's `[dependencies]`)
    ///
    /// ```no_run
    /// use futures::StreamExt as _;
    ///
    /// # async fn example(session: oxvif::OnvifSession) -> Result<(), oxvif::OnvifError> {
    /// let sub = session.create_pull_point_subscription(None, None).await?;
    /// let mut stream = session.event_stream(&sub.reference_url, "PT5S", 10);
    /// while let Some(Ok(msg)) = stream.next().await {
    ///     println!("Event: {} {:?}", msg.topic, msg.data);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn event_stream<'a>(
        &'a self,
        subscription_url: &'a str,
        timeout: &'a str,
        max_messages: u32,
    ) -> std::pin::Pin<
        Box<dyn futures_core::Stream<Item = Result<NotificationMessage, OnvifError>> + 'a>,
    > {
        self.client
            .event_stream(subscription_url, timeout, max_messages)
    }

    // ── Recording Service ─────────────────────────────────────────────────────

    /// List all recordings stored on the device.
    pub async fn get_recordings(&self) -> Result<Vec<RecordingItem>, OnvifError> {
        self.client.get_recordings(self.recording_url()?).await
    }

    // ── Search Service ────────────────────────────────────────────────────────

    /// Start an asynchronous search for recordings.
    pub async fn find_recordings(
        &self,
        max_matches: Option<u32>,
        keep_alive_timeout: &str,
    ) -> Result<String, OnvifError> {
        self.client
            .find_recordings(self.search_url()?, max_matches, keep_alive_timeout)
            .await
    }

    /// Retrieve results for a recording search.
    pub async fn get_recording_search_results(
        &self,
        search_token: &str,
        max_results: u32,
        wait_time: &str,
    ) -> Result<FindRecordingResults, OnvifError> {
        self.client
            .get_recording_search_results(self.search_url()?, search_token, max_results, wait_time)
            .await
    }

    /// Release a search session on the device.
    pub async fn end_search(&self, search_token: &str) -> Result<(), OnvifError> {
        self.client
            .end_search(self.search_url()?, search_token)
            .await
    }

    // ── Replay Service ────────────────────────────────────────────────────────

    /// Retrieve an RTSP URI for replaying a stored recording.
    pub async fn get_replay_uri(
        &self,
        recording_token: &str,
        stream_type: &str,
        protocol: &str,
    ) -> Result<String, OnvifError> {
        self.client
            .get_replay_uri(self.replay_url()?, recording_token, stream_type, protocol)
            .await
    }

    /// Create a new recording configuration on the device.
    pub async fn create_recording(
        &self,
        source_name: &str,
        source_id: &str,
        location: &str,
        description: &str,
        content: &str,
    ) -> Result<String, OnvifError> {
        self.client
            .create_recording(
                self.recording_url()?,
                source_name,
                source_id,
                location,
                description,
                content,
            )
            .await
    }

    /// Delete a recording and all its tracks from the device.
    pub async fn delete_recording(&self, recording_token: &str) -> Result<(), OnvifError> {
        self.client
            .delete_recording(self.recording_url()?, recording_token)
            .await
    }

    /// Add a new track to an existing recording.
    pub async fn create_track(
        &self,
        recording_token: &str,
        track_type: &str,
        description: &str,
    ) -> Result<String, OnvifError> {
        self.client
            .create_track(
                self.recording_url()?,
                recording_token,
                track_type,
                description,
            )
            .await
    }

    /// Remove a track from a recording.
    pub async fn delete_track(
        &self,
        recording_token: &str,
        track_token: &str,
    ) -> Result<(), OnvifError> {
        self.client
            .delete_track(self.recording_url()?, recording_token, track_token)
            .await
    }

    /// List all recording jobs on the device.
    pub async fn get_recording_jobs(&self) -> Result<Vec<RecordingJob>, OnvifError> {
        self.client.get_recording_jobs(self.recording_url()?).await
    }

    /// Create a new recording job.
    pub async fn create_recording_job(
        &self,
        config: &RecordingJobConfiguration,
    ) -> Result<String, OnvifError> {
        self.client
            .create_recording_job(self.recording_url()?, config)
            .await
    }

    /// Enable or disable a recording job (`mode`: `"Active"` or `"Idle"`).
    pub async fn set_recording_job_mode(
        &self,
        job_token: &str,
        mode: &str,
    ) -> Result<(), OnvifError> {
        self.client
            .set_recording_job_mode(self.recording_url()?, job_token, mode)
            .await
    }

    /// Delete a recording job from the device.
    pub async fn delete_recording_job(&self, job_token: &str) -> Result<(), OnvifError> {
        self.client
            .delete_recording_job(self.recording_url()?, job_token)
            .await
    }

    /// Get the current operational state of a recording job.
    pub async fn get_recording_job_state(
        &self,
        job_token: &str,
    ) -> Result<RecordingJobState, OnvifError> {
        self.client
            .get_recording_job_state(self.recording_url()?, job_token)
            .await
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "tests/session_tests.rs"]
mod tests;
