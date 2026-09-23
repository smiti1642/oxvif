# Events pull-point lifecycle EP2

[English](mock-fidelity-event-lifecycle.md) | [繁體中文](mock-fidelity-event-lifecycle_zh.md)

Status: DONE for the bounded EP2 batch. W01 recorded before edits, baseline `6ec107f`, 2026-09-23.
Owner: Q01 in the [execution queue](../active/autonomous-completion-schedule.md).

| Section | Purpose |
| --- | --- |
| [Operation cards](#operation-cards) | Identity, existing paths and proposed effects |
| [Contract](#contract) | Bounded model and compatibility |
| [Acceptance](#acceptance) | C01–C12 and independent evidence |

## Operation cards

Reference reviewed: [Core 26.06 §9.1](https://www.onvif.org/specs/core/ONVIF-Core-Specification.pdf).
Pinned event.wsdl, b-2.xsd and r-2.xsd identified by packaging/schema-sources.json
were inspected outside the checkout. No schema-derived fixture is stored here.
Client methods and wrappers are in client/events.rs and session.rs; the shared
events types parse subscription references/times and notifications.

| Ledger ID / full Action suffix | Current path and fields | Target state / fault / output |
| --- | --- | --- |
| events.GetEventProperties / EventPortType/GetEventPropertiesRequest | dispatch_events → events::resp_event_properties; static topics and dialects, no body fields | Retain static read and document supported filter subset |
| events.GetServiceCapabilities / EventPortType/GetServiceCapabilitiesRequest | events::resp_event_service_capabilities; static limit 4 | Enforce four concurrent pull points; no push/policy/persistent-storage claim |
| events.CreatePullPointSubscription / EventPortType/CreatePullPointSubscriptionRequest | events::resp_create_pull_point_subscription extracts global TopicExpression; overwrites one event_filter; fixed subscription_1; ignores lifetime/policy | Scoped optional filter/time; unique endpoint and private volatile subscription; capacity rejection before mutation |
| events.PullMessages / PullPointSubscription/PullMessagesRequest | events::resp_pull_messages ignores Timeout/MessageLimit and endpoint; consumes one instance queue/counter | Scoped bounded inputs; select only existing live endpoint; independent queue/filter/counter; coherent current/termination timestamps |
| events.Renew / SubscriptionManager/RenewRequest | policy refusal by default; opt-in events::resp_renew static times | Live pull point extends/sets expiry atomically; unknown/expired/time refusal preserves runtime; explicit receipt opt-in stays receipt-only |
| events.Unsubscribe / SubscriptionManager/UnsubscribeRequest | policy refusal; opt-in resp_empty | Live endpoint removes only its subscription/queue; repeat/unknown refuses; receipt opt-in unchanged |
| events.SetSynchronizationPoint / PullPointSubscription/SetSynchronizationPointRequest | policy refusal; opt-in resp_empty | Remains explicitly unmodeled pending full property initialization semantics; no false synchronization claim |
| events.Subscribe / NotificationProducer/SubscribeRequest | policy refusal; opt-in resp_subscribe | Push delivery remains unmodeled; receipt opt-in unchanged |

EventPortType/PullPointSubscription Action prefix is
`http://www.onvif.org/ver10/events/wsdl/`; SubscriptionManager/NotificationProducer
prefix is `http://docs.oasis-open.org/wsn/bw-2/`. Operation elements use the events
namespace or `http://docs.oasis-open.org/wsn/b-2` respectively. Transport URL,
not a Header/body decoy or last-created global ID, selects a subscription.

## Contract

- Carry endpoint privately through built-in chains; preserve the four public
  RequestCtx fields, custom responder ordering and raw/fault/auth precedence.
- Keep subscription runtime private on MockState and outside persisted DeviceState.
  New instances start with no subscriptions. IDs never alias during an instance.
  Existing instance event fields remain available as synthetic IO ingress; accepted
  lifecycle operations notify after all locks are released.
- Bound live subscriptions to four and each queue to a documented finite limit;
  slow consumers cannot consume another subscription's messages. Validate all
  rejection conditions before draining ingress or changing queues/counters.
- Resolve filter topic prefixes by their actual namespace scope where modeled;
  unsupported dialects/content filters/policies/extensions explicitly refuse.
  Do not silently reinterpret arbitrary XPath as literal matching.
- Accept validated bounded relative lifetimes and supported absolute UTC times;
  expiry uses a private controllable clock for deterministic tests. No real pacing
  sleep: PullMessages returns queued or one synthetic event immediately, at most
  its requested limit. Document this timing model and any unmodeled lexical forms.
- Unknown/expired references, invalid time/limit, capacity and unsupported fields
  have reviewed structured faults. Include relevant independent Fault captures;
  mock-specific policy remains visibly distinct from normative service faults.
- Built-in replay must not answer a live pull point from a static recording.
  Standalone fixture replay remains an explicit caller-owned policy.

## Acceptance

C01–C04: exact Actions, namespaces, endpoint binding, scoped selectors, duplicate/
missing fields, Header decoys, scalar/attribute limits and literal escapes.
C05–C06: two subscriptions with different filters, fan-out, slow-consumer isolation,
unique allocation, capacity, expiry/renew/unsubscribe, rejected snapshots/hooks,
reentrant callbacks and separate instances. C07–C08: full typed values, structured
faults and real externally validated captures. C09: existing auth/resource controls.
C10–C11: capabilities, explicit receipt policy, replay and public construction.
C12: one combined full unfiltered mutation run, exact restoration, five workspace
gates, strict rustdoc where applicable, bilingual inventory and link checks.

Remaining full-topic/property synchronization, push delivery and physical event
timing are separate W15 work, not silently accepted by this batch.

## Delivered model and evidence

The operation cards above record the pre-edit baseline. Live handlers now reside
in services/events/lifecycle.rs; obsolete test-only copies were removed and their
assertions migrated to production handlers. The model has four live subscriptions,
128 events per queue, 1–3600 second lifetimes and 0–60 second immediate pulls.
Integer PT combinations and exact UTC are accepted; fractional/calendar forms,
policy, full XPath and message-content filters explicitly refuse. Unknown/expired,
time and overflow use Sender/mock:RequestPolicy, capacity Receiver/mock:RequestLimit,
unsupported settings Sender/mock:UnmodeledEffect. Full normative WSNT Fault detail
is outside this bounded batch. event_filter is now an unused compatibility field;
event_seq remains a wrapping aggregate synthetic-event count. Subscription runtime
is volatile and excluded from DeviceState persistence.

EP2-01: scoped QName-content checking exposed an undeclared client tns1 prefix;
SoapEnvelope now declares the standard topic namespace. Relay notifications also
use the already advertised Relay path. Capabilities now deny subscription policy
and push producers, retaining the enforced MaxPullPoints=4.

Tests: mock_event_lifecycle covers both transports, independent filters/fan-out,
unknown/malformed/time refusals preserving ingress/hooks, bounded concurrent
allocation, selective removal and recorded-fixture fallback. lifecycle unit tests
cover deterministic expiry/absolute renewal, ID exhaustion, calendar arithmetic,
prefix shadowing, slow-consumer overflow and reentrant removal after commit/counter
wrap. Migrated mock_event_pull and workflow tests require real subscriptions.

Independent evidence: 108 captured exchanges / 216 XML instances / 59 Actions
pass pinned offline Xerces XSD 1.1. Thirteen exchanges cover create, queued/filter/
synthetic pull, renewal, unsubscribe and an unknown-endpoint Fault. Export indexes
now map Events portType Action names to actual payload namespaces; the first run
correctly rejected the previous inferred mapping. Seven validator qualification
tests pass with the pinned Python environment. This is local structural evidence,
not whole-service semantic or hosted CI acceptance.

Combined full unfiltered workspace all-feature mutation run exited 101 with ten
failing tests across four targets: expiry, QName scope, filter, capacity, lifecycle,
queue and captured-path controls detected changes. Both modified source files were
restored byte-for-byte. Selective-remove/replay mutations were masked/redundant in
that combined run, so it does not prove independent sensitivity for those switches;
ordinary positive/refusal controls verify their delivered behavior.

Final local gates: fmt, both workspace/all-target Clippy modes and workspace tests pass (1368 all-feature / 1249 default). Strict rustdoc passes both modes. Inventory reports 159 routes / 161 Action sites / 162 reader occurrences; link checks pass. W15 remains active for the explicitly excluded work.
