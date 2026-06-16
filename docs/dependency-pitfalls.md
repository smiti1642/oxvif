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
