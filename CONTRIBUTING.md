# Contributing to oxvif

Thank you for helping improve oxvif. Bug reports, compatibility observations,
documentation corrections, and focused pull requests are welcome.

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

