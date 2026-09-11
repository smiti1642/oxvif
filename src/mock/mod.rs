//! In-process / bound-port mock ONVIF device for testing client code without a
//! real camera.
//!
//! Every vendor's ONVIF differs and depending on a physical IP camera in unit
//! tests is painful. This module answers SOAP requests for **every operation
//! oxvif implements**; most are stateful (a `Set` persists and the matching
//! `Get` reflects it), others are static reads, effectful stubs or explicit
//! refusals. Which is which is not a matter of reading the source — see
//! *Strictness* below.
//!
//! The outward-facing reference is `docs/mock-server.md` in the repository
//! (not shipped in the published package): routing, the namespace contract, the
//! full state model, the seeded fixture, all 157 operations marked
//! state-backed, static or refused, worked request/response pairs, and the fault
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
//! Hooks receive mutation snapshots after the state lock is released. Reentrant
//! writes must be bounded by the callback; concurrent callback order is not guaranteed.
//!
//! # Strictness
//!
//! Synthetic routing matches complete Action identities, including the Events
//! port segment; wrong hosts and inserted path segments are not aliases. Requests
//! reaching synthetic dispatch share bounded XML parsing, namespace/operation
//! identity checks and SOAP container checks, including static reads. Full field
//! validation and HTTP binding are not complete. Explicit fault/replay/custom
//! responders retain their precedence.
//! Media2 `GetProfiles` honors its decoded `Token` and `Type` selectors without
//! changing stored bindings. Omitting `Type` returns no configuration details;
//! the full-profile client method explicitly requests `Type=All`.
//! Media profile reads capture the profile list and all associated catalogues
//! under one read lock; separate requests remain independent snapshots.
//! Profile creation requires one direct scalar Name, stores decoded text and
//! escapes it once in profile responses; empty names and whitespace are retained.
//! Shared escaping uses character references for CR/LF/tab data so XML
//! normalization does not replace them; the compatibility client DOM still trims
//! leading/trailing text. Structured Fault output uses the same escaping helper.
//! Media1 CreateProfile/GetProfile and all six existing Media binding entry
//! points use scoped decoded profile tokens; both profile views escape the
//! token attribute once. Persisted tokens remain literal strings, never
//! automatically decoded again. Explicit empty Create tokens/read selectors
//! return Sender / `mock:RequestPolicy` before mutation; omit a Create token to
//! allocate one. Unfiltered profile reads containing an empty seeded token return
//! Receiver / `mock:RequestPolicy`, without repairing the snapshot. Valid profiles
//! remain individually readable. This is a mock limit, not a universal ONVIF
//! string restriction. Other configuration text, typed adapters and recorded-key
//! migration remain under review.
//! The 19 PTZ handlers using profile/head resolution now read one direct,
//! namespace-qualified ProfileToken from the shared parsed operation, preserving
//! decoded whitespace and rejecting duplicate or nested scalar values. Other PTZ
//! fields, fault policies and adapter token paths are not fully migrated.
//! Source configuration uses a complete scoped candidate and one atomic commit.
//! Only zero-origin crop storage is modeled; unsupported settings and malformed
//! values refuse without hooks or replay invalidation. Oversized positive crops
//! are clamped to the selected sensor and exposed through both service views.
//! Source options follow physical sensor dimensions, not the mutable crop.
//! Shared encoder rates use `f32`; present rate blocks validate before mutation.
//! Media1 views with fractional rates return an explicit model-policy Fault rather
//! than rounding. Invalid seeded rates also refuse rendering; persistence rejects
//! negative/nonfinite rates. Other encoder fields/options remain under review.
//!
//! 0.15 made this mock noticeably harder to satisfy than 0.14, on purpose. A
//! mock that answers everything is not a test harness — it is a way of proving
//! your client compiles. Each rule below exists because its absence let a real
//! defect through, and each is held by a standing test rather than by care.
//!
//! **1. A per-channel operation needs its token, and a wrong token is refused.**
//! Operations addressing a specific head, sensor or configuration
//! (`ProfileToken` for PTZ, `VideoSourceToken` for Imaging, `ConfigurationToken`
//! for the remaining Media encoder/audio options getters) fault on a missing
//! token and fault again on one that names nothing. Source options now support
//! omitted selectors explicitly as conservative generic ranges, not a default
//! channel; explicit unknown source configuration/profile references still fault.
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
//! cannot model an operation or requested setting, it returns a fault rather
//! than a success no getter could contradict; classified receipt-only operations
//! require explicit opt-in. `tests/mock_roundtrip.rs` pins its selected `Set` rows to the getter
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
//!
//! # Unreleased request hardening
//!
//! Eleven classified reset, auxiliary, reboot/maintenance, subscription and
//! search-ending stubs refuse by default with Receiver / `mock:UnmodeledEffect`,
//! including Events SetSynchronizationPoint. Select individual
//! [`AckOnlyOperation`] values using `with_acknowledgment_only` on mock, replay
//! or adapter transports, or the HTTP builder, for receipt-only workflow tests.
//! Neither path changes state, invokes hooks or invalidates replay; reset,
//! auxiliary execution, maintenance, subscription/search lifetimes and events
//! are not modeled. Upload URIs, references and timestamps are fixtures. Full
//! operation contracts and capability reconciliation remain under review.
//!
//! Media1/Media2 `DeleteProfile` now select a namespace-qualified direct token
//! child, decode XML text once and preserve token whitespace. Invalid requests
//! are rejected before mutation. This parser is bounded to 2 MiB, 64 element
//! levels and 16,384 elements; other operations still use legacy extraction.
//! Missing/fixed-profile refusals use nested Sender faults; the public error's
//! subcode is still the first level, not the deepest condition. Refused deletion
//! does not invoke the change hook; successful deletion invokes it once. Built-in
//! replay clones retire both services' profile views and dependent configuration
//! reads (including reference counts) only after successful
//! synthetic creation, deletion, Media1 video binding or Media2 generic binding;
//! refusals preserve recordings. Idempotent binding commits retire reads too.
//! Other mutation/dependency and transaction/callback paths
//! remain under review.
//! Profile creation enforces the advertised limit of eight without truncating
//! larger imported fixtures. Media2 initial bindings, optional rename and All
//! selection are applied atomically; touched reference counts track committed
//! profiles. Conflicting assignments to one slot are refused. This is not full
//! physical configuration compatibility or arbitrary seed normalization.
//! Authentication faults use a structured serializer with a bound first
//! `wsse:FailedAuthentication` subcode and escaped reason text; XML-invalid reason
//! characters become U+FFFD. Credential parsing now requires unique qualified
//! Header/UsernameToken fields, explicit PasswordDigest Type and a nonempty
//! base64 nonce; input identities are decoded without trimming and errors do not
//! echo credentials. Auth defaults and HTTP status are unchanged. Freshness,
//! nonce reuse prevention and user-level authorization are not implemented.
//! Remaining shared fault output escapes text and binds known prefixes, but still
//! uses the legacy flat code hierarchy. These changes do not claim full conformance.

#[cfg(feature = "metamorph")]
mod adapter_request;
#[cfg(feature = "metamorph")]
pub(crate) use adapter_request::AdapterRequest;
mod auth;
pub(crate) mod canon;
pub(crate) mod dispatch;
pub(crate) mod effect;
mod fault;
pub(crate) mod fault_injection;
pub(crate) mod helpers;
pub(crate) mod policy;
mod request;
#[cfg(feature = "metamorph")]
pub(crate) use request::recording_equivalent;
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

pub use policy::AckOnlyOperation;
pub use responder::{Chain, RequestCtx, Responder};
pub use state::{DeviceState, MockState};
pub use transport::MockTransport;

#[cfg(feature = "mock-server")]
pub use discovery_responder::DiscoveryResponder;
#[cfg(feature = "mock-server")]
pub use fleet::{Fleet, FleetBuilder};
#[cfg(feature = "mock-server")]
pub use server::MockServer;
