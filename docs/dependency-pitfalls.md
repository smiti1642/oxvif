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
