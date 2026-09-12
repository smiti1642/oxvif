# 0.17 release cut and acceptance

[English](release-0.17-cut.md) | [繁體中文](release-0.17-cut_zh.md)

Status: IN-PROGRESS / NOT RELEASE-READY. Authorized 2026-09-11.
Update 2026-09-12: the operator reopened CLI scope for manage discovery filters,
F12 status-bar design and menu-position retention. See [CLI re-entry](cli-0.17-reentry.md).
Earlier acceptance and CI are historical evidence, not validation of these new bytes.
The later [basic Mock Fleet](mock-fleet-basic-plan.md) is implemented with local
evidence, but release inclusion is undecided. Current development is on
`feat/basic-mock-fleet` (`32fcc77` before this documentation reconciliation),
including CLI `dcbff41`; remote master/develop were both `5a1821b` at inspection.
Comparison base: `v0.16.0`; frozen scope baseline:
`f9448e515baec6169f40ec7f38ec2e0fcc752826` (evidence update `2f92b75`).
This records the original frozen scope, not a claim that the hardening ancestry is accepted.
The K27 repair below follows that baseline within the approved integrity scope.

The [pre-version review packet](release-0.17-approval.md) tracks the user's four
requested steps, A01/A02 repairs and the [shared snapshot repair](snapshot-auth-repair.md),
maintainer staging instructions and the version edits reserved for approval.
The earlier local complete-candidate/security review is closed for its recorded
baseline; CLI re-entry, installation and final acceptance retain separate gates.

| Section | Purpose |
| --- | --- |
| [Included scope](#included-scope) | Bounded batches and their evidence |
| [Blocking acceptance](#blocking-acceptance) | Conditions that cannot be deferred |
| [Publication surfaces](#publication-surfaces) | Short summary, full record and registry links |
| [Execution order](#execution-order) | Resume without conversation history |
| [Evidence](#evidence) | Exact results and unavailable checks |

## Included scope

The original cut excluded new user-facing features. The operator subsequently
authorized the CLI re-entry above as an explicit exception. Other discoveries
are triaged by severity, not silently added; Mock Fleet remains outside confirmed
release scope until a separate decision, even though it is present in the development branch.

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
| G01 Complete candidate review | BASELINE LOCAL-PASS; CURRENT DELTA OPEN | The 208-path review and recorded deltas remain accepted only for their input hashes in the [review record](release-0.17-review.md). Reconcile subsequent CLI and any included Fleet changes against the final candidate; passing tests do not extend the frozen review ledger |
| G02 Data integrity, K27 | LOCAL-PASS | Collision buckets preserve distinct requests across record/load/save and request-aware replay; key-only ambiguity returns None; equivalent requests still replace and credential cleanup remains targeted. Report groups retain all rows. See the K27 evidence below; G05 remains separate |
| G03 Security and response integrity | BASELINE LOCAL-PASS; CURRENT DELTA OPEN | A01–A05 remain repaired with recorded assertions. Reconcile later terminal behavior and any included Fleet LAN/control/discovery exposure; documented limits and local tests do not establish whole-candidate security acceptance |
| G04 Local code and documents | LOCAL-PASS, Windows development batch | At B4: all-features 1,316 passed / 0 failed / 6 ignored; default 1,204 / 0 / 6. Both Clippy/strict rustdoc modes and fmt passed; [exact evidence and exclusions](mock-fleet-basic-plan.md#local-evidence). Rust 1.88/schema evidence from earlier batches is historical, not a new run. Revalidate affected gates after version changes, scope removal or merges |
| G05 Native CI | HISTORICAL PASS; CURRENT PENDING | [Run 34672460802](https://github.com/smiti1642/oxvif/actions/runs/34672460802) passed all 27 jobs/five native targets at fed6777; it does not cover later CLI/Fleet runtime or CI-step changes. Dispatch and record the final candidate separately |
| G06 Package and distribution | PARTIAL | Package/docs passed at fed6777: library package verification, CLI package file listing, archive controls and docs. Listing is not CLI package/install verification; final-version packages, portable installs, SBOM/checksums and non-publishing staging remain required |
| G07 Human and Agent acceptance | PARTIAL | CLI dcbff41 Windows ConPTY covers manage discovery search/registration filters, exact selection, resize, bottom status and menu retention; B4 separately covers four-camera CLI reads/restart/cleanup. Keep remaining navigation/IME/other-platform checks and native LAN/VMS acceptance explicit; historical hardware evidence is not a fresh run |
| G08 Versions and release links | OPEN | Update library/CLI versions together after candidate acceptance; preserve schema-v3 claims only if tests agree, resolve every draft link to the final tag, keep migration warnings visible |
| G09 RC and approval | NOT-RUN | Publish an RC only with explicit authorization; suggested 3–7 day observation, no calendar-based automatic success; obtain final release approval |

`tests/mock_replay_key_gaps.rs` now asserts retention rather than reproducing loss:
ten collision pairs retain both requests across record/load/save, public lookup,
reports and both replay transports. Old recordings already overwritten cannot be
recovered; old readers can collapse the unchanged JSON shape on downgrade.
See [storage and report migration](../replay-storage.md).

A01 repairs invalid HTTP UTF-8 handling. Remaining HTTP binding and fault/field
semantics were triaged against the included claims and retain the documented
subset limits in the backlog. A demonstrated severe failure affecting this cut
must still block release; local review does not waive future findings.

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

Steps 1–3 below record the original construction order. For the current candidate,
first decide Fleet release placement, reconcile the later G01/G03 deltas and
use the updated [maintainer instructions](release-0.17-approval.md#maintainer-actions)
for exact-revision CI/staging. Removing Fleet from the release also changes the
tested input and requires affected verification; do not reuse the combined count unchanged.

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

### Native CI maintenance-test follow-up

The maintainer's [run 34594415559](https://github.com/smiti1642/oxvif/actions/runs/34594415559)
tested `111c185`. Windows, Linux x64/ARM and macOS ARM test jobs passed. macOS
Intel failed `chunked_snapshot_limit_is_enforced_without_content_length`,
`diagnostic_picker_retains_evidence_and_never_falls_back`,
`fleet_diagnose_retains_partial_and_total_failure_evidence` and
`managed_session_reuses_expires_and_preserves_no_clobber`. Package/docs was
skipped; native credential, CLI smoke and selected external-schema jobs passed.
This is partial native evidence, not a green release gate.

The original maintenance tests passed locally (19 tests). All four failures use
the shared two-second functional network budget. Their original assertions
omitted the actual error/diagnostic sections, so runner contention is a working
hypothesis rather than a proven explanation of the macOS failures.

The follow-up changes only `maintenance.rs`'s `cfg(test)` module:

- A bounded 30-second functional budget also covers fixture-server acceptance;
  explicit 10 ms SOAP and 100 ms snapshot deadline tests retain their own budgets.
- Delayed HTTP and SOAP fixtures exceed the old two-second budget and must still
  return exact image bytes/type and the distinctive SOAP-stage result.
- The chunked size test requires the exact 16 MiB refusal and non-retryable
  classification, not merely any error. The stalled test requires timeout evidence.
- Export completeness, diff results and picker/fleet failures expose their
  structured evidence while preserving no-clobber, selected-profile, exit-code,
  session reuse/expiry, fault and cancellation assertions. No test is skipped and
  production timeouts, image limits, retries and CLI output are unchanged.

The focused revised suite passed 20 tests. One full workspace all-feature
`--no-fail-fast` mutation campaign then reinstated the two-second budget, allowed
one extra image byte and replaced both timeout messages with an unrelated
retryable error. Exactly three assertions failed: the delayed progress, chunked
limit and stalled snapshot tests (1,298 passed, three failed, five ignored across
41 suites). The delayed test fails at its SOAP-stage assertion first; this does
not establish independent mutation coverage for every subsequent assertion.
All temporary mutations were restored before the final gates. Native acceptance
still requires a new run of this follow-up, not a rerun of the old commit.

Final local gates after restoration:

| Check | Result |
| --- | --- |
| Workspace all-features / default, locked, no-fail-fast | 1,301 / 1,191 passed; five existing ignored each, 41 suites |
| Workspace Clippy all-features / default, all-targets | Passed with `-D warnings` |
| Formatting and diff whitespace | Passed |
| Production boundary | `maintenance.rs` before `cfg(test)` matches `111c185`; no product behavior change |
| Documentation | 44 local Markdown file targets checked; published CHANGELOG content unchanged |
| Hosted acceptance | New run still required; current GitHub CLI inspection reports READ permission |

No new native-platform, package/install, hardware, MSRV or external-schema run is
claimed for this test-only follow-up. Previous production-code evidence remains
historical; it does not close G05 or the other outstanding release gates.

### Pre-version acceptance follow-up

The preceding run-required statements are historical. The maintainer's
[run 34597167495](https://github.com/smiti1642/oxvif/actions/runs/34597167495)
passed all 27 jobs at `3eccfd15274d4e978501634e4155477760502b6b`, including
Windows, Linux x64/ARM and macOS Intel/ARM tests, smoke and native credentials.
Package/docs verified the library but only listed CLI package files. This does
not accept later fixes or distribution installation.

Review finding A01: HTTP `from_utf8_lossy` changed invalid bytes into U+FFFD and
successfully created a different profile. The new HTTP entry guard rejects invalid
UTF-8 with HTTP 400 / Sender / mock:RequestPolicy before the responder chain.
It does not redesign Content-Type, declared charset or general fault status mapping.
The full workspace all-feature pre-fix sensitivity run had 1,301 passes, two
assertion failures and five ignored (41 suites). The profile test observed an
actual state mutation; the second test observed the armed-fault path returning 200.
After the fix: 1,303 all-feature / 1,193 default passes, five ignored each;
both workspace Clippy modes and strict rustdoc modes passed. Five invalid byte
patterns are refused; valid Chinese/U+FFFD roundtrips and an armed fault survives.

The user requested a stop before the formal 0.17 version/release commit. Versions
remain 0.16.0; no tag, publication or main-branch merge is authorized by these checks.
