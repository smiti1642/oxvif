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

## Push integration follow-up, 2026-11-10

The user authorized auditing dependencies and pushing seven completed local
commits, with HB1 left uncommitted. The original local lockfile reported eleven
vulnerabilities. The remote had four intervening commits, ending at `4119c7a`,
including the existing 0.17.1 TLS/QUIC security release. Merging that history
preserves all seven original commit hashes and removes those vulnerability
entries. No new release, tag or credential backend migration is part of this work.

The fresh audit also reported Tokio 1.53.1's unsound safe `AsyncFd` constructors
([RUSTSEC-2026-0386](https://rustsec.org/advisories/RUSTSEC-2026-0386.html)).
`cargo update -p tokio --precise 1.53.3` applies the compatible patched version;
no repository code calls `AsyncFd`. Tokio is the only package version changed
relative to the merged lockfile; Cargo also normalizes existing Windows dependency
edges to already locked `windows-sys` versions. Feature definitions are unchanged,
and the [feature-unification review](../dependency-pitfalls.md#push-integration-2026-11-10-tokio-1531-to-1533)
found no new disappearing API used here. Downstream consumers must update their
own lockfiles; the declared dependency minimum is unchanged.

Final `cargo audit` exits successfully: **zero vulnerabilities and three
unmaintained warnings**, across 410 locked dependencies. The three warnings are
the entries above; none is ignored. Rechecked with `--all-features --target all`,
the inverted `zbus@4.4.0` tree still prints nothing. The Linux inverse
`dbus-secret-service` feature tree selects 4.1.0 via keyring's synchronous,
`crypto-rust` and `vendored` features. This is build-graph evidence rather than
native credential-store acceptance.

Validation in the isolated integration worktree, without HB1: format check;
workspace/all-target Clippy with and without all features (`-D warnings`);
locked, unfiltered, no-fail-fast workspace tests (1,417 all-feature and 1,285
default passes; seven ignored, 51 suites each); and strict workspace rustdoc in
both feature modes. Ignored external-resource/native-store tests remain ignored.
HB1's tracked diff and all twelve file contents were backed up before branch
integration; none of that implementation is staged in the integration worktree.

`cargo outdated --workspace --depth 1` also exits successfully. Its reviewed
direct updates remain deferred except for the Tokio safety patch:

| Package | Locked | Reported latest |
| --- | --- | --- |
| rand | 0.10.2 | 0.10.3 |
| serde_json | 1.0.151 | 1.0.154 |
| thiserror | 2.0.20 | 2.0.21 |
| tokio | 1.53.3 | 1.54.1 |
| tokio-rustls | 0.26.5 | 0.26.6 |
| toml | 1.1.6+spec-1.1.0 | 1.2.1+spec-1.1.0 |
| clap | 4.6.6 | 4.6.7 |
| clap_complete | 4.6.9 | 4.6.11 |
| jsonschema (test-only) | 0.56.0 | 0.61.1 |
| keyring | 3.6.3 | 4.2.0 |
| sha2 | 0.11.0 | 0.11.1 |
| unicode-width | 0.2.2 | 0.2.3 |
| zeroize | 1.9.0 | 1.9.1 |

The remaining minor/patch maintenance belongs to a separate reviewed update;
the final audit reports no vulnerability requiring those upgrades. Keyring's
major update removes the configured native/backend feature names, which explains
the outdated tool's obsolete-feature warnings for the candidate version. Keep
the existing credential integration until its separate migration acceptance is
complete. No blanket dependency update or advisory suppression was performed.
