# 0.17 release cut and acceptance

[English](release-0.17-cut.md) | [繁體中文](release-0.17-cut_zh.md)

Status: IN-PROGRESS / NOT RELEASE-READY. Authorized 2026-09-11.
Comparison base: `v0.16.0`; frozen scope baseline:
`f9448e515baec6169f40ec7f38ec2e0fcc752826` (evidence update `2f92b75`).
This freezes scope, not a claim that the hardening ancestry is accepted.
The K27 repair below follows that baseline within the approved integrity scope.

| Section | Purpose |
| --- | --- |
| [Included scope](#included-scope) | Bounded batches and their evidence |
| [Blocking acceptance](#blocking-acceptance) | Conditions that cannot be deferred |
| [Publication surfaces](#publication-surfaces) | Short summary, full record and registry links |
| [Execution order](#execution-order) | Resume without conversation history |
| [Evidence](#evidence) | Exact results and unavailable checks |

## Included scope

No new user-facing feature enters this cut. Fixes required for the following
contracts are allowed; discoveries are triaged by severity, not silently added.

| ID | Included behavior | Source and verification entry points |
| --- | --- | --- |
| R01 | CLI maintenance, manage, profile metadata, human/Agent parity | `crates/oxvif-cli/src/{maintenance,manage,interactive,describe,agent,output}.rs`; CLI unit/executable tests; [maintenance acceptance](../cli-maintenance.md#manual-acceptance) |
| R02 | Vim core, line preferences, layout and cancellation | `navigation.rs`, `ui_settings.rs`, `interactive.rs`; `tests/navigation_core.rs`, `tests/cli.rs`; [prior terminal evidence](cli-vim-navigation-plan.md) |
| R03 | Selected common XML/Action/Fault/auth boundaries and literal identity | W03–W08 implemented slices, P1/P2, PTZ1, A1; `request`, `dispatch`, `fault`, `auth`, shared escaping; request/identity/auth/adapter tests |
| R04 | Media profile create/read/delete/binding, source, rate, encoder, audio/metadata | E1, PA1, VS1, rate correction, VE1, AM1; [operation ledger](mock-fidelity-operation-ledger.md), service cards, profile/source/rate/encoder/audio tests; both service views and transports |
| R05 | Selected atomic snapshots/hooks and committed replay dependencies | W18/W19 implemented slices; `state.rs`, `responder.rs`, `metamorph/replay.rs`; reentrant-hook, profile/read/replay tests |
| R06 | Thirteen classified receipt-only operations, including Media sync | A2/A3 plus B16; `policy.rs`, `mock_ack_policy`, `mock_media_sync`, operation cards; no physical-effect claim |
| R07 | Notification peer wrapper/listener and compatibility | B17; `client/events.rs`, public notification types, `notification_origin` tests |
| R08 | Dependency, XML compatibility, source SBOM and verification tooling | B14 plus already included dependency work; lockfile, schema tooling, release workflow, downstream XML tests, source-SPDX controls |

These are selected contracts, not completion of W00–W26 or every operation in a
service. An unchanged handler can still be affected by shared parsing/fault code;
review all transitive consumers of an included helper. Public claims may not say
“fully hardened Mock” or “all ONVIF behavior validated.”

## Blocking acceptance

| Gate | Current disposition | Required closure |
| --- | --- | --- |
| G01 Complete candidate review | OPEN | Review all differences from v0.16.0, not only PRs #14/#16/#17; verify each included batch's read/write/options/capability/replay closure and migration |
| G02 Data integrity, K27 | LOCAL-PASS | Collision buckets preserve distinct requests across record/load/save and request-aware replay; key-only ambiguity returns None; equivalent requests still replace and credential cleanup remains targeted. Report groups retain all rows. See the K27 evidence below; G05 remains separate |
| G03 Security and response integrity | OPEN | Disposition of known credential leaks, partial writes, unsafe URLs, ambiguous effect receipts and shared-boundary regressions; a severe known issue cannot become future work by relabeling it |
| G04 Local code and documents | LOCAL-PASS baseline | Reuse exact unchanged-code evidence; revalidate affected gates after fixes, version changes or merges |
| G05 Native CI | BLOCKED | Actual Windows/Linux/macOS results on final candidate; previous workflow dispatch returned HTTP 403 and no run exists |
| G06 Package and distribution | PARTIAL | Library dry-run passed at 0.16.0; final-version library/CLI packages, native credential/portable-install checks, source/binary SBOM, checksums and non-publishing staging remain required |
| G07 Human and Agent acceptance | PARTIAL | Existing Windows synthetic terminal/executable evidence; final-candidate terminal resize/cancel/input plus real-camera read-only snapshot/diagnose/export/diff evidence. No secrets/images in public evidence |
| G08 Versions and release links | OPEN | Update library/CLI versions together after candidate acceptance; preserve schema-v3 claims only if tests agree, resolve every draft link to the final tag, keep migration warnings visible |
| G09 RC and approval | NOT-RUN | Publish an RC only with explicit authorization; suggested 3–7 day observation, no calendar-based automatic success; obtain final release approval |

`tests/mock_replay_key_gaps.rs` now asserts retention rather than reproducing loss:
ten collision pairs retain both requests across record/load/save, public lookup,
reports and both replay transports. Old recordings already overwritten cannot be
recovered; old readers can collapse the unchanged JSON shape on downgrade.
See [storage and report migration](../replay-storage.md).

HTTP binding/UTF-8 handling and remaining fault/field semantics need G01/G03
triage for their impact on included claims. A full redesign may be deferred;
a demonstrated severe failure affecting this cut may not.

## Publication surfaces

- `docs/releases/0.17.0.md` / `_zh.md`: concise user-facing summary, consumed by
  the existing release workflow's `--notes-file`; no workflow bypass is needed.
- `docs/releases/0.17.0-changelog.md` / `_zh.md`: complete grouped change record,
  breaking changes, migration examples, verification scope and limitations.
- `CHANGELOG.md`: compact Unreleased summary linking to the full record; do not
  rewrite published historical entries.
- Root and CLI package READMEs: short “next release” link while developing.
  Before publication both crates.io pages must point via absolute GitHub URLs to
  the full record under the actual release tag. crates.io does not auto-render
  the repository changelog.
- Keep current install commands at published 0.16 until a releasable version is
  prepared. Draft files retain `Status: Unreleased` so the existing release
  contract refuses premature publication.
- Final links use `blob/v0.17.0/...`, not master/develop or a temporary branch;
  RC links must use their own actual RC tag.

## Execution order

1. Commit the scope, paired full/summary documents and [follow-up backlog](post-0.17-backlog.md).
2. Recheck selected contracts using existing batch suites and bounded independent
   controls. Record failures before fixing; do not count known-gap tests as fixes.
3. Resolve G02 and G01/G03 findings with separate cohesive fixes. Preserve original
   contributor credit. Do not broaden this into all-service migration.
4. Restore workflow permission or ask the maintainer to run CI on the exact
   candidate. Do not change triggers, open a PR or switch credentials to evade 403.
5. After local/hosted acceptance, prepare the actual RC/version and rerun affected
   package/link/CLI-version gates; run staging with `publish=false`.
6. Complete G07/G09, then ask for publication approval. Main-branch merge, tags,
   crates.io publishing and system installation are not implied by a green test.

The [follow-up backlog](post-0.17-backlog.md) schedules remaining work without
marking historical PARTIAL/TODO rows complete. This cut owns release scheduling;
the operation ledger and evidence cards remain the technical source of truth.

## Evidence

Baseline evidence is in [contributor integration](contributor-pr-integration-plan.md#execution-record):
1,298 all-feature and 1,190 workspace-default tests passed, each with five ignored;
both Clippy modes, fmt, strict docs, Rust 1.88, isolated-feature checks and XML
encoding-off/on controls passed. Selected external corpus: 160 XML instances,
80 exchanges, 46 operations, 21 refusals; not whole-program conformance.
Library package dry-run passed; no upload occurred. These results do not close
the known storage defect, new version packaging, native CI or hardware gates.

2026-09-11 scope/document acceptance rerun at `e661781`, unchanged implementation
(historical baseline, before the K27 repair):

| Check | Observed result |
| --- | --- |
| Workspace all-features / default, no-fail-fast | 1,298 / 1,190 passed; five ignored each, 41 suites |
| Both workspace Clippy modes and fmt | Passed |
| Packaging/schema-tool unit controls | 24 passed in the pinned external Python environment |
| Inventory self-tests | Passed; 159 routes, 161 Action sites, 191 reader sites |
| Cargo audit | Passed; 410 locked dependencies scanned |
| Markdown | 466 local targets checked; new release-document local anchors checked; published CHANGELOG history unchanged |
| Existing release-note consumer | Source review confirms it reads the short version file; no workflow change or remote publication |
| K27 | Existing executable regression still reproduces storage overwrite; G02 remains BLOCKED |
| CI dispatch | Reattempt returned HTTP 403; no new run, G05 remains BLOCKED |

That baseline commit changes documentation only. Previously verified MSRV, strict rustdoc,
XML feature and external-corpus results are reused for identical Rust/lockfile
inputs, not represented as newly executed. No new real-camera/terminal session,
final-version package or installation test is claimed. Versions remain 0.16.0
until a releasable candidate is prepared; no published artifacts were changed.

### K27 collision-retention repair

2026-09-11, following `e661781`. The storage index now holds collision buckets;
replacement requires equivalent sanitized requests. Public request-aware lookup
selects a unique match, key-only lookup refuses ambiguity, and legacy-key cleanup
does not merge distinct requests. `QuirkDiff` retains ambiguous rows as unmatched
observations rather than dropping rows or inventing correspondence.

The collision regression covers ten synthetic pairs, including whitespace,
namespace/structure/attribute boundaries, body and unqualified-header fields,
mixed content, trailing roots and QName bindings. Each pair checks distinct
payloads, save/load/source-file preservation, same-request replacement, report
rows/progress, key-only refusal and in-process/HTTP replay. Qualified ephemera
and ordinary token/Action controls remain positive. Legacy URL cleanup has
separate same-request merge and distinct-request retention controls.

One workspace-wide all-feature `--no-fail-fast` mutation campaign reinstated
same-key replacement and report-map overwriting. Exactly three tests failed on
assertions, not compilation:

- `metamorph::fixture::tests::legacy_key_cleanup_preserves_distinct_colliding_requests`
- `metamorph::quirk::tests::colliding_report_rows_are_retained_without_an_invented_pairing`
- `legacy_key_collisions_preserve_both_recordings_and_replay_identity`

The mutation run had 1,297 passes, three failures and five ignored across 41
suites. The collision-loop mutation fails on its first case; this does not claim
independent mutation sensitivity for every XML comparison rule. Both mutations
were restored exactly before the following gates.

| Check | K27 result |
| --- | --- |
| Workspace all-features / default, no-fail-fast | 1,300 / 1,190 passed; five ignored each, 41 suites |
| Workspace Clippy, all/default, and fmt | Passed |
| Rust 1.88 workspace all-features/all-targets | Passed |
| Isolated library features | Clippy passed for default, health, mock, mock-server, metamorph, metamorph-server and serde |
| Strict workspace rustdoc, all/default | Passed with `-D warnings` |
| Markdown | 310 changed-document local file targets and 18 navigation anchors checked; published CHANGELOG content unchanged after line-ending normalization |
| Remote permission | Read-only inspection reports `viewerPermission=READ`; no new workflow dispatch, credentials change or bypass; G05 remains blocked |

No ONVIF method, SOAP parser, dependency, workflow or CLI implementation changed.
Earlier XML feature-unification and external-corpus evidence is reused for those
unchanged inputs, not claimed as a new schema run. No hardware, native-platform,
final-version package or installation acceptance occurred. G02 is locally closed;
G01/G03 complete-candidate and security review, G05–G09 remain as listed above.
