# Plan: per-service capabilities + PTZ preset tours (target 0.15.0)

Status: **shipped in 0.15.0** (2026-07-29). Written 2026-07-28.

Progress against §7:

| Step | State |
|------|-------|
| Stage 0 — WSDL transcription | **done** — all twelve types verified against the published schema; corrections listed in that commit |
| Stage A — mock responders + dispatch split | **done** (`1d224f4`) — all nine services answer `GetServiceCapabilities`; `dispatch_recording` split into three |
| Stage A — types / client / session | **done** (`883e64f`) |
| Stage B — preset tours | **done** (`ac1f83e`) |
| Stage C — PTZ `SendAuxiliaryCommand` | **done** (`a7b9f10`) |

Pulling the mock forward was the right call: the client work in Stage A had a
device to test against on day one, and §2's field lists were settled fact rather
than something to verify mid-implementation.

> **The *how* now lives in a companion document:
> [tier1-implementation-map.md](tier1-implementation-map.md).** This file
> decides what to build and why; that one carries the per-attribute
> correspondence tables (schema name → Rust field → mock fixture value → the
> test that pins it), the pre-allocated fault codes, and the perturbation
> matrix. Read its §1 first — it corrects the quality gate for this work:
> plain `cargo test` runs **none** of the mock tests added in `1d224f4`.

The "Tier 1" slice of the ONVIF-depth push: finish the services oxvif already
owns rather than open new ones. Everything here lands inside an existing
`src/client/<service>.rs`; no new service module, no new dependency, no new
transport work.

Scope: **17 operations.**

| Stage | What | Ops |
|-------|------|----:|
| 0 | WSDL transcription for the types A–C need (prerequisite) | — |
| A | `GetServiceCapabilities` on all nine services | 9 |
| B | PTZ preset tours | 7 |
| C | PTZ-level `SendAuxiliaryCommand` | 1 |

Explicit non-goals for this release: Analytics, DeviceIO, Receiver, Profile
A/C/D, the metadata stream parser, and the Media1 `Set*/Add*/Remove*`
configuration family. Those are separate decisions; see §8.

---

## 1. Why these, and what they unlock

### The gap that motivated Stage A

`GetServiceCapabilities` is **not implemented on a single service** — a
repo-wide grep for the string returns nothing in `src/`, not even in the mock.
Every ONVIF service defines this operation. It is the standard way to ask one
service what it can do.

What oxvif has instead is the *device-level* `GetCapabilities` (`tds`), which
answers a much weaker question. Concretely, from `src/types/capabilities.rs`:

```rust
pub struct PtzCapabilities {
    /// PTZ service endpoint URL (`None` if not supported).
    pub url: Option<String>,
}
```

That is the whole thing. It tells you the PTZ service *exists*. It cannot tell
you whether this camera supports E-flip, reverse, `MoveStatus` reporting, or
preset tours — all of which live in `tptz:Capabilities`, which oxvif never asks
for.

### What Stage A buys beyond the operation count

This is the reason to do it first: it makes work already paid for stronger.

- **health check** currently reasons from service *presence*.
  `src/health/checks.rs:689` is the shape of it:

  ```rust
  if s.capabilities().ptz.url.is_none() {
  ```

  With per-service capabilities, health gains a genuinely new category:
  **claim-vs-behaviour**. "Advertises `MoveStatus` but `GetStatus` returns no
  `MoveStatus` element." "Advertises `WSPullPointSupport=false` but a
  pull-point subscription succeeded." Today those cameras look clean because
  nothing asks the claim.

- **metamorph quirk diff** gets the same upgrade for free. A clone records the
  capability claims; the diff against the synthetic (spec-ideal) mock surfaces
  every vendor that over- or under-claims. This is the sort of divergence
  quirk diff exists to find, and it is currently invisible to it.

Stage A is also the cheapest of the three: nine near-identical
empty-request/one-struct-response operations. Good place to establish the type
and test template that B and C then follow.

### Why preset tours (Stage B)

The largest user-visible feature gap in a service oxvif otherwise covers
18/29. Effectively every PTZ camera has a tour/cruise function and oxdm cannot
touch it. No new transport work — it is the same `call` → `parse_soap_body` →
`find_response` path as every other PTZ method.

### Why PTZ `SendAuxiliaryCommand` (Stage C)

Wiper, washer, IR lamp. One operation. Note `docs/reference/ptz.md:48` already
flags the trap: oxvif's existing `send_auxiliary_command` is the **Device**
operation. The PTZ one is a different operation on a different endpoint,
carries `tt:AuxiliaryData`, and **returns a payload** where the Device one
returns nothing. Cameras implement the wiper on the PTZ one.

---

## 2. Stage 0 — WSDL transcription (do this first, it is real work)

`docs/reference/` deliberately gives field detail only for *operations* oxvif
has not built; **complex types are pointers**. So the reference does not
currently contain enough to implement Stage A or B. Current state:

| Type | Reference today | Gap |
|------|-----------------|-----|
| `tds:DeviceServiceCapabilities` | `device.md:125` — "Network / Security / System sub-trees" | no field list |
| `trt:Capabilities` | `media1.md:88` — name only | no field list |
| `tr2:Capabilities2` | `media2.md:68` — name only | no field list |
| `tptz:Capabilities` | `ptz.md:56` — name only | no field list |
| `timg:Capabilities` | `imaging.md:32` — 3 attrs + "incl." | partial |
| `tev:Capabilities` | `events.md:46` — 6 attrs + "incl." | partial |
| `trc:Capabilities` | `recording.md:58` — 5 attrs + "…" | partial |
| `trp:Capabilities` | `replay.md:25` — 2 attrs + "…" | partial |
| `tse:Capabilities` | `search.md:29` — table row only | no field list |
| `tt:PresetTour` | `ptz.md:99` — pointer to onvif.xsd | no field list |
| `tt:PTZPresetTourOptions` | `ptz.md:99` — pointer | no field list |
| `tt:AuxiliaryData` | `ptz.md:99` — pointer | no field list |

**Task:** transcribe these twelve into their service files (or `types.md` for
the `tt:` ones), from the official WSDL/XSD, and cite the fetch date — the
existing convention at `docs/reference/README.md:16-17`.

That same convention is binding here:

> Anything not verified against the schema is marked `(unverified)` rather
> than guessed.

Two things already worth carrying into Stage 0:

- `media2.md:68` names the type **`tr2:Capabilities2`**, not `tr2:Capabilities`.
  If that is right it is a genuine Media2 oddity and the struct/parser must
  match; if it is a typo in our own reference, fix it there. Verify against the
  WSDL before writing any code.
- `imaging.md:32` writes the attribute as **`AdaptablePreset`**. Check the
  spelling against the schema — `AdaptivePresets` is also plausible, and an
  attribute name that is wrong by one letter parses as "absent" forever
  without ever failing a test.

Stage 0 output is a docs-only commit. It has no quality gate beyond the usual,
but it is what makes A and B mechanical instead of guesswork.

---

## 3. Stage A — `GetServiceCapabilities` × 9

### 3.1 The naming collision, and the decision

`src/types/capabilities.rs` already exports `PtzCapabilities`,
`MediaCapabilities`, `Media2Capabilities`, `EventsCapabilities`,
`ImagingCapabilities`, `RecordingCapabilities`, `SearchCapabilities`,
`ReplayCapabilities`. Every one of those is a **sub-tree of the device-level
`GetCapabilities` response** — a different operation, a different endpoint, and
a different (much thinner) shape.

**Decision: new types named `<Service>ServiceCapabilities`.**

```
PtzServiceCapabilities        MediaServiceCapabilities
Media2ServiceCapabilities     ImagingServiceCapabilities
EventsServiceCapabilities     RecordingServiceCapabilities
SearchServiceCapabilities     ReplayServiceCapabilities
DeviceServiceCapabilities
```

It mirrors the operation name, so the mapping is obvious, and it reads
correctly next to the old ones. Rejected alternatives: extending the existing
structs with an `Option<…>` field (wrong — it would silently imply the data
came from the same call, when it needs a second request to a different
endpoint), and any `*Caps2` form.

**The doc comment on each new type must name the operation it comes from, and
the old types' comments must be amended to name theirs.** Two similarly named
types in one crate, distinguished only by which request populates them, is
precisely the thing a reader gets wrong. This is a required part of the stage,
not a nicety.

Put them in a new `src/types/service_capabilities.rs` rather than growing
`capabilities.rs` (already 14.3K) — and the file boundary itself documents the
split. Wire up in `src/types/mod.rs` and re-export from `src/lib.rs` per SOP
steps 2 and 4.

### 3.2 `Option<bool>`, not `bool`

The existing device-level structs use bare `bool` defaulting to `false`. For
**service** capabilities that is the wrong choice, and it matters for the whole
point of the stage: health's claim-vs-behaviour checks need to tell
**"the camera said no"** from **"the camera didn't say"**. Collapsing both to
`false` produces false accusations against cameras that simply omitted an
optional attribute.

So: `Option<bool>` / `Option<u32>` throughout the new structs. This is a
deliberate divergence from the neighbouring file's style; note it in the
module doc comment so it does not read as an inconsistency to be "fixed"
later.

### 3.3 Method naming

Uniform prefixed form, one per service:

```
device_get_service_capabilities      media_get_service_capabilities
media2_get_service_capabilities      ptz_get_service_capabilities
imaging_get_service_capabilities     events_get_service_capabilities
recording_get_service_capabilities   search_get_service_capabilities
replay_get_service_capabilities
```

Media1's existing methods are unprefixed (`get_profiles`) while Media2's carry
a suffix (`get_profiles_media2`). Nine uniform names are worth more than
consistency with that legacy split; a bare `get_service_capabilities` would be
ambiguous about which of nine endpoints it hits.

### 3.4 Faults are expected, and must not be treated as failure

Plenty of shipping firmware answers `GetServiceCapabilities` with an
`ActionNotSupported` fault while implementing the service perfectly well. The
client method returns `Err` as normal, but **health must record that as
`Skip`/unknown, never `Fail`**, and metamorph must record the fault as the
recorded behaviour rather than dropping the operation. Call this out in the
health check when Stage A is wired in.

### 3.5 Per-service work list

Per service: one struct + `from_xml` in `src/types/service_capabilities.rs`,
one method in `src/client/<service>.rs`, one wrapper in `src/session.rs`
(pattern at `session.rs:1100`), one mock handler, one positive + one negative
test.

Note `dispatch.rs` currently routes recording/search/replay through a single
`dispatch_recording(op)` that takes no state or body
(`src/mock/dispatch.rs:23-27`). Three services sharing one dispatcher means
three distinct `GetServiceCapabilities` operations arrive with the **same `op`
string** and no way to tell them apart. Splitting that match on the action
prefix is part of Stage A, not an afterthought.

---

## 4. Stage B — PTZ preset tours (7 ops)

From `docs/reference/ptz.md:67-91`:

| Operation | Req | Resp |
|-----------|-----|------|
| `GetPresetTours` | `ProfileToken` [1] | `PresetTour` `tt:PresetTour` [0..*] |
| `GetPresetTour` | `ProfileToken` [1]; `PresetTourToken` [1] | `PresetTour` [1] |
| `GetPresetTourOptions` | `ProfileToken` [1]; `PresetTourToken` [0..1] | `Options` `tt:PTZPresetTourOptions` [1] |
| `CreatePresetTour` | `ProfileToken` [1] | `PresetTourToken` `tt:ReferenceToken` [1] |
| `ModifyPresetTour` | `ProfileToken` [1]; `PresetTour` [1] | _(empty)_ |
| `OperatePresetTour` | `ProfileToken` [1]; `PresetTourToken` [1]; `Operation` (`Start\|Stop\|Pause\|Extended`) [1] | _(empty)_ |
| `RemovePresetTour` | `ProfileToken` [1]; `PresetTourToken` [1] | _(empty)_ |

Types: `PtzPresetTour` and `PtzPresetTourOptions` into `src/types/ptz.rs`
(4.4K today, room to grow), following `PtzPreset` at `src/types/ptz.rs:23`.

Binding rules from CLAUDE.md that apply directly here:

- `PresetTour/@token` is required → `.ok_or_else(|| SoapError::missing("PresetTour/@token"))?`,
  never `unwrap_or("")`. Same for `TourSpot` tokens.
- `GetPresetTours` returns a collection, so the `.map(…)` closure returns
  `Result<T, OnvifError>` and `.collect()` propagates the first error — not
  `Ok(iter.map(…).collect())`.
- `ModifyPresetTour` serialises a whole `PresetTour` back out, so it needs a
  `to_xml_body()` and **every string field in it goes through `xml_escape()`** —
  tour names are user-supplied and this is the one operation in the stage that
  writes structured user data.

`ModifyPresetTour` is the most substantial single item in Tier 1: it is the
only operation that round-trips a non-trivial nested structure (status,
starting condition, and a repeated `TourSpot` list carrying preset detail,
speed, and stay time). Budget for it accordingly and write the round-trip test
first.

### Mock state

Tours are create/modify/remove, so unlike Stage A they need real state on
`DeviceState` in `src/mock/state.rs` (50.4K), not a canned response — a tour
created via `CreatePresetTour` has to come back from a subsequent
`GetPresetTours`, or the mock is not an integration harness for this feature.
`OperatePresetTour` should move a status field the way the existing
`SetRecordingJobMode` handler does.

---

## 5. Stage C — PTZ `SendAuxiliaryCommand` (1 op)

Req: `ProfileToken` [1], `AuxiliaryData` [1]. Resp: `AuxiliaryResponse`
`tt:AuxiliaryData` [1].

`tt:AuxiliaryData` is a plain string in the schema, but the *values* are
vendor namespaces (`tt:Wiper|On`, `tt:IRLamp|Auto`, …). Keep the API a
`&str` in / `String` out; do **not** invent an enum of commands. An enum here
would be a guess at a vendor-open set, and it would need a breaking change
every time a camera used a value we had not enumerated. Document a few common
values in the method doc comment instead.

`xml_escape` the command on the way in.

---

## 6. Testing

Per CLAUDE.md, per operation: one positive, one negative, both asserting
payload. All in `src/tests/client/ptz_tests.rs` etc., mirroring
`src/client/<service>.rs`.

- Positives assert **returned fields**, not `is_ok()`. For the write
  operations (`Create/Modify/Operate/RemovePresetTour`) use
  `RecordingTransport` and assert **both** `c.action` and `c.body` — for
  `ModifyPresetTour` that means asserting the serialised tour body, which is
  the only place the `to_xml_body()` escaping is actually pinned.
- Negatives assert the exact fault `code` **and** `reason`, or the exact
  `MissingField` path. Give every fixture a distinctive payload
  (`"NoSuchPresetTour-4471"`-style); `code` and `reason` must vary
  independently across the file.
- One test must pin that `GetPresetTours` **propagates** a missing
  `@token` on the *second* tour rather than returning the first — that is the
  specific bug the `vec_from_xml` rule exists to prevent, and a fixture with
  one good tour and one bad one is the only thing that catches it.

**Perturbation is required before commit** and is easy to shortcut on a batch
this size. Cheapest correct form for the whole stage: make
`SoapError::missing()` ignore its argument, run the suite unfiltered, and diff
the failing test **names** against the same run with the fault parser in
`src/soap/xml.rs` returning a constant `code`/`reason`. Every real negative
goes red under one or the other; anything that stays green in both is hollow
and has to be rewritten. Run it unfiltered — `cargo test <filter>` silently
skips the integration crates.

---

## 7. Per-stage completion

Commit at the end of each stage, not once at the end (CLAUDE.md commit
discipline). Gate before each: `cargo fmt`, `cargo clippy --all-targets --
-D warnings`, `cargo test`.

1. **Stage 0** — `docs(reference): transcribe … from WSDL`. Docs only.
2. **Stage A** — 9 ops. Includes the `dispatch_recording` split (§3.5) and the
   doc-comment amendment on the old `*Capabilities` types (§3.1).
3. **Stage B** — 7 ops + mock `DeviceState` for tours.
4. **Stage C** — 1 op.
5. **Release** — SOP steps 6, 6a, 7, then 8–17.

Release-time items that are easy to miss:

- `src/lib.rs` crate header (SOP 6a) — no new feature here, but the
  **Profile coverage table** moves and the PTZ prose changes.
- `examples/camera.rs` — new command + a `full_workflow()` section.
- `docs/reference/ptz.md:8` says `18 / 29`; after Tier 1 it is `26 / 29`.
  `media1.md:9`, `media2.md:8`, `events.md:9`, `imaging.md`, `recording.md`,
  `search.md`, `replay.md` headers all move too. These headers are the thing
  that goes stale silently — the ROADMAP was deleted for exactly this failure
  mode.
- README `Implemented ONVIF operations` tables + unit-test count.

---

## 8. Open questions

1. **Batch capability fetch.** Health wants all nine at once. A
   `session.service_capabilities()` returning one struct of nine `Option`s is
   the obvious convenience — but it is nine sequential round-trips against
   cameras that may fault on most of them, and it is not required by anything
   in Tier 1. Deliberately **not** in this plan. Decide when the health
   integration is written and the real access pattern is known, rather than
   guessing now.
2. **Whether health integration ships in 0.15 or waits.** Stage A only adds
   the client methods. Actually *using* them for claim-vs-behaviour checks is a
   separate body of work with its own baseline-diff implications (a new check
   changes every stored `HealthReport` baseline). Recommend: land Tier 1
   as 0.15.0, do the health integration as 0.16.0 alongside Analytics.
3. **`GeoMove` and `MoveAndStartTracking`** are the two remaining PTZ
   operations after Tier 1. Both need `tt:GeoLocation`, both are rare on real
   cameras. Left out on purpose; taking them would close PTZ at 29/29, which
   is the only argument for doing them.
