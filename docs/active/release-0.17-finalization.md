# 0.17 finalization record

[English](release-0.17-finalization.md) | [繁體中文](release-0.17-finalization_zh.md)

Updated 2026-09-14. Version preparation is authorized; public publication is not.
Prepared branch: `codex/release-0.17-finalize`, based on `d3ac1b6`.

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
| Both packaged crates | `cargo package --workspace --allow-dirty --locked --target-dir target/package-017-final` passed; CLI compiled against packaged oxvif through Cargo's temporary local registry, not a crates.io upload |
| Initial package attempt | Could not overwrite the user's running debug CLI; isolated target-directory retry passed. The running user process was not stopped |
| Packaged CLI | Version 0.17.0, schema 3, guide 8; structured describe and manage/diagnose/snapshot/config export/diff help passed |
| Documentation | 620 public local/version-tag targets and 334 anchors passed before final evidence edits; published CHANGELOG history from 0.16.0 onward unchanged |
| Human acceptance | Operator-reported manual PASS plus previously recorded Windows discover/manage/resize ConPTY evidence; no inferred additional platform/device/VMS matrix |
| Final 0.17 hosted CI and staging | Pending dispatch on the committed preparation SHA; record exact run URLs and outcomes before merge/publication approval |

Previous full-workspace test totals, native installs, hardware samples and
ConPTY results retain their input revisions; they are not newly executed by a
version-only build. Final native CI/staging must cover the prepared version.
Staging install checks do not mean admission to official winget/Chocolatey,
Homebrew core or Debian/Ubuntu repositories.

## Publication boundary

Keep `Status: Unreleased` and the undated CHANGELOG release guard until approval.
The prepared v0.17.0 links are checked against local paths/anchors; the tag does
not exist yet. After required CI/staging pass, present evidence and request
confirmation before finalizing dates, cargo publish, tag or GitHub Release.
Publish the library before its dependent CLI. Do not install into the user's
system or change credentials as part of staging.
