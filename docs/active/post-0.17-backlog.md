# Work after the 0.17 release cut

[English](post-0.17-backlog.md) | [繁體中文](post-0.17-backlog_zh.md)

Status: active follow-up after 0.17.0 publication; reconciled 2026-09-22.
No implementation or release date is promised. The completed
[release cut](../done/release-0.17-cut.md#blocking-acceptance) and
[publication evidence](../done/release-0.17-finalization.md) are archived.
Their closure does not complete the broader work below or turn unperformed checks into passes.

| Section | Purpose |
| --- | --- |
| [Calibrated work ownership](#calibrated-work-ownership) | Current disposition, owner and exit criteria |
| [Deferred batches](#deferred-batches) | Remaining scope and original IDs |
| [Re-entry rule](#re-entry-rule) | When a finding becomes a current blocker |
| [Preserved tracking](#preserved-tracking) | Do not lose technical work cards |

## Calibrated work ownership

Calibration date: 2026-09-22; source baseline `80bcf14`. This is a work inventory,
not a commitment to put every item into 0.18. All 16 previously active document
families were checked; dependency integration/grouping can now be archived, leaving
15 active families at calibration; G3 closure below now leaves 14. Several are registers or preflights for the same Mock programme.

Use four dispositions: **implementation** (confirmed missing scope),
**evidence needed** (implementation exists but the named acceptance is not recorded),
**decision/deferred** (proposal awaiting a bounded product decision), and
**maintained reference** (inventory or supporting evidence, not another feature).
`TODO` in W12–W15 means the planned hardening audit is missing; it does not mean
those services have no existing handlers. A route count is not a bug count.

| ID | Current disposition / single owner | Next work and completion condition |
| --- | --- | --- |
| F01 | Implementation: W10, [execution checklist](mock-fidelity-execution-checklist.md); profile preflight is supporting evidence | Select remaining OSD/URI/source-mode/binding operations; complete field/Fault/state/replay cards and both-transport/independent-instance checks. Do not redo PA1/VS1/VE1/AM1. |
| F02 | Implementation: W11/W12, same checklist | Finish non-profile PTZ selectors/spaces/effects and per-source Imaging/focus contracts. Verify two distinct heads/sources, limits and refusal without mutation; no physical motion guarantee. |
| F03 | Implementation: W13/W14, same checklist | Audit Device/DeviceIO and Recording/Search/Replay state/lifetimes in bounded service groups; include cascades, timeout/termination and state-after-error. Existing handlers are the baseline, not absent functionality. |
| F04 | Implementation: W15, same checklist | Model per-subscription identity/filter/queue and renew/expiry/unsubscribe; test isolation and controlled lifetime. Media synchronization receipts do not satisfy this. |
| F05 | Implementation: W07–W09; [pipeline](mock-fidelity-pipeline-preflight.md) | Classify full HTTP/auth/fault-injection gaps and the notification listener separately. Require bounded read time/concurrency, malformed-input controls and exact refusal evidence; do not reimplement delivered scoped auth. |
| F06 | Implementation: W16–W19; pipeline owns dependency detail | Choose remaining capability/mutation/read pairs; test atomic commits, refusal preservation and replay visibility. K27 storage retention is DONE; future key-format redesign is a distinct decision. |
| F07 | Implementation/evidence: W20–W23; [schema preflight](mock-fidelity-schema-preflight.md) | Expand beyond selected 160 instances/46 operations; preserve explicit anchors, pinned external resources, negative controls and bounded fuzz/property seeds. The 0.17 selected CI gate already passed. |
| F08 | Decision/deferred: [CLI roadmap](oxvif-cli-plan.md), [navigation](cli-vim-navigation-plan.md) owns M6 | Separate decoder/playback, batch export, controlled writes and crate extraction. Select a bounded deliverable and its interface/permission/recovery tests before implementation. None is automatically a 0.18 blocker. |
| F09 | Evidence/operations/deferred submission: [distribution](oxvif-cli-three-platform-distribution-plan.md) | Inventory actual APT/tap URLs, signing/recovery owners and current metadata; then verify isolated install/upgrade/downgrade/removal. 0.17 artifacts/staging are DONE; official admission needs its own evidence. |
| F10 | Evidence needed: snapshot investigation, [repair record](../done/snapshot-auth-repair.md) | Obtain a sanitized reproducible non-image/JPEG response; test bounded format handling without weakening destination/auth/TLS/size/no-clobber policies. Trailing CR/LF is a known synthetic difference, not a proven Hanwha root cause. |
| F11 | Evidence needed: [Fleet](mock-fleet-basic-plan.md); broader discovery features are deferred | Record authorized OS/interface/VMS four-device identity, endpoint, isolation, shutdown/restart checks. Full scopes/Hello/Bye/Resolve, mixed personas and sustained load require separate implementation/acceptance. |
| F12 | Evidence needed: navigation plan | Record native terminal and human IME composition/cancellation/restoration; Windows resize evidence exists. CI and key injection do not replace the missing platform/input-method checks. |
| F13 | New maintenance review: [closed dependency plan](../done/dependency-maintenance-plan.md#closure-evidence-2026-09-22) is evidence only | Review open PR #18 and keyring 4.x native-store migration separately; record each update's compatibility and affected tests. Group reuse/no-duplicate verification is DONE, not a task to rerun or grounds to auto-merge. |
| F14 | Decision/implementation: [Metamorph](metamorph.md), [closed clone note](../done/metamorph-clone-in-oxdm.md) records G3 | G1/G3 are DONE and G2 is SUPERSEDED. G3 adds offline summary counts and standing regressions (unreleased). Remaining work is M4 persona/control transitions or M7 reference-value comparisons and their tests. |
| F15 | Implementation/evidence: [CLI hardening](oxvif-cli-release-hardening-plan.md) | Select R2 observability/retry, R3 health details or R4 descriptor-contract subgroup with exact assertions. R5 multi-vendor/soak/signing/support and recovery stay bounded separately; first package/publication is DONE. |

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
Fresh GitHub inspection for F13 is dated in the closed dependency plan. Next
execution must recheck inputs before reuse.

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
| F01 | Remaining Media OSD, URI/source-mode and unsupported configuration/binding semantics | W10; preserve the already accepted Media slices, classify each remaining operation |
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

F05 also retains the notification listener's inherited bounded HTTP reader:
the peer wrapper is connection metadata, not authentication. It does not add
TLS, chunked decoding, per-connection read deadlines or a connection limit.
Listener lifecycle tests and peer assertions do not establish suitability for an
untrusted public endpoint. No new internet-exposure claim is part of this cut.

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
