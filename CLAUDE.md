# oxvif — Development Guidelines

## Project overview

`oxvif` is an async Rust client library for the ONVIF IP camera protocol.
Library crate (no binary). Published on crates.io.

## Before every commit

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

All three must pass cleanly before committing.

## Coding rules

### Required fields must return `Result`

Every `from_xml` / `vec_from_xml` function that parses a required field
(especially `token` attributes) must return `Err` on missing input — never
silently default to an empty string.

```rust
// WRONG
token: node.attr("token").unwrap_or("").to_string()

// CORRECT
let token = node
    .attr("token")
    .filter(|t| !t.is_empty())
    .ok_or_else(|| SoapError::missing("Foo/@token"))?
    .to_string();
```

### XML escaping

All user-supplied strings or device-echoed strings interpolated into XML
bodies must be wrapped in `xml_escape()` (defined in `src/types/mod.rs`).

```rust
// WRONG
format!("<tt:Name>{name}</tt:Name>")

// CORRECT
format!("<tt:Name>{}</tt:Name>", xml_escape(name))
```

This applies to:
- `format!()` calls in `client.rs` that embed `&str` parameters
- `to_xml_body()` methods in `src/types/*.rs`

### No `unwrap()` in library code

Library code must not panic on malformed device responses.
Use `?`, `if let`, or `.ok_or_else()` instead of `.unwrap()`.

Test code may use `.unwrap()` / `.expect()` where appropriate.

### No panics in `vec_from_xml` closures

When using `.map(|node| ...)` to parse a collection, the closure must return
`Result<T, OnvifError>` and the final `.collect()` will propagate the first
error. Do not use `Ok(iter.map(|n| Self { ... }).collect())` when any field
can fail.

```rust
// WRONG — silently skips errors
Ok(resp.children_named("Foo").map(|n| Self { ... }).collect())

// CORRECT — propagates first error
resp.children_named("Foo").map(|n| {
    let token = ...?;
    Ok(Self { token, ... })
}).collect()
```

## Testing rules

- Every new client method needs at least one **positive test** (happy path)
  and one **negative test** (missing required field or SOAP Fault).
- Fixtures go in `src/tests/client_tests.rs`.
- Use `MockTransport` for happy-path tests and `ErrorTransport` for HTTP
  error tests.
- Negative SOAP Fault tests: use `make_soap_fault_xml(code, reason)`.

## Adding a new ONVIF service

1. Create `src/types/<service>.rs` with all response structs.
2. All `from_xml` returning structs with required fields → `Result<Self, OnvifError>`.
3. Add `mod <service>;` and `pub use <service>::*;` to `src/types/mod.rs`.
4. Add methods to `src/client.rs` following existing patterns.
5. Re-export new public types from `src/lib.rs`.
6. Add tests to `src/tests/client_tests.rs`.
7. Update `CHANGELOG.md` and bump version in `Cargo.toml`.
8. Update version in `README.md` installation section.

## Rust 2024 edition notes

- `gen` is a reserved keyword — do not use it as a variable or method name.
- Use `rand::random::<T>()` instead of `rng.gen::<T>()`.

## Publishing checklist

- [ ] `cargo fmt && cargo clippy --all-targets -- -D warnings` clean
- [ ] `cargo test` — all tests pass
- [ ] `cargo publish --dry-run` — no errors
- [ ] `CHANGELOG.md` updated
- [ ] `Cargo.toml` version bumped
- [ ] `README.md` installation version updated
- [ ] Committed and on `master` branch
