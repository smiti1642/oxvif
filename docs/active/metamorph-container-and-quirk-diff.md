# metamorph — clone-into-a-container + quirk diff (plan)

> **Status: in progress.** Serve a recorded camera clone from a bound-port
> `MockServer` (the "container") so oxdm / Frigate / ODM can point at it, and add
> a structural **quirk diff** that shows where the clone's responses deviate from
> oxvif's synthetic (spec-ideal) mock. Supersedes the in-process approach in
> [`metamorph-clone-in-oxdm.md`](metamorph-clone-in-oxdm.md) — see §6.

---

## 1. Goal

1. **Container** — serve a cloned camera (`FixtureStore`) from a real bound-port
   `MockServer`, so any HTTP ONVIF client (oxdm, Frigate, ODM) can drive it. Reads
   replay the camera's real responses (quirks and all); writes / unrecorded ops
   fall to synthetic `DeviceState` with the existing coarse copy-on-write, so
   `Set → Get` still round-trips.
2. **Quirk diff** — compare the clone, per operation, against oxvif's synthetic
   mock (the "spec ideal") and report where the response *shape* deviates.

The two are deliberately separate: the diff needs only `metamorph` (runs
in-process, no server); the container needs a new `metamorph-server` feature.

## 2. Why the container path (not the in-process design)

The earlier note ([`metamorph-clone-in-oxdm.md`](metamorph-clone-in-oxdm.md) §2)
chose in-process consumption and listed gap **G2** (`HealthCheck::with_transport`).
Serving the clone from a bound port makes G2 unnecessary: oxdm just runs the
existing `HealthCheck::new(clone_url)` against the container, and "compare" can
reuse the existing `health::ReportDiff` between a synthetic server and the clone
server. The container is the smaller, more reusable seam.

## 3. Changes (this repo)

| File | Change |
|------|--------|
| `Cargo.toml` | Feature `metamorph-server = ["metamorph", "mock-server"]`; `[[example]] metamorph_serve` (`required-features = ["metamorph-server"]`). |
| `src/mock/server.rs` | `MockServerBuilder::replay(FixtureStore)` (cfg `metamorph`); `Ctx` carries a cfg-gated `ReplayHandle { store, invalidated }`; `handle_soap` splices `ReplayResponder` via `Chain::mock_with_extra` when replay is set, else `default_mock`. |
| `src/metamorph/quirk.rs` (new) | `QuirkReport` / `OperationQuirk` (+ serde); `FixtureStore::diff_against_synthetic()` — run each fixture's request through synthetic `dispatch`, diff the two responses' **element-path sets** (prefix-agnostic, via the canon parser). |
| `src/metamorph/fixture.rs` | `pub fn fixtures(&self) -> &[Fixture]` accessor. |
| `src/metamorph/mod.rs` | `mod quirk;` + `pub use quirk::{OperationQuirk, QuirkReport};` |
| `examples/metamorph_serve.rs` (new) | Load a fixtures dir → serve a bound replay `MockServer` → print `device_url` + a quirk summary. The oxdm-facing demo. |
| `docs/active/metamorph.md` | Progress note: bound-server replay + structural quirk-diff (part of M4 / start of M7 — not a completion claim). |
| `docs/active/metamorph-clone-in-oxdm.md` | Record the pivot to the container path; G2 obviated. |
| `CHANGELOG.md` `[Unreleased]` | metamorph-server + replay + quirk-diff entry. |

## 4. Honest scope limits (also documented in code)

- The quirk diff compares **structure only** (which element paths exist), not
  values — `Manufacturer="Hikvision"` vs `"oxvif-mock"` is expected noise, not a
  quirk. Value-level / type quirks are M7's deeper work, out of scope here.
- The synthetic mock as "spec ideal" is an approximation (design-note §9-Q1's
  "synthetic default" baseline).

## 5. Verification (per metamorph.md §8)

```
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

No existing public API changes; every new capability is feature-gated.

## 6. Relationship to the milestones

- Bound-server replay is the replay slice of **M4** (persona-over-port) without
  the full control plane — a targeted subset, not M4 done.
- The structural quirk diff is the first, structural half of **M7**; semantic /
  value diff (needs deeper work over the serde-derived types) stays M7.
