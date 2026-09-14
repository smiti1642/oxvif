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
| [Release presentation](#release-presentation) | CLI download table, notes and asset explanations |
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

## Release presentation

For releases after 0.17.0, the GitHub Release body must guide ordinary CLI users
to the correct download without requiring them to inspect the complete Assets list.

1. **Use English only in the GitHub Release body.** Keep translated documentation
   in the repository, but omit bilingual text and language-switch rows from the
   file consumed by the release workflow.
2. **Lead with a user-facing headline, then a `Download CLI` table.** Put the table
   before the change categories. Include one row for each actually built and
   verified platform/architecture, with columns for Platform, Architecture,
   Download and SHA-256. Link directly to the portable archive and its checksum;
   users normally need one archive, not every attachment.
3. **Use recognizable platform names.** For the current matrix, distinguish
   Windows Intel/AMD 64-bit, Linux Intel/AMD 64-bit, Linux ARM64, macOS Intel and
   macOS Apple Silicon. Map these to the actual x86_64/aarch64 artifacts. Do not
   promise additional architectures or OS compatibility that was not verified.
4. **Keep alternative installers separate.** Place Debian `.deb` packages and
   Homebrew formula/bottles below the primary portable-download table. Explain
   that bottles are Homebrew inputs, not the general macOS archive. Publishing
   these files does not establish official APT, Homebrew, winget or Chocolatey
   channel availability; advertise install commands only for verified channels.
5. **Use categorized, substantive bullets.** Follow the 0.15 style: Added, Fixed,
   Changed and Breaking/limitations as applicable. Explain what each important
   change enables or fixes, prioritizing user-visible workflows over internal
   Mock/test refactoring. Keep migration warnings visible and link to the complete
   versioned changelog for implementation details, evidence and attribution.
6. **Add a short `Other assets` explanation.** Identify `.sha256` as integrity
   checks, `.spdx.json` as binary-scan SBOMs, `.source.spdx.json` as source/lockfile
   inventories, and `.build-tools.txt` as build-tool records. Explain that GitHub's
   Source code archives are source, not compiled CLI downloads. These are not
   additional programs that ordinary users need to install. Retain the SBOM
   limitations described below; inventories are not security certifications.

Before publication, resolve every table link to the actual release tag and asset
filename, using absolute `https://github.com/smiti1642/oxvif/releases/download/`
URLs. Cross-check the table against the verified staging artifact set and remove
all template placeholders. After publication, confirm the linked assets exist
and match the intended platform/checksum. A download table does not replace the
release gates below.

This is a presentation rule, not permission to remove, rename or repackage assets
to reduce their count. Preserve existing consumer URLs and verification material;
changes to the asset layout require their own reviewed packaging change. Do not
rewrite an existing release or move its tag merely to apply this future policy.

## Dependency and release-tooling changes

Review each dependency's upstream changes, feature requirements, and MSRV;
then test the combined lockfile. Preserve fingerprint, XML, and native credential
contracts. Do not use a full `cargo update` to resolve a targeted PR conflict.

Dependabot routine version updates use one `maintenance` multi-ecosystem group
for Cargo and GitHub Actions, scheduled Mondays at 09:00 Asia/Taipei. All update
types, including breaking versions, remain visible in the batch. Review each
component and require the combined gates; grouping does not enable auto-merge.
A failing update blocks the batch until repaired or an explicit, dated exception
is reviewed. Do not silently ignore major upgrades. Security updates may create
additional PRs and must not be disabled to enforce a global one-PR limit.

For XML dependency updates also run (Python 3.11 or newer is required):

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

Release staging emits two inventories per target: `.spdx.json` is the binary
scan; `.source.spdx.json` inventories Cargo.lock and the workspace manifests.
Rust binary scans can omit dependencies and report an unknown application
version. The source inventory is checked against every locked package/version,
including the CLI, but includes development and other-platform packages; it
must not be described as a precise per-binary linkage inventory.

`publish=false` uploads temporary Actions artifacts only. It does not authorize
creating a GitHub Release, publishing crates, or changing public package channels.
