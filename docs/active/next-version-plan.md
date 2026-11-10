# Next version implementation plan

[English](next-version-plan.md) | [繁體中文](next-version-plan_zh.md)

Status: IN-PROGRESS. Code walkthrough: 2026-10-08, committed baseline
`afe37316ac60aa8c4470f8608a6eab7fa1450692` (workspace version 0.17.0).
Candidate release: 0.18. The user authorized implementation on 2026-10-08 after
planning commit `2bad316`; versions, device writes, publication, push and background
scheduling remain outside this authorization.

The next version should make real-device failures bounded and explainable,
and add a limited offline reference comparison to Metamorph. Deliver the
runtime work first; do not make the entire Mock fidelity programme a release
prerequisite. Implementation status remains TODO until each exit criterion has
evidence.

## Scope and ownership

| Slice | Existing owner | Treatment |
| --- | --- | --- |
| SOAP transport and response parsing | Library; F15 runtime consumer, F07 affected verification | Required P1/P2; newly identified client-side work, not a claim that W07 request validation is absent |
| Session diagnostics and CLI execution | F15 R2/R4, Q08 | Required P3/P4; reuse delivered retry classification and schema tests |
| Real-device regression evidence | F15 R5 compatibility, F10 snapshot evidence where applicable | Required P5 infrastructure; actual device acceptance tracked separately |
| Offline reference comparison | F14 M7, Q10 | Bounded P6 feature; keep M4 persona/control transition separate |
| Existing HTTP binding changes | F05 W07 / Q02 / HB1 | Preserve separate ownership and acceptance; no integration or completion claim here |
| Device writes, playback, distribution, navigation extraction | F08/F09/F12 | Deferred; hostname preview/apply remains a later product slice |

The [post-0.17 backlog](post-0.17-backlog.md) remains the programme index.
This is its next-version cut and code map, not another copy of F01–F15.
The [autonomous queue](autonomous-completion-schedule.md) retains its historical
scope; this planning request does not resume that queue or inherit its push policy.

## Code walkthrough

Paths are relative to the repository. Line references describe the walkthrough
baseline; use the named symbols when subsequent commits move lines.

| Finding | Implementation and existing controls | Consequence |
| --- | --- | --- |
| C1 Fixed inner timeout | [transport.rs](../../src/transport.rs), `HttpTransport::new` / `with_root_certificates_pem` / `soap_post` (111–224); [application.rs](../../crates/oxvif-cli/src/application.rs), `build_http_transport` (1728) does not pass `ExecutionOptions.timeout` | Increasing CLI timeout above ten seconds does not extend the inner HTTP deadline. Root-certificate reconstruction must preserve future options. |
| C2 Entire response buffered | `soap_post` calls `response.text()` before status classification; [snapshot.rs](../../src/health/snapshot.rs), `fetch` bounds accumulated chunks to 16 MiB | SOAP has no explicit body-byte cap here, including non-success response bodies. Snapshot policy cannot be assumed to cover SOAP. |
| C3 Compatibility parser is permissive | [xml.rs](../../src/soap/xml.rs), `XmlNode::parse`, `from_bytes_start`, `parse_soap_body`; [xml_compat.rs](../../tests/xml_compat.rs), `malformed_and_partial_inputs_keep_existing_policy` explicitly accepts unfinished/multiple-root inputs | Completeness changes are compatibility changes. Public `XmlNode` strips namespaces and cannot prove namespace correctness. The separate Mock request parser already has byte/depth/node bounds. |
| C4 Initialization loses context | [session.rs](../../src/session.rs), `OnvifSessionBuilder::build` (123), `fill_missing_service_urls` (164) | Clock-sync failure aborts; missing optional URLs trigger best-effort `GetServices` whose failure is discarded. Current routing fills Recording/Search/Replay/Media2/DeviceIO only. |
| C5 Retry classification is shared, delays are not | [application.rs](../../crates/oxvif-cli/src/application.rs), `is_retryable_onvif_error`, `HealthRetryTransport`, `retry_delay`; [maintenance.rs](../../crates/oxvif-cli/src/maintenance.rs), `onvif_step` uses fixed 100 ms delay | Reuse existing classification. Single SOAP calls, complete diagnostics, whole Health runs and maintenance stages have different retry units; a common helper must preserve them. |
| C6 Metadata schema is closed | [contract.rs](../../crates/oxvif-cli/src/contract.rs), `SCHEMA_VERSION = "3"`, `ResultMeta`; [envelope schema](../../crates/oxvif-cli/schema/oxvif-envelope.schema.json), `$defs/meta.additionalProperties = false` | Adding attempts to `meta` is not compatible with the published v3 validator. Version and schema migration must be explicit. |
| C7 Trace and capture are different surfaces | [client/mod.rs](../../src/client/mod.rs), `OnvifClient::call` (200–202) traces raw request/response; [capture.rs](../../src/health/capture.rs), `CaptureTransport`; [redact.rs](../../src/redact.rs) | CLI sanitized verbosity does not sanitize Library trace. Health capture redacts request Password/Nonce but retains response/Fault/error material and groups by Action suffix. Do not advertise it as a safe support bundle. |
| C8 Legacy fixture recorder overwrites identities | [fixtures.rs](../../src/fixtures.rs), `safe_action_name`, `CapturingTransport`; [record_fixtures.rs](../../examples/record_fixtures.rs) reads first profile/source; [fixture.rs](../../src/metamorph/fixture.rs), `record`, `lookup_request` preserve collision buckets | Use param-aware Metamorph stores for new multi-profile regression evidence. The tracked fixture directory contains README only; this says nothing about private captures outside the checkout. |
| C9 Structural diff is not reference-value comparison | [quirk.rs](../../src/metamorph/quirk.rs), `element_paths`, `diff_details`, `QuirkDiff::compute`; [parse.rs](../../src/metamorph/parse.rs), `parse_check`, `ParseReport::all_parsed` | Shape diff ignores values; display normalization masks tokens/addresses. `all_parsed` permits Faulted/Unverified. None may imply equal reference values or full conformance. |
| C10 Existing inventory comparison is already useful | [maintenance.rs](../../crates/oxvif-cli/src/maintenance.rs), `compare` (1042), `diff_value`; [registry.rs](../../crates/oxvif-cli/src/registry.rs), `with_mutation` | `config.diff` checks device identity and reports incomparable sections. Preserve it. Atomic registry replacement exists; crash durability is separate F15 work, not a proven defect from this walkthrough. |

Existing controls to retain: transport 200/400/500 and Digest tests; Session
clock-before-capabilities and no-overwrite service URL tests; CLI recovery,
cancellation, Fleet ordering and six-outcome JSON/JSONL schema matrix;
`tests/mock_replay_key_gaps.rs` collision/full-Action tests; XML Unicode/entity
compatibility fixtures. Source inspection is not a fresh execution result.

## Decisions for this cut

1. Preserve `Transport::soap_post -> Result<String, TransportError>` and custom
   transport injection. Add configuration beside it, not a mandatory trait method.
2. Keep `OnvifSessionBuilder::build()` and `with_clock_sync()` behavior. Add
   report-bearing initialization; do not silently turn `auto` clock sync into a
   best-effort authentication fallback.
3. Preserve v3 structured stdout in the required runtime slice. Put attempt
   details on sanitized stderr and inside extensible diagnostic data only where
   its command schema permits them. New common `meta` fields require a separately
   reviewed v4 migration; do not merely loosen the v3 schema.
4. Keep local-name compatibility of existing typed parsers. Add a complete,
   bounded production SOAP parse path, while retaining documented permissive
   direct `XmlNode::parse` behavior. No blanket namespace migration in P2.
5. Keep fixture storage and replay key format. A comparator must pair full
   Action plus proven request identity and preserve ambiguity.
6. Do not treat synthetic schema instances, Mock capacity smoke, successful
   image signatures, or same-parser comparisons as real-device conformance.

## Delivery order

| Package | Prerequisites | Deliverable | Status |
| --- | --- | --- | --- |
| P0 | None | Freeze implementation input and compatibility cases | DONE |
| P1 | P0 | Configurable HTTP deadline and bounded SOAP bodies | DONE (local); cap validation awaits P5 |
| P2 | P0; P1 for combined network checks | Complete bounded production XML parsing | DONE (local) |
| P3 | P1/P2 | Report-bearing Session initialization and safe trace | DONE (local) |
| P4 | P1/P3 | Shared delay policy and CLI diagnostic integration | DONE (local) |
| P5 | P0; acceptance after P1–P4 | Param-aware regression harness and device evidence | TODO |
| P6a | P2; P5 synthetic corpus | Offline pairing and explicit comparison outcomes | TODO |
| P6b | P6a | Selected typed projections and usable comparison example | TODO |

P5's harness can start alongside P1/P2 without a camera. Missing hardware inputs
do not block local tests or document completion; they block the corresponding
compatibility claims. P6 is an independent feature cut, not grounds to leave
finished runtime repairs unpublished indefinitely.

## P0 Implementation baseline

- Record the chosen commit and dirty-worktree disposition before code changes.
  At this walkthrough HB1 modifies CHANGELOG, public Mock docs, lib/mock files
  and adds `http_binding.rs` plus tests; it is uncommitted and IN-PROGRESS.
  Do not absorb those files into the planning commit or declare HB1 complete.
- Separate known shipped, committed-unreleased and local-uncommitted behavior.
  The manifests remain 0.17.0 until an actual release decision.
- Capture fixture/CLI compatibility cases, enum matches, schema consumers and
  downstream compilation points before introducing new errors or reports.

Exit: an explicit implementation revision, preserved unrelated changes, and a
consumer map covering Library, CLI, Health, record/replay and examples.

Implementation input: `2bad316`, with the existing HB1 tracked diff and four
untracked files retained separately. Consumers reviewed: Library/Health transport
construction, CLI `build_http_transport` and typed retry classification, Health
capture's exhaustive `TransportError` match, recording/examples, and the sibling
oxdm Session custom transport (no exhaustive error match found). No oxdm changes.
Public error additions require downstream exhaustive-match migration. The v3
closed metadata schema and permissive direct XML tests remain pinned controls.

## P1 HTTP options and response bounds

Entry points: `HttpTransport` constructors and `soap_post`, CLI
`build_http_transport`, plus Health's default transport construction.

- Introduce one options source used by all reqwest client construction paths.
  Default timeout remains ten seconds; the CLI passes its per-attempt timeout.
  Private CA and credentials retain their behavior regardless of builder order.
  The CLI's outer timeout still bounds a whole attempt including Digest challenges.
- Proposed initial SOAP cap: 4 MiB of accumulated response bytes. Confirm this
  against P5 captures before shipping; it is a design value, not a measured device
  maximum. Library users may choose a positive bounded cap; no new CLI knob is
  required for the first slice. Reject invalid options before sending.
- Read chunks with overflow-safe accounting. Content-Length can reject early,
  but cannot replace the actual-byte counter. Bound success, Fault and unexpected
  HTTP bodies equally; document the counted representation and UTF-8 behavior.
- Use a typed non-retryable size failure, not HTTP 413/503 invented locally.
  Public `TransportError` is exhaustively matchable today: a new variant needs
  minor-version migration notes and updates to CLI/Health classifiers.
- Keep SOAP Fault body handling at 200/400/500. Do not turn this package into a
  redirect/proxy policy redesign; inventory those separately before changing them.

Acceptance: exact-cap success and cap+1 refusal, absent/lying Content-Length,
chunked oversize and non-success oversize, cancellation releasing a connection,
401 challenge followed by capped response, private CA with non-default timeout,
builder-order preservation, no raw body in the new error. Assert the exact error
and observed request count. Use paused time or injected clocks for long-deadline
checks, not a standing test that sleeps more than ten seconds.

## P2 Production XML boundary

Entry points: `XmlNode::parse`, `parse_soap_body`, SOAP error mapping, and the
client/Health/Metamorph callers of the SOAP helper.

- Add a complete-document parser path for production SOAP responses. Consume
  through EOF; reject unclosed input, a second root, trailing non-whitespace,
  malformed/duplicate attributes and prohibited DTD constructs. Preserve legal
  Unicode, entity and CDATA behavior where existing compatibility cases require it.
- Proposed initial limits: 4 MiB input, depth 64, 65,536 element nodes. Count
  Start and Empty elements and include attributes/text in the byte budget.
  Exercise exact limits and one-over limits. These are client response settings,
  separate from Mock's 2 MiB/64/16,384 request limits.
- Keep direct `XmlNode::parse` legacy semantics and document its limits; route
  `parse_soap_body` through the new complete bounded path so custom transports
  cannot bypass production parse bounds. Preserve `XmlNode` public fields and
  local-name typed parsing. Any new `SoapError` variant has a migration impact.
- Inspect recursive consumers and destruction of deeply nested trees; an
  iterative construction loop alone does not establish stack/memory safety.
  Bounds are workload limits, not a measured total-memory guarantee.

Acceptance: production helper rejects each malformed case retained by legacy
direct parsing; direct XML compatibility tests still pass. Network and injected
transport versions reject the same oversized/deep SOAP input. Assert exact
syntax/resource categories, returned token/text values, and a valid recovery call.
Namespace conformance remains outside this package.

## P3 Session report and safe diagnostics

Entry points: `OnvifSessionBuilder::build`, `fill_missing_service_urls`,
`OnvifClient::call`, Health initialization, and CLI maintenance `connect`.

- Add a report-bearing build entry point that shares one implementation with
  `build`. A successful optional `GetServices` fallback may produce warnings;
  a failed mandatory stage returns both the original typed cause and its report.
  Do not add mandatory fields to existing public structs constructed by literals.
- Report `clock_read`, `capabilities`, `services_fallback`: outcome, elapsed time,
  clock offset if available, and the source of each resolved service URL.
  Use service labels and controlled error classes in the safe projection.
  Never serialize raw reqwest errors, URL queries/userinfo, SOAP bodies or Fault
  reason text into routine verbosity. Retain internal causes for callers.
- Preserve current URL precedence and missing-service behavior. No guessed
  hostname substitution, automatic endpoint probing, or expanded routing family
  hidden inside this diagnostics change.
- Replace unconditional raw SOAP trace with metadata-only trace: controlled
  operation identity, elapsed time, byte counts and error class. Keep explicit
  capture APIs for wire investigation; targeted capture redaction is not a
  general secret/PII sanitizer. Update misleading recorder safety claims.
- The current String transport result cannot expose actual HTTP 200/400/500 on
  success. Mark that status unknown in Session reports; a telemetry wrapper
  observes SOAP-call attempts, not necessarily Digest HTTP exchanges.

Acceptance: positive/negative clock offsets, clock-read failure before any
capability call, capability refusal, missing service with successful/failed
fallback, existing URL never overwritten, no duplicate initialization traffic,
and custom transport builder-order behavior. Trace/CLI tests inject credentials,
userinfo, queries and credential-shaped Fault text; none appears in safe output.

## P4 CLI retry and diagnostic integration

Entry points: `retry_delay`, diagnostic/Health/enrichment execution,
maintenance `onvif_step`, `ExecutionOptions`, `ResultMeta`, output/describe/Agent
guide and their schemas.

- Extract a small internal retry policy reused by existing call sites. Keep
  classification and existing attempt limits; replace maintenance's fixed delay
  with the same bounded backoff. Avoid a generic framework or a new crate.
- Preserve retry units: per selected diagnostic, whole Health attempt, one
  enrichment read, or one maintenance stage. A successful discovery scan is not
  repeated. Never automatically retry future device writes.
- `-v` retains concise sanitized stderr. `-vv` adds service-resolution outcomes,
  attempts performed, retry classes and delay. Call them SOAP/operation attempts;
  do not label them HTTP request counts. Bound diagnostic entries per operation
  and produce deterministic per-device ordering after Fleet workers finish.
- Retain envelope schema v3, exits and JSON/JSONL framing. Diagnostic/workflow
  payload additions require command-schema verification; do not add common
  `ResultMeta` fields in this cut. If v4 becomes necessary, do its schema,
  Agent guide, descriptors, migration examples and six-outcome matrix together.

Acceptance: 503 then success with two attempts; 401/403/SOAP Fault/resource errors
with one attempt; cancellation during backoff; per-stage versus whole-run counts;
bounded Fleet workers and deterministic item order; table/JSON/JSONL at quiet,
normal, `-v`, `-vv`; unchanged v3 success/error validation and stdout cleanliness.
Retain the existing recovery/cancellation suites, not just newly named tests.

## P5 Real-device regression harness and acceptance

Entry points: `record_surface_with_progress`, `SurfaceSelection`,
`FixtureStore::record/lookup_request`, `verify_parsing`, conformance examples,
`tests/fixtures/README.md`, and CI's normal integration-test job.

- Use a new param-aware harness over existing Metamorph recording; do not seed
  a multi-profile corpus through suffix-named last-write-wins files. Iterate all
  selected profiles/sources and assert at least two deliberately different token
  answers in the synthetic harness.
- Keep an explicit manifest beside each corpus: pseudonymous device identity,
  vendor/model/firmware when shareable, recording commit, selected full Actions,
  request identity, per-operation sweep outcome, expected typed field values,
  source provenance and permitted sharing scope. Store transport failure outcomes
  separately; FixtureStore does not retain failed transport responses.
- Reject partial/equal-looking coverage as success: `ParseStatus::Unverified`,
  Faulted, Failed and Parsed are separate; `all_parsed()` is insufficient.
  Compare selected known fields, not just parser success or list counts.
- Raw device captures and official schema resources stay outside the checkout.
  Commit only independently reviewed, permitted sanitized fixtures and manifests;
  secret removal must be followed by manual review of username, serial/MAC,
  hostnames, query tokens, vendor extensions and logs. Do not copy schema-derived
  instances or indexes into this repository.
- Resolve contradictory recorder documentation: existing capture code already
  removes selected credential forms, while the fixture README says it does not.
  Neither current transform guarantees arbitrary device data is safe to publish.

Local exit: synthetic multi-source corpus, identity-preserving runner, exact
expected values and negative controls in ordinary CI; missing capture files fail
an explicitly selected corpus run, never silently skip.

Device acceptance target: two vendors and three device/firmware capture sets,
including one multi-source case. Record actual outcomes per Action and firmware.
This is an initial compatibility target, not the broader five-vendor commercial
claim. Without these inputs, the harness can be DONE while hardware acceptance
remains WAITING FOR EVIDENCE. Hanwha snapshot F10 needs its own failure body and
metadata; do not infer its root cause from synthetic CR/LF images.

## P6 Offline reference comparison

Entry points: `FixtureStore`, `lookup_request`, private `recording_equivalent`,
`parse_check`, `QuirkReport`, and the public Metamorph exports.

### P6a Pairing and outcomes

- Add a separate comparison API/module; do not redefine structural quirk diff,
  `ParseReport::all_parsed`, CLI `config.diff`, or fixture on-disk keys.
- Pair exact full Action and proven request identity. A canonical key is only a
  bucket hint. Preserve duplicate candidates as Ambiguous; never arbitrarily zip
  rows by order, join Action suffixes, or collapse Media1/Media2.
- Proposed outcomes: Equal, Different, MissingLeft, MissingRight, Ambiguous,
  Unparseable, Unsupported, DeviceFault. Report left/right fixture ordinals,
  controlled reasons, compared projection, explicit masks and field paths.
  Do not put raw XML or arbitrary Fault text into the default summary.
- Request identity retains meaningful profile/config/source tokens. Existing
  exact-raw replay of malformed requests is not proof that they are valid or
  semantically comparable; return Unsupported when identity cannot be established.
- Bound file loading, pair count, output changes and retained comparison bytes
  before exposing a file-based example. Current `FixtureStore::load` reads all
  JSON into memory. A limit refusal must be explicit, never a truncated Equal.

### P6b Selected projections

- Initial supported Actions: Device `GetDeviceInformation` and `GetHostname`;
  Media1/Media2 `GetProfiles` and `GetVideoEncoderConfigurations`. Six exact
  Actions, using existing typed parsers. Options/PTZ/Events/Recording stay
  Unsupported until separately mapped. Reuse `parse_check` behind a private
  interface rather than duplicate its routing or change public verdict fields.
- Check complete bounded XML and namespace-aware response identity before the
  local-name parser. Use a bounded `NsReader` response projection, not the Mock
  request validator as an implicit response-conformance oracle. Compare expanded
  names for selected fields; unresolved QName-valued data stays Unsupported.
- Equal means the declared selected projection is equal, not every optional/vendor
  field or every protocol rule. List unexamined extensions/fields explicitly;
  never count them as fully verified. Wrong/changed relevant namespaces cannot
  yield Equal merely because local names and typed JSON match.
- Default volatile mask is empty for these Actions. Do not reuse
  `Masking::Value`/`normalize_instance_values`: token, firmware, address and
  identity differences must remain visible unless a field-specific caller mask
  explicitly excludes them. Record every applied mask.
- Match repeated profiles/configurations by unique semantic token; duplicate or
  absent tokens are Ambiguous/Unparseable. Do not reorder arbitrary nested arrays
  with unknown semantics or normalize unknown data to an empty value.
- Add one offline example with deterministic JSON, progress and documented exit
  meanings. No new CLI parser path, MCP server or oxdm UI implementation is
  required. A downstream integration can consume the report later.

Acceptance: same shape/different hostname or bitrate; firmware change; different
token; same token/different namespace; namespace alias only; list reorder with
unique tokens; duplicate token; one-sided missing Action; two requests sharing a
legacy key; malformed response/request; unknown Action; DeviceFault; unexamined
extension; explicit mask only; size/output limit refusal; repeated identical input
producing identical summaries. Both same-parser JSON comparison and synthetic
Mock matching remain distinct from an independent conformance verdict.

## Compatibility and release checks

| Contract | Required treatment |
| --- | --- |
| Public error enums | New variants need downstream exhaustive-match review, exact error tests and migration notes; do not encode resource refusal as a misleading status |
| Public structs and builders | Keep literal construction and old methods; put new reports/options in new types or private fields |
| CLI stdout | Keep v3 common envelope; any v4 proposal is a separate explicit migration |
| XML behavior | Keep permissive direct DOM behavior; document stricter complete production SOAP acceptance and resource limits |
| Fixture storage | Keep legacy collision retention/replay; new comparison report may have its own version |
| Snapshot and registry | Retain destination/auth/TLS/no-overwrite rules and inventory identity guards; no automatic cleanup or durability claim |

Each implementation batch updates its public docs, examples and CHANGELOG only
when behavior exists. Keep English/Traditional Chinese plan status synchronized.
Run the standing repository gate with RTK; do not count ignored schema/native
tests as acceptance:

```powershell
rtk cargo fmt --all -- --check
rtk cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo test --workspace --all-features --locked --no-fail-fast
rtk cargo test --workspace --locked --no-fail-fast
```

Run strict rustdoc in feature/default modes after public API changes; run
affected individual feature/MSRV checks, the existing schema/CLI contract matrix,
and independently qualified external schema checks for changed wire shapes.
Prove new assertions with focused fault/limit/identity perturbations and restore
exactly. Preserve the user's unrelated work during each batch.

Release acceptance requires P1–P4 local evidence, explicit P5 device dispositions,
and P6a/P6b evidence if the feature ships. Publish no broader commercial/VMS or
all-operation conformance claim from this cut. Current publication, push and
version changes remain outside the planning request.

## Implementation evidence

- P4 implementation: a small internal retry module retains typed classification
  and delay constants; maintenance stages now use that same backoff. Invocation
  metadata has a task-local scope explicitly inherited by Fleet/enrichment workers,
  capped at 128 entries per device and 1,024 lexicographically first device keys,
  with explicit omitted-event counts. Anonymous device indices are emitted after
  workers finish. Existing ExecutionOptions literals, v3 ResultMeta and stdout
  schemas are unchanged. Session reports are consumed without extra requests;
  standalone device-info reads retain their existing no-Session path.
  Boxing the command/capture future avoids Windows main-stack overflow.
- P4 negative controls: the unfiltered all-feature run detects reducing the entry
  budget; focused assertion failures detect schedule-dependent device retention,
  removed actual backoff, suppressed Session stages (Library adapter and CLI),
  and missing Fleet scope propagation. Every mutated source was restored exactly.
  Final format, both workspace Clippy modes, all-feature/default tests
  (1,411/1,292 passed, seven ignored, 50 suites) and both strict rustdoc modes
  passed on 2026-11-10. The six-outcome v3 contract matrix, cancellation/recovery
  and bounded Fleet suites remain passing. Five new tests plus extended Fleet
  assertions cover buffered output and actual units, with table/JSON/JSONL across
  quiet, normal, `-v` and `-vv` for both single-read and Session-based commands.

- P3: `build` delegates to `build_with_report` without changing initialization
  order, mandatory clock/capability failures or missing-URL fallback precedence.
  Reports use controlled enums/numbers only: three stages, clock offset when
  available, known error HTTP status and ten routing URL sources (no URLs).
  Boxed mandatory failures retain the original typed cause separately.
  `HealthCheck::run_with_session_report` returns the same HealthReport plus
  initialization metadata; `run` remains the original projection.
- Client trace retains fixed operation Action, serialized request/returned text
  byte counts, elapsed time and controlled transport error class. A returned
  String is not SOAP success or a known HTTP status. An isolated trace test
  prevents concurrent client tests from racing tracing callsite registration.
  Recorder docs distinguish targeted redaction from a sharing guarantee.
- P3 acceptance: eight new tests cover legacy traffic, either credential/transport
  builder order, positive/negative/unknown clock offset, strict mandatory failure,
  fallback precedence/outcomes, original device Fault cause, separate Health
  metadata and trace privacy. Overwriting a capability URL failed two tests in
  the unfiltered all-feature run; focused controls broke offset/unknown handling,
  authentication/Fault classification, skipped stages and raw URL trace privacy.
  Sources were restored byte-for-byte. Final five gates passed: 1,406 all-feature
  and 1,287 default tests, each with seven ignored and 50 suites. Strict rustdoc
  passed in both workspace modes and the no-feature Library; no-feature and
  health-only Library Clippy also passed.
- Hardware capture inputs: user confirmed none available on 2026-10-08. Continue
  all local packages; P5 device acceptance remains WAITING FOR EVIDENCE.

- P2: complete validation runs before bounded DOM construction; default limits
  are 4 MiB input, root-inclusive depth 64 and 65,536 Start/Empty elements.
  Direct DOM compatibility remains separate. An attribute-key set avoids
  quadratic duplicate checks in validation and the already-validated DOM pass.
  XML 1.0 UTF-8 declarations, names/characters/references and attribute syntax
  are checked without namespace/XSD conformance claims. Existing quick-xml
  syntax error detail remains available to callers; resource errors retain no XML.
  Health maps XML resource failures to its existing Parse class; CLI retry refuses
  them. The exhaustive action-outcome test helper handles the new variant.
- P2 acceptance: six independent syntax/payload/resource/network/recovery/wide-
  attribute tests, plus the unchanged direct XML compatibility fixtures. Allowing
  one extra depth level failed three tests in the unfiltered all-feature run;
  bypassing validation failed five focused tests; changing decoded attributes
  failed the Unicode and wide-attribute value assertions. All sources restored
  exactly. Final format, both Clippy modes, all-feature/default tests
  (1,398/1,279 passed; seven ignored, 48 suites each) and strict rustdoc passed.

- P1: `HttpTransportOptions` controls each HTTP request deadline and accumulated
  response bytes (10 s / 4 MiB defaults). CLI forwarding and Health's inner/outer
  timeout race preserve the existing timeout outcome. Limits apply before
  retaining successful, Fault and HTTP error bodies. UTF-8 replacement decoding
  is explicit, independent of downstream reqwest charset feature unification.
  Digest challenge bodies are discarded by the auth helper; the cap applies to
  the returned response body.
- Wire fixtures assert exact-limit payloads, one-over errors, declared/unknown/
  chunked framing, invalid UTF-8, cancellation and connection EOF. Private-root
  rebuilding retains options/roots; Digest retains credentials across options
  changes with exactly one challenge and one authenticated request. CLI forwarding
  is tested without an outer timeout.
- Negative controls: raising the byte cap by one made three refusal tests fail
  in an unfiltered all-feature/no-fail-fast run. That run also exposed the Health
  timeout race above, now fixed. Restoring the fixed ten-second CLI inner timeout
  made the focused deadline assertion fail. Both sources restored byte-for-byte.
- No maximum real-device response has been measured. The 4 MiB default's release
  suitability remains a P5 hardware acceptance question.
- P1 final gate: format, both workspace/all-target Clippy modes, locked unfiltered
  all-feature tests (1,392 passed) and default tests (1,273 passed), seven ignored
  and 47 suites each; strict rustdoc in both feature modes. HB1 remains separate.

## Original planning verification

- Completed: source/control walkthrough and ownership reconciliation above.
- Passed on the current worktree: 240 local file-link destinations, bilingual
  work-package presence, source spot checks, `git diff --check` and format check;
  both workspace/all-target Clippy modes; all-feature tests (1,385 passed,
  seven ignored, 47 suites) and default tests (1,266 passed, seven ignored,
  47 suites), both locked and with `--no-fail-fast`.
- These checks include the pre-existing uncommitted HB1 worktree input; they
  do not establish HB1 closeout or implement P0–P6. Original tracked diffs and
  the four pre-existing untracked HB1 files were checked unchanged.
- Not run by this walkthrough: live camera, external schema, native terminal,
  package installation, publication, or any future implementation acceptance.

