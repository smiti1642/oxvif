# Authentication freshness AF1

[English](mock-fidelity-auth-freshness.md) | [繁體中文](mock-fidelity-auth-freshness_zh.md)

Status: DONE for AF1. W01 recorded before edits on 2026-09-23, baseline `3e5d2b2`.
Owner: Q02 / W08; existing scoped-auth controls remain mandatory.

| Surface | Existing contract | Planned effect / refusal |
| --- | --- | --- |
| MockState | Private state plus volatile subscription runtime | Private bounded nonce cache and controllable test clock; no public fields or persistence change |
| AuthResponder → validate_ws_security | Scoped Header token, live user password, digest; exact GetSystemDateAndTime exemption | Verify digest before freshness and atomic nonce admission; preserve armed-fault/auth/raw/synthetic ordering |
| MockTransport / MockServer / AdapterTransport / MetamorphTransport | Shared auth gate when explicitly enabled | Same cache across clones of one state, separate across instances; disabled/exempt paths do not consume entries |
| Failed authentication | Sender/wsse:FailedAuthentication with fixed reason, no hooks/state writes | Static stale/future/invalid-time/replay/capacity reasons, no supplied token echo and no failed-request cache mutation |

Reference: [OASIS UsernameToken 1.1.1 §3.1](https://docs.oasis-open.org/wss-m/wss/v1.1.1/os/wss-UsernameTokenProfile-v1.1.1-os.html).
It recommends timestamp freshness and nonce retention. The concrete limits below
are explicit mock policy choices, not a claim of complete WS-Security conformance.

Accepted timestamp age: at most 300 seconds past or 60 seconds future, inclusive.
Accept exact UTC second timestamps (trim surrounding XML whitespace for time
validation only; digest continues hashing the original decoded Created text).
Retain each decoded nonce until both its creation freshness interval and 300
seconds after admission have elapsed. Maximum 4096 entries and 256 decoded bytes
per nonce. Refuse capacity without evicting live entries. Admission is one lock;
replayed/nonfresh/bad-digest/capacity-refused requests leave the cache unchanged.
Successful auth consumes the nonce even if downstream service validation refuses.
An armed injected fault precedes auth and does not consume it. Authentication
does not invoke device change hooks. Keep a last-accepted clock floor so wall-clock
rollback cannot reopen an already evicted freshness interval. Restart clears this
volatile cache; persistent replay protection is not claimed.

Acceptance: deterministic boundary/expiry/rollback/capacity/refusal tests; concurrent
identical tokens accept exactly once; base64 lexical variants share decoded nonce
identity; live user changes remain visible; both transports, disabled/exempt paths,
fault precedence, instance/clone isolation and zero hooks. Existing scoped-header
tests must use fresh independent tokens for positive cases without weakening their
namespace/whitespace/literal-value assertions. Run one full mutation campaign,
restore exactly, all five workspace gates, docs and bilingual inventory checks.
HTTP binding, role authorization and structured/raw injection are other Q02 slices.

## Verification

The private cache and clock floor implement the policy above. Existing scoped
auth positives now re-sign fresh independent tokens, retaining their namespace,
base64/Created whitespace and literal-identity assertions. New unit and both-transport
tests cover expiry boundaries, capacity recovery, rollback, concurrent admission,
cross-user decoded-nonce identity, bad-digest preservation, exemptions, fault order,
disabled auth and zero device hooks. Auth-success/service-refusal consumes a nonce.

The full unfiltered workspace all-feature mutation campaign produced 9 failing
tests and exit 101; auth.rs was restored byte-for-byte before final gates.
Formatting, both workspace/all-target Clippy modes and workspace tests pass:
1378 all-feature / 1259 default. Strict rustdoc passes both modes.
The inventory still reconciles 159 routes, 161 Action sites and 162 reader occurrences.

Pinned external Xerces validates 224 XML instances / 112 exchanges / 60 Actions.
AF1 adds one actual success and three captured authentication Fault responses.
The exporter's no-credentials guard was retained: synthetic Security headers are
removed from these four request captures before export. Thus service payloads and
response wire are covered, but exported authentication headers are not validated.
Runtime auth tests exercise the original complete tokens. No real credentials,
camera writes, public release or full WS-Security/authorization acceptance is claimed.
