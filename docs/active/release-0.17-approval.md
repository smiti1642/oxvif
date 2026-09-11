# 0.17 pre-version review packet

[English](release-0.17-approval.md) | [繁體中文](release-0.17-approval_zh.md)

Status: IN-PROGRESS / NOT APPROVED. Updated 2026-09-11.
This packet supplements the [release cut](release-0.17-cut.md), not replaces its
blocking gates. The user requested a stop before the formal 0.17 version/release
commit. Ordinary repairs and evidence commits are allowed; version promotion,
main-branch merges, tags, publication, PR closure and host-system installation
are not performed by this acceptance work.

| Section | Purpose |
| --- | --- |
| [Four requested steps](#four-requested-steps) | Current status without conflating evidence |
| [Findings](#findings) | Repairs and unresolved results |
| [Executed checks](#executed-checks) | Local, hosted and real-camera observations |
| [Review closure](#review-closure) | Remaining source-review work |
| [Maintainer actions](#maintainer-actions) | Non-publishing CI and installation staging |
| [Version-edit checklist](#version-edit-checklist) | Changes reserved for the approved version commit |
| [Approval boundary](#approval-boundary) | Conditions before asking for promotion |

## Four requested steps

| Step | Status | Remaining work |
| --- | --- | --- |
| 1. Complete candidate/security review | IN-PROGRESS | A01 and A02 repaired locally; complete the all-diff closure below, including transitive consumers |
| 2. Package/install and human/real-camera acceptance | PARTIAL | Native CI passed at 3eccfd1; later repairs need CI. Real export/diff passed; snapshot HTTP 401 remains unresolved. Final terminal and distribution staging remain open |
| 3. Versions, links and documents | PREPARED, not promoted | Bilingual release records and this checklist updated. Actual versions remain 0.16.0; apply the version-edit checklist only after approval |
| 4. User confirmation | NOT REQUESTED for promotion | Present the final evidence and unresolved risks before the formal version commit; publication requires separate authorization |

## Findings

| ID | Observation | Disposition |
| --- | --- | --- |
| A01 | HTTP lossy UTF-8 decoding converted byte FF into U+FFFD and committed a differently named profile | Repaired in 6135e72: HTTP 400 / Sender / mock:RequestPolicy before responders. Five invalid-byte classes, valid Unicode, full-state/hook preservation and queued-fault control pass locally |
| A02 | Snapshot Authorization rewrote `qop=auth` as a quoted value | Remove the rewrite; assert unquoted qop/algorithm/nc and exact request URI. This follows [RFC 7616 §3.4](https://www.rfc-editor.org/rfc/rfc7616.html#section-3.4), not camera-specific downgrade behavior |
| A03 | Both real-camera profiles return HTTP 401 at snapshot_fetch, before and after A02 | OPEN: do not attribute the entire failure to A02. Confirm camera HTTP/snapshot credentials and permissions, then reproduce with a trusted independent client if needed. No credential guessing or Basic/TLS/redirect fallback |

A01 does not complete the general HTTP binding/charset/fault-status audit. A02
does not prove the camera accepts the saved credentials. Neither permits an
unverified success claim to be deferred into the [post-release backlog](post-0.17-backlog.md).

## Executed checks

| Evidence | Result and boundary |
| --- | --- |
| Hosted CI at 3eccfd15274d4e978501634e4155477760502b6b | [Run 34597167495](https://github.com/smiti1642/oxvif/actions/runs/34597167495): all 27 jobs passed, five native targets; does not cover later repairs |
| A01 sensitivity | Full workspace all-features, no-fail-fast: 1,301 pass / two assertion failures / five ignored; actual unwanted state mutation observed |
| A01 restored gates | 1,303 all-feature / 1,193 default passes, five ignored each, 41 suites; both Clippy and strict rustdoc modes, fmt, local links and published-history check pass |
| A02 sensitivity | Full workspace all-features, no-fail-fast: 1,302 pass / one assertion failure / five ignored; outgoing quoted qop fails the wire-format assertion |
| A02 restored gates | 1,303 all-feature / 1,193 default passes, five ignored each, 41 suites; both workspace Clippy modes pass. Strict rustdoc results from A01 apply to unchanged public documentation; current format/link checks accompany the commit |
| Fresh dependency checks | cargo audit: 410 locked packages, no reported vulnerabilities. cargo outdated reports reqwest 0.13.5, tokio-rustls 0.26.5, toml 1.1.6 and dev-only dirs 7 available; keyring 4 migration emits obsolete-feature warnings. No lockfile change; these updates need a separate reviewed batch, not an automatic pre-release upgrade |
| Real camera, Windows x64 | GeoVision_2 GV-TBL8810, firmware V111_2025_12_09; info and two profile queries exit 0 |
| Real diagnosis, both profiles | exit 20, complete=false: five stages pass, snapshot_fetch returns 401, RTSP transport and decode remain not_tested. Error reports preserve partial evidence; no playback claim |
| Human table / Agent version | Plain table also exits 20 and names the 401/partial result/playback limit; not an interactive terminal test. Executed guide reports CLI 0.16.0, schema 3 and guide 8 |
| Real export/diff | Eight sections pass; export exit 0/complete=true; second export to the same path exits 4/RESOURCE_ALREADY_EXISTS and file hash is unchanged; live diff exits 0/complete=true/matches=true, zero changes |
| Privacy boundary | Only read-only camera calls. Baseline inventory remains in a private temporary directory outside Git; no IP, serial, credentials, URI, digest challenge or image is recorded here |
| Final documentation/inventory | fmt and diff whitespace pass; 123 local Markdown targets checked, published CHANGELOG history unchanged. Inventory self-tests pass: 159 routes, 161 Action sites, 191 readers; no normative/transitive-graph acceptance implied |

This is one device/firmware, not brand-wide interoperability. No real image was
saved successfully; no setting was changed to manufacture a diff. The public
evidence intentionally omits raw camera responses and private artifacts.

## Review closure

The exact review baseline is `git diff v0.16.0 3eccfd1`: 200 changed files,
39,614 added and 3,007 removed lines. Later repair/evidence commits are an
additional delta. A file inventory or green suite is not an all-diff review.

| Group | Current evidence | Required final closure |
| --- | --- | --- |
| CLI application/maintenance/manage, navigation/preferences, Agent/exit contracts | Production entry points inspected; existing regression and live checks above | Finish main/interactive/output/registry/schema/descriptor diff review together; terminal lifecycle, control-character handling, explicit selectors and local-file side effects |
| Library Media1/2, session, notification peer, XML/type migrations | Contributor and operation cards; additive peer wrapper and migration entry points inspected | Reconcile all public methods/types/reexports/examples and compatibility docs against the final diff; do not infer complete XML validation from scoped fixes |
| Mock request/auth/fault/dispatch and service handlers | Existing operation ledger and paired batch evidence; A01 independently reproduced | Read/write/options/capabilities/replay closure for every included batch; inherited unmodified handlers affected by shared helpers; complete disposition of remaining HTTP/fault semantics |
| Metamorph storage/reports/replay/adapters | K27 collision repair and current retention tests | Check all remaining fixture/parse/canonicalization/adapter/replay changes and downgrade documentation; coarse invalidation remains an explicit subset |
| Dependencies, workflows and packaging | Native CI evidence; changed CI/release/SBOM/formula code inspected | Validate source/tooling refs, staging artifacts, checksums, source/binary SBOM distinction and installed commands; finish schema-verifier/tool-source review |
| Documentation and test assertions | Bilingual release/migration records and historical evidence preserved | Reconcile current claims across READMEs/guides/support/manpage; distinguish synthetic controls, independently validated corpus and real hardware. Resolve every changed file into a reviewed group, not just these entry points |

Until those closures are signed off, G01/G03 remain open. Existing per-batch
source reviews and external-corpus results can be reused only for identical
inputs; they must not be represented as a new independent whole-program audit.

## Maintainer actions

The current GitHub CLI identity has read-only repository permission. Do not
change triggers, use another account or open a workaround PR to dispatch jobs.
After ordinary repair commits are pushed, the maintainer can run:

```powershell
git fetch origin codex/contributor-pr-integration
$candidate = git rev-parse origin/codex/contributor-pr-integration
gh workflow run ci.yml --ref codex/contributor-pr-integration
gh workflow run release.yml --ref codex/contributor-pr-integration -f tag=$candidate -F publish=false -F prerelease=true
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

Human terminal acceptance still needs resize, filter, counts/gg/G, Ctrl-D/U,
line-number modes, input/password cancellation and screen restoration through
manage/discover/diagnose. Test snapshot success with an authorized account;
do not share passwords in an issue, commit or conversation.

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

PR #14/#16/#17 are still open. Their adapted/credited implementations are in the
candidate; do not merge the old implementations a second time or close the PRs
as though master/develop already contain this work. Main-branch integration and
PR disposition follow approval, separately from publication authorization.
