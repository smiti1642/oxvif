//! The metamorph responder chain — the seam that lets a mock device answer a
//! request from more than one source.
//!
//! A [`Chain`] holds an ordered list of [`Responder`]s. Each is offered the
//! request and may answer (`Some`) or pass (`None`) to the next. The default
//! pipeline ([`Chain::default_mock`]) is
//! `[FaultResponder, AuthResponder, SyntheticResponder]` — a byte-for-byte
//! reproduction of the inline flow `MockTransport` / `MockServer` used before
//! the chain existed. Later personas (fixture replay, device adapter) slot new
//! responders in ahead of the terminal [`SyntheticResponder`] without touching
//! the callers.
//!
//! The trait is deliberately `async` (see `docs/active/metamorph.md` D3): the chain is
//! always invoked from an async context, and a future adapter responder needs to
//! `.await` real-device I/O — a sync trait would force a `block_on` inside a
//! tokio worker thread.

use std::sync::Arc;

use async_trait::async_trait;

use crate::mock::dispatch::dispatch;
use crate::mock::fault_injection::FaultInjector;
use crate::mock::state::MockState;
use crate::mock::{auth, helpers};

/// One request in flight through the [`Chain`].
pub struct RequestCtx<'a> {
    /// SOAP action URI (from the `action=` content-type parameter / WS-Addressing).
    pub action: &'a str,
    /// Base URL the device uses to build absolute URLs (snapshot, subscription refs).
    pub base: &'a str,
    /// Raw request body XML.
    pub body: &'a str,
    /// The shared device state.
    pub state: &'a MockState,
}

/// A source of responses. Answers with `Some`, or passes with `None` to defer
/// to the next responder.
///
/// Implement this to teach a metamorph device a new way to answer — a fixture
/// replay, an adapter over a non-ONVIF device, a fault gate. This trait is the
/// stable extension seam (`docs/active/metamorph.md` D2); it is `async` so responders
/// that do real I/O can `.await` directly.
#[async_trait]
pub trait Responder: Send + Sync {
    /// Answer `ctx`, or return `None` to defer to the next responder in the chain.
    async fn respond(&self, ctx: &RequestCtx<'_>) -> Option<String>;
}

/// An ordered list of [`Responder`]s, consulted first to last.
pub struct Chain {
    responders: Vec<Box<dyn Responder>>,
}

impl Chain {
    /// Build a chain from an explicit responder list. The last responder should
    /// be terminal (always answer); an otherwise-unhandled request falls to a
    /// defensive `s:Receiver` fault.
    pub fn new(responders: Vec<Box<dyn Responder>>) -> Self {
        Self { responders }
    }

    /// The default mock pipeline: armed fault → auth gate → synthetic dispatch.
    /// Reproduces the pre-chain inline flow exactly.
    pub(crate) fn default_mock(faults: Arc<FaultInjector>, enforce_auth: bool) -> Self {
        Self::mock_with_extra(faults, enforce_auth, Vec::new())
    }

    /// The default mock pipeline with `extra` responders spliced in immediately
    /// before the terminal [`SyntheticResponder`] — the insertion point for
    /// metamorph personas (replay, adapter). Fault + auth heads and the
    /// synthetic terminal are kept intact, so writes still land in `DeviceState`.
    pub(crate) fn mock_with_extra(
        faults: Arc<FaultInjector>,
        enforce_auth: bool,
        extra: Vec<Box<dyn Responder>>,
    ) -> Self {
        let mut responders: Vec<Box<dyn Responder>> = Vec::with_capacity(extra.len() + 3);
        responders.push(Box::new(FaultResponder { faults }));
        responders.push(Box::new(AuthResponder { enforce_auth }));
        responders.extend(extra);
        responders.push(Box::new(SyntheticResponder));
        Self::new(responders)
    }

    /// Offer the request to each responder in turn; return the first answer.
    pub async fn respond(&self, ctx: &RequestCtx<'_>) -> String {
        for r in &self.responders {
            if let Some(resp) = r.respond(ctx).await {
                return resp;
            }
        }
        // Unreachable while a terminal responder is present; defensive fallback.
        helpers::resp_soap_fault("s:Receiver", "no responder handled the request")
    }
}

/// Chain head: consumes a single-shot armed fault matching the action, else passes.
pub(crate) struct FaultResponder {
    faults: Arc<FaultInjector>,
}

#[async_trait]
impl Responder for FaultResponder {
    async fn respond(&self, ctx: &RequestCtx<'_>) -> Option<String> {
        self.faults
            .take_for_action(ctx.action)
            .map(|f| helpers::resp_soap_fault(&f.code, &f.reason))
    }
}

/// WS-Security gate: when auth is enforced and the action requires it, rejects
/// invalid credentials with an auth fault; otherwise passes.
pub(crate) struct AuthResponder {
    enforce_auth: bool,
}

#[async_trait]
impl Responder for AuthResponder {
    async fn respond(&self, ctx: &RequestCtx<'_>) -> Option<String> {
        if self.enforce_auth && auth::requires_auth(ctx.action) {
            if let Err(reason) = auth::validate_ws_security(ctx.body, ctx.state) {
                return Some(auth::auth_fault(&reason));
            }
        }
        None
    }
}

/// Terminal responder: synthesises a stateful response from `DeviceState`.
/// Always answers, so it must be last in the chain.
pub(crate) struct SyntheticResponder;

#[async_trait]
impl Responder for SyntheticResponder {
    async fn respond(&self, ctx: &RequestCtx<'_>) -> Option<String> {
        Some(dispatch(ctx.action, ctx.base, ctx.state, ctx.body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::fault_injection::PendingFault;

    const GET_DEVICE_INFO: &str = "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation";

    fn ctx<'a>(action: &'a str, body: &'a str, state: &'a MockState) -> RequestCtx<'a> {
        RequestCtx {
            action,
            base: "http://mock",
            body,
            state,
        }
    }

    #[tokio::test]
    async fn synthetic_terminal_answers() {
        let state = MockState::new();
        let chain = Chain::default_mock(Arc::new(FaultInjector::new()), false);
        let out = chain.respond(&ctx(GET_DEVICE_INFO, "", &state)).await;
        assert!(out.contains("oxvif-mock"), "expected synthetic device info");
    }

    #[tokio::test]
    async fn armed_fault_short_circuits_before_synthetic() {
        let state = MockState::new();
        let faults = Arc::new(FaultInjector::new());
        faults.inject(PendingFault {
            action_suffix: "GetDeviceInformation".into(),
            code: "ter:NotAuthorized".into(),
            reason: "nope".into(),
        });
        let chain = Chain::default_mock(faults, false);
        let out = chain.respond(&ctx(GET_DEVICE_INFO, "", &state)).await;
        assert!(
            out.contains("ter:NotAuthorized"),
            "expected the armed fault"
        );
        assert!(
            !out.contains("oxvif-mock"),
            "fault must short-circuit before synthetic"
        );
    }

    #[tokio::test]
    async fn auth_gate_blocks_when_enforced_without_credentials() {
        let state = MockState::new();
        let chain = Chain::default_mock(Arc::new(FaultInjector::new()), true);
        // GetDeviceInformation requires auth; an empty body has no WS-Security.
        let out = chain.respond(&ctx(GET_DEVICE_INFO, "", &state)).await;
        assert!(
            !out.contains("oxvif-mock"),
            "auth gate must block synthetic when credentials are missing"
        );
    }
}
