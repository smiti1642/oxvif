## What changed

Describe the observable library, CLI human-output, or Agent-contract change.

## Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- [ ] `cargo test --workspace --all-features --locked`
- [ ] Rust 1.88 MSRV check passes when dependencies or public API changed
- [ ] Documentation and changelog match the implemented behavior
- [ ] Structured stdout remains log/prompt/color free
- [ ] Fixtures and logs are sanitized; no credentials, URI userinfo, private endpoints, serials, UUIDs, or customer/site identifiers

## Compatibility and risk

State schema/exit-code/help changes, device mutation risk, tested platforms, and
real-camera evidence separately. Use `Not applicable` rather than leaving a
risk surface ambiguous.

