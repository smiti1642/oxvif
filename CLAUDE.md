# oxvif — Development Guidelines

## Project overview

`oxvif` is an async Rust client library for the ONVIF IP camera protocol.
Library crate (no binary). Published on crates.io.

## Commit discipline

**Commit at the end of every completed piece of work — do not batch several
unrelated pieces into one commit.** A piece is complete when the quality gate
below passes and the change stands on its own; that is the moment to commit,
not "once everything is done".

Message format:

- **Subject line names what the piece was**, in the existing
  `type(scope): summary` form (`fix(client): …`, `test(ptz): …`, `docs: …`).
  One line, imperative, no trailing period.
- **Body is a bulleted list, and it must be detailed.** One bullet per
  substantive change — file or symbol touched, what changed, and why. Do not
  compress several changes into one bullet, and do not write a prose paragraph
  where a list belongs. A reader must be able to reconstruct the change set from
  the bullets alone, without opening the diff.
- Include what was *verified*, not just what was written: which gate ran, what
  the perturbation proved, what was deliberately left out.

Confirm the git identity is `smiti1642 <smiti1642@gmail.com>` before running
`git commit`.

## Before every commit

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

All three must pass cleanly before committing.

## Before every publish (additional checks)

```
cargo test --doc                      # verify all doc examples compile and run
cargo doc --no-deps --all-features    # what docs.rs actually builds
cargo doc --no-deps                   # the default-feature build; keep it warning-free too
cargo audit                           # zero vulnerabilities required
cargo outdated --depth 1              # review; upgrade direct deps if significantly behind
```

**Both `cargo doc` forms, and both must be warning-free.** `[package.metadata.docs.rs]`
sets `all-features = true`, so the plain `cargo doc --no-deps` is *not* what
docs.rs renders — it is the no-feature build, and this crate has no default
features, so it omits `mock`, `health` and `metamorph` entirely. An intra-doc
link from the crate header to a feature-gated item resolves under
`--all-features` and warns under the plain form; prefer a plain `` `code span` ``
in the crate header for anything behind a feature.

Also re-read the rendered front page after publishing. `all-features` was
added in 0.14 precisely because an explicit feature list had silently omitted
`metamorph` — docs.rs rendered ten modules instead of eleven for two releases
and nobody noticed, because nothing in the local build fails when a module is
merely absent.

After `cargo outdated`, if any direct dependency was updated, re-check for
feature-unification footguns (a public API a sibling crate can flip off via
`#[cfg(not(feature = …))]`). See [docs/dependency-pitfalls.md](docs/dependency-pitfalls.md)
for the audit steps and the `quick-xml/encoding` case that motivated this.

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

### Multi-sensor devices

A dual/quad-lens camera is one ONVIF device with **several video sources**, and
almost every per-channel answer depends on which one you asked about. Both
failure modes below are silent: the call succeeds, the data is wrong, and no
test fails.

**Never omit the token on a per-channel query.** Every `Get…Options` /
`Get…Configuration` that accepts a `ConfigurationToken` or `ProfileToken` must
be given one. A device answering a token-less request is not obliged to say so
— it answers for its *default* channel, and on a single-sensor camera the
result is indistinguishable from correct.

Measured on a real two-sensor device (2026-07-28): a token-less
`GetVideoEncoderConfigurationOptions` returned lens 0's list
(`2592x1944 … 1280x720`) — the same list every caller would then show for lens
1, whose real maximum is `1280x720`. Passing `ConfigurationToken` alone was
enough to get all four (lens, stream) lists right; the profile token was not
required. So the rule is **send a token**, not "send the profile token".

**A single-sensor fixture cannot cover a per-channel feature.** Any test for
one of these calls needs at least two channels with *deliberately different*
answers, or it passes just as well against a parser that ignores the token
entirely. Prefer a fixture where the two channels disagree on a value the
assertion reads.

When touching one of these calls, list the affected operations and check each:
Media1/Media2 `Get*ConfigurationOptions`, `GetProfiles` → per-profile config
tokens, `GetVideoSources`, Imaging (every method is per-`VideoSourceToken`),
and PTZ (per-profile).

### Data nested in `Extension` levels

ONVIF extends types by nesting a same-named element one level deeper rather
than adding a field, so the deeper copy is a **superset** and the shallow copy
is what an older device sends. `XmlNode::child` returns the *first direct*
child, so a parser reading only the top level silently drops whatever the
extension added.

The case that motivated this rule — Media1 video encoder options:

```text
Options/JPEG, Options/H264                       no BitrateRange
Options/Extension/JPEG, Options/Extension/H264   adds BitrateRange
Options/Extension/Extension/H265                 the ONLY place H265 lives
```

Devices commonly send **both** copies. Prefer the deepest node and fall back
outward. Before writing a parser for a type with an `Extension` member, check
the schema for what the extension adds — and give the mock the nested shape, not
the flat one. A mock that sends a shape no conformant device produces is how
this defect survived: mock and unit fixture agreed with each other and with
nothing else.

## Testing rules

- Every new client method needs at least one **positive test** (happy path)
  and one **negative test** (missing required field or SOAP Fault). Both must
  assert the payload — see [No hollow tests](#no-hollow-tests); a test that
  only asserts "an error happened" does not satisfy this rule.
- Client tests are split by service, mirroring `src/client/`:
  `src/tests/client/<service>_tests.rs` (`device`, `media`, `media2`, `ptz`,
  `imaging`, `events`, `recording`). Each file is attached to the module it
  exercises by a `#[cfg(test)] #[path = "../tests/client/<service>_tests.rs"]
  mod tests;` declaration at the foot of `src/client/<service>.rs`.
- Put a fixture next to the tests that use it. Only promote it to
  `src/tests/common.rs` when more than one service needs it.
- `src/tests/common.rs` holds the shared transports (`MockTransport` + the
  `mock()` builder, `RecordingTransport` + `Captured`, `ErrorTransport`) and the
  cross-service fixtures `empty_response_xml` / `make_soap_fault_xml`. It is
  declared once from `src/lib.rs` as `#[cfg(test)] mod tests;` (via
  `src/tests/mod.rs`) and pulled in with `use crate::tests::common::*;`.
- Use `MockTransport` for happy-path tests and `ErrorTransport` for HTTP
  error tests.
- Negative SOAP Fault tests: use `make_soap_fault_xml(code, reason)`.
- Black-box tests that only touch the public API (plus `oxvif::mock`) belong in
  the integration directory `tests/`, not inside the library crate — see
  `tests/mock_action_snapshot.rs` and `tests/mock_workflow.rs`.

### No hollow tests

A test that cannot fail for the reason it was written is not coverage. It is
worse than no test, because it reports the method as covered.

**Banned in a negative test** — every one of these passes when the device
returns a *completely different* error:

```rust
// WRONG — all four are hollow
assert!(res.is_err());
assert!(matches!(res, Err(_)));
assert!(matches!(err, OnvifError::Soap(_)));
assert!(matches!(err, OnvifError::Soap(SoapError::Fault { .. })));
```

This is measured, not theoretical: change one letter in the response tag so a
`Fault` becomes `UnexpectedResponse`, and all four stay green.

**Required instead — assert the payload:**

```rust
// CORRECT — fault: the exact code and reason the fixture sent
let err = client.delete_recording(url, "bad").await.unwrap_err();
assert_fault(err, "env:Receiver", "NoSuchRecording-delete-8821");

// CORRECT — missing field: the exact path string the parser emits
let err = client.get_replay_uri(url, "r").await.unwrap_err();
assert_missing_field(err, "Uri");
```

Use the `#[track_caller]` helpers at the top of
`src/tests/client/recording_tests.rs`; copy them into a second service's file
only until a third needs them, then promote to `src/tests/common.rs`.

Rules that make the assertion load-bearing:

- **Assert what the fixture chose, not what the enum is.** `code`, `reason`,
  the field-path string, the message text. If the assertion would still hold
  after you edit the fixture, it is asserting nothing.
- **Give each fixture a distinctive payload.** `make_soap_fault_xml("env:Sender",
  "InvalidToken")` repeated across ten tests means no test can tell which
  operation faulted. Use `"InvalidJobMode-3160"`-style strings.
- **`code` and `reason` must vary independently** across a file, or asserting
  both proves no more than asserting one.
- **Positives are subject to the same rule.** `res.unwrap();` or
  `assert!(res.is_ok());` with no field assertion is a hollow positive. Assert
  returned fields, or for write methods assert `c.action` **and** `c.body` from
  `RecordingTransport`.

**Prove it before you commit.** A new assertion that passes on the first run has
proved nothing yet:

1. Perturb *that test's* fixture — change the fault code, the reason, or omit a
   different element so a different path is reported.
2. Run that one test. It must fail on the **assertion**, not on a compile error.
3. Revert exactly, confirm green.

For a whole batch, mutate the library instead and diff the failing test **names**
before and after: make `SoapError::missing()` ignore its argument, or make the
fault parser in `src/soap/xml.rs` return a constant `code`/`reason`. Every real
negative goes red; every hollow one stays green. Run it unfiltered — a
`cargo test <filter>` run silently excludes the integration crates.

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

5. Append tests to `src/tests/client/<service>_tests.rs` — the file that
   mirrors the `src/client/<service>.rs` you added the methods to. For a
   brand-new service, create the test file and attach it from the bottom of
   `src/client/<service>.rs`:
   ```rust
   #[cfg(test)]
   #[path = "../tests/client/<service>_tests.rs"]
   mod tests;
   ```
   - At least one positive test per method (fixture XML + assert fields)
   - At least one negative test per method (missing token or SOAP Fault)
   - For write methods: use `RecordingTransport` and assert `c.action` + `c.body`

### Mock server coverage

5a. Add a handler for every new ONVIF action under `src/mock/` (the mock
    engine moved into the library in 0.9.6; `examples/mock_server/` is now
    a thin wrapper over `oxvif::mock::MockServer`). Including write/Set
    methods. This keeps both `MockTransport` and `MockServer` as full
    integration harnesses that run without a real device.
    - Add the action URI to the match block in `src/mock/dispatch.rs`.
    - Add a `resp_<operation>()` function in the right
      `src/mock/services/<service>.rs` returning a plausible response
      (or mutating `DeviceState` for write methods).
    - Write methods that return `void` may share the empty-body helper
      from `src/mock/helpers.rs`.
    - The behind-the-scenes example binary needs no change — it auto-picks
      up new handlers because they live in the library now.

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
6a. **Update the crate-level docs in `src/lib.rs`** — the `//!` header. This is
    what docs.rs renders as the crate's front page, and it is the first thing
    most readers see; the README is the *second*. It does not follow the README
    on its own and has silently fallen two releases behind before. Check every
    one of these:
    - `## Optional features` — one bullet per feature in `Cargo.toml`. A new
      feature with no bullet here is invisible: it appears in the auto-generated
      Modules list with no explanation of what to turn on or why. This is
      exactly how `metamorph` shipped undocumented on the front page for two
      releases while being the headline of both.
    - A prose section for any feature big enough to need one (see `## Metamorph`).
    - `## Supported services` and the Profile coverage table — percentages and
      the "not yet implemented" notes both go stale.
    - Version numbers in the example `Cargo.toml` snippets.
    - Any behaviour statement about a type whose behaviour changed this release.
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
    If any direct dep was updated, re-audit for feature-unification footguns
    (see [docs/dependency-pitfalls.md](docs/dependency-pitfalls.md)).
13. Commit, merge to `master`.
14. Tag the release commit: `git tag v<version>` (e.g. `git tag v0.4.1`).
    Tags appear in GitHub Desktop next to commits — useful for version-based debugging.
15. Push tags to GitHub: `git push origin --tags`.
16. Create a GitHub release (notes = this version's CHANGELOG section):
    ```sh
    gh release create v<version> --title "v<version>" \
      --notes "$(awk '/^## \[<version>\]/{found=1;next} found && /^## \[/{exit} found{print}' CHANGELOG.md)"
    ```
    e.g. for v0.8.0:
    ```sh
    gh release create v0.8.0 --title "v0.8.0" \
      --notes "$(awk '/^## \[0\.8\.0\]/{found=1;next} found && /^## \[/{exit} found{print}' CHANGELOG.md)"
    ```
17. `cargo publish`.

## Rust 2024 edition notes

- `gen` is a reserved keyword — do not use it as a variable or method name.
- Use `rand::random::<T>()` instead of `rng.gen::<T>()`.

## Publishing checklist

- [ ] `cargo fmt && cargo clippy --all-targets -- -D warnings` clean
- [ ] `cargo test` — all tests pass
- [ ] `cargo test --doc` — all doc examples pass
- [ ] `cargo doc --no-deps --all-features` — what docs.rs builds; no warnings
- [ ] `cargo doc --no-deps` — the default-feature build; no warnings either
- [ ] `cargo publish --dry-run` — no errors
- [ ] `cargo audit` — zero vulnerabilities
- [ ] `cargo outdated --depth 1` — review; upgrade direct deps if significantly behind
- [ ] If a direct dep was updated: re-audit for feature-unification footguns ([docs/dependency-pitfalls.md](docs/dependency-pitfalls.md))
- [ ] `CHANGELOG.md` updated with new version entry
- [ ] `Cargo.toml` version bumped
- [ ] `README.md` installation version updated + content updated
- [ ] **`src/lib.rs` crate-level `//!` docs updated** — the docs.rs front page.
      Every feature in `Cargo.toml` has an `## Optional features` bullet; new
      capabilities have a prose section; version numbers in the example
      snippets are current
- [ ] `examples/camera.rs` updated (new command + `full_workflow` sections)
- [ ] Committed and on `master` branch
- [ ] `git tag v<version>` — tag the release commit
- [ ] `git push origin --tags` — push tags to GitHub (visible in GitHub Desktop + useful for version debugging)
- [ ] `gh release create v<version> --title "v<version>" --notes "$(awk ...)"` — GitHub release with changelog notes

---

## Behavioral guidelines

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

### 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

### 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.
