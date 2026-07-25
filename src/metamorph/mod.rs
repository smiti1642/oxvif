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
//! The fixture key is the canonical, ephemera-masked request
//! (`Masking::Key`), so `GetProfile(token=A)` and
//! `(token=B)` never collide while volatile fields (MessageID, nonce,
//! timestamps) don't fragment the key.
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
//!   diff is blind to (e.g. a non-integer where an int is expected). The two are
//!   complementary and share the `(action, key_canon)` key so a UI can join them:
//!   the parse verdict as the badge, the SOAP diff as the drill-down evidence.
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
pub use fixture::{Fixture, FixtureStore};
pub use parse::{ParseReport, ParseStatus, ParseVerdict};
pub use quirk::{ChangedQuirk, OperationDiff, OperationQuirk, QuirkDiff, QuirkReport};
pub use record::{RecordingTransport, record_standard_surface, record_surface};
pub use replay::{MetamorphTransport, ReplayResponder};
pub use surface::{
    OpOutcome, SurfaceGroup, SurfaceOp, SurfaceSelection, SweepReport, drive_standard_surface,
    drive_surface,
};
