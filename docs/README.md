# oxvif `docs/`

Development-only documentation. **None of this is compiled into the crate** —
`docs/` is excluded from the published package (`exclude = ["docs/"]` in
`Cargo.toml`). oxvif's own public API docs live in the top-level
[`README.md`](../README.md) and on [docs.rs](https://docs.rs/oxvif).

## Layout

| Path | What it holds |
|------|---------------|
| [`reference/`](reference/) | **ONVIF protocol reference** — the WSDL/XSD transcriptions used while implementing oxvif. Stable facts, not plans. |
| [`active/`](active/) | **In-progress plans** — design docs / milestones for work that is under way or not yet finished. |
| [`done/`](done/) | **Completed plans** — finished design/audit docs, kept as a record. |
| [`dependency-pitfalls.md`](dependency-pitfalls.md) | Standing engineering guide (feature-unification footguns). Not a plan and not ONVIF reference, so it sits at the root; referenced from the release SOP in `CLAUDE.md`. |

A plan graduates from `active/` to `done/` when its milestones are all shipped.

## `reference/` — ONVIF protocol

A per-service catalogue of ONVIF operations transcribed from the official ONVIF
WSDLs, for cross-reference while implementing oxvif. See
[`reference/README.md`](reference/README.md) for the full service index,
conventions, and attribution/licensing.

## `active/` — in-progress plans

| Doc | About |
|-----|-------|
| [`metamorph.md`](active/metamorph.md) | The shape-shifting mock device (three personas). M0–M3, M5, M6 shipped; **M4** (control plane + Persona A) and **M7** (quirk diff) not yet started. |
| [`metamorph-clone-in-oxdm.md`](active/metamorph-clone-in-oxdm.md) | Draft design for wiring Persona B (clone/replay) into oxdm so a user can clone their own IP camera and hunt its quirks. |
| [`refactor-2026-07.md`](active/refactor-2026-07.md) | Staged bug-fix programme from the 2026-07 audit. Carries the locked release decision, the confirmed-defect list with citations, the per-stage **Critic protocol**, a correction log of wrong beliefs held during the work, and the evidence behind each stage verdict. Read §5 and §6 before reviewing any stage; §9 for what "done" already means. |

## `done/` — completed plans

| Doc | About |
|-----|-------|
| [`audit-2026-05.md`](done/audit-2026-05.md) | Implemented-operation fidelity audit (2026-05); all flagged items resolved by 0.9.8. |
| [`service-capabilities-and-ptz-tours.md`](done/service-capabilities-and-ptz-tours.md) | Shipped in 0.15.0. **Why and what.** `GetServiceCapabilities` on all nine services (it was implemented on **zero**), PTZ preset tours, PTZ-level `SendAuxiliaryCommand` — 17 operations. |
| [`tier1-implementation-map.md`](done/tier1-implementation-map.md) | Shipped in 0.15.0. **How.** The correspondence tables behind the plan above: every schema attribute → Rust field → mock fixture value → the test that pins it. Its §1 gate correction is now folded into `CLAUDE.md`; its §7 perturbation protocol is the record of what was actually proved. Worth re-reading before the next batch of parse-heavy types. |

---

Cross-references from source code, `Cargo.toml`, and `CLAUDE.md` point at the
current path of each doc. When you move a doc between buckets, update those
pointers too (grep the repo for the old path).
