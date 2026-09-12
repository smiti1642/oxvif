# 0.17 pre-version review packet

[English](release-0.17-approval.md) | [繁體中文](release-0.17-approval_zh.md)

Status: IN-PROGRESS / NOT APPROVED. Updated 2026-09-12.
This packet supplements the [release cut](release-0.17-cut.md), not replaces its
blocking gates. The user requested a stop before the formal 0.17 version/release
commit. Ordinary repairs and evidence commits are allowed; version promotion,
main-branch merges, tags, publication, PR closure and host-system installation
are not performed by this acceptance work.

Current development includes CLI `dcbff41` and the locally verified Fleet batch
`6cf345c` (exclusion clarification `32fcc77`). Fleet release placement remains
undecided. The 208-path review and hosted CI below are historical baselines, not
automatic acceptance of these later changes. See the [updated gates](release-0.17-cut.md#blocking-acceptance).

| Section | Purpose |
| --- | --- |
| [Four requested steps](#four-requested-steps) | Current status without conflating evidence |
| [Findings](#findings) | Repairs and unresolved results |
| [Executed checks](#executed-checks) | Local, hosted and real-camera observations |
| [Review closure](#review-closure) | Completed local review and remaining release gates |
| [Maintainer actions](#maintainer-actions) | Non-publishing CI and installation staging |
| [Version-edit checklist](#version-edit-checklist) | Changes reserved for the approved version commit |
| [Approval boundary](#approval-boundary) | Conditions before asking for promotion |

## Four requested steps

| Step | Status | Remaining work |
| --- | --- | --- |
| 1. Complete candidate/security review | BASELINE LOCAL-PASS; CURRENT DELTA OPEN | The 208-path review and recorded follow-ups remain valid for their hashes; A01–A05 repaired, T01/T02 strengthened. Reconcile later CLI and any included Fleet changes before final-candidate approval |
| 2. Package/install and human/real-camera acceptance | PARTIAL | Native CI passed at fed6777, not at the later CLI/Fleet revisions. New bounded Windows ConPTY/local Fleet evidence is listed below; current-candidate CI, remaining human/platform checks and distribution staging are open. Historical real-device limits, including Hanwha non-image responses, remain |
| 3. Versions, links and documents | PREPARED, not promoted | Bilingual release records and this checklist updated. Actual versions remain 0.16.0; apply the version-edit checklist only after approval |
| 4. User confirmation | NOT REQUESTED for promotion | Present the final evidence and unresolved risks before the formal version commit; publication requires separate authorization |

## Findings

| ID | Observation | Disposition |
| --- | --- | --- |
| A01 | HTTP lossy UTF-8 decoding converted byte FF into U+FFFD and committed a differently named profile | Repaired in 6135e72: HTTP 400 / Sender / mock:RequestPolicy before responders. Five invalid-byte classes, valid Unicode, full-state/hook preservation and queued-fault control pass locally |
| A02 | Snapshot Authorization rewrote `qop=auth` as a quoted value | Remove the rewrite; assert unquoted qop/algorithm/nc and exact request URI. This follows [RFC 7616 §3.4](https://www.rfc-editor.org/rfc/rfc7616.html#section-3.4), not camera-specific downgrade behavior |
| A03 | Both saved-camera profiles returned HTTP 401 before and after A02; wider testing found 18 similar failures | REPAIRED locally: title-case HTTP/1.1 field names resolve case-sensitive firmware handling; shared CLI/health Digest core retains authentication and destination safeguards. Saved-camera profiles now save and decode successfully. See [repair evidence](snapshot-auth-repair.md); hosted release gates remain open |
| A04 | Human output emitted terminal commands embedded in camera/profile text, verbose details and error hints | REPAIRED locally: escape C0/C1 and bidirectional formatting controls at human report boundaries and single-line menu/context fields; JSON/JSONL values remain unchanged. Report LF/TAB layout remains allowed; this is not a guarantee against every multiline presentation ambiguity |
| A05 | Full-screen rendering still emitted directional formatting controls | REPAIRED in 214d989: escape before width, truncation, wrapping and cell layout; original selection data is preserved; a pre-repair assertion failed |
| C01 | Windows native failures could be hidden by later successful commands | REPAIRED in 210bfc3: four CI and fourteen release exit guards; local success/failure controls pass. Hosted CI passed at fed6777; release-workflow staging remains required |

A01 does not complete the general HTTP binding/charset/fault-status audit. A02
alone did not resolve A03. The A03 result below is scoped to tested devices and
profiles, not universal compatibility; remaining work stays explicit in the
[post-release backlog](post-0.17-backlog.md).

## Executed checks

| Evidence | Result and boundary |
| --- | --- |
| Latest combined development batch, 6cf345c | All-features: 1,316 passed / 0 failed / 6 ignored; default: 1,204 / 0 / 6, each 41 suites. Both Clippy/strict rustdoc modes and fmt pass. Three example tests and 64/256 loopback capacity smoke pass separately; [six exclusions and boundaries](mock-fleet-basic-plan.md#ignored-tests). No new MSRV/schema/native CI result |
| CLI re-entry, dcbff41 | All-features/default: 1,311 / 1,199 passed, zero failures and five ignores each. Windows ConPTY verifies actual manage discovery search/registration filters, exact saved/session-only selection, status bar, viewport retention, resize and cancellation/restoration; see [scope](cli-0.17-reentry.md#acceptance) |
| Fleet terminal, B4 | Two four-camera foreground serve cycles; CLI reads each configured identity, Ctrl+C exits zero, ports can be rebound and manifest hash is unchanged. Loopback only; native multicast/VMS not accepted |
| Historical pre-re-entry local closure, 2026-09-12 | All-feature/default workspace: 1,310 / 1,198 passed, five ignored and 41 suites each; both all-target Clippy modes, strict rustdoc modes, fmt and Rust 1.88 all-target/all-feature check pass. [A05/T01/T02 and native-exit evidence](release-0.17-review.md#a05-terminal-display-and-t01-assertion-follow-up) |
| Windows debug ConPTY | 40 synthetic devices, manage/detail/profile/input/password/resize, started snapshot cancellation and exact console-mode restoration on normal/Ctrl-C exits pass; configuration hashes unchanged. No new camera scan or host install; remaining G07 scope below |
| V01 expanded encoder replay, 2026-09-12 | Suppressed retirement: 1,308 pass / four assertion failures / five ignored, including two new and two existing rate tests. Restored: 1,312 all-feature / 1,200 default passes, five ignored each, 41 suites. Production unchanged from 1ea4fff; see [scope](release-0.17-review.md#v01-encoder-replay-coverage). Historical V01 result; final local closure is recorded below |
| A04 human-output repair, 2026-09-12 | Before repair: 1,308 pass / two assertion failures / five ignored. Repaired: 1,310 all-feature / 1,200 default passes, five ignored each, 41 suites; both Clippy and strict rustdoc modes pass. Actual debug executable preserves JSON/JSONL values and error exit 3 while escaping human stderr. See the [batch record](release-0.17-review.md#a04-human-output-repair); this is not full-screen terminal acceptance |
| Hosted CI at 3eccfd15274d4e978501634e4155477760502b6b | [Run 34597167495](https://github.com/smiti1642/oxvif/actions/runs/34597167495): all 27 jobs passed, five native targets; does not cover later repairs |
| A01 sensitivity | Full workspace all-features, no-fail-fast: 1,301 pass / two assertion failures / five ignored; actual unwanted state mutation observed |
| A01 restored gates | 1,303 all-feature / 1,193 default passes, five ignored each, 41 suites; both Clippy and strict rustdoc modes, fmt, local links and published-history check pass |
| A02 sensitivity | Full workspace all-features, no-fail-fast: 1,302 pass / one assertion failure / five ignored; outgoing quoted qop fails the wire-format assertion |
| A02 restored gates | 1,303 all-feature / 1,193 default passes, five ignored each, 41 suites; both workspace Clippy modes pass. Strict rustdoc results from A01 apply to unchanged public documentation; current format/link checks accompany the commit |
| Fresh dependency checks | cargo audit: 410 locked packages, no reported vulnerabilities. cargo outdated reports reqwest 0.13.5, tokio-rustls 0.26.5, toml 1.1.6 and dev-only dirs 7 available; keyring 4 migration emits obsolete-feature warnings. No lockfile change; these updates need a separate reviewed batch, not an automatic pre-release upgrade |
| Real camera, Windows x64 | GeoVision_2 GV-TBL8810, firmware V111_2025_12_09; info and two profile queries exit 0 |
| Before shared repair: real diagnosis, both profiles | exit 20, complete=false: five stages pass, snapshot_fetch returns 401, RTSP transport and decode remain not_tested. Error reports preserve partial evidence; no playback claim |
| Before shared repair: human table / Agent version | Plain table also exits 20 and names the 401/partial result/playback limit; not an interactive terminal test. Executed guide reports CLI 0.16.0, schema 3 and guide 8 |
| Real export/diff | Eight sections pass; export exit 0/complete=true; second export to the same path exits 4/RESOURCE_ALREADY_EXISTS and file hash is unchanged; live diff exits 0/complete=true/matches=true, zero changes |
| Privacy boundary | Only read-only camera calls. Baseline inventory remains in a private temporary directory outside Git; no IP, serial, credentials, URI, digest challenge or image is recorded here |
| Earlier documentation/inventory | fmt and diff whitespace pass; 123 local Markdown targets checked, published CHANGELOG history unchanged. Inventory self-tests pass: 159 routes, 161 Action sites, 191 readers; no normative/transitive-graph acceptance implied |

The preceding real-device rows record the pre-repair baseline. Updated evidence:

| Shared repair, Windows x64 | Result and boundary |
| --- | --- |
| Candidate CLI SHA-256 | `9F2856CC727596791945A2DCC3F0243E3FB1ECA2CB09D054B5C0E894FEA1DF97`; ordinary local debug build, not a published release binary |
| Full sensitivity campaign | Corrupt the outgoing Digest response: 1,306 pass / two assertion failures / five ignored, 41 suites. Restore production source exactly; no compile-only failure |
| Restored gates | 1,308 all-feature / 1,198 default passes, five ignored each, 41 suites; both workspace Clippy and strict rustdoc modes, fmt pass |
| Previously failing fleet | First-profile sample of 18 devices: 17 succeed with an 8-second budget; one times out, then succeeds in a single 20-second-budget retry. All 18 yield JPEG signatures; do not report the first run as 18/18 |
| Passing control / Hanwha | Control remains successful. Hanwha XND-C6083RV still fails image recognition, including a 20-second retry; no automatic MJPEG profile or CGI change |
| Original saved GV-TBL8810 | Both profiles diagnose with exit 0 / complete=true and save with exit 0; independent System.Drawing decoding confirms 640×360 images. Repeated save exits 4 and preserves each file hash |
| Privacy / interpretation | Two images remain in a private temporary directory outside Git. No camera setting changed. RTSP transport and video decoding remain not_tested; sampled snapshots do not establish brand-wide or all-profile conformance |

No IP, credentials, URI/query, authentication header or actual camera image is
included in public evidence. These repaired local results do not replace hosted,
terminal or distribution-staging acceptance.

## Review closure

G01/G03 are LOCAL-PASS for the recorded R01–R08 baseline, with later CLI/Fleet
delta reconciliation still open. The [per-file ledger](release-0.17-review-ledger.json)
contains 208 reviewed paths from `v0.16.0..b7bc881`, plus separately reviewed later
deltas. Each reviewed input has a blob ID; the ledger itself is identified by its
containing commit. The six groups in the [batch review](release-0.17-review.md#batch-order)
record source, affected consumers, assertions and public-claim reconciliation.

The earlier `v0.16.0..3eccfd1` comparison had 200 files, 39,614 added and 3,007
removed lines. It is a historical checkpoint, not the final reviewed input.
A01–A05 repairs, T01/T02 test-value work, CI/release guard controls and bilingual
claim corrections close the locally actionable findings. The included cut has
no remaining known severe blocker identified by this review.

This is bounded engineering review, not independent whole-program or ONVIF
certification. Full HTTP/field/Fault semantics, inherited listener hardening,
raw-recording privacy limits, concurrent replay visibility and snapshot-format
compatibility remain explicitly scoped in the [backlog](post-0.17-backlog.md).
Historical external-corpus and hardware evidence applies only to its matching
inputs. G05 passed at `fed6777` in [run 34672460802](https://github.com/smiti1642/oxvif/actions/runs/34672460802).
Later CLI/Fleet runtime, CI-step and documentation edits are not covered by that run;
G05 remains pending for the current candidate. G06 still requires
non-publishing staging and final-version package acceptance; G07 remains partial,
and G08/G09 retain their approval boundaries.

## Maintainer actions

Use the maintainer's authorized account with workflow permissions. Earlier READ/403
observations in linked records are historical, not current account policy.
Do not switch accounts or open a workaround PR to bypass permission failures.
The current development branch includes Fleet; testing it does not approve Fleet
for release. After confirming the desired candidate, the maintainer can run:

```powershell
$candidateBranch = 'feat/basic-mock-fleet'
git fetch origin $candidateBranch
if ($LASTEXITCODE -ne 0) { throw 'Fetch failed' }
$candidate = git rev-parse "origin/$candidateBranch"
if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve candidate' }
gh workflow run ci.yml --ref $candidateBranch
if ($LASTEXITCODE -ne 0) { throw 'CI dispatch failed' }
gh workflow run release.yml --ref $candidateBranch -f tag=$candidate -F publish=false -F prerelease=true
if ($LASTEXITCODE -ne 0) { throw 'Staging dispatch failed' }
```

Record each run URL and exact checked-out SHA; a branch moving after dispatch
must not silently change the accepted candidate. The staging tag input is a
filename-safe commit SHA, not the slash-containing branch name. Despite the
input's name, this command does not create a Git tag. `publish` must remain false.

Acceptance requires Windows/Linux/macOS archives, checksums, source/binary SBOMs,
Debian package install/remove, signed ephemeral APT repository install on both
Linux architectures, and Homebrew formula/bottle install/reinstall on both Mac
architectures. These are CI-runner installs, not changes to the user's machine.
Current 0.16-valued staging cannot substitute for final 0.17 package verification.
Official channel admission and production APT signing keys remain separate work.

The bounded current Windows ConPTY run accepts the manage/profile/input/resize,
started-snapshot cancellation and console-restoration paths listed above. Prior
Vim/discovery evidence retains its recorded revision. The later dcbff41 manage
discovery/status/viewport checks and B4 loopback Fleet checks above are additional,
not replacements for untested paths. Complete the remaining
final-candidate discover/diagnose navigation, filter/counts/gg/G/Ctrl-D/U/line-number
matrix and other-platform human acceptance; this is why G07 remains PARTIAL.
Real snapshot success retains its authorized-device evidence, not a new fleet run.

## Version-edit checklist

Reserved for the later, explicitly approved version commit:

| Surface | Required edit/check |
| --- | --- |
| Cargo.toml; crates/oxvif-cli/Cargo.toml; Cargo.lock | Set workspace library/CLI and CLI library dependency to 0.17.0 together; verify metadata and packaged dependency resolution |
| CHANGELOG.md; docs/releases/0.17.0{,_zh}.md and full changelog pair | Replace draft status/date only when approved; update index and final-tag links, keep short notes separate from the full migration record |
| README pair; CLI package README | Replace next-release warning with released CLI-first summary and absolute blob/v0.17.0 links; keep both OnvifSession and OnvifClient quick starts |
| LIBRARY_GUIDE pair; src/lib.rs | Update installation examples and public migration prose; retain true historical comparison references |
| docs/oxvif-cli pair; docs/cli-maintenance pair; docs/support pair; packaging/oxvif.1 | Synchronize supported version, command coverage, diagnostics-only limits, navigation and actual accepted installation channels |
| CLI guide/describe/schema | Verify CLI version=0.17.0, Agent guide version and schema version independently; do not bump schema merely because package version changes |
| Package verification | Verify both packages against the actual publication dependency chain; a CLI --list or local workspace build is insufficient. Library must be available before publishing its dependent CLI |
| Final frozen candidate | Repeat affected local/native/staging/version/link gates and preserve hashes; any merge conflict resolution requires revalidation |

Do not mechanically replace every 0.16 reference: published history, migration
comparisons and fixed test fixtures can intentionally reference the old version.

## Approval boundary

Before the formal version commit, present completed G01/G03 reviews, exact local
and hosted results, staging evidence, human/real-camera acceptance, resolved A03,
the proposed version/link diff and explicit limitations. If any required gate
is missing, report it as missing and do not describe the four steps as complete.

Repository inspection on 2026-09-12 confirms
[PR #14](https://github.com/smiti1642/oxvif/pull/14),
[PR #16](https://github.com/smiti1642/oxvif/pull/16) and
[PR #17](https://github.com/smiti1642/oxvif/pull/17) are closed, with GitHub's
`merged=false`. Their credited adaptations (`07a7d61`, `f9448e5`, `563bbcf`,
respectively) are ancestors of remote master/develop `5a1821b`; this is adapted
integration, not a GitHub merge of each original PR. Do not integrate them again.
Later CLI/Fleet work is on `feat/basic-mock-fleet` and has not been merged into
those main branches. This documentation update performs no merge or publication.
