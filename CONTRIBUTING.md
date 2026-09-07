# Contributing to oxvif

[English](CONTRIBUTING.md) | [繁體中文](CONTRIBUTING_zh.md)

Thank you for helping improve oxvif. Bug reports, compatibility observations,
documentation corrections, and focused pull requests are welcome.

| Section | Purpose |
| --- | --- |
| [Before opening a change](#before-opening-a-change) | Scope and compatibility |
| [Local verification](#local-verification) | Required checks |
| [Fixtures and device reports](#fixtures-and-device-reports) | Sanitization |
| [Pull requests](#pull-requests) | Acceptance criteria |
| [Dependency and release-tooling changes](#dependency-and-release-tooling-changes) | Additional review gates |

## Before opening a change

1. Search existing issues and active plans under `docs/active/`.
2. Keep library API, CLI human output, and structured Agent output as separate
   compatibility surfaces.
3. Do not add device-mutating behavior without an active plan that defines
   confirmation, plan/apply, recovery, and real-device validation gates.

## Local verification

Use Rust 1.88 or newer. Before submitting a pull request, run:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo +1.88.0 check --workspace --all-features --locked
cargo audit
```

Changes to `oxvif-cli` should also exercise the relevant human and structured
output paths. Never update a JSON schema, descriptor, exit code, or public help
claim without a focused test.

## Fixtures and device reports

Real-device evidence is valuable, but it must be sanitized before it enters an
issue, fixture, log, commit, or CI artifact. Remove:

- IP and MAC addresses unless a documentation-only address such as `192.0.2.0/24`
  is sufficient;
- usernames, passwords, authorization headers, WS-Security password digests,
  nonce material, cookies, tokens, and private keys;
- URI userinfo and externally reachable snapshot or stream URLs;
- serial numbers, UUIDs, hostnames, and site/customer names unless explicitly
  needed and safe to disclose.

Prefer a record containing OS/architecture, oxvif version, camera vendor,
model, firmware, declared ONVIF profiles, sanitized command, exit code,
structured error code, and expected versus observed behavior. Do not commit
ONVIF WSDL/XSD files or derived schema fixtures; see `.gitignore` and the
repository's schema policy.

## Pull requests

Keep changes reviewable and explain the observable contract. A pull request is
ready when its tests pass, documentation matches behavior, new diagnostics are
secret-safe, and unrelated formatting or generated-file churn is absent.

## Dependency and release-tooling changes

Review each dependency's upstream changes, feature requirements, and MSRV;
then test the combined lockfile. Preserve fingerprint, XML, and native credential
contracts. Do not use a full `cargo update` to resolve a targeted PR conflict.
For XML dependency updates also run:

```text
cargo fetch --locked
python packaging/check_xml_features.py
```

Changes to `.github/workflows/release.yml`, its pinned actions, or `packaging/`
require a successful **manual, non-publishing release-staging run before merge**.
Ordinary PR CI alone is insufficient. A maintainer must review the candidate
workflow before dispatching it; never execute unreviewed PR code through
`pull_request_target` or supply release credentials to a dependency PR.

```text
gh workflow run release.yml --ref <candidate-branch> -f tag=<full-candidate-commit-sha> -f publish=false
```

`--ref` selects the changed workflow, while `tag` selects its source checkout.
Use the full commit SHA for `tag`: branch names containing `/` are not valid
artifact labels. Record both SHAs and the run URL in the PR. Require all native
artifact rows, credential tests, SPDX output, APT install/remove, and Homebrew
install/bottle/reinstall checks to pass. Compare scanner version and meaningful
dependency coverage with the previous staging output; a nonempty SBOM file is
not sufficient. If code or release tooling changes afterwards, rerun staging.
Documentation-only changes may refer to the unchanged tested code/tooling tree.

`publish=false` uploads temporary Actions artifacts only. It does not authorize
creating a GitHub Release, publishing crates, or changing public package channels.
