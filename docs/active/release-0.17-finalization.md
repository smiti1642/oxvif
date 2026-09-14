# 0.17 finalization record

[English](release-0.17-finalization.md) | [繁體中文](release-0.17-finalization_zh.md)

Updated 2026-09-14. Public v0.17.0 tagging and GitHub Release are authorized.
Prepared branch: `codex/release-0.17-finalize`, based on `d3ac1b6`.
Candidate: `9305f5d8c080feca6adf555d4177f10556f45db3`, committed and pushed as
`smiti1642`. Subsequent evidence-only documentation edits do not change its runtime/tooling tree.
Preparation and evidence commits were fast-forwarded into local/remote master
and develop. The temporary preparation branch was deleted; its commits remain
reachable from both main branches. No release tag or public upload was created.

| Section | Purpose |
| --- | --- |
| [Delta review](#delta-review) | Bounded G01/G03 reconciliation |
| [Verification](#verification) | Actual results and pending gates |
| [Publication boundary](#publication-boundary) | Actions requiring confirmation |

## Delta review

This supplements, not rewrites, the original 208-path review ledger.
Reviewed runtime delta: `5a1821b..d3ac1b6`. This preparation changes versions,
documentation and the man page, not runtime behavior.

| Source | Review focus and disposition |
| --- | --- |
| CLI application/main/manage/maintenance | Shared discovery classification; exact filtered selection; explicit add without changing current device; cached rescan only replaced on success; per-device workspace/session/profile isolation; retained success/failure evidence; local path/baseline preflight. No new blocking finding identified |
| CLI interactive | Literal search versus navigation; contextual settings/help; responsive viewport and bottom status; UTF-8 input; password ownership/clearing; terminal control/bidi escaping before width/wrapping. No new blocking finding identified; no universal terminal/IME claim |
| Mock fleet/server | Per-member identity/state; startup rollback and shutdown; bounded joins; explicit assigned advertised address; non-loopback exposure. Mock HTTP is unauthenticated and must remain on an isolated trusted test network |
| Shared discovery responder | Bounded datagram/structure, namespace-aware Probe/Types, anonymous ReplyTo, unsupported scope rejection, bounded duplicate cache/device count and response delay; ready HTTP members only. Restricted discovery subset is documented, not general discovery conformance |
| Fleet example/config | Strict bounded TOML, unique IDs/UUIDs/ports/serials, explicit state-file failure, create-new manifest, no writeback, Ctrl+C cleanup and LAN warning. No RTSP, per-brand emulation or long-duration VMS stability claim |
| Terminal fixture / CI controls | Test-only fixture and isolated harness; d3ac1b6 repairs portable Python imports and exact reader inventories. Hosted controls retain failure propagation; no CI assertion or publication guard weakened |

Relevant evidence remains in [CLI continuity](cli-workflow-continuity.md),
[Fleet acceptance](mock-fleet-basic-plan.md#local-evidence), the
[baseline review](release-0.17-review.md) and [follow-up backlog](post-0.17-backlog.md).
This is bounded engineering review, not independent security or ONVIF certification.

## Verification

| Check | Result |
| --- | --- |
| Prior exact runtime/CI baseline | [34809397553](https://github.com/smiti1642/oxvif/actions/runs/34809397553), d3ac1b6: all 27 jobs passed across five native targets |
| Local 0.17 workspace | `cargo check --workspace --all-features --offline` passed |
| Local strict rustdoc | Workspace/all-features/no-deps/locked with `RUSTDOCFLAGS=-D warnings` passed |
| Both packaged crates | `cargo package --workspace --locked --target-dir target/package-017-final` passed again on clean commit 9305f5d without allow-dirty; CLI compiled against packaged oxvif through Cargo's temporary local registry, not a crates.io upload |
| Initial package attempt | Could not overwrite the user's running debug CLI; isolated target-directory retry passed. The running user process was not stopped |
| Packaged CLI | Version 0.17.0, schema 3, guide 8; structured describe and manage/diagnose/snapshot/config export/diff help passed |
| Documentation | 706 public/current-record local/version-tag targets and 382 anchors passed; separate all-repository inbound check: 604 targets/343 anchors passed. Published CHANGELOG history from 0.16.0 onward unchanged; fmt and diff whitespace passed |
| Human acceptance | Operator-reported manual PASS plus previously recorded Windows discover/manage/resize ConPTY evidence; no inferred additional platform/device/VMS matrix |
| Final 0.17 hosted CI | [34813217979](https://github.com/smiti1642/oxvif/actions/runs/34813217979), exact candidate above; all 27 jobs passed |
| Final 0.17 staging | [34813220307](https://github.com/smiti1642/oxvif/actions/runs/34813220307), same workflow/source SHA, publish=false; all 17 verification jobs passed, public GitHub Release job correctly skipped |
| Downloaded artifacts | Five archives and two Debian packages: all seven SHA-256 checksums passed; no user-system installation |
| SBOM comparison | Compared all five targets with [prior staging 34811578831](https://github.com/smiti1642/oxvif/actions/runs/34811578831). Both inventories use syft 1.51.1. Each source SBOM has 411 entries covering all 410 locked package/version pairs; only own package versions changed to 0.17.0. Binary inventory names/versions/counts are unchanged: 3 on Windows, 1 on each other target |

Previous full-workspace test totals, native installs, hardware samples and
ConPTY results retain their input revisions; they are not newly executed by a
version-only build. Final native CI/staging must cover the prepared version.
Staging install checks do not mean admission to official winget/Chocolatey,
Homebrew core or Debian/Ubuntu repositories.
The sparse binary inventories are not complete Rust dependency inventories;
source inventories include development and other-platform dependencies, not
precise per-binary linkage. Scanner parity does not remove these limitations.

## Publication boundary

The maintainer published oxvif 0.17.0 at 2026-09-14 08:04:23 UTC and oxvif-cli
0.17.0 at 08:11:04 UTC; both registry entries were independently confirmed and
are not yanked. No registry upload is performed by this task.
The user explicitly authorized v0.17.0 tagging and stable GitHub Release.
Release dates and publication guards are finalized; the existing workflow will
publish with publish=true and prerelease=false after its gates pass.
Its notes input is the English-only categorized summary, not the Chinese file
or full technical changelog. No host-system installation is authorized.
GitHub publication outcome: pending workflow completion.
