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
//! | **Profile T** | Advanced streaming (H.265, focus, OSD, audio) | ~95% | HTTP Digest Auth, Media2 audio/metadata/analytics config, PTZ compat + preset tours, per-service capabilities; Analytics rules not yet implemented, and DeviceIO only for `get_digital_inputs` |
//! | **Profile G** | Recording & playback | ~85% | Read/search/replay + full recording/job write management; live-source job binding not yet implemented |
//!
//! ## Supported services
//!
//! - **Device** — capabilities, scopes, device info, hostname, NTP, reboot,
//!   user management, network interfaces/protocols/DNS/gateway, relay outputs,
//!   storage configurations, system log/URIs, factory default, discovery mode,
//!   firmware upgrade / system restore (upload-URI flow),
//!   auxiliary commands (wiper/IR lamp)
//! - **DeviceIO** — digital inputs, and nothing else yet. A separate endpoint
//!   from device management: pass `capabilities().device_io.url` to
//!   `get_digital_inputs`, or let `OnvifSession` resolve it. The relay-output
//!   operations stay under **Device** above, because `deviceio.wsdl` types
//!   those messages with the device service's own elements
//! - **Media1 / Media2** — profiles, RTSP/snapshot URIs, video + audio config, OSD,
//!   metadata config, audio decoder/output config, video source modes,
//!   unified AddConfiguration/RemoveConfiguration
//! - **PTZ** — absolute/relative/continuous move, presets, home position, status,
//!   configurations, nodes, compatible configurations, preset tours,
//!   per-profile auxiliary commands
//! - **Imaging** — brightness/contrast/exposure settings, focus move/stop/status
//! - **Events** — pull-point subscriptions, event polling, renew, unsubscribe,
//!   continuous `event_stream`, synchronization point
//! - **Recording** — list stored recordings; create/delete recordings, tracks, and recording jobs
//! - **Search** — find recordings by scope, collect results, end search
//! - **Replay** — get RTSP playback URI for a stored recording
//! - **WS-Discovery** — UDP multicast probe to find cameras on the local network
//!
//! Nine of those services also answer their own `GetServiceCapabilities` — see
//! [Per-service capabilities](#per-service-capabilities). DeviceIO is not one
//! of them: only `GetDigitalInputs` is implemented there. WS-Discovery is a UDP
//! protocol, not a SOAP service, and has none.
//!
//! ## Per-service capabilities
//!
//! [`OnvifClient::get_capabilities`] answers *which services exist and at what
//! URL*. Each service separately answers *what it can do*, and all nine are
//! implemented: [`DeviceServiceCapabilities`], [`MediaServiceCapabilities`],
//! [`Media2ServiceCapabilities`], [`PtzServiceCapabilities`],
//! [`ImagingServiceCapabilities`], [`EventsServiceCapabilities`],
//! [`RecordingServiceCapabilities`], [`SearchServiceCapabilities`] and
//! [`ReplayServiceCapabilities`].
//!
//! ```no_run
//! # use oxvif::{OnvifClient, OnvifError};
//! # async fn run() -> Result<(), OnvifError> {
//! let client = OnvifClient::new("http://192.168.1.1/onvif/device_service");
//! let caps = client.device_get_service_capabilities().await?;
//!
//! assert_eq!(caps.security.tls1_2, Some(true));
//! // What `ptz_send_auxiliary_command` will accept on this camera:
//! let commands = caps.misc.map(|m| m.auxiliary_commands).unwrap_or_default();
//! # Ok(()) }
//! ```
//!
//! **Every flag on these types is `Option<bool>`, not `bool`.** `None` means
//! the device did not mention the attribute; `Some(false)` means it said no.
//! Those are different answers, and collapsing them would defeat the reason for
//! asking. [`Capabilities`], from the device-level `GetCapabilities`, uses bare
//! `bool` and cannot make that distinction — which is why the two families are
//! separate types rather than one shared set.
//!
//! List-valued attributes are the deliberate exception: `Vec<_>`, empty when
//! absent, because for a list "absent" and "present but empty" both mean *no
//! items*.
//!
//! ## Optional features
//!
//! - **`health`** — `health::HealthCheck`, a fast read-only conformance check
//!   that returns a `health::HealthReport` (per-check Pass/Warn/Fail/Skip plus
//!   a Profile S/T/G assessment). Pure library code over [`OnvifSession`].
//!   Reports serialise to JSON (`to_json` / `to_json_pretty`) and a
//!   later report can diff against an earlier one via
//!   `HealthReport::diff` — see
//!   `examples/healthcheck.rs --baseline <file.json>`.
//! - **`mock`** / **`mock-server`** — a built-in mock ONVIF device for
//!   unit-testing client code without a camera (see the `mock` module).
//!   The `mock` feature also exposes `fixtures::CapturingTransport` /
//!   `fixtures::FixtureTransport` for recording real-camera exchanges
//!   into `tests/fixtures/<vendor>-<model>/` and replaying them in tests
//!   without the device (see `examples/record_fixtures.rs`). Captures are
//!   redacted as they are written — the WS-Security `Password` / `Nonce` and
//!   any `user:pass@` in a returned URL — so a fixture directory is safe to
//!   commit.
//! - **`metamorph`** / **`metamorph-server`** — clone a real camera into a
//!   replayable fixture set, ask whether oxvif can parse a given device, and
//!   diff a device's response shapes against oxvif's own reference mock. See
//!   [Metamorph](#metamorph) below and the `metamorph` module.
//! - **`serde`** — `Serialize` / `Deserialize` on every public response type in
//!   [`types`], plus the WS-Discovery result types, so a result can go straight
//!   to a REST layer or on disk without a hand-cloned parallel struct. Pulls no
//!   new dependency and costs nothing unless enabled.
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
//! // Cargo.toml:  oxvif = { version = "0.15", features = ["mock"] }
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
//! ## Metamorph
//!
//! The **`metamorph`** feature turns that mock into a shape-shifter, for the
//! work you cannot do against a synthetic device: answering *what does this
//! particular camera actually send, and can oxvif read it?*
//!
//! - **Clone and replay** — `metamorph::record_surface` drives a chosen set of
//!   read operations against a real camera once and returns a
//!   `metamorph::FixtureStore`: the recorded exchanges, keyed by
//!   `(SOAP action, canonical request)` so two services that canonicalise
//!   identically stay distinct. Save it, replay it in-process, or serve it from
//!   a real bound port with **`metamorph-server`** — the camera can then be
//!   unplugged. Credentials are redacted as it records.
//! - **Pick what to clone** — `metamorph::SurfaceSelection` selects whole
//!   service zones (`SurfaceGroup`) or individual operations (`SurfaceOp`).
//!   Prerequisites are expanded for you, so selecting `GetStreamUri` still
//!   yields a replayable clone. Every sweep returns a `metamorph::SweepReport`
//!   saying per operation whether it was recorded, failed, or skipped — and
//!   *why* it was skipped, which separates "this device has no such path" from
//!   "the command broke".
//! - **Will oxvif parse this device?** — `FixtureStore::verify_parsing` runs
//!   oxvif's own typed parser over each recorded response and returns a
//!   `metamorph::ParseReport`. This catches value and type quirks that a
//!   structural diff cannot see. A device that answers with a SOAP `Fault` is
//!   reported as declined rather than unparseable — it is behaving correctly.
//! - **What is unusual about it?** — `FixtureStore::diff_against_synthetic`
//!   compares each recorded response against oxvif's reference mock and returns
//!   a `metamorph::QuirkReport`. `QuirkReport::diff` compares that against an
//!   earlier saved report, so you can answer "did this firmware update change
//!   the device?" without reading two reports side by side.
//! - **Put an ONVIF skin on something else** — implement
//!   `metamorph::DeviceAdapter` to answer ONVIF operations from a non-ONVIF
//!   source.
//!
//! The four long operations have `*_with_progress` twins that report
//! determinate progress, so a UI can show a real bar rather than freezing for
//! the length of a 52-operation sweep.
//!
//! ## Health & conformance checks
//!
//! Enable the **`health`** feature for `health::HealthCheck` — a fast,
//! read-only conformance check that reports Pass/Warn/Fail/Skip per ONVIF
//! service plus a Profile S/T/G verdict. It includes a **parse-coverage**
//! dimension that flags when a parser silently drops list data (a wrong element
//! name returning an empty result with no error). To validate the parsers
//! against real hardware, the `conformance` example dumps each device's raw
//! responses next to the parsed summary so silent-parse gaps stand out.
//!
//! Since 0.15 it also asks every advertised service its own
//! `GetServiceCapabilities`, and cross-checks the twenty-four facts a device
//! states **twice** — once in the device-level `GetCapabilities`, again in a
//! service's. Only `GetCapabilities` saying yes where the service says no is
//! reported: the device-level type uses bare `bool` and cannot tell "said no"
//! from "did not say", so the other direction is counted rather than warned
//! about. See the `README` for the table.
//!
//! [ONVIF]: https://www.onvif.org

pub mod client;
pub mod discovery;
pub mod error;
#[cfg(feature = "mock")]
pub mod fixtures;
#[cfg(feature = "health")]
pub mod health;
#[cfg(feature = "metamorph")]
pub mod metamorph;
#[cfg(feature = "mock")]
pub mod mock;
#[cfg(any(feature = "mock", feature = "health"))]
pub(crate) mod redact;
pub mod session;
pub mod soap;
pub mod transport;
// `types` is the module API users read most — every response struct they get
// back lands here — so its public surface is held to full documentation. A
// warning, not a deny: it should block a lazy field, not a build.
#[warn(missing_docs)]
pub mod types;

/// Helpers shared by the `#[path]`-attached unit-test modules.
#[cfg(test)]
mod tests;

pub use client::{OnvifClient, notification_listener};
pub use discovery::{
    DiscoveredDevice, DiscoveryEvent, DiscoveryInterface, discovery_interfaces, probe_result,
    probe_result_on, probe_unicast,
};
pub use error::OnvifError;
#[cfg(feature = "mock")]
pub use fixtures::{CapturingTransport, FixtureTransport};
#[cfg(feature = "health")]
pub use health::{CapturedExchange, HealthCheck, HealthReport};
#[cfg(feature = "metamorph")]
pub use metamorph::{
    AdapterResponder, AdapterResult, AdapterTransport, DeviceAdapter, DeviceIdentity, Fixture,
    FixtureStore, MetamorphTransport, PtzVector, RecordingTransport, ReplayResponder,
};
pub use session::{OnvifSession, OnvifSessionBuilder};
pub use types::{
    AnalyticsCapabilities, AudioDecoderConfiguration, AudioEncoderConfiguration,
    AudioEncoderConfigurationOptions, AudioEncoderOptions, AudioEncoding, AudioOutputConfiguration,
    AudioSource, AudioSourceConfiguration, BoundsRange, Capabilities, DeviceCapabilities,
    DeviceInfo, DeviceIoCapabilities, DeviceMiscCapabilities, DeviceNetworkCapabilities,
    DeviceSecurityCapabilities, DeviceServiceCapabilities, DeviceSystemCapabilities, DigitalInput,
    DnsInformation, EncoderInstanceInfo, EventProperties, EventsCapabilities,
    EventsServiceCapabilities, FindRecordingResults, FirmwareUpgradeStart, FloatRange, FocusMove,
    H264Configuration, H264Options, H265Configuration, H265Options, Hostname, ImagingCapabilities,
    ImagingMoveOptions, ImagingOptions, ImagingServiceCapabilities, ImagingSettings, ImagingStatus,
    IntRange, IoCapabilities, IpStackConfig, JpegOptions, ManualAddress, Media2Capabilities,
    Media2ProfileCapabilities, Media2ServiceCapabilities, Media2StreamingCapabilities,
    MediaCapabilities, MediaProfile, MediaProfile2, MediaProfileCapabilities,
    MediaServiceCapabilities, MediaStreamingCapabilities, MetadataConfiguration,
    MetadataConfigurationOptions, MulticastConfiguration, NetworkCapabilities, NetworkGateway,
    NetworkInterface, NetworkInterfaceConfig, NetworkProtocol, NotificationMessage, NtpInfo,
    OnvifService, OsdColor, OsdConfiguration, OsdOptions, OsdPosition, OsdTextString,
    PtzCapabilities, PtzConfiguration, PtzConfigurationOptions, PtzNode, PtzPreset, PtzPresetTour,
    PtzPresetTourDirection, PtzPresetTourOperation, PtzPresetTourOptions,
    PtzPresetTourPresetDetail, PtzPresetTourPresetDetailOptions, PtzPresetTourSpot,
    PtzPresetTourSpotOptions, PtzPresetTourStartingCondition,
    PtzPresetTourStartingConditionOptions, PtzPresetTourState, PtzPresetTourStatus,
    PtzServiceCapabilities, PtzSpaceRange, PtzSpeed, PtzStatus, PullPointSubscription,
    PushSubscription, RecordingCapabilities, RecordingConfiguration, RecordingInformation,
    RecordingItem, RecordingJob, RecordingJobConfiguration, RecordingJobState,
    RecordingServiceCapabilities, RecordingSourceInformation, RecordingTrack, RelayOutput,
    ReplayCapabilities, ReplayServiceCapabilities, Resolution, SearchCapabilities,
    SearchServiceCapabilities, SecurityCapabilities, SetDateTimeRequest, SnapshotUri, SourceBounds,
    StorageConfiguration, StreamUri, StreamingCapabilities, SystemCapabilities, SystemDateTime,
    SystemLog, SystemRestoreStart, SystemUris, User, UtcDateTime, VideoEncoderConfiguration,
    VideoEncoderConfiguration2, VideoEncoderConfigurationOptions,
    VideoEncoderConfigurationOptions2, VideoEncoderInstances, VideoEncoderOptions2, VideoEncoding,
    VideoRateControl, VideoRateControl2, VideoSource, VideoSourceConfiguration,
    VideoSourceConfigurationOptions, VideoSourceMode,
};
