# metamorph clone-in-oxdm — design note

## Closure (2026-09-22)

**DONE — oxvif-side integration gaps are closed; new G3 code is unreleased.**
G1 public recorders are delivered. G2 was superseded by the shipped network
container. G3 now provides `FixtureStore::summary()`, `FixtureSummary` and
`FixtureActionSummary`; the recorder example consumes the totals.

The summary counts current retained fixtures, exact sorted Action URIs, direct
Body/Fault responses recognized by the existing SOAP parser, unreadable XML/body
responses and colliding Action/key buckets. Upserts replace only equivalent
requests; collisions remain separate. Transport errors are not stored and belong
to `SweepReport`. This is neither schema validation nor reference-value comparison.
Raw XML, keys, labels and Fault reasons are omitted; caller-supplied Action strings
remain subject to review before sharing.

Standing tests cover empty/mixed stores, SOAP 1.1/1.2 Fault shapes versus a nested
non-Fault element, malformed/missing Body responses, exact Action grouping,
collision/upsert behavior, load/save preservation, JSON serialization and
transport-error exclusion. The public example is also a compile-checked doctest.

Closure verification: workspace all-feature tests 1,331 passed; default 1,215
passed, each with seven ignored and 41 suites. Both all-target Clippy modes,
the isolated `metamorph` feature, strict rustdoc in both feature modes, formatting,
inventory controls and 1,879 local links / 812 anchors passed. No camera was used.

This closes this repository's G1–G3 scope. It does not claim new oxdm UI work,
real-camera acceptance or M4/M7 completion. Those broader features remain in
[Metamorph](../active/metamorph.md) and [F14](../active/post-0.17-backlog.md#calibrated-work-ownership).
The sections below preserve the original design and privacy context; their
prospective language does not reopen the superseded transport approach.

> **Original status: draft for review.** Wires Persona B (clone / replay) into
> oxdm so a user can clone their own IP camera's behaviour and hunt its quirks —
> without hardware after the first capture. Companion to
> [`metamorph.md`](../active/metamorph.md); this is the "M-clone" pre-work note.

---

## 1. Goal

In oxdm: point at your own IPCam → **clone it** (record the standard read
surface) → **replay it offline** → **find its quirks** (where the camera
deviates from what oxvif / the ONVIF spec expects).

Non-goal here: serving the clone over a bound port to *external* clients
(Frigate / ONVIF Device Manager). That is Persona A / M4 and is **not** required
for this feature — see §2.

---

## 2. Key architectural decision — this does **not** need M4

M4 (control plane + persona-over-port) exists to serve a virtual device to
*external* network clients. This feature is different:

- oxdm is a **native Rust** app (Dioxus desktop), already links `oxvif`, and
  already holds a live `OnvifSession` to the camera.
- So Persona B is consumed **as a library, in-process**. Record → replay →
  quirk-scan all run inside oxdm; no server, no `/admin` control plane, no
  bound port.

**Consequence:** this ships independently of, and much lighter than, M4. Do it
first; M4 stays a separate track.

---

## 3. User flow (in oxdm)

1. Discover / select the IPCam (oxdm already does this).
2. **"Clone this camera"** → wrap the session's transport in `RecordingTransport`,
   drive the standard read surface, write `fixtures.json` into oxdm's data dir.
   Show an `X / N` progress indicator.
3. **"Replay / inspect"** → load the fixtures into a `MetamorphTransport`, drive
   an in-process session, render the parsed structs. (The new `serde` feature
   makes rendering in Dioxus trivial — serialize straight to JSON for the view.)
4. **"Find quirks"** → §5.

---

## 4. What oxvif must provide — the gaps to close (this repo)

Original gap analysis, reconciled against `80bcf14`:

| # | Gap | Why it blocks oxdm | Size |
|---|-----|--------------------|------|
| **G1 — DONE** | Public `record_standard_surface(device_url, credentials, label)`, selected-surface and progress APIs exist in `src/metamorph/record.rs`; the example calls the library. | Recorder extraction is no longer a blocker. | Delivered |
| **G2 — SUPERSEDED** | **Original in-process transport proposal.** `health` already has `CoverageTransport` — the silent-drop / list-emptying / field-defaulting detector — but `HealthCheck::new()` hard-builds its own `HttpTransport` (`src/health/mod.rs:137`) and takes no custom transport. Add `HealthCheck::with_transport(Arc<dyn Transport>)` so oxdm can run it against a `MetamorphTransport`. | Turns the existing coverage detector into an **offline, reproducible quirk finder** over the clone. | Small |
| **G3 — DONE** | `FixtureStore::summary()` supplies current fixture totals and per-Action Fault/unreadable counts plus collision-bucket totals. | Consumers can render metadata without copying aggregation logic. | Delivered, unreleased |

G2 is not a current prerequisite: the shipped container supports normal network
health checks, and structural/typed-parse reports already exist. Neither report
proves all semantic differences from a real camera. G3 now has the bounded contract recorded in the closure above.

---

## 5. What "find quirks" means — now vs later

- **(a) Parse-coverage quirks — available now (via G2).** Where this camera's
  responses make oxvif's parser silently drop data. `CoverageTransport` already
  detects it; G2 just lets it run on the clone.
- **(b) Structural / semantic diff — this is M7.** Masker-driven diff of the
  clone's raw XML against a synthetic baseline → "your camera deviates here."
  The semantic-diff prerequisite ([`metamorph.md` §5.5](../active/metamorph.md)) is the
  `serde` derive across `src/types/`, **now landed** — so M7 is unblocked, though
  the diff itself is still unwritten.

`FixtureStore` stores the **real response bytes** as each fixture's value, so the
raw material for (b) is already in `fixtures.json` — M7 needs no re-capture.

---

## 6. Phasing

1. **Phase 1 (oxvif, small): G1 + G2 + G3.** After this, oxdm can do
   record → save → replay → quirk-scan (a).
2. **Phase 2 (oxdm repo, the user-facing bulk):** clone button, fixtures list,
   replay / inspect view (serde-rendered), progress, raw-vs-parsed side-by-side.
3. **Phase 3:** M7 structural / semantic diff (serde prerequisite already met),
   surfacing the deeper quirks (b) in oxdm.

---

## 7. Risks / open decisions

- **Credentials in fixtures.** Recording holds the camera's creds. Fixtures
  already blank WS-Security `Password`/`Nonce`, but **snapshot / stream URIs can
  embed credentials** (`rtsp://user:pass@…`). Confirm the masker scrubs them, or
  warn the user, before writing `fixtures.json`.
- **Coverage honesty.** The clone covers only the standard read surface, not the
  whole device. oxdm must label it "standard-surface snapshot," not imply a 100%
  clone (the no-silent-caps principle).
- **Raw vs parsed.** The strongest quirk signal is raw XML next to the parsed
  struct. The raw response is already in the fixture — worth a side-by-side view
  in Phase 2.
- **Standard-surface scope (decide at G1).** Is the recorded op list fixed, or
  should oxdm be able to extend it (e.g. add Profile G / events)? Fixed first;
  make the `Recorder` op set overridable only if a real need appears.

---

## 8. One-line summary

Don't go through M4. Consume Persona B as a library inside oxdm. oxvif closes
three small gaps (library-ify the recorder, let `HealthCheck` take a custom
transport, add a `FixtureStore` summary); oxdm then does
"clone my camera → replay → find quirks with the existing coverage detector."
Deep semantic diff (M7) is Phase 3, its prerequisite already unblocked by serde.
