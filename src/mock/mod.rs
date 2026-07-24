//! In-process / bound-port mock ONVIF device for testing client code without a
//! real camera.
//!
//! Every vendor's ONVIF differs and depending on a physical IP camera in unit
//! tests is painful. This module answers SOAP requests with plausible, stateful
//! canned responses (Set persists, Get reflects it) covering every operation
//! oxvif implements.
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
