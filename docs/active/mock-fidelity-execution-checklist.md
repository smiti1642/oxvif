# Mock fidelity execution checklist

[English](mock-fidelity-execution-checklist.md) | [繁體中文](mock-fidelity-execution-checklist_zh.md)

Planning baseline: `b134f73`, 2026-09-10. Policy: [approved main plan](mock-fidelity-hardening-plan.md).
Scope index: [159 routed operations](mock-fidelity-operation-ledger.md).
This document turns M0–M6 into traceable work; it does not mark those milestones
complete. No additional runtime behavior is changed by this planning revision.

Related bounded work: [contributor PR integration](contributor-pr-integration-plan.md)
covers #14 dependencies, #17 notification origin and W26/#16 Media synchronization.
It does not mark the remaining service batches complete or authorize main-branch merges.

| Section | Purpose |
| --- | --- |
| [Execution rules](#execution-rules) | Resume without conversation history |
| [Batch and verification cadence](#batch-and-verification-cadence) | Approved reduction of repeated full-suite runs |
| [Readiness and operation card](#readiness-and-operation-card) | Required work before editing a handler |
| [Review axes](#review-axes) | Checklist inherited by every operation |
| [Work register](#work-register) | IDs, dependencies, locations and acceptance |
| [Service batches](#service-batches) | Bounded migration slices |
| [Source and test map](#source-and-test-map) | Shared code and independent evidence |
| [Risk register](#risk-register) | Known observations versus open questions |
| [Verification commands](#verification-commands) | Reproducible local gates |
| [Closure and handoff](#closure-and-handoff) | Evidence, release boundaries and next task |

## Execution rules

Release scheduling is now owned by the [0.17 cut](release-0.17-cut.md) and
[follow-up backlog](post-0.17-backlog.md). Existing PARTIAL/TODO states remain
technical evidence, not a requirement to finish every service before 0.17.
Known data loss and security blockers cannot be deferred.

1. Read the main plan's D1–D3 decisions, this checklist, and the operation ledger.
   Start from `git status` and the actual commit, not the last conversation.
2. Run the inventory checker. Review source changes since the recorded baseline;
   the checker only sees dispatch signatures, not handler changes or new helpers.
3. Select a work ID whose dependencies are satisfied. For a service migration,
   first complete W01 cards for **every row in that batch**, including reads.
4. Commit independently verifiable service subgroups using the cadence below, not
   each individual helper or operation fix. Update both language versions, operation
   statuses, evidence and the next task in the same slice. Do not mark a milestone
   complete because one helper or two operations have been repaired.
5. New findings get an ID, source symbol, affected operation IDs, reproduction or
   explicit `UNVERIFIED`, risk, test target and disposition before further work.
   New scope cannot disappear into a chat message or an aggregate test count.

Status vocabulary: `TODO`, `IN-PROGRESS`, `DONE`, `BLOCKED:<reason>`,
`NA:<reason/evidence>`. `PARTIAL` is reserved for a documented sub-slice.
No time or completion percentage is inferred from route counts.

## Batch and verification cadence

Maintainer-approved on 2026-09-11, after `6382458`: reduce repeated test runs by
completing coherent service subgroups before full validation and commit. This
changes execution granularity, not the agreed fidelity or release criteria.

| Stage | Required work | Avoid repeating |
| --- | --- | --- |
| Batch entry | Record operation IDs, shared dependencies, acceptance cases and intended commit scope; reuse current inventory unless relevant source changed | Whole-workspace baseline runs when a matching green baseline already exists |
| Implementation | Finish the subgroup together; use compilation and focused affected tests at meaningful checkpoints | A full suite after every helper, assertion, operation or prose edit |
| Sensitivity check | One planned batch mutation campaign, with the required unfiltered all-feature/no-fail-fast run; restore exactly | Separate full mutation campaigns for each new assertion |
| Batch acceptance | Run the five per-code-commit gates once on the restored candidate; run relevant external schema checks once if wire output changed | An additional full green run immediately before the same acceptance gate |
| Failure recovery | Fix and rerun the failing/affected tests first; establish final green gates for changed code before commit | Repeated unaffected matrices while diagnosing one failure |
| Documentation and delivery | Update paired docs and evidence together; commit and push the accepted subgroup once | Separate documentation-only pushes that trigger the same CI repeatedly |
| Final programme acceptance | Complete the feature/MSRV/native-platform and release-specific matrix | The full release matrix on every service subgroup unless that change affects it |

No test is deleted, ignored or weakened to reduce run count. Retain positive and
negative controls, exact fault assertions, state/effect checks and HTTP/in-process
coverage. Reuse evidence only when its relevant code, dependency, feature and
tooling inputs are unchanged; record the source revision and do not call an old
run a new result. New failures or changed common dependencies require the affected
checks to run again. Do not claim release acceptance from a subgroup's green gate.

The first batch under this cadence covers eight source-confirmed effect stubs: Device and
PTZ auxiliary commands, Device reboot/firmware-upgrade/system-restore, Events
subscribe/renew, and Search EndSearch. Treat their exact-operation policy,
mock/replay/HTTP behavior, existing workflow migration and paired documentation
as one cohesive delivery, not eight separately gated commits. Implementation is
recorded as A3 in the policy preflight. The subsequent [PA1 assembly batch](mock-fidelity-profile-assembly.md)
implements initial Configuration, optional rename/All, same-slot conflicts, capacity
and touched reference counts. Next: video configuration/options, including shared
source/encoder compatibility and remaining field contracts. [VS1](mock-fidelity-video-source.md)
now implements the eight source operations; next are the eight encoder
configuration/options/instances operations. Neither subgroup closes W10.
Report completed/remaining subgroups and open
blockers rather than using test counts as a completion percentage.

## Readiness and operation card

The route locations are now enumerated. **Field-by-field contracts, complete
Fault mappings and fidelity classifications have not yet been audited.** W01 is
mandatory design work before the next handler migration, not something to fill
in after changing tests. Work may proceed batch by batch; unrelated batches do
not have to be normative-audited before shared infrastructure work begins.

For each ledger ID, create a section in a batch record under `docs/active/`
(paired English/`_zh` files with table navigation). Use the following fields:

| Required field | What must be recorded before implementation |
| --- | --- |
| Identity | Stable ledger ID; source commit; full Action expression in client; dispatch arm; operation namespace/local name; intended endpoint routing |
| Code path | Client method and session wrapper; handler; every transitive request reader, validator, state writer and renderer; relevant type parsers/serializers |
| Current inputs | Project-source field paths, attributes, repeats, defaults/trim/decoding, ignored arguments; identify no-body handlers explicitly |
| Current outputs | Response renderer, reused fragments and escaping boundary; ordinary Fault return paths; HTTP versus in-process behavior |
| Reference evidence | Applicable official Service/Core/SOAP document URL, version and section; external manifest ID/hash; a reviewed conclusion or `BLOCKED`, not “probably conformant” |
| Target contract | What changes and why; compatibility impact; valid extensions/repeats to preserve; unmodeled fields explicitly refused or documented |
| Behavior | One of modelled/static-read/ack-only/unsupported, with observable limits; request receipt is not effect; planned D2 default and opt-in behavior |
| State | Exact collections/keys/fields written, getter or state assertion; secondary effects, hooks, queues, invalidation, rollback and cross-service dependencies |
| Cases | Named existing/new tests for C01–C12; each axis has a test or reviewed NA; invalid input must check state and side effects remain unchanged |
| Delivery | Work ID, prerequisites, affected public docs, owner when work starts, planned evidence location and any maintainer decision needed |

Do not bundle normative field tables, schema-derived fixtures or generated schema
indexes in these cards. Keep such artifacts external under D3; project-authored
source observations and sanitized review conclusions may live in the repository.
Do not promise strict typed/extension validation that the parser cannot provide.

Readiness is blocked by unknown identity, unchecked required input semantics,
unmapped ordinary Fault branches, unexplained state loss or a public API decision
beyond D1. Discovery/read-only hardware checks cannot unblock write semantics.

## Review axes

Every operation row inherits all axes; applicability must be recorded, not assumed.

| ID | Required checks and discriminating cases |
| --- | --- |
| C01 Identity | Correct/incorrect full Action, same local name in another service, operation/body mismatch, endpoint dispatch; no substring acceptance assumed valid |
| C02 Text | `&`, `<`, quotes, Unicode, literal `&amp;`, numeric references, CDATA, significant whitespace, empty versus absent; exactly-once decode/escape |
| C03 XML scope | Alternate/default namespaces, rebinding on siblings/ancestors, decoy in Header/Extension/nested body, namespace-qualified attributes, duplicate expanded attributes |
| C04 Shape and types | Required/optional/duplicate/repeated members; field order, subtrees, extensions; booleans including numeric lexical forms, enum/range/overflow, NaN/infinity where relevant; verify applicable lexical rules externally |
| C05 Selection | Two deliberately different profiles/sensors/configurations/jobs; absent, unknown, wrong-family, escaped and duplicate tokens; distinguish “all” filter from missing required selector |
| C06 State | Every accepted field observable; failed request atomicity, fixed/in-use/deleted targets, allocation/collision/cascade, shared Media state, hooks/queues and instance isolation |
| C07 Success wire | Independently inspect envelope and payload QName, attributes, order/cardinality, literal text and URI escaping; do not rely only on client round trips |
| C08 Fault wire | Correct Code and ordered nested Subcodes, scoped QName binding, reason/language/detail, escaped payload, HTTP mapping; specific negative payload and no mutation |
| C09 Auth and limits | Enforced/disabled auth, exempt actions, duplicate/misplaced WSSE fields, invalid digest/nonce/time inputs; bounded bytes/depth/nodes and no secret echoes |
| C10 Fidelity | Capability claims agree with implementation; static reads documented; unsupported and unmodeled effects reject by default; opt-in stubs do not claim real effects |
| C11 Consumer compatibility | Client parsing and `SoapError`, HTTP transport, session fallback, CLI human output/Agent envelope/error/exit code; malformed injection and replay remain explicit |
| C12 Evidence sensitivity | Exact assertions, positive and negative controls, targeted mutation, clean restore, HTTP/in-process parity, external schema coverage and platform/feature evidence |

## Work register

Paths are repository-relative. Existing files are indexed below; new paths here
are proposals, not claims that tooling or tests already exist. W10–W15 each
include operation-sized Fault migration after W05, not just request parsing.

| ID / milestone / status | Prerequisites | Locations and deliverable | Acceptance evidence |
| --- | --- | --- | --- |
| W00 / M0 / DONE | None | `dispatch.rs`, ledger, checker and source audit: literal Action sites including session reconcile with every route; permissive runtime aliases remain separately tracked as K06/W07 | Full Action/site index and exact route-set equality checked; positive/missing/changed/duplicate controls passed; completion is source inventory, not runtime rejection or normative validation |
| W01 / M0 / IN-PROGRESS | W00 for selected batch | Per-operation cards using the template above; normative review external; field/fault/effect coverage per operation | All batch cards pass readiness, C01–C12 assigned, no unexplained defaults or response branches; source/spec disagreement is logged before edits |
| W02 / M0 / PARTIAL | W00 | `xml_parse.rs`, all service files, `request.rs`, `auth.rs`, `canon.rs`: enumerate callers and transitive helper dependencies; classify text/attribute/subtree/raw uses | Each legacy caller has a migration owner or explicit isolation rationale, including test-only readers; newly found readers added to source map |
| W03 / M3 / PARTIAL | W02 | One parsed synthetic request and private route now validate common XML/container/operation identity; DeleteProfile borrows the parsed operation | HTTP/in-process static/stateful boundary controls, raw precedence and quirk baseline controls; full HTTP/header policy and operation-specific field migration remain open |
| W04 / M1,M3 / PARTIAL | W02 | `request.rs`: P-A private owning request, scalar/scoped-attribute/subtree/repeated access implemented; typed field rules and QName-valued content remain open | Seven P-A controls perturbed and restored; C02–C04/C09 controls must also cover migrated handlers, legal extensions and field-specific constraints |
| W05 / M2 / PARTIAL | W01 for mapping; W02 for callers | Structured auth/empty-chain/selected DeleteProfile faults plus reviewed generic synthetic-boundary faults; resource/DTD policy names are explicitly mock-specific | Expanded fault QName, typed failure, consumer and state controls; other ordinary service branches and structured Detail remain open; no blanket conformance claim |
| W06 / M2 / PARTIAL | W05 design before default switch | First-subcode client/health controls and auth CLI JSON/table subprocess controls implemented; public error fields unchanged | Broader nested/flat/vendor error, transport/session and consumer coverage still required before the service default switch; preserve diagnostics and exit-code meanings |
| W07 / M2,M3 / PARTIAL | W03/W05 designs, W06 | Exact synthetic Action routing and common body identity implemented; HTTP extraction and binding remain open | Source-wide routing and HTTP/in-process boundary controls; content type/status/invalid UTF-8/missing/conflicting headers and endpoint policy are not accepted |
| W08 / M3 / PARTIAL | W02/W04/W05 | Scoped Header/UsernameToken parsing, explicit digest/encoding/role policy, static non-reflecting errors and exact exemption; see [auth preflight](mock-fidelity-auth-preflight.md) | Both-transport identity, refusal, state/hook and live user-table controls; auth defaults/order and ordinary CLI classification retained. No freshness/nonce-reuse, role authorization or complete WSSE/HTTP security acceptance |
| W09 / M2 / TODO | W05/W06 | `fault_injection.rs`, `responder.rs`, server admin endpoints and public injection builders | Literal/structured versus intentionally raw malformed output separated; custom QName handling, one-shot matching, ordering, concurrency, clear/reset and compatibility tests |
| W10 / M2–M4 / PARTIAL | W01 for batch, W03–W06 | `services/media.rs`, `media2.rs`, shared state and renderers; batches below | Every Media row passes C01–C12; both views agree on state without sharing incorrect wire shapes; E1 does not close DeleteProfile rows; PA1 implements initial binding/rename/All/capacity/reference counts; VS1 implements scoped source reads/writes; VE1 implements scoped encoder candidates/options/capacity and committed replay; see batch records |
| W11 / M2–M4 / PARTIAL | W01, W03–W06 | PTZ1 scoped ProfileToken identity for 19 existing profile/head consumers; other selectors, coordinate attributes/spaces, configuration subtree, presets/tours and auxiliary commands remain open | Both transports and two different heads; complete field effects/fault policy/concurrency still unaccepted, no invented movement/timing guarantees |
| W12 / M2–M4 / TODO | W01, W03–W06 | `services/imaging.rs`: per-source settings/options/status/move/stop | Fixed versus movable lens, nested settings and typed ranges; no global-name fallback or silent partial application |
| W13 / M2–M4 / TODO | W01, W03–W06, W08 design | `services/device.rs`, DeviceIO dispatch, device state | Repeated users/network entries/scopes, storage subtrees and relay tokens; failure leaves state/auth/events/hooks unchanged; maintenance effects classified under D2 |
| W14 / M2–M4 / TODO | W01, W03–W06 | `services/recording.rs`: separate Recording/Search/Replay dispatch and state lifecycles | Recording/track/job discrimination and cascades; search token/termination/timeout and replay selection audited; finite simulation, not actual recording/media delivery |
| W15 / M2–M4 / TODO | W01, W03–W06 | `services/events.rs`, IO event queue and subscription state | Filter namespace/dialect and lifetime/renew/unsubscribe/pull limits reviewed; queue isolation/order/termination; existing Events sync is not PR #16 Media sync |
| W16 / M4 / PARTIAL | W01 classifications, W05/W06 | [A2/A3 acknowledgment policy](mock-fidelity-ack-policy-preflight.md): 11 classified reset/auxiliary/maintenance/subscription/search-ending routes; shared transport/server policy | Exact-operation opt-in, default refusal, no state/hook/effect/replay retirement; this stub subgroup is migrated, but full operation semantics, partial effects and capability reconciliation remain open |
| W17 / M4 / PARTIAL | W10–W16 classifications | All capability renderers, `discovery_responder.rs`, `fleet.rs`, `snapshot.rs`, `font.rs`, public mock docs | Services/XAddrs/features/limits agree with modeled behavior; discovery/snapshot side channels checked; codec/stream rendering not claimed from static URIs or images; PA1 reconciles profile capacity and five supported binding kinds; other declarations remain open |
| W18 / M4 / PARTIAL | W10–W16 candidate behavior | K13 collision-safe allocation; K16 atomic binding plans; conditional notifications; K08 hooks run outside locks with commit snapshots, and profile/catalogue reads share one snapshot | Selected allocation, binding, reentrant and three-path profile snapshot controls; broader concurrent writes, instances, rollback, other queue/read snapshots and replay remain open; public signatures unchanged, callback ordering caller-managed |
| W19 / M3,M6 / PARTIAL | W03/W09 designs | Built-in profile creation/deletion, Media1 video binding and Media2 generic binding use private committed effects; cross-service reads, HTTP, instance and chain controls | Remaining configuration writes and other mutations, standalone replay policy, full read dependencies, full normalization/key-format redesign and concurrent/callback visibility remain open (bounded K27 storage repair is recorded below); no new recording of device secrets; PA1 extends committed profile effects to reference-count and PTZ-compatible reads; VS1 adds post-commit source/profile/options invalidation while physical-source recordings remain intact; VE1 adds source-capacity and profile-encoder-options invalidation; AM1 adds audio/metadata committed effects. |
| W20 / M5 / PARTIAL | W04/W05 corpus | `tests/mock_schema_shape.rs`: scoped resolution, Envelope/Fault inclusion and missing-resource failure implemented; see schema preflight | Seven generic controls and Fault-wrapper perturbation passed sensitivity checks; QName values, wildcard/unresolved accounting and request corpus remain open; pins unchanged |
| W21 / M5 / PARTIAL | W20, D3 | Pinned offline tooling, 20 schema-free controls, seven independent-backend tests, and selected 46-operation Media corpus export with explicit payload anchors | 160 instances pass locally after B16, including 21 refusals (seven profile, three source, two rate, two encoder, three audio/metadata and four synchronization). Remaining operations and broader input/semantic coverage are not accepted |
| W22 / M5 / PARTIAL | W00 for inventory; W21 for schema job | Windows/Linux inventory, Xerces qualification, official-source compilation and selected profile/source/rate/encoder-instance validation gate package | Sources and corpus stay external with no uploaded artifacts. Selected corpus has 160 instances over 46 operations; combined-candidate hosted CI, whole-program instance coverage and release evidence check remain pending |
| W23 / M1,M6 / TODO | Each migrated batch | All named regression suites, client fixtures and generic parser tests | Audit hollow positives/negatives and namespace-stripped or fragment probes; mutations fail at intended assertions; include all targets with `--no-fail-fast`; bounded fuzz/property campaign with seed/limits recorded |
| W24 / M6 / TODO | Integration candidate | Cargo features/MSRV, `.github/workflows/ci.yml`, `packaging/check_xml_features.py`, docs builds | Native Windows/Linux/macOS default/all-feature runs, per-feature warning sweep, MSRV and downstream XML feature-unification checks; mark unavailable evidence blocked/not-run |
| W25 / M6 / TODO | W00–W24 accepted | Bilingual mock/library/CLI/support docs as affected, `OPERATIONS`, README links, CHANGELOG, rustdoc, release evidence | Document D1/D2 migration with working examples; audit all current claims and historical notes without rewriting shipped facts; approval before publish/merge/push/install |
| W26 / IN-PROGRESS | Authorized contributor integration | PR #16 client/session port; scoped receipt-only mock, operation cards, bilingual docs and S01–S08 controls implemented | Targeted tests, inventory and 160 external instances pass; final gates/hosted CI tracked in [B16 evidence](contributor-pr-integration-plan.md#execution-record). No main-branch merge or media-effect acceptance |

## Service batches

Each listed group includes every matching getter/options/capability path, not
only setters. The ledger is the exhaustive list; groups below determine order.
Before each batch, list the exact selected ledger IDs in its record and verify
their union for a completed service equals that service's ledger rows.

| Work | Suggested independently committable sequence | Extra interaction to inspect |
| --- | --- | --- |
| W10 | Profiles/create/delete/binding → video sources/encoders/options → audio/metadata → OSD → stream/snapshot URI and source-mode/static capability rows | Shared `ConfigKind`/selectors/renderers; Media2 wrappers calling Media1 helpers; type attribute versus element differences; fixed/in-use references; PA1 implements initial binding/rename/All/capacity/reference counts; VS1 implements scoped source reads/writes; VE1 implements scoped encoder candidates/options/capacity and committed replay; see batch records |
| W11 | Profile/node/config selectors → configuration/spaces → movement/home/presets → tours/auxiliary/static capabilities | Profiles sharing one node versus separate heads; repeated tour spots; disabled axes; no real motion timing claim |
| W12 | Source selector/settings/options → status/move options/move/stop → capabilities | Focus support, nested mode/value settings, currently ignored inputs |
| W13 | Hostname/time/scopes → users/auth → DNS/NTP/interfaces/protocols/gateway → storage → relays/DeviceIO → maintenance/discovery/services/capabilities/log/URI | Partial multi-entry updates, password handling, IO events versus saved settings, simulated network changes must not reconfigure host networking |
| W14 | Recording CRUD → tracks/jobs/state → Search lifecycle → Replay URI and service capabilities | Deleting referenced records/tracks/jobs, generated-token collision, static session/URI claims |
| W15 | Event properties/capabilities → create/filter/pull → subscribe/renew/unsubscribe/sync | Multiple subscriptions, shared filter/event queue, IO-to-event visibility, timeout/limit handling |

## Source and test map

The map includes shared/non-routed surfaces so “all operations listed” does not
hide surrounding code. W02 must extend it when a new dependency is found.

| Surface | Existing locations | Owner / evidence starting point |
| --- | --- | --- |
| Routing and readers | [dispatch](../../src/mock/dispatch.rs), [legacy extraction](../../src/mock/xml_parse.rs), [request parser](../../src/mock/request.rs), [helpers](../../src/mock/helpers.rs) | W00–W07; `tests/mock_request_identity.rs`, dispatch tests |
| Pipeline and transport | [responder](../../src/mock/responder.rs), [mock transport](../../src/mock/transport.rs), [server](../../src/mock/server.rs), [HTTP client transport](../../src/transport.rs) | W03/W06–W09; HTTP/in-process request boundary tests implemented; HTTP binding tests remain open |
| Auth and injection | [auth](../../src/mock/auth.rs), [fault injection](../../src/mock/fault_injection.rs), [SOAP security](../../src/soap/security.rs), [envelope](../../src/soap/envelope.rs) | W08/W09; unit tests plus invalid-WSSE direct HTTP controls |
| Service implementation | [services module](../../src/mock/services/mod.rs), each file linked in the operation ledger | W10–W15; corresponding `src/client/*.rs`, `src/tests/client/*_tests.rs`, `src/types/*.rs` |
| State and side channels | [state](../../src/mock/state.rs), [discovery responder](../../src/mock/discovery_responder.rs), [fleet](../../src/mock/fleet.rs), [snapshot](../../src/mock/snapshot.rs), [font](../../src/mock/font.rs) | W17/W18; existing state tests and `tests/mock_multi_sensor.rs` |
| Replay/canonicalization | [canon](../../src/mock/canon.rs), [Metamorph module](../../src/metamorph/mod.rs), `src/metamorph/*.rs` | W19; in-module replay/fixture/record/quirk tests; raw fixture preservation |
| Client consumers | [XML](../../src/soap/xml.rs), [SOAP errors](../../src/soap/error.rs), [errors](../../src/error.rs), [session](../../src/session.rs), `src/types/*.rs` | W06; `tests/xml_compat.rs`, session/client/type tests; no broad client rewrite implied |
| CLI consumers | [error](../../crates/oxvif-cli/src/error.rs), [application](../../crates/oxvif-cli/src/application.rs), [output](../../crates/oxvif-cli/src/output.rs), [agent](../../crates/oxvif-cli/src/agent.rs), [contract](../../crates/oxvif-cli/src/contract.rs), [manage](../../crates/oxvif-cli/src/manage.rs), [maintenance](../../crates/oxvif-cli/src/maintenance.rs) | W06/W25; `crates/oxvif-cli/tests/cli.rs`; trace additional classification consumers with `rg`, do not redesign navigation |
| Semantic properties | [round trips](../../tests/mock_roundtrip.rs), [token discrimination](../../tests/mock_token_discrimination.rs), [Media agreement](../../tests/mock_media1_media2_agree.rs) | W10–W18/W23; reconcile every existing `Broken`/`Static`/`Blind` row by name, not old counts |
| Wire and workflow | [schema shape](../../tests/mock_schema_shape.rs), [workflow](../../tests/mock_workflow.rs), [action snapshot](../../tests/mock_action_snapshot.rs), [capabilities](../../tests/mock_service_capabilities.rs), [fixtures](../../tests/fixtures/README.md), [common test helpers](../../src/tests/common.rs) | W20–W23; distinguish helper-generated fixtures from independent validation |
| Public claims and redaction | [mock module](../../src/mock/mod.rs), [crate header](../../src/lib.rs), [redaction](../../src/redact.rs), `docs/mock-server*.md`, `docs/support*.md`, public guides | W08/W17/W25; documentation and sanitized artifact review |

## Risk register

These are planning seeds, not a claim of an exhaustive defect audit.

| ID / evidence level | Observation or question | Owner / closure |
| --- | --- | --- |
| K01 / reproduced, partial fix | DeleteProfile escaped/decoy token failures fixed in E1; other operations not systematically reproduced | W04/W10; retain E1 and add operation cards |
| K02 / source-confirmed | Legacy fragment readers remain across services and auth | W02–W15; no ordinary unaccounted callers at M3 exit |
| K03 / selected DeleteProfile branches fixed | Both services now use nested Sender faults for missing/fixed profiles; independent selected corpus passes | W05/W06; other operation mappings remain open. Client still exposes the first subcode, not the leaf; full-program acceptance is pending |
| K04 / documented risk | Legacy escaped input echoed through the escaped fault helper can double-escape | W05/W23; trace every interpolated reason and reproduce affected paths |
| K05 / selected policy implemented | Factory Reset and Events unsubscribe/sync default to refusal; individual acknowledgment-only opt-in is explicit | [W16 evidence](mock-fidelity-ack-policy-preflight.md); no reset/lifecycle/event effects modeled; remaining operation contracts and effectful stubs open |
| K06 / routing slice fixed; HTTP open | Synthetic Action aliases no longer reach handlers; HTTP handler still returns 200 and uses lossy UTF-8 conversion | W03/W07; see pipeline routing evidence; common body agreement and generic boundary faults implemented; HTTP extraction/status and replay remain open |
| K07 / source-confirmed | Schema/namespace probes have scope/Fault coverage blind spots | W20–W22; fail-sensitive independent validation |
| K08 / selected hook defects reproduced and fixed | Hooks previously held a read lock and could observe an intervening write instead of their mutation; owned commit snapshots now run without that lock | W18/W19 partial; deterministic reentrant/snapshot controls in pipeline preflight; queues, read snapshots, broader writes and replay visibility remain open |
| K09 / unverified | Required fields/ranges/extensions/capability claims may differ from contract | W01/W10–W17; complete batch readiness cards before implementation |
| K10 / not run | Full external schema, Linux/macOS native and multi-vendor validation absent from E1 | W21/W24; real-device evidence remains separate and read-only unless authorized |
| K11 / candidate implemented | Credited PR #16 client/session port and replacement mock in B16 | W26; final candidate evidence pending, not merged to main branches |
| K17 / built-in DeleteProfile path fixed | Built-in in-process/HTTP clones preserve recordings on rejected deletion and retire selected cross-service profile reads after commit | W19/W03/W18 partial; other mutations, standalone responder policy, full dependency graph and concurrent/callback visibility remain open; see pipeline preflight |
| K18 / selected create/binding edges fixed | Committed CreateProfile and modeled Media bindings retire all three profile-read Actions; refused writes preserve recordings | W19/W10 partial; both transports, unrelated services and independent instances tested; full read graph and other mutations remain open |
| K27 / collision storage repaired | Distinct requests survive collision buckets, load/save and request-aware replay; key-only ambiguity returns None; report groups retain all rows | W19 remains partial; see [storage migration](../replay-storage.md) and [release evidence](release-0.17-cut.md#evidence). Lost historical entries cannot be recovered; full key/QName/protocol redesign remains open |
| K28 / URL credentials in keys fixed | Canonical projections and legacy loaded/caller keys strip URL credential pairs; load is read-only and explicit save persists cleanup | W19 partial; synthetic privacy/lookup/disk controls, unchanged JSON shape; raw-envelope redaction remains targeted; K27 storage retention is repaired separately; see pipeline preflight and storage migration |
| K29 / shared XML whitespace preservation fixed | Common escaping emits numeric references for CR/LF/tab in text and attributes; Fault serialization uses the same helper | W03/W06/W10 partial; both transports, shared helper and 34 external instances checked; raw token/nested renderers and field constraints remain open; see profile preflight |
| K30 / bounded policy corrected | Explicit empty Create/read selectors refuse before effects; invalid seeded lists return Receiver without snapshot repair | W01/W10/W06; both-transport state-after-error reproduction, raw/client payload, hook, replay and external policy-Fault controls in profile preflight |
| P2 / selected profile identity paired | Create/read/render/six binding paths now preserve decoded nonempty tokens through Media/PTZ workflows | W10 partial; other configuration fields, replay keys, capacity and complete semantics remain open; see profile preflight |
| A1 / typed adapter identity paired | Four exact Action identities use bounded qualified operation parsing; token whitespace is preserved and unrepresentable PTZ arguments do not invoke typed hooks | Public signatures/raw fallback unchanged; Transport/Responder controls in profile preflight. Other stream options, authorization, physical effects and broader adapter contracts remain open |
| K19 / reproduced external compatibility finding | Current Media source closures reject XSD 1.0 compilation in independent validators but compile under strict XSD 1.1 | W21; [schema preflight](mock-fidelity-schema-preflight.md), no schema edits or disabled checks; qualify the candidate tool and report the schema language explicitly |
| K20 / reproduced and formatter fixed | `auth::auth_fault` formerly emitted an unbound wsse subcode and raw reason text | W05 serializer migration preserves code/subcode, fixes scoped binding/text, and adds client/health/CLI controls; authentication parsing/policy remains W08 work |
| K21 / Python limitation; independent compilation available | Python reports a Device type-table warning; pinned Xerces compiles the same complete closure with full checking and warnings-as-errors | W21; independent generic qualification passes without schema edits or warning suppression. Does not prove the Python warning false or mock instances valid |
| K22 / client and selected mock selectors fixed | Full Media2 profile query omitted Type, while the mock returned configurations regardless; client now explicitly requests All | W06/W10; captured request regression fails on the old body. Scoped mock Token/Type selection now has two-transport and token-table controls; broader field/rendering rules remain P-E; XSD validity alone cannot catch the original request intent |

Additional output-closure findings are tracked in the source audit: K24 audio
codec vocabulary (W01/W10), K25 multicast read-only/effect meaning and K26 video
codec representability across shared service views (W01/W10/W17). All require
discriminating wire/state reproduction and explicit disposition before the
associated profile/configuration cards can close; source review is not runtime
verification.

K15 now has a bounded Name repair: both CreateProfile operations read scoped,
decoded scalar names and both profile renderers escape them once. Seeded-state
and HTTP/in-process create/read/refusal controls are implemented. Caller-supplied
profile tokens, configuration text and other field rules remain W01/W10 work;
this does not close the profile batch or bypass its remaining readiness checks.

## Verification commands

Run from repository root; build into an isolated directory, not the installed CLI.
`rtk` is this workspace's command proxy. Check native exit codes; a summary alone
is not evidence of success. Capture sanitized results with commit/toolchain/OS.

```powershell
rtk git status --short --branch
rtk git rev-parse HEAD
rtk rustc --version
rtk powershell -NoProfile -File docs/active/check-mock-fidelity-inventory.ps1 -SelfTest
rtk rg -n 'extract_tag|extract_all_tags|extract_attr|XmlNode|from_xml' src/mock
rtk rg -n 'resp_soap_fault|auth_fault|SoapError::Fault|subcode' src crates/oxvif-cli tests
rtk cargo fmt --all -- --check
rtk cargo clippy --workspace --all-targets --all-features --locked --target-dir target/mock-fidelity-build -- -D warnings
rtk cargo clippy --workspace --all-targets --locked --target-dir target/mock-fidelity-build -- -D warnings
rtk cargo test --workspace --all-features --locked --target-dir target/mock-fidelity-build --no-fail-fast
rtk cargo test --workspace --locked --target-dir target/mock-fidelity-build --no-fail-fast
```

When rustdoc links change, set `RUSTDOCFLAGS=-D warnings` in that process and run
both `cargo doc --no-deps` variants with locked dependencies and isolated target;
record the environment. Run doctests and the per-feature/MSRV/XML-unification
checks prescribed by `CLAUDE.md` at W24. `rg` is an audit aid, not a call-graph
proof: inspect aliases, wrappers and subtree consumers, including test fixtures.

W21 must provide the exact external-validator invocation after tool selection.
The existing `OXVIF_ONVIF_SCHEMA` ignored test is only a structural check and
cannot stand in for that gate. Do not invent a command or mark it passed now.

## Closure and handoff

An operation is DONE only when its card, C/R/F/B/V evidence, relevant cross-cutting
work and documentation agree. Attach exact test names and assertion intent,
negative controls, mutation/restoration evidence, commands/exit codes, commit,
features/toolchain/OS and external manifest hash. Record passed/failed/blocked/
not-run separately; schema skip and known `Broken`/`Blind` expectations are not
new conformance passes. Exceptions require a named scope, reason and disposition.

At each handoff record: completed work IDs and operation IDs; current commit and
dirty files; open findings; next unblocked work ID; tests still required; any
user decision or external prerequisite. Findings can be carried forward only
with an ID and owner work package. Resume from those records without chat history.

Current progress: W00 is complete; 191 direct readers remain indexed, not accepted.
K34's approved rate migration is tracked in the [VE1 plan](mock-fidelity-video-encoder.md).
VE1 now implements selectors, options, full candidates, codec views and capacity;
its dated evidence distinguishes local gates from hosted CI (VE1 run 34579594778 passed).
[AM1 audio/metadata](mock-fidelity-audio-metadata.md) implements the 15-operation
subgroup with the K36/D4 migration approved on 2026-09-11; see its dated gates.
VE1 does not close W10 or the overall programme.
Scoped synthetic request/auth boundaries, selected profile identity/effects,
11 acknowledgment-only policies, [PA1 assembly](mock-fidelity-profile-assembly.md)
and [VS1 source configuration](mock-fidelity-video-source.md)
and [VE1 encoder configuration](mock-fidelity-video-encoder.md)
and [AM1 audio/metadata](mock-fidelity-audio-metadata.md)
are implemented subgroups. **Next: remaining W10 URI/OSD/capability closure**,
followed by other service subgroups. W07 HTTP binding, remaining W04 typed/QName rules, broader W06
Fault migration and security semantics remain open. The [schema preflight](mock-fidelity-schema-preflight.md)
records working external tooling and selected corpus coverage, not whole-programme acceptance.

The planning revision does not select a release version, merge PR #16, publish,
push or install binaries. Final release acceptance requires M0–M6 evidence at a
recorded candidate commit and the applicable maintainer authorization. Keep the
requirement to notify the user before updating a release on their system.
