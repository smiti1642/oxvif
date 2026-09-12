# 0.17 candidate review batches

[English](release-0.17-review.md) | [繁體中文](release-0.17-review_zh.md)

Status: LOCAL REVIEW COMPLETE. Updated 2026-09-12. This is an execution record, not release
approval. [Approval packet](release-0.17-approval.md) and
[release gates](release-0.17-cut.md) remain authoritative.
This completion applies to the input hashes recorded below. Later CLI `dcbff41`
and Fleet development are not silently added to this ledger; their current-delta
review status is tracked by those release gates.

| Section | Purpose |
| --- | --- |
| [Inventory](#inventory) | Frozen inputs and honest coverage |
| [Batch order](#batch-order) | Completed local review and retained limits |
| [A04 human-output repair](#a04-human-output-repair) | Reproduction, repair and checks |
| [V01 encoder replay coverage](#v01-encoder-replay-coverage) | Expanded tests, not a new production repair |
| [A05 terminal display and T01 assertion follow-up](#a05-terminal-display-and-t01-assertion-follow-up) | Terminal repair, native-exit guards and local acceptance |
| [T02 exact assertion follow-up](#t02-exact-assertion-follow-up) | Stronger existing drivers and negative controls |
| [Assertion review boundaries](#assertion-review-boundaries) | What the tests do and do not establish |
| [Handoff boundary](#handoff-boundary) | Maintainer CI and reserved actions |

## Inventory

The [machine-readable ledger](release-0.17-review-ledger.json) enumerates all
208 changed paths in `git diff --name-only v0.16.0 b7bc881`. Its status describes
**this review pass**, not the amount of prior implementation or testing.
`pending` means not yet recorded here; `in_progress` records the precise
inspection performed but leaves final closure open. Do not turn passing test
counts or a source-read count into a readiness percentage.

A row can become `reviewed` only after its applicable source changes, affected
consumers, assertions and public claims have been reconciled. Unchanged earlier
operation-card evidence can be reused with an exact matching input revision;
newly modified code needs delta review. Additions after the frozen inventory
are tracked in `delta_paths`, including this record and its translation.

All 208 frozen rows are now reviewed. Subsequent paths are reconciled separately
in `delta_reviews`; per-file blob IDs identify reviewed inputs. This closes the
local source/consumer/assertion/claim review for G01/G03, not native, staging,
hardware or whole W00–W26 acceptance. Historical deficiencies and test totals
remain dated in their original cards; the final disposition is recorded here.

## Batch order

Work by complete service subgroup. Combine fixes in a coherent batch before
running the complete sensitivity/restored gates; do not repeat the entire
workspace after each file or documentation edit.

| Batch | Frozen paths | Completed local review | Retained acceptance boundary |
| --- | ---: | --- | --- |
| CLI | 18 | Entry points, selectors, registry, reports, maintenance/manage, navigation, preferences, rendering/lifecycle, Agent/schema/descriptor and affected tests reconciled with guides | Actual Windows debug ConPTY is bounded; remaining human/platform and packaged installation acceptance is separate |
| Library | 26 | Media1/2, session, types/XML, public reexports/examples, notification listener and consumers reconciled with wire/fault/serde/loopback and migration assertions | Snapshot recognition is not decoding; inherited listener is not Internet-hardened; no universal hardware conformance |
| Mock | 54 | Shared request/auth/fault/dispatch/state and included profile/PTZ/source/encoder/audio/metadata paths, inherited consumers, effects and operation cards reconciled | Selected contracts only; remaining legacy HTTP/field/Fault semantics and full W00–W26 stay explicit |
| Replay | 7 | Fixture/parse/report collision buckets, adapter/auth/raw chain, committed effects and cross-service identity/reference consumers reconciled with K27 and retention controls | Raw recording remains caller-owned; downgrade and concurrent publication limits remain |
| Delivery | 19 | Manifests/lockfile, complete CI/release workflows, schema tools/source refs, source-SPDX, archive/formula and install assertions reviewed; local native-exit controls passed | Final-candidate native CI, distribution staging and final-version package/install acceptance remain required |
| Documents | 84 | Public guides/READMEs/manpage, paired migration/release records, historical operation cards, local links and published history reconciled | Historical external-corpus/hardware/CI results retain exact scope and revision; no new execution inferred from prose |

The counts classify files, not independent risk units. A shared helper can cross
several groups. Security findings within the included cut must be resolved or
explicitly block release; remaining product features stay in the
[post-release backlog](post-0.17-backlog.md).

## A04 human-output repair

Human table reports previously emitted ESC/OSC/CSI, carriage returns, C1 controls
and bidirectional formatting controls from untrusted data. The repair is at
`render_success`, the verbose-report completion boundary and `render_error`.
Single-line profile choices and implicit device-context fields use
`terminal_field`. Escapes are display values, never inputs to camera requests.

JSON/JSONL retain the original values. Human report layout retains LF/TAB; this
is not a guarantee that every data field occupies one line, a universal Unicode
spoofing defense, or a new claim about all full-screen input/rendering paths.
No camera protocol, credentials, retry policy, file-write semantics or schema
version changes in A04.

| Check, Windows x64 | Result |
| --- | --- |
| Before production repair | Complete workspace all-features/no-fail-fast: 1,308 passed, two expected assertion failures, five ignored, 41 suites |
| Sensitive assertions | `human_output_neutralizes_terminal_commands_without_changing_json` and `verbose_stages_and_error_hints_cannot_emit_terminal_commands`; no compile-only failure |
| Repaired all-features/default | 1,310 / 1,200 passed; zero failed, five ignored each, 41 suites each |
| Static/document gates | Both workspace all-target Clippy modes with warnings denied; both strict rustdoc modes; fmt and diff whitespace passed |
| Actual debug executable | Synthetic unknown-command input with ESC/BEL/directional text: human error on stderr is escaped; JSON/JSONL stdout parse back to original text; all exit 3 and structured stderr remains empty |
| Scope of executable check | Local `describe` metadata only; no camera requests or host-system install. Not an interactive terminal acceptance test |

The pre-repair run proves the new assertions catch the original behavior.
The executable check supplements unit tests; it does not replace native CI.
Raw logs are local evidence, not committed camera data.

## V01 encoder replay coverage

Existing `mock_video_rate` tests already exercise failed-rate retention,
fractional readback, Media1 refusal and physical-source preservation. V01
extends that coverage; it does not repair a newly reproduced production defect.

Two tests in `mock_video_encoder` exercise a Media2 encoder write over in-process
and HTTP replay. Eight unique raw markers establish the initial recording hits.
A rejected nonfinite Quality leaves the full state and all recordings unchanged.
An accepted integer-rate configuration refreshes six dependent profile/encoder
views across Media1/2 with the actual committed name, while preserving unrelated
physical-source and encoder-options recordings. The test inputs are synthetic
controls, not independently schema-validated or real-camera fixtures. Media1
write entry is not newly exercised by these two tests.

Temporarily suppressing the `VideoEncoderCommitted` retirement arm fails the two
new tests on stale `GetProfile`, and the two existing rate-replay tests. This
checks the shared retirement boundary, not independent mutation sensitivity of
each of the six action entries. Production source is restored exactly before
the final gates. No ONVIF method, parser, dependency or production behavior is
changed by V01.

| V01 check | Result |
| --- | --- |
| Suppressed retirement, complete all-features/no-fail-fast | 1,308 passed; four expected assertion failures; five ignored; 41 suites |
| Restored all-features/default | 1,312 / 1,200 passed; zero failed; five ignored and 41 suites each |
| Static checks | Both workspace all-target Clippy modes with warnings denied and fmt pass; strict rustdoc evidence from A04 applies to unchanged public source |
| Production restoration | No diff from `1ea4fff` in `src`, CLI crates, manifests/lockfile or workflows; only tests and review documentation change |

## A05 terminal display and T01 assertion follow-up

2026-09-12, on `codex/release-0.17-closure` after `44302c8`. Full-screen
rendering removed control characters but still emitted Unicode directional
formatting characters. A new test failed on the original menu title before the
repair. Menu cells, truncation and wrapping now escape display data before
measuring its width. Original selection data is retained. This extends A04 to
these rendering paths; it is not a universal Unicode spoofing defense.

The test audit retained distinct transport, state, replay and executable
boundaries. Three redundant test functions were merged into stronger existing
drivers: XML-parsed PTZ presets with exact JSON/field checks; capabilities Faults
with and without `xml:lang`; and seeded literal profile names over three reads
and both transports. Native credential errors now use one shared redaction
function in the real keyring adapter, so the test can inject an error that
actually contains synthetic sensitive text.

Four targeted perturbations produced five assertion failures: backend-error
leakage, changed preset Name, changed capabilities reason, and changed seeded
profile text in both transports. All were restored; ten affected tests passed.
These are targeted sensitivity checks, not a new workspace mutation campaign.

| Check | Observed result |
| --- | --- |
| Workspace all-features/default, locked, no-fail-fast | 1,310 / 1,198 passed; five existing ignored and 41 suites each |
| Both workspace all-target Clippy modes | Passed with warnings denied |
| Documentation, compatibility and dependency gates | Both strict workspace rustdoc modes, fmt, diff whitespace and Rust 1.88 all-feature/all-target check passed; fresh cargo audit found no advisory in 410 locked dependencies |
| Windows ConPTY, actual debug executable | 40 synthetic saved devices; `21G`, detail/back, 100×24 → 48×12 → 100×24 resize, profile lookup/cancel, text editing and masked-password cancel passed |
| Snapshot cancellation in ConPTY | Loopback proxy signaled a started image body; Esc cancelled, published no destination or temporary file, and started no further request; next operation required reconnect |
| Terminal and local state | Normal exit and Ctrl-C exit restored exact stdin/stdout console modes measured by the parent in the same console; saved configuration hashes unchanged |
| Executable provenance | Final terminal run used a copied debug executable, SHA-256 `9fbcb9d166ecfc9987647f54051c7bcb17a3c7bd9387a4680c6e7e54606d1f1d`; local Windows evidence, not release archive/install acceptance |
| Windows CI smoke source | Four immediate exit checks added. Extracted-script success control plus four simulated native failures passed; each failure stopped before later commands |
| Windows release smoke and packaging | Fourteen immediate native-command checks, including PE inspection and archive generation. Each extracted guard passed real native exit-0/exit-17 controls (28 cases); no hosted workflow was dispatched |
| Packaging test placement | Removed only the duplicate Clippy-job invocation; dedicated Ubuntu/Windows schema-tooling jobs retain the same 24 controls and remain package prerequisites |
| Final documentation/inventory closure | 1,360 local Markdown targets and 670 anchors resolve; published CHANGELOG history is unchanged. Inventory self-tests pass with 159 routes, 161 Action sites and 191 readers; indexed operation/source rows and W00–W26 statuses are preserved |

The first default build overlapped a running Mock executable and failed with
Windows linker LNK1104. It passed after terminal helpers used executable copies.
The initial terminal harness used the HTTP root instead of the Mock device path
and received a transport failure; correcting it to `/onvif/device` required no
product change. Neither failed attempt is acceptance. Inputs and local artifacts
are synthetic; no host CLI installation or new camera scan occurred.
The Rust repair and test consolidation are committed as `214d989`, and workflow
guards as `210bfc3`. T02 changes only existing assertions; later source-comment
and documentation changes do not alter product behavior.

## T02 exact assertion follow-up

Commit `a5907d3` strengthens four existing test files without adding test functions
or changing production behavior. Encoder replay now pins Sender → InvalidArgVal
→ ConfigModify and the Quality reason as well as full-state/recording preservation.
Rate replay rereads the previously recorded bare Media2 GetProfiles after commit:
one response, four literal token/name pairs and no Configurations. Encoder-instance
discrimination requires successful capacities (VSC_1 total 4; VSC_2 total 2),
including each codec list. Both generic H264 option tests pin the full ordered
nine-resolution union, including 480×240.

Four targeted perturbations produced seven assertion failures: altered Quality
reason (two), stale bare profile names (two), wrong VSC_1 capacity (one), and missing
480×240 from the union (two). All perturbations were restored exactly; the 17
affected tests passed. The final workspace runs passed 1,310 all-feature and
1,198 default tests, with five ignored and 41 suites each; both Clippy modes
passed. These controls establish the strengthened assertions' sensitivity, not
independent conformance or a new whole-workspace mutation campaign.

## Assertion review boundaries

The selected Mock/schema and CLI/navigation assertion audit found useful
overlapping boundaries, not a reason to delete tests based on similar names.

- Some profile/PTZ identity differential expectations come from another instance
  of the same implementation. Literal tokens, distinct head positions, state and
  hooks add useful checks; they do not independently validate all semantics.
  The profile snapshot race test is a bounded probe, not a forced-interleaving proof.
- Advertised encoder/audio option loops check self-consistency. Separate explicit
  value/list assertions guard important examples; the loops alone do not establish
  exhaustive non-vacuous codec validation.
- The initial encoder Fault-detail and bare-profile replay gaps are closed by
  T02 above; V01 separately checks Type=All profile readback. The shared retirement
  mutation does not establish independent sensitivity for every action entry.
- CLI schema checks alone do not pin every command's metadata. Some maintenance
  refusals still use `is_err`. The manage unit decision table alone did not prove
  end-to-end cancellation; the ConPTY run supplies that additional evidence.
- Corpus export is not validation. Ignored structural tests are not passes;
  external XSD results remain separate from lexical, semantic, HTTP and hardware
  acceptance. These limits do not describe newly observed product defects.
- T02 closes the encoder-instances row's ability to distinguish two errors
  rather than requiring two successes.
  The generic encoder-options widths 2592 and 352 already distinguish a VEC_1
  fallback: 352 is absent from that encoder. The initial audit said otherwise;
  checking the literal catalogue corrected that assessment. The Action snapshot flattens
  success to `ok` and drops Fault subcode/detail; it is not a lossless payload
  oracle. Preserve these limits when interpreting its coverage counts.

## Handoff boundary

G01/G03 are LOCAL-PASS for the included cut: all changed inputs, affected consumers,
assertions and public claims are reconciled; findings A01–A05 are locally repaired.
The remaining scoped product limits are recorded in the backlog, not silently
promoted to conformance. G05–G09 retain their separate dispositions. Maintainer CI and
non-publishing staging require repository write permission and must record the
exact candidate SHA; commands are in the [approval packet](release-0.17-approval.md#maintainer-actions).
A CI run on an earlier SHA cannot accept the later repairs or workflow guards.

Keep library/CLI versions at 0.16.0 until the reserved version change is approved.
Do not merge master/develop, create a tag, publish packages/releases, close PRs
or install the candidate into the host system during this work. Ordinary repair
commits do not constitute the formal 0.17 version commit.
