# oxvif CLI release-hardening and product-readiness plan

**Status:** active; implementation in progress. Written 2026-09-01 after a
repository, package, runtime UX, security, and public-release audit of `develop`
at `421595b`.

**Scope:** turn the existing diagnostic-first `oxvif-cli 0.1.0` implementation
into an honestly publishable open-source beta, then establish the engineering
gates required for a supported commercial pilot. This plan closes release and
contract gaps before adding device-mutating ONVIF operations.

**Related plans:**

- [CLI operation-surface plan](oxvif-cli-plan.md) — the product model and later
  controlled-write stages;
- [completed human UX plan](../done/oxvif-cli-human-ux-plan.md) — the quick
  command and interactive onboarding design already implemented.

---

## 1. Outcome

The detailed implementation and acceptance sequence for native credentials and
three-platform package distribution is maintained in
[`oxvif-cli-three-platform-distribution-plan.md`](oxvif-cli-three-platform-distribution-plan.md).

This plan has two cut lines.

### 1.1 Open-source beta cut

The beta is ready when a user can install the advertised package, use every
documented platform path without encountering a known false promise, and rely
on CI, structured output, exit codes, retry policy, and release artifacts.

Target release:

- `oxvif 0.16.0` published first;
- `oxvif-cli 0.1.0` published and installable with `--locked`;
- Windows, Linux, and macOS native-credential contracts all pass;
- signed project-owned APT and Homebrew channels plus the Windows artifact are
  installed and verified before any three-platform support claim.

### 1.2 Commercial-pilot cut

The pilot is ready when the beta also has cross-platform credential storage,
actionable diagnostics, a published schema contract, signed/reproducible
artifacts, registry recovery guidance, and measured compatibility against a
declared camera matrix.

The commercial-pilot cut is not a claim of general ONVIF device-manager
completeness. Device mutation remains a later, separately gated stage.

---

## 2. Audit baseline

The plan starts from the following measured state on 2026-09-01.

### 2.1 Passing evidence

- `cargo fmt --all -- --check` passes.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  passes.
- `cargo test --workspace --all-features` reports 1,023 passed and 3 ignored
  across 14 suites.
- `cargo test -p oxvif-cli --locked` reports 78 passed across 4 suites.
- `cargo +1.88.0 check --workspace --all-features` passes at the declared MSRV.
- `cargo build --release -p oxvif-cli --locked` produces a working Windows
  executable.
- The built-in mock completes device information, profiles, health, and
  structured-output workflows end to end.
- The registry uses an OS-released exclusive lock and atomic replacement, and
  has tests for concurrent writers, migration, corruption refusal, redaction,
  and import plan/apply behavior.

### 2.2 Release blockers found

- `cargo package -p oxvif-cli` cannot verify until `oxvif ^0.16.0` exists on
  crates.io.
- The public release list ends at 0.15.0 while local release documentation says
  0.16.0/CLI 0.1.0 is already released.
- `cargo audit` finds `RUSTSEC-2026-0258` through `h2 0.4.15`; `chacha20
  0.10.1` is also yanked.
- CI is Ubuntu-only and its branch filters do not run on direct `develop`
  pushes.
- The only production native credential backend is Windows; the non-Windows
  backend always returns `CREDENTIAL_UNAVAILABLE`.
- `-v/--verbose` is accepted and stored but does not alter diagnostics.
- `--retries` applies to general diagnostics but not to health or discovery,
  despite the global wording and retryable metadata.
- most human diagnostic output falls back to pretty-printed JSON.
- 56 of 59 command descriptors have no example and 58 report only the generic
  output type `object`.
- the repository has no `SECURITY.md`, `CONTRIBUTING.md`, code of conduct,
  issue templates, PR template, or automated dependency-update configuration.

### 2.3 Operational unknowns

- There is no CI coverage report, fuzz target, or long-running fleet soak gate.
- Windows Credential Manager is compiled locally but its production adapter is
  not exercised by a CI-safe contract test.
- Authenticated CLI sessions do not opt into the library's clock-offset
  synchronization path.
- There is no CLI policy for private CAs or self-signed HTTPS cameras.
- Live CLI verification records broad discovery and one selected GeoVision
  diagnostic path, but not a declared multi-vendor support matrix.

### 2.4 Implementation evidence — 2026-09-01

Local implementation after the baseline audit currently has the following
evidence:

- format and `-D warnings` Clippy gates pass for the full workspace;
- `cargo test --workspace --all-features --locked` reports 1,052 passed and
  four ignored across 14 suites;
- a transient proxy fixture proves a first-attempt HTTP 503 recovers on the
  second attempt, while the existing deterministic-status fixture proves HTTP
  400 is attempted only once;
- both verbosity levels reject URL-embedded credentials without exposing URI
  userinfo, process-environment credentials, WS-Security names, or HTTP
  authorization material in stdout or stderr;
- the exact Rust 1.88 workspace check passes;
- timeout cancellation closes the hanging fixture connection, returns active
  work to zero, releases a single fleet worker for the next device, and stops a
  retry before the next request; cancelling the CLI discovery wrapper also
  aborts every spawned interface worker and returns its active count to zero;
- private HTTPS fails without its generated CA and succeeds only after that CA
  is explicitly merged with platform roots; malformed/empty bundles and
  private-key material fail before connection;
- malformed custom roots fail before any socket activity across setup,
  diagnostics, health, refresh, fleet items, and discovery enrichment, proving
  those execution paths use the shared transport factory;
- target-specific `keyring 3.6.3` adapters implement Credential Manager,
  Keychain, and synchronous Secret Service behind one contract; native
  lifecycle tests pass on Windows x64, macOS Intel/Apple Silicon, and Ubuntu
  x86_64/aarch64 CI, both Linux rows pass the missing-D-Bus negative path, and
  CLI-owned password buffers zeroize on drop;
- `cargo audit` scans the tracked 399-package release lock graph with no
  vulnerability or yanked-package result;
- `cargo package -p oxvif --allow-dirty --locked` packages and rebuilds 128
  files (677.7 KiB compressed); `oxvif-cli --locked --list` contains only its
  manifests, lockfile, README, versioned schemas, source, and integration test;
- library and CLI rustdoc pass separately with `RUSTDOCFLAGS=-D warnings`,
  avoiding the Cargo lib/bin output-name collision;
- Windows local binary smoke passes version, descriptor, Agent guide v5, and
  Bash/Zsh/Fish/PowerShell completion generation; structured parse failure
  remains a schema-v3 `INVALID_ARGUMENT` envelope;
- GitHub workflow and issue-template YAML parses locally. CI run
  [`33496836237`](https://github.com/smiti1642/oxvif/actions/runs/33496836237)
  passes the five architecture rows, native credential contracts, release
  binary smoke, package-content checks, rustdoc, MSRV, Clippy, and audit gates.

The CLI package cannot complete Cargo's independent package verification until
`oxvif 0.16.0` is published. That and public artifact/install verification stay
open at the mandatory cut line; no tag, crate, or GitHub Release was published
during this implementation pass.

---

## 3. Locked decisions

1. **Do not start Stage 4 device mutation before Stages R0-R2 close.** Adding
   writes while release, retry, logging, and platform contracts drift increases
   both user and device risk.
2. **The first public CLI release is a diagnostic beta.** Its limited surface is
   intentional and must be stated plainly.
3. **The first public CLI release requires secure native credential storage on
   all three desktop platforms.** Windows Credential Manager, macOS Keychain,
   and Linux Secret Service must pass the same backend contract before release;
   no plaintext fallback is allowed.
4. **Machine stdout is a compatibility surface.** JSON/JSONL stdout never
   contains logs, progress, color, prompts, or human hints.
5. **A retryable descriptor is an executable promise.** A command marked
   retryable must honor the common retry policy, or its descriptor must say it
   is not retryable.
6. **Release truth wins over scheduled dates.** A document says `Released` only
   after crates.io packages, tags, and release artifacts exist and verify.
7. **Do not block 0.1 on a broad source refactor.** Large files may be split
   incrementally only when a stage already changes that domain and standing
   tests protect behavior.
8. **No insecure TLS default.** Private CA support lands before an opt-in
   insecure escape hatch; any insecure mode is visible in human and structured
   output and prohibited by the default Agent guide.
9. **Commercial readiness is evidence-based.** A support claim names tested OS,
   architecture, credential backend, camera vendor/model/firmware, and command
   surface.
10. **Graduate distribution in two controlled steps.** The first public CLI
    release uses a project-owned signed APT repository and Homebrew tap. After
    those channels have repeatable install/upgrade/uninstall evidence, the
    project submits to Homebrew Core and Debian; Ubuntu inclusion should follow
    Debian synchronization where practical.

---

## 4. Delivery map

```text
R0 release truth + dependency security
                 |
                 v
R1 CI/release pipeline and publishable packages
                 |
                 v
R2 runtime reliability contract
          /                 \
         v                   v
R3 human UX             R4 Agent schema
          \                 /
           v               v
       three-platform credential and distribution gate
                    |
                    v
        beta cut-line closeout
                    |
                    v
       pilot cut-line closeout
                    |
                    v
        commercial diagnostic pilot
                    |
                    v
        separate controlled-write implementation plan
```

R0-R2 are release blockers. R3, the minimum R4 descriptor corrections, the R5
native-credential backends, and the three-platform distribution channels are
required for a credible 0.1 beta. The remaining R5 compatibility and artifact
controls establish the commercial-pilot cut.

---

## 5. Stage R0 — release truth and dependency security

### 5.1 Dependency repair

- [x] Update the lockfile so `h2 >= 0.4.16` is selected.
- [x] Resolve or explicitly document the yanked `chacha20 0.10.1` path; prefer
  an upstream dependency update that removes it.
- [x] Run `cargo audit` with no vulnerability result.
- [x] Add a CI audit gate. Yanked/unmaintained warnings have an explicit allow
  file with owner, reason, and expiry; they are never silently ignored.
- [x] Record dependency changes in `CHANGELOG.md` when they affect the release
  lockfile or supported behavior.

### 5.2 Release-state correction

- [x] Change unreleased 0.16.0/0.1.0 documents to `Unreleased` until publication
  is verified.
- [x] Gate README installation claims so they distinguish crates.io install
  from source install.
- [x] At the audit baseline, state that the then-current 0.1 backend was
  Windows-only; later implementation status is tracked in the three-platform
  plan and must not be presented as verified before native CI passes.
- [x] Qualify native OS credential wording with the actual per-platform
  implementation and verification status.

### 5.3 R0 exit gate

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo audit
```

Exit requires all four commands to return zero and the documentation to contain
no claim that depends on an unpublished artifact.

---

## 6. Stage R1 — CI, packaging, and release pipeline

### 6.1 CI matrix

- [x] Run format once on Ubuntu.
- [x] Run stable check/test on Ubuntu, Windows, and macOS.
- [x] Run the exact Rust 1.88 MSRV check on at least Ubuntu and Windows.
- [x] Compile all targets/features on Windows so the native credential adapter
  cannot rot behind `cfg(windows)`.
- [x] Run CLI smoke tests from the produced binary on every OS:
  `--version`, `--help`, structured parse failure, `describe`, `agent guide`,
  and every completion shell.
- [x] Run CI for pull requests targeting `master` or `develop`, and pushes to
  both maintained branches.
- [x] Add cancellation/concurrency controls so a superseded branch run does not
  consume release resources.

### 6.2 Package verification

- [ ] Publish and verify `oxvif 0.16.0` first.
- [x] Run `cargo package -p oxvif` and rebuild the generated package.
- [ ] Run `cargo package -p oxvif-cli` without `--no-verify`.
- [ ] Install the generated CLI package from its unpacked package directory in
  a clean environment.
- [ ] Verify the installed executable, not the workspace target binary.
- [x] Confirm the package contains only the intended manifest, lockfile,
  README, source, tests, and versioned schemas.
- [x] Defer public `oxvif-cli` publication and the crates.io install test to the
  mandatory cut-line closeout after R2-R4 pass.

### 6.3 Release artifacts

- [x] Add a release workflow that builds declared OS/architecture targets.
- [x] Build Windows x86_64, Linux x86_64/aarch64, and macOS
  x86_64/aarch64 artifacts before the first public CLI release.
- [x] Produce SHA-256 checksums next to every staged artifact.
- [x] Produce an SBOM for the release dependency graph.
- [ ] Sign Windows artifacts before claiming commercial-pilot readiness.
- [x] Attach version-matched shell completions and schema files when those
  become external artifacts.
- [x] Make the workflow verify that the tag, Cargo versions, guide version,
  stdout schema version, changelog, and release note agree.

### 6.4 R1 exit gate

The stage passes only when a clean host can install the published CLI with
`--locked`, the installed binary passes smoke tests, and the public tag/release
links resolve to the same source revision.

---

## 7. Stage R2 — runtime reliability contract

### 7.1 Retry policy

Implement one shared policy used by single-device diagnostics, health,
discovery enrichment, and fleet items.

- [x] Define retryable transport classes explicitly: timeout, connection reset,
  temporary connection refusal, and selected HTTP transport failures.
- [x] Do not retry invalid arguments, missing resources, schema corruption,
  authentication rejection, or deterministic SOAP faults.
- [x] Apply exponential backoff with bounded jitter and a documented maximum.
- [x] Decide whether `--timeout` is per attempt or total command budget; encode
  that decision in help, descriptors, and tests.
- [x] Make health honor `--retries`, or change its metadata and help before
  release.
- [x] Decide discovery scan retry semantics separately from per-record
  enrichment; avoid repeating a full multicast scan accidentally.
- [ ] Include attempt count in verbose diagnostics and structured metadata where
  additive compatibility permits it.

Required tests:

- [x] retryable first failure followed by success;
- [x] non-retryable failure performs one attempt;
- [x] timeout cancellation does not leave work running;
- [x] fleet retries preserve deterministic device ordering;
- [x] partial and total fleet failures retain their documented exits;
- [ ] health and enrichment use the same policy promised by their descriptors.

### 7.2 Diagnostic observability

- [x] Implement `-v` as sanitized command/selector/attempt/timing diagnostics.
- [ ] Implement `-vv` as sanitized service-resolution and retry-reason detail.
- [x] Keep all verbose output on stderr.
- [x] Make `-q` suppress only non-essential human context, never command data or
  structured warnings.
- [x] Prove that credentials, WS-Security material, HTTP authorization, and URI
  userinfo never appear at either verbosity level.
- [ ] If this cannot be completed for 0.1, remove `-v/-q` rather than ship
  no-op semantics.

### 7.3 Authentication and TLS compatibility

- [x] Add an `auto|always|never` client-side clock-offset policy; `auto` is the
  default and never changes device time.
- [ ] Test an authenticated mock with deliberate clock skew.
- [x] Add private-CA input with an explicit file path and validation.
- [x] Decide whether a TLS server-name override is required for IP-addressed
  devices: it is not part of 0.1; certificates must contain the addressed DNS
  name or IP SAN.
- [x] Keep an insecure TLS escape hatch out of 0.1 and explicitly forbid
  disabling certificate or hostname verification in the default Agent guide.
- [x] Add tests that target normalization continues to reject URL-embedded
  credentials.

### 7.4 Registry recovery

- [x] Document the exact config path and files on every supported OS.
- [x] Add `config path` and `config validate` or an equivalent `doctor` command.
- [x] Document manual backup/restore before adding an automated mutation.
- [ ] Define whether registry writes require a durable directory sync in
  addition to atomic file replacement; test the chosen guarantee where the OS
  permits it.
- [x] Detect orphaned snapshot files and report them without destructive
  automatic cleanup.

### 7.5 R2 exit gate

Every global reliability option must have a standing integration test showing
an observable effect. No help text, descriptor, or Agent guide may promise
behavior that is absent from execution.

---

## 8. Stage R3 — human terminal UX

### 8.1 Purpose-built renderers

- [x] Render media profiles as a compact token/name/fixed/video/audio/PTZ table.
- [x] Render capabilities by service and high-value feature instead of dumping
  the serialized object.
- [x] Render services as namespace/service URL rows.
- [x] Render PTZ status and presets in concise tables.
- [x] Render health as summary plus warnings/failures by default.
- [ ] Add health detail controls such as `--details`, `--failures-only`, or a
  category selector after locking their command spelling.
- [ ] Keep `--json` as the full-fidelity representation; the human renderer must
  not remove structured fields.

### 8.2 Terminal behavior

- [ ] Keep widths deterministic when stdout is redirected.
- [ ] Define truncation and Unicode width behavior with tests.
- [ ] Do not add color until `NO_COLOR`, non-TTY, and structured-output rules
  are specified.
- [ ] Decide whether long health/discovery output gets an opt-in pager; never
  start one under `--non-interactive` or redirected output.
- [x] Add help snapshots or focused assertions for root and every first-level
  command before 0.1 is tagged.

### 8.3 Completion contract

- [x] Reject meaningless combinations such as `completion bash --output json`,
  or define a structured envelope that contains the script. Do not silently
  ignore the selected output contract.
- [x] Keep static completion network-free and registry-free.
- [x] Defer dynamic device/Group/View completion until it can be implemented
  without network access, prompts, or observable registry writes.

### 8.4 R3 exit gate

Run a human smoke tour against the mock and capture reviewable output for info,
profiles, capabilities, services, PTZ, health, discovery, and fleet partial
success. No default diagnostic view may be only a generic JSON dump.

---

## 9. Stage R4 — Agent and schema contract

### 9.1 Descriptor completion

- [x] Give every command an exact output kind rather than generic `object`.
- [ ] Give every argument a semantic description, allowed values/range, and
  conditional requirement/conflict where applicable.
- [ ] List only errors reachable from that command.
- [ ] Add at least one executable success example per command.
- [ ] Add safe plan/apply examples to commands that mutate local state.
- [ ] Represent partial-success exits and fleet item/summary shapes directly.
- [ ] Test that every descriptor maps to an implemented parser path and every
  canonical parser path has a descriptor.

### 9.2 Published schema

- [x] Publish JSON Schema for success envelope, error envelope, warnings,
  metadata, command descriptors, and tagged command data.
- [x] Decide whether schemas ship inside the crate, release artifacts, the
  executable through a command, or all three.
- [x] Validate representative JSON and JSONL output against the schemas in CI.
- [x] Define additive versus breaking changes and when `SCHEMA_VERSION` must
  increment.
- [ ] Add contract fixtures for single success, argument error, device error,
  fleet success, fleet partial success, and fleet total failure.

### 9.3 Single source of truth

Avoid a pre-release rewrite, but stop descriptor drift incrementally:

- [x] introduce shared command metadata consumed by descriptor generation and
  tests;
- [x] keep Clap-specific presentation in `main.rs` while deriving stable command
  identity, risk, retryability, and result kind from shared definitions;
- [x] fail tests when help, parser, request name, descriptor, or Agent guide
  disagrees.

### 9.4 R4 exit gate

An Agent using only `agent guide`, `describe`, published schemas, process exit,
and structured stdout must be able to execute every 0.1 command without reading
the Markdown manual or encountering a prompt.

---

## 10. Stage R5 — platform, community, and commercial-pilot hardening

### 10.1 Native credentials

- [x] Keep Windows Credential Manager as a tested backend.
- [x] Add macOS Keychain support.
- [x] Add Linux Secret Service/libsecret support with clear headless-session
  failure behavior.
- [x] Add backend contract tests for set/get/delete/no-entry/error mapping.
- [x] Never fall back to plaintext credential files.
- [x] Consider zeroizing in-memory password buffers after use; document the
  remaining process-memory threat model.

### 10.2 Three-platform distribution

- [x] Produce versioned Debian packages for Linux x86_64 and aarch64.
- [ ] Publish signed APT repository metadata so a configured host can run
  `apt update` followed by `apt install oxvif`.
- [x] Produce staged Homebrew bottles for macOS Intel and Apple Silicon.
- [ ] Maintain a versioned Homebrew formula so a configured host can run
  `brew install oxvif` through the selected tap.
- [x] Smoke-test the installed binary from staged APT and Homebrew channels,
  including Homebrew formula tests before and after bottle reinstall, rather
  than the workspace or unpacked release artifact.
- [ ] Document repository/tap setup, upgrade, downgrade, uninstall, checksum,
  signing-key rotation, and supported-architecture behavior.
- [x] Keep project-owned APT repository and Homebrew tap publication behind the
  same explicit release approval as crates.io, tags, and GitHub Release.

Non-publishing evidence: commit `f9790ec` passed
[release staging run 33508813383](https://github.com/smiti1642/oxvif/actions/runs/33508813383)
with all build, signed-APT, and Homebrew jobs successful; the publish job was
intentionally skipped. Matching
[general CI run 33508813343](https://github.com/smiti1642/oxvif/actions/runs/33508813343)
passed all 21 jobs. Public install and lifecycle checks remain open until the
mandatory approval cut line.

### 10.2a Official-channel graduation

Official distribution is a committed follow-up rather than a dependency on an
external review queue for the first public release:

- [ ] Maintain reproducible source and binary packaging, license/copyright
  metadata, man pages, changelog, and architecture declarations suitable for
  downstream review.
- [ ] Record repeatable install, upgrade, downgrade, and uninstall evidence for
  at least two consecutive releases through the project-owned APT repository
  and Homebrew tap.
- [ ] Submit the stable formula to Homebrew Core and track the review URL,
  required changes, and accepted version in the release record.
- [ ] Prepare a Debian Policy-compliant source package, file the required Debian
  packaging request, and track sponsorship/review through acceptance.
- [ ] Prefer Ubuntu synchronization from Debian; document a separate Ubuntu
  submission only if synchronization cannot meet supported-release needs.
- [ ] Keep the project-owned channels maintained as a fallback until official
  channels are verified and their update latency is understood.

### 10.3 Community operation

- [x] Add `CONTRIBUTING.md` with local gates and fixture/redaction rules.
- [x] Add `SECURITY.md` with a private reporting channel and supported versions.
- [x] Add a code of conduct.
- [x] Add issue and pull-request templates that request OS, CLI version, camera
  vendor/model/firmware, sanitized command, and structured error code.
- [x] Add automated dependency-update configuration.
- [x] Publish a compatibility and support policy for the library and CLI
  separately.

### 10.4 Camera compatibility matrix

- [x] Define a sanitized test record containing vendor, model, firmware, ONVIF
  profiles, transport, authentication method, and tested CLI commands.
- [ ] Cover at least five vendors before a broad commercial claim.
- [ ] Include clock-skew, HTTP Digest, WS-Security, multiple profiles, PTZ/no-PTZ,
  Media2/no-Media2, and SOAP-fault behavior.
- [ ] Retain scrubbed fixtures when licensing and secret-removal rules permit.
- [ ] Run a 205-device mock fleet soak repeatedly with injected timeout,
  connection, authentication, malformed XML, and partial-interface faults.
- [ ] Define latency and memory ceilings for single and fleet diagnostics.

### 10.5 Commercial artifact controls

- [ ] Sign release artifacts and publish checksums/SBOM.
- [ ] Document upgrade, downgrade, registry backup, and schema rollback.
- [x] Define supported OS/architecture and minimum terminal requirements.
- [x] Establish a vulnerability response target and dependency update cadence.
- [ ] Provide a support bundle/doctor output that is sanitized by construction.

### 10.5a Sanitized downstream validation evidence

The following evidence is useful for compatibility tracking but does **not**
constitute release acceptance or a broad commercial-support claim:

- 2026-09-01, Windows x64, GeoVision_2 GV-TBL8810,
  firmware `V111_2025_12_09`, hardware ID `GV-TBL8810`;
- the current local release binary completed
  `device info --output json --non-interactive` with process-only environment
  credentials, exit code 0, envelope `ok=true`, and no structured error code;
- an opt-in downstream ONVIF DLL relay test observed both active and inactive
  states, with the generic Profile M callback preceding the legacy STA_GPIO
  callback; all write/pair/restore assertions passed;
- the downstream non-device regression suite reported 315 passed, five
  pre-existing conditional skips, and zero failures; the rerunnable real-device
  test remains skipped by default (downstream commit `e248e96`);
- no IP address, account, password, URI userinfo, or credential material was
  retained in this record.

### 10.6 R5 exit gate

The public CLI beta requires all three native credential backends and verified
Windows, APT, and Homebrew installs. The commercial diagnostic pilot additionally
requires a versioned support matrix, signed artifacts, a recovery procedure, a
clean security gate, and at least one successful upgrade rehearsal from the
public 0.1 registry/schema.

---

## 11. Mandatory cut-line closeout

Reaching an engineering gate is not the end of a release. Every beta or pilot
cut line completes the following sequence in order: verification, documentation,
publication, then downstream notification. A release is incomplete if any step
is skipped.

### 11.1 Final verification

- [ ] Run every standing verification command from a clean worktree.
- [ ] Verify the intended package with a clean Cargo home and target directory.
- [ ] Confirm the release artifact, installed executable, schema, Agent guide,
  and source build report the same versions.
- [ ] On clean supported hosts, install through the configured APT repository
  and Homebrew tap and run the standing binary smoke suite.
- [ ] Re-run the built-in mock smoke tour and the declared real-camera release
  matrix.
- [ ] Confirm `cargo audit` is clean at the release lockfile revision.

### 11.2 Documentation update

- [ ] Update the root README, CLI package README, English CLI manual, and
  Traditional Chinese CLI manual for the shipped behavior.
- [ ] Update `CHANGELOG.md` and the version-specific release note with verified
  commands, supported platforms, credential/TLS limitations, compatibility
  evidence, and known issues.
- [ ] Change `Unreleased` to the actual release date only after publication
  succeeds.
- [ ] Update every affected active plan's implementation status and stage
  verdict record.
- [ ] Move a completed plan from `docs/active/` to `docs/done/`, update
  `docs/README.md`, and grep the repository for stale cross-references.
- [ ] Verify that installation instructions name only artifacts that exist
  publicly.

### 11.3 Release update

- [ ] Publish packages in dependency order: `oxvif`, wait for index visibility,
  verify the CLI package, then publish `oxvif-cli`.
- [ ] Verify `cargo install oxvif-cli --locked` from crates.io in a clean
  environment.
- [ ] Create the matching git tag and GitHub Release from the verified commit.
- [ ] Attach the declared binaries, checksums, SBOM, completions, and schemas.
- [ ] Publish signed APT metadata/packages and the version-matched Homebrew
  formula/bottles only after the same release approval.
- [ ] Verify `apt install oxvif`, `brew install oxvif`, upgrade, and uninstall
  against the public channels.
- [ ] Confirm every public download and documentation link resolves.
- [ ] Record the release URL, crates.io versions, artifact hashes, and final
  verification output in the stage verdict.

### 11.4 ONVIF Refector Agent handoff

The current downstream live-validation task is the Codex task titled
**`ONVIF Refector Agent`**. At plan creation it is running from
`C:\Users\smiti\Documents\GitHub\ONVIF-refactor` and testing the CLI against
authorized LAN cameras.

- [ ] Do not send a completion notice before the package and Release are
  publicly verified.
- [ ] After release, send the task the exact CLI/library versions, git tag,
  Release URL, installation command, schema version, and supported platforms.
- [ ] Summarize changed retry, timeout, credential, TLS, output, and exit-code
  behavior that can affect its live-camera workflow.
- [ ] Provide a sanitized regression checklist covering discovery, setup/auth,
  info, capabilities, services, profiles, stream/snapshot URI, PTZ reads,
  health, and fleet partial failure.
- [ ] Ask it to report vendor/model/firmware, command outcome, structured error
  code, and sanitized diagnostics for every regression.
- [ ] Record that the handoff was sent in the stage verdict; do not claim live
  validation passed until the downstream task reports its result.

### 11.5 Cut-line exit gate

The cut line passes only when documentation matches the published artifacts,
the GitHub Release and crates.io packages are verified, and the `ONVIF Refector
Agent` task has received the release handoff. Downstream live-camera results may
remain a subsequent pilot gate, but the notification itself is mandatory.

---

## 12. Controlled-write handoff

After R0-R5, controlled writes return to Stage 4 of the main CLI plan. The first
candidate should be bounded PTZ movement rather than network, users, firmware,
restore, or factory reset.

The separate implementation plan must require:

- explicit immutable device selection for high-risk actions;
- capability preflight;
- bounded movement duration and best-effort stop on cancellation;
- plan fingerprint and explicit apply authorization;
- stable risk and idempotency metadata;
- mock fault injection plus authorized real-device recovery testing;
- no dependence on ambient current-device state for dangerous actions.

No Stage 4 implementation is part of this release-hardening plan.

---

## 13. Standing verification commands

Every stage runs the smallest relevant tests during development and the complete
gate before its verdict:

```sh
cargo fmt --all -- --check
cargo +1.88.0 check --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p oxvif-cli --locked
cargo audit
cargo doc --workspace --all-features --no-deps
```

Release candidates additionally run:

```sh
cargo package -p oxvif
cargo package -p oxvif-cli
cargo install oxvif-cli --locked
```

The package and install commands must run in a clean environment against the
published dependency order. A source-path install is useful smoke evidence but
does not substitute for crates.io verification.

---

## 14. Stage verdict record

Each completed stage appends a record here containing:

- commit or pull request;
- checklist items closed;
- exact verification commands and results;
- defects found by the stage and the tests that now pin them;
- deviations from this plan and the reason;
- remaining blockers to the next cut line.

Do not mark a stage complete from code inspection alone. A release, platform,
retry, security, or compatibility claim requires executable evidence from the
environment it names.
