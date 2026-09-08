# CLI maintenance workflows plan

[English](cli-maintenance-workflows-plan.md) | [繁體中文](cli-maintenance-workflows-plan_zh.md)

Status: local implementation complete; real-camera and native cross-platform
release acceptance pending. Based on 0.16.0; these features remain unreleased.

| Section | Purpose |
| --- | --- |
| [Scope](#scope) | Workflow boundaries |
| [Implementation](#implementation) | Ordered deliverables |
| [Acceptance](#acceptance) | Required verification |
| [Evidence and remaining gates](#evidence-and-remaining-gates) | Results and handoff |

## Scope

Deliver snapshot downloads, layered diagnostics, and read-only camera configuration
inventory/diff through the shared application layer. Preserve existing URI-only
commands and local `config path` / `config validate`. Human and non-interactive
callers receive the same capabilities; JSON never contains binary image data.

No device writes, automatic discovery scans, real-camera operations without an
explicit test target, package publication, Release changes, or installed-binary
replacement. Camera imagery and inventory are sensitive local artifacts.

## Implementation

1. [x] Snapshot: add `snapshot --save` and canonical `media snapshot-save`, reuse
   profile selection, support explicit targets, reject existing destination files,
   and publish completed files atomically. Bound download size/time; honor private
   CA roots; support challenged Basic/Digest authentication. Reject redirects,
   embedded credentials, foreign-host snapshot URLs and HTTPS downgrade before
   sending credentials. Do not log URLs with secret query parameters or bodies.
2. [x] Diagnose: show individually bounded ONVIF/session, identity, profiles,
   stream-URI and snapshot-fetch stages, with pass/fail/unsupported/not-tested
   status, elapsed time, stable stage names and next-step guidance. Preserve partial
   evidence, return nonzero on failed checks, and explicitly mark RTSP transport
   and video decoding as not tested. External decoder integration is deferred:
   this iteration must not claim playback verification.
3. [x] Configuration: `config export` / `config diff` collect hostname, NTP, DNS,
   network interfaces/protocols/gateway, Media1 profiles and video encoders. Use
   versioned inventory, per-section errors, stable ordering and JSON-pointer diffs.
   Do not collect passwords, live URIs, logs, or a ticking clock. Validate baseline
   format/identity; incomplete sections must never compare as equal. Export is not
   a restorable backup. Existing baseline/output files are never overwritten.
4. [x] Contracts and UX: command descriptors/help/examples, structured envelope,
   meaningful table rendering, exit semantics, Agent guide and schema tests.
   Diagnose supports existing Group/View execution. File-producing workflows are
   initially single-device (explicitly reject fleet selectors to prevent collisions).
5. [x] Documentation: update English/Traditional Chinese CLI and maintenance guides,
   the English CLI crate README,
   concise root README references, Changelog Unreleased, documentation indexes and
   this plan's evidence. Keep reciprocal language links and table navigation.

## Acceptance

- Tests with local mock cameras/HTTP servers: successful image, authentication,
  malformed/non-image body, oversized/chunked body, stalled request, redirect,
  foreign host, downgrade, existing destination and cancellation cleanup.
- Diagnostics retain earlier results when later stages fail; no implicit profile
  selection when multiple profiles exist; fleet and JSON failure semantics agree.
- Inventory tests: deterministic reorder, real field changes, missing/unsupported
  sections, mismatched identity/schema, bounded baseline read, and secret exclusion.
- Parser/descriptor/schema parity and human/Agent command tests.
- `cargo fmt --all --check`; workspace clippy (default/all features); workspace
  tests (default/all features). Document real-device and non-Windows verification
  as pending unless actually executed; do not count mocks as hardware acceptance.

## Evidence and remaining gates

Local Windows x64 validation on 2026-09-08:

The following records the initial implementation. Subsequent UX changes and
updated verification counts are tracked in the [UX refinement plan](cli-maintenance-ux-plan.md).

- `cargo test --workspace --all-features`: 1,095 passed, 4 existing conditional
  ignores. Includes 19 new workflow tests across library and executable coverage.
- `cargo test --workspace`: 1,015 passed, 4 existing conditional ignores.
- Workspace default/all-feature clippy with `-D warnings`: passed.
- Rust 1.88 workspace/all-target/all-feature check: passed.
- CLI rustdoc with warnings denied: passed.
- `cargo audit`: 411 dependency packages scanned, no known vulnerabilities reported.
- CLI package file list includes implementation and updated schema; local Markdown
  targets checked across eight guides. Package-list inspection is not publication
  or an installation acceptance test.
- Positive/negative HTTP, challenged Basic/Digest, private CA and wrong-host TLS,
  chunked size limit, cancellation, no-clobber, stage retries, mock camera download,
  export/diff, incomplete sections, identity mismatch, profile ambiguity, fleet
  partial/total failure and executable/schema tests are covered.

The diagnostic network/TLS check is part of session establishment, not separate
DNS/TCP/TLS probes. HTTP downloads enforce a single total timeout and do not retry;
SOAP stages use bounded per-attempt retry. Snapshots use signature checks, not full
image decoding. DNS/NTP preference order is preserved; keyed records are sorted.
No hardware was contacted, no Release/tag/installed binary was changed, and no
camera settings were written.

Remaining release gates:

- [ ] Authorized real-camera acceptance from the bilingual maintenance guide.
- [ ] Native macOS/Linux and Windows release-candidate CI/installation validation.
- [ ] Review version selection and Release approval with the operator before publishing.

RTSP transport/decoder integration, batch file exports, events and device mutation
are outside this implementation and remain future work.
