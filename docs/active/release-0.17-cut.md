# 0.17 release cut and acceptance

[English](release-0.17-cut.md) | [繁體中文](release-0.17-cut_zh.md)

Status: PUBLICATION AUTHORIZED. Updated 2026-09-14.
The maintainer published both crates and authorized v0.17.0 tagging and GitHub
Release. The preparation checklist below retains its original order; its approval
stop is now satisfied. See the finalization record for publication outcome.
The user authorized version/document preparation after reporting CI and manual
acceptance passed. This cut includes the merged CLI continuity and basic Mock
Fleet work. Host-system installation remains outside this authorization.

| Section | Purpose |
| --- | --- |
| [Included scope](#included-scope) | Bounded release content |
| [Blocking acceptance](#blocking-acceptance) | Current gates |
| [Publication surfaces](#publication-surfaces) | Version and documents |
| [Execution order](#execution-order) | Remaining actions |
| [Evidence](#evidence) | Historical, revision-specific records |

## Included scope

R01–R08 remain the bounded CLI, shared XML/auth, selected Media/Mock state,
receipt-only operations, notification listener, dependencies and replay-integrity
cut recorded in the [review](release-0.17-review.md) and
[full changelog](../releases/0.17.0-changelog.md).
The approved later scope includes manage workflow continuity, responsive lists,
contextual settings/key help and [basic Mock Fleet](../mock-fleet.md).
No all-service fidelity, ONVIF certification, RTSP streaming, brand emulation or
long-duration 64/256-camera VMS stability is claimed.

## Blocking acceptance

| Gate | Current disposition | Evidence / remaining boundary |
| --- | --- | --- |
| G01 Candidate review | LOCAL-PASS for bounded cut and reviewed CLI/Fleet delta | See [finalization review](release-0.17-finalization.md#delta-review); original 208-path ledger is not silently extended |
| G02 Replay/data integrity | LOCAL-PASS | K27 collision retention and migration warnings remain; overwritten old recordings cannot be recovered |
| G03 Security/response integrity | LOCAL-PASS for reviewed scope | A01–A05 plus CLI/Fleet delta review; documented HTTP/listener/recording limits remain |
| G04 Local code/documents | PASS for version-preparation checks | Workspace check, strict rustdoc, package verification and version/help/link checks; previous full-suite totals remain historical |
| G05 Native CI | PASS, 0.17 candidate 9305f5d | [All 27 jobs](https://github.com/smiti1642/oxvif/actions/runs/34813217979) passed |
| G06 Packages/distribution | PASS, 0.17 non-publishing staging | [17 verification jobs](https://github.com/smiti1642/oxvif/actions/runs/34813220307) passed; both local packages, five native artifacts, APT/Homebrew installs, SBOM and checksums verified; not official-channel admission |
| G07 Human/Agent | Scoped automated evidence plus operator-reported manual PASS | Discover/manage/resize ConPTY and schema 3/guide 8; no inferred additional OS, device or VMS coverage |
| G08 Versions/documents | PREPARED | Both packages/dependency/lockfile 0.17.0; paired documents and tag-pinned links; release date/status intentionally pending |
| G09 Publication approval | AUTHORIZED | Maintainer published both crates on 2026-09-14; v0.17.0 tag and stable GitHub Release authorized; no RC required |

## Publication surfaces

- Root and CLI READMEs use prepared 0.17 examples, publication warnings and
  absolute final-tag links. Both library quick-start APIs remain.
- Short release notes link to the full technical/migration changelog.
- Existing English/Traditional Chinese public guide pairs describe included
  behavior and limitations. Historical 0.16 comparisons remain intentional.
- `Status: Unreleased` and `## [0.17.0] - Unreleased` remain publication guards.
  Future `blob/v0.17.0` links are structurally checked locally, not claimed live.
- See the [approval packet](release-0.17-approval.md) for remaining publication steps.

## Execution order

Steps 1–3 are complete. The preparation branch was removed after fast-forward
integration into master/develop; the next action is publication approval.

1. Commit the version/document preparation on `codex/release-0.17-finalize`.
2. Run CI and manual non-publishing release staging on the exact candidate SHA.
3. Record results; synchronize main branches only after required gates pass.
4. Present remaining limitations and request explicit publication authorization.
5. Only after approval: finalize date/status, publish library before dependent
   CLI, create the approved tag and release. Revalidate any code/tooling changes.

## Evidence

The following is historical evidence, not the current gate table. Statements
about earlier branches, versions, blocked CI or pending authorization apply only
to their dated inputs. Current results are in [finalization](release-0.17-finalization.md).

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
