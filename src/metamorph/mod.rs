//! Clone a real camera and replay it offline, or put an ONVIF skin on a device
//! that does not speak ONVIF.
//!
//! # Borrow a camera once, keep it forever
//!
//! ```no_run
//! use std::sync::Arc;
//! use oxvif::OnvifClient;
//! use oxvif::metamorph::{FixtureStore, MetamorphTransport, record_standard_surface};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! // The only step that needs the camera.
//! let clone = record_standard_surface(
//!     "http://192.168.1.100/onvif/device_service",
//!     Some(("admin", "password")),   // None for an open device
//!     "hikvision-ds2cd",
//! ).await?;
//! clone.save("clones/hikvision-ds2cd")?;
//!
//! // Later, anywhere, with no camera on the network:
//! let store  = FixtureStore::load("clones/hikvision-ds2cd")?;
//! let client = OnvifClient::new("http://replay")
//!     .with_transport(Arc::new(MetamorphTransport::new(store)));
//!
//! let info = client.get_device_info().await?;   // the real camera's answer
//! # Ok(()) }
//! ```
//!
//! A saved clone carries **no secrets**: WS-Security `Password`/`Nonce` and any
//! `user:pass@` in a URL are scrubbed before anything reaches disk.
//!
//! # Will oxvif parse this device correctly?
//!
//! ```no_run
//! # use oxvif::metamorph::FixtureStore;
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let store = FixtureStore::load("clones/hikvision-ds2cd")?;
//!
//! // Value/type level: run oxvif's own parsers over every recorded response.
//! for v in store.verify_parsing().await.failures() {
//!     println!("cannot parse {}: {}", v.action, v.error.as_deref().unwrap_or(""));
//! }
//!
//! // Shape level: how the device's element paths differ from the reference mock.
//! for q in &store.diff_against_synthetic().quirks {
//!     println!("{}: +{:?} -{:?}", q.action, q.only_in_clone, q.only_in_synthetic);
//! }
//! # Ok(()) }
//! ```
//!
//! `failures()` excludes operations the device *declined* with a SOAP Fault —
//! that is correct device behaviour, not an oxvif problem. Use `faulted()` for
//! those; it matters when sweeping with a restricted account.
//!
//! # Make a non-ONVIF device look like ONVIF
//!
//! Only two methods are required — enough for an NVR or Frigate to ingest an
//! RTSP-only camera as ONVIF. Everything else falls through to the synthetic mock.
//!
//! ```no_run
//! use std::sync::Arc;
//! use oxvif::OnvifClient;
//! use oxvif::metamorph::{AdapterTransport, DeviceAdapter, DeviceIdentity};
//!
//! struct RtspCam { rtsp: String }
//!
//! #[async_trait::async_trait]
//! impl DeviceAdapter for RtspCam {
//!     fn identity(&self) -> DeviceIdentity {
//!         DeviceIdentity {
//!             manufacturer: "Acme".into(),
//!             model: "RTSP-Skin".into(),
//!             firmware_version: "1.0".into(),
//!             serial_number: "SN-0001".into(),
//!             hardware_id: "HW-0001".into(),
//!         }
//!     }
//!     fn stream_uri(&self, _profile: &str) -> Option<String> {
//!         Some(self.rtsp.clone())
//!     }
//! }
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let adapter = Arc::new(RtspCam { rtsp: "rtsp://192.168.1.77:554/ch1".into() });
//! let client  = OnvifClient::new("http://adapter")
//!     .with_transport(Arc::new(AdapterTransport::new(adapter)));
//!
//! let info = client.get_device_info().await?;   // identity from the adapter
//! # Ok(()) }
//! ```
//!
//! Runnable versions of all three: `examples/metamorph_record.rs`,
//! `metamorph_serve.rs`, `metamorph_adapter.rs`.
//!
//! # How it works
//!
//! Metamorph personas built on the public mock responder
//! [`Chain`](crate::mock::Chain) — each slots a responder ahead of the synthetic
//! terminal, so anything a persona doesn't answer falls through to the mock:
//!
//! - **Persona B — replay / clone (M2)**: [`ReplayResponder`] answers reads from
//!   a recorded [`FixtureStore`]; writes fall through to synthetic `DeviceState`
//!   and invalidate that operation family (coarse copy-on-write, so
//!   `Set → Get` round-trips). Driven by [`MetamorphTransport`].
//! - **Persona C — adapter / skin (M5)**: [`AdapterResponder`] answers from a
//!   [`DeviceAdapter`] you implement for a non-ONVIF device; unimplemented
//!   operations fall through to synthetic. Driven by [`AdapterTransport`].
//!
//! Persona B's two halves:
//!
//! - **Record**: wrap a live transport in [`RecordingTransport`], drive a normal
//!   `OnvifSession` against the camera, then [`FixtureStore::save`] the set. See
//!   `examples/metamorph_record.rs`.
//! - **Replay**: [`FixtureStore::load`] a set into a [`MetamorphTransport`] and
//!   point an `OnvifClient` at it — no camera required.
//!
//! The fixture key is the SOAP action paired with the canonical,
//! ephemera-masked request (`Masking::Key`), so `GetProfile(token=A)` and
//! `(token=B)` never collide while volatile fields (MessageID, nonce,
//! timestamps) don't fragment the key. The action is part of the key because
//! the canonical request alone does not separate services: Media1's
//! `<trt:GetProfiles/>` and Media2's `<tr2:GetProfiles/>` canonicalise to the
//! same string once prefixes and the endpoint URL are removed.
//!
//! ## Serving the clone + finding quirks
//!
//! - **Container**: with the `metamorph-server` feature, serve a clone from a
//!   real bound port via
//!   [`MockServerBuilder::replay`](crate::mock::MockServer::builder) — any HTTP
//!   ONVIF client (oxdm, Frigate, ODM) can then drive the cloned camera.
//! - **Structural quirk diff**: [`FixtureStore::diff_against_synthetic`] compares
//!   the clone against oxvif's synthetic reference mock (its own well-formed
//!   response, not the ONVIF WSDL/XSD), per operation, reporting where the
//!   response *shape* deviates ([`QuirkReport`]). Structural **only** — which
//!   element paths exist, not their values; a different `Manufacturer` string is
//!   expected, not a quirk. [`FixtureStore::diff_details`] renders the same
//!   comparison as aligned XML for a git-style side-by-side view.
//! - **Parse verification**: [`FixtureStore::verify_parsing`] runs oxvif's own
//!   typed parser over each recorded response and reports whether it parses plus
//!   the value extracted ([`ParseReport`]) — the value / type-level half. It
//!   answers "will oxvif choke on this device", catching quirks the structural
//!   diff is blind to (e.g. a non-integer where an int is expected). A response
//!   the device *declined* with a SOAP Fault is classified
//!   [`ParseStatus::Faulted`], kept out of [`ParseReport::failures`] — a
//!   restricted account gets "the device said no", not "oxvif cannot parse this".
//!   The two reports are complementary and share the `(action, key_canon)` key so
//!   a UI can join them: the parse verdict as the badge, the SOAP diff as the
//!   drill-down evidence.
//! - **Progress**: each long pass has a `*_with_progress` twin taking an
//!   `Fn(..) + Send + Sync` callback — [`drive_surface_with_progress`],
//!   [`record_surface_with_progress`], [`FixtureStore::verify_parsing_with_progress`]
//!   and [`FixtureStore::diff_against_synthetic_with_progress`] — so a UI can
//!   drive a determinate progress bar instead of one opaque await. The sweep's
//!   unit is a [`SurfaceOp`] ([`SweepProgress`]); the two fixture passes count
//!   recorded exchanges ([`FixtureProgress`]). The plain forms delegate to these
//!   with a no-op callback.
//!
//! Gated on the `metamorph` feature (a superset of `mock`).

mod adapter;
mod fixture;
mod parse;
mod quirk;
mod record;
mod replay;
mod surface;

pub use adapter::{
    AdapterResponder, AdapterResult, AdapterTransport, DeviceAdapter, DeviceIdentity, PtzVector,
    soap_body,
};
pub use fixture::{Fixture, FixtureProgress, FixtureStore};
pub use parse::{ParseReport, ParseStatus, ParseVerdict};
pub use quirk::{ChangedQuirk, OperationDiff, OperationQuirk, QuirkDiff, QuirkReport};
pub use record::{
    RecordingTransport, record_standard_surface, record_surface, record_surface_with_progress,
};
pub use replay::{MetamorphTransport, ReplayResponder};
pub use surface::{
    OpOutcome, SurfaceGroup, SurfaceOp, SurfaceSelection, SweepProgress, SweepReport,
    drive_standard_surface, drive_surface, drive_surface_with_progress,
};
