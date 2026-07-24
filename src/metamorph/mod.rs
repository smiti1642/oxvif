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
//! ([`Masking::Key`](crate::mock::canon::Masking)), so `GetProfile(token=A)` and
//! `(token=B)` never collide while volatile fields (MessageID, nonce,
//! timestamps) don't fragment the key.
//!
//! ## Serving the clone + finding quirks
//!
//! - **Container**: with the `metamorph-server` feature, serve a clone from a
//!   real bound port via
//!   [`MockServerBuilder::replay`](crate::mock::MockServer::builder) — any HTTP
//!   ONVIF client (oxdm, Frigate, ODM) can then drive the cloned camera.
//! - **Quirk diff**: [`FixtureStore::diff_against_synthetic`] compares the clone
//!   against oxvif's synthetic (spec-ideal) mock, per operation, reporting where
//!   the response *shape* deviates ([`QuirkReport`]). The diff is **structural
//!   only** — which element paths exist, not their values; a different
//!   `Manufacturer` string is expected, not a quirk. Value / type-level drift is
//!   the deeper, still-unbuilt half of M7.
//!
//! Gated on the `metamorph` feature (a superset of `mock`).

mod adapter;
mod fixture;
mod quirk;
mod record;
mod replay;

pub use adapter::{
    AdapterResponder, AdapterResult, AdapterTransport, DeviceAdapter, DeviceIdentity, PtzVector,
};
pub use fixture::{Fixture, FixtureStore};
pub use quirk::{OperationQuirk, QuirkReport};
pub use record::RecordingTransport;
pub use replay::{MetamorphTransport, ReplayResponder};
