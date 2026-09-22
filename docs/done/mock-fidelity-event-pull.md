# Event pull consistency batch EP1

[English](mock-fidelity-event-pull.md) | [繁體中文](mock-fidelity-event-pull_zh.md)

Status: DONE for the bounded EP1 consistency batch. Baseline `cb25276`, 2026-09-22. W15 with W16 consistency.

| Section | Purpose |
| --- | --- |
| [Operation card](#operation-card) | W01 review and selected defect |
| [Acceptance](#acceptance) | Bounded evidence and exclusions |

## Operation card

ID `events.PullMessages`; Action
`http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/PullMessagesRequest`.
Client/session event polling reaches `dispatch_events` → `events::resp_pull_messages`
→ `SharedState::modify_returning`, `io_event_response`, `soap` and the clock helper.
The handler receives only state, after the shared request boundary; Timeout and
MessageLimit are not modeled. Current state is one filter/queue/counter per mock
instance, not per subscription. No public request context or endpoint API changes.

EP1-QUEUE: pending IO events bypass the active filter, although synthetic events
honor it. EP1-SNAPSHOT: queue selection, synthetic sequence update and filter read
use separate state acquisitions; an on-change hook can change the filter before
the selected event is rendered. IO token output also lacks XML escaping. These
are source-observed internal consistency defects, not a complete W15 contract.

The target takes queue selection/counter advance/filter snapshot in one state
transaction, then renders outside the lock. A filtered queued event consumes one
queue slot and yields an empty response, matching the existing synthetic policy.
Queued events retain priority; filtering does not drain arbitrary backlog in one
call. Hooks fire once per pull transaction; a reentrant change cannot change that
pull's captured filter. Synthetic sequence wraps at u64 exhaustion instead of
panicking. Token text is escaped. Existing lexical topic matching is retained.

References reviewed externally: pinned event service WSDL and SOAP 1.2 resources
from `packaging/schema-sources.json`; [Core](https://www.onvif.org/specs/core/ONVIF-Core-Specification.pdf)
event handling is the broader contract. This batch preserves the existing immediate
single-message model; it does not claim full subscription timing or TopicExpression
semantics. `RelayOutput` versus advertised `Relay` topic naming remains a separately
tracked source inconsistency; no normative topic renaming is inferred here.

## Acceptance

C01/C02: shared Action/operation boundary unchanged, no new readers.
C03/C04/C08/C09: retain parser/auth/limit/fault controls; no new field or ordinary
fault policy. Full Timeout/MessageLimit validation remains open.
C05/C06: two instances, queued include/exclude order, sequence, unchanged unrelated
state, one hook and reentrant-filter snapshot, concurrent distinct queue consumption.
C07: decoded escaped token and selected full-response external validation.
C10/C11: preserve immediate model and typed parser behavior; demonstrate both HTTP
and in-process delivery. C12: mutation campaign, exact restore, five gates and
independent corpus validation. Lifecycle, per-subscription identity/filter/queue,
renew/expiry/termination, topic namespace semantics and replay retirement stay open.

## Results

Queue selection, counter advancement and filter snapshot now share one state
transaction. Both transports verify filtered IO consumption, escaped input identity,
typed notification data, sequence and one hook per pull, plus instance isolation.
The four-worker test consumes 32 distinct queue items exactly once. A reentrant
hook test changes the live filter after commit and proves the selected event uses
its earlier snapshot; the u64 boundary no longer panics.

Disabling the IO filter and replacing the captured synthetic filter with a live
read caused both transport tests, the reentrant test and the corpus driver to
fail in the unfiltered all-feature/no-fail-fast campaign. Candidate bytes were
restored exactly. The four new client exchanges select IO/synthetic include and
exclude paths. Their Action-to-payload mapping explicitly retains the Events
Request suffix and port namespace distinction.

W15/W16 remain PARTIAL. This does not create independent subscriptions, expire
them, implement Timeout/MessageLimit, or fix advertised Relay topic naming.

Final acceptance: 178 XML instances / 89 exchanges / 51 operations pass pinned strict Xerces (68 successes, 21 existing faults). Workspace all-features 1,351 and default 1,235 pass, seven ignored each across 43 suites; both Clippy modes, fmt, inventory controls and documentation links pass. The new snapshot example is tested separately and is not added to those workspace totals.
