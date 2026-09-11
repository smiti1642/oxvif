//! The metamorph responder chain — the seam that lets a mock device answer a
//! request from more than one source.
//!
//! A [`Chain`] holds an ordered list of [`Responder`]s. Each is offered the
//! request and may answer (`Some`) or pass (`None`) to the next. The default
//! pipeline ([`Chain::default_mock`]) is
//! `[FaultResponder, AuthResponder, SyntheticResponder]` — originally a byte-for-byte
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

use crate::mock::dispatch::respond_with_effect;
use crate::mock::effect::EffectObserver;
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
    /// Preserves fault/auth precedence and the synthetic terminal.
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
        Self::mock_with_observer(faults, enforce_auth, extra, None)
    }

    pub(crate) fn mock_with_observer(
        faults: Arc<FaultInjector>,
        enforce_auth: bool,
        extra: Vec<Box<dyn Responder>>,
        observer: Option<EffectObserver>,
    ) -> Self {
        let mut responders: Vec<Box<dyn Responder>> = Vec::with_capacity(extra.len() + 3);
        responders.push(Box::new(FaultResponder { faults }));
        responders.push(Box::new(AuthResponder { enforce_auth }));
        responders.extend(extra);
        responders.push(Box::new(SyntheticResponder { observer }));
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
        super::fault::Fault::new(
            super::fault::Code::Receiver,
            &[],
            "no responder handled the request",
        )
        .to_xml()
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
        if self.enforce_auth
            && auth::requires_auth(ctx.action)
            && let Err(reason) = auth::validate_ws_security(ctx.body, ctx.state)
        {
            return Some(auth::auth_fault(&reason));
        }
        None
    }
}

/// Terminal responder: synthesises a stateful response from `DeviceState`.
/// Always answers, so it must be last in the chain.
pub(crate) struct SyntheticResponder {
    observer: Option<EffectObserver>,
}

#[async_trait]
impl Responder for SyntheticResponder {
    async fn respond(&self, ctx: &RequestCtx<'_>) -> Option<String> {
        let (xml, effect) = respond_with_effect(ctx.action, ctx.base, ctx.state, ctx.body);
        if let (Some(observer), Some(effect)) = (&self.observer, effect) {
            observer(effect);
        }
        Some(xml)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::effect::Effect;
    use crate::mock::fault_injection::PendingFault;
    use crate::soap::{SoapError, find_response, parse_soap_body};
    use std::sync::Mutex;

    struct ProbeResponder {
        label: &'static str,
        seen: Arc<Mutex<Vec<(&'static str, String)>>>,
        answer: Option<&'static str>,
    }

    #[async_trait]
    impl Responder for ProbeResponder {
        async fn respond(&self, ctx: &RequestCtx<'_>) -> Option<String> {
            self.seen
                .lock()
                .unwrap()
                .push((self.label, ctx.body.into()));
            self.answer.map(str::to_owned)
        }
    }

    fn assert_fault(xml: &str, code: &str, reason: &str, subcode: Option<&str>) {
        let body = parse_soap_body(xml).unwrap();
        assert_eq!(
            find_response(&body, "GetDeviceInformationResponse").unwrap_err(),
            SoapError::Fault {
                code: code.into(),
                reason: reason.into(),
                subcode: subcode.map(str::to_owned),
                detail: None,
            }
        );
    }

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
    async fn committed_effect_observer_is_not_a_request_or_fault_hook() {
        let state = MockState::new();
        state.modify(|s| s.profiles.profiles[1].fixed = false);
        let before = serde_json::to_value(&*state.read()).unwrap();
        let token = state.read().profiles.profiles[1].token.clone();
        let action = "http://www.onvif.org/ver10/media/wsdl/DeleteProfile";
        let body = format!(
            "<m:DeleteProfile xmlns:m='http://www.onvif.org/ver10/media/wsdl'><m:ProfileToken>{token}</m:ProfileToken></m:DeleteProfile>"
        );
        let effects = Arc::new(Mutex::new(Vec::new()));
        let seen = effects.clone();
        let observer: EffectObserver = Arc::new(move |effect| seen.lock().unwrap().push(effect));

        for (inject, auth) in [(true, true), (false, true), (false, false)] {
            let faults = Arc::new(FaultInjector::new());
            if inject {
                faults.inject(PendingFault {
                    action_suffix: "DeleteProfile".into(),
                    code: "env:Receiver".into(),
                    reason: "effect-gate-832".into(),
                });
            }
            let chain = Chain::mock_with_observer(
                faults,
                auth,
                vec![Box::new(ProbeResponder {
                    label: "raw",
                    seen: Arc::default(),
                    answer: Some("<raw-effect-832/>"),
                })],
                Some(observer.clone()),
            );
            let xml = chain.respond(&ctx(action, &body, &state)).await;
            if inject {
                assert_fault(&xml, "env:Receiver", "effect-gate-832", None);
            } else if auth {
                assert_fault(
                    &xml,
                    "s:Sender",
                    "Missing Username",
                    Some("wsse:FailedAuthentication"),
                );
            } else {
                assert_eq!(xml, "<raw-effect-832/>");
            }
            assert!(effects.lock().unwrap().is_empty());
            assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
        }
        let chain = Chain::mock_with_observer(
            Arc::new(FaultInjector::new()),
            false,
            Vec::new(),
            Some(observer),
        );
        let duplicate = body.replace(
            "</m:DeleteProfile>",
            "<m:ProfileToken>decoy</m:ProfileToken></m:DeleteProfile>",
        );
        assert_fault(
            &chain.respond(&ctx(action, &duplicate, &state)).await,
            "env:Sender",
            "InvalidRequest-DELETEPROFILE: duplicate request field",
            None,
        );
        assert!(effects.lock().unwrap().is_empty());
        let xml = chain.respond(&ctx(action, &body, &state)).await;
        assert!(
            parse_soap_body(&xml)
                .unwrap()
                .child("DeleteProfileResponse")
                .is_some()
        );
        assert!(
            !state
                .read()
                .profiles
                .profiles
                .iter()
                .any(|p| p.token == token)
        );
        assert_eq!(*effects.lock().unwrap(), [Effect::ProfilesChanged]);
        assert_fault(
            &chain.respond(&ctx(action, &body, &state)).await,
            "s:Sender",
            &format!("Profile not found: {token}"),
            Some("ter:InvalidArgVal"),
        );
        assert_eq!(*effects.lock().unwrap(), [Effect::ProfilesChanged]);
    }

    #[tokio::test]
    async fn synthetic_terminal_answers() {
        let state = MockState::new();
        let chain = Chain::default_mock(Arc::new(FaultInjector::new()), false);
        let body = "<GetDeviceInformation xmlns='http://www.onvif.org/ver10/device/wsdl'/>";
        let out = chain.respond(&ctx(GET_DEVICE_INFO, body, &state)).await;
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

    #[tokio::test]
    async fn fault_precedes_auth_and_extras_even_for_malformed_input() {
        let state = MockState::new();
        let faults = Arc::new(FaultInjector::new());
        faults.inject(PendingFault {
            action_suffix: "GetDeviceInformation".into(),
            code: "env:Receiver".into(),
            reason: "pipeline-order-731 <&>".into(),
        });
        let seen = Arc::new(Mutex::new(Vec::new()));
        let chain = Chain::mock_with_extra(
            faults,
            true,
            vec![Box::new(ProbeResponder {
                label: "must-not-run",
                seen: seen.clone(),
                answer: Some("unexpected extra response"),
            })],
        );
        let request = ctx(GET_DEVICE_INFO, "not <valid XML &unknown;", &state);
        assert_fault(
            &chain.respond(&request).await,
            "env:Receiver",
            "pipeline-order-731 <&>",
            None,
        );
        // The injected fault is consumed once; the same request now reaches auth.
        assert_fault(
            &chain.respond(&request).await,
            "s:Sender",
            "Invalid WS-Security header",
            Some("wsse:FailedAuthentication"),
        );
        assert!(seen.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn extras_preserve_raw_input_order_and_short_circuit_output() {
        let state = MockState::new();
        let seen = Arc::new(Mutex::new(Vec::new()));
        // Intentionally malformed: the extension seam is not the normal
        // synthetic parser. A replay/custom responder may test raw deviations.
        let raw = "  <r:Command> &amp;amp;\r\n</r:Command><second/>  ";
        let answer = " <unbound:Recorded> &unknown; </unbound:Recorded>\r\n";
        let extras: Vec<Box<dyn Responder>> = [
            ("pass", None),
            ("answer", Some(answer)),
            ("unreachable", Some("wrong")),
        ]
        .into_iter()
        .map(|(label, answer)| {
            Box::new(ProbeResponder {
                label,
                seen: seen.clone(),
                answer,
            }) as Box<dyn Responder>
        })
        .collect();
        let chain = Chain::mock_with_extra(Arc::new(FaultInjector::new()), false, extras);
        assert_eq!(
            chain.respond(&ctx(GET_DEVICE_INFO, raw, &state)).await,
            answer
        );
        assert_eq!(
            *seen.lock().unwrap(),
            vec![("pass", raw.to_owned()), ("answer", raw.to_owned())]
        );
    }
}
