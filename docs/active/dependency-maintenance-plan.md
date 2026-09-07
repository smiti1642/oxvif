# Dependency maintenance and consolidated Dependabot updates

[English](dependency-maintenance-plan.md) | [繁體中文](dependency-maintenance-plan_zh.md)

Status: Implemented and integrated; first grouped PR verified, subsequent-cycle check pending.
Date: 2026-09-07. Review baseline: `master` at `ee0460f`.

| Section | Purpose |
| --- | --- |
| [Outcome](#outcome) | Scope and completion criteria |
| [Execution record](#execution-record) | Current results and pending gates |
| [Review evidence](#review-evidence) | Current six PRs |
| [Execution](#execution) | Ordered implementation and integration |
| [Dependabot configuration](#dependabot-configuration) | One routine cross-ecosystem PR |
| [Verification](#verification) | Tests and operational evidence |
| [Documentation and release](#documentation-and-release) | Handoff and release boundaries |
| [References](#references) | GitHub configuration sources |

## Outcome

Complete PRs #6–#11, including the required source migrations, and consolidate
future routine Cargo and GitHub Actions updates into one weekly Dependabot group.
Keep Rust 1.88 support, CLI schema v3, existing fingerprints, and XML behavior.
Retain human review; grouping does not enable automatic merging.

“One PR” means one consolidated routine version-update PR per group/update cycle.
Security updates use a separate GitHub mechanism and may create additional PRs.
Do not disable security updates or suppress advisories to satisfy a numeric PR limit.
GitHub updater errors or unsupported combinations must be reported, not silently
replaced with a two-PR-per-ecosystem configuration.

Execution was authorized on 2026-09-07. Publishing remains outside this task.

## Execution record

- Integration branch: `codex/dependency-maintenance`, based on `ee0460f`.
  All six reviewed heads were unchanged when execution began; the default branch
  has no configured branch protection or repository rulesets.
- The bot commits were cherry-picked in order #7, #8, #10, #11, #9, #6,
  preserving authorship. Replacement PR #12 validated the combined tree;
  old PRs were retained until integration was verified.
- sha2 0.10.9 fixed outputs were captured before migration. The migrated encoder
  passes the same record/plan constants and leading-zero vector. Existing plan
  application checks cover the captured old fingerprint.
- Three synthetic XML compatibility tests passed with quick-xml 0.41 before
  migration, then with 0.42 through an isolated downstream consumer, encoding off
  and on. The optional schema test's parser also required the 0.42 API migration;
  no external ONVIF schema files or derived data were added.
- Local workspace all-feature tests: **1,076 passed, 4 ignored**; default library
  tests: **548 passed, 1 ignored**. Both Clippy configurations, six individual
  feature checks, formatting, and Rust 1.88 passed. Hosted acceptance is below.
- `cargo audit` on 2026-09-07: no known vulnerabilities, 411 dependencies.
  `cargo outdated --workspace --root-deps-only` completed; remaining updates
  (including keyring 4.2 and newer ipnet) are deferred to later reviewed batches.
- Future release-tooling changes have an explicit manual staging-before-merge
  gate in `CONTRIBUTING.md`, using the candidate workflow ref and source SHA.
- Baseline staging run `33856099748` exposed a pre-existing SBOM coverage gap:
  Linux binary output contained only the scan directory; Windows output added
  two executable entries with unknown versions, but no Rust dependency inventory.
  The candidate now retains binary SBOMs and adds separate `.source.spdx.json`
  files checked against all 411 locked package/version pairs. Local Syft 1.51.1
  passed this check; scanner archive checksum was verified against its upstream
  release checksum. Four positive/negative checker tests passed. Source inventory
  includes development and target-specific packages, not per-binary linkage.
  Initial candidate staging `34092581258` was intentionally cancelled for this
  coverage fix; it is not acceptance evidence.
- Candidate `391880f` passed all 21 CI jobs (`34092932594`). Its staging run
  `34092981151` exposed Python 3.10 on the Ubuntu 22.04 release runners: source
  SBOM generation succeeded but the checker could not import `tomllib`.
  The workflow now explicitly selects Python 3.12 via a reviewed, SHA-pinned
  `actions/setup-python` v7.0.0. That interrupted staging run is not a pass.
- Accepted candidate: `1fd35f1333252033520138e2af65077a5f2a439d`.
  [CI 34094259257](https://github.com/smiti1642/oxvif/actions/runs/34094259257)
  passed all **21 jobs**. [Staging 34094279728](https://github.com/smiti1642/oxvif/actions/runs/34094279728)
  passed **17 jobs**; `Publish GitHub Release` was **skipped**. Both signed APT
  repository install/purge rows and Homebrew install/bottle/reinstall rows passed.
- Downloaded all five native artifacts: seven archive/package checksums passed;
  every source SPDX contains the 411 locked package/version pairs plus a directory
  entry, including `oxvif-cli 0.16.0`, with Syft 1.51.1 metadata. Binary package
  coverage matches the five-platform baseline (directory only on Unix; directory
  plus two unknown-version executable entries on Windows). Source inventories
  supplement, rather than conceal, that binary-scanner limitation. Temporary
  Windows artifact smoke returned version 0.16.0 and successful schema-v3 JSON.
- [PR #12](https://github.com/smiti1642/oxvif/pull/12) merged as `279bd00`.
  PRs #6–#11 were closed individually as superseded only after integration.
  [PR #13](https://github.com/smiti1642/oxvif/pull/13) then merged the schema-validated
  grouping configuration as `03b8d30`. Its scope is config/docs only; source,
  lockfile, and release workflow match the accepted candidate exactly.
- `master` and `develop` were synchronized to `03b8d30`. The develop push starts
  an additional automatic non-publishing staging run; the accepted candidate
  evidence above does not depend on calling that new run successful in advance.
- GitHub Cargo updater [34095680778](https://github.com/smiti1642/oxvif/actions/runs/34095680778)
  and Actions updater [34095680772](https://github.com/smiti1642/oxvif/actions/runs/34095680772)
  both succeeded against `03b8d30`. Both logs specify `multi-ecosystem-update: true`,
  group `maintenance`, all update types, and no ignore conditions. Cargo created
  the single grouped [PR #14](https://github.com/smiti1642/oxvif/pull/14), containing
  six new dependency updates; Actions reported nothing to update. This validates
  both ecosystem configurations, but this first batch contains Cargo changes only.
  PR #14 is a new, unreviewed batch and is intentionally not merged by this task.
- The Cargo updater reports `No update possible for keyring 3.6.3` (and for the
  workspace path dependency `oxvif`). No ignore rule was added. Dated follow-up:
  review keyring 4.x feature/native-store migration in the next maintenance
  review, by 2026-09-14; do not infer that a successful grouped job includes
  every version shown by `cargo outdated` or that its batch is ready to merge.
- A subsequent full version-update cycle with #14 open remains to be observed.
  GitHub rejected `gh run rerun 34095680778` with “This workflow run cannot be
  retried”; the available browser is not signed in. Use the repository's
  Dependabot **Check for updates** control in an authenticated owner session,
  or observe the next Monday 09:00 Asia/Taipei scheduled cycle. Confirm the
  existing grouped PR is updated rather than a duplicate routine PR appearing.
  Do not treat a PR-only rebase as equivalent to a full scheduled update check.
  Keep this plan in `active/` until that remaining operational check completes.
- Published v0.16.0 release metadata and all 24 assets retain their 2026-09-04
  timestamps. No tag, crate, public package channel, or installed CLI was updated.

## Review evidence

All six reviewed PRs target baseline `ee0460f`. Recheck their heads before execution.

| PR | Reviewed head | Finding | Required disposition |
| --- | --- | --- | --- |
| [#7 base64](https://github.com/smiti1642/oxvif/pull/7) | `6b8f7e0` | Lockfile-only; upstream patch changes tests, not runtime codec logic; 21 CI checks passed. | Integrate after checking the current diff. |
| [#8 ipnet](https://github.com/smiti1642/oxvif/pull/8) | `4f4208c` | Lockfile-only; subnet iterator boundary fix; CLI parsing/containment paths unchanged; 21 checks passed. | Integrate after checking the current diff. |
| [#10 async-trait](https://github.com/smiti1642/oxvif/pull/10) | `062ec48` | Lockfile-only; removes redundant generated `must_use`; 21 checks passed. | Integrate after checking the current diff. |
| [#11 sha2](https://github.com/smiti1642/oxvif/pull/11) | `25de4f7` | New digest output lacks `LowerHex`; `registry.rs:1735` and `:1824` fail to compile. | Add compatible fingerprint encoding and regression evidence. |
| [#9 quick-xml](https://github.com/smiti1642/oxvif/pull/9) | `f0607a6` | Removed decoder APIs and bytes-to-string API changes break `src/soap/xml.rs`. | Migrate parser and verify semantic compatibility. |
| [#6 SBOM action](https://github.com/smiti1642/oxvif/pull/6) | `9406771` | Action update also changes Syft 1.42.3 → 1.51.1 and installation behavior. Ordinary PR CI does not execute this step. | Run non-publishing release staging before accepting. |

The failures in #9 and #11 are source compatibility failures on stable and MSRV
jobs; they do not establish a need to raise the minimum supported Rust version.

## Execution

### 1. Establish integration state

- [x] Fetch the current default branch and PR heads; record their full SHAs and CI runs.
- [x] Use a dedicated integration branch (or isolated checkout when needed);
  preserve unrelated user work.
- [x] Confirm merge policy and check requirements. Follow ordinary PR integration;
  do not bypass failed checks or force-push protected branches.
- [x] Keep existing individual PRs until their changes are integrated or an explicit
  replacement PR contains the changes and passes verification.

### 2. Integrate reviewed patch updates

- [x] Process #7, #8, and #10 in order, preserving each change's attribution.
- [x] Re-evaluate lockfile conflicts and CI after the base advances. Regenerate only
  the required lock entries; do not introduce the unrelated full `cargo update` set.
- [x] Verify the combined result, rather than treating three historical green runs
  as proof that the final combined tree passes.

### 3. Complete sha2 migration

- [x] Extend #11, or use a clearly linked replacement if editing the bot branch
  would discard migration work during regeneration.
- [x] Encode digest bytes explicitly as two lowercase hexadecimal digits per byte.
  Keep the `sha256:` prefix and exactly 64 hexadecimal digits.
- [x] Capture deterministic fingerprint fixtures on the old implementation before
  changing it. Require identical output on the new implementation, including a
  digest with leading zero bytes.
- [x] Verify old saved discovery records and reviewed import plans remain usable;
  changed records or options still invalidate stale plans before any mutation.
- [x] Pass CLI tests, Clippy, and Rust 1.88 before integration.

### 4. Complete quick-xml migration

- [x] Update both normal and development dependency declarations together.
- [x] Migrate `Reader::decoder`, reference decoding, text/CDATA access, local names,
  namespace checks, and attribute normalization to 0.42 APIs.
- [x] Preserve whitespace around split entity events, named/numeric entities,
  unknown entity handling, CDATA, Unicode, attribute normalization, and namespace
  stripping. Preserve established malformed-input behavior; document any unavoidable
  behavior change before acceptance.
- [x] Test with `encoding` enabled and disabled. Avoid reintroducing the documented
  feature-unification failure through an API that disappears with `encoding`.
- [x] Exercise library, discovery, SOAP faults, mock, health, and CLI fixtures;
  include the existing feature-combination checks for changed parser code.
- [x] Pass stable and Rust 1.88 gates before integration.

### 5. Validate the SBOM action and release pipeline

- [x] Confirm #6 still pins the expected upstream commit and review action inputs,
  runtime requirements, bundled scanner version, and downloader behavior.
- [x] Run the changed workflow with `publish=false` using the candidate workflow
  ref itself. Setting a source SHA input on an old workflow is insufficient.
- [x] Validate all five native artifact rows and successful SPDX JSON generation;
  check CLI identity/version, scanner metadata, and representative dependency
  inventory against the prior output. Investigate missing coverage or large
  inventory changes rather than accepting a nonempty file as sufficient evidence.
- [x] Keep automatic action artifact/release uploads disabled as currently configured;
  retain the workflow's controlled staging artifact upload.
- [x] Ensure future PRs touching the release workflow or packaging execute a
  non-publishing validation path (or a clearly required, recorded manual staging
  gate). Do not claim ordinary Rust CI covers SBOM execution. If automated, use
  unprivileged PR execution, not privileged `pull_request_target` checkout of PR code.

### 6. Activate consolidated Dependabot updates

- [x] Finish existing PR integration first, then enable the configuration below.
- [x] Validate YAML and current Dependabot configuration schema/options; confirm
  both Cargo and GitHub Actions are accepted by the hosted updater.
- [x] Merge the configuration into the default branch and inspect the actual
  Dependabot job log and generated PR. Do not mark activation complete merely
  because local YAML parsing succeeds.
- [ ] Check a subsequent updater execution for duplicate routine PRs while the
  grouped PR is open. Record actual behavior and any service limitations.
- [x] Reconcile obsolete bot PRs only after checking that their changes have landed
  or are covered by the validated replacement. Avoid blanket closure or deletion.

## Dependabot configuration

Configuration activated by PR #13 in `.github/dependabot.yml`:

```yaml
version: 2

multi-ecosystem-groups:
  maintenance:
    schedule:
      interval: weekly
      day: monday
      time: "09:00"
      timezone: Asia/Taipei
    labels: [dependencies]

updates:
  - package-ecosystem: cargo
    directory: /
    patterns: ["*"]
    multi-ecosystem-group: maintenance
    labels: [rust]
  - package-ecosystem: github-actions
    directory: /
    patterns: ["*"]
    multi-ecosystem-group: maintenance
    labels: [ci]
```

Use one group covering all routine update types, including breaking updates.
A failing dependency blocks acceptance of that batch until repaired. Do not
silently ignore major upgrades or split them into routine individual PRs.
If an update cannot be completed, record the reason and a dated follow-up before
proposing any temporary exclusion. Security remediation can proceed separately.

Setting each ecosystem's `open-pull-requests-limit` to 1 alone would still permit
separate ecosystem PRs; grouping is the mechanism that combines their changes.
Existing PRs are not assumed to transform automatically when the configuration changes.

## Verification

Run these against the final combined candidate, respecting native OS requirements:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo clippy -p oxvif --no-default-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo test -p oxvif --no-default-features --locked
cargo +1.88.0 check --workspace --all-features --locked
cargo audit
cargo outdated --workspace --root-deps-only
```

- [x] Windows x64, Linux x64/ARM64, and macOS Intel/Apple Silicon CI passes,
  including native credential lifecycle tests, CLI smoke, package checks, and docs.
- [x] Focused fingerprint and XML tests prove old/new compatibility, not merely
  agreement between two functions using the same new implementation.
- [x] Non-publishing release staging passes on the final integrated state.
- [x] Each of #6–#11 is merged or explicitly superseded by an integrated equivalent.
- [x] One routine multi-ecosystem PR is observed when updates are available;
  hosted logs confirm both ecosystems ran. If no updates exist, record activation
  as awaiting operational verification rather than inventing a test release.
- [x] Record SHA, workflow URL, versions, results, and unresolved exceptions in this plan.

## Documentation and release

- [x] Update `docs/dependency-pitfalls.md` with the XML and digest migration findings.
- [x] Add maintenance notes to `CHANGELOG.md` under Unreleased; update contribution
  instructions for grouped review and release-workflow validation.
- [x] Keep English and `_zh` public counterparts synchronized when modified.
- [x] Synchronize `develop` with integrated `master` through the existing merge policy.
- [x] Preserve published `v0.16.0`, crate contents, and release assets. This plan
  does not publish a new version or resume APT/Homebrew publication.
- [ ] Prepare a maintenance-release candidate only after integration; remind the
  owner before any external release action, as previously requested.
- [ ] Move this plan and its translation to `docs/done/` and update links only when
  integration and operational grouping verification are both complete.

## References

Checked 2026-09-07:

- [Configure multi-ecosystem updates](https://docs.github.com/en/code-security/how-tos/secure-your-supply-chain/secure-your-dependencies/configuring-multi-ecosystem-updates)
- [Multi-ecosystem grouping behavior](https://docs.github.com/en/code-security/concepts/supply-chain-security/multi-ecosystem-updates)
- [Security update grouping](https://docs.github.com/en/code-security/how-tos/secure-your-supply-chain/secure-your-dependencies/configure-security-updates)
- [Dependabot errors and separate update limits](https://docs.github.com/en/code-security/reference/supply-chain-security/troubleshoot-dependabot/dependabot-errors)
