//! In-process / bound-port mock ONVIF device for testing client code without a
//! real camera.
//!
//! Every vendor's ONVIF differs and depending on a physical IP camera in unit
//! tests is painful. This module answers SOAP requests for **every operation
//! oxvif implements**; most are stateful (a `Set` persists and the matching
//! `Get` reflects it), a handful are declared static stubs, and one refuses
//! outright. Which is which is not a matter of reading the source — see
//! *Strictness* below.
//!
//! The outward-facing reference is `docs/mock-server.md` in the repository
//! (not shipped in the published package): routing, the namespace contract, the
//! full state model, the seeded fixture, all 157 operations marked
//! state-backed or static, worked request/response pairs, and the fault
//! catalogue.
//!
//! Two entry points, behind features:
//!
//! - **`mock`** → [`MockTransport`]: an in-process [`Transport`](crate::transport::Transport).
//!   No sockets, no axum — the fast path for unit tests.
//!
//!   ```no_run
//!   use std::sync::Arc;
//!   use oxvif::{OnvifClient, mock::MockTransport};
//!   # async fn run() -> Result<(), oxvif::OnvifError> {
//!   let client = OnvifClient::new("http://mock")
//!       .with_transport(Arc::new(MockTransport::new()));
//!   let profiles = client.get_profiles("http://mock/media").await?;
//!   # Ok(()) }
//!   ```
//!
//! - **`mock-server`** → [`MockServer`]: a real HTTP server bound to an
//!   ephemeral port (pulls `axum`), for when you need an actual endpoint.
//!
//!   ```ignore
//!   let server = oxvif::mock::MockServer::start().await?;
//!   let client = oxvif::OnvifClient::new(server.device_url());
//!   ```
//!
//! State is in-memory; the library never writes to disk. Opt into persistence
//! via [`MockState::set_on_change`].
//!
//! # Strictness
//!
//! 0.15 made this mock noticeably harder to satisfy than 0.14, on purpose. A
//! mock that answers everything is not a test harness — it is a way of proving
//! your client compiles. Each rule below exists because its absence let a real
//! defect through, and each is held by a standing test rather than by care.
//!
//! **1. A per-channel operation needs its token, and a wrong token is refused.**
//! Operations addressing a specific head, sensor or configuration
//! (`ProfileToken` for PTZ, `VideoSourceToken` for Imaging, `ConfigurationToken`
//! for the Media options getters) fault on a missing token and fault again on
//! one that names nothing. They no longer fall back to a default channel.
//!
//! This is the harshest change and the one most likely to break existing tests
//! — and it is the whole point. A device that silently answers for channel 0 is
//! indistinguishable from a correct one *until* you point your code at a
//! dual-lens camera, which is exactly when it is expensive to find out.
//! `tests/mock_token_discrimination.rs` pins it: every token-taking operation
//! declares `Discriminates` or `Blind`, with two tokens the seeded fixture
//! deliberately disagrees about.
//!
//! **2. A write either persists or says it cannot.** `Set` handlers do not
//! return an empty success while discarding the body. Where the mock genuinely
//! cannot model an operation — currently only Media2 `SetVideoSourceMode` — it
//! returns a fault (`ter:ActionNotSupported`) rather than a success no getter
//! could contradict. `tests/mock_roundtrip.rs` pins every `Set` to the getter
//! that should show it, and a row must declare `Works`, `Broken` — a real defect
//! with an audit citation — or `Static`, a deliberate stub. Wiring a `Broken` or
//! `Static` row up turns the test red so the declaration cannot rot. As of
//! 0.15.0 all 49 rows are `Works`.
//!
//! **3. Responses are namespace-well-formed.** Every element prefix the mock
//! emits is declared, and no start-tag repeats an attribute. Neither held before
//! 0.15: roughly a third of responses used an unbound prefix. Nothing here
//! noticed, because `find_response` matches on local name and quick-xml enforces
//! neither rule — but a conforming external client rejects such a document
//! outright. Guarded across all 157 actions by
//! `every_response_binds_the_prefixes_it_uses` and
//! `no_response_declares_an_attribute_twice`.
//!
//! **4. Clocks are real clocks.** `GetSystemDateAndTime` and PTZ `GetStatus`
//! both report the current time. Two hardcoded dates shipped before this was a
//! rule, each drifting a day further into the past per day, and each invisible
//! because a frozen ISO-8601 string never stops being valid.
//!
//! **5. Media1 and Media2 are one device.** An operation both services expose
//! reads and writes the same state, so they cannot report contradictory facts.
//! `tests/mock_media1_media2_agree.rs`.
//!
//! They are not, however, the same *schema*. Where the two ONVIF types genuinely
//! differ, the mock differs with them: `tt:AudioEncoderConfiguration` sequences
//! `Multicast` and `SessionTimeout` after `SampleRate` and requires both, while
//! `tt:AudioEncoder2Configuration` puts `Multicast` before `Bitrate` and has no
//! `SessionTimeout` member at all. One catalogue, two renderings — and a Media2
//! write leaves the stored `SessionTimeout` alone, because it has no way to say
//! anything about it.
//!
//! **6. A request the schema would reject is refused.** Media1
//! `SetAudioEncoderConfiguration` faults on a body missing `Multicast` or
//! `SessionTimeout` (`ter:ConfigModify`), because a validating device would.
//! oxvif itself sent that body until 0.15; accepting it here would have made the
//! mock the one device on which the bug did not show.
//!
//! What is *not* strict is written down rather than left to discovery:
//! `docs/mock-server.md` §13 lists every declared stub, fidelity gap and
//! deliberate simplification. If an operation is not on that list and does not
//! behave, it is a bug worth reporting.

mod auth;
pub(crate) mod canon;
pub(crate) mod dispatch;
pub(crate) mod fault_injection;
pub(crate) mod helpers;
pub(crate) mod responder;
mod services;
mod transport;
mod xml_parse;

pub mod state;

#[cfg(feature = "mock-server")]
mod discovery_responder;
#[cfg(feature = "mock-server")]
mod fleet;
#[cfg(feature = "mock-server")]
mod font;
#[cfg(feature = "mock-server")]
mod server;
#[cfg(feature = "mock-server")]
mod snapshot;

pub use responder::{Chain, RequestCtx, Responder};
pub use state::{DeviceState, MockState};
pub use transport::MockTransport;

#[cfg(feature = "mock-server")]
pub use discovery_responder::DiscoveryResponder;
#[cfg(feature = "mock-server")]
pub use fleet::{Fleet, FleetBuilder};
#[cfg(feature = "mock-server")]
pub use server::MockServer;
