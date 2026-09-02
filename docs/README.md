# oxvif `docs/`

Project documentation that complements the crate API reference. Public guides
linked by the project README ship with the library package; development plans,
protocol transcriptions, and release records remain repository-only. Start
with the top-level [`README.md`](../README.md), the
[complete library guide](../LIBRARY_GUIDE.md), the
[complete CLI guide](oxvif-cli.md), the coverage tables in
[`OPERATIONS.md`](../OPERATIONS.md), or the library API on
[docs.rs](https://docs.rs/oxvif).

## Layout

| Path | What it holds |
|------|---------------|
| [`../README_zh.md`](../README_zh.md) | **專案首頁（繁體中文）** — the Traditional Chinese counterpart to the concise project README. |
| [`../LIBRARY_GUIDE.md`](../LIBRARY_GUIDE.md) | **Complete library and feature guide** — detailed session/client usage, service APIs, health checks, testing, Mock, and Metamorph, moved out of the concise project README. |
| [`reference/`](reference/) | **ONVIF protocol reference** — the WSDL/XSD transcriptions used while implementing oxvif. Stable facts, not plans. |
| [`active/`](active/) | **In-progress plans** — design docs / milestones for work that is under way or not yet finished. |
| [`done/`](done/) | **Completed plans** — finished design/audit docs, kept as a record. |
| [`mock-server.md`](mock-server.md) | **Reference for `oxvif::mock`** — the outward-facing one. Routing, envelope/namespace contract, the full state model, the seeded fixture, all 157 operations with which are state-backed and which are static, worked request/response examples, the fault catalogue, and an explicit list of what the mock does *not* model. Read this before driving the mock from anything, Rust or not. |
| [`oxvif-cli.md`](oxvif-cli.md) | **Complete CLI guide** — installation, human and Agent workflows, inventory, discovery, fleet execution, credentials, TLS, structured output, typed errors, and exit codes. |
| [`oxvif-cli_zh.md`](oxvif-cli_zh.md) | **CLI 使用指南（繁體中文）** — the Traditional Chinese companion to the complete CLI guide. |
| [`dependency-pitfalls.md`](dependency-pitfalls.md) | Standing engineering guide (feature-unification footguns). Not a plan and not ONVIF reference, so it sits at the root; referenced from the release SOP in `CLAUDE.md`. |
| [`support.md`](support.md) | Versioned support boundaries for the Rust library and CLI beta, including OS, credential, schema, TLS, camera-evidence, and commercial-claim limits. |
| [`releases/`](releases/) | Version-specific release notes and verification evidence. Entries marked unreleased describe staging, not public availability. |

A plan graduates from `active/` to `done/` when its milestones are all shipped.

## `reference/` — ONVIF protocol

A per-service catalogue of ONVIF operations transcribed from the official ONVIF
WSDLs, for cross-reference while implementing oxvif. See
[`reference/README.md`](reference/README.md) for the full service index,
conventions, and attribution/licensing.

## `active/` — in-progress plans

| Doc | About |
|-----|-------|
| [`oxvif-cli-release-hardening-plan.md`](active/oxvif-cli-release-hardening-plan.md) | Release-hardening programme for the diagnostic CLI: dependency security, publishable packages, CI/platform gates, retry and observability contracts, human rendering, Agent schemas, community readiness, and the commercial-pilot cut line before controlled device writes. |
| [`oxvif-cli-three-platform-distribution-plan.md`](active/oxvif-cli-three-platform-distribution-plan.md) | Executable plan for native Windows/macOS/Linux credentials, private CA and timeout gates, signed APT distribution, Homebrew tap/bottles, the approval-controlled release, and later Homebrew Core/Debian/Ubuntu graduation. |
| [`metamorph.md`](active/metamorph.md) | The shape-shifting mock device (three personas). M0–M3, M5, M6 shipped; **M4** (control plane + Persona A) and **M7** (quirk diff) not yet started. |
| [`metamorph-clone-in-oxdm.md`](active/metamorph-clone-in-oxdm.md) | Draft design for wiring Persona B (clone/replay) into oxdm so a user can clone their own IP camera and hunt its quirks. |
| [`mock-audit-2026-07.md`](active/mock-audit-2026-07.md) | **Measured** audit of the mock's state/get/set architecture, after an external report found Media2's profile family ignoring `DeviceState`. Every finding is probe-backed. **Tiers 1 and 2 are closed** (16 defects, all fixed, each behind a standing property test). **Tier 3 and Tier 4 are being closed now** — 0.15.0 waits for them (decision 2026-07-31); Storage is done, Audio / PTZ configurations / Media2 metadata remain, each still pinned by a `Static` or `Blind` row. Read §8 before touching `src/mock/` — it names the one structural cause, and the two property tables are the answer to it. |
| [`ptz-wiring-plan-2026-07.md`](active/ptz-wiring-plan-2026-07.md) | **Plan for the PTZ family** — the last Tier 3 item but one, and the last Tier 4 item. Carries the decisions taken, the ONVIF schema facts behind them (including the `DefaultAbsolutePantTiltPositionSpace` spelling defect in oxvif's *client*, which is a real-device bug and ships first), the target state model and fixture, an exhaustive list of the tests whose premise inverts, and the table/pin deltas. Read §2 before touching `src/mock/services/ptz.rs`. |
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
