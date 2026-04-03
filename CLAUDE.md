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

## Before every publish (additional checks)

```
cargo test --doc          # verify all doc examples compile and run
cargo doc --no-deps       # verify HTML docs generate cleanly (mirrors docs.rs)
cargo audit               # zero vulnerabilities required
cargo outdated --depth 1  # review; upgrade direct deps if significantly behind
```

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

## Adding a new ONVIF service — step-by-step SOP

### Implementation

1. Create `src/types/<service>.rs` with all response structs.
   - All `from_xml` / `vec_from_xml` that parse required fields → `Result<Self, OnvifError>`
   - Token attributes → `.ok_or_else(|| SoapError::missing("Elem/@token"))?`
   - `to_xml_body()` string fields → `xml_escape(&self.field)`
2. Add `mod <service>;` and `pub use <service>::*;` to `src/types/mod.rs`.
3. Add methods to `src/client.rs`:
   - Add new types to the `use crate::types::{ ... }` import list
   - All `&str` params interpolated into XML → `xml_escape(param)`
4. Re-export all new public types from `src/lib.rs`.

### Testing

5. Append tests to `src/tests/client_tests.rs`:
   - At least one positive test per method (fixture XML + assert fields)
   - At least one negative test per method (missing token or SOAP Fault)
   - For write methods: use `RecordingTransport` and assert `c.action` + `c.body`

### Mock server coverage

5a. Add a handler for every new ONVIF action in `examples/mock_server.rs` —
    including write/Set methods. This makes `mock_server` a full integration
    harness that runs without a real device.
    - Add the action URI to the `dispatch()` match block.
    - Add a `resp_<operation>()` function returning a plausible canned response.
    - Write methods that return `void` may share `resp_empty(prefix, tag)`.

### Quality gate (run before every commit)

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

All three must pass cleanly.

### Documentation

6. Update `README.md`:
   - Architecture diagram (top of file) if a new service is added
   - Add a new `## <Service> methods` section with method table and code example
   - Update the `Implemented ONVIF operations` status table (— → ✓)
   - Update test count (`N unit tests`)
   - Update installation version number
7. Update `examples/camera.rs`:
   - Add new command to the doc comment at the top
   - Add new arm to the `match` in `main()`
   - Add to `print_help()`
   - Add the async function implementing the example
   - Add relevant sections to `full_workflow()` (sections 17, 18, …)

### Version and release

8. Bump version in `Cargo.toml` (patch = bug fix, minor = new feature).
9. Add entry to `CHANGELOG.md` at the top.
10. Run `cargo publish --dry-run` — must succeed with no errors.
11. Run `cargo audit` — must return zero vulnerabilities.
12. Consider running `cargo outdated --depth 1` — if direct dependencies are
    significantly behind, upgrade before publishing so the crate ships with a
    green dependency health indicator on lib.rs / crates.io.
13. Commit, merge to `master`.
14. Tag the release commit: `git tag v<version>` (e.g. `git tag v0.4.1`).
    Tags appear in GitHub Desktop next to commits — useful for version-based debugging.
15. Push tags to GitHub: `git push origin --tags`.
16. `cargo publish`.

## Rust 2024 edition notes

- `gen` is a reserved keyword — do not use it as a variable or method name.
- Use `rand::random::<T>()` instead of `rng.gen::<T>()`.

## Publishing checklist

- [ ] `cargo fmt && cargo clippy --all-targets -- -D warnings` clean
- [ ] `cargo test` — all tests pass
- [ ] `cargo test --doc` — all doc examples pass
- [ ] `cargo doc --no-deps` — HTML docs generate without errors or broken links
- [ ] `cargo publish --dry-run` — no errors
- [ ] `cargo audit` — zero vulnerabilities
- [ ] `cargo outdated --depth 1` — review; upgrade direct deps if significantly behind
- [ ] `CHANGELOG.md` updated with new version entry
- [ ] `Cargo.toml` version bumped
- [ ] `README.md` installation version updated + content updated
- [ ] `examples/camera.rs` updated (new command + `full_workflow` sections)
- [ ] Committed and on `master` branch
- [ ] `git tag v<version>` — tag the release commit
- [ ] `git push origin --tags` — push tags to GitHub (visible in GitHub Desktop + useful for version debugging)
