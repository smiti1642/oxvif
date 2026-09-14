# 0.17 release approval packet

[English](release-0.17-approval.md) | [繁體中文](release-0.17-approval_zh.md)

Updated 2026-09-14. The user authorized 0.17 version/document preparation, not
public publication. The candidate includes the merged CLI and basic Mock Fleet;
master/develop were both `d3ac1b6` before preparation.
Current branch: `codex/release-0.17-finalize`.
Historical evidence retains its original revision. Current gates are in the
[cut](release-0.17-cut.md) and [finalization record](release-0.17-finalization.md).

| Section | Purpose |
| --- | --- |
| [Four requested steps](#four-requested-steps) | Current status |
| [Findings](#findings) | Historical repairs and limits |
| [Executed checks](#executed-checks) | Revision-specific evidence |
| [Review closure](#review-closure) | Current delta review |
| [Maintainer actions](#maintainer-actions) | CI/staging |
| [Version-edit checklist](#version-edit-checklist) | Prepared changes |
| [Approval boundary](#approval-boundary) | Stop before publication |

## Four requested steps

| Step | Current status |
| --- | --- |
| 1. Candidate/security review | Baseline and bounded CLI/Fleet delta LOCAL-PASS; not ONVIF certification |
| 2. Packages, installation and human acceptance | Both 0.17 packages locally verified; operator-reported manual PASS; final CI/staging to be recorded |
| 3. Versions, links and documents | Both packages 0.17.0; existing public English/Chinese guides synchronized; date and Unreleased guards retained |
| 4. User confirmation | Preparation authorized; public publication requires separate confirmation |

The following findings/checks are historical, revision-specific records. Earlier
pending dispositions do not override the current table.

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

The original 208-path ledger retains its blob evidence. The
`5a1821b..d3ac1b6` CLI/Fleet delta is separately recorded in the
[finalization review](release-0.17-finalization.md#delta-review); passing tests do
not silently extend the ledger. Subset, snapshot-format, recording-privacy and
listener limits remain in the backlog.

## Maintainer actions

Use the authorized `smiti1642` account. This runs CI and temporary Actions
artifacts only: no tag, crates.io upload, GitHub Release or user-system install.

```powershell
$candidateBranch = 'codex/release-0.17-finalize'
git fetch origin $candidateBranch
if ($LASTEXITCODE -ne 0) { throw 'Fetch failed' }
$candidate = git rev-parse "origin/$candidateBranch"
if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve candidate' }
gh workflow run ci.yml --ref $candidateBranch
if ($LASTEXITCODE -ne 0) { throw 'CI dispatch failed' }
gh workflow run release.yml --ref $candidateBranch -f tag=$candidate -F publish=false -F prerelease=true
if ($LASTEXITCODE -ne 0) { throw 'Staging dispatch failed' }
```

Record exact SHAs and URLs. Require all five native packages, credentials,
checksums, both SBOM inventories, APT install/remove and Homebrew
install/bottle/reinstall. This does not establish official-channel admission.

## Version-edit checklist

Workspace/CLI dependency/lockfile are 0.17.0. READMEs, guides, man page, short
release notes and full changelog are synchronized. Agent schema 3/guide 8 are
independent of package version. Local workspace packaging uses Cargo's temporary
registry and compiles the packaged CLI against the packaged library; actual
publication must make the library available on crates.io before the CLI.

## Approval boundary

After CI/staging pass, present exact evidence and limitations, then stop for
explicit publication approval. Only afterwards finalize date/status and perform
cargo publish, tagging and GitHub Release.
PRs #14/#16/#17 were integrated through credited adaptations, not GitHub merges
of the original PRs; do not integrate them again. Earlier CLI/Fleet branches
have been merged and cleaned up; their old commands are no longer current.
