# Mock acknowledgment-only policy preflight

[English](mock-fidelity-ack-policy-preflight.md) | [繁體中文](mock-fidelity-ack-policy-preflight_zh.md)

W16/K05, 2026-09-11, baseline `a51490c`. Design recorded before implementation.
Current implementation: 11 classified operations. Scope/Design/Verification below
retain the first three-operation checkpoint; the A3 section records the eight-operation extension.

| Section | Purpose |
| --- | --- |
| [Scope](#scope) | First classified operations and remaining work |
| [Design](#design) | Explicit, local policy and compatibility boundaries |
| [Verification](#verification) | Refusal, opt-in and precedence controls |
| [Remaining effect-stub batch](#remaining-effect-stub-batch) | Eight additional policy classifications delivered together |

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

## Remaining effect-stub batch

Baseline `6382458`, 2026-09-11; recorded before implementation. Apply approved D2
to the following complete source-confirmed stub subgroup in one delivery. This
is a policy migration, not full operation-field or normative Fault acceptance.

| Ledger ID | Current handler/input | Current response and absent effect |
| --- | --- | --- |
| `device.SendAuxiliaryCommand` | `device::resp_send_auxiliary_command`, ignores request fields | `OK`; no auxiliary execution |
| `ptz.SendAuxiliaryCommand` | `ptz::handle_ptz_send_auxiliary_command`, legacy AuxiliaryData allowlist; ignores ProfileToken | Accepted text or legacy refusal; no auxiliary execution |
| `device.SystemReboot` | `device::resp_system_reboot`, no request reader | Reboot message; no restart |
| `device.StartFirmwareUpgrade` | `device::resp_start_firmware_upgrade`, base URL only | Upload URI/durations; no upload or upgrade |
| `device.StartSystemRestore` | `device::resp_start_system_restore`, base URL only | Upload URI/duration; no restore |
| `events.SubscribeRequest` | `events::resp_subscribe`, base URL only | Reference/current timestamps; no push subscription or delivery |
| `events.RenewRequest` | `events::resp_renew`, no request reader | Current timestamps; no lifetime extension |
| `search.EndSearch` | `recording::resp_end_search`, no request reader | Current timestamp; no search termination |

All eight handlers currently avoid state writes; their existing client/session
and dispatch identities remain unchanged. Add distinct enum variants/full Actions
to the existing policy, before handlers; default refusal has the existing static
`mock:UnmodeledEffect` shape. Opt-in retains the existing response projection and
PTZ allowlist, except the reboot message must explicitly state no reboot occurred.
No new upload endpoints, subscriptions, timers or search sessions are introduced.
Common parser checks still precede policy; raw adapters still own their responses.
Replay must not retire data on either receipt or refusal. No public required
struct fields, device snapshots, error types or CLI exits change.

C01/C07/C08/C10/C11: exercise exact selection, wrong service/body, response fields,
ordinary refusal and existing typed workflow. C06/C09: non-default state, zero
hooks/invalidation and existing shared auth/limit controls. C12: one planned batch
mutation plus final gates, not one full gate per operation. C02–C05 full field,
token, enum/extension and lifecycle validation remain unaccepted; these policy
changes do not upgrade legacy request readers. Normative payload mappings are
unchanged and are not redefined from project fixtures. Preserve all previous
successful external-schema probes by adding explicit opt-in alongside defaults.
Update operation ledger, affected public docs and source claims as one batch.

### A3 implementation evidence

The extended default-refusal controls failed against `6382458` for all eight
previously successful operations over both transports (`1789107385_cargo_test.log`).
The focused policy/workflow/action-snapshot run passed 34 tests. One unfiltered
workspace/all-feature/no-fail-fast mutation campaign then bypassed reboot policy,
changed the auxiliary receipt and altered Renew replay handling
(`1789107591_cargo_test.log`). Default/isolation/snapshot assertions caught the
reboot bypass; the typed auxiliary workflow caught the changed receipt. Replay
tests stopped at the earlier reboot assertion, so this combined run does not
independently prove sensitivity to the Renew-invalidation mutation. All mutations
were restored. The final expanded replay test checks all eleven operations leave
the invalidation set unchanged.

That full campaign also found two existing HTTP maintenance tests still assuming
default success; they were migrated to explicit per-operation opt-in and exact
URI/duration assertions. Their focused rerun passed. This was fixture migration,
not a reason to weaken default refusal. No actual upload, restart, push delivery
or search termination was performed. Prior commit `6382458` passed CI `34566149186`.

Final restored Windows workspace gates passed: all features 1,224 passed and
default features 1,127 passed, each with 5 conditional ignores across 33 suites.
Formatting, both all-target Clippy modes with warnings denied, strict docs in both
feature modes, paired inventory self-tests and `git diff --check` passed. The
inventory remains 159 Action sites / 157 routes / 243 direct readers.

The separately selected legacy external-schema check passed with 169 responses:
111 success payloads, 58 Faults, 1,319 anchored nodes, 1,464 skipped children and
409 checked attributes; all ten finding categories remain zero. Extra per-operation
opt-ins preserve the previous success coverage alongside default refusals. No
schema resources, finding pins or coverage floors changed. This is not independent
Xerces acceptance of the eight operations, full field/lifecycle validation or a
real-device effect test. A3 completes this eight-stub policy migration, bringing
the classified total to eleven; W16 remains PARTIAL and W17 capability reconciliation
remains open. No release, system binary update or branch merge is included.
