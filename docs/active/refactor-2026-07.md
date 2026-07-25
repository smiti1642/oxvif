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
| Test-coverage scope | **the 26 zero-coverage methods + the 21 hollow negatives = 47** | Decided 2026-07-26, replacing the earlier "all zero-coverage" wording once the survey showed that covered only 26 of 148. Both classes selected *mislead a reader*: the 26 have no test at all, and the 21 feed a real SOAP Fault then assert only `is_err()`, so they read as green. The remaining 69 `partial` methods simply have no negative — an absence that is visible in the ledger — and are deferred. See Stage 4. |
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

---

## 2. Stages

| # | Content | Nature | Status |
|---|---|---|---|
| 0 | Regression safety net (tests only) | additive | **done** — `1e8d634` |
| 0.5 | Split client tests by service; move mock snapshot to `tests/` | pure move | **done** — `e21ed7f` |
| 1a | Split the two collapsed `dispatch.rs` arms; six operations start working | non-breaking | **done** — `894b865` |
| 1b | `AudioEncoderConfiguration::to_xml_body_media2()`; `xml_escape` on 4 encoding sites | non-breaking | **done** — `573168a` |
| 2 | `get_discovery_mode` strictness; `is_complete()` empty-report case | behaviour change | **done** — `ddfde44` |
| 3 | Fixture key → `(action, key_canon)`, in two steps | **breaking** | step 1 in progress |
| 4 | Positive+negative pairs for the 26 zero-coverage + 21 hollow-negative methods | additive | not started |

Verdicts for the four finished stages are in [§9](#9-stage-verdicts) — what each one
actually rested on, not just that it passed.

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

**Stage 4 scope is larger than §1 assumed.** A survey of all **148** public
`OnvifClient` methods (149 `pub fn` in `src/client/` minus the free function
`notification_listener`; enumeration cross-checked against the 8 `impl OnvifClient`
blocks and against `--list`) scores them against the CLAUDE.md positive+negative
rule as:

| verdict | count | meaning |
|---|---|---|
| covered | 32 | real positive **and** discriminating negative |
| partial | 90 | real positive, negative **missing or weak** (`assert!(res.is_err())` only) |
| zero | 26 | neither |

Closing only the 26 zero-coverage methods would leave **90 methods still violating
the rule**; full CLAUDE.md compliance would be 116 methods.

**Decided 2026-07-26: Stage 4 covers 47 — the 26 `zero` plus the 21 whose negative
is hollow** (`assert!(res.is_err())` against a real SOAP Fault). The selection
criterion is *misleadingness*, not count: those 21 look green while discriminating
nothing, which is strictly worse than a visible gap. The other 69 `partial` methods
are deferred with their absence recorded in the ledger, **not** silently closed —
§8 is where deferred work goes, and Stage 4 must not be described afterwards as
having made the crate rule-compliant. Stage 4 ships inside 0.14.0 as §1 locked.

The 21 hollow negatives: `ptz_get_configurations`, `ptz_get_nodes`, `get_profiles`
(×2 sites), `get_profile`, `get_stream_uri`, `set_scopes`,
`set_system_date_and_time`, `event_stream`, and all twelve recording ones
(`create_recording`, `delete_recording`, `create_track`, `delete_track`,
`get_recording_jobs`, `create_recording_job`, `set_recording_job_mode`,
`delete_recording_job`, `get_recording_job_state`, `get_recording_search_results`,
`end_search`, `search_recordings`).

Two findings that sharpen it:
- **PTZ has zero `covered` methods out of 18.** Six are zero-coverage
  (`ptz_absolute_move`, `ptz_relative_move`, `ptz_continuous_move`, `ptz_stop`,
  `ptz_get_presets`, `ptz_goto_preset`); the other twelve all lack a real negative.
- **The 12 recording negatives are the most dangerous cluster.** Nine feed a real
  `make_soap_fault_xml` response and then assert only `is_err()`. Turning a Fault
  into an `UnexpectedResponse` or `MissingField` would leave all of them green.
- `with_utc_offset` and `device_url` have **no call site in any test at all** — not
  even in the snapshot net.

The full ledger was measured while Stage 1a was in flight (see C11b) and must be
**regenerated against a committed ref** before Stage 4 begins.

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
| C16 | Every new test must be shown red before the fix | Not one whose subject is "property X still holds". A cross-module premise guard is **green the moment it is written**, because the premise already holds — red-before-green proves nothing and accepting it green is indistinguishable from accepting a vacuous test. Validate it by **mutating the module that owns the property** and naming the expected victims. Stage 2's two whitespace pins were the only 2 of 654 lib tests that caught a mutation of the `Event::End` trim; 1b's `hostile_encoding_reaches_display_unescaped` is the same shape, with the method left implicit. | agent |

---

## 7. Environment facts that bite

- **Branch layout.** This programme lives on **`refactor/2026-07`**, branched from
  `f94a747` (= `origin/develop`). `develop` was rewound to exactly its remote so
  the staged work cannot leak into it; nothing here has been pushed. Merge back to
  `develop` only when the whole programme is green, then follow the release SOP.
- **Mutation sandbox:** `C:/Users/Null/Documents/GitHub/oxvif-mut`, a detached-HEAD
  worktree with a commit-refusing hook. See §5 C. Keep it out of the main tree.

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
- **Known-red baseline:** a feature-free `cargo test` fails on
  `examples/conformance.rs` (C1). Unrelated to this programme; do not fix it
  inside a stage. CI only ever builds `--all-features`, so this class of breakage
  is structurally invisible to CI.
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
  fields; Media1/Media2 profile and encoder state are disjoint.
- `examples/write_workflow.rs` reimplements the library's mock server (~400 lines)
  and its harness prints failures instead of failing, exiting 0 regardless.

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
