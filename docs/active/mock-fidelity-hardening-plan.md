# Mock fidelity hardening plan

[English](mock-fidelity-hardening-plan.md) | [繁體中文](mock-fidelity-hardening-plan_zh.md)

Status: D1–D3 approved on 2026-09-10; first implementation slice locally verified;
the full programme remains in progress.
Date: 2026-09-10. Repository baseline: `9dccf9d`.
Trigger: review of [PR #16](https://github.com/smiti1642/oxvif/pull/16)
at `dc69e9a`; that contribution is not assumed to be merged.
The [2026-09-11 PR integration review](mock-fidelity-pr16-integration.md) rechecks
head `3db6459` and records reuse, correction and verification prerequisites.

| Section | Purpose |
| --- | --- |
| [Objectives and boundaries](#objectives-and-boundaries) | Scope and excluded work |
| [Evidence and dependencies](#evidence-and-dependencies) | Observations versus unverified risks |
| [Decisions required](#decisions-required) | Compatibility, simulation policy and schema delivery |
| [Architecture](#architecture) | Parsing, faults, fidelity and independent validation |
| [Milestones](#milestones) | Ordered implementation and acceptance criteria |
| [Verification and completion](#verification-and-completion) | Test gates and evidence requirements |
| [Documentation and delivery](#documentation-and-delivery) | Public documentation and release boundaries |
| [Decision record](#decision-record) | Maintainer approvals and execution status |
| [First implementation slice](#first-implementation-slice) | Scope, regression evidence and remaining work |
| [Execution documents](#execution-documents) | Source-indexed checklist, per-operation tracking and resume point |
| [Source audit checkpoint](#source-audit-checkpoint) | W00 source reconciliation, W01/W02 progress and reproduced gaps |

## Objectives and boundaries

Make the synthetic mock a reliable, explicitly bounded test double. A successful
client/mock round trip must not be the only evidence of protocol correctness.

- Correct request identity and text handling, synthetic fault serialization,
  and observable behavior claims across the currently routed service operations.
- Keep `MockTransport` and `MockServer` behavior aligned; distinguish transport
  status checks from in-process SOAP checks.
- Retain adversarial device-response testing through explicit injection, not
  accidental deviations in the normal synthetic response path.
- Reuse the existing round-trip, token-discrimination and schema audit work.
  This plan extends it; historical acceptance counts are not a new baseline.

Not included: implementing every missing ONVIF method, producing RTP/video,
emulating camera timing, changing CLI navigation, bulk dependency upgrades,
automatic real-camera writes, ONVIF certification, or release publication.
Do not normalize recorded Metamorph/replay fixtures: their deviations may be
the subject of a test. Changes to public client error behavior need explicit
compatibility review, not an incidental mock refactor.

## Evidence and dependencies

| Observation | Evidence and planning consequence |
| --- | --- |
| PR #16 rejects an existing token containing XML-sensitive characters | Public-API reproduction on that PR. The new handler compares escaped text with stored text. Port the regression only when the operation exists in the implementation branch; do not silently absorb the PR. |
| String extraction is shared technical debt | `src/mock/xml_parse.rs` extracts fragments by local name and trims text; it is not an operation-scoped XML parser. Inventory callers before replacing it. Some callers need a subtree rather than decoded text. |
| Fault construction accepts unstructured strings | `src/mock/helpers.rs` interpolates code/reason; the mock guide already records a QName-binding deviation. Correct new behavior and enumerate downstream assertion changes together. |
| Client parsing is not an independent validator | `src/soap/xml.rs` discards namespace information, trims text, and exposes one subcode level. Do not reuse it as the strict mock parser or alter its public error contract silently. |
| The existing schema check has blind spots | At the baseline, `tests/mock_schema_shape.rs` excluded Faults and used a shared prefix map. The [W20 checkpoint](mock-fidelity-schema-preflight.md) repairs scoped resolution and includes Fault structure; full XSD validation and SOAP HTTP behavior remain separate work. |
| Prior review tests passed despite the token defect | PR head: 830 library tests, 28 workflow/action-snapshot tests, formatting passed. This was not full workspace, schema, three-platform or real-stream acceptance. Rerun at the implementation baseline. |

Primary references for implementation verification:
[ONVIF specification catalogue, June 2026](https://www.onvif.org/profiles/specifications/specification-history/june-2026/),
[Media1](https://www.onvif.org/specs/2412/ONVIF-Media-Service-Spec-v2412.pdf),
[Media2](https://www.onvif.org/specs/2606/ONVIF-Media2-Service-Spec-v2606.pdf),
[Media1 WSDL](https://www.onvif.org/ver10/media/wsdl/media.wsdl),
[Media2 WSDL](https://www.onvif.org/ver20/media/wsdl/media.wsdl),
[SOAP messaging](https://www.w3.org/TR/soap12-part1/), and
[SOAP bindings](https://www.w3.org/TR/soap12-part2/).
Select the relevant service/Core documents per operation. Record URLs, retrieval
date, version and hashes in the external verification manifest; do not assume
mutable WSDL URLs identify immutable versions.

The repository's existing schema non-bundling decision remains in force:
no official schema files, generated schema indexes, schema-derived fixtures or
hardcoded schema tables are added to the repository/package. Generic XML tests
use project-authored synthetic names; normative checks read external resources
at runtime. This is a project boundary, not a new legal assessment.

## Decisions required

The maintainer approved all three recommendations on 2026-09-10. They are the
implementation target, not a claim that the defaults have all been changed yet.

| ID | Decision | Recommendation | Tradeoff and blocked work |
| --- | --- | --- | --- |
| D1 | When to switch normal synthetic behavior and fault output | Target the next minor release, with corrected behavior as the default and migration notes. Preserve old/malformed responses only through explicit test injection; do not introduce a global legacy mode without a demonstrated consumer need. | Existing tests matching fault strings or permissive request handling may break. Blocks the default switch and final public API/version choice, not inventory or additive tests. A patch-only approach would require a narrower compatibility-preserving scope. |
| D2 | How unmodelled effectful operations behave | Refuse them by default; allow explicitly selected acknowledgment-only stubs for workflow tests. Static read fixtures remain valid when documented. | Some current successful workflows will need opt-in settings. A stub can record request receipt, but cannot prove a real effect. Blocks behavior changes for affected operations; inventory/classification can proceed. |
| D3 | How schema acceptance is made enforceable | Approve a dedicated CI verification job using pinned external resources outside the checkout; retain schema-free offline developer tests. Require successful schema evidence for release acceptance. | Changes the historical manual-only policy. Official resources are not republished, placed in build artifacts, or bundled. Missing files/tools, digest mismatch or download failure must block that job, not pass as skipped. If declined, require an equivalent maintainer-run gate and report CI as not covering schema. |

No further product decision is needed to inventory call sites, design regression
cases, or keep documentation synchronized. Parser internals, test file placement
and service migration batches are engineering choices. Escalate only if the
inventory demonstrates a necessary public API break beyond D1 or a new dependency
or resource-distribution requirement outside D3.

## Architecture

### Request parsing

Introduce a private mock request representation using the existing XML dependency
where suitable, with namespace scope and decoded text preserved. Parse once at
the synthetic request boundary. Select direct children relative to the operation,
not the first matching local name anywhere in a message.

- Separate text, attribute and subtree access; never replace every `extract_tag`
  call with a text decoder mechanically.
- Preserve token contents after XML normalization; apply whitespace rules only
  for the specific typed field, not a global trim.
- Check operation identity against Action and the service namespace. Prefix
  spelling may vary; namespace identity may not be silently ignored.
- Reject malformed documents, trailing roots, unbound prefixes and inappropriate
  duplicate required fields. Do not reject legitimate repeated members or
  extension content merely because it is unfamiliar.
- No external entity resolution or DTD loading. Apply bounded input/depth/node
  handling to both entry points; measure limits before publishing them.
- Inventory auth, canonicalization and replay dependencies separately. Do not
  put synthetic normalization ahead of replay and thereby erase recorded quirks.

### Fault construction and consumers

Use a structured internal representation for fault code QNames, ordered nested
subcodes, reason and optional detail. Serialize text safely and bind every QName
in its actual scope. Verify each operation's error mapping against its applicable
reference; do not infer a whole hierarchy from an arbitrary `ter:*` string.

Audit `fault_injection.rs`, `responder.rs`, `server.rs`, `transport.rs`, client
fault parsing and CLI error classification together. Keep ordinary synthetic
errors distinct from explicitly malformed/raw injections. Preserve the public
injection call shape where practical; document any semantic migration under D1.

The current client exposes one subcode level. Decide the smallest compatible way
to retain useful error classification after nested fault output is introduced;
test raw wire content independently. Do not redefine the existing `subcode`
field or add a public enum field as an unannounced side effect.

HTTP status, content type, Action handling and authentication failures must be
audited against the SOAP binding/Core references. A correct XML fault body alone
does not validate `MockServer`; current HTTP behavior must not be assumed correct.

### Fidelity declarations

Maintain one project-authored inventory keyed by service plus operation, covering
all routed actions. It records behavior class, token tests, state/effect tests,
known limitations and documentation anchors; it is not a copied schema catalogue.
Reuse it for test coverage checks and public mock tables where practical.

Classes: modelled, static read fixture, acknowledgment-only, unsupported.
Distinguish accepted request, state transition and externally observable effect.
Under D2, acknowledgment-only behavior is explicit per operation; rejected calls
must not mutate state. If receipt tracing is added, it is bounded, per-instance,
resettable, non-persistent and excludes credentials/raw envelopes. It proves only
receipt. Synchronization-point tracing must never claim frame delivery.

### Independent verification

Keep three separate layers: client request/response tests, direct mock wire tests,
and external schema validation. Round trips complement these layers but do not
replace them. The external validator must not share the mock/client parser.

First strengthen the existing structural checker, including scoped namespace
resolution and honest coverage reporting. Then evaluate a mature external XSD
validator in the verification environment (not a library runtime dependency).
Validate the SOAP envelope and service payload roots separately so wildcard
content cannot make payload validation vacuously pass. Validate Faults as well
as successful payloads. Schema-valid output still needs semantic tests.

Resolve imports through an external pinned catalogue. Disable arbitrary network
resolution during validation. Under D3, prepare resources in a constrained fetch
step, with no repository secrets or privileged fork-PR execution. Logs/artifacts
contain sanitized findings and source hashes, not schema contents.

## Milestones

No milestone is fully accepted yet; the first slice below contains completed
subtasks. Commit each independently verified slice, not all milestones at once.
Use the execution documents below for exact work IDs and operation-level status.
Suggested commit scopes are illustrative.

| Phase | Work and likely files | Exit criteria |
| --- | --- | --- |
| M0 — inventory and baseline | `src/mock/**`, existing mock property tests, `src/soap/**`, CLI fault consumers, public guides. Record behavior classes, parser/fault callers, consumer compatibility risks and PR #16 integration status. | Every routed operation is accounted for; observations have repro cases or are explicitly unverified; D1–D3 recorded before affected behavior changes. No historical counts reused without measuring. |
| M1 — regression foundation | Generic XML tests, direct mock requests and separate client negatives. Inventory old fragment-based test probes. | Cases for escaped/Unicode/whitespace tokens, attributes, CDATA, namespace shadowing, misleading nested fields, malformed documents and fault escaping fail for the intended old behavior and pass for controls. No schema-derived fixture tables are bundled. |
| M2 — structured faults | Private builder plus synthetic call-site batches; explicit injection boundary; consumer classification and HTTP contract checks. Suggested `fix(mock)` / `test(soap)` slices. | Migrated faults have independently verified structure and mappings; missing/unknown inputs do not mutate state; old injection use cases have a documented migration or explicit compatibility path. No arbitrary error-code remapping. |
| M3 — request parser migration | Private namespace-aware request view; Media1/Media2 first, then PTZ/Imaging, then Device/Recording/Search/Replay/Events and remaining shared paths. | Each batch has token discrimination and request-path tests; cross-service state agreement remains valid. Every legacy extraction caller is migrated or explicitly isolated outside the normal synthetic path. Reject malformed requests consistently across entry points. |
| M4 — fidelity policy | Behaviour inventory, D2 policy and targeted receipt tracing only where needed; state/effect tests and advertised capability audit. | No effectful acknowledgment-only operation silently succeeds by default under the approved policy. Static reads and opt-in stubs are documented. Supported capabilities, modeled behavior and test claims do not contradict each other. |
| M5 — external validation gate | `tests/mock_schema_shape.rs`, verification tooling and, subject to D3, CI job. Expand request/success/Fault corpus, parser-scope checks and full-validator cross-checks. | Missing prerequisites cannot pass the required gate; every declared corpus category has measured coverage. Injecting wrong namespaces, missing required content or broken fault structure produces failures. Opaque/unresolved/wildcard coverage is separately reported, not counted as fully validated. |
| M6 — integration and documentation | Full regression, feature/platform checks, bilingual guides, release migration/evidence draft. | All accepted milestones pass at one recorded commit; known limitations and unrun checks are explicit; no certification or real-stream claim inferred from mock tests. Release remains approval-controlled. |

M2 and M3 are service-sized migrations, not single sweeping rewrites. A shared
helper change that exposes a client defect gets a distinct regression and review;
do not loosen the mock to restore an unjustified green test.

## Verification and completion

- Retain all five `CLAUDE.md` per-commit gates: formatting, clippy with and without
  all features, tests with and without all features. Use locked dependencies for
  repeatability; run full workspace gates for integration and CLI consumers.
- Run existing workflow, action snapshot, round-trip, token discrimination,
  multi-sensor, Media1/Media2 agreement, service capability, replay/canonicalization
  and XML compatibility suites. Update expectations only after checking meaning.
- Positive writes assert request payload and observable modeled state; negatives
  assert specific error payloads, not just `is_err()`.
- Demonstrate test sensitivity with controlled mutations in an isolated checkout:
  bypass token selection, skip decoding, corrupt namespace scope or flatten fault
  structure. Restore exact changes; batch runs use `--no-fail-fast` and all features.
- Run schema-free tests on Windows, Linux and macOS; run the external validation
  gate in its documented verification environment. Do not claim native coverage
  for an operating system that was only cross-compiled or not run.
- Build docs and doctests for relevant feature combinations; include the
  per-feature warning sweep before release. Public API/MSRV changes require the
  repository's additional compatibility checks.
- Report outcomes separately as passed, failed, blocked or not run. A missing
  schema resource is not evidence of conformance. Sanitized reports record commit,
  toolchain, features, OS, commands, corpus coverage and external resource hashes.

Completion means all routed operations have explicit fidelity classifications,
normal synthetic parsing/fault paths meet the accepted contract, migrated behavior
has non-hollow tests, and required gates pass. Any retained exception needs a named
scope and rationale; it cannot disappear into a total count or an updated pin.

## Documentation and delivery

Update alongside implementation:

- `docs/mock-server.md` and `_zh.md`: request strictness, fault/injection contract,
  fidelity table, stub policy, transport boundaries and migration instructions.
- `LIBRARY_GUIDE.md` / `_zh.md`, `OPERATIONS.md` / `_zh.md`, `src/mock/mod.rs`,
  `src/lib.rs` and affected public method docs: accurate behavior and support claims.
- `README.md` / `_zh.md`: brief overview/link changes only if positioning changes;
  no expanded technical manual. CLI guides change only if observable error output
  or existing examples require migration.
- `CHANGELOG.md`: concise unreleased behavior changes and migration guidance;
  detailed sanitized evidence belongs in a version-specific release record once
  the target version is selected. Do not rewrite shipped history as if fixed then.
- `docs/README.md`, this plan and relevant historical audit status notes: current
  navigation and cross-links without replacing historical measurements.

The initial planning turn created the bilingual draft and index only. The
maintainer subsequently authorized implementation and optional CLI-assisted
real-device verification. Real-device writes are not authorized by that testing
permission; use discovery and read-only inspection unless separately agreed.
Implementation commits follow the approved decisions and repository gates.
Do not merge or modify PR #16, post a review, push branches/tags, publish crates,
create a GitHub Release, or update installed binaries as part of writing this plan.
Those actions require the applicable later user instruction; retain the user's
requirement to notify them before installing/updating a release on their system.

## Decision record

| Item | Status |
| --- | --- |
| D1 — default behavior and release boundary | Approved 2026-09-10; next minor release, corrected defaults, no global legacy mode |
| D2 — unmodelled effects and opt-in stubs | Approved 2026-09-10; implementation pending |
| D3 — external-schema CI/release gate | Approved 2026-09-10; existing non-bundling policy unchanged; implementation pending |
| Execution authorization | Approved 2026-09-10: implement and verify through the plan, commit own work directly without opening own PRs; push the hardening branch for CI. Investigate integration of PR #16's requested feature. No automatic contributor-PR merge, main-branch merge, tag, publication or installed-binary update. |
| M0–M3 | In progress; first slice below does not complete these milestones |
| M4–M6 | Not yet complete |
| Release version, tag and publication | Not selected or authorized by this plan |

## First implementation slice

Branch: `codex/mock-fidelity-hardening`. This is an independently verifiable
first slice, not acceptance of the full programme or PR #16.

- Added a private bounded, namespace-scoped scalar request parser. Migrated only
  Media1/Media2 `DeleteProfile`; fragment/subtree callers are not mechanically
  rewritten. The public client parser and replay are unchanged.
- Escaped shared fault code/reason text and bound known prefixes. Flat error
  hierarchy, HTTP status behavior and the full structured builder remain pending.
- Before fixes, three fault-safety tests failed and both request-identity
  regressions failed: an existing escaped token was rejected, and a nested decoy
  incorrectly deleted mock state. The old unbound-prefix unit fixtures were
  corrected instead of weakening the parser.
- Initial baseline: Windows workspace/all-features, 1,127 passed, 4 ignored.
  Final first-slice gates: workspace/all-features 1,141 passed, 4 ignored;
  workspace/default 1,061 passed, 4 ignored. Formatting and both workspace/all-targets
  clippy configurations passed with warnings denied. Both library documentation
  configurations built without warnings. Gates used `--locked` and the isolated
  `target/mock-fidelity-build` directory where applicable.
- Mutation evidence: temporarily restoring a token `trim()` made
  `text_decodes_once_and_preserves_whitespace` fail on its payload assertion.
  Restored the exact parser content (Git blob hash checked) and reran the test.
- Existing release CLI baseline: ephemeral discovery found 191 devices; one
  discovered saved device passed `info` and `profiles`, with exit 0, `ok=true`,
  no structured errors or warnings. No device writes, credential output, device
  addresses or identifiers were retained. This is read-only client evidence, not
  real-device validation of `DeleteProfile` or proof of the new mock behavior.
- The branch-built debug CLI repeated the same read-only smoke: discovery found
  189 devices, with the same successful saved-device reads and zero warnings.
  Discovery counts are observations of separate scans, not fixed fleet sizes.
  The installed/release binary was not overwritten.
- External schema validation and Linux/macOS native runs were not performed in
  this slice. The four ignored tests are not reported as passed.
- Remaining: whole-operation inventory/classification, structured fault mapping,
  remaining request migration, opt-in stub policy, independent schema CI, full
  platform acceptance and next-minor release preparation.

## Execution documents

Planning revision on 2026-09-10, indexed against implementation commit `b134f73`:

| Document | Role |
| --- | --- |
| [Execution checklist](mock-fidelity-execution-checklist.md) | W00–W26 dependencies, source/test map, C01–C12 review axes, operation-card readiness, risk register, commands and closure evidence |
| [Operation ledger](mock-fidelity-operation-ledger.md) | All 157 literal route arms across 10 production sub-dispatchers, exact handler/arguments and separate contract/request/Fault/behavior/verification status |
| [Read-only inventory checker](check-mock-fidelity-inventory.ps1) | Source/ledger equality and bilingual tracking; positive and ten rejection self-tests; not a schema validator or CI gate yet |

The original milestone table alone is not sufficient for implementation handoff.
Use these linked documents as its execution layer, not conversation memory.
Route enumeration is complete at this baseline; full Action reconciliation,
transitive caller mapping, per-field contracts, Fault mapping and behavior
classification are not. Complete a batch's W01 cards before migrating its handlers.
This preserves an explicit investigation step instead of inventing specifications.

Resume with W00 full Action reconciliation and W02 caller mapping, then W01 for
the W10 profile/binding batch. The documentation/tooling revision changes no Rust
runtime code, CI, dependency, release version or installed binary. Main-plan
milestone states remain in progress; PR #16 remains outside the baseline.

Planning-revision verification on Windows: the inventory checker and its ten
rejection controls passed; all 214 local links/anchors across the six planning
documents resolved. Formatting and both workspace Clippy configurations passed.
Workspace tests were rerun: all-features 1,141 passed / 4 ignored; default 1,061
passed / 4 ignored. These are regression checks, not new schema or platform
acceptance. No camera commands were run for this documentation revision.

## Source audit checkpoint

Started from `892aa94` on 2026-09-10. [Source audit](mock-fidelity-source-audit.md)
records the complete literal Action/method correspondence and direct reader
index; [profile/binding preflight](mock-fidelity-profile-preflight.md) covers the
first 13 operation cards. W00 source reconciliation is complete for the current
source forms; W01 and W02 remain partial, not whole-contract acceptance.

The dispatch sweep now includes the session's direct request path. The incorrect
fixed-profile binding comment is corrected without changing runtime behavior,
with an Add/Remove state control for both Media services. K13–K16 are executable
known-gap probes: generated-token collision, refused-delete notification, raw
profile-name markup, and a late binding failure leaving an earlier write applied.
These probes pass by reproducing the defects, not by fixing them.

Next: finish the preflight's external field/Fault review and shared parsed-input,
atomicity and compatibility designs (W02–W06), then migrate bounded handler
batches. Runtime routing, schema CI, releases and installed binaries are unchanged.

Subsequent implementation status is maintained in the execution checklist and
preflights: K13 allocation and K14 deletion notification now have corrected
regressions; selected DeleteProfile fault/effect paths, K22 client selection and
external schema/corpus CI have also changed. The preceding paragraphs describe
the initial checkpoint, not current acceptance. Broad parsing, semantic migration
and PR #16 integration remain open; releases and installed binaries are untouched.
