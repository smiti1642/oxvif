# Work after the 0.17 release cut

[English](post-0.17-backlog.md) | [繁體中文](post-0.17-backlog_zh.md)

Status: PARTIAL — active follow-up after 0.17.0 publication; reconciled 2026-09-23.
No implementation or release date is promised. The completed
[release cut](../done/release-0.17-cut.md#blocking-acceptance) and
[publication evidence](../done/release-0.17-finalization.md) are archived.
Their closure does not complete the broader work below or turn unperformed checks into passes.

| Section | Purpose |
| --- | --- |
| [Calibrated work ownership](#calibrated-work-ownership) | Current disposition, owner and exit criteria |
| [Closure review](#closure-review-2026-09-23) | Completed batches and plans that still remain active |
| [Deferred batches](#deferred-batches) | Remaining scope and original IDs |
| [Re-entry rule](#re-entry-rule) | When a finding becomes a current blocker |
| [Preserved tracking](#preserved-tracking) | Do not lose technical work cards |

## Calibrated work ownership

Calibration date: 2026-09-23; reviewed through `2d430d8`. This is a work inventory,
not a commitment to put every item into 0.18. All 16 previously active document
families were checked; dependency integration/grouping was archived, leaving
15 active families at calibration; G3 closure below leaves 14. Several are registers or preflights for the same Mock programme.

Use four dispositions: **implementation** (confirmed missing scope),
**evidence needed** (implementation exists but the named acceptance is not recorded),
**decision/deferred** (proposal awaiting a bounded product decision), and
**maintained reference** (inventory or supporting evidence, not another feature).
`TODO` in W12–W15 means the planned hardening audit is missing; it does not mean
those services have no existing handlers. A route count is not a bug count.

| ID | Current disposition / single owner | Next work and completion condition |
| --- | --- | --- |
| F01 | Implementation: W10, [execution checklist](mock-fidelity-execution-checklist.md); profile preflight is supporting evidence | OS1 modeled OSD CRUD is DONE; select remaining URI/source-mode/binding operations or explicitly expand OSD support; complete field/Fault/state/replay cards and both-transport/independent-instance checks. Do not redo PA1/VS1/VE1/AM1/OS1. |
| F02 | Implementation: W11/W12, same checklist | RS1 GetNode/GetConfiguration selectors are DONE. Finish remaining PTZ spaces/effects and per-source Imaging/focus contracts. Verify two distinct heads/sources, limits and refusal without mutation; no physical motion guarantee. |
| F03 | Implementation: W13/W14, same checklist | Audit Device/DeviceIO and Recording/Search/Replay state/lifetimes in bounded service groups; include cascades, timeout/termination and state-after-error. Existing handlers are the baseline, not absent functionality. |
| F04 | Implementation: W15, same checklist | EP2 delivers bounded per-subscription lifecycle. Remaining: push delivery, property synchronization, broader topic grammar and normative WSNT Fault detail; immediate synthetic pacing remains explicit. |
| F05 | Implementation: W07–W09; [pipeline](mock-fidelity-pipeline-preflight.md) | Notification connection/deadline/framing limits are delivered below. Remaining work is broader mock HTTP binding, freshness/replay, roles and fault injection; do not reimplement delivered scoped auth. |
| F06 | Implementation: W16–W19; pipeline owns dependency detail | Choose remaining capability/mutation/read pairs; test atomic commits, refusal preservation and replay visibility. K27 storage retention is DONE; future key-format redesign is a distinct decision. |
| F07 | Implementation/evidence: W20–W23; [schema preflight](mock-fidelity-schema-preflight.md) | RS1/EP1/OS1/EP2 expanded the selected corpus to 216 instances/59 operations, independently validated locally. Expand remaining coverage while preserving explicit anchors, pinned external resources, negative controls and bounded fuzz/property seeds. The 0.17 selected CI gate is separate historical evidence. |
| F08 | Decision/deferred: [CLI roadmap](oxvif-cli-plan.md), [navigation](cli-vim-navigation-plan.md) owns M6 | Separate decoder/playback, batch export, controlled writes and crate extraction. Select a bounded deliverable and its interface/permission/recovery tests before implementation. None is automatically a 0.18 blocker. |
| F09 | Evidence/operations/deferred submission: [distribution](oxvif-cli-three-platform-distribution-plan.md) | Read-only channel inventory is DONE and identified missing public ownership/URL/key inputs. Obtain those inputs, then verify isolated install/upgrade/downgrade/removal. 0.17 artifacts/staging are DONE; official admission needs its own evidence. |
| F10 | Evidence needed: snapshot investigation, [repair record](../done/snapshot-auth-repair.md) | Obtain a sanitized reproducible non-image/JPEG response; test bounded format handling without weakening destination/auth/TLS/size/no-clobber policies. Trailing CR/LF is a known synthetic difference, not a proven Hanwha root cause. |
| F11 | Evidence needed: [Fleet](mock-fleet-basic-plan.md); broader discovery features are deferred | Record authorized OS/interface/VMS four-device identity, endpoint, isolation, shutdown/restart checks. Full scopes/Hello/Bye/Resolve, mixed personas and sustained load require separate implementation/acceptance. |
| F12 | Evidence needed: navigation plan | Record native terminal and human IME composition/cancellation/restoration; Windows resize evidence exists. CI and key injection do not replace the missing platform/input-method checks. |
| F13 | Maintenance: [review record](../dependency-pitfalls.md#post-017--reviewed-maintenance-batch-2026-09-22) | Reviewed dependency updates and the rustls advisory fix are integrated and verified. Windows keyring 3.6.3 ↔ 4.2.0 native compatibility probe is DONE; other OS and locked/denied/unavailable/recovery cases remain. Production stays on 3.6.3; publication is separate. Group reuse/no-duplicate verification is DONE. |
| F14 | Decision/implementation: [Metamorph](metamorph.md), [closed clone note](../done/metamorph-clone-in-oxdm.md) records G3 | G1/G3 are DONE and G2 is SUPERSEDED. G3 adds offline summary counts and standing regressions (unreleased). Remaining work is M4 persona/control transitions or M7 reference-value comparisons and their tests. |
| F15 | Implementation/evidence: [CLI hardening](oxvif-cli-release-hardening-plan.md) | R2 typed retry policy, R3 health details and R4 six-outcome envelope matrix are DONE. Remaining work includes observability/clock-skew/durability, native-terminal acceptance and descriptor reachability/executable examples. R5 multi-vendor/soak/signing/support and recovery remain open; first package/publication is DONE. |

The Mock main plan owns policy; the execution checklist owns W statuses; operation
ledger/source audit are maintained references; profile/pipeline/schema preflights
own supporting cards and dependency evidence. Do not create parallel work items
for the same requirement in all seven documents. W26 is DONE for integration;
W24/W25 remain PARTIAL for the full programme, with the 0.17 subset delivered.

Calibration checks read source/tests and the recorded 0.17 finalization; no new
camera, native-terminal, schema-resource or release validation was run. At the
calibration commit `c35704f`, workspace gates passed: 1,327 all-feature / 1,215
default tests, seven ignores each (41 suites per mode), both all-target Clippy
configurations and formatting. The inventory checker and its rejection controls
also passed. These do not expand the recorded hardware or external-schema scope.
Fresh F13 dependency inspection and local integration evidence are recorded in
[dependency pitfalls](../dependency-pitfalls.md#post-017--reviewed-maintenance-batch-2026-09-22).
Remote checks describe the named PR revision, not these later local changes.

F09 read-only channel inventory is complete: the proposed tap returns 404 with
current credentials; the repository contains temporary APT staging, not a named
production URL/key. Public ownership, signing/recovery policy and native lifecycle
acceptance remain open in [distribution D](oxvif-cli-three-platform-distribution-plan.md#read-only-channel-inventory-2026-09-22).

## Closure review (2026-09-23)

The selected local work from the previous pass is **DONE**, with commits and
bounded verification in [the acceptance register](remaining-plan-acceptance.md).
That register remains active because it also owns outstanding acceptance inputs;
it is supporting material, not a fifteenth plan family.

The completed dependency-maintenance plan, G3 clone integration, RS1 read-selector
batch and EP1 event-pull batch are already in `docs/done/` with explicit DONE
status. No additional active plan meets its whole-plan exit criteria in this
review, so no partially completed plan is moved or marked wholly complete.
Fleet still needs LAN/VMS evidence; navigation needs native terminal/IME evidence;
Mock hardening, CLI runtime/descriptors, distribution and Metamorph M4/M7 retain
the implementation, decision or acceptance work listed above. Completed slices
must not be scheduled again merely because their parent remains active.

## Deferred batches

Near-complete items closed on 2026-09-22:

- **F15/R2 retry policy:** connection errors retain structured classification;
  health/enrichment typed retry limits, recovery and cancellation are covered by
  standing tests. Successful discovery scans are not repeated. Observability,
  clock skew and registry durability remain open.


- **F14/G3:** offline clone summary and its standing tests are complete;
  [the integration note](../done/metamorph-clone-in-oxdm.md) is archived in `24bf983`.
  M4/M7 remain open; the new API has not been released.
- **F15/R4 §9.2:** the [six-outcome envelope matrix](oxvif-cli-release-hardening-plan.md#envelope-acceptance-closed-2026-09-22)
  verifies 12 real CLI invocations across JSON/JSONL with schema rejection controls.
  Descriptor exhaustiveness, runtime work and commercial acceptance remain open.

| ID | Scope | Original tracking / prerequisite |
| --- | --- | --- |
| F01 | OSD extensions beyond OS1, URI/source-mode and unsupported configuration/binding semantics | W10; preserve the already accepted Media slices, classify each remaining operation |
| F02 | PTZ configuration/space, movement/preset/home/tour effects beyond profile identity; Imaging fields and focus modeling | W11/W12; no claim that current profile checks model motion |
| F03 | Full Device/DeviceIO/network/user/relay contracts and Recording/Search/Replay lifetimes | W13/W14; no host-network changes; separate authorization for physical effects |
| F04 | Events filters, subscriptions, queue isolation, renew/expiry/termination and delivery modeling | W15; Media sync is not Events sync; existing receipt-only refusal remains |
| F05 | Broader HTTP binding, WSSE freshness/replay protection, roles and fault injection redesign | W07–W09; only after current-release security/integrity triage; raw hooks remain explicit escape hatches |
| F06 | Whole-program capability consistency, concurrency/rollback, replay dependencies and key-format redesign beyond the K27 collision-retention repair | W16–W19; K27 storage retention is fixed in the candidate, not deferred; see [migration](../replay-storage.md) |
| F07 | Remaining schema/QName/wildcard controls, wider corpus and fuzz/property coverage | W20–W23; retain selected independent corpus gates, no all-operation coverage claim |
| F08 | RTSP decoder/playback, batch file exports, camera-write CLI commands and standalone reusable navigation crate | Existing CLI maintenance/navigation plans; new threat model and permission UX before writes |
| F09 | Official Homebrew Core, Debian/Ubuntu and Windows community-channel submissions | Distribution plan; current native packaging checks remain a release gate, submission is not implied by generated artifacts |
| F10 | Bounded snapshot image-compatibility investigation | Preserve A03 destination/auth/size/no-clobber policies; obtain a sanitized reproducible response before changing image acceptance |
| F11 (basic slice shipped in 0.17) | Remaining Mock Fleet discovery and acceptance | [Basic Fleet evidence](mock-fleet-basic-plan.md) and [operator guide](../mock-fleet.md). Full scope matching, Hello/Bye/Resolve, native LAN/VMS acceptance and Metamorph mixtures remain open |
| F12 (status bar shipped in 0.17) | Remaining native-terminal/IME acceptance | The [CLI re-entry](../done/cli-0.17-reentry.md) delivery is archived. Keep unverified platform/IME checks explicit in the [navigation plan](cli-vim-navigation-plan.md); publication adds no terminal evidence |
| F13 | Review subsequent dependency batches and keyring migration | Grouping acceptance is complete; see calibrated ownership above |
| F14 | Metamorph M4/M7 | G1/G3 delivered, G2 superseded; do not reopen recorder extraction |
| F15 | Remaining CLI runtime/descriptor and commercial-pilot hardening | First diagnostic release is delivered; choose bounded R2–R5 work |

F10 research disposition, 2026-09-12: review of the user-provided ONVIF Device
Manager source found GetSnapshotUri → HTTP stream download → WPF image decoding,
with no RTSP fallback in that path. A synthetic JPEG with trailing CR/LF decoded
through the independent Windows decoder but failed oxvif's terminal-EOI signature
check. This establishes one compatibility difference, not the cause of the
observed Hanwha non-image response. No GPL code was transplanted; any follow-up
must be independently implemented. Permissive certificate acceptance, redirects,
proxy behavior or credential-bearing URL normalization are not accepted solutions.
The existing successful read-only snapshot/diagnose evidence remains in the
[snapshot repair record](../done/snapshot-auth-repair.md).

F05 notification slice, 2026-09-22: both listener APIs now limit active connections
to 32, request read/ack time to ten seconds, headers to 128 KiB and UTF-8 bodies
to 1 MiB. HTTP POST requires one decimal Content-Length; duplicate lengths and
Transfer-Encoding are rejected. Queue delivery retains a connection slot and is
outside the request deadline. Standing tests cover partial-header/body timeout,
overload/recovery with exact origin, malformed framing/UTF-8 rejection and the
header size boundary. The peer wrapper remains metadata, not authentication.
TLS, chunked decoding and the wider W07–W09 mock HTTP/auth/fault work remain open;
this bounded listener slice does not establish public-endpoint suitability.
Validation: 14 listener tests pass. Disabling deadline enforcement or accepting
duplicate lengths makes the corresponding standing test fail. Both workspace
Clippy/test modes pass (1,341 all-feature / 1,225 default; seven ignored each),
as do formatting, strict rustdoc, inventory controls and local Markdown links.

No new dependency major upgrade enters 0.17 just because an outdated report lists
it. Advisory remediation and compatibility fixes remain governed by the release
blocker policy.

## Re-entry rule

A deferred item returns to the current release if reproducible evidence shows
data loss, secret exposure, unsafe side effects, a misleading success in an
included contract, or a shared change breaking previously working behavior.
Record the exact case, impact, fix/revert/refusal options and acceptance check
in G01/G02/G03 before deciding its disposition. Missing optional functionality
alone is not a blocker.

## Preserved tracking

Keep [W00–W26](mock-fidelity-execution-checklist.md), C01–C12 and K findings with
their existing evidence and honest PARTIAL/TODO status. R01–R08 select completed
slices, not entire milestones. W00–W06 foundations and W24/W25 release gates remain
owned by current acceptance; this document does not defer them wholesale.
Future implementation must use the existing operation cards, batch cadence and
paired-document requirements, not reconstruct scope from conversation memory.

F15/R3, 2026-09-22: `health check --details` and `health --details` now show every check, status, reason/detail and elapsed time in human output. JSON/JSONL retain their full payload; `-v` remains diagnostic verbosity. Renderer and real-binary acceptance pass, together with both workspace Clippy/test modes (1,343 all-feature / 1,227 default; seven ignored each). Broader R3 terminal acceptance remains open.

RS1 (2026-09-22): [four scoped read selectors](../done/mock-fidelity-read-selectors.md) are complete as a bounded W04 batch across Media/PTZ/Recording. Shared rendering now preserves escaped values and OSD structure; the selected corpus expands to 170 instances / 50 operations and passes local strict Xerces. Full operation, lifecycle, fault and capability coverage remains open.

EP1 (2026-09-22): [event pull consistency](../done/mock-fidelity-event-pull.md) applies the active lexical filter to queued IO and snapshots selection/filter atomically. Concurrency/reentrant controls and four client captures supplement RS1; per-subscription lifecycle and full topic semantics remain open.

2026-09-22 follow-up: [remaining-plan acceptance inputs](remaining-plan-acceptance.md) record the fresh Windows capacity/terminal/credential evidence, offline snapshot probe and exact remaining platform/product/channel prerequisites. This is a supporting register for these existing work families, not a new plan group.

OS1 (2026-09-23): [bounded OSD CRUD](../done/mock-fidelity-osd-crud.md) adds scoped candidates, atomic per-source quotas, binding refusal and commit-only persistence/replay. Client coordinate/color/persistence XML and quota parsing are corrected. Selected external corpus: 198 instances / 56 operations. Background color, temporary text and arbitrary write extensions refuse explicitly; URI/source-mode and wider W10 work remain open.

EP2 (2026-09-23): [bounded pull-point lifecycle](../done/mock-fidelity-event-lifecycle.md) delivers independent endpoints/filters/queues, atomic allocation, expiry/renew/unsubscribe and preserved public RequestCtx construction. External corpus: 216 XML instances / 59 Actions. Push, property synchronization, complete topic grammar and normative WSNT Fault detail remain separate W15 work.
