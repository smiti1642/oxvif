# Media profile and binding preflight

[English](mock-fidelity-profile-preflight.md) | [繁體中文](mock-fidelity-profile-preflight_zh.md)

Work: W01/W02 for the first W10 batch. Baseline `892aa94`; investigation started
2026-09-10. **IN-PROGRESS, NOT READY for broad handler migration.**
Owner: current hardening branch. [Execution checklist](mock-fidelity-execution-checklist.md) ·
[Source index](mock-fidelity-source-audit.md) · [Operation ledger](mock-fidelity-operation-ledger.md)

| Section | Purpose |
| --- | --- |
| [Scope and identities](#scope-and-identities) | Exact 13 operation cards |
| [Shared paths and current behavior](#shared-paths-and-current-behavior) | Inputs, effects and consumers |
| [Reference review](#reference-review) | Checked conclusions and missing evidence |
| [Cases and readiness](#cases-and-readiness) | Required regression and remaining design |
| [Profile-token dependency closure](#profile-token-dependency-closure) | Paired migration paths and test boundaries |
| [Shared whitespace serialization](#shared-whitespace-serialization) | K29 data preservation before token migration |
| [PTZ profile identity](#ptz-profile-identity) | PTZ1 prerequisite before Media token migration |

## Shared whitespace serialization

K29/W03/W06/W10, 2026-09-11: the common private `types::xml_escape` helper is
used for both text and attributes, but previously left CR/LF/tab literal.
Its output now uses numeric character references for these data characters;
ordinary strings retain the borrowed fast path. This is a shared representation
fix, not a public signature or field-validation change. It also corrects profile
Name CR preservation, since both Create readers already decode text and both
profile renderers call this helper. Raw token renderers/readers remain separate.

Reviewed primary XML behavior: [XML 1.0 line-ending handling](https://www.w3.org/TR/xml/#sec-line-ends)
and [attribute-value normalization](https://www.w3.org/TR/xml/#AVNormalize).
The synthetic values in regression tests are project-authored, not copied schema
fixtures. `xml_escape_preserves_whitespace_in_text_and_attributes` checks exact
wire spelling and decoded text/attribute values; normal borrowed-value tests remain.
Both-transport profile Name tests exercise raw references and client-generated
values through creation, state, all three profile views and deletion.

Old-code failures: RTK 1789096994 (both profile transports) and 1789097019 (shared
helper assertion). The first full regression found the structured Fault serializer's
redundant CR-only transform and old hexadecimal-spelling assertion (1789097328).
It now uses shared escaping directly, with the exact decoded reason/code/subcode
assertions preserved; only its wire spelling changes to the shared decimal form.
The first-profile external corpus also uses CR/LF/tab in project-authored creation
Names; it still captures 17 exchanges / 34 XML instances, not new operation coverage.

Final verification passed formatting, both workspace Clippy modes, 1,208
all-feature and 1,113 default tests (5 ignored, 27 suites each), both strict
documentation builds and unchanged 159/157/255 inventory self-tests. External
corpus `oxvif-profile-corpus-20260911-01` passed all 34 instances with pinned
Xerces XSD 1.1. The legacy breadth check retained 158 responses, 111 success
payloads, 47 faults, 1,242 anchors, 1,431 skipped children and 398 checked
attributes, with all ten finding pins at zero. Neither validator proves full
operation semantics or hardware behavior. Prior commit `31e2e20` passed hosted
CI 34558092566; that run does not cover this later serialization change.

Remaining: full profile-token/input/output closure, raw nested renderers, invalid
XML character handling outside Faults, field-specific whitespace/length constraints
and real-device compatibility. Existing snapshots are not decoded or rewritten.

## PTZ profile identity

PTZ1, 2026-09-11, baseline `2bdf4c2`: migrate the existing 19 users of
`require_profile`/`require_head` before changing Media Create token storage.
They now borrow the already parsed operation from dispatch and select one
direct, qualified scalar without trimming or decoding persisted values again.
Non-profile fields retain their current readers. No public signature changes.
The pinned external PTZ WSDL and common token type were reviewed for this
identity slice; the normative notes remain outside the repository. No full PTZ
operation/fault/field-length or schema-instance acceptance is implied.

Preserve ordinary missing/empty/unknown/unbound-profile Fault payloads. Duplicate
and nested scalar fields use generic InvalidArgs before lookup or mutation.
Foreign, Header and extension descendants cannot supply a missing direct field;
extra decoys do not replace a valid direct field. Field-specific sequence,
attribute and extension policies are still open. The state test helper now
constructs qualified requests and exercises dispatch; it does not bypass parsing.

`tests/mock_ptz_profile_identity.rs` compares each selected operation for two
different heads under ordinary and renamed literal tokens. It checks response
payloads (excluding only the dynamic UtcTime value), complete resulting state,
the untouched other head, mutation notifications and an explicit zoom target.
Additional controls cover reference/CDATA identity, namespace aliases, foreign,
nested and Header decoys, missing/empty/unknown faults and refused state/hooks.
Both transports failed against the old reader on the intended token lookup
assertion (RTK 1789098260; the earlier 1789098216 run used a subsequently improved
order-independent test comparison). These are authored behavioral probes, not
official schema fixtures or real-device movement tests.

Verification: disabling duplicate-field rejection failed both new transport
controls in the full workspace/all-feature/no-fail-fast mutation run
(`1789098685_cargo_test.log`); the mutation was restored. Final formatting,
both workspace Clippy modes, 1,210 all-feature and 1,115 default tests (5 ignored,
28 suites each), both strict documentation builds and inventory self-tests
passed. Inventory is now 159 Action sites / 157 routes / 254 direct readers;
both ledgers track the changed arguments. Legacy shape coverage retained
158 responses, 111 successes, 47 faults, 1,242 anchors, 1,431 skipped children,
398 checked attributes and all zero finding pins. No new PTZ XSD-instance
acceptance is claimed. Prior whitespace commit `2bdf4c2` passed hosted
CI 34559338436; this local PTZ slice is a later change.

Remaining closure: Media Create/read/render/bind token paths, typed adapter and
recorded-key migration. PTZ node/config/preset/tour tokens and nested values,
complete fault mapping, capability/effect policy, concurrent validation/mutation
and replay dependencies remain W11/W16/W19 work. Existing state strings are
never automatically unescaped or rewritten.

## Profile-token dependency closure

Source checkpoint `4220ec2`, 2026-09-11; W02/W10/W11/W19. This inventories the
profile identity path before changing its stored representation. It does not
declare all configuration tokens or PTZ operations audited.

| Path | Current reader/consumer | Paired migration obligation |
| --- | --- | --- |
| Media1 creation and reads | `handle_create_profile` optional Token and `resp_profile` ProfileToken use fragments; `render_profile` writes the profile token attribute verbatim | Decode scoped optional/required scalar fields and escape the attribute together; preserve meaningful whitespace and distinguish character references from literal attribute whitespace |
| Media2 reads/deletion | `resp_profiles_media2` Token and both DeleteProfile readers already use Node; `render_profile_media2` writes the token attribute verbatim | Preserve decoded identity across both views; do not decode persisted literal strings again |
| Profile bindings | `media::{bind_configuration,unbind_configuration}` and `media2::apply_media2_configuration` read ProfileToken via legacy extraction | Migrate profile identity on all six wrappers; configuration-token and Type/Name validation remain separately scoped, not silently certified |
| PTZ head resolution | `ptz::require_profile` → `require_head` → ProfileEntry PTZ config → node; 18 `head!` callers plus `resp_ptz_compatible_configurations` | Pass parsed identity through all selected handlers; migrate the unqualified `state.rs::ptz_req` test inputs rather than weakening namespace validation; check both deliberately different heads (completed for this identity slice in PTZ1 above) |
| Typed adapter | `adapter::{profile_token,ctx_profile}` parse a trimmed local-name DOM for GetStreamUri/ContinuousMove | Preserve public DeviceAdapter signatures and raw fallback; review exact Action dispatch separately; do not substitute strict synthetic handling for user-owned raw adapters |
| Recorded replay | `canon::canonicalize` → fixture index → ReplayResponder, ahead of synthetic | K27 reproduces six key collisions; design persisted-key compatibility and exact input identity before claiming the complete token workflow works in replay |

Acceptance sequence: establish discriminating special-token fixtures; define the
key migration without changing public fixture/adapter contracts inadvertently;
pair Create/read/render/profile-binding/PTZ paths; then test Create → both service
views → bind → PTZ head query → remove/delete across both transports, with exact
state/hook/refusal assertions. Use raw wire assertions where the compatibility
client DOM trims text. Include tabs/newlines/CR references in attribute output
tests: escaping only `<`, `&` and quotes does not preserve XML attribute whitespace.
Retain normal-token, alternate-profile and malformed/duplicate/wrong-namespace
controls. Do not call this complete until adapter and replay paths are separately
verified. Empty-token and length/capacity policies still require their reviewed
operation cards; do not infer those rules from a renderer fix.

## Scope and identities

The table below is the per-operation current-behavior part of each card.
All 13 cards inherit the shared-path, reference and C01–C12 sections below.
Inputs describe existing source, **not** a normative field table or approved
defaults. Action URI and source method are individually recorded in the source
index; handlers are in `src/mock/services/media.rs` or `media2.rs`, dispatch in
`src/mock/dispatch.rs`. Full URI must match its indexed value, not just its tail.
Both client service URLs come from session service discovery; MockTransport
currently ignores URL and MockServer uses its catch-all POST route.

All rows now inherit common synthetic XML/container/Action-body validation before
their handler. The table's "input extraction" column describes handler fields,
not a bypass of this boundary. DeleteProfile borrows the parsed operation;
`required_text` remains only a test helper. Generic boundary faults precede the
listed operation-specific branches. CreateProfile Name and Media2 GetProfiles
selectors also borrow parsed fields; remaining field migrations stay open.

| Ledger ID | Client → handler | Current input extraction | State/renderer path | Current ordinary Fault codes (flat unless noted) |
| --- | --- | --- | --- | --- |
| `media.GetProfiles` | get_profiles → resp_profiles | No body consumed | profiles + catalogues → render_profile | No operation-specific branch in handler |
| `media.GetProfile` | get_profile → resp_profile | GetProfile fragment → ProfileToken; absent becomes empty | profiles + catalogues → render_profile | ter:NoProfile |
| `media.CreateProfile` | create_profile → handle_create_profile | Parsed direct scalar Name; optional Token still uses legacy fragment/text | create_profile_in_state → profiles, next_token_id → render_profile | Generic field faults; ter:ProfileExists |
| `media.DeleteProfile` | delete_profile → handle_delete_profile | parsed operation.required_child_text(ProfileToken), strict scalar identity | delete_profile_in_state → profiles → empty response | Field validation: env:Sender; missing/fixed: nested s:Sender (review below) |
| `media.AddVideoSourceConfiguration` | add_video_source_configuration → handle_add_video_source_configuration | ProfileToken; ConfigurationToken with Token fallback | bind_configuration(VideoSource) → profile slot | env:Sender / ter:NoProfile / ter:NoConfig |
| `media.RemoveVideoSourceConfiguration` | remove_video_source_configuration → handle_remove_video_source_configuration | ProfileToken; legacy scalar | unbind_configuration(VideoSource) → profile slot | env:Sender / ter:NoProfile |
| `media.AddVideoEncoderConfiguration` | add_video_encoder_configuration → handle_add_video_encoder_configuration | ProfileToken; ConfigurationToken with Token fallback | bind_configuration(VideoEncoder) → profile slot | env:Sender / ter:NoProfile / ter:NoConfig |
| `media.RemoveVideoEncoderConfiguration` | remove_video_encoder_configuration → handle_remove_video_encoder_configuration | ProfileToken; legacy scalar | unbind_configuration(VideoEncoder) → profile slot | env:Sender / ter:NoProfile |
| `media2.GetProfiles` | get_profiles_media2 → resp_profiles_media2 | Parsed Token/Type, scoped scalar and direct sequence checks | cloned profile projection + media::catalogues → render_profile_media2 | Generic field faults; nested s:Sender / InvalidArgVal / NoProfile |
| `media2.CreateProfile` | create_profile_media2 → handle_create_profile_media2 | Parsed direct scalar Name; Configuration not read | media::create_profile_in_state(None) → profiles, counter → Token response | Generic field faults; ter:ProfileExists branch (currently unreachable with None) |
| `media2.DeleteProfile` | delete_profile_media2 → handle_delete_profile_media2 | parsed operation.required_child_text(Token), strict scalar identity | media::delete_profile_in_state → profiles → empty response | Field validation: env:Sender; missing/fixed: nested s:Sender (review below) |
| `media2.AddConfiguration` | add_configuration_media2 → handle_add_configuration_media2 | ProfileToken; repeated Configuration/Type/Token; Name not read | apply_media2_configuration → atomic media::apply_configuration_bindings(add=true) | env:Sender / ter:ConfigurationConflict / ter:NoProfile / ter:NoConfig |
| `media2.RemoveConfiguration` | remove_configuration_media2 → handle_remove_configuration_media2 | ProfileToken; repeated Configuration/Type/Token | apply_media2_configuration → atomic media::apply_configuration_bindings(add=false) | env:Sender / ter:ConfigurationConflict / ter:NoProfile |

## Shared paths and current behavior

See the [pipeline preflight](mock-fidelity-pipeline-preflight.md) for the
selected transitive closure, consumer map and P-A–P-E implementation order.
K17 tracks pre-success replay invalidation separately from K14's state hook.

- Request: session wrappers (and media-version preference/fallback) → client
  methods → `OnvifClient::call` envelope/security → MockTransport/MockServer →
  FaultResponder → AuthResponder → optional replay → SyntheticResponder →
  dispatch. These 13 client methods escape caller strings; old mock extraction
  does not decode them. Common identity/XML validation now includes static handlers.
- Scalar extraction: `xml_parse::{extract_tag,extract_all_tags,extract_attr}`
  locate local names and return trimmed/raw fragments. Delete uses
  the already-parsed operation's scalar accessor instead. Binding now passes extracted values into a
  shared atomic plan; it no longer synthesizes and reparses per-entry XML.
  Scoped input decoding remains a separate parser migration.
- Creation: explicit duplicate check and insertion now share the write lock;
  generated identities skip occupied tokens. `ProfileEntry` is appended with all
  configuration slots None and fixed false. There is no modeled capacity check.
  Media2 ignores initial Configuration entries. K13's collision is now a corrected
  regression for both services; shared-helper controls cover concurrent allocation,
  duplicate full-state preservation, notification counts and counter boundaries.
- Deletion: `delete_profile_in_state` locates token under write lock, refuses
  fixed/missing, otherwise removes one profile. K14 is fixed through an explicit
  committed-outcome notification predicate; refusals preserve state and skip the
  hook, while successful deletion notifies once. This is not rollback or a change
  to public state-helper semantics. No profile-change event or reference-cascade audit has
  been completed; do not claim absence of those effects is conformant.
- Binding: `ConfigKind::{from_media2_type,known_token,slot}` select five modeled
  kinds. Both wrappers use `apply_configuration_bindings`: profile/config checks
  and all slot writes share one write lock. Media2 pre-resolves kinds and submits
  the complete list. Fixed does not prevent binding changes. The reproduced K16
  partial write is repaired, with full-state and one-notification controls through
  both transports. Unsupported kinds, Type=All, name-only updates and configuration
  conflicts need reviewed target behavior, not accidental fallbacks.
- Rendering: `profile_snapshot` captures profiles and all configuration catalogues
  under one read guard; `render_profile` / `render_profile_media2` inline VSC, encoder,
  audio source/encoder and PTZ render helpers. Profile Name now escapes decoded
  state text once; profile-token and nested-configuration text remain open under K15.
  Further nested renderers' escaping and other snapshot paths remain W10/W18.
- State hooks: `MockState::{modify,modify_returning,notify}` capture the mutation
  snapshot under the write lock and invoke callbacks after releasing it. Bounded
  reentrant writes are supported; callback serialization and failed-write replay
  invalidation remain W18/W19. The mock does not implement external persistence.
- Consumers: `MediaProfile` / `MediaProfile2` and nested configuration parsers
  in `src/types/media.rs`; SOAP errors via `parse_soap_body/find_response`;
  session profile selection and CLI profile views/errors. Review `SoapError`
  classification before nested faults; do not fix mismatch by weakening tests.
- Behavior classification: state-modeled **with gaps**, not acknowledgment-only,
  for profile mutations; state-backed snapshots for reads. D2 does not turn all
  these operations into stubs. Unmodeled subfeatures require explicit handling.
- Public surfaces: `docs/mock-server.md` and `_zh`, client/session rustdoc,
  library guide and affected CLI examples, CHANGELOG. No public API additions,
  release version or real-camera writes are part of this preflight.

## Reference review

Read [Media1 v24.12](https://www.onvif.org/specs/2412/ONVIF-Media-Service-Spec-v2412.pdf)
§4.1, §5.2.1–5.2.5, §5.2.13–5.2.14 and §5.2.22, and
[Media2 v26.06](https://www.onvif.org/specs/2606/ONVIF-Media2-Service-Spec-v2606.pdf)
§4.1 and §5.1.1–5.1.5 for this batch. The fixed-profile conclusion is verified:
fixed prevents deletion, not configuration binding changes. The old contrary
comment has been corrected and an independent state assertion added.

The Media2 reference exposes additional selector/initial-configuration/name/list
semantics beyond the fields currently read. Those are review targets, not
authorization to silently broaden public client methods.

K22: §5.1.2 and the pinned Media2 request declaration were checked directly.
The existing full-profile client method omitted `Type`, asking a conforming
device to omit configuration data. The mock previously ignored selectors and masked this.
The client now explicitly sends `Type=All`, without adding parameters or changing
return types; the session delegates to that method. Its existing field test now
records and asserts endpoint, complete Action and request selection. The old
request failed this assertion during a full all-feature no-fail-fast run.
The bounded mock-selector slice below is now implemented; broader W10/P-E field
and output validation remains open. XSD
validity alone cannot detect this valid-but-inappropriate request choice.

Direct input sequences for all 13 operations were also inspected in the pinned
WSDL closure; detailed field notes remain outside the checkout under the external
source root (`profile-contract-review-20260910.md`). This is not full output-type
or Core/common error review. Source capacity and capability inconsistencies also
remain open: creation does not enforce the advertised profile limit; Media2's
advertised configuration kinds disagree with the current binding implementation.

The bounded DeleteProfile fault review uses Media1 §5.2.22 and Media2 §5.1.5:
both services require Sender → InvalidArgVal → NoProfile for an unknown profile,
and Sender → Action → DeletionOfFixedProfile for a fixed one. These four branches
now use the private serializer. The client keeps first-subcode semantics; reason
text and error types remain unchanged. The corpus checks both nested levels and
client/health classification; fixed refusals preserve serialized state.
K14 notifications are fixed separately; built-in replay now uses committed
DeleteProfile effects for selected profile reads (see pipeline preflight).
Remaining mutation/dependency paths stay open. Invalid-request fault paths
and virtual-profile behavior are not part of this bounded migration.

Pinned external compilation and 34 selected instance checks now pass; see the
[schema preflight](mock-fidelity-schema-preflight.md). Outstanding: complete
WSDL/XSD field validation, Core/common and other operation Fault mappings, and
capacity/conflict/extension rules. Do not mark C done from selected instance results.

## Cases and readiness

Implemented bounded K15 Name slice: both CreateProfile Name readers and both
profile Name renderers migrated together. They use the existing parsed operation for one
required direct scalar Name, preserving decoded empty and significant-whitespace
values; reject missing, duplicate or nested Name before allocation. Media1
5.2.1 and Media2 5.1.1 plus the external direct-input review support this narrow
contract. Do not migrate caller-supplied profile tokens, binding readers,
configuration names, capacity, attributes, field lengths or complete operation
ordering in this slice. Those remain explicit W01/W10 work, not accepted inputs.
The corrected seeded literal-markup probe and `mock_profile_names` exercise
client-created entity spellings, numeric references, CDATA and whitespace across
both services. HTTP/in-process creation refusals preserve complete state and hook
counts. Only the K15 Name gap probe is converted to the corrected invariant;
K15 token/nested-renderer risks stay open.

K15 Name verification: the old implementation failed the seeded scalar check
and both transport tests at the literal stored-name assertion
(`1789047728_cargo_test.log`). Bypassing duplicate-Name rejection made both
transport tests fail on an unexpected successful creation
(`1789047974_cargo_test.log`); the mutation was restored. Three state unit probes
now use identified operations through dispatch rather than bypassing parsing.

The legacy external shape probe initially fell from 111 to 109 successful
payloads because its two CreateProfile requests omitted Name; its green result
was not retained as equivalent coverage. Both probes now carry a project-authored
literal-name input. A new schema-free control requires successful creation and
literal stored text; removing that input failed the intended response assertion
in the full no-fail-fast run (`1789048420_cargo_test.log`), then was restored.
The rerun restores 158 responses / 111 successes / 47 Faults / 1,242 anchors /
1,431 skipped children / 398 attributes, with all ten finding pins zero.

Final local gates passed formatting, both workspace Clippy modes, 1,196
all-feature and 1,111 default tests (5 ignored, 25 suites each), both strict
workspace documentation builds and the updated inventory self-tests (159 Action
sites / 157 routes / 255 direct readers). Fresh external corpus `-10` passes
strict Xerces XSD 1.1 for the existing 34 instances; it is not expanded Name
variant acceptance. Previous snapshot commit `418eacc` passed hosted CI
34483819340; that result predates this Name change. Next: committed CreateProfile
effects for built-in replay, including strict-name refusals and stale list reads.

Implemented bounded K18 effect slice: both CreateProfile handlers identify their
existing `CreateOutcome::Created` result without changing input or allocation
contracts. Emit the existing private ProfilesChanged effect only for that outcome,
and add only their exact Actions to built-in commit tracking. Reuse the reviewed
three-read dependency set (Media1 GetProfile/GetProfiles, Media2 GetProfiles),
including conservative retirement of singular profile recordings. Test missing
Name and Media1 duplicate-token refusals, successful creation visible in both
lists, unrelated service recordings and instance isolation through both built-in
transports. Raw/auth/fault short circuits still do not report a committed effect.
Standalone responders, bindings, full dependency closure and concurrent callback
visibility remain W19; no new public observer API or real-camera write is needed.

K18 verification: the corrected old-behavior run failed at stale-list identity
and both transports' refused-creation recording assertions
(`1789048799_cargo_test.log`). Suppressing only the Media2 creation effect caused
both transport tests to fail at retained-recording identity after the Media1
sequence passed (`1789048940_cargo_test.log`); it was restored. Final formatting,
both workspace Clippy modes, 1,198 all-feature and 1,111 default tests (5 ignored,
25 suites each), both strict workspace doc builds and unchanged 159/157/255
inventory self-tests passed. No response format or new XSD acceptance is claimed.
Name commit `aa78a31` passed hosted CI 34485846940; that run predates this replay
slice. Next engineering work is binding effects and first-batch token/field
migration; the complete programme remains in progress.

Implemented bounded binding-effect slice: four Media1 video binding handlers and the
two Media2 generic binding handlers already receive an atomic `Result<(), String>`
from the shared plan. Emit ProfilesChanged only on Ok; add exactly their six
Actions to built-in commit tracking. A successful idempotent removal remains a
committed plan and conservatively retires profile views, matching the existing
hook policy. Preserve handler field/fault contracts in this effect-only slice.
Use same-sensor source/encoder fixtures; compare complete expected state and all
three profile reads, missing-profile/late-config refusal, unrelated service
recordings sharing the old family spelling, and instance isolation through both
transports. Separate configuration writes, full dependency closure, typed parsing,
Type=All/conflicts and concurrent visibility remain open.

Binding-effect verification: the old behavior failed both transport controls on
the unrelated same-family recording after a refused AddVideoSourceConfiguration
(`1789049462_cargo_test.log`). Removing only Media2 RemoveConfiguration effect
delivery failed both controls at retained profile recording identity after earlier
cases passed (`1789049709_cargo_test.log`); it was restored. The final controls
include all six Actions and three already-empty removal cases, complete state,
late-configuration refusal and separate instances. Final formatting, both Clippy
modes, 1,200 all-feature and 1,111 default tests (5 ignored, 25 suites each), both
strict workspace doc builds and unchanged 159/157/255 inventory self-tests passed.
This slice does not change response formats or add schema-instance acceptance.
Previous creation-effect commit `ba35567` passed hosted CI 34486980965. Next:
complete the remaining first-batch request/field and output-text contracts; the
replay dependency graph and concurrent visibility are still not fully accepted.

Implemented bounded W18 read-snapshot slice: Media1 GetProfiles/GetProfile and
Media2 GetProfiles capture profiles and catalogues under one shared read guard,
preserving response shapes and selector behavior. `catalogues_from_state` takes
a borrowed DeviceState rather than reacquiring a lock. The generation-tagged
control covers all three read paths, bounded writer bursts and actual writer
progress. This repairs shared-state consistency, not schema acceptance or
atomicity across separate requests.

The initial single-write handoff did not expose the race and was strengthened to
256 bounded writes per handoff with a readiness signal. That control failed
against the old split snapshots (`1789046635_cargo_test.log`). Reintroducing the
split in the shared helper then detected mixed generations on all three read
paths (`1789046899_cargo_test.log`: 295/314/274 mixed samples respectively out of
400 per path). The mutation was restored. This is scheduling-dependent stress
evidence of the unsafe split, not a deterministic reproduction count or a general
linearizability proof. The writer is joined and its final committed generation
is asserted; no real camera is involved.

Restored verification passed formatting, both workspace Clippy modes, 1,193
all-feature and 1,108 default tests (5 ignored, 24 suites each), both
warnings-as-errors documentation builds and the unchanged inventory self-tests
(157 routes / 159 Action sites / 258 direct reader occurrences). No new XML
instance acceptance is claimed for this state-only change.

Bounded P-E read slice implemented: `media2.GetProfiles` selection only, after
re-reading Media2 §5.1.2 and the pinned request declaration. The shared parsed
operation preserves decoded scalar values and distinguishes omitted selectors
from supplied values. Configuration projection changes only cloned profiles.
Duplicate/scalar/sequence inputs are checked before lookup. Both transports cover
empty/default/selected profile sets, repeated and combined configuration lists,
aliases and misplaced decoys, exact nested missing-profile faults, full unchanged
state and no hooks. The full-profile client still requests all configurations.
The token-discrimination table adds an explicit two-profile raw-selector row.
This does not accept profile rendering/escaping, capacity, configuration conflicts,
unmodeled catalogue storage, field-length/attribute policy or all 13 cards.

Selector evidence: both new transport controls failed against `65cd053` before
implementation (`1789044668_cargo_test.log`). Disabling sequence-order rejection
failed both controls (`1789044986_cargo_test.log`); ignoring the profile token
failed both plus the new token-table row (`1789045103_cargo_test.log`). Restored
gates passed: formatting, both workspace Clippy modes, 1,190 all-feature and
1,105 default tests (5 ignored, 23 suites each), both warnings-as-errors docs
and unchanged 159/157/258 inventory with self-tests. Strict external Xerces
corpus 09 passed its 34 selected client instances. The legacy shape probe now
explicitly requests full Media2 configurations, preserving 158 responses,
111 success payloads, 47 faults, 1,242 anchors and all unchanged zero-finding pins.
These corpora do not cover every new raw selector or all semantic rules.
Prior boundary commit `65cd053` passed hosted CI run 34478736427; this is not
hosted acceptance of the later selector slice or a release.

Bounded K16 atomicity slice: preserve existing request extraction, supported
kinds and ordinary fault payloads while replacing per-entry writes with a shared
value-based binding plan. Under one write lock, check the profile and every
required configuration token before changing any slot. Media1 passes one entry;
Media2 passes its complete pre-resolved list without constructing XML fragments.
On refusal preserve the entire state and emit no notification; successful plans
notify once, including successful idempotent removals as in the existing helper
contract. Repeated kinds retain their existing ordered last-write behavior.
This state-only slice does not settle normative repeated-kind conflicts, name-only
updates, Type=All, unsupported kinds, strict parsing, fault hierarchy or replay.
Controls must cover late unknown/empty/wrong-family tokens, valid multi-slot
write/read, fixed profiles, removal and one notification per request.

K16 verification: the original implementation failed the corrected full-state
assertion. Suppressing successful plan notifications made both new transport
controls fail at their exact committed-slot observation in a full-workspace
all-feature no-fail-fast run; the mutation was restored. Formatting, both
workspace Clippy modes, 1,179 all-feature and 1,095 default tests (5 ignored,
21 suites each), both warnings-as-errors documentation builds and unchanged
157/159/260 inventory passed. A fresh external 34-instance corpus passed strict
pinned Xerces XSD 1.1 validation; full operation semantics remain unaccepted.

Hosted run 34471659927 for prior allocation commit `2a488be` failed only the
Windows CLI line-number output comparison because `meta.elapsed_ms` differed
between subprocesses (0 versus 9); packaging was therefore skipped. It is not a
green hosted acceptance. This independent test-harness finding will be corrected
in a separate commit without changing the CLI timing contract.

Bounded K13 state slice: the externally reviewed token-uniqueness requirement
and existing duplicate refusal are sufficient to repair allocation without
migrating request parsing or fault contracts. Move the explicit duplicate check
into the same write lock as insertion, search past seeded generated-token
collisions, and notify only for a committed creation. Treat the persisted u32
counter as a search hint, not a promise that every generated token fits u32;
use a wider temporary candidate to avoid arithmetic overflow at its boundary.
Retain the serialized field type and ordinary token spelling when no collision
occurs. Verify both service entry points, seeded collisions, counter boundary,
duplicate full-state preservation and concurrent explicit/generated requests.
Capacity, name decoding/escaping, initial bindings and replay remain separate
work; this slice does not close the CreateProfile operation cards.

K13 verification: the original implementation failed the corrected collision
assertion. Disabling duplicate rejection and perturbing the counter start caused
all three new helper controls to fail on their intended payload/invariant checks
in a full-workspace all-feature no-fail-fast run; changes were restored. Formatting,
both workspace Clippy modes and both warnings-as-errors documentation builds
passed. The restored suite passed 1,177 all-feature and 1,093 default tests (5
ignored, 20 suites each); inventory remains 157 routes / 159 Action sites / 260
direct reader occurrences. These results do not close capacity or HTTP/parser
acceptance. Prior deletion-effect commit `340fc89` passed hosted CI run
34470506265; that hosted result does not cover this subsequent allocation change.

| Axis | Existing evidence or exact next case | State |
| --- | --- | --- |
| C01 | Source index and runtime alias/body/service mismatch controls implemented; HTTP binding policy remains open under W03/W07 | PARTIAL |
| C02 | `delete_profile_preserves_escaped_and_whitespace_identity`; K15 markup baseline; add literal name/token round trips on create/get/bind | PARTIAL |
| C03 | `delete_profile_rejects_ambiguous_or_mislocated_identity_without_mutation`; extend namespace/decoy controls to other 11 rows | PARTIAL |
| C04 | Add required/empty/duplicate/repeated/extension cases per field after external field review; preserve legal repeats | TODO |
| C05 | Existing `mock_token_discrimination` and `mock_media1_media2_agree`; add escaped tokens and wrong-family targets for all bindings | PARTIAL |
| C06 | K13 allocation, K14 notification and K16 partial binding have corrected regressions; broader transactions, conflicts and callbacks remain open | PARTIAL |
| C07 | `profile_name_remains_literal_text_in_both_services` and both-transport Name controls; complete token/nested renderer escaping and independent namespace/shape checks | PARTIAL |
| C08 | `unknown_token_fault_preserves_literal_text_and_state`; corpus checks missing/fixed DeleteProfile nested faults; other mappings/HTTP codes pending W05–W07 | PARTIAL |
| C09 | Common static/stateful depth/node limits covered in both transports; byte limit in-process; scoped auth and HTTP byte mapping pending | PARTIAL |
| C10 | Model limits and K12 corrected; `fixed_profile_configuration_remains_mutable_in_both_media_services` proves Add/Remove changes actual state on fixed profiles | PARTIAL |
| C11 | Selected DeleteProfile client/health first-subcode controls and K22 request selection verified; remaining consumer/CLI review pending W06 | PARTIAL |
| C12 | Audit assertions perturbed: all four known-gap tests failed at payload/state assertions; fixed-binding control also failed when expected attachment was inverted, then restored green | PARTIAL |

The K15 Name test in `tests/mock_fidelity_known_gaps.rs` now asserts the corrected
literal-text invariant. Token and nested-renderer risks remain open. Other
`known_gap_` tests still deliberately assert current defects: **passing means
reproduced, not fixed.** Convert each when repaired; never restore a defect for green.

Readiness remains blocked by engineering work W02 transitive closure, W03/W04
parsed-input boundary, W05/W06 fault design, external per-field review and explicit
atomicity/capacity behavior. No new maintainer product decision has been identified.
Next scope is to finish these designs and split profile read/create/delete and
binding migration into separately verified commits. The initial source audit did
not change handlers; subsequent selected Fault and K14 notification repairs are
recorded above and do not close the broader migration prerequisites.
