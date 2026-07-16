# oxvif-metamorph — the shape-shifting ONVIF device server

> Evolve the existing `oxvif` mock server into a **device that changes shape**.
> One server, three personas: **synthesise** (hand-authored), **clone** (record a
> real camera and replay its quirks verbatim), **adapt** (put an ONVIF skin on a
> non-ONVIF device).

This file is the authoritative spec a coding agent builds against. It is dev-only
(the `docs/` directory is excluded from the published crate).

Status: **spec locked** for M0. The pre-work decisions in
[§1](#1-locked-decisions) are settled; the milestone-scoped open questions in
[§9](#9-still-open-decide-at-the-milestone-not-now) are deliberately deferred to
their milestone and must NOT be pre-empted.

---

## 0. Naming & placement

- **Spelling**: `metamorph` (correct — "a thing that changes shape").
- **Home**: stays **inside the `oxvif` crate** as a `metamorph` feature; code under
  `src/metamorph/`. It is a **superset** of `mock` / `mock-server` and reuses
  `dispatch` / `DeviceState` / `MockTransport` / `MockServer` directly — no
  cross-crate dependency friction. The server binary/target uses a
  `metamorph-server` feature (reuses the existing axum dependency).
- The `mock` module is **left untouched**; it becomes metamorph's synthetic
  "terminal responder" (see [§3](#3-keystone--the-responder-chain-m0)).
- **Hard constraint — backward compatibility**: `oxvif` is published on crates.io.
  The public API of `mock` / `mock-server` **must not break**. Every new
  capability is feature-gated (`metamorph` / `metamorph-server`).

---

## 1. Locked decisions

Settled in design review; these are the M0 spec. Do not relitigate mid-build.

| # | Decision | Resolution | Why |
|---|----------|------------|-----|
| **D1** | Crate form | Stay in `oxvif` as a `metamorph` feature | M0 is a refactor of oxvif internals; a separate crate would force the keystone across a crate boundary. |
| **D2** | Extraction seam | Make `Responder` + `RequestCtx` **public and stable early** | The public trait is the fission line: as long as `ReplayResponder`/`AdapterResponder` depend only on the public `Chain`/`Responder`/`SharedState` API and never on `mock` private internals, lifting them into a sibling `oxvif-metamorph` later is near-painless. |
| **D3** | Chain shape | **async** — `#[async_trait] Responder::respond` | The chain is *already* invoked from async (`Transport::send` is `#[async_trait]`, the `MockServer` handler is axum-async), so `async fn` costs nothing at the call site. The one responder that does I/O — `AdapterResponder` (`snapshot`/`ptz_move` hit a real device) — needs it; a sync trait would force `block_on` inside a tokio worker thread (panics / starves the executor) precisely there, and M5 would have to convert every responder anyway. `async-trait` is already a dependency; the per-call future box is irrelevant for a mock server. |
| **D4** | Masking classes | **Two classes** (see [§5.1](#51-normaliser--volatile-field-masker-keystone)) | Transport ephemera (MessageID/UUID, UtcTime, nonce, created, subscription refs) are masked in both the fixture *key* and value comparison; semantic identifiers (profile/media token) are **preserved in the key** and masked only in value comparison. Masking tokens into the key would collapse `GetProfile(token=A)` and `(token=B)` — the exact collision the param-aware key must avoid. Resolves the old open-Q4. |
| **D5** | Replay copy-on-write | **Coarse first** | A `Set*` invalidates the whole replay for that operation family; subsequent reads fall to synthetic `DeviceState`. Get `SetHostname → GetHostname` round-tripping before attempting field-level overlay (the highest-risk part of the plan). |
| **D6** | Control plane placement | **Binary only** | The REST/WS control plane lives in the `metamorph-server` binary target; the library stays headless-first. The Dioxus UI lives in oxdm as a pure control-plane client (keeps CI headless; avoids the Dioxus-desktop WebView issues). |
| **D7** | Delivery scope | **Ship M0–M2 as one increment** | M0–M2 already yields "clone a real camera, replay it in CI without hardware" — the highest-value slice. Personas A and C are separable products, re-evaluated after M2; they do not ride the same release train. |

### D3 in code

```rust
/// One request in flight.
pub struct RequestCtx<'a> {
    pub action: &'a str,       // SOAP operation
    pub base: &'a str,         // service base URL
    pub body: &'a str,         // raw request XML
    pub state: &'a SharedState,
}

/// Answers if it can; returns `None` to pass to the next responder.
#[async_trait]
pub trait Responder: Send + Sync {
    async fn respond(&self, ctx: &RequestCtx<'_>) -> Option<String>;
}
```

---

## 2. Existing mock architecture (read first)

| Path | Role |
|------|------|
| `src/mock/dispatch.rs` | Entry point `dispatch(action, base, state, body) -> String`; `match action` → per-service handler. **Single strategy today**: synthesise from state. |
| `src/mock/state.rs` | `DeviceState` (serde), `MockState`, `SharedState`; `.modify()` / `.read()` / `on_change` (`ChangeHook`) persistence seam. |
| `src/mock/services/*.rs` | device / media / media2 / ptz / imaging / events / recording handlers. |
| `src/mock/transport.rs` | `MockTransport` — in-process `Transport`. |
| `src/mock/server.rs` | `MockServer` (axum, bound port). Builder: `.port()` `.initial_state()` `.on_change()` `.enforce_auth()`. Extra HTTP: `POST /admin/inject_fault`, `POST /admin/clear_faults`, `GET /mock/snapshot.jpg`. |
| `src/mock/fault_injection.rs` | Single-shot fault queue — the ready-made head of the responder chain. |
| `src/mock/snapshot.rs` + `font.rs` | Synthetic test JPEG (bitmap font). |
| `src/mock/auth.rs` | WS-UsernameToken verification. |
| `src/fixtures.rs` | `CapturingTransport` (record) / `FixtureTransport` (replay). **Client-layer replay only today (unit tests); not wired into `MockServer`.** |
| `src/health/` | `HealthReport`, `ReportDiff`, and the `CaptureTransport` masking already shipped in 0.13.0 — reuse its WS-Security redaction as the [§5.1](#51-normaliser--volatile-field-masker-keystone) (a)-class seed. |
| `src/types/` | Public protocol types. **No serde derive yet.** |

Gaps this plan closes:
1. `dispatch` is a single strategy — no seam to insert replay or adapter sources.
2. fixtures aren't wired into the server; keyed by action name only, param-blind,
   last-write-wins, static.
3. **No WS-Discovery responder** — the mock can't be found by multicast probe.
4. No reusable normalise + volatile-field mask (current fixtures are hand-scrubbed).
5. `MockServer` is one `DeviceState` per server — no multi-device.

### Non-goals (do NOT over-build)

- ❌ No full XML C14N — a pragmatic normaliser is enough ([§5.1](#51-normaliser--volatile-field-masker-keystone)).
- ❌ No media transcode/remux — media is **URL pass-through** first ([§5.4](#54-media-policy)).
- ❌ No WebRTC.
- ❌ No rewrite of `mock` — layer on top of it only.

---

## 3. Keystone — the responder chain (M0)

Abstract `dispatch`'s single strategy into **a chain of responders, each of which
can answer or pass**. This is the shared floor under every persona.

```rust
pub struct Chain {
    responders: Vec<Box<dyn Responder>>,
}

impl Chain {
    pub async fn respond(&self, ctx: &RequestCtx<'_>) -> String {
        for r in &self.responders {
            if let Some(resp) = r.respond(ctx).await {
                return resp;
            }
        }
        // Terminal fallback: synthetic dispatch always answers.
        crate::mock::dispatch::dispatch(ctx.action, ctx.base, ctx.state, ctx.body)
    }
}
```

Refactor:
- Wrap existing `dispatch` as **`SyntheticResponder`** (terminal, always `Some`).
- Wrap existing `fault_injection` as **`FaultResponder`** (chain head): hit → override, else pass.
- Default chain: `[FaultResponder, (ReplayResponder?), (AdapterResponder?), SyntheticResponder]`.
- **Write semantics (`Set*`) always land in `SyntheticResponder`'s `DeviceState`** — so
  even when reads come from replay/adapter, state changes still work (COW basis, [§4-B](#persona-b--replay--clone-4-b)).

**M0 acceptance — the only success condition**: every existing mock test stays
green. Swap `dispatch` for a `Chain` of `[FaultResponder, SyntheticResponder]`;
behaviour is byte-for-byte unchanged. No new behaviour in M0.

---

## 4. The three personas

### Persona A — synthetic + control plane (M4)

Operate the virtual device like a real camera's web admin.

- Server stays **headless-first**; exposes a **control-plane API** (the existing
  `/admin/*` grown into a full REST/WS surface): read/write `DeviceState`
  (identity, network, profiles…), switch persona, arm/clear faults, query status.
- **Dioxus web UI (in oxdm) is a pure control-plane client** — forms hit the API,
  snapshot thumbnails poll. UI is never welded into the server (per D6).
- Reuse the `on_change` hook for config persistence (TOML/JSON).

**Acceptance**: change device name/resolution/network in the web UI → the
corresponding ONVIF `Get*` response reflects it immediately.

### Persona B — replay / clone (M2)

Record a real camera, then have metamorph **play that model verbatim, quirks and all**.

- **Record**: CLI subcommand (`oxvif metamorph record <camera-url>`) drives a
  standard operation set through `CapturingTransport`, lands a fixture set.
- **Replay**: new **`ReplayResponder`** inserts fixtures into the chain, ahead of
  `SyntheticResponder`.
- Two fixture problems this must solve:
  1. **Copy-on-write (coarse, per D5)**: reads prefer the fixture; once a client
     `Set*`s an operation family, that family's fixture is invalidated and reads
     fall to synthetic `DeviceState` (so `Set → Get` round-trips).
  2. **Param-aware key (per D4)**: the fixture key hashes the **(a)-masked,
     normalised body** — transport ephemera don't fragment keys, but semantic
     params (profile token) stay in the key so `GetProfile(token=A)` and `(=B)`
     don't collide.
- Recorded fixtures are auto-scrubbed with the [§5.1](#51-normaliser--volatile-field-masker-keystone) masker before landing.

**Acceptance**: `record` a real camera (or a mock pretending to be one), `replay`,
drive it with an oxvif client — `GetDeviceInformation` etc. match the original;
`SetHostname` then `GetHostname` reflects the new value.

### Persona C — adapter / skin template (M5)

Put an ONVIF skin on a device that only speaks RTSP / a private protocol.
The core is a **low-barrier template**: supply an ONVIF-shaped in/out mapping.

```rust
/// Implement this for a non-ONVIF device to get a working ONVIF device.
/// Only a couple of methods are required; the rest fall through to synthetic.
#[async_trait]
pub trait DeviceAdapter: Send + Sync {
    /// Advertised identity (GetDeviceInformation / discovery).
    fn identity(&self) -> DeviceIdentity;

    /// Media URI for a profile (the real stream being skinned).
    fn stream_uri(&self, profile: &str) -> Option<String>;

    /// All optional; default = unsupported / no-op.
    async fn ptz_move(&self, _req: PtzMove) -> AdapterResult { AdapterResult::Unsupported }
    async fn snapshot(&self) -> Option<Vec<u8>> { None }
    // … other optional hooks
}
```

- New **`AdapterResponder`** translates ONVIF operations into `DeviceAdapter`
  calls; unimplemented operations fall through to `SyntheticResponder`.
- **Minimum viable set**: `identity()` + `stream_uri()` is enough for any standard
  NVR / Frigate to ingest it as an ONVIF camera.
- Ship one example adapter (e.g. a fixed RTSP URL → ONVIF device) as the template.

**Acceptance**: a <50-line example adapter wrapping one RTSP stream is discovered
by an oxvif client (or a standard NVR) and yields the correct RTSP URL.

---

## 5. Shared foundations

### 5.1 Normaliser + volatile-field masker (keystone)

One module, three payoffs (replay fidelity, fixture auto-scrub, stable regression).

- **Normalise**: namespace-prefix-agnostic, attribute ordering, whitespace collapse.
- **Mask — two classes (per D4)**:
  - **(a) transport ephemera** — token-less: `MessageID`/UUID, `UtcTime`/timestamps,
    subscription reference URLs, nonce/created. Masked in **both** the fixture key
    and value comparison. Seed the (a) list from the shipped `health::CaptureTransport`
    WS-Security redaction.
  - **(b) semantic identifiers** — profile/media tokens. **Preserved in the key**;
    masked only in value comparison.
- **Pragmatic**: do NOT chase W3C XML C14N.
- Output a comparable intermediate form for replay matching and (later) diff.

### 5.2 WS-Discovery responder

Add a multicast Probe **responder** (`discovery.rs` today is client-probe only), so
metamorph is transparently found by oxdm/Frigate. Reported scopes/types match the
current persona (synthetic default identity, or the clone's real scopes). Keep it
feature-gated; note multicast is flaky in CI/containers — also cover a direct
unicast probe path in integration tests.

### 5.3 Multi-device / fleet

- **Short term**: start several `MockServer`s, each on its own port (do first).
- **Mid term**: `dispatch` routes by path/port to multiple `DeviceState`s (cleaner, later).

### 5.4 Media policy

- **URL pass-through first**: when skinning/cloning, the advertised RTSP/snapshot
  URI points at the real source; the client connects directly.
- Media relay (proxying through this host) is **later, non-essential**.

### 5.5 serde on public types

`src/types/` has no serde derive. Parsed-struct-level diff/inspection needs opt-in
`Serialize` first (a roadmap item, promoted to a prerequisite here). Structural
(raw-XML) comparison is not blocked by this and can proceed.

---

## 6. Milestones

Each ends with: existing tests green + new tests added + CHANGELOG/feature docs updated.

- **M0 — Responder chain refactor ([§3](#3-keystone--the-responder-chain-m0))**. Synthetic dispatch + fault queue move
  behind the async `Chain`, behaviour unchanged. *Keystone — do first.*
- **M1 — Normaliser + masker ([§5.1](#51-normaliser--volatile-field-masker-keystone))**. Standalone module + unit tests (same
  response with different tokens/timestamps → equal after masking; two profile
  tokens → distinct keys).
- **M2 — Persona B record/replay ([§4-B](#persona-b--replay--clone-m2))**. CLI `record`, `ReplayResponder`,
  coarse COW, param-aware key. *End of the shippable increment (per D7).*
- **M3 — WS-Discovery responder ([§5.2](#52-ws-discovery-responder))**. Clone is multicast-discoverable; scopes
  match the current persona.
- **M4 — Control plane + Persona A ([§4-A](#persona-a--synthetic--control-plane-m4))**. Grow `/admin/*`; oxdm Dioxus UI drives it.
- **M5 — Persona C skin template ([§4-C](#persona-c--adapter--skin-template-m5))**. `DeviceAdapter` + `AdapterResponder` + one
  RTSP example adapter.
- **M6 — Multi-device fleet ([§5.3](#53-multi-device--fleet))**. Multi-port first; path routing if time allows.
- **M7 (stretch) — quirk diff**. Masker-driven structural diff (baseline vs clone),
  surfaced in oxdm; semantic diff waits on [§5.5](#55-serde-on-public-types).

M0–M2 is a complete, independently deliverable "metamorph that clones a real
camera" — do not wait for the rest.

---

## 7. Guardrails

1. **Backward compatible**: `mock` / `mock-server` public API unbroken; new
   capability all feature-gated (`metamorph` / `metamorph-server`).
2. **Headless-first**: the server core never depends on UI; UI is a control-plane
   client only.
3. **No over-engineering**: no full C14N, no transcode, no WebRTC ([§2 non-goals](#non-goals-do-not-over-build)).
4. **Single write sink**: every `Set*` mutates only `DeviceState` — no split state.
5. **Independently deliverable stages**: M0–M2 alone is "a metamorph that clones a
   real camera".
6. **Test-first**: every responder and the masker gets unit tests; the clone flow
   gets a record→replay integration test (use "a mock pretending to be a real
   camera" as the record target — no real device needed).

---

## 8. Gate (before every commit, per CLAUDE.md — run with `--all-features`)

```
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

CI runs `--all-features`; the plain `--all-targets` form is red on this repo
because `examples/conformance.rs` lacks a `required-features` declaration.

---

## 9. Still open — decide at the milestone, not now

Deferred by design; do NOT pre-empt these during M0.

1. **Clone baseline (affects M7)**: is the diff reference "synthetic default",
   "spec ideal", or "device-vs-device"?
2. **Control-plane protocol (affects M4)**: REST-only, or WS push for live state
   to oxdm?
3. **serde derive scope (affects M7)**: derive across all of `src/types/` at once,
   or only the types the diff/adapter path touches?
