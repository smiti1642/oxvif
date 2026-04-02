//! # oxvif
//!
//! An async Rust client library for the [ONVIF] IP camera protocol.
//!
//! ONVIF (Open Network Video Interface Forum) is the industry standard for
//! interoperability between IP-based security cameras. This library provides
//! a complete async client covering device management, media streaming,
//! PTZ control, imaging, on-screen display, events, recording, search, and
//! replay — all over SOAP/HTTP(S) with WS-Security authentication.
//!
//! ## ONVIF Profile coverage
//!
//! | Profile | Description | Coverage |
//! |---------|-------------|----------|
//! | **Profile S** | Video streaming | ~100% |
//! | **Profile T** | Advanced streaming (H.265, focus, OSD) | ~75% |
//! | **Profile G** | Recording & playback | ~80% |
//!
//! ## Supported services
//!
//! - **Device** — capabilities, scopes, device info, hostname, NTP, reboot
//! - **Media1 / Media2** — profiles, RTSP/snapshot URIs, video + audio config, OSD
//! - **PTZ** — absolute/relative/continuous move, presets, home position, status
//! - **Imaging** — brightness/contrast/exposure settings, focus move/stop/status
//! - **Events** — pull-point subscriptions, event polling, renew, unsubscribe
//! - **Recording** — list stored recordings
//! - **Search** — find recordings by scope, collect results, end search
//! - **Replay** — get RTSP playback URI for a stored recording
//! - **WS-Discovery** — UDP multicast probe to find cameras on the local network
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────┐
//! │                    OnvifClient                       │  ← public API
//! ├──────────────────────────────────────────────────────┤
//! │    soap::SoapEnvelope  │  soap::WsSecurityToken      │  ← SOAP layer
//! ├──────────────────────────────────────────────────────┤
//! │                  Transport trait                     │  ← HTTP abstraction
//! │          (HttpTransport / mock in tests)             │
//! └──────────────────────────────────────────────────────┘
//! ```
//!
//! ## Quick start
//!
//! ```no_run
//! use oxvif::{OnvifClient, OnvifError};
//!
//! async fn run() -> Result<(), OnvifError> {
//!     let client = OnvifClient::new("http://192.168.1.100/onvif/device_service")
//!         .with_credentials("admin", "password");
//!
//!     // Sync device clock for WS-Security timestamps
//!     let dt = client.get_system_date_and_time().await?;
//!     let client = client.with_utc_offset(dt.utc_offset_secs());
//!
//!     // Discover service endpoints
//!     let caps = client.get_capabilities().await?;
//!     let media_url = caps.media.url.as_deref().unwrap();
//!
//!     // List media profiles and get the first RTSP stream URI
//!     let profiles = client.get_profiles(media_url).await?;
//!     let uri = client.get_stream_uri(media_url, &profiles[0].token).await?;
//!     println!("RTSP stream: {}", uri.uri);
//!     Ok(())
//! }
//! ```
//!
//! ## Testing without a real camera
//!
//! Implement [`transport::Transport`] to inject any XML fixture:
//!
//! ```no_run
//! use oxvif::transport::{Transport, TransportError};
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! struct MockTransport { xml: String }
//!
//! #[async_trait]
//! impl Transport for MockTransport {
//!     async fn soap_post(&self, _url: &str, _action: &str, _body: String)
//!         -> Result<String, TransportError>
//!     {
//!         Ok(self.xml.clone())
//!     }
//! }
//!
//! # async fn example() {
//! let client = oxvif::OnvifClient::new("http://ignored")
//!     .with_transport(Arc::new(MockTransport { xml: "<s:Envelope/>".into() }));
//! # }
//! ```
//!
//! [ONVIF]: https://www.onvif.org

pub mod client;
pub mod discovery;
pub mod error;
pub mod soap;
pub mod transport;
pub mod types;

pub use client::OnvifClient;
pub use discovery::DiscoveredDevice;
pub use error::OnvifError;
pub use types::{
    AnalyticsCapabilities, AudioEncoderConfiguration, AudioEncoderConfigurationOptions,
    AudioEncoderOptions, AudioEncoding, AudioSource, AudioSourceConfiguration, BoundsRange,
    Capabilities, DeviceCapabilities, DeviceInfo, EncoderInstanceInfo, EventProperties,
    EventsCapabilities, FindRecordingResults, FloatRange, FocusMove, H264Configuration,
    H264Options, H265Configuration, H265Options, Hostname, ImagingMoveOptions, ImagingOptions,
    ImagingSettings, ImagingStatus, IntRange, IoCapabilities, JpegOptions, MediaCapabilities,
    MediaProfile, MediaProfile2, NetworkCapabilities, NotificationMessage, NtpInfo, OnvifService,
    OsdConfiguration, OsdOptions, OsdPosition, OsdTextString, PtzConfiguration,
    PtzConfigurationOptions, PtzNode, PtzPreset, PtzSpaceRange, PtzStatus, PullPointSubscription,
    RecordingInformation, RecordingItem, RecordingSourceInformation, RecordingTrack, Resolution,
    SecurityCapabilities, SnapshotUri, SourceBounds, StreamUri, StreamingCapabilities,
    SystemCapabilities, SystemDateTime, VideoEncoderConfiguration, VideoEncoderConfiguration2,
    VideoEncoderConfigurationOptions, VideoEncoderConfigurationOptions2, VideoEncoderInstances,
    VideoEncoderOptions2, VideoEncoding, VideoRateControl, VideoRateControl2, VideoSource,
    VideoSourceConfiguration, VideoSourceConfigurationOptions,
};
