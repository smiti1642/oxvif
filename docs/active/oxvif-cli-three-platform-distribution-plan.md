# oxvif CLI three-platform distribution plan

Status: Active  
Decision date: 2026-09-01  
Applies to: first public `oxvif-cli` release and official-channel graduation

## 1. Outcome

The first public CLI release must be securely usable and installable on Windows,
Linux, and macOS. A source build, raw archive, or process-environment credentials
alone do not pass this gate.

The controlled first release uses project-owned channels:

- Windows x86_64 artifact with native Credential Manager integration;
- a signed APT repository for Linux x86_64 and aarch64, with Secret Service;
- a Homebrew tap and Intel/Apple Silicon bottles, with Keychain integration.

After at least two consecutive releases prove install, upgrade, downgrade, and
uninstall behavior, oxvif applies to Homebrew Core and Debian. Ubuntu should
consume the Debian package through synchronization where practical.

No crates.io publication, git tag, GitHub Release, APT publication, Homebrew tap
publication, or downstream completion notice may occur without reminding the
user immediately before the external release action and receiving approval.

## 2. Locked decisions

1. Native secret storage is mandatory on all three platforms. There is no
   plaintext credential fallback.
2. Environment and stdin remain supported for ephemeral/headless automation,
   but are not persistent storage.
3. The first channels are project-owned and reversible. Official distribution
   is a committed follow-up, not a dependency on an external review queue.
4. Linux targets x86_64/aarch64, Homebrew targets Intel/Apple Silicon, and the
   initial Windows target is x86_64.
5. `--timeout` remains a per-attempt limit. A future total deadline is separate.
6. Private-CA support retains chain and hostname verification. The first release
   has no `--insecure` or hostname-verification bypass.
7. Machine stdout remains schema-versioned and contains no logs, prompts,
   package-manager progress, or secret-store diagnostics.

## 3. Authority boundary

Local code, tests, documentation, package recipes, metadata templates, and
dry-run artifacts may be created without further approval.

These external mutations require the explicit pre-release reminder and approval:

- publishing either Rust crate;
- creating/pushing a release tag or public GitHub Release;
- creating/publishing the APT repository or signing key;
- creating/publishing the Homebrew tap, formula, or bottles;
- submitting to Homebrew Core, Debian, or Ubuntu;
- sending the release-complete handoff to `ONVIF Refector Agent`.

## 4. Supported release matrix

| Platform | Architecture | Persistent secret backend | First channel |
|---|---|---|---|
| Windows | x86_64 | Windows Credential Manager | GitHub Release artifact |
| Linux | x86_64 | Secret Service over D-Bus | Signed project APT repository |
| Linux | aarch64 | Secret Service over D-Bus | Signed project APT repository |
| macOS | x86_64 | Keychain | Project Homebrew tap/bottle |
| macOS | arm64 | Keychain | Project Homebrew tap/bottle |

Unsupported backends return stable `CREDENTIAL_UNAVAILABLE` errors. A headless
Linux session without D-Bus/Secret Service explains process-only credentials
without writing a password to disk.

## 5. Workstream A — reliability prerequisites

### A1. Timeout cancellation

- [x] Add a delayed HTTP fixture that observes connection closure after timeout.
- [x] Prove active request/task counters return to zero.
- [x] Prove fleet concurrency permits are released.
- [x] Prove retry sleeps stop with their parent command.
- [x] Prove discovery tasks stop with their parent command at the CLI wrapper.
- [x] Keep help, descriptors, Agent guide, and manuals explicit that timeout is
  per attempt.

### A2. Shared TLS transport construction

- [x] Introduce one fallible transport factory used by diagnostics, health,
  setup/refresh, discovery enrichment, and fleet items.
- [x] Add repeatable `--ca-certificate <FILE>` support for PEM bundles.
- [x] Reject missing/unreadable files, malformed/empty bundles, and private-key
  material before connecting.
- [x] Preserve certificate-chain and hostname validation.
- [x] Prove every device path uses the same custom roots.
- [x] Prove output does not expose certificate contents or unnecessary paths.

### A3. Command contract consolidation

- [x] Add exhaustive `CommandId` and shared `CommandSpec` definitions.
- [x] Derive name, result kind, risk, retryability, reachable errors, and
  examples from the catalogue.
- [x] Keep Clap responsible for syntax while tests map every parser path to one
  `CommandId`.
- [x] Fail CI when parser, descriptor, Agent guide, schema, or help drifts.

Exit gate: timeout cancellation and TLS propagation have executable evidence;
the command catalogue covers every public command.

Implementation record (2026-09-01): all 61 public paths have one `CommandId`;
human aliases map explicitly to canonical typed operations, `CommandRequest`
derives its stable name from that identity, and `CommandSpec` pairs each ID with
its risk, retryability, output, errors, arguments, and examples. Tests reject
duplicate/order drift and parse every catalogue example through the same Clap
surface that generates help. Existing schema tests validate descriptor output,
and the embedded Agent guide test locks its schema version.

## 6. Workstream B — native credential backends

### B1. Backend-neutral contract

- [x] Define shared set/get/delete/no-entry/unavailable/error semantics.
- [x] Namespace secrets without embedding passwords in identifiers.
- [x] Define replacement, deletion, rollback, and concurrent-access behavior.
- [x] Never persist a registry reference when native secret persistence failed.
- [x] Zeroize temporary password buffers where practical and document remaining
  process-memory exposure.

### B2. Windows Credential Manager

- [x] Preserve the current Windows-native implementation.
- [x] Run isolated set/get/replace/delete/no-entry tests on Windows CI.
- [x] Prove test cleanup removes generated credential entries.
- [x] Prove errors contain no account names or secret data.

### B3. macOS Keychain

- [x] Select and audit a backend compatible with Rust 1.88.
- [x] Implement the same contract/error mapping as Windows.
- [x] Run isolated contract tests on Intel and Apple Silicon where runners
  permit; never silently skip the only backend test.
- [x] Document session prompts and non-interactive failure behavior.

### B4. Linux Secret Service

- [x] Select and audit a backend compatible with Rust 1.88.
- [x] Test inside an isolated D-Bus session with an ephemeral collection.
- [x] Test unavailable D-Bus, missing entry, replacement, deletion, and cleanup.
- [ ] Add integration coverage for a locked collection and explicit access denial.
- [x] Fail with `CREDENTIAL_UNAVAILABLE` in headless environments; never create
  a plaintext substitute.
- [x] Document environment/stdin workflows for containers and automation.

Implementation record (2026-09-01): target-specific `keyring 3.6.3` features
select Credential Manager, Keychain, and synchronous Secret Service adapters;
the crate declares Rust 1.75, below oxvif's Rust 1.88 MSRV. The shared ignored
contract covers no-entry, set, get, replace, delete, idempotent delete, and
cleanup. It passed against Windows Credential Manager locally. Dedicated native
CI jobs run that contract on Windows, macOS, and an isolated GNOME Keyring
session on Linux; Linux separately checks the no-D-Bus error path. CI runs
[`33496836237`](https://github.com/smiti1642/oxvif/actions/runs/33496836237)
and
[`33499987562`](https://github.com/smiti1642/oxvif/actions/runs/33499987562)
passed all five native platform/architecture rows.

Four Windows-hosted cross-target checks were attempted, but Linux checks lacked
the GNU cross C compilers required by vendored native dependencies and macOS
checks lacked the Apple compiler/SDK. They are recorded as environment-blocked,
not as target compilation evidence. The lockfile currently contains 399
packages; `event-listener` was pinned to patched 5.4.2 after the audit identified
the 5.4.1 thread-safety advisory, and the resulting `cargo audit` is clean.

Exit gate: the same black-box contract passes on all three OSes, and runtime
tests prove secrets are absent from registries, logs, diagnostics, temporary
files, and packages.

## 7. Workstream C — reproducible artifacts

### C1. Common contents

- [x] Pin the release Rust toolchain and every GitHub Action to immutable
  revisions, with Dependabot tracking action updates.
- [ ] Pin or digest-lock OS packaging tools where practical; record any tool
  that must remain distribution-managed in artifact metadata.
- [x] Build the verified tag with `Cargo.lock` and `--locked`.
- [x] Include executable, license, README, schemas, completions, and man page in
  platform-appropriate locations.
- [x] Generate SHA-256 checksums, SPDX SBOM, metadata, and provenance.
- [x] Make archives and Debian payload timestamps deterministic from the tag
  commit where tooling permits.
- [x] Smoke-test artifact binaries and installed binaries.

Implementation record (2026-09-01): the release workflow is defined for all
five supported rows, uses Rust 1.88.0 with `Cargo.lock`, runs native credential
contracts before builds, and stages versioned archives, Debian packages,
checksums, SPDX SBOMs, GitHub provenance attestations, completion scripts,
schemas, metadata, and a manual page. Debian packages are linted, installed,
smoked, and removed on native amd64/arm64 Ubuntu 22.04 runners. A signed APT
repository is assembled with an ephemeral CI-only key and tested through
`apt update`, install, smoke, and purge on both architectures. A Homebrew
formula is rendered from verified archive hashes, audited, installed, bottled,
and reinstalled on macOS 15 Intel/Apple Silicon runners. Formula tests verify
the CLI, schemas, and Bash/Zsh/Fish completions both before and after the bottle
reinstall. The Windows archive is built with a static C runtime and its PE
imports are checked. The final non-publishing evidence is commit `f9790ec` in
[release staging run 33508813383](https://github.com/smiti1642/oxvif/actions/runs/33508813383);
the matching [general CI run 33508813343](https://github.com/smiti1642/oxvif/actions/runs/33508813343)
passed all 21 jobs. No tag, release, public signing key, package repository,
tap, formula, bottle, or attestation has been published.

### C2. Windows

- [x] Build/test x86_64 on Windows and stage a versioned portable archive.
- [ ] Publish the verified portable archive in the approved GitHub Release.
- [ ] Record the signing path required before a commercial-pilot claim.
- [ ] Keep MSI/WinGet as follow-ups unless promoted by a later decision.

### C3. Debian packages

- [x] Produce `amd64` and `arm64` `.deb` packages with correct metadata.
- [x] Install binary, completions, man page, and schemas in standard locations.
- [x] Avoid maintainer scripts; if required, test every lifecycle transition.
- [x] Validate with current Debian packaging lint tools.
- [x] Install on clean supported Debian/Ubuntu hosts and run CLI smoke tests.

### C4. Homebrew formula and bottles

- [ ] Maintain a formula in the selected project tap.
- [x] Produce staged Intel/Apple Silicon bottles and record hashes.
- [x] Install completions using policy-compliant paths.
- [x] Test version, descriptors, Agent guide, schema, and completions before and
  after bottle reinstall.
- [x] Run current Homebrew audit and style gates against a named staging tap.
- [ ] Run `brew test-bot` in the public tap repository before publication.

Exit gate: every matrix artifact installs cleanly and reports matching CLI,
library, Agent-guide, and schema versions.

## 8. Workstream D — project-owned channels

### D1. Signed APT repository

- [ ] Choose the public host and disaster-recovery owner before creation.
- [ ] Establish signing-key ownership, expiry, rotation, and offline/CI split.
- [ ] Publish signed metadata for supported distributions/architectures.
- [ ] Make publication atomic and retain documented downgrade versions.
- [ ] Verify repository setup, `apt update`, install, upgrade, downgrade, remove,
  and purge on clean hosts.
- [ ] Test signature rejection, expired-key guidance, wrong architecture, and
  repository outage behavior.

### D2. Project Homebrew tap

- [ ] Use proposed `smiti1642/homebrew-tap` unless the user selects another
  owner/repository before external creation.
- [ ] Publish only after release assets and hashes verify.
- [ ] Verify tap/install/upgrade/downgrade-or-pin/uninstall/untap on Intel and
  Apple Silicon.
- [ ] Document rollback for withdrawn formulas or bottles.

### D3. Documentation

- [ ] Avoid piping unaudited network content into a privileged shell.
- [ ] Publish signing fingerprints, architectures, upgrade/removal policy, and
  security contact.
- [ ] Keep Cargo, APT, Homebrew, and direct artifact versions aligned.
- [ ] Advertise install commands only after public verification.

Exit gate: the public README alone is sufficient to install, validate, upgrade,
and remove oxvif on every declared platform without building Rust.

## 9. Workstream E — release cut line

### E1. Clean verification

- [ ] Run format, Clippy, full tests/features, rustdoc, MSRV, audit, schema,
  package, and mock/human smoke gates from a clean worktree.
- [x] Verify all five architecture rows and credential backends.
- [x] Verify private CA and timeout cancellation fixtures.
- [ ] Verify clean installs from crates.io, APT, Homebrew, and Windows artifact.
- [ ] Re-run the sanitized real-camera matrix from the parent plan.

### E2. Approval and publication order

When all evidence is green, stop and remind the user. After approval:

1. publish `oxvif` and verify index visibility;
2. publish `oxvif-cli` and verify clean `cargo install --locked`;
3. create the verified tag and GitHub Release;
4. publish signed APT metadata/packages;
5. publish Homebrew tap formula/bottles;
6. verify every public install, link, hash, and version;
7. update documentation status/date;
8. notify `ONVIF Refector Agent`.

On failure, stop, preserve evidence, and apply the documented rollback. Never
send the downstream completion notice for a partially published release.

## 10. Workstream F — official-channel graduation

### F1. Evidence threshold

- [ ] Complete at least two consecutive project-channel releases with recorded
  lifecycle evidence.
- [ ] Resolve packaging/security issues and document maintenance ownership.
- [ ] Re-check official policies at submission time because they can change.

### F2. Homebrew Core

- [ ] Confirm current naming, source, build, test, license, and notability rules.
- [ ] Submit the formula and track review findings and acceptance.
- [ ] Keep the project tap working until Core installation verifies.

### F3. Debian and Ubuntu

- [ ] Produce a Debian Policy-compliant reproducible source package with
  copyright, watch/upstream metadata, man page, tests, and clean lint results.
- [ ] File the appropriate Debian packaging request and obtain review/sponsorship.
- [ ] Verify the package from the official Debian archive.
- [ ] Prefer Ubuntu synchronization; use a separate Ubuntu path only when
  supported-release timing requires it.
- [ ] Ensure the project APT package can migrate safely to the official package.

Official acceptance depends on external maintainers. Completion requires
compliant submissions, review responses, and verification of accepted official
artifacts—not an invented external review date.

## 11. Standing verification record

Each completed workstream records commit/PR, tool versions, commands/results by
OS and architecture, hashes/signing identity, credential negative-path evidence,
limitations, rollback results, and only verified external URLs.

On 2026-09-01, CI run
[`33496836237`](https://github.com/smiti1642/oxvif/actions/runs/33496836237)
passed the Windows x64, Ubuntu x86_64/aarch64, and macOS Intel/Apple Silicon
test and release-binary smoke matrices. The same run passed Credential Manager,
Keychain, and isolated Secret Service lifecycle contracts; both Linux rows also
passed the no-D-Bus `CREDENTIAL_UNAVAILABLE` negative path. This is CI evidence,
not evidence of a published package channel or release artifact.

This plan moves to `docs/done/` only after the first three-platform release is
fully published, verified, documented, and handed to downstream validation.
Official-channel graduation remains in an active follow-up until Homebrew Core
and Debian artifacts are accepted and verified.
