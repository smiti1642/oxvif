# Mock acknowledgment-only policy preflight

[English](mock-fidelity-ack-policy-preflight.md) | [繁體中文](mock-fidelity-ack-policy-preflight_zh.md)

W16/K05, 2026-09-11, baseline `a51490c`. Design recorded before implementation.

| Section | Purpose |
| --- | --- |
| [Scope](#scope) | First classified operations and remaining work |
| [Design](#design) | Explicit, local policy and compatibility boundaries |
| [Verification](#verification) | Refusal, opt-in and precedence controls |

## Scope

First classify three direct empty-success routes: Device SetSystemFactoryDefault,
Events Unsubscribe and Events SetSynchronizationPoint. They do not reset device
state, terminate subscriptions or enqueue synchronization events. Under approved
D2 they must refuse by default. This is project fidelity policy, not a claim that
the ONVIF operation is unsupported by real devices. Other auxiliary, reboot,
upgrade/restore, subscription and search lifecycle stubs remain explicitly open;
this first registry is not an acceptance of all 157 routes.

## Design

Add a non-exhaustive public `AckOnlyOperation` enum whose variants map to exact
full Action URIs, and an additive `with_acknowledgment_only(operation)` builder
method on mock, replay and adapter transports and the HTTP mock builder. Repeated
calls accumulate selected operations only; clones copy policy while retaining
their existing shared device state. No global legacy switch or arbitrary suffix.

Apply policy at the synthetic terminal after bounded XML/Action/body identity
validation, before dispatch/effects. Default refusal is a structured Receiver /
`mock:UnmodeledEffect` Fault with a static reason. Opt-in retains only the empty
response shape; neither path changes state, invokes change hooks, emits an effect
nor retires replay fixtures. Fault/auth and caller-owned raw adapter precedence
remain unchanged. Built-in replay passes these operations without pre-write
invalidation; an acknowledgment is not a committed device mutation.

Do not add required public struct fields or change existing signatures. Policy is
private transport/builder configuration, not persisted DeviceState. This slice
does not validate every operation field, model subscription identity/lifecycle,
add receipt tracing, perform real-device writes or integrate PR #16. That PR's
Media operations require their own variants/contracts; Events sync is distinct.

## Verification

Prove all three default refusals against old code. Both transports must assert
exact Fault code/subcode/reason, complete unchanged state and zero change hooks.
Opt-in each operation independently; other two still refuse, repeated builder
calls accumulate, and cloning does not retroactively enable other clones.
Assert qualified empty response payloads, ordinary reads, unsupported/mismatched
Action controls and fault/auth precedence. Replay and adapter fallback exercise
the same policy; custom raw responses remain caller-owned. Update the existing
public action snapshot honestly rather than opting every test into legacy success.
Run an unfiltered workspace/all-feature/no-fail-fast mutation, restore, then all
five gates, strict docs and inventory. Existing profile XSD corpus does not prove
these operation contracts; external validation coverage must be stated separately.

Implementation evidence: all three former successes were observed before the fix
in both transports (`1789103471_cargo_test.log`). A full workspace/all-feature/
no-fail-fast mutation incorrectly allowed every operation after enabling one and
restored pre-write replay invalidation; the two per-selection transport tests and
replay invalidation assertion failed (`1789104095_cargo_test.log`). Mutations were
restored. Fixtures now include a non-default hostname so an accidental factory
reset cannot hide behind a default-state comparison. HTTP tests cover both plain
and replay-enabled builders, independently selected and accumulated options.

The separately selected legacy external-schema run passes with 161 responses:
111 success payloads, 50 Faults, 1,263 anchored nodes, 1,440 skipped children and
401 checked attributes; all ten finding categories remain zero. Three opt-in
success instances were retained alongside the new default Faults. No schema
resource, coverage floor or finding pin was loosened. This shape check is not
independent Xerces acceptance of these operations or evidence of real effects.

Local Windows gates passed: workspace all-feature 1,224 tests and default 1,127
tests, both with 5 conditional ignores across 33 suites; both Clippy modes,
formatting, strict default/all-feature docs and paired inventory self-tests.
Inventory remains 159 Action sites / 157 routes / 243 direct readers. Authentication
predecessor `a51490c` passed CI `34564803484`. This slice is not release acceptance;
W16 remains PARTIAL until the remaining effectful routes are classified.
