# oxvif support and compatibility policy

**English** | [繁體中文](support_zh.md)

This policy separates the Rust library from the `oxvif` command-line product.
It applies to the oxvif 0.16.x library and the oxvif-cli 0.16.x diagnostic beta.

## Rust library

- Public crates.io releases follow semantic versioning within Rust's practical
  pre-1.0 compatibility conventions.
- The supported release lines are listed in `SECURITY.md`; development branches
  and untagged git revisions are unsupported preview builds.
- The declared MSRV is Rust 1.88. A release must pass workspace checks on that
  toolchain and stable Rust.
- oxvif is an independent implementation, not ONVIF-certified, and does not
  claim that every vendor or ONVIF profile is supported.

## CLI diagnostic beta

The first CLI release is a read-only device diagnostic beta. It may mutate its
local registry, Groups, Views, discovery snapshots, and credential references,
but it does not expose device-setting writes.

| Surface | Beta support |
| --- | --- |
| Windows x86_64 | Tests, binary smoke, and Windows Credential Manager lifecycle contract |
| Linux x86_64/aarch64 | Tests, binary smoke, Secret Service lifecycle contract, and signed-package install/remove evidence |
| macOS x86_64/arm64 | Tests, binary smoke, Keychain lifecycle contract, and formula/bottle install evidence |
| Structured output | Schema version 3; additive fields allowed, breaking changes require a schema-version increment |
| Agent guide | Versioned independently from the stdout schema |
| Native credentials | Credential Manager, Keychain, and Secret Service; native lifecycle contracts are verified on all supported targets; no plaintext fallback |
| TLS | Platform trust roots plus repeatable explicit PEM `--ca-certificate` bundles; no insecure or hostname-verification bypass |

`OXVIF_USERNAME` and `OXVIF_PASSWORD` are intended only for trusted,
process-scoped automation. They are not a persistent credential backend.
Headless Linux without an available, unlocked Secret Service returns
`CREDENTIAL_UNAVAILABLE` and should use this process-scoped path.

## Camera compatibility claims

A compatibility statement must name OS/architecture, oxvif version, camera
vendor/model/firmware, declared ONVIF profiles, authentication path, and tested
commands. A successful command on one model or firmware does not imply support
for the vendor's full product line. Sanitized community reports are evidence,
not certification.

Broad commercial support is not claimed until the release-hardening plan has a
versioned multi-vendor matrix, signed artifacts, recovery procedures, and an
explicit support agreement.

## Reporting and response

Use the issue templates for bugs and sanitized compatibility reports. Use the
private process in `SECURITY.md` for vulnerabilities. Best-effort community
support does not carry a response-time guarantee; any commercial service level
must be stated in a separate written agreement.
