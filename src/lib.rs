//! # oxvif
//!
//! An async Rust client library for the [ONVIF] IP camera protocol.
//!
//! ONVIF (Open Network Video Interface Forum) is the industry standard for
//! interoperability between IP-based security products. This library covers
//! device discovery, capability negotiation, media profile management, and
//! RTSP stream URI retrieval over SOAP/HTTP(S).
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │               OnvifClient                   │  ← public API
//! ├─────────────────────────────────────────────┤
//! │  soap::SoapEnvelope  │  soap::WsSecurityToken│  ← SOAP layer
//! ├─────────────────────────────────────────────┤
//! │             Transport trait                 │  ← HTTP abstraction
//! │    (HttpTransport / mock in tests)          │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! ## Quick Start
//!
//! ```no_run
//! use oxvif::{OnvifClient, OnvifError};
//!
//! async fn run() -> Result<(), OnvifError> {
//!     let client = OnvifClient::new("http://192.168.1.100/onvif/device_service")
//!         .with_credentials("admin", "password");
//!
//!     // Discover service endpoints
//!     let caps = client.get_capabilities().await?;
//!     let media_url = caps.media.url.as_deref().unwrap();
//!
//!     // List media profiles and get the first RTSP stream URI
//!     let profiles = client.get_profiles(media_url).await?;
//!     let uri = client.get_stream_uri(media_url, &profiles[0].token).await?;
//!
//!     println!("RTSP stream: {}", uri.uri);
//!     Ok(())
//! }
//! ```
//!
//! [ONVIF]: https://www.onvif.org

pub mod client;
pub mod error;
pub mod soap;
pub mod transport;
pub mod types;

pub use client::OnvifClient;
pub use error::OnvifError;
pub use types::{
    AnalyticsCapabilities, BoundsRange, Capabilities, DeviceCapabilities, DeviceInfo,
    EncoderInstanceInfo, EventsCapabilities, FloatRange, H264Configuration, H264Options,
    H265Configuration, H265Options, IntRange, IoCapabilities, JpegOptions, MediaCapabilities,
    MediaProfile, MediaProfile2, NetworkCapabilities, OnvifService, PtzPreset, Resolution,
    SecurityCapabilities, SnapshotUri, SourceBounds, StreamUri, StreamingCapabilities,
    SystemCapabilities, SystemDateTime, VideoEncoderConfiguration, VideoEncoderConfiguration2,
    VideoEncoderConfigurationOptions, VideoEncoderConfigurationOptions2, VideoEncoderInstances,
    VideoEncoderOptions2, VideoEncoding, VideoRateControl, VideoRateControl2, VideoSource,
    VideoSourceConfiguration, VideoSourceConfigurationOptions,
};
