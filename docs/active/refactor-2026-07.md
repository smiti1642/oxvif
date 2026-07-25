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
| Test-coverage scope | **all zero-coverage client methods**, not just `get_services` | See Stage 4. |
| Test layout | split by service; keep in `src/tests/` | `src/tests/client_tests.rs` uses zero `pub(crate)` internals so it *could* move to `tests/`, but that needs a several-hundred-site `crate::` → `oxvif::` rewrite, producing a diff too large to review for silently weakened assertions. Only the black-box mock snapshot moved to `tests/`. `src/tests/types_tests.rs` **cannot** move — 73 call sites of `pub(crate)` fns (46 `from_xml`, 15 `vec_from_xml`, 12 `to_xml_body`; all 84 definitions in `src/types/` are `pub(crate)`). |

**Layout as shipped in `e21ed7f`:** each `src/tests/client/<svc>_tests.rs` is attached
from the foot of the matching `src/client/<svc>.rs` via `#[cfg(test)] #[path]`, so tests
sit in the module they exercise (`client::device::tests::…`). Shared transports and the
two multi-service fixture helpers live in `src/tests/common.rs`, declared once from
`src/lib.rs` through `src/tests/mod.rs`. The black-box snapshot is
`tests/mock_action_snapshot.rs`, gated `#![cfg(feature = "mock")]`.

**Migration note for the 0.14.0 release notes:** Stage 3 invalidates every
already-recorded metamorph clone. The lost Media1 fixtures were never written to
disk, so no upgrade path can recover them — users must re-record.

---

## 2. Stages

| # | Content | Nature | Status |
|---|---|---|---|
| 0 | Regression safety net (tests only) | additive | **done** — `1e8d634` |
| 0.5 | Split client tests by service; move mock snapshot to `tests/` | pure move | **done** — `e21ed7f` |
| 1a | Split the two collapsed `dispatch.rs` arms; six operations start working | non-breaking | not started |
| 1b | `AudioEncoderConfiguration::to_xml_body_media2()`; `xml_escape` on 3 encoding sites | non-breaking | not started |
| 2 | `get_discovery_mode` strictness; `is_complete()` empty-report case | behaviour change | not started |
| 3 | Fixture key → `(action, key_canon)`, in two steps | **breaking** | not started |
| 4 | Positive+negative pairs for every uncovered client method | additive | not started |

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

Must preserve: ephemera de-duplication (`src/metamorph/fixture.rs`,
`ephemera_jitter_does_not_fragment_the_key`) — two records differing only in
`MessageID` must still collapse to one.

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
| D4 | `xml_escape` bypassed via `Display` — `Other(String)` returns the device's raw string. Invisible to a `grep xml_escape` audit. | `src/types/audio.rs:99`,`:173`; `src/types/video.rs:532`,`:749` |
| D5 | `get_discovery_mode` doc promises one of two strings; code can return `""`. | `src/client/device.rs:840` vs `:847-850` |
| D6 | `Transport` 400-handling contract undocumented at the trait, and contradicted by three doc sites that say 200/500 only. | `src/transport.rs:11-15`, `:40-41`, `src/error.rs:19-21` vs code at `src/transport.rs:163` |
| D7 | Non-reqwest `diqwest` errors reported as a fabricated HTTP 401. | `src/transport.rs:138-144` |
| D8 | Devices with an empty endpoint overwrite each other; only the first survives. Strict path only — the lenient path already rejects them. | `src/discovery.rs:61` + `:283` vs `:638-643` |
| D9 | `listen()` aborts the whole window on one transient recv error; `probe_once` explicitly does not. | `src/discovery.rs:493` vs `:420` |
| D10 | `CapturingTransport` writes WS-Security digests and RTSP credentials verbatim, and the module doc tells the user to commit them. The successor module spent ~100 lines solving this. | `src/fixtures.rs:73`,`:78` vs `:15`; cf. `src/metamorph/fixture.rs:178-266` |
| D11 | `SweepReport::is_complete()` is vacuously `true` on an empty sweep. | `src/metamorph/surface.rs:414` |
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
| C11 | Disjoint file scope means two stages can run concurrently | Not in one working tree. Both agents run `cargo test`, so each sees the other's half-finished edits and red-before-green becomes unprovable. Serialise, or give each agent its own worktree. Read-only work may run alongside. | reviewer |
| C12 | Counting Rust items with a line pattern is fine | `grep -c '^    ("'` gave **137** `EXPECTED` rows; the real count is **141**. The six missing rows are rustfmt-wrapped across lines *because their values are long* — i.e. the exact six broken entries the count existed to track. Count with a brace-matching parser, and prefer asserting a post-condition ("all 141 rows are `ok`") over a delta ("6 rows changed"). | reviewer |

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
- Test totals: **652** before Stage 0 → **674** after (629 lib / 9 integration / 36 doc),
  verified by running the suite at `7daa4ac`. Note `-- --list` reports **676**
  names, because 2 doc tests are `ignored` and so are listed but never run.
  **Check #4 compares the 676-name set, not the 674 pass count.** Baseline name set
  captured at `<scratchpad>/baseline-tests.txt`.

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
