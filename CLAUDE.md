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

## Every change updates what claims to describe it

**A change is not finished when the code is right. It is finished when every
statement about it is still true.** Two surfaces rot silently, and neither is
covered by any of the five gate lines — nothing in this repo asserts prose.

### The CHANGELOG entry for the release you are in

**Keep the entry to what a reader deciding whether to upgrade needs**: what
changed, what broke, how to migrate. The forensics — schema citations, measured
counts, the perturbation that verified each fix — go in
`docs/releases/<version>.md`, which the entry links to. Measured on 0.15.0: the
entry reached 2,006 lines, 57% of the whole changelog and **seven times the
previous largest release**, because it tried to be both. Splitting it lost
nothing and took it to ~700.

The split also has a failure mode worth naming: `docs/` is excluded from the
published package (`Cargo.toml`), so the link out **must be an absolute GitHub
URL**, and it must point at a branch rather than the release tag if the file
was added after the tag.

Not "add a bullet" — **re-read the bullets already there**. Within one release,
later work routinely falsifies what earlier work wrote.

Measured on 0.15.0 (2026-08-03): an eight-way audit checked the entry's 66
top-level claims against source, with `v0.14.0` as the baseline, and found **26
wrong statements**. Not one was invented — every one was true when written. The
shape of the failure is the lesson:

- **Counts that later work moved** — `47 Set → Get pairs` (49), `26 rows` (34),
  `six nested types` (8), `resp_empty went 22 → 13` (24 → 4), `config_token is
  now required on four options getters` in a bullet that then listed five.
- **Contradicted by a later bullet in the same entry** — `Type="PTZ"` listed
  among the types `AddConfiguration` refuses, three hundred lines above the
  bullet announcing that it binds; "the four seeded heads" against a later
  "two-head PTZ device".
- **One that would actively mislead** — a migration snippet keying
  `PtzState.channels` by *profile* token after the map had been re-keyed by
  *node* token. Following it seeds an entry no read path ever reaches, silently.

The two property tables already print *"update this expectation **and** the
counts in `docs/mock-server.md` §12 and `docs/active/mock-audit-2026-07.md` §2"*
when their pin fails. Both of those files were correct. **Only `CHANGELOG.md`
was wrong — because it was the one place the message did not name.** So: when a
pinned number is quoted anywhere, name *every* file that quotes it in the pin's
failure message.

### The doc comments docs.rs actually renders

`cargo doc` builds clean whether or not the prose is true. And the part that is
easy to miss: **a `pub(crate)` item's doc comment renders nowhere at all.**
Every `from_xml` / `to_xml_body` in `src/types/` is `pub(crate)`, so reasoning
written next to a parser fix is invisible to every reader of the crate. Measured
the same day: the explanations for the Media1/Media2 options nesting, for
Media1's required `Multicast`/`SessionTimeout`, and for why
`PtzConfiguration::to_xml_body` returns `Result` were all on private items,
while the public methods a caller reaches said nothing had changed.

After changing behaviour, check in this order:

1. **The public method or type the caller reaches.** Does its `///` still
   describe what it does? Does it need an `# Errors` section it did not need
   before? `ptz_set_configuration` gained a way to fail before sending anything
   and said so only on a private function.
2. **`src/lib.rs`'s `//!` header** — see [step 6a](#documentation).
3. **Any `///` on a private helper that states a *fact*** — a count, a schema
   shape, "X is a static fixture". These rot the same way and nobody re-reads
   them. `ConfigKind::known_token` said *"the audio families are static
   fixtures, so there is nothing to validate a binding against"* for two commits
   after they gained catalogues — and it was still returning `true` for any
   audio token, so the stale comment was also a live bug. **A justification that
   outlives its premise is where to look for the next defect.**

When a change adds intra-doc links, run
`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` and the plain
form. The publish checklist's two `cargo doc` lines do not set the flag, so a
broken link is a warning nothing fails on.

## Before every commit

```
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --all-targets -- -D warnings
cargo test --all-features
cargo test
```

All five must pass cleanly before committing.

**The second clippy line — no features — is not redundant.** A warning that
exists *only* without features is invisible to every other line: the
`--all-features` clippy is a different compilation, and neither `cargo test`
line carries `-D warnings`, so both compile a warning and pass. Measured: an
unused `use std::sync::Arc;` in `src/tests/client/ptz_tests.rs` whose only
consumer was `#[cfg(feature = "mock")]` went through this gate clean and shipped
in 0.15.0 (fixed in `8031ab0`). It was found by an editor, not by the gate.

**`--all-features` is not optional on the first two.** This crate has **no
default features**, and `src/mock/` is behind `#[cfg(feature = "mock")]`
(`src/lib.rs`). Without the flag, `cargo test` collects only the non-mock
subset — measured at `1d224f4`: 461 tests versus 698 — and `clippy
--all-targets` lints only that same subset, so a warning inside `src/mock/`,
`src/health/` or `src/metamorph/` fails nothing. Two commits' worth of mock
tests had been invisible to this gate before it was measured.

Keep the plain `cargo test` as well, as the last line: a no-feature build
breaking is its own bug, and it has happened — a `[[example]]` missing its
`required-features` entry made a bare `cargo test` fail to compile, so *no*
test ran at all and the gate reported nothing wrong.

## Before every publish (additional checks)

```
cargo test --doc                      # verify all doc examples compile and run
cargo doc --no-deps --all-features    # what docs.rs actually builds
cargo doc --no-deps                   # the default-feature build; keep it warning-free too
cargo audit                           # zero vulnerabilities required
cargo outdated --depth 1              # review; upgrade direct deps if significantly behind
```

### The per-feature warning sweep

```sh
for fs in "" health mock mock-server metamorph metamorph-server serde; do
  echo -n "$fs: "
  cargo clippy --all-targets ${fs:+--features $fs} --message-format short 2>&1 \
    | grep -cE '^(src|examples|tests).*warning'
done
```

**All must print 0.** The two per-commit clippy lines cover exactly two of the
sixty-four feature combinations this crate can be built with, and a warning that
lives only in one of the other sixty-two is invisible to the whole gate. That is
not hypothetical — **three instances so far, all the same shape**: an item used
only under a *narrower* feature than the one gating its module.

| found in | symptom | fix |
|---|---|---|
| `8031ab0` | `use std::sync::Arc` in `ptz_tests.rs`, sole consumer `#[cfg(feature = "mock")]` | gate the import |
| 0.15.0 | `redact::scrub_url_userinfo` dead under `--features health` (module is `mock` **or** `health`; only recorders use it) | `#[cfg(feature = "mock")]` on the fn + its test |
| 0.15.0 | `use crate::metamorph::SurfaceOp` in `record.rs` tests, sole consumer `#[cfg(feature = "mock-server")]` | gate the import |

The tell is a module gated on `any(feature = A, feature = B)`, or a test module
whose only user of an import is itself gated. Too slow for every commit (~8
clippy runs); right before publish is the place.

### The schema-shape check

```sh
OXVIF_ONVIF_SCHEMA=/path/to/onvif/schema \
  cargo test --features mock --test mock_schema_shape -- --ignored --nocapture
```

**This is the only thing that runs it.** `tests/mock_schema_shape.rs` is
`#[ignore]`d and reads the ONVIF schema set at run time from a directory
*outside* the working tree, because nothing derived from that schema may enter
this repository (`docs/active/schema-shape-plan-2026-08.md` §4, decision D2 —
which covers a hardcoded schema fact in a test file just as much as the `.xsd`
itself). So it cannot join the five gate lines: a fresh clone has nothing to
read. **That makes it weaker than a gate line, and it is worth saying so.**

Why it exists: the mock writes XML as hand-built strings, and `XmlNode` is
namespace-stripped, so oxvif's own parser is namespace-blind and
order-independent. A mock response with every element in the wrong namespace
and the wrong order parses identically — **no other test here can see the
class.** Six instances were found before the checker existed, every one by a
human reading a schema file. **This sentence used to end there, and used to say
"one of them was a client bug".** As of the 0.15.0 sweep it is five client bugs
out of the set, and the two the checker found on its own are the ones worth
knowing about:

| client bug | how it surfaced |
|---|---|
| Media2 `Audio` → `AudioEncoder` | by hand, `8091892` |
| `GetDigitalInputs` sent to device management | the checker's **unanchored-root** line, not a pin |
| `set_storage_configuration` request body in `tt:` | asking why a guard that should have fired stayed silent |
| `ImagingMoveOptions` reading `PositionSpace`/`SpeedSpace` | fixing the mock, which then disagreed with the client |
| `VideoEncoder2Configuration` `GovLength`/`Profile` read as elements | reusing a renderer, which surfaced the row at a second path |
| `MetadataConfigurationOptions::analytics_supported` | a name declared nowhere in the schema set — the fix was to delete the field |
| `VideoEncoderOptions2` list attributes read as repeated elements | `xs:list` attributes: a **cardinality** error, not a location one |
| `SystemUris::system_log_uri` walking `SystemLogUri` | the *type* name read as though it were the *element* name |
| `VideoEncoderInstances::encodings` walking `Encoding` | two levels sharing a name, and `XmlNode` matches local names |
| `SecurityCapabilities`: one undeclared member, eight declared ones unread | comparing the struct against the type member-by-member |

**Ten, and nine of them in one sweep.** The counts in this section were "six
instances, one client bug" when the checker landed and "five client bugs" a day
later; both were true when written. If you are about to quote a number from this
file, re-derive it.

Six of the ten were found *because a mock fix made the client disagree with
it*. That is the argument for fixing the mock even though no caller sees the
mock: **a conformant mock turns a silent client bug into a visible
disagreement** — but only if some test actually drives the client through the
mock and asserts a value, which is why `tests/mock_workflow.rs` gained
`imaging_move_options_ranges_survive_the_round_trip` and
`media2_encoder_gov_length_and_profile_are_attributes`. A hollow positive there
(`let _ = client.foo().await.unwrap();`) breaks the whole chain.

Two ways to read the result:

- **It printed `SKIPPED`.** Then nothing was checked. That is the failure mode
  this whole arrangement has, and the message says so rather than passing
  quietly.
- **A pin moved.** `PINS` holds the distinct finding count *per kind*, not a
  total — measured: putting the Media2 defect back leaves the total at 63 while
  moving two kinds, so a single total would have let it through. Lower is a
  fix; update the pin in the same commit and also
  `docs/active/mock-schema-conformance-2026-08.md` §1, which quotes the same
  numbers. Never edit a pin to make a run green.

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
negative goes red; every hollow one stays green. Run it **unfiltered, with
`--all-features`, and with `--no-fail-fast`** — a `cargo test <filter>` run
silently excludes the integration crates, and a no-feature run silently excludes
every mock test.

**`--no-fail-fast` is not a nicety, and this line said only "unfiltered and with
`--all-features`" until it was measured.** Cargo stops after the first target
that fails, and a batch mutation is *designed* to make targets fail — so the
`src/lib.rs` unit target reddens and cargo never runs the nine integration
crates at all. Measured on `987cd0c`, mutating one field of
`VideoEncoderConfiguration2::from_xml`:

```
cargo test --all-features                 1 of 10 targets ran, 3 tests red
cargo test --all-features --no-fail-fast  10 targets ran,     4 tests red
```

The fourth was `media2_encoder_gov_length_and_profile_are_attributes` in
`tests/mock_workflow.rs` — the guard that mutation exists to validate. Without
the flag the batch mutation reports a *smaller* red set than the truth, which is
the one direction of error this technique cannot survive: a hollow test and an
unreached test look identical in the diff of failing names.

Two batch mutations worth keeping in the rotation beyond those two, because
they catch a class the missing/fault pair cannot:

- Make an `Option<bool>` parse helper return `Some(false)` where it returns
  `None`. Every "the device did not say" assertion must go red. Nothing else
  proves that distinction is observable in the tests rather than merely written
  in the types.
- Make a list-valued parse helper return empty unconditionally. Every
  `Vec<String>` assertion must go red.

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

    **This step is now enforced.**
    `mock_handles_every_action_the_client_can_send` in `src/mock/dispatch.rs`
    reads every action URI out of `src/client/*.rs` with `include_str!` and
    asserts none falls through to the `Not implemented` fault. So a new client
    method with no dispatch arm fails the gate — you do not have to remember
    this step, only satisfy it. It asserts *routing*, not payload: give the
    handler a plausible response as well, because nothing checks that for you.

5b. **If the operation exists on both Media1 and Media2, they share one state.**
    Media1 and Media2 are two views of one device, so an operation present in
    both dispatchers must read and write the same `DeviceState` — otherwise the
    mock reports contradictory facts about itself, with no error and nothing
    failing.

    Two instances shipped before this was written down, both found from
    outside: Media2's whole profile family was a string literal while Media1's
    was state-driven (a harness seeding 20 profiles got 20 from one service and
    4 from the other), and `SetVideoEncoderConfiguration` wrote state on Media2
    while Media1 answered `resp_empty`.

    The audit is mechanical — for every operation name in both `dispatch_media`
    and `dispatch_media2`, check whether both arms take `state`:

    ```sh
    python -c "
    import io,re
    s=io.open('src/mock/dispatch.rs',encoding='utf-8').read()
    def arms(fn):
        m=re.search(r'fn '+fn+r'\(.*?\n\}\n', s, re.S)
        return {op:h for op,h in re.findall(r'\"([A-Za-z]+)\" => ([^,]+),', m.group(0))}
    m1,m2=arms('dispatch_media'),arms('dispatch_media2')
    for op in sorted(set(m1)&set(m2)):
        a,b=('state' in m1[op]),('state' in m2[op])
        if a!=b: print('DIVERGENT:',op)
    "
    ```

    Both must be state-driven or both static — a *consistent* stub is fine
    (audio is static on both sides), a divergent one is the bug. Put the state
    operation in `services/media.rs` and let each service render its own
    envelope: the shapes genuinely differ — Media1 lists a profile's
    configurations as siblings of `Name`, Media2 groups them under one
    `<tr2:Configurations>`, the member *names* differ (`VideoSource` against
    `VideoSourceConfiguration`), the two sequences are separate declarations,
    and two of the *types* differ (`VideoEncoder2Configuration`,
    `AudioEncoder2Configuration`) — and
    `tr2:DeleteProfile` names its token element `Token` where
    `trt:DeleteProfile` says `ProfileToken`, so a shared handler reads the
    wrong element. `tests/mock_media1_media2_agree.rs` is the standing guard.

    **What is *not* a difference is the nesting.** `tr2:ConfigurationSet` types
    every member as the *full* configuration — three of the five are the very
    types `tt:Profile` inlines — so both services inline, and each member's body
    comes from the helper the corresponding list getter uses. Two renderers over
    one state, not two bodies for one configuration.

    **This paragraph used to say "Media2 emits token references".** It was
    wrong, and it was wrong in a way that hid a shipped client defect for two
    releases; believing it made the whole element-naming question look settled
    when it was not — the audio member is `AudioEncoder`, the parser read
    `Audio`, and the fixture and the mock had both been written to agree with
    the parser. **Check a shape claim against the WSDL before writing it down
    here**; a design note in this file is read as settled fact by everything
    downstream of it.

    **It then said the mock's token-only rendering was "a documented
    simplification", and that outlived its premise too.** A simplification is
    defensible while nothing depends on the omission, and something did:
    `MediaProfile2::video_source_token` is read from a `SourceToken` *inside*
    the video source configuration, so it was permanently `None` against the
    mock — a field that existed, was parsed, and could not be exercised. The
    mock inlines as of 0.15. **When a shape claim here is downgraded from "the
    schema says" to "we chose", re-ask what the choice costs** — the second
    sentence is where the first one's defect goes to hide.

5c. **Every `Set` needs a row in `tests/mock_roundtrip.rs`.** The table pairs
    each write with the getter that should show it, and each row declares
    `Expect::Works`, `Expect::Broken(audit §)` or `Expect::Static(audit §)`.

    This exists because **nothing else distinguishes "deliberately static" from
    "not wired up yet"** — not the type system, not the dispatch table, not the
    tests. `resp_profiles_media2()` (a bug) and `resp_audio_sources()` (a fine
    stub) had the same signature and sat in the same match block; five instances
    of that class were reported from *outside* the project before the table
    existed. Declaring the intent is the whole point, so a row is not optional
    and `Broken` is a legitimate answer — what is not legitimate is no row.

    All three arms are asserted, not skipped: wire a `Broken` row up and the
    test goes red telling you to move it. That is what stops the list rotting
    into the permanent blind spot an xfail list usually becomes.

    Two things the table taught that the SOP did not say before:

    - **A partial write is worse than no write.** `SetNetworkInterfaces` wrote
      `Enabled`, `FromDHCP`, `Address` and `PrefixLength` and silently dropped
      `MTU`, which the client sends and the getter reports. The state log said
      `[STATE] interface updated`, the dispatch arm took `state`, and `grep` for
      `resp_empty` never named it. **Only writing a value and reading it back
      finds this.** Found by this table on its first run, after a hand audit
      with three probe axes had missed it.
    - **A field you cannot store must be named in a comment.** `Bounds/@x` and
      `@y` have no home in `VideoSourceConfigEntry`; saying so where the write
      happens is what keeps a real gap from reading like the `MTU` bug. §6 of the
      audit states the general rule: a documented omission is a design decision,
      an undocumented one is a bug.

    **And every token-taking operation needs a row in
    `tests/mock_token_discrimination.rs`** — same contract, `Discriminates` or
    `Blind(audit §)`, both arms asserted. It catches what the round-trip table
    cannot: a handler that persists state perfectly and still answers for the
    wrong channel. Its rows name two tokens the fixture *disagrees on*, which is
    the executable form of the multi-sensor rule above.

    When a write has **no getter that could ever show it** — `SetVideoSourceMode`
    (`VideoSourceMode` has no active-mode field), `SetRelayOutputState`
    (`GetRelayOutputs` never returns the live state) — there is no pair to add.
    Prefer faulting over reporting success on such an operation, so the caller
    learns the mock does not model it instead of being told nothing happened.

### Quality gate (run before every commit)

```
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --all-targets -- -D warnings
cargo test --all-features
cargo test
```

All five must pass cleanly. See [Before every commit](#before-every-commit) for
why `--all-features` is load-bearing — without it the mock tests you just added
in step 5a are not collected — and why the no-feature clippy line is not a
duplicate of the first.

### Documentation

6. Update `README.md`:
   - Architecture diagram (top of file) if a new service is added
   - Add a new `## <Service> methods` section with method table and code example
   - Update test count (`N unit tests`)
   - Update installation version number
   - **The `Implemented ONVIF operations` coverage tables are not in this file.**
     They moved to [`OPERATIONS.md`](OPERATIONS.md); `README.md` only links to
     it. Add the new operation there (— → ✓). A method table updated in the
     README but not in `OPERATIONS.md` leaves the crate's only coverage
     statement wrong, and nothing fails.
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

- [ ] `cargo fmt && cargo clippy --all-targets --all-features -- -D warnings` clean
- [ ] `cargo clippy --all-targets -- -D warnings` — the no-feature lint pass too
- [ ] **Per-feature warning sweep** — every single feature at 0 warnings; see
      [The per-feature warning sweep](#the-per-feature-warning-sweep). Three
      warnings have shipped through the two-combination gate already.
- [ ] `cargo test --all-features` — all tests pass
- [ ] `cargo test` — the no-feature build compiles and passes too
- [ ] `cargo test --doc` — all doc examples pass
- [ ] **Schema-shape check** — `OXVIF_ONVIF_SCHEMA=… cargo test --features mock
      --test mock_schema_shape -- --ignored --nocapture`; see
      [The schema-shape check](#the-schema-shape-check). It is `#[ignore]`d and
      reads the schema from outside the tree, so **this line is the only thing
      that runs it**. If it printed `SKIPPED`, nothing was checked — that is not
      a pass.
- [ ] `cargo doc --no-deps --all-features` — what docs.rs builds; no warnings
- [ ] `cargo doc --no-deps` — the default-feature build; no warnings either
- [ ] `cargo publish --dry-run` — no errors
- [ ] `cargo audit` — zero vulnerabilities
- [ ] `cargo outdated --depth 1` — review; upgrade direct deps if significantly behind
- [ ] If a direct dep was updated: re-audit for feature-unification footguns ([docs/dependency-pitfalls.md](docs/dependency-pitfalls.md))
- [ ] `CHANGELOG.md` updated with new version entry
- [ ] `Cargo.toml` version bumped
- [ ] `README.md` installation version updated + content updated
- [ ] `OPERATIONS.md` coverage tables updated if any operation was added
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
