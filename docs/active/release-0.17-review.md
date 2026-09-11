# 0.17 candidate review batches

[English](release-0.17-review.md) | [繁體中文](release-0.17-review_zh.md)

Status: IN-PROGRESS. Updated 2026-09-12. This is an execution record, not release
approval. [Approval packet](release-0.17-approval.md) and
[release gates](release-0.17-cut.md) remain authoritative.

| Section | Purpose |
| --- | --- |
| [Inventory](#inventory) | Frozen inputs and honest coverage |
| [Batch order](#batch-order) | Work remaining before closure |
| [A04 human-output repair](#a04-human-output-repair) | Reproduction, repair and checks |
| [V01 encoder replay coverage](#v01-encoder-replay-coverage) | Expanded tests, not a new production repair |
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

## Batch order

Work by complete service subgroup. Combine fixes in a coherent batch before
running the complete sensitivity/restored gates; do not repeat the entire
workspace after each file or documentation edit.

| Batch | Frozen paths | Inspected in this pass | Remaining closure |
| --- | ---: | --- | --- |
| CLI | 18 | Changed application/contract/registry/main/descriptor/Agent/schema paths; maintenance/manage/navigation/preferences production; interactive rendering/lifecycle; human-output boundaries | Remaining test diffs and unchanged consumers; executable and final interactive-terminal acceptance; package prose |
| Library | 26 | Media1/2/session/type/XML changed paths and notification listener, including its inherited HTTP reader | Public migration/reexport/examples and all affected readers/writers; reconcile exact prior snapshot evidence; notification limitations and assertions |
| Mock | 54 | Scoped request tree, authentication, receipt policy, responder production, server/transport deltas | Dispatch/fault/shared-state and every included service subgroup; reconcile read/write/options/capability/replay and inherited helper consumers against operation cards |
| Replay | 7 | Storage/report changes reconciled with K27 assertions and bilingual migration; typed-adapter projection reviewed; committed-effect implementation and selected assertions inspected | Finish adapter policy/auth chain and cross-service profile/binding/reference consumers; this does not close all replay dependencies |
| Delivery | 19 | Prior CI/package evidence exists; no new closure claimed | Source/tool versions, workflow inputs, schema tools, SBOM/checksum/install assertions; new native/staging runs |
| Documents | 84 | Release-cut/approval/backlog boundaries and A04 claim delta | Reconcile final claims across guides/READMEs/manpage, paired links and published history; do not repeat historical tests solely for prose |

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

## Handoff boundary

G01/G03 remain open until the remaining rows are reconciled. Maintainer CI and
non-publishing staging require repository write permission and must record the
exact candidate SHA; commands are in the [approval packet](release-0.17-approval.md#maintainer-actions).
A CI run on an earlier SHA cannot accept A04.

Keep library/CLI versions at 0.16.0 while this pre-version review is incomplete.
Do not merge master/develop, create a tag, publish packages/releases, close PRs
or install the candidate into the host system during this work. Ordinary repair
commits do not constitute the formal 0.17 version commit.
