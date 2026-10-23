# Remaining dependency advisory review — 2026-10-23

Scope: the lockfile and CLI configuration at `3cebbb3`, after the TLS/QUIC
security updates. This is a dated applicability review, not an exemption from
future dependency audits.

## Results

`cargo audit` reports no vulnerability entries and three unmaintained warnings:

| Package | Advisory | Applicability and disposition |
| --- | --- | --- |
| zvariant 4.2.0 | [RUSTSEC-2026-0370](https://rustsec.org/advisories/RUSTSEC-2026-0370.html) | Retained by keyring's optional zbus/secret-service dependency resolution; absent from the CLI's selected build dependency trees. No backend migration for this warning. |
| zbus_names 3.0.0 | [RUSTSEC-2026-0371](https://rustsec.org/advisories/RUSTSEC-2026-0371.html) | Same optional dependency path and disposition. |
| zvariant_derive 4.2.0 | [RUSTSEC-2026-0372](https://rustsec.org/advisories/RUSTSEC-2026-0372.html) | Same optional dependency path; this is a procedural macro, not camera-facing runtime code. |

The library has no keyring dependency. The CLI uses keyring 3.6.3 with explicit
target features in `crates/oxvif-cli/Cargo.toml`: `sync-secret-service`,
`crypto-rust`, and `vendored` on Linux; `windows-native` on Windows; and
`apple-native` on macOS. The selected Linux backend is **dbus-secret-service
4.1.0**, not the optional `secret-service 4.0.0 -> zbus 4.4.0` backend.

The upstream warnings describe the standalone crates being incorporated into
zbus 6, rather than a disclosed flaw in the configured credential path. A keyring
major-version/store migration would change credential integration and needs the
separate compatibility/lifecycle review already recorded in
[dependency maintenance](../dependency-pitfalls.md#post-017--reviewed-maintenance-batch-2026-09-22).

## Evidence and reproduction

Run from the workspace root:

```sh
cargo audit
cargo tree --locked -p oxvif-cli --all-features \
  --target x86_64-unknown-linux-gnu -i keyring -e features
cargo tree --locked -p oxvif-cli --all-features --target all -i zbus@4.4.0
```

The first tree selects the synchronous backend features listed above. The
inverted zbus tree prints `nothing to print`, including with `--target all`.
Do not treat a lockfile reverse-edge walk, or an over-approximated metadata
resolution, as proof that an optional crate is compiled. No Cargo.lock entries
were deleted manually and no advisory ignores were added.

Required default-feature and all-feature checks remain the normal pre-commit
gate. No real keychain entries or credential identifiers were changed by this
review.

## Reassessment conditions

Recheck these warnings if a dependency enables `async-secret-service`, `tokio`,
`async-io`, or a persistent async keyring backend, if the CLI's supported targets
change, or when the keyring migration is undertaken. Continue showing all three
warnings in ordinary `cargo audit` output; this report is not a blanket safety
claim for zbus consumers or other feature combinations.
