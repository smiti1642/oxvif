//! Persona B — record a real camera and replay it verbatim (metamorph M2).
//!
//! Built on the public mock responder [`Chain`](crate::mock::Chain): a
//! [`ReplayResponder`] answers reads from a recorded [`FixtureStore`] and falls
//! through to synthetic `DeviceState` for writes and unrecorded operations
//! (coarse copy-on-write, so `Set → Get` round-trips).
//!
//! Two halves:
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
//! Gated on the `metamorph` feature (a superset of `mock`).

mod fixture;
mod record;
mod replay;

pub use fixture::{Fixture, FixtureStore};
pub use record::RecordingTransport;
pub use replay::{MetamorphTransport, ReplayResponder};
