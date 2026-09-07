# Dependency pitfalls — feature-unification footguns

A running log of *weird* dependency breakages that pass our own CI but blow up
in a downstream user's workspace. The common thread: **Cargo feature
unification only ever *adds* features (it takes the union across the whole
build graph), so a sibling crate we never named can flip a dependency's
feature on for us.** The dangerous shape is a **public API gated
`#[cfg(not(feature = X))]`** — it *disappears* the moment any crate enables
`X`, and we get a compile error we can't reproduce in isolation.

This file is dev-only (the `docs/` directory is excluded from the published
crate). Linked from `CLAUDE.md` → checked before every publish.

| Section | Purpose |
| --- | --- |
| [Historical encoding failure](#case-1--quick-xml-encoding-removes-attributeunescape_value) | Why downstream feature tests exist |
| [Audit procedure](#how-to-audit-for-new-instances-run-before-each-publish) | Review steps |
| [Audit log](#audit-log) | Version-specific findings |
| [Current XML and digest migration](#unreleased--quick-xml-042-and-sha2-011) | Current APIs and compatibility gates |

---

## Case 1 — `quick-xml` `encoding` removes `Attribute::unescape_value`

**Fixed in:** 0.9.9 · **File:** `src/soap/xml.rs`

### Symptom

`oxvif` compiled fine on its own and in CI, but a downstream Tauri project that
also depended on `calamine` failed to build:

```
error[E0599]: no method named `unescape_value` found for struct `Attribute<'a>`
   --> .../oxvif-0.9.8/src/soap/xml.rs:217
help: there is a method `decode_and_unescape_value` with a similar name
```

### Root cause

`quick-xml` 0.39 gates `Attribute::unescape_value` with
`#[cfg(any(doc, not(feature = "encoding")))]`. `calamine` enables
`quick-xml/encoding`; feature unification turns it on for *our* `quick-xml`
too, so the method we called vanished. Confirmed with:

```
cargo tree -e features -i quick-xml   # shows the `encoding` feature edge
```

### Fix

This is the historical 0.39 fix. For 0.42, use the string-based API described
in the [current migration](#unreleased--quick-xml-042-and-sha2-011); `Reader::decoder`
no longer exists.

Go through the always-available decoder variant instead:

```rust
// before — disappears under the `encoding` feature
let value = attr.unescape_value()?;
// after — present with `encoding` on or off; input is always UTF-8 `&str`
let decoder = reader.decoder();              // capture once before the loop
let value = attr.decode_and_unescape_value(decoder)?;
```

> The upstream issue suggested `quick_xml::Decoder::utf8()` — **that does not
> compile**, the constructor is `pub(crate)`. Use the reader's own
> `decoder()`, which is public.

### Regression guard

`Cargo.toml` dev-dependencies pin `quick-xml = { features = ["encoding"] }`, so
`cargo test` always compiles the library with `encoding` unified on — exactly
as a downstream crate would. A future call into an `encoding`-gated API now
fails our own test build instead of only a user's workspace.

---

## How to audit for new instances (run before each publish)

Do this **after `cargo outdated`**, because the risk is introduced when a
dependency *updates* and adds a new feature or newly gates an existing public
API behind `not(feature = …)`.

1. List the crates `oxvif` calls into directly (anything `use`d from
   `src/`), and for each, scan its source for **public** items gated on
   `not(feature)`:

   ```sh
   # in a dependency's source dir
   grep -rn -A2 'cfg(not(feature' src/ | grep -B1 -E 'pub (fn|struct|enum|trait|use|mod|const) '
   ```

   A hit means "this public item disappears when that feature is enabled."
   Cross-check whether `oxvif` actually calls it. (As of 0.9.9, `quick-xml`
   was the only real instance; `serde`/`serde_json`/`tracing` hits are
   `no_std` / macro-internal plumbing we don't touch.)

2. If a *new* feature appeared on a direct dependency since the last release
   (visible in the `cargo outdated` / changelog review), check whether
   enabling it would gate away anything we use.

3. When in doubt, reproduce the way a downstream crate sees us: a scratch crate
   that depends on `oxvif` **and** force-enables the suspect feature
   (`dep = { features = ["…"] }`), then `cargo build`.

---

## Audit log

Record the *outcome* here, not just that an audit happened — the next audit is
much cheaper when it can start from "these were clear last time, and why".

### 0.15.0 — `base64` 0.22 → 0.23

The only direct dependency behind by a major; everything else moved within
semver via `cargo update`.

**No Case-1 footgun.** base64 0.23 has **zero** public items gated on
`not(feature = …)` — the audit grep above returns nothing for it. The only hits
across every crate `oxvif` calls into were `tracing`'s `__disabled_span`, which
is the macro-internal plumbing this file already dismisses. **No code change was
needed:** the `Engine` trait plus `engine::general_purpose::STANDARD` API that
`src/soap/security.rs` and `src/mock/auth.rs` use is unchanged.

**A different shape worth naming: a new default-on feature called
`simd-unsafe`.** 0.23 added runtime-dispatched AVX2/NEON engines behind it, on by
default. Not a footgun — it gates nothing away — but oxvif base64s a 16-byte
nonce and a 20-byte SHA-1 digest on the WS-Security path, so SIMD buys nothing
measurable. `Cargo.toml` therefore takes base64 as
`{ default-features = false, features = ["std"] }`.

Verified, because "we declined it" is a claim about the build graph:

```sh
cargo tree -e features -i base64@0.23.0 --all-features   # alloc + std only
```

**And verified that declining it is not load-bearing.** Feature unification only
ever adds, so a downstream crate that takes base64 with default features turns
`simd-unsafe` back on for us. Reproduced per step 3 — a scratch crate depending
on `oxvif` plus `base64 = "0.23"`:

```
├── base64 feature "default"
│   ├── base64 feature "simd-unsafe"     <-- re-enabled by the sibling
```

…and it compiles and runs fine. So this is a **default we choose, not a guarantee
we make**, and the distinction is why declining it is safe rather than fragile.
Do not write it up in the README as "no unsafe in the dependency tree".

One thing to watch: **two** transitive dependencies still pull **base64 0.22** —
`hyper-util` and `reqwest` — a separate major that does not unify with ours, so
both are in the lock file. When *both* move to 0.23 the two collapse into one
and that sibling's default features will apply; the reproduction above is what
says that is a non-event.

Also in 0.23, and checked: `DecodeError::InvalidLastSymbol` now carries the
decoded value, so its `Display` text changed. `src/mock/auth.rs` only formats the
error (`"Invalid nonce base64: {e}"`) and nothing matches on the variant or
asserts the message, so nothing depends on it. MSRV moved to 1.71; ours is 1.85.

`cargo audit`: zero vulnerabilities, 245 crate dependencies.

### Unreleased — quick-xml 0.42 and sha2 0.11

quick-xml 0.42 events now hold UTF-8 strings. `XmlNode` and the optional schema
check use string local names, reference/CDATA access, and
`Attribute::normalized_value(XmlVersion::Implicit1_0)`. Unlike the old API in
Case 1, this method is unconditional in 0.42; `encoding` does not remove it.
Text still uses XML 1.0 line-ending normalization, and CDATA remains raw.
Namespace declarations are omitted, element text is trimmed only after joining
split events, unknown text entities are preserved, and the existing permissive
partial-document/invalid-attribute handling is unchanged. This migration does
not turn the DOM into a validating XML parser.

`tests/xml_compat.rs` was run against 0.41 before migration. It pins Unicode,
entities, CDATA, line endings, attributes, namespace stripping, and representative
malformed inputs. Normal test builds enable `encoding` through the dev-dependency.
`python packaging/check_xml_features.py` additionally runs these fixtures from
an isolated consumer with `encoding` off and on, verifies the actual feature
graph, and rejects registry versions absent from the workspace lockfile. Run
`cargo fetch --locked` first; the probe resolves offline and does not modify the
workspace lockfile. CI runs the probe as part of the Clippy job.

sha2 0.11 returns a digest array without the `LowerHex` implementation used by
the CLI. The shared fingerprint encoder now writes two lowercase hex characters
per byte, preserving `sha256:` plus exactly 64 characters. Regression constants
were captured using sha2 0.10.9 for a synthetic discovery record, its reviewed
import plan, and input `286` (whose digest starts with `00`). They are not values
calculated by the replacement encoder inside the assertions. Existing stale-plan,
atomicity, and idempotency tests remain in place. No registry or CLI schema
migration is required. Transitive sha2 0.10 remains where credential dependencies
require it; forcing a single version is outside this update.

The accompanying base64 0.23.1, ipnet 2.12.1, and async-trait 0.1.92 changes are
targeted lockfile updates. They do not change oxvif's base64 feature policy.
The 2026-09-07 audit found no known vulnerabilities across 411 dependencies.
`cargo outdated` still reports newer packages, including keyring 4.2; these are
not silently included in the six reviewed updates. They belong to later reviewed
maintenance batches, particularly the platform credential migration for keyring.
