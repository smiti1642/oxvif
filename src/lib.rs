//! # oxvif
//!
//! An async Rust client library for the [ONVIF] IP camera protocol.
//!
//! ONVIF (Open Network Video Interface Forum) is the industry standard for
//! interoperability between IP-based security cameras. This library provides
//! a complete async client covering device management, media streaming,
//! PTZ control, imaging, on-screen display, events, recording, search, and
//! replay — all over SOAP/HTTP(S) with WS-Security and HTTP Digest
//! authentication.
//!
//! ## ONVIF Profile coverage
//!
//! | Profile | Description | Coverage | Notes |
//! |---------|-------------|----------|-------|
//! | **Profile S** | Video streaming | ~95% | All core operations implemented |
//! | **Profile T** | Advanced streaming (H.265, focus, OSD, audio) | ~95% | HTTP Digest Auth, Media2 audio/metadata/analytics config, PTZ compat; Analytics rules and DeviceIO not yet implemented |
//! | **Profile G** | Recording & playback | ~85% | Read/search/replay + full recording/job write management; live-source job binding not yet implemented |
//!
//! ## Supported services
//!
//! - **Device** — capabilities, scopes, device info, hostname, NTP, reboot,
//!   user management, network interfaces/protocols/DNS/gateway, relay outputs,
//!   storage configurations, system log/URIs, factory default, discovery mode,
//!   firmware upgrade / system restore (upload-URI flow),
//!   auxiliary commands (wiper/IR lamp)
//! - **Media1 / Media2** — profiles, RTSP/snapshot URIs, video + audio config, OSD,
//!   metadata config, audio decoder/output config, video source modes,
//!   unified AddConfiguration/RemoveConfiguration
//! - **PTZ** — absolute/relative/continuous move, presets, home position, status,
//!   configurations, nodes, compatible configurations
//! - **Imaging** — brightness/contrast/exposure settings, focus move/stop/status
//! - **Events** — pull-point subscriptions, event polling, renew, unsubscribe,
//!   continuous `event_stream`, synchronization point
//! - **Recording** — list stored recordings; create/delete recordings, tracks, and recording jobs
//! - **Search** — find recordings by scope, collect results, end search
//! - **Replay** — get RTSP playback URI for a stored recording
//! - **WS-Discovery** — UDP multicast probe to find cameras on the local network
//!
//! ## Optional features
//!
//! - **`health`** — `health::HealthCheck`, a fast read-only conformance check
//!   that returns a `health::HealthReport` (per-check Pass/Warn/Fail/Skip plus
//!   a Profile S/T/G assessment). Pure library code over [`OnvifSession`].
//! - **`mock`** / **`mock-server`** — a built-in mock ONVIF device for
//!   unit-testing client code without a camera (see the `mock` module).
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────┐
//! │                  OnvifSession                        │
//! │     caches service URLs — delegates every call       │
//! ├──────────────────────────────────────────────────────┤
//! │                   OnvifClient                        │
//! │     stateless — you supply service URLs per call     │
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
//! Two ways to use oxvif — pick whichever suits your workflow.
//!
//! ### `OnvifSession` — URL caching handled for you
//!
//! [`OnvifSession`] calls `GetCapabilities` once at construction and caches all
//! service URLs. No URL parameters needed for individual methods.
//!
//! ```no_run
//! use oxvif::{OnvifSession, OnvifError};
//!
//! async fn run() -> Result<(), OnvifError> {
//!     let session = OnvifSession::builder("http://192.168.1.100/onvif/device_service")
//!         .with_credentials("admin", "password")
//!         .with_clock_sync()  // syncs WS-Security timestamp with device clock
//!         .build()
//!         .await?;
//!
//!     let profiles = session.get_profiles().await?;
//!     let uri = session.get_stream_uri(&profiles[0].token).await?;
//!     println!("RTSP stream: {}", uri.uri);
//!
//!     let status = session.ptz_get_status(&profiles[0].token).await?;
//!     println!("Pan: {:?}  Tilt: {:?}", status.pan, status.tilt);
//!     Ok(())
//! }
//! ```
//!
//! ### `OnvifClient` — direct control, you manage service URLs
//!
//! [`OnvifClient`] is stateless and gives direct control over every call.
//! You fetch and forward service URLs yourself for full routing control.
//!
//! ```no_run
//! use oxvif::{OnvifClient, OnvifError};
//!
//! async fn run() -> Result<(), OnvifError> {
//!     let client = OnvifClient::new("http://192.168.1.100/onvif/device_service")
//!         .with_credentials("admin", "password");
//!
//!     let dt = client.get_system_date_and_time().await?;
//!     let client = client.with_utc_offset(dt.utc_offset_secs());
//!
//!     let caps = client.get_capabilities().await?;
//!     let media_url = caps.media.url.as_deref().unwrap();
//!
//!     let profiles = client.get_profiles(media_url).await?;
//!     let uri = client.get_stream_uri(media_url, &profiles[0].token).await?;
//!     println!("RTSP stream: {}", uri.uri);
//!     Ok(())
//! }
//! ```
//!
//! ## Testing without a real camera
//!
//! Enable the **`mock`** feature for a built-in, stateful mock ONVIF device and
//! drive an [`OnvifClient`] against it — no network, no real camera. The
//! **`mock-server`** feature additionally provides a bound-port `mock::MockServer`
//! for cross-process / non-Rust clients. See the `mock` module for details.
//!
//! ```ignore
//! // Cargo.toml:  oxvif = { version = "0.9", features = ["mock"] }
//! use std::sync::Arc;
//! use oxvif::{OnvifClient, mock::MockTransport};
//!
//! let client = OnvifClient::new("http://mock")
//!     .with_transport(Arc::new(MockTransport::new()));
//! // exercise client commands — Set persists, Get reflects it.
//! ```
//!
//! For full control you can instead implement [`transport::Transport`] yourself
//! to inject any fixture:
//!
//! ```no_run
//! use oxvif::transport::{Transport, TransportError};
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! struct FixtureTransport { xml: String }
//!
//! #[async_trait]
//! impl Transport for FixtureTransport {
//!     async fn soap_post(&self, _url: &str, _action: &str, _body: String)
//!         -> Result<String, TransportError>
//!     {
//!         Ok(self.xml.clone())
//!     }
//! }
//!
//! # async fn example() {
//! let client = oxvif::OnvifClient::new("http://ignored")
//!     .with_transport(Arc::new(FixtureTransport { xml: "<s:Envelope/>".into() }));
//! # }
//! ```
//!
//! [ONVIF]: https://www.onvif.org

pub mod client;
pub mod discovery;
pub mod error;
#[cfg(feature = "health")]
pub mod health;
#[cfg(feature = "mock")]
pub mod mock;
pub mod session;
pub mod soap;
pub mod transport;
pub mod types;

pub use client::{OnvifClient, notification_listener};
pub use discovery::{DiscoveredDevice, DiscoveryEvent, probe_unicast};
pub use error::OnvifError;
#[cfg(feature = "health")]
pub use health::{HealthCheck, HealthReport};
pub use session::{OnvifSession, OnvifSessionBuilder};
pub use types::{
    AnalyticsCapabilities, AudioDecoderConfiguration, AudioEncoderConfiguration,
    AudioEncoderConfigurationOptions, AudioEncoderOptions, AudioEncoding, AudioOutputConfiguration,
    AudioSource, AudioSourceConfiguration, BoundsRange, Capabilities, DeviceCapabilities,
    DeviceInfo, DeviceIoCapabilities, DnsInformation, EncoderInstanceInfo, EventProperties,
    EventsCapabilities, FindRecordingResults, FirmwareUpgradeStart, FloatRange, FocusMove,
    H264Configuration, H264Options, H265Configuration, H265Options, Hostname, ImagingCapabilities,
    ImagingMoveOptions, ImagingOptions, ImagingSettings, ImagingStatus, IntRange, IoCapabilities,
    JpegOptions, Media2Capabilities, MediaCapabilities, MediaProfile, MediaProfile2,
    MetadataConfiguration, MetadataConfigurationOptions, MulticastConfiguration,
    NetworkCapabilities, NetworkGateway, NetworkInterface, NetworkProtocol, NotificationMessage,
    NtpInfo, OnvifService, OsdColor, OsdConfiguration, OsdOptions, OsdPosition, OsdTextString,
    PtzCapabilities, PtzConfiguration, PtzConfigurationOptions, PtzNode, PtzPreset, PtzSpaceRange,
    PtzSpeed, PtzStatus, PullPointSubscription, PushSubscription, RecordingCapabilities,
    RecordingConfiguration, RecordingInformation, RecordingItem, RecordingJob,
    RecordingJobConfiguration, RecordingJobState, RecordingSourceInformation, RecordingTrack,
    RelayOutput, ReplayCapabilities, Resolution, SearchCapabilities, SecurityCapabilities,
    SetDateTimeRequest, SnapshotUri, SourceBounds, StorageConfiguration, StreamUri,
    StreamingCapabilities, SystemCapabilities, SystemDateTime, SystemLog, SystemRestoreStart,
    SystemUris, User, UtcDateTime, VideoEncoderConfiguration, VideoEncoderConfiguration2,
    VideoEncoderConfigurationOptions, VideoEncoderConfigurationOptions2, VideoEncoderInstances,
    VideoEncoderOptions2, VideoEncoding, VideoRateControl, VideoRateControl2, VideoSource,
    VideoSourceConfiguration, VideoSourceConfigurationOptions, VideoSourceMode,
};
