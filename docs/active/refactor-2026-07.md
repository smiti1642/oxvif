# Staged bug-fix programme — 2026-07

Origin: a six-agent read-only audit of `src/` (~37k lines) on 2026-07-25/26.
This doc is the **working contract** for the fixes that came out of it.

> **How to use this doc.** It is not a narrative. Before reviewing any stage's
> commit, walk [§5 Critic protocol](#5-critic-protocol) top to bottom and record
> the result. Before *starting* any stage, re-read [§6 Correction log](#6-correction-log)
> — every entry there is a mistake that was actually made in this programme,
> including by the reviewer. The log exists because context is lost between
> sessions and the same wrong assumptions come back.

---

## 1. Locked decisions

| Decision | Value | Rationale |
|---|---|---|
| Release | **single 0.14.0** | Stage 3 is breaking; a second release would mean running the 17-step SOP twice and moving oxdm's pin twice for a few days' head start. Stages are separate commits, so cutting a 0.13.1 from the Stage-2 commit stays possible if Stage 3 stalls. |
| Test-coverage scope | **64** — 28 with no real positive + 20 `is_err`-only + 16 variant-only hollow negatives | Decided 2026-07-26. Supersedes the earlier "26 + 21 = 47", whose halves were wrong in opposite directions (see §2.2 for the four-way reconciliation of the 148-method universe). All three classes *mislead a reader*: they read as green while asserting nothing that can fail. The remaining 68 methods have no negative at all — an absence visible in the ledger — and are deferred to §8. |
| Release timing | **the whole programme goes green first**, then one 0.14.0 | Re-affirmed 2026-07-26, with the alternative explicitly on the table and declined: Stages 0–3 are verified and are the entire user-visible content of 0.14.0, and Stage 4 changes no shipped behaviour, so shipping early was possible. Declined because the contract exists precisely to stop Stage 4 becoming the permanent next thing. Merge `refactor/2026-07` → `develop` only when all 64 are done. |
| Test layout | split by service; keep in `src/tests/` | `src/tests/client_tests.rs` uses zero `pub(crate)` internals so it *could* move to `tests/`, but that needs a several-hundred-site `crate::` → `oxvif::` rewrite, producing a diff too large to review for silently weakened assertions. Only the black-box mock snapshot moved to `tests/`. `src/tests/types_tests.rs` **cannot** move — 73 call sites of `pub(crate)` fns (46 `from_xml`, 15 `vec_from_xml`, 12 `to_xml_body`; all 84 definitions in `src/types/` are `pub(crate)`). |

**Layout as shipped in `e21ed7f`:** each `src/tests/client/<svc>_tests.rs` is attached
from the foot of the matching `src/client/<svc>.rs` via `#[cfg(test)] #[path]`, so tests
sit in the module they exercise (`client::device::tests::…`). Shared transports and the
two multi-service fixture helpers live in `src/tests/common.rs`, declared once from
`src/lib.rs` through `src/tests/mod.rs`. The black-box snapshot is
`tests/mock_action_snapshot.rs`, gated `#![cfg(feature = "mock")]`.

**Migration notes for the 0.14.0 release notes** (accumulating; the release SOP
writes `CHANGELOG.md`, the stages deliberately do not):

- *Stage 3.* **The on-disk format does not change** — `Fixture` already carries
  `action`, and `load()` rebuilds the index by re-`insert()`ing each entry
  (`src/metamorph/fixture.rs:98-100`), so old `fixtures.json` files keep loading.
  What an old clone cannot recover is the exchanges that were never written:
  **4 of the 64 exchanges in a recommended sweep** (measured, see D1). Those four
  replay as an honest miss after the fix instead of silently returning the other
  service's envelope, so an un-re-recorded clone gets *less* wrong, not more.
  Re-recording is still the recommendation, and it is the only way to fill the gap.
- *Stage 2.* `OnvifClient::get_discovery_mode` / `OnvifSession::get_discovery_mode`
  now return `Err(SoapError::MissingField("GetDiscoveryModeResponse/DiscoveryMode"))`
  where a device that omitted the element previously produced `Ok("")`. Signature
  unchanged; callers that read `""` as "unknown mode" now see an error.
- *Stage 2.* `SweepReport::is_complete()` returns `false` for an empty report
  instead of `true`. A caller gating on it with an empty selection flips.
- *Stage 3.* `FixtureStore::lookup` takes the SOAP action as its first argument —
  a compile break, deliberately, so the change cannot pass silently. **There is no
  compatibility shim and no deprecation cycle.** `lookup_by_key` existed only
  between `5d3fbc7` and `0c156b2`, both unreleased, so no published version ever
  offered it and nothing is owed to callers of it; it must not appear in the notes
  as an escape hatch. Decided 2026-07-26 on two grounds: a function marked
  deprecated *and* removed in the same release supports no one, and the only known
  consumer (`oxdm`) never called `lookup` at all. Callers get `E0061` and add the
  action. This also retires the earlier release-time to-do about the shim's
  `since = "0.14.0"` string — there is no such attribute left to check.

---

## 2. Stages

| # | Content | Nature | Status |
|---|---|---|---|
| 0 | Regression safety net (tests only) | additive | **done** — `1e8d634` |
| 0.5 | Split client tests by service; move mock snapshot to `tests/` | pure move | **done** — `e21ed7f` |
| 1a | Split the two collapsed `dispatch.rs` arms; six operations start working | non-breaking | **done** — `894b865` |
| 1b | `AudioEncoderConfiguration::to_xml_body_media2()`; `xml_escape` on 4 encoding sites | non-breaking | **done** — `573168a` |
| 2 | `get_discovery_mode` strictness; `is_complete()` empty-report case | behaviour change | **done** — `ddfde44` |
| 3 | Fixture key → `(action, key_canon)`, in two steps | **breaking** | **done** — `5d3fbc7` (step 1) + `0c156b2` (step 2) |
| 4 | Positive+negative pairs for **64** methods (28 no-positive + 20 `is_err`-hollow + 16 variant-only) | additive | ledger `c903816`; batch 1 recording 15 — `1c01977`; batch 2 media+media2 22 — `71e349d` + `3c8b420`; **27 left** (device 14 + mod 2, ptz 8, events 1 + imaging 2) |

Verdicts for every finished stage are in [§9](#9-stage-verdicts) — what each one
actually rested on, not just that it passed. (Stage 4 is now the only one left,
and its prerequisite is C11b: regenerate the coverage ledger against a
**committed** ref before the stage starts.)

### 2.1 Stage 2 — the API decision

**Decided 2026-07-26: option A — `Err` on a missing or empty `DiscoveryMode`,
signature unchanged.** Concretely
`SoapError::missing("GetDiscoveryModeResponse/DiscoveryMode")`, with an empty
text node treated as missing per the CLAUDE.md `.filter(|t| !t.is_empty())` idiom,
so `Ok("")` becomes unreachable rather than merely undocumented. The pin
`get_discovery_mode_without_the_element_yields_an_empty_string`
(`src/tests/client/device_tests.rs:1765`) exists to be flipped by this stage;
`get_discovery_mode_pins_action_body_and_parsed_value` next to it must survive
untouched. Rationale below.

*Shipped in `ddfde44`.* The whitespace-only half of the decision holds only
because `XmlNode::parse` collapses blank text to `None` when it trims at
`Event::End` (`src/soap/xml.rs:130-137`) — a property of another module that **no
test defended**. It is now pinned at both altitudes, which is why the stage
touches a fourth file (`src/soap/xml.rs`, tests only, hunk after its `#[cfg(test)]`
at `:329`). See §9 and C16.

`get_discovery_mode` (`src/client/device.rs:838-851`) documents its return as
`"Discoverable"` or `"NonDiscoverable"` and then does
`.map(|n| n.text().to_string()).unwrap_or_default()` — a device that omits
`<tds:DiscoveryMode>` yields `Ok("")`, a third value the doc denies exists.

| option | signature | cost |
|---|---|---|
| **A — `Err` on missing** (reviewer's recommendation) | unchanged `Result<String, OnvifError>` | smallest diff; matches the CLAUDE.md rule "required fields must return `Result`, never silently default to an empty string"; `src/types/` has no precedent for opening an enum over a single device field |
| B — introduce `DiscoveryMode` enum | `Result<DiscoveryMode, OnvifError>` | rules the third value out at the type level, but expands the public API and must then decide what an unknown-but-present string does (`Other(String)`? `Err`?) — which reintroduces A's question one layer down |

Either way `SweepReport::is_complete()` (`src/metamorph/surface.rs:414`) is
vacuously `true` on an empty `outcomes` map (D11); that half has no API question
attached and no dependency on this one.

Note that Stage 2 is the only stage whose commit could be cut as a standalone
0.13.1 if Stage 3 stalls — see §1. That does not change the decision, but it does
mean the answer lands in released docs.

### 2.2 Stage 4 scope

**Universe: 148 methods.** 149 `pub fn` in `src/client/` minus the free function
`notification_listener` (`src/client/events.rs:321`, outside every `impl` block).
Reconciled four ways: reviewer's Grep 149, agent's line-anchored Grep 149, a token
parser 150 (the extra is `pub(crate) async fn call`, `mod.rs:144`, not public), and
142 `pub async fn` + 6 non-async methods. `src/session.rs` mirrors all 142 async
ones 1:1. **`148` is the count to cite; `142` counts only the async half.**

**The ledger was regenerated at `c903816` (C11b satisfied) and is committed at
[`docs/active/stage4-ledger.md`](stage4-ledger.md)** — 148 rows with per-method
evidence, quoting the assertion for every row scored `hollow`, `weak` or `unsure`.
It is in the repo rather than a scratchpad on purpose: Stage 4 runs in batches,
possibly across sessions, and a work list that evaporates is a work list that gets
re-derived wrongly. Re-measure it against the then-current ref before each batch;
do not work from the summary below alone.

| class | count |
|---|---|
| fully compliant (real positive **and** real negative) | **13** |
| no real positive (27 zero + 1 weak) | 28 |
| negative is `assert!(res.is_err())` only | 20 methods / **21 sites** |
| negative asserts the variant but no payload (`Fault { .. }`, `MissingField(_)`) | 16 |
| no negative test at all | 94 |
| infallible, exempt | 3 (`new`, `with_credentials`, `with_transport`) |

**Decided 2026-07-26: Stage 4 covers 64 = 28 + 20 + 16.** The two hollow classes are
one defect wearing two costumes — *a negative test that does not assert what went
wrong* — and the earlier "21" undercounted only because it was gathered by grepping
`is_err()`. Excluding the 16 would let the stage claim it cleared the hollow class
while leaving 16 known-hollow tests in place. They are also the cheapest rows in the
ledger: `get_storage_configurations` is one `assert_eq!` on the field path.
Template: **assert the payload, not the variant** (§9).

Full CLAUDE.md compliance is **132** methods, not the 116 previously stated
(`148 − 13 compliant − 3 exempt`). The 68 beyond this stage's 64 — almost all
"never had a negative at all" — stay deferred in §8. **Stage 4 must not be described
afterwards as having made the crate rule-compliant.** Stage 4 ships inside 0.14.0.

Superseded numbers, kept so the drift is visible: the first survey scored
`covered 32 / partial 90 / zero 26` and set the scope at 47. Regenerated, `covered`
is 13 strict (29 if variant-only counts), and the scope is 64. The `32` is the one
figure that could not be reconciled at all — see the ledger's §"unreconcilable".
Stages 1a–3 cleared exactly **two** rows: `imaging_stop` (1a) and
`get_discovery_mode` (2). Every one of the 21 hollow sites is still hollow.

Findings that shape the batching:
- **PTZ has zero compliant methods out of 18** — five zero-positive, one weak
  (`ptz_stop`: called at `session_tests.rs:498` and `.unwrap()`ed with no assertion,
  so it is `weak`, not `zero` as first recorded), the rest lacking a real negative.
- **Recording is the most dangerous cluster: 15 hollow, not 12.** The regenerated
  ledger found three the first survey missed (`get_recordings`, `find_recordings`,
  `get_replay_uri`). Most feed a real `make_soap_fault_xml` and assert only
  `is_err()`, so turning a Fault into `UnexpectedResponse` leaves them all green.
- **`media` and `media2` must go to the same agent** — three Media1 setters have
  their tests in `media2_tests.rs`.
- `with_utc_offset` and `device_url` have **no call site in any test at all**, not
  even in the snapshot net. Both are infallible, so they need a positive only.
- `get_capabilities` has **six** tests and is still not compliant: its negative
  asserts `SoapError::Fault { .. }` with neither code nor reason. A high test count
  is not coverage.
- `set_video_encoder_configuration`'s only negative is a client-side
  `InvalidArgument` gate that never reaches the transport — it counts, but the
  method has no SOAP-Fault negative.
- Outside the universe by definition, but untested all the same: the free function
  `notification_listener` has no unit test, only a snapshot call site.

Per-service locked scope (28 + 20 + 16 = 64): recording 15, media2 12, device 14,
media 10, ptz 8, mod 2, events 1, imaging 2. Batch recording first — one file,
mechanically uniform, largest single win.

**Ordering constraints (not preferences):**
- Stage 0 had to complete alone — it photographs current behaviour, so a
  concurrent behaviour change would corrupt the baseline.
- Stage 0.5 must land before Stage 1 — a large mechanical file move conflicts
  with concurrent test edits.
- Stages 1a and 1b are disjoint in file scope — 1a touches only `src/mock/`, 1b
  touches `src/types/{audio,video}.rs` + `src/client/media2.rs` — but they must
  still be **serialised**, because they share one working tree: two agents each
  running `cargo test` see each other's edits, which destroys red-before-green
  verification, and they block on the same cargo target lock. Run them in sequence,
  or give each its own worktree. Only genuinely read-only work may run alongside a
  writing stage. **1b must be one unit** — splitting it would put two agents in
  `src/types/audio.rs` at once.
- Stage 4 must run last. Earlier stages change what the correct assertion *is*
  (e.g. after 1b, `set_audio_encoder_configuration_media2`'s positive test asserts
  `tr2:`, not `trt:`), so writing those tests earlier wastes them.
  *Datum for Stage 4, found while mutation-testing 0.5:* `ptz_get_presets` has **no
  unit-level test at all** — breaking its response tag left all 23 `client::ptz`
  tests green, and only `tests/mock_action_snapshot.rs` caught it. Concrete
  confirmation of C6: the snapshot creates a call site, not a positive/negative pair.
- Stage 3 step 1 adds the new `lookup` signature with the old kept as a
  deprecated shim; step 2 removes the shim. Test-first is impossible in one step
  because the target test cannot compile against a signature that does not exist.
  **The shim cannot be correct** — the old signature has no `action` to
  disambiguate with, so it can only keep today's ambiguous "first match by key"
  behaviour. Its `#[deprecated]` note must say that outright, not just "use the
  new API": a caller who reads it as an equivalent rename keeps the bug.
  Three facts that shrink this stage, all verified 2026-07-26:
  `ReplayResponder::respond` already holds `ctx.action` (it calls
  `operation(ctx.action)` at `src/metamorph/replay.rs:47`), so no plumbing is
  needed; `record()` already takes `action` and merely fails to key on it; and the
  *reporting* layer already treats the pair as the identity
  (`src/metamorph/fixture.rs:47`). The break is confined to one public function.
  *Outcome (`5d3fbc7` + `0c156b2`):* both steps shipped as planned, and the shim
  is **gone** — see §1 for why it got no deprecation cycle. The paragraph above
  describes how the break was staged, not a shim that still exists.

---

## 3. Confirmed defects

Every row was verified by opening the cited line. "Empirical" means it was
reproduced by running code, not by reading it.

### Tier 1 — reproduced by running code

**D1 · Fixture key omits the SOAP action → silent data loss and wrong replay answers.**
`src/metamorph/fixture.rs:123` keys on `canonicalize(request_raw, Masking::Key)`
alone (`record()` at `:122` takes `action` but never feeds it into the key) and
`insert()` upserts — the doc comment at `:118-121` says "last write wins" outright. `src/soap/xml.rs:215` keeps only `local_name`;
`:221-226` drops `xmlns` declarations; `src/mock/canon.rs:100` writes the bare
local name. So `<trt:GetProfiles/>` (`src/client/media.rs:21`) and
`<tr2:GetProfiles/>` (`src/client/media2.rs:24`) collapse to one key.

*Reproduced:* recording both actions into a `FixtureStore` yields `len() == 1`,
Media2 surviving. Worse than loss — `src/metamorph/replay.rs:59` also looks up by
key alone and `find_response` is prefix-agnostic, so a Media1 read replays the
Media2 envelope, **parses successfully, and returns wrong data** while
`SweepReport` still reports `Recorded`.

*Measured 2026-07-26* (sandbox, scratch tap over a `MockServer` sweep, reverted
after): a `SurfaceSelection::recommended()` sweep issues **64 exchanges** that
collapse to **59 keys**. Four of those five collapses are cross-action — real
loss — and they are exactly the Media1/Media2 pairs whose canonical bodies match:

| colliding key | actions |
|---|---|
| `<GetProfiles/>` | `ver10/media/wsdl/GetProfiles`, `ver20/media/wsdl/GetProfiles` |
| `<GetVideoSourceConfigurations/>` | ver10, ver20 |
| `<GetVideoEncoderConfigurations/>` | ver10, ver20 |
| `<GetVideoEncoderConfigurationOptions/>` | ver10, ver20 |

`GetStreamUri` and `GetSnapshotUri` do **not** collide despite sharing a local
name — Media1 sends `StreamSetup/Stream/Transport`, Media2 sends `Protocol`, so
the bodies differ. The fifth collapse is same-action and is the legitimate
ephemera de-dup. Note what the key looks like:
`<Envelope><Header><To>__MASKED__</To></Header><Body><GetProfiles/></Body></Envelope>`
— the endpoint URL, the one field that *would* have separated the two services,
is masked as transport ephemera. That is why the action has to enter the key
rather than the masking being loosened.

Since the key derives from the request the *client* builds, this measurement is
device-independent; a real device only changes how many operations get swept.

Must preserve: ephemera de-duplication (`src/metamorph/fixture.rs`,
`ephemera_jitter_does_not_fragment_the_key`) — two records differing only in
`MessageID` must still collapse to one. The measurement above already contains
one such legitimate collapse, so a fix that keys on the raw request would take
the count from 59 to 60 and *look* like it lost nothing while breaking dedup.

**D2 · Six mock operations return response elements that exist in no ONVIF WSDL.**
`src/mock/dispatch.rs:102-105` collapses four ops onto `"ConfigurationResponse"`;
`:192` collapses Move/Stop onto `"ImagingResponse"`.

*Reproduced:* all six fail with `SoapError::UnexpectedResponse` against
`MockTransport` while control operations on the same transport succeed.
Expected tags: `src/client/media.rs:175,195,220,239`,
`src/client/imaging.rs:106,125`.

### Tier 2 — verified by reading the cited lines

| ID | Defect | Citation |
|---|---|---|
| D3 | `AudioEncoderConfiguration::to_xml_body()` emits `trt:` into a `tr2:` Media2 request. The fix pattern already exists in-tree: `VideoSourceConfiguration` carries two serialisers for exactly this reason. | `src/types/audio.rs:160` × `src/client/media2.rs:514`; precedent `src/types/video.rs:157` / `:177` |
| D4 | `xml_escape` bypassed via `Display` — `Other(String)` returns the device's raw string. Invisible to a `grep xml_escape` audit. **Fixed in `573168a` at four sites, not three** — D3's new `to_xml_body_media2()` creates a fourth interpolation. All four now escape `self.encoding.as_str()`; `Display` itself is deliberately left unescaped and pinned by `hostile_encoding_reaches_display_unescaped`. | `src/types/audio.rs:99`,`:175`,`:202`; `src/types/video.rs:534`,`:753` |
| D5 | `get_discovery_mode` doc promises one of two strings; code can return `""`. **Fixed in `ddfde44`** — missing, empty and whitespace-only all become `SoapError::missing("GetDiscoveryModeResponse/DiscoveryMode")`; signature unchanged. | `src/client/device.rs:840` vs `:847-850` |
| D6 | `Transport` 400-handling contract undocumented at the trait, and contradicted by three doc sites that say 200/500 only. | `src/transport.rs:11-15`, `:40-41`, `src/error.rs:19-21` vs code at `src/transport.rs:163` |
| D7 | Non-reqwest `diqwest` errors reported as a fabricated HTTP 401. | `src/transport.rs:138-144` |
| D8 | Devices with an empty endpoint overwrite each other; only the first survives. Strict path only — the lenient path already rejects them. | `src/discovery.rs:61` + `:283` vs `:638-643` |
| D9 | `listen()` aborts the whole window on one transient recv error; `probe_once` explicitly does not. | `src/discovery.rs:493` vs `:420` |
| D10 | `CapturingTransport` writes WS-Security digests and RTSP credentials verbatim, and the module doc tells the user to commit them. The successor module spent ~100 lines solving this. | `src/fixtures.rs:73`,`:78` vs `:15`; cf. `src/metamorph/fixture.rs:178-266` |
| D11 | `SweepReport::is_complete()` is vacuously `true` on an empty sweep. **Fixed in `ddfde44`** — it now requires a non-empty report; no production caller existed either way. | `src/metamorph/surface.rs:414` |
| D12 | `SoapError::InvalidValue` is never constructed; `src/health/report.rs:178` has an unreachable match arm for it. | `src/soap/error.rs:45`,`:55` |

### Rejected / downgraded

- **Mock dispatch is *not* shotgun surgery.** Adding an operation touches
  **2 files** (one `dispatch.rs` arm + one `resp_*`/`handle_*` fn), or 1 for a
  void op. No registry refactor. *This was the reviewer's wrong premise.*
- **The four `*_with_progress` variants do not duplicate their twins** — each
  plain form is a one-line delegation (`src/metamorph/surface.rs:458`,
  `src/metamorph/record.rs:126`, `src/metamorph/parse.rs:167`,
  `src/metamorph/quirk.rs:280`). No divergence risk. Concern closed.
- **`SurfaceSelection::recommended() == all()`** is intentional API, not a bug.
- **A `#[derive(FromXml)]` for `src/types/` is not recommended.** Per-field
  strictness genuinely varies and fallback chains carry interop rationale
  comments that must stay adjacent to the code.

---

## 4. Docs that are wrong and must be fixed with Stage 3

Both were written by the reviewer in a previous session and both are load-bearing
for D1:

- `src/metamorph/parse.rs:522-523` — "Real recordings carry the operation
  element, which is what keeps them distinct here too." **False.** Media1 and
  Media2 share the operation element's local name, and the canonicaliser keeps
  only the local name.
- `src/metamorph/quirk.rs:128-129` — "`key_canon` alone is in fact unique
  *within* one store (it is `FixtureStore`'s index key)". Literally true but it
  conceals D1: uniqueness is maintained by silently destroying one side.
- `src/metamorph/mod.rs:21-24` and `src/metamorph/fixture.rs:1-8` make the same
  one-sided collision-safety claim.

---

## 5. Critic protocol

Run **all** of this per stage. Record pass/fail in the stage's review, not from memory.

**A. Boundary**
1. `git show --stat HEAD` — do the touched files match the stage's declared scope?
2. `git show -U0 HEAD | grep -c '^-[^-]'` — deleted lines. For a tests-only or
   pure-move stage this must be 0 (a move uses `git mv`, so renames show as such).
3. For additions inside production files, confirm each hunk's start line falls
   **after** that file's first `cfg(test)` marker.

**B. Test integrity**
4. `cargo test --all-features -- --list` before and after; diff the name set.
   Nothing may vanish. Module-path prefixes may change.
5. Read every changed assertion. Ban list: `assert!(x.is_ok())` with nothing
   checked after it; substring assertions on a value that appears incidentally;
   assertions inside a branch or loop that may not execute; a "negative" test
   that is not actually negative.

**C. Mutation kill (the main instrument)**

> **Mutate only in the sandbox worktree — never in the main checkout.**
> `C:/Users/Null/Documents/GitHub/oxvif-mut` is a detached-HEAD worktree of this
> repo carrying a `pre-commit` hook (scoped to it via `git config --worktree
> core.hooksPath`) that refuses every commit. Reason: a mutation left in the main
> tree can be swept into someone else's commit — that already happened once, when
> a `git add` picked up a concurrent agent's staged `git rm`. Isolation also lets
> a mutation run while agents work in the main tree.
>
> ```sh
> cd C:/Users/Null/Documents/GitHub/oxvif-mut
> git fetch . refactor/2026-07 && git checkout --detach FETCH_HEAD   # sync to the commit under review
> # …mutate, test, then:
> git checkout -- . && git status --short                            # must be clean
> ```

6. Pick a mutation point **the agent did not use** — an agent that verifies its
   own net tends to pick the one case it had in mind.
7. Break production code, run the test suite **unfiltered** (see C10 — a filtered
   `cargo test` silently drops the integration crates), confirm red, then
   `git checkout -- .` and confirm `git status` clean.
8. At least one mutation per net/fix. Prefer breaking something that currently
   *works* over flipping something already known broken.
8b. **The mutation must compile**, or it measures the compiler and not the tests
   (C14). Inside a `format!` block prefer substitution — retag `<tt:Channels>` to
   `<tt:Channel>` — over deletion, which orphans the named argument and is
   rejected by rustc before a single test runs.

**D. Red-before-green (Stages 1–4)**
9. Require the agent to paste the actual pre-fix failure output, not a claim that
   it wrote the test first.
10. Independently re-verify: revert only the production half and confirm the
    target test goes red.

**E. Provenance**
11. `git log -1 --format='%an <%ae>'` must be `smiti1642 <smiti1642@gmail.com>`.
12. Commit message body ASCII-only (see §7).

**F. Cross-check against this doc**
13. Does the stage's claim contradict anything in §3? If an agent reports a
    finding that §3 already rejected, the burden is on the new evidence.
14. Did the stage introduce a fact that belongs in §6? Add it.

---

## 6. Correction log

Mistakes actually made in this programme. Re-read before each stage.

| # | Wrong belief | Truth | Who |
|---|---|---|---|
| C1 | `examples/conformance.rs` needs `required-features = ["health"]` | It needs **`["mock"]`** — `CapturingTransport` is gated on `mock` (`src/lib.rs:204-205`), and the file's own doc at `examples/conformance.rs:10` already says so. No `[[example]]` stanza exists for it. | reviewer |
| C2 | Adding a mock operation is shotgun surgery across many files | It is 2 files, sometimes 1. | reviewer |
| C3 | Stage 1a would clear ~6 methods from the zero-coverage list, leaving 18 | Only `imaging_stop` overlapped. The other five broken ops already had `RecordingTransport` body tests — which is precisely why the mock breakage went unnoticed. | reviewer |
| C4 | `key_canon` alone is not unique across actions | It *is* the store's index key — but only because collisions destroy one side (D1). Both halves of the earlier statement were misleading. | reviewer |
| C5 | "grep for unescaped `{var}` in XML" finds escaping holes | `xml_escape` is applied three different ways — shadowing `let` (63 sites), named `format!` arg (15), inline. A naive grep produces false positives *and* misses `Display`-laundered values (D4). | reviewer |
| C6 | "has a call site in the test suite" == "has test coverage" | The CLAUDE.md rule requires a **positive and a negative** test per method. Snapshot nets create call sites without creating pairs; counting by grep overstates coverage. | reviewer |
| C7 | A test helper claiming to build N cases actually builds N | One built 3 fixtures that collided to 1 under D1's keying, so two thirds of the test body never ran. Whenever a test asserts a count, verify the count is *constructed*, not assumed. | agent |
| C8 | The counts written into this doc were measured | Three were not, and all three came from shell `grep`, which §7.1 shows returns 0 on parenthesised patterns: "99 uses of `pub(crate)`" (real: 73), "16 lock poison sites" (real: 25), `src/lib.rs:200` (real: `:204-205`). **A tool that fails by returning `0`/nothing cannot be distinguished from a true negative.** Re-measure every number in this doc with the Grep tool before citing it. | reviewer |
| C9 | rtk only mangles *search patterns* | It also **truncates command output** and appends a fake cargo-style summary (§7.2). Detected only because a 676-test baseline came back as 373. Any evidence gathered through rtk that "looks a bit short" is short. | reviewer |
| C10 | `cargo test <filter>` is enough to prove a mutation was caught | It silently **excludes the integration crates** ("2 filtered out"). Mutation D (`GetPresetsResponse` typo) read as *not caught* under `cargo test --all-features client::ptz`, and as caught only under the unfiltered run. Always run the mutation check unfiltered, or the net looks weaker than it is. | reviewer |
| C11 | Disjoint file scope means two stages can run concurrently | Not in one working tree. Both agents run `cargo test`, so each sees the other's half-finished edits and red-before-green becomes unprovable. Serialise, or give each agent its own worktree. | reviewer |
| C11b | *(amending C11)* Read-only work may safely run alongside a writing stage | **No.** A read-only *analysis* of a tree being written produces a ledger of a state that never existed. The Stage 4 survey watched `imaging_tests.rs` grow 495 → 588 lines between two of its own tool calls, and only caught it by cross-checking `--list` against the file it had just read. Read-only work must run against a **committed** ref, not the live tree. | reviewer |
| C12 | Counting Rust items with a line pattern is fine | `grep -c '^    ("'` gave **137** `EXPECTED` rows; the real count is **141**. The six missing rows are rustfmt-wrapped across lines *because their values are long* — i.e. the exact six broken entries the count existed to track. Count with a brace-matching parser, and prefer asserting a post-condition ("all 141 rows are `ok`") over a delta ("6 rows changed"). | reviewer |
| C13 | `git checkout -- .` undoes a mutation | Not if the mutation was applied with `git checkout <commit> -- <path>`, which **stages** it. `checkout -- .` then restores the worktree *from the poisoned index*, and `git diff` reads clean while `git status` shows `M ` (staged). Undo with `git checkout HEAD -- <path>` and verify with `git diff HEAD --stat`, not `git diff`. | reviewer |
| C14 | Any mutation of the fixed code proves the net | Only if it **compiles**. Deleting `<tt:Channels>{channels}</tt:Channels>` from the new Media2 serialiser left `channels = self.channels` as an unused named `format!` argument and rustc rejected the build — zero tests ran, so the mutation said nothing about the suite. That the compiler happened to be a stronger net *there* does not transfer to the next site. Retagging `Channels` → `Channel` compiles, and went red in 4 tests. | reviewer |
| C15 | A name vanishing from `-- --list` means a test was deleted | **Doc-test names embed a line number** (`src/metamorph/surface.rs - metamorph::surface::drive_surface_with_progress (line 514)`). Any doc-comment edit above a doc test makes its name vanish and a near-identical one appear — indistinguishable from a deletion unless you match on name-minus-line. Stage 2's 4 added doc lines moved that test 514 → 518. Stages 3 and 4 edit many doc comments, so expect this. | agent |
| C17 | A test marked "do not edit" should come out of the stage byte-identical | Not when the stage changes a **public signature** — that mechanically rewrites every call site, including in tests whose subject is unrelated (Stage 3 step 1 had to touch both ephemera de-dup tests for exactly this reason). Read the instruction as "do not weaken its assertions" and review by diffing the **assertion set**, not the byte count. The check that matters: does the test still fail for the reason it was written? | agent |
| C19 | Giving an agent an isolated worktree guarantees it analyses the right tree | It guarantees only that the tree **stops moving**. The Stage 4 ledger agent was handed a worktree created from `5789f41` — a stale `develop` commit predating the whole programme, with no `src/tests/client/`, no `tests/`, and no copy of this document. Frozen, and frozen at a tree Stage 4 will never touch: C11b's failure mode inverted. The agent caught it and re-detached to the programme tip. **Isolation addresses drift, not provenance** — require the agent to report the SHA it measured, and check that SHA against the ref you meant. | reviewer |
| C18 | A mutation's red **count** from an earlier stage is a reusable expectation | It is not — it is a measurement of one tree. Replaying Stage 3 step 1's mutations after step 2, the reviewer's first draft asserted "expect 13 again"; the deleted shim test had itself been red under both mutations, so the true answer was 12 and the hard-coded expectation would have flagged a clean commit as a weakened net. Re-measure the baseline on the old ref and diff the red **name sets** — the invariant worth asserting is *which* tests defend a fix, not how many. | reviewer |
| C21 | Dropping the filter is enough to make a mutation check see the whole suite | It is not. `cargo test --all-features` **aborts after the first failing target**, so the moment the lib tests go red the integration crates are never built or run — a mutation killed only by `tests/mock_action_snapshot.rs` reads as killed by nothing. Distinct from C10, which is about a *filter* dropping those crates; this one drops them because the mutation worked. Batch 2's agent hit it on its first round and switched to `--no-fail-fast`; the reviewer's own batch-1 driver had the same hole and was re-run. **Mutation checks must be `--all-features --no-fail-fast`, and the run must report how many targets reported a result** (4 here) — a red count alone cannot distinguish "nothing else caught it" from "nothing else ran". | agent |
| C20 | `get_capabilities` is a hollow negative — it asserts only `Fault { .. }` | The ledger's `yes / yes` is right and the reviewer's citation was wrong. `device_tests.rs` holds **two** fault tests for it: `..._returns_error` at `:75`, which asserts the bare variant, and `..._returns_err` at `:361`, which asserts `code == "s:Sender"` — and a third pins `HttpStatus { status: 401 }`. A method's class is its **strongest** test; quoting one assertion without sweeping its siblings misreads it, and the names differ by one letter. Batch 1's fault mutation settles this objectively: `:361` is red at the baseline, so `get_capabilities` was never in scope. (The weaker `:75` is pre-existing dead weight; not this programme's to remove.) | reviewer |
| C16 | Every new test must be shown red before the fix | Not one whose subject is "property X still holds". A cross-module premise guard is **green the moment it is written**, because the premise already holds — red-before-green proves nothing and accepting it green is indistinguishable from accepting a vacuous test. Validate it by **mutating the module that owns the property** and naming the expected victims. Stage 2's two whitespace pins were the only 2 of 654 lib tests that caught a mutation of the `Event::End` trim; 1b's `hostile_encoding_reaches_display_unescaped` is the same shape, with the method left implicit. | agent |

---

## 7. Environment facts that bite

- **Branch layout.** This programme lives on **`refactor/2026-07`**, branched from
  `f94a747` (= `origin/develop`). `develop` was rewound to exactly its remote so
  the staged work cannot leak into it; nothing here has been pushed. Merge back to
  `develop` only when the whole programme is green, then follow the release SOP.
- **Mutation sandbox:** `C:/Users/Null/Documents/GitHub/oxvif-mut`, a detached-HEAD
  worktree with a commit-refusing hook. See §5 C. Keep it out of the main tree.
- **`oxdm` builds against this branch, not against a release.** `oxdm/Cargo.toml`
  overrides `oxvif` with `path = "../oxvif"` — the *main worktree* — at both `:31`
  and `:70` (dev-deps). Two consequences: any oxdm build during this programme
  compiles mid-programme code, and, usefully, `cargo check --all-features` in
  `oxdm` is a free downstream check for a breaking stage. Run it before declaring
  a breaking stage done; it took 23 s at Stage 3 step 1.

- **rtk silently corrupts two things. Both produce plausible wrong answers, not errors.**
  1. *Regex mangling.* `grep -n '#\[cfg(test)\]'`, `grep 'x>{y}</x'` and
     `grep -rE '\.lock\(\)\.unwrap\(\)'` all return **0 matches** through the rtk
     proxy while the real count is non-zero (the last one: real answer 105). The
     trigger is any pattern containing `[ ] { } ( )`. Use the **Grep tool**, never
     shell `grep`, for these.
  2. *Output truncation.* `cargo test --all-features -- --list` through rtk emitted
     **373** of 676 lines and capped it with its own summary line
     `cargo test: 5 errors, 0 warnings (0 crates)` — which reads like cargo output
     but is not. This would have silently corrupted Critic check #4. For any command
     whose **full** output is the evidence, run it as `rtk proxy <cmd>`.
- **Windows console mangles UTF-8 commit messages.** Use
  `git -c i18n.commitEncoding=UTF-8 commit -F -` with an **ASCII-only** body.
- **Agent worktrees are not created from your `HEAD`.** The Stage 4 ledger agent's
  isolated worktree came from `5789f41` (stale `develop`), not the branch tip. Any
  agent given `isolation: worktree` must be told to `git checkout --detach` the
  intended ref and to **report the SHA it actually measured** (C19). Verify it:
  `git worktree list` names every worktree and its commit.
- **Known-red baseline: feature-free `--all-targets` has _two_ failure sources.**
  `error[E0432]: unresolved import oxvif::CapturingTransport` in
  `examples/conformance.rs` (C1), **and** `error: unused import: std::sync::Arc`
  at `src/tests/client/ptz_tests.rs:5`, which only surfaces without features.
  Both unrelated to this programme; do not fix either inside a stage. The second
  was missing from this note until Stage 3 step 2 — so "feature-free clippy is red"
  was not, on its own, evidence that a stage had introduced nothing: one new error
  could have hidden in a baseline believed to hold exactly one. Compare the error
  *list*, not the exit code. CI only ever builds `--all-features`, so this whole
  class of breakage is structurally invisible to CI.
- **Gate for every stage:**
  ```
  cargo fmt
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --all-features
  cargo test --all-features --doc
  ```
- **Test totals move every stage — re-baseline before each review, from a *clean*
  tree.** `-- --list` counts 2 `ignored` doc tests that the pass count omits, so
  the listed number is always 2 above the passing number. Check #4 compares the
  *listed name set*.

  | at | listed | passing | split (lib / integration / doc) |
  |---|---|---|---|
  | `7daa4ac` (pre-0.5) | 676 | 674 | 629 / 9 / 36 |
  | `e21ed7f` (post-0.5) | 676 | 674 | 627 / 2 + 9 / 36 |
  | `894b865` (post-1a) | 688 | 686 | 640 / 1 + 9 / 36 |
  | `573168a` (post-1b) | 698 | 696 | 650 / 1 + 9 / 36 |
  | `ddfde44` (post-2) | 702 | 700 | 654 / 1 + 9 / 36 |
  | `5d3fbc7` (post-3 step 1) | 707 | 705 | 659 / 1 + 9 / 36 |
  | `0c156b2` (post-3 step 2) | 706 | 704 | 658 / 1 + 9 / 36 |

  The 0.5 → 1a delta is −1 (`known_broken_mock_actions_are_pinned`, deleted by
  design) +13. The 1a → 1b delta is −1 +11, where the −1 is a *rename*:
  `…_emits_trt_configuration_known_bug` → `…_emits_tr2_configuration`, the pin
  this stage existed to flip. The 1b → 2 delta is −2 +6, where one −1 is again a
  rename (`…_yields_an_empty_string` → `…_is_a_missing_field_error`, Stage 0's
  pin) and the other is **not a deletion at all** but a doc-test line-number shift
  (C15). Baseline sets live at `<scratchpad>/{baseline,after,1a,1b}-tests.txt` and
  `<scratchpad>/names-{1a,1b,2}.txt`.

---

## 8. Known gaps, deliberately not addressed here

Real findings from the audit that are out of this programme's scope. Do not let
them silently become in-scope; open a separate plan.

- `src/discovery.rs` fuses seven concerns in 826 implementation lines, including
  a second hand-rolled XML parser at `:620-759` that shares nothing with
  `src/soap/xml.rs`. D8 and D9 are both consequences of that split brain.
- `pub mod soap::{xml,security,…}` makes `compute_digest` and
  `unix_secs_to_iso8601` permanent public API on a published crate.
- **25** `.lock()/.read()/.write().unwrap()` poison-panic sites in production code
  across `src/metamorph/` (12), `src/mock/` (9), `src/health/` (4), plus 2 in
  `src/discovery.rs`. `src/discovery.rs:417` shows the correct
  `unwrap_or_else(|e| e.into_inner())` form, so the inconsistency is internal.
  (Counted with the Grep tool — shell `grep` reports 0 here, see §7.)
- Mock-side correctness: undeclared namespace prefixes on every void response;
  SOAP faults put the ONVIF code in `Code/Value` instead of `Code/Subcode/Value`,
  so `subcode` is always `None`; `SetImagingSettings` silently drops 5 of 11
  fields; Media1/Media2 profile and encoder state are disjoint. **The mock's OSD
  payload parser tolerates a renamed `<tt:Type>` and defaults it** — measured in
  batch 2, where retagging that element left `tests/mock_action_snapshot.rs`
  entirely green with all four targets running. The snapshot net does not
  discriminate that field, so it must not be cited as coverage for it.
- `examples/write_workflow.rs` reimplements the library's mock server (~400 lines)
  and its harness prints failures instead of failing, exiting 0 regardless.
- **`MissingField` path strings are inconsistent and half of them are unqualified.**
  Recording alone emits `"Uri"`, `"JobToken"`, `"SearchToken"`, `"RecordingToken"`,
  `"TrackToken"` with no operation or element context, and `"RecordingJob/JobToken"`
  names a Rust type where the XML element is `JobItem`. `Missing required field: Uri`
  is not a diagnosable message. Normalising them is a library change; Stage 4
  batch 1 has **pinned** the current strings in 7 tests, so the normalisation will
  fail loudly and its blast radius is already measured. Found by the batch-1 agent.

---

## 9. Stage verdicts

What each verdict actually rested on. "Done" in §2 means *this*, not an agent's
self-report. Mutations listed are the reviewer's own, chosen per check #6 to be
points the agent did **not** use.

**Stage 0 · `1e8d634` — baseline, no separate Critic pass.**
It *is* the photograph the later stages are checked against, so it has nothing
prior to be checked against. Its integrity is established indirectly: 0.5 proved
the net survives a file move byte-for-byte, and 1a proved the six pinned rows were
pinned for the right reason.

**Stage 0.5 · `e21ed7f` — PASS.**
Listed test names 676 → 676, leaf-name multiset identical. An independent extractor
(`<scratchpad>/equiv.py`, a lexer-lite that handles `r#"…"#` — v1 gave 8 false
positives without it) found 324 → 324 fns, 0 missing, 0 gained, 2 bodies changed —
both `crate::` → `oxvif::` in the file that moved to `tests/`, byte lengths
unchanged because both spellings are 5 characters. `const EXPECTED` identical.
2 mutations caught.

**Stage 1a · `894b865` — PASS.**
`EXPECTED` 141 → 141 rows, exactly 6 changed and all 6 now `"ok"`, the other 135
byte-identical, order preserved — diffed against a copy frozen from `git HEAD`
*before* the agent started (C11b). Names 676 → 688 with one deliberate deletion
(`known_broken_mock_actions_are_pinned`). Production diff is six `dispatch.rs`
arms and nothing else. Sandbox mutation — revert `src/mock/dispatch.rs` alone —
turned exactly the six new round-trip tests red (634 passed, 6 failed), run
unfiltered per C10.

**Stage 1b · `573168a` — PASS.**
Agent pasted real red, not a claim: `E0599: no method named to_xml_body_media2`,
plus a behaviour failure printing the actual wire body
`<tr2:SetAudioEncoderConfiguration><trt:Configuration …>`. `fmt`/`clippy` re-run
by the reviewer, clean. Names 688 → 698; the one vanished name is the rename this
stage existed to perform. Two mutations, neither at the agent's suggested point:
reverting the `src/client/media2.rs` call site → D3 target test red; retagging
`<tt:Channels>` → `<tt:Channel>` in the Media2 serialiser only → 4 tests red,
including the drift invariant
`audio_media1_and_media2_differ_only_in_the_wrapper_prefix`. A third attempt was
discarded for not compiling (C14).

**Stage 2 · `ddfde44` — PASS** (amended once, see below).
Red pasted verbatim for all three behaviour tests, unfiltered: both
`get_discovery_mode` negatives failed with `Ok("")` — confirming the
present-but-empty element is a genuinely separate code path from the absent one
(`child()` finds the node, `text()` returns `""`), not a duplicate case. Names
698 → 702; the two vanished are the renamed Stage 0 pin and a C15 line-number
shift, verified as +4 doc lines against a 514 → 518 move. Four reviewer
mutations, all caught unfiltered:

| mutation | red |
|---|---|
| revert the D5 production half alone | the 2 new negatives |
| revert the D11 production half alone | `is_complete_is_false_when_the_report_is_empty` |
| shorten the field path to `"DiscoveryMode"` | the 2 new negatives — so they discriminate on the path, not on `is_err()` |
| `all()` → `any()` in `is_complete` | the 2 Stage 0 non-empty tests — the pre-existing net still bites after the change |

**The one thing sent back:** the whitespace-only half of the decision was
unguarded. It works only because `XmlNode::parse` collapses blank text to `None`,
and neither the new tests nor any of `soap::xml`'s own 23 tests asserted that. The
amend pins it at both altitudes. Verified with a *narrower* mutation than the
agent's — keep the trim for non-empty text, drop only the collapse-to-`None`
branch — which turned exactly those two tests red out of 654. That is the whole
defence: nothing else in the crate notices.

**Stage 3 step 1 · `5d3fbc7` — PASS.**
Two reds pasted, in the right order: the compile error against the old arity
(6 errors, nothing ran), then — after applying the *signature only*, leaving the
key-only index in place — the behavioural red, which prints the defect verbatim:
a Media1 request answered with `<tr2:GetProfilesResponse><Profiles token="media2-profile"/>`.
Names 702 → 707, +5 real, the single vanished name a C15 line shift
(`MetamorphTransport` 91 → 93, the `respond` call site rustfmt-wrapped to three
lines). Three reviewer mutations, none at the agent's point (it had used
`canon.rs`):

| mutation | red |
|---|---|
| neutralise the action half in `insert` *and* `lookup` — the pre-fix bug, still compiling | the 3 collision/round-trip/replay tests + the shim test |
| **key on the raw request instead of the canonical one** — the trap named in §3 | 13 tests, including **both** ephemera de-dup invariants |
| shim returns the last key match instead of the first | the shim test alone |

The middle one is the point of the exercise: a "fix" that keys on the raw request
would show 60 fixtures where 59 stood and *look* like it lost nothing, while
silently destroying de-dup. The net catches it.

**Downstream verified, not assumed:** `oxdm` (the only known consumer) uses
`FixtureStore::{save, load, device}` and `serve(store)` and **never calls
`lookup`**. `cargo check --all-features` in `oxdm` against this commit finishes
clean — no errors, and no deprecation warnings either, since it does not touch the
shim. The break costs the desktop app nothing. See §7 for the pin that made this
checkable at all.

**Deviation accepted:** the two protected de-dup tests could not stay
byte-identical — each ends on a `store.lookup(&key)` line, and a public signature
change mechanically rewrites every call site. The reviewer diffed both: only the
call line moved, every `len()`, last-write-wins and field assertion is unchanged,
and the tests are marginally *stronger* since the entry must now resolve under its
recorded action. Recorded as C17.

**Not bug-compatible, by design:** the shim returns the *first* fixture matching
the key, while the old `lookup` returned the *surviving* one (the last write, which
had overwritten the other service). For a store that now holds both, "the old
behaviour" is not well defined — so the shim is documented as first-match rather
than pretending to be a drop-in. The 0.14.0 notes must not describe it as
compatible.

### Stage 3 step 2 — `0c156b2` — PASS

Removed `lookup_by_key` and the one test whose entire subject was it. Diff is 52
deletions, 0 insertions, one file, exactly the two authorised blocks — verified by
reading the diff, not the agent's description of it.

**The red for a deletion is a compile error, not a failing assertion.** With the
function gone and its test still present: one `E0599`, naming `lookup_by_key` at
`fixture.rs:683` and nothing else. That single error *is* the proof that the
reference surface was what §7's search said it was.

Names 707 → 706, passing 705 → 704: exactly one vanished, none appeared, no C15
line shift (`fixture.rs` contributes no doc tests — checked positively, the same
list holds 12 doc-test lines from elsewhere in `metamorph`).

**What actually needed proving.** A function with no callers is nearly
unmutatable, so mutating the deletion is theatre. The real risk is collateral: that
removing a test quietly took some of the net with it. So the reviewer replayed
Stage 3 step 1's two load-bearing mutations against *both* commits and diffed the
red **name sets**:

| mutation | at `5d3fbc7` | at `0c156b2` | set difference |
|---|---|---|---|
| neutralise the action half in `insert` *and* `lookup` | 4 red | 3 red | exactly `{the shim test}` |
| key on the raw request instead of the canonical one | 13 red | 12 red | exactly `{the shim test}` |

Both compiled (not a C14 false red). Every test that defended D1 before still
defends it. Drivers at `<scratchpad>/mut4.py`, captures at
`<scratchpad>/red-step{1,2}.json`.

**A count would have been the wrong instrument, and nearly was.** The reviewer's
first draft of that check hard-coded "expect 13 again" from the step-1 verdict.
Measuring the baseline instead showed the shim's own test was red under *both*
mutations, so the correct expectation was 12 — the hard-coded 13 would have raised
a false alarm against a perfectly good commit. Recorded as C18.

**Feature-free `--all-targets` has two failure sources, not one** — found by the
agent, confirmed by the reviewer, and pre-existing by construction since this diff
touches only `fixture.rs`. §7's known-red note has been corrected.

### Stage 4 batch 1 — `1c01977` — PASS

All 15 recording methods, one file (`src/tests/client/recording_tests.rs`), 92
insertions / 46 deletions. No test added, none removed, none renamed — the name
set is byte-identical at 706, so the whole diff is assertion strength. Two
`#[track_caller]` helpers (`assert_fault`, `assert_missing_field`) plus 15
call-site rewrites and 7 fixtures given distinctive payload strings.

**The independent proof is a library mutation, not the agent's own perturbations.**
The agent proved each assertion fixture-sensitive by perturbing its own fixture
(22 perturbations, 22 red, 22 reverted green). That is necessary but self-scored,
so the reviewer ran two mutations of the *library* across both refs, unfiltered,
and diffed red name sets:

| mutation | at `2d39f63` | at `1c01977` | set difference |
|---|---|---|---|
| `SoapError::missing()` ignores its argument, returns `"MUTANT_PATH"` | 11 red | 19 red | exactly the **8** recording missing-field negatives |
| the fault parser returns constant `code`/`reason` | 16 red | 23 red | exactly the **7** recording fault negatives |

8 + 7 = 15, nothing else moved in either direction, both compiled (not a C14 false
red). **Before the batch not one of the 15 was red under either mutation** — the
hollowness §2.2 asserted is now measured, not argued. Drivers at
`<scratchpad>/mut5.py`, captures at `<scratchpad>/red-b1-{before,after}.json`.

*Amended during batch 2:* those first runs lacked `--no-fail-fast`, so cargo
aborted after the failing lib target and the three integration crates never ran
(C21). Both refs were re-measured with it — 11/19 and 16/23 unchanged, because
this batch touched no integration test — but the original runs proved less than
the verdict claimed. Fresh captures: `<scratchpad>/red-nff-*.json`.

**These two mutations are the standing instrument for the remaining batches.** They
are service-independent: any negative that asserts a `MissingField` path goes red
under the first, any that asserts a fault `code`/`reason` under the second. Run
them before and after each batch and require the difference to equal that batch's
method list. They also give a cheap lower-bound audit of the ledger — a method
already red at the baseline has a real negative regardless of what a grep says.

**The two hollow flavours were never distinguishable in power, only in
appearance.** Seven of the agent's perturbations turn a `Fault` into
`UnexpectedResponse`; both `assert!(res.is_err())` and
`matches!(err, OnvifError::Soap(_))` pass either way. The ledger's split between
`[is_err-only]` and `[outer-variant-only]` predicts cost, not risk — which is the
reasoning §2.2 already used to fold the 16 variant-only rows into the 64.

**Deliberate coupling, recorded so nobody unpicks it by accident.** The batch pins
seven `MissingField` path strings exactly as the library emits them, and they are
not consistent: `get_recording_jobs` reports `"RecordingJob/JobToken"` although the
XML element is `JobItem`, while its sibling reports the element-named
`"RecordingItem/RecordingToken"`, and five more are bare (`"Uri"`, `"JobToken"`,
`"SearchToken"`, `"RecordingToken"`, `"TrackToken"` — a user sees
`Missing required field: Uri` with no operation named). Normalising them is a
library change and out of scope here; the point of pinning is that such a change
now fails loudly instead of silently. Logged in [§8](#8-known-gaps-deliberately-not-addressed-here).

**Minor, accepted:** four fixtures now send `env:Receiver` for what ONVIF would
call a sender-side fault. The strings exist to make `code` and `reason`
independently discriminating, and no library behaviour depends on the value —
but they are not device-plausible, and a future real-camera fixture should not
copy them.

### Stage 4 batch 2 — `71e349d` + `3c8b420` — PASS

22 methods (media 10, media2 12), split into two commits because the two halves
need different proofs. Scope was *derived by the agent from the ledger* against a
stated rule and cross-checked against the six methods the reviewer already knew
were red at the mutation baseline; it reconciled at 10 + 12 with none of the six
present. 706 → 722 names, 16 added, **none removed**, 704 → 720 passing.

**2b — the 6 hollow negatives.** Same instrument as batch 1, run at three refs:

| mutation | pre-b1 `2d39f63` | post-b1 `1c01977` | post-b2 `3c8b420` |
|---|---|---|---|
| `SoapError::missing()` returns a constant path | 11 | 19 (+15's 8) | **25 (+6)** |
| fault parser returns constant `code`/`reason` | 16 | 23 (+15's 7) | 23 (**+0**) |

The +6 are exactly the six strengthened sites; nothing was removed. **FA moving by
zero is the correct answer, not a miss** — none of the six in-scope methods has a
SOAP-Fault negative to strengthen. The agent flagged that the brief's
"distinctive fault payload" rule therefore did not apply here and declined to
invent fault fixtures, which would have replaced the subject of tests it was told
to strengthen in place. Correct call; giving these six fault negatives as well is
new scope, not batch 2.

**A seventh site is defended by a third instrument.** `get_profiles` has two
negative sites and the second is a *parse-error* test; its payload is a quick-xml
message string, so neither MF nor FA can see it. Do not expect it in a delta.

**2a — the 16 new positives.** A brand-new positive is green the moment it is
written (C16), so the proof is a compiling mutation of the library code each
assertion depends on, with victims **named before the run**. Eight rounds, all
reverted and verified with `git diff HEAD --stat` (C13). The reviewer replayed two
independently at `3c8b420`, both reproducing exactly:

- `VideoEncoderConfiguration2::from_xml` reading `GopLength` → 3 red, as predicted.
- `OsdConfiguration::to_xml_body` retagging `<tt:Type>` → **1 red**, where the
  agent had predicted 2. Its prediction was that the mock snapshot would flip too.
  It does not: the mock's OSD payload parser tolerates the renamed element and
  defaults it, so `tests/mock_action_snapshot.rs` does not discriminate the OSD
  `Type` at all and the new unit test is the only thing defending it. **Fewer kills
  than predicted is a finding, not noise** — logged against the mock bullet in §8.
  The reverse also happened: retagging Media1 `DeleteOSD`'s `OSDToken` killed 4 not
  3, because that mock handler parses the token out of the request *body*
  (`src/mock/services/media.rs:589-590`) while the Media2 profile handlers are
  static. The mock's dispatch is not uniformly action-only.

**Accepted deviation.** `recording_tests.rs` was to come out byte-identical apart
from the moved helper block; it lost one more line, `use crate::soap::SoapError`,
which the move orphaned and `-D warnings` rejects. That is CLAUDE.md's "remove
imports YOUR changes made unused", verified by reading the diff: −32 lines, the
helper block plus that import, no assertion touched.

**Pinning now spans services.** `"Uri"` is asserted verbatim by both
`get_replay_uri` (recording) and `get_stream_uri` (media), and `"Profile/@token"`
by two media methods, because the library genuinely emits the same bare string for
different operations. The agent did not change the library to make its assertions
prettier. This widens the §8 coupling: normalising those paths now fails across
two services.

### Test-design patterns these stages produced

Worth reusing in Stages 3 and 4 rather than rediscovering.

- **Premise guard** (1b `hostile_encoding_reaches_display_unescaped`, 2
  `test_whitespace_only_element_text_is_empty`). Asserts the *precondition* a
  group of tests silently relies on. Without the 1b one, anyone who later makes
  `as_str` escape turns all four site tests vacuous with nothing going red.
  Validate by mutating the module that owns the premise — see C16.
- **Two-altitude pinning** (2). When a method's contract depends on another
  module's behaviour, pin it in both places: the call site so the contract fails
  loudly, and the source so the failure points at the module that actually
  changed. Costs a few lines; stops a `soap::xml` regression from being reported
  as a device-client bug.
- **Drift invariant** (1b `audio_media1_and_media2_differ_only_in_the_wrapper_prefix`).
  Compares two implementations' whole output after normalising the one difference
  that is legitimate, so any future divergence fails instead of silently spreading.
- **Assert the payload, not the variant** (2). Both new negatives assert the
  `MissingField` *path string*, which is what made the shortened-path mutation
  fail. This is the concrete template for un-hollowing the 21 negatives in Stage 4.
