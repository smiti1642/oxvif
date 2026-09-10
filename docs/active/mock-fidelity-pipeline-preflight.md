# Mock request pipeline preflight

[English](mock-fidelity-pipeline-preflight.md) | [繁體中文](mock-fidelity-pipeline-preflight_zh.md)

W02/W03/W06/W19 engineering checkpoint, 2026-09-10. Source baseline `9978220`.
This is a source-derived dependency map, not a normative protocol catalogue.
W02 remains PARTIAL across the programme; W03 default validation and broad W06
service-error migration are **not implemented**. Completed foundations are appended
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

## Entry points and ownership

| Source symbols | Current input/output and next dependency | Work owner |
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
2. `handle_create_profile` and `handle_create_profile_media2` pass that raw Name
   to `create_profile_in_state`, which stores it unchanged. Media1 also supplies
   a raw Token. Therefore adding output escaping alone can double-escape ordinary
   client input; decoding every legacy helper would corrupt subtree callers.
3. `resp_profiles/resp_profile` and `resp_profiles_media2` collect profiles and
   catalogues in separate snapshots. Both renderers read stored identity/text and
   call VSC/video/audio/PTZ renderers. Creation also reaches render_profile on
   Media1; Media2 returns the generated token. K15 fixes need both seeded literal
   state and client-created entity-looking text controls, not just one getter.
4. `apply_media2_configuration` extracts entries, resolves kinds, synthesizes
   ProfileToken/ConfigurationToken fragments, then calls bind/unbind per entry.
   Future shared mutation helpers must receive values or borrowed parsed nodes,
   never a decoded value reinserted into an XML string. K16 atomic validation must
   complete before mutation; extra per-entry parsing cannot provide that guarantee.
5. `create_profile_in_state/delete_profile_in_state/bind_configuration` reach
   `MockState::modify[_returning]` → `notify` → caller callback under a read guard.
   Collection equality, callback counts, and replay visibility are separate
   assertions. Changing this generic helper globally would affect other services.

**K17 — reproduced, not fixed:** ReplayResponder inserts
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

**K18 — create/list mismatch reproduced, not fixed:** `replay::family` strips
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

The `metamorph`-gated K17/K18 tests in `mock_fidelity_known_gaps.rs` now reproduce
both paths using project-authored raw identity markers, not schema fixtures.
K17 asserts the exact rejection, complete serialized device-state equality and
the unwanted switch from recording to synthetic. K18 asserts successful stored
creation, the stale list, retirement of the singular read, and independent-instance
preservation. Binding/service dependency edges are still source-only findings.
Temporarily suppressing DeleteProfile invalidation and additionally invalidating
Profiles on CreateProfile made the two baselines fail at their intended replay
assertions in a full all-feature `--no-fail-fast` run; both mutations were restored.
These tests expose defects and do not implement or accept an invalidation design.
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
  dependencies, not just a delayed verb-stripped family key. Partial K16 writes also need
  resolution; deferring invalidation by itself is not transaction rollback.
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
| P-C | Define structured internal Fault/outcome and verify client/health/CLI compatibility before switching defaults; decide the private hook for post-success replay invalidation | W05/W06/W19; no implicit public trait redesign |
| P-D | Route both synthetic entry points through one parsed request; reject Action/body mismatch and malformed input, including static reads, while retaining chain controls | P-A/P-C plus W07 binding review and K17 handling |
| P-E | Migrate profile read/create/delete/binding in bounded commits; decode on input and escape once on output; atomic validation and no rejected-write notifications | P-B/P-D; K13–K16 tests become corrected invariants |

These are sub-slices of existing work IDs, not replacement milestones. External
schema CI remains W20–W22; the newly wired inventory job checks source drift only.

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
their fixes remain open.

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
post-success replay outcomes and broad nested-consumer coverage are still open.
No public `SoapError` field is added or redefined; its docs now state the existing
first-level/prefix-preserving subcode and extracted-text Detail limitations.

Local acceptance: formatting, default/all-feature workspace Clippy, 1,167
all-feature and 1,087 default tests passed (4 ignored each); Rust 1.88 workspace
check passed. Inventory remains 157 routes / 159 Action sites / 260 direct readers.
Prior checker commit `9469bb6` passed all 23 jobs in CI run 34456850826; that hosted
run does not cover this subsequent Fault change.
