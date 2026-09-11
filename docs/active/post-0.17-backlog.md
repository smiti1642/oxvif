# Work after the 0.17 release cut

[English](post-0.17-backlog.md) | [繁體中文](post-0.17-backlog_zh.md)

Status: scheduled after the bounded 0.17 cut; no implementation or release date
is promised. [Release blockers](release-0.17-cut.md#blocking-acceptance) stay in
0.17 acceptance and cannot be moved here merely to make the checklist green.

| Section | Purpose |
| --- | --- |
| [Deferred batches](#deferred-batches) | Remaining scope and original IDs |
| [Re-entry rule](#re-entry-rule) | When a finding becomes a current blocker |
| [Preserved tracking](#preserved-tracking) | Do not lose technical work cards |

## Deferred batches

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

F10 research disposition, 2026-09-12: review of the user-provided ONVIF Device
Manager source found GetSnapshotUri → HTTP stream download → WPF image decoding,
with no RTSP fallback in that path. A synthetic JPEG with trailing CR/LF decoded
through the independent Windows decoder but failed oxvif's terminal-EOI signature
check. This establishes one compatibility difference, not the cause of the
observed Hanwha non-image response. No GPL code was transplanted; any follow-up
must be independently implemented. Permissive certificate acceptance, redirects,
proxy behavior or credential-bearing URL normalization are not accepted solutions.
The existing successful read-only snapshot/diagnose evidence remains in the
[snapshot repair record](snapshot-auth-repair.md).

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
