# Mock request pipeline preflight

[English](mock-fidelity-pipeline-preflight.md) | [繁體中文](mock-fidelity-pipeline-preflight_zh.md)

W02/W03/W06/W19 engineering checkpoint, 2026-09-10. Source baseline `9978220`.
This is a source-derived dependency map, not a normative protocol catalogue.
W02 and W03 remain PARTIAL across the programme; common synthetic validation is
implemented, while broad W06 service-error migration is not. Completed foundations are appended
below. No new product decision is required here.

| Section | Purpose |
| --- | --- |
| [Entry points and ownership](#entry-points-and-ownership) | Trace inputs through shared code |
| [Profile dependency closure](#profile-dependency-closure) | Parsing, rendering and side effects |
| [Compatibility constraints](#compatibility-constraints) | Preserve intentional raw behavior |
| [Implementation order](#implementation-order) | Prerequisites and concrete next slices |
| [Evidence and remaining work](#evidence-and-remaining-work) | Tests versus unverified findings |
| [Parsed-node implementation](#parsed-node-implementation) | P-A delivered scope and remaining W04 work |
| [Structured fault foundation](#structured-fault-foundation) | P-C serializer slice and consumer boundaries |
| [Committed deletion effects](#committed-deletion-effects) | Selected K17 repair and remaining replay boundaries |
| [Committed creation effects](#committed-creation-effects) | K18 create/read dependencies and refusal preservation |
| [Exact Action routing](#exact-action-routing) | K06 routing slice and remaining W03/W07 work |
| [Parsed synthetic boundary](#parsed-synthetic-boundary) | P-D implemented checks, evidence and exclusions |
| [State hook snapshot work](#state-hook-snapshot-work) | W18 bounded lock and observation policy |

## Entry points and ownership

The table is a historical source baseline. Current routing changes are recorded
under [Exact Action routing](#exact-action-routing).

| Source symbols | Baseline input/output and next dependency | Work owner |
| --- | --- | --- |
| `mock/transport.rs::soap_post` | Already a Rust String; URL ignored; builds `RequestCtx`, then `Chain::default_mock` | W03/W07 |
| `mock/server.rs::handle_soap` | `helpers::extract_action` reads only Content-Type; missing becomes empty; bytes become lossy UTF-8; constructs default or replay chain; every result returned as HTTP 200 | W03/W07; HTTP binding review remains required |
| `metamorph/replay.rs::MetamorphTransport::soap_post` | Builds `Chain::mock_with_extra` with ReplayResponder; no HTTP status layer | W03/W19 |
| `responder.rs::Chain::{mock_with_extra,respond}` | Fault → Auth → custom/replay responders → Synthetic; first Some(String) short-circuits; RequestCtx fields and Responder trait are public | W03/W06/W09/W19 |
| `FaultResponder::respond` | `FaultInjector::take_for_action` consumes a suffix-matched entry; `resp_soap_fault` escapes its code/reason | W05/W09; do not consume after auth accidentally |
| `AuthResponder::respond` | `auth::requires_auth` uses exact exemption URI; `validate_ws_security` reads four local names via legacy extract_tag; `auth_fault` has a separate raw reason formatter | W08/W05; not covered by ordinary Fault helper fix |
| `ReplayResponder::respond` | Action-tail write classification → family invalidation before synthetic execution; reads use canonicalize(Key), store.lookup(Action,key), response_raw | W19; K17 below |
| `canon::canonicalize` → `write_node` | Namespace-stripped XmlNode; mask_text/mask_attr; sorted attrs and collapsed text; parse failure falls back to collapsed raw string | W19; not a strict synthetic request reader |
| `SyntheticResponder::respond` → `dispatch` | Passes the original body; permissive Action routing; only migrated DeleteProfile handlers call required_text | W03/W04/W07 |
| `request::required_text` → `parse` | NsReader, scoped identities, bounded tree, one SOAP Body operation or standalone operation; direct scalar child | W04; attributes validated but discarded, no typed/repeated/subtree API yet |

No normal synthetic parse may silently become a validator for recorded responses
or all public custom responders. HTTP UTF-8/action extraction occurs before this
chain and needs its own acceptance; raw String preservation is not byte fidelity
for invalid HTTP encodings.

## Profile dependency closure

This extends the [13 profile cards](mock-fidelity-profile-preflight.md), not the
complete per-field audit of other services.

1. `extract_tag` → `find_open_tag/find_close_tag`: local-name scans, trimmed raw
   inner fragment, no entity decoding. `extract_all_tags` repeats this approach;
   `extract_attr` scans the first matching tag header and returns raw spelling.
2. `handle_create_profile` and `handle_create_profile_media2` now pass a decoded
   direct scalar Name to `create_profile_in_state`; both profile renderers escape
   this stored text once. Media1 still supplies a raw Token. The remaining token
   and configuration text migration must pair decoding with output escaping;
   decoding every legacy helper would corrupt subtree callers.
3. `resp_profiles/resp_profile` and `resp_profiles_media2` now collect profiles and
   catalogues in one `profile_snapshot` read guard; the former separate snapshots
   were reproduced as mixed revisions under concurrent writes. Both renderers read stored identity/text and
   call VSC/video/audio/PTZ renderers. Creation also reaches render_profile on
   Media1; Media2 returns the generated token. K15 fixes need both seeded literal
   state and client-created entity-looking text controls, not just one getter.
4. At baseline `apply_media2_configuration` synthesized per-entry XML and called
   bind/unbind repeatedly. The K16 state repair now passes an extracted-value
   plan into `apply_configuration_bindings`, validating the full plan under the
   write lock before changing slots. Scoped decoding remains a parser task; the
   shared writer no longer reinserts values into XML or reparses fragments.
5. `create_profile_in_state/delete_profile_in_state/bind_configuration` reach
   `MockState::modify[_returning]` → committed snapshot → `notify` → caller
   callback after releasing the state lock. Collection equality, callback counts,
   callback ordering and replay visibility are separate assertions; see the
   implemented hook slice below for its bounded guarantees.

**Initial K17 finding — built-in DeleteProfile repair below:** ReplayResponder inserts
the operation family into `invalidated` before SyntheticResponder can accept or
reject a write. A later fault therefore does not undo the invalidation. A new
strict synthetic rejection alone cannot guarantee unchanged replay visibility.
Owner W19/W03/W18; affected profile mutation cards: both CreateProfile/DeleteProfile,
Media1 Add/RemoveVideoSourceConfiguration and Add/RemoveVideoEncoderConfiguration,
Media2 Add/RemoveConfiguration. Other mutation families require the same audit.
Target regression: record a distinctive Media1 GetProfile read, reject a
DeleteProfile without state changes, read again and assert the recording still
wins; include a valid write control that retires it. No K17 fix is claimed by
the chain-order tests.

**K18 — initial create/list mismatch, now repaired for built-in creation below:** `replay::family` strips
only the leading verb; it does not model read/write dependencies. GetProfiles
maps to `Profiles`, but CreateProfile/DeleteProfile map to `Profile`. Binding
maps to `VideoSourceConfiguration`, `VideoEncoderConfiguration` or `Configuration`,
not to either profile read. Even a successful mutation can therefore leave a
recorded profile list active. Keys also omit service identity at invalidation
time (store lookup does include the full Action). Owner W19/W10; audit the same
13 cards' read/write edges before designing outcome-based invalidation. Target
tests: successful create/delete/bind → recorded GetProfiles must reflect the
changed profile state; unrelated service/instance recordings remain available.
Do not test only a naturally matching pair such as GetHostname/SetHostname.

The initial `metamorph`-gated K17/K18 tests in `mock_fidelity_known_gaps.rs` reproduced
both paths using project-authored raw identity markers, not schema fixtures.
K17 asserts the exact rejection, complete serialized device-state equality and
the unwanted switch from recording to synthetic. K18 asserts successful stored
creation, the stale list, retirement of the singular read, and independent-instance
preservation. Binding/service dependency edges are still source-only findings.
Temporarily suppressing DeleteProfile invalidation and additionally invalidating
Profiles on CreateProfile made the two baselines fail at their intended replay
assertions in a full all-feature `--no-fail-fast` run; both mutations were restored.
Those baseline assertions exposed defects, not an accepted invalidation design.
Both have since been converted to corrected invariants for built-in deletion and
creation; binding and full dependency acceptance remain open.
Restored local gate: formatting and both workspace Clippy modes passed; 1,169
all-feature and 1,087 default tests passed, with four ignored in each mode.
Inventory self-tests and source reconciliation passed unchanged. Hosted CI
[34458754641](https://github.com/smiti1642/oxvif/actions/runs/34458754641)
passed for the preceding authentication-Fault commit `e6145b3`, not this new slice.

## Compatibility constraints

- Keep `RequestCtx` and `Responder::respond` source-compatible while designing a
  private parsed request/outcome path. Do not add a required public struct field
  merely to cache parsing. Preserve original body for extensions and replay.
- Place normal synthetic validation at the synthetic boundary; fault injection,
  auth and replay ordering must be preserved or separately reviewed. Static
  handlers must eventually share validation, not bypass it by ignoring body.
- Resolve K17 with explicit successful-effect information before claiming failed
  writes are side-effect free. Do not guess success from XML text searches or
  retire all fixtures on any generic state hook. K18 needs explicit affected-read
  dependencies, not just a delayed verb-stripped family key. K16's selected
  binding atomicity is repaired separately; deferring invalidation is not rollback.
- Preserve `SoapError::Fault` code/subcode/detail meanings. `soap::find_response`
  exposes only the first Subcode. `health::CheckError::from` copies it;
  `CheckError::is_auth`, health assessment/JUnit, CLI application diagnostics,
  and `metamorph::parse::extract_fault` consume the result. Nested faults require
  tests of these consumers, not just the fault renderer. No public error field
  or CLI exit-code change is approved by this checkpoint.
- Auth still has its own legacy parser and formatter; strict DeleteProfile and
  escaped `resp_soap_fault` do not establish auth security or authorization.

## Implementation order

| Slice | Exact next work and acceptance | Prerequisite |
| --- | --- | --- |
| P-A | Private parsed-node accessors for decoded text, scoped attrs and ordered repeated children; generic namespace/normalization/resource controls; keep standalone operation test entry | W04 selected dependencies above; no whole-program W02 completion claim |
| P-B | Review official per-field/Fault references externally; pin source closure/hash record; resolve the 13 cards' defaults, repeats, capacity/conflicts and state effects | W01; do not substitute this source table for WSDL/XSD evidence |
| P-C | Structured Fault foundation and selected DeleteProfile commit observer implemented; finish other mappings, effects and consumer review before broad defaults change | W05/W06/W19; no implicit public trait redesign |
| P-D | Route both synthetic entry points through one parsed request; reject Action/body mismatch and malformed input, including static reads, while retaining chain controls | P-A/P-C plus W07 binding review and K17 handling |
| P-E | Migrate profile read/create/delete/binding in bounded commits; decode on input and escape once on output; atomic validation and no rejected-write notifications | P-B/P-D; K13–K16 tests become corrected invariants |

These are sub-slices of existing work IDs, not replacement milestones. External
schema CI remains W20–W22; the newly wired inventory job checks source drift only.

P-D implementation contract (original target; delivered subset recorded below):

- Resolve one private route from the complete supplied Action and use that same
  route for operation QName validation and dispatch. Events Action suffixes and
  DeviceIO namespace casing must follow existing client request construction;
  source-wide client round trips must distinguish routing from payload acceptance.
- Build one owning request at the synthetic boundary, after fault/auth/raw/replay
  responders. Pass its borrowed operation to migrated handlers, starting with
  DeleteProfile, without reparsing. Keep public RequestCtx/Responder unchanged.
- Reject malformed XML and Action/body disagreement for static and stateful
  handlers alike. Explicitly test SOAP container multiplicity/text/order, prefix
  aliases and Header decoys. Standalone operation support in the private parser
  remains distinct from the pending HTTP envelope policy.
- Match typed errors to reviewed generic faults. Resource/policy limits need an
  explicitly documented mock-specific boundary, not a fabricated ONVIF hardware
  limit or a diagnostic-string classifier. Preserve public client error meanings.
- Exercise normal HTTP and in-process paths, exact fault payloads, complete state
  and notifications; keep malformed raw responder and committed-effect controls.
  Legacy replay's pre-write effects remain W19 and cannot be claimed fixed here.
- Repair test-only bare fragment producers before interpreting response coverage:
  dispatch source sweeps, the legacy structural corpus, and Metamorph quirk
  baselines need named disposition. Do not make all responses generic faults and
  count that as a passing success-payload namespace/schema sweep.

## Evidence and remaining work

Two `mock::responder::tests` controls guard the existing extension seam:

- `fault_precedes_auth_and_extras_even_for_malformed_input`: first response has
  the exact injected fault code/reason, second has the exact auth fault after
  one-shot consumption, and the custom responder was never called.
- `extras_preserve_raw_input_order_and_short_circuit_output`: first extra passes,
  second answers, third is not called; request text and response string remain
  exactly unchanged, including CRLF, entities, whitespace and intentional defects.

These are in-process ordering tests, not HTTP, real replay-store, schema or
conformance acceptance. Other services' transitive readers, field contracts,
and external validation remain open. K17/K18 runtime evidence is recorded below;
remaining mutation fixes remain open; the selected deletion repair is recorded below.

Local evidence: Windows, rustc 1.97.0, PowerShell 7.5.4, isolated
`target/mock-fidelity-build`. Both new tests passed, then failed at the intended
assertions after changing the injected code and trimming captured request text;
both changes were restored. Formatting, default/all-feature workspace Clippy and
tests passed: 1,148 all-feature / 1,068 default, 4 ignored each. The inventory
self-tests/reconciliation, 192 local file links and 10 new section anchors passed.
No runtime implementation, public API, installation, release or real-device
behavior changed in this checkpoint. Resume with P-A; P-B/P-C remain prerequisites
for broad handler migration. Do not repeat completed W00 inventory work.

## Parsed-node implementation

P-A now has a private owning `Request` and borrowed `Node` accessors in
`src/mock/request.rs`: unique direct child, ordered repeated children, scalar
text, required nonempty child text, and expanded-name attributes. `required_text`
uses this representation without changing the two DeleteProfile contracts.
The earlier source table describes baseline `9978220`; attributes are now retained
instead of discarded. The staged attribute accessor has a narrow non-test
dead-code allowance until handler migration; remove it when first used there.

Generic controls cover attribute identity/default namespace, XML whitespace and
character-reference normalization, repeated/subtree scope, absent versus empty
versus structured values, reuse of the same tree, normalized namespace aliases,
and accepted depth/node boundaries. Seven new tests were perturbed individually
within one run and each failed at its intended assertion, then restored.
References: [XML 1.0 §2.11/§3.3.3](https://www.w3.org/TR/REC-xml/) and
[Namespaces §6.2](https://www.w3.org/TR/xml-names/#defaulting).

This is not complete W04 or global parse-once dispatch: schema-specific types,
ranges, QName-valued content, extension policy and P-B/P-C remain open. Existing
resource bounds are retained, not newly justified as device capacity limits.
No new dependency, public API, schema table or service operation is introduced.

P-A local gate: formatting, both workspace Clippy modes, 1,155 all-feature and
1,075 default tests passed (4 ignored each); inventory remains 159 declaration
sites / 157 routes / 260 direct reader occurrences. These counts include the
existing known-gap assertions and do not close K13–K18.

## Structured fault foundation

The later P-D prerequisite replaces the private string-only `RequestError` with
28 explicit variants. Parsing and scoped accessors select variants at the point
of failure; `message()` preserves the existing static diagnostics used by the
two DeleteProfile handlers. No request payload is stored in an error. This is an
internal representation change, not global validation or a new SOAP fault mapping.
Future boundary policy must match variants rather than diagnostic substrings.

Sensitivity: temporarily returning `MissingField` for a structured scalar made
`absent_empty_scalar_and_subtree_are_distinct` and
`misleading_nested_fields_are_not_selected` fail at enum payload assertions in
the full workspace all-feature no-fail-fast run (log `1789042352_cargo_test.log`).
The mutation was restored. Existing malformed-input diagnostic and DeleteProfile
wire regressions remain in place. Formatting and both workspace Clippy modes
passed; restored suites passed 1,184 all-feature and 1,100 default tests (5
ignored each), and inventory self-tests/reconciliation passed unchanged. This
internal-only slice does not expand the externally validated instance corpus.

P-C has a private `Fault` representation with typed Sender/Receiver code,
ordered subcodes and reason. Trusted QName definitions are compile-time ASCII
NCNames; each Value carries its own namespace binding, so repeated prefixes with
different namespaces cannot contaminate one another. This is not an arbitrary
vendor/injection QName API. Literal reason text is escaped once, XML-invalid
characters become U+FFFD, and CR is serialized as a character reference.

Only `auth::auth_fault` and the empty-chain defensive Receiver response migrate
in this slice. K20 was reproduced before the fix: the auth QName was unbound,
and an XML-shaped reason inserted a child and changed text. Both tests failed at
their intended assertions. The fix keeps `s:Sender`, `wsse:FailedAuthentication`
and the existing credential-policy decisions. Fault injection and ordinary service
fault mapping remain on the legacy path. Authentication request parsing remains
the old local-name reader; this is not W08 security acceptance.

Nested generic Fault controls independently inspect QName scope/depth and assert
that the client and health consumers retain the FIRST subcode, not the deepest
one. Authentication remains classified as an auth failure; the generic nested
error does not. The CLI subprocess control uses a loopback auth-enforced mock,
JSON and plain-table modes: exit 20, DEVICE_CONNECTION_FAILED, exact reason and
non-retryable classification are preserved. No real camera or credentials are used.

The two auth controls failed before implementation. A subsequent full-workspace
mutation run changed the auth code and the two generic fixtures: the auth payload,
both generic Fault tests, existing chain-order control and CLI JSON control all
failed at the intended assertions. All mutations were restored. No narrowed test
filter hid the CLI target from this mutation run.

SOAP design reference: [SOAP 1.2 Part 1 §5.4](https://www.w3.org/TR/soap12-part1/#soapfault).
Optional structured Detail, complete ordinary code mappings, HTTP status,
remaining post-success replay outcomes and broad nested-consumer coverage are still open.
No public `SoapError` field is added or redefined; its docs now state the existing
first-level/prefix-preserving subcode and extracted-text Detail limitations.

Local acceptance: formatting, default/all-feature workspace Clippy, 1,167
all-feature and 1,087 default tests passed (4 ignored each); Rust 1.88 workspace
check passed. Inventory remains 157 routes / 159 Action sites / 260 direct readers.
Prior checker commit `9469bb6` passed all 23 jobs in CI run 34456850826; that hosted
run does not cover this subsequent Fault change.

## Committed creation effects

Both CreateProfile route arms now pass the same private effect slot used by
deletion. Only `CreateOutcome::Created` sets `ProfilesChanged`; typed Name
refusals and duplicate-token refusals do not. Built-in replay defers legacy
invalidation for exactly those two additional Actions. Committed creation retires
Media1 GetProfile/GetProfiles and Media2 GetProfiles using complete Action
identities, rather than the singular `Profile` family. This conservatively retires
all recorded singular profile reads; it is not token-specific invalidation.

`tests/mock_replay_effects.rs` adds HTTP/in-process controls for both service
creations, Name and duplicate-token refusals, full state equality, all three read
views, unrelated service recordings and independent instances. K18's successful
creation/stale-list baseline is now a corrected state/list regression. The shared
chain's existing fault/auth/raw-response ordering is unchanged; its deletion
observer test remains the generic short-circuit control, not a new CreateProfile
authentication acceptance test. The observer still runs after the state hook;
callback/concurrent visibility is not made transactional. Standalone public
ReplayResponder, bindings and additional read dependencies remain W19 work.

## Committed deletion effects

Initial deletion-effect slice: P-C/W19 carries an optional private `Effect` alongside synthetic XML.
The existing two DeleteProfile route arms pass a per-request effect slot; only
their `Deleted` branch sets `ProfilesChanged`. The terminal invokes its private
observer after the handler completes, not by scanning XML or subscribing to a
generic persistence hook. `RequestCtx` fields and `Responder::respond` are unchanged.
The ordinary `dispatch` wrapper still returns only XML for internal analysis users.

Built-in MetamorphTransport and HTTP replay clones defer invalidation for the two
exact DeleteProfile Actions. A committed deletion retires Media1 GetProfile and
GetProfiles plus Media2 GetProfiles by complete Action identity. Missing/fixed
refusals retain recordings; other-service lookalikes and independent instances
remain untouched. K17's old known-gap test is now a preservation regression.
`tests/mock_replay_effects.rs` exercises both service writes, both transport paths,
all three selected read views, full state equality and unrelated/instance controls.
`committed_effect_observer_is_not_a_request_or_fault_hook` checks injected Fault,
auth, raw custom response, invalid input, successful deletion and repeated refusal.

The public standalone ReplayResponder constructor retains its existing policy:
it cannot observe a caller-owned downstream responder. Built-in clones enable the
private commit-aware path only where the terminal is owned. This is a staged
migration, not a public configuration switch or whole W19 acceptance. Other
mutations other than the creation slice above still use the old family invalidation.
Additional profile-dependent reads, malformed Action handling, callback
ordering and concurrent linearizability remain open. The effect observer runs
after the existing state-change callback; this slice does not make that callback
and replay invalidation atomic or add rollback.

The inventory extractor now accepts multiline dispatcher parameter lists,
with an additional positive self-test and all prior rejection checks retained.
The two ledger argument lists include the effect slot; the route/Action/reader
counts remain unchanged. No schema-derived metadata or new runtime dependency
is introduced.

Deletion-effect verification: restoring early invalidation made K17 and both
transport regressions fail; suppressing committed-effect delivery made the
observer control and both transport regressions fail. Both full-workspace,
all-feature no-fail-fast mutation runs exited unsuccessfully at assertions; all
mutations were restored. Formatting, both workspace Clippy modes, narrow mock
Clippy, both warnings-as-errors documentation builds and all 24 packaging
controls passed. The restored suite passed 1,174 all-feature and 1,090 default
tests (5 ignored, 20 suites each). A fresh external corpus contained 34 XML
instances, all accepted by pinned strict Xerces XSD 1.1 validation; this is not
semantic conformance or whole-programme acceptance.

## Exact Action routing

K06 routing sub-slice, based on `4bcb024`: `respond_with_effect` now splits the
last separator and matches the whole preceding service/port identity. Events
operations additionally belong to their particular port; the shared dispatcher
is not permission to accept another port's operation tail. Public client/session
Action declarations and the 157 routed operations are unchanged.

The existing source Action inventory supplies the identity set; pinned Events
and WSN resources were checked externally. [Basic Profile 2.0 R2744 and R2900](https://docs.oasis-open.org/ws-brsp/BasicProfile/v2.0/BasicProfile-v2.0.html)
support equality of declared Actions. R2757 separately permits omission of the
Content-Type action parameter: this routing repair does **not** implement that
HTTP fallback. SOAP 1.2 status selection, media type/encoding, WSA consistency,
body identity, parse-once dispatch and generic fault mappings were open at this
routing checkpoint; the later P-D section records their implemented subset.

The source-wide `action_aliases_never_reach_a_service_handler` control mutates
every extracted client Action in five ways and asserts the exact existing
unsupported response, absent committed effect and complete unchanged state.
The two `*_action_identity_rejects_aliases_before_state_changes` controls add
wrong Events ports and an actual hostname write through HTTP/in-process entry
points, checking that only the canonical write notifies. All three failed at
payload assertions against the old routing in the full all-feature no-fail-fast
run (local log `1789041634_cargo_test.log`); the hostname probes exposed an actual
unwanted write. Existing source-wide positive routing and chain-order tests are
retained. Formatting, both workspace Clippy modes and both warnings-as-errors
documentation builds passed. Restored workspace suites passed 1,184 all-feature
and 1,100 default tests (5 ignored, 21 suites each). Inventory self-tests and
159 Action sites / 157 routes / 260 readers remain unchanged. Fresh external
corpus 07 passed strict Xerces XSD 1.1 validation for all 34 selected instances;
neither the corpus nor ignored tests imply whole-programme acceptance.

Scope: normal synthetic routing only. Public raw responders, suffix-based fault
injection and legacy replay invalidation remain separate work. The unknown-Action
fault retains its existing flat code/reason; it is not accepted as the final
normative fault contract. No public API or installed binary changes.

## Parsed synthetic boundary

P-D slice based on `4f04f3b`: a private source-derived `Route` owns service/port,
operation and body identity. Both normal synthetic entry points parse once after
fault/auth/custom/replay precedence, then check the identified operation before
either static or stateful dispatch. DeleteProfile borrows that operation rather
than reparsing; `required_text` is now test-only. Other handlers still use legacy
field readers. The source index is now 159 Action sites, 157 routes and 258 direct
reader occurrences (243 production, 15 test; 77 production enclosing symbols).

Checks cover bounded XML parsing, one operation, namespace identity, SOAP 1.2
Envelope with optional Header before Body, whitespace-only container text and
qualified header blocks. Qualified standalone operations remain supported by the
shared engine; this is not acceptance of standalone payloads as an HTTP SOAP
binding. Thirty typed diagnostics select generic faults without string matching.
Core generic faults are distinguished from the explicitly mock-specific
`urn:oxvif:mock:error` resource/DTD policies. Limits remain 2 MiB of UTF-8 text,
depth 64 and 16,384 nodes; these do not represent advertised hardware capacity.
See the public [request boundary](../mock-server.md#synthetic-request-boundary)
for the supported scope.

`mock_request_boundary` exercises static reads and hostname writes through HTTP
and in-process paths. It asserts full unchanged serialized state, no hook on
rejection, exact fault payloads and independently resolved Code/Subcode QNames;
valid prefix aliases and a committed write remain positive controls. The byte
limit has a separate in-process control; HTTP byte-limit mapping is not accepted.
The new quirk control proves malformed stored requests are not normalized and
the recorded request/response remain unchanged when the stricter baseline faults.
Normal source-routing and legacy shape probes now supply identified operation
XML instead of empty strings or bare fragments, avoiding generic-fault-only runs.

Evidence: both initial boundary controls failed against the old implementation
(`1789042812_cargo_test.log`). Disabling envelope-version/operation identity checks
failed five controls, including the quirk baseline and actual DeleteProfile
identity protection (`1789043524_cargo_test.log`). Mutating the fault namespace
and byte-limit classification failed four controls, including expanded QName and
resource-policy assertions (`1789043659_cargo_test.log`). Both mutations were
restored. The restored full workspace all-feature run passed 1,188 tests with
5 ignored in 22 suites; the default run passed 1,103 tests with 5 ignored in
22 suites. Formatting, both workspace Clippy modes, inventory self-tests and both
warnings-as-errors documentation builds passed.
Fresh external corpus 08 passed strict Xerces XSD 1.1 for its 34 selected
instances; it does not include the new generic boundary faults. The legacy
external shape probe passed all unchanged zero-finding pins: 158 responses,
111 success payloads, 47 faults, 1,242 anchors, 1,431 skipped children and 398
checked attributes. More payloads are reached after wrapping its previously bare
fields; those legacy request fields are not a normative request corpus.

Remaining W03/W07/W08/W19 work includes HTTP action fallback/consistency, media
type/encoding/status/endpoint rules, SOAP mustUnderstand/encodingStyle and
attribute policy, processing instructions/outside comments, scoped WSSE/auth,
operation-specific field semantics, ordinary service faults and replay effects.
Raw fault/custom/replay responses retain their earlier precedence and bytes.
No public API, error type, CLI exit code or installed binary was changed.

## State hook snapshot work

Implemented W18 slice, baseline `c6af85b`: `MockState::notify` formerly invoked
hooks under a read lock acquired after releasing the mutation's write lock. Both
the blocked reentrant write and wrong-snapshot observation were reproduced by
new controls against that implementation (`1789045613_cargo_test.log`).

The shared mutation helper now captures an owned `DeviceState` snapshot within
the original write lock only when a hook is registered, then invokes callbacks
after releasing the lock. Conditional commit predicates retain outside-lock
evaluation and refused outcomes do not notify. Public signatures are unchanged.
`change_hooks_release_state_lock_before_bounded_reentrant_writes` probes lock
availability before attempting a bounded nested mutation, so regressions fail
promptly rather than hanging. It covers modify, returning and conditional entry
points, exact outer/nested snapshots, return values and rejected outcomes.
`conditional_change_hook_retains_its_commit_snapshot_after_an_intervening_write`
deterministically interposes another mutation via the outside-lock predicate;
the delayed notification must carry the first mutation, including non-persisted
event state, not a fresh read of the second. No scheduling or sleep is required.

Callbacks are not serialized across threads and may run out of commit order.
Persistence owners must coordinate mutations or version their stored snapshots;
callback owners must prevent their own infinite recursion. This fixes the
selected K08 lock/snapshot defects, not every W18 queue/read snapshot or W19
replay visibility boundary. No protocol response or official corpus was changed.

Restored verification: 1,192 all-feature and 1,107 default workspace tests passed
(5 ignored, 23 suites each), both Clippy modes, formatting, both warnings-as-errors
documentation builds and unchanged 159/157/258 inventory self-tests passed.
Existing HTTP/in-process committed-effect and state-hook controls stayed green.
No new external schema acceptance is claimed for this state-only slice. Prior
selector commit `c6af85b` passed hosted CI run 34480290494.
