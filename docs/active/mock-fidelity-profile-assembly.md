# Profile assembly batch

[English](mock-fidelity-profile-assembly.md) | [繁體中文](mock-fidelity-profile-assembly_zh.md)

Baseline: `0ae5b44`, 2026-09-11. W10/W16/W18/W19 subgroup, not whole-programme acceptance.

Counts and next-subgroup references below record this historical delivery. Current
candidate acceptance is tracked in the [execution checklist](mock-fidelity-execution-checklist.md).

| Section | Purpose |
| --- | --- |
| [Scope](#scope) | Source paths and reviewed decisions |
| [Acceptance](#acceptance) | Batch verification and remaining limits |
| [Evidence](#evidence) | Actual results |

## Scope

Extends the 13 source-operation cards in [profile preflight](mock-fidelity-profile-preflight.md).
Primary writers: Media1/Media2 CreateProfile, DeleteProfile; Media1 Add/RemoveVideoSourceConfiguration
and Add/RemoveVideoEncoderConfiguration; Media2 Add/RemoveConfiguration. Readers include both
profile views and the five modeled configuration catalogues; service capabilities declare their limit.
Source entry points are `src/mock/services/media.rs`, `media2.rs`, shared `ProfileEntry` state,
`effect.rs` and built-in replay. Public client/session signatures remain unchanged.

Direct review of Media1 v24.12 sections 5.2.1–5.2.5 and Media2 v26.06 sections 5.1.1–5.1.5
reconfirms initial binding, optional rename, All handling and capacity semantics. External source
notes remain outside the checkout. No official schema tables or derived fixtures are added here.

Implementation decisions: parse binding members from qualified direct children, preserving decoded
identity; validate the complete candidate before a single commit/notification. Ignore All on add/create,
clear modeled bindings on remove-All, and preserve unspecified Name. Conflicting assignments to one
modeled slot are refused instead of silently choosing the last value. Identical repeats are idempotent.
The existing five slots remain the supported model; unsupported types are explicitly refused rather
than adding required public fields. Correct the misleading Metadata capability to the actual PTZ slot.
Use the existing advertised profile limit (8) for creation; imported larger fixture lists remain readable
and deletable, but cannot create until below the limit. Do not truncate imported state.

Reference counts must follow committed profile references for affected catalogues. Extend committed
replay dependencies to any configuration reads whose counts change. Retain state and recordings on
refusal. Seeded arbitrary state and full video/audio compatibility rules are distinct from this mutation
contract; do not invent physical encoder or PTZ timing guarantees.

## Acceptance

C01–C05: two services/transports, scoped/escaped identities, repeats, omissions, decoys and malformed
binding entries. C06: initial multi-binding, rename-only, mixed rename/refusal rollback, All and
idempotent remove, capacity/collision/concurrent allocation, reference counts and single notification.
C07–C08: exact response fields and structured reviewed Faults, independent QName/shape controls.
C09–C11: retain shared auth/limits, cross-service reads, raw-response ownership, built-in replay and
existing client/session workflows. C12: focused old-code failures, one unfiltered batch mutation, restore,
one final five-gate cycle plus relevant external checks. Update both languages and migration docs.

Full nested configuration writer/renderer validation, all compatibility combinations, HTTP binding,
security freshness and unsupported configuration storage remain assigned to subsequent work, not
implicitly accepted here. No hardware writes, release, installation or main-branch merge.

## Evidence

The initial focused assembly run passed three tests. Expanded field/replay controls
and migrated atomicity/known-gap snapshots passed 14 tests across three suites.
One unfiltered workspace/all-feature/no-fail-fast mutation campaign
(`1789109637_cargo_test.log`) allowed one extra profile and incremented each touched
reference count incorrectly. Concurrent allocation assertions caught excess creation;
assembly, binding, identity, deletion and replay payload/state assertions caught
incorrect counts. The negative capacity helper also rejected the unexpected success.
Both mutations were restored exactly. The campaign additionally exposed old flat
Fault expectations in replay tests; these were migrated to exact structured payloads,
not classified as mutation sensitivity. No independent sensitivity is claimed for
every other assertion from this combined campaign.

Final Windows workspace gates passed with locked dependencies: formatting,
all-target Clippy with/default all features (`-D warnings`), 1,230 all-feature
tests and 1,131 default tests; each test run had five intentional ignores across
34 suites. Both strict rustdoc builds passed. Final review removed an unnecessary
encoder-instance replay dependency; the repeated affected gates include exact
recording-preservation assertions on both transports. No second mutation campaign
was run for that review correction.

The explicitly selected legacy schema check passed with unchanged finding pins.
The profile exporter at this checkpoint produced 40 instances; strict pinned Xerces
validated all 40 in external corpus `oxvif-profile-corpus-20260911-04`. These cover
the existing 13-operation corpus, not all new raw initial-binding, rename,
capacity or conflict variants. Inventory self-tests passed: 159 Action sites,
157 routes, 238 direct readers (223 before top-level test modules / 15 inside them); five legacy reader
calls were removed. This batch does not establish native Linux/macOS or release
acceptance. Next subgroup at that handoff: video source/encoder configuration and options.
