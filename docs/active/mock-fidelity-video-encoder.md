# Video encoder batch VE1

[English](mock-fidelity-video-encoder.md) | [繁體中文](mock-fidelity-video-encoder_zh.md)

Baseline: `e6962b5`, 2026-09-11. Owner: current hardening task. W01/W10/W17–W19.

| Section | Purpose |
| --- | --- |
| [Operation cards](#operation-cards) | Complete encoder subgroup and shared consumers |
| [Decisions and risks](#decisions-and-risks) | Contract, model and API boundary |
| [Verification](#verification) | Discriminating cases and batch gates |
| [Evidence](#evidence) | Actual results, unresolved decisions and handoff |

## Operation cards

Actions are the client source namespace plus `/` and the operation suffix. Media1:
`http://www.onvif.org/ver10/media/wsdl`; Media2: `http://www.onvif.org/ver20/media/wsdl`.
The operation body has the same qualified identity. Client entry points are in
`src/client/media.rs` / `media2.rs`, with session wrappers in `src/session.rs`.
The existing shared Action resolver remains responsible for routing.

| ID | Baseline handler / input | Target behavior |
| --- | --- | --- |
| media.GetVideoEncoderConfigurations | `media::resp_video_encoder_configurations(state, body)`; global optional token on an empty-message operation | Validate empty body, escaped complete catalogue, explicit unrepresentable-view policy |
| media.GetVideoEncoderConfiguration | `media::resp_video_encoder_configuration`; global required token | Qualified unique scalar, one snapshot and structured NoConfig |
| media.GetVideoEncoderConfigurationOptions | `media::resp_video_encoder_configuration_options`; global required token, ignored profile | Optional configuration/profile context, generic options, coherent writable ranges/codecs and retained deep-options control |
| media.SetVideoEncoderConfiguration | `media::handle_set_video_encoder_configuration` → `apply_video_encoder_write` | Complete scoped candidate, required persistence input, no partial write, explicit unsupported settings |
| media2.GetVideoEncoderConfigurations | `media2::resp_video_encoder_configurations`; global token, unknown becomes empty list | Scoped configuration/profile selectors, structured missing-reference Faults and escaped Media2 view |
| media2.GetVideoEncoderConfigurationOptions | `media2::resp_video_encoder_configuration_options_media2`; global required token and fixed codec/rate lists | Same model as setter, distinct per-source limits and service-specific wire shape |
| media2.SetVideoEncoderConfiguration | `media2::handle_set_video_encoder_configuration` → shared global writer | Service-specific attribute/element contract, atomic candidate and committed effect |
| media2.GetVideoEncoderInstances | `media2::resp_video_encoder_instances()`; ignores input | Required **source configuration** token, modeled capacity rather than live usage, structured NoConfig |

Shared consumers: `VideoEncoderState`, both inline profile renderers, Media1
`render_vec_body`, Media2 `render_video_encoder`, `VideoEncoderConfiguration`,
`VideoEncoderConfiguration2`, replay dependencies, source reassignment and profile
reference counts. Review all those paths together. Do not add unrelated operations.

## Decisions and risks

Primary review: [Media1 v24.12](https://www.onvif.org/specs/2412/ONVIF-Media-Service-Spec-v2412.pdf)
§5.5 and [Media2 v26.06](https://www.onvif.org/specs/2606/ONVIF-Media2-Service-Spec-v2606.pdf)
§5.2.3/5.3.2–5.3.5, plus the pinned external schema set. Official resources and
schema-derived tables/fixtures remain outside the repository.

- **K26:** H265 can enter shared state and then an invalid Media1 view. Keep codec
  identity unchanged; refuse a Media1 response that cannot represent its selected
  encoder with an explicitly labeled receiver model-policy Fault. Do not silently
  omit a binding/configuration or invent a codec conversion. Media2 remains usable.
- **K34 baseline defect:** Media2 rate control uses a public `u32` field and parses 12.5 as zero;
  mock state also stores integers. This is source-confirmed, with a dedicated
  reproduction required. The maintainer approved the next-minor `f32` migration
  on 2026-09-11 and requested implementation. Preserve optional absence versus
  malformed present fields, reject nonfinite outbound rates before transport,
  migrate shared mock storage and reject unrepresentable Media1 views explicitly.
  Retain old integer JSON loading; document Rust/JSON migration. Do not hide the
  client defect by weakening a test.
- **K35:** Global partial setters, unescaped names/tokens, ignored selectors and
  unvalidated numeric fields can disagree with options. Validate the complete
  modeled value before one conditional state commit. Preserve read-only UseCount;
  omission of optional rate control retains its current modeled value.
- Quality/rate may adapt; syntactically valid out-of-range bitrate must adapt.
  Use one explicit synthetic options policy for both reads and writes. Keep
  per-encoder resolution lists and distinct physical-source limits. Advertise
  JPEG as well as H264 when writable. Do not infer device encoding performance
  or actual RTP effects from these synthetic settings.
- Media1 codec children and Media2 codec attributes are not interchangeable.
  Require all modeled mandatory fields and validate nested numbers, booleans,
  unique direct fields and namespaces. Explicitly refuse unmodeled extension,
  multicast destination, encoding interval, signing or guaranteed-rate changes;
  accept the unchanged no-streaming defaults needed for Get→Set roundtrips.
- Source/profile contexts follow the documented logical reassignment model;
  physical routing conflicts are not claimed. Instances describe an explicit
  per-source synthetic capacity, not current binding counts. Configuration,
  source and profile effects must retire their actual dependent recordings.
- Unknown configuration/profile uses Sender/InvalidArgVal with NoConfig/NoProfile;
  unassignable modeled settings use ConfigModify. Structural errors use the
  common boundary policy; unsupported effects use the explicit model policy.

## Verification

VE1 execution resumed at `87cbf6c` on 2026-09-11. The remaining eight cards move
together. The synthetic options model keeps each encoder's resolution catalogue,
supports H264/JPEG and H265 only on the high-capacity source, and accepts integer
rates throughout its range plus advertised fractional Media2 rates. Generic options
are a device-wide union: existing encoder resolution catalogues have no common
intersection. Callers must re-query with the actual configuration before writing.
No arbitrary default channel is selected. Profile references are
validated under the existing all-profile logical compatibility policy. Quality and
rate adapt deterministically; signed out-of-range bitrate clamps. Optional rate
omission retains state; unknown settings refuse, not disappear. Media1 accepts
only the unchanged no-streaming multicast/session/interval defaults. Capacity is
synthetic, source-configuration-selected and bounded by the profile limit, not a
measurement of streaming performance. Tests must update obsolete permissive
fixtures explicitly; preserve existing deep-options discrimination.

Add `tests/mock_video_encoder.rs` with shared in-process/HTTP driver controls:
complete snapshots/hooks after late-invalid fields; valid Get→Set→Get; escapes,
duplicates, wrong namespaces, nested decoys, nonfinite floats and numeric bounds;
generic/profile/config selectors; two-source options; advertised-value writes;
bitrate adaptation; H265 versus Media1 view; required source-token instances;
refused versus committed replay. Add the independent client fractional-rate
reproduction without using the production parser as the oracle. Keep existing
roundtrip, token, workflow and deep-options tests, correcting only invalid input
or obsolete expectations with an explicit reason.

Extend the credential-free external corpus with all eight operations, write
readback and representative Faults. Perform targeted old-code reproduction,
one whole-subgroup mutation campaign (workspace/all-feature/no-fail-fast), exact
restoration and the final five gates. Run strict docs, inventory and external
validation once for the batch, repairing only affected checks as necessary.
The newly approved public rate-control contract is an independently verifiable
K34 delivery within VE1: client read/write, shared mock rate and Media1 views,
finite-value refusal, serde compatibility and migration documentation move together.
Run its one mutation/final-gate cycle at this boundary; do not gate individual helpers.
The remaining eight-operation selector/options/complete-candidate work retains its
own whole-subgroup cycle. This separates a public API migration from unrelated
encoder semantics without claiming VE1 complete. Update paired guides, changelog,
audit/ledger/checklist and evidence at each delivery. No live writes, releases,
installation or main-branch merge.

## Evidence

Preflight recorded before VE1 runtime changes. K34 is reproduced through the
public client at `e6962b5` using an external, source-authored offline responder:
`25` parses as 25 (control), while `12.5` parses as **0** and fails the explicit
value assertion. The reproducer is outside the checkout at
`C:/Users/smiti/AppData/Local/Temp/oxvif-fractional-rate-20260911`.
Command: `cargo run --offline --manifest-path <reproducer>/Cargo.toml --target-dir target/mock-fidelity-docs`.
The external diagnostic has its own lockfile; it is not a replacement for locked
workspace gates or independent schema validation. No camera/network call occurs.

K34's public API choice was approved on 2026-09-11 and is implemented as the
rate-contract delivery below. Media1's own public integer type is unchanged.
The later eight-operation VE1 delivery is recorded below. The next service
subgroup after VE1 is audio/metadata; VE1 alone will not close W10.
VS1 is committed/pushed as `e6962b5`; CI `34574805465` subsequently passed.

### K34 delivery, 2026-09-11

- `VideoRateControl2` and `VideoEncoderState` use `f32`. Optional absence is kept;
  present incomplete, duplicate, nested or invalid rate fields return exact parse
  errors. Negative/nonfinite outbound values refuse before transport. Serde accepts
  ordinary old integer JSON and rejects invalid rates instead of persisting `null`.
- Both mock setters validate the qualified rate block before mutation and perform
  existing field writes under one conditional lock. Media1 rejects fractional or
  otherwise unrepresentable rates explicitly at top-level encoder/profile views;
  invalid seed rates are not repaired or emitted. Unselected independent encoders
  remain readable. A successful write retires dependent built-in replay reads;
  refusals retain recordings. Other fields remain the K35 legacy boundary.
- Public regressions: `tests/media2_rate_control.rs` covers fractional reads,
  absence/malformed/ambiguous rate control, exact field errors, captured outbound
  Action/body, pre-transport refusals and JSON migration. The initial old-code
  failures are `fractional_rate_survives_public_client_read` and
  `malformed_rate_is_not_zero` (RTK `1789112740_cargo_test.log`). The external
  reproducer also passes after correction: wire 25 → 25 and 12.5 → 12.5.
- `tests/mock_video_rate.rs`: `rate_state_and_views` and its HTTP counterpart
  assert full state/hook preservation on invalid later rate fields and qualified
  structure; they verify shared fractional storage, both service views and integral
  recovery. `public_mock_rate_roundtrip_and_old_snapshot` checks 29.97, omission
  preservation and old integer JSON. `invalid_seed_rates_are_explicit_and_not_persisted_as_null`
  checks exact Faults, unchanged seeds, persistence refusal and explicit zero.
  `rate_replay_commit_boundary` and its HTTP counterpart check refusal/commit
  dependency behavior and preservation of an unrelated recording.
- One unfiltered workspace/all-feature/no-fail-fast mutation campaign
  (`1789113536_cargo_test.log`) produced **10 runtime failures in three targets**:
  client fractional read, outbound guard and JSON tests; all six mock-rate tests;
  and corpus `captures_fractional_rate_and_explicit_media1_limit`. Mutations floored
  parsed rates and disabled outbound, serialization and Media1 view guards. All
  four changes were restored exactly. This proves these failure paths, not every
  independent assertion or complete encoder semantics.
- Local all-feature workspace tests: **1,250 passed, 5 ignored, 37 suites**;
  default workspace: **1,147 passed, 5 ignored, 37 suites**. Both locked
  workspace/all-target Clippy modes pass with `-D warnings`; separate `mock` and
  `serde` all-target sweeps also pass. Formatting, diff whitespace and strict
  all-feature/default workspace rustdocs pass. New plan/migration link targets exist.
- The explicit external exporter adds seven rate exchanges across three operations.
  Strict Xerces XSD 1.1 passed **84/84 instances** in new external directory
  `oxvif-profile-corpus-20260911-06`: 42 exchanges, 24 operations, 30 successes and
  12 Faults. Explicit legacy shape validation passed. These are selected structure
  checks, not full semantic conformance; ignored tests are not counted as passes.
- Inventory/self-tests pass: 159 Action sites, 157 routes, **231** direct readers
  (216 before top-level test modules, 15 inside; 68 production enclosing symbols).
  Both ledgers and source audits agree. Public migration documentation is in
  [English](../media2-frame-rate.md) / [繁體中文](../media2-frame-rate_zh.md).

Historical K34 handoff: local delivery gates passed; hosted CI was not included then.
No release,
installation, live write, main-branch merge or contributor PR merge occurred.
Next at that handoff: execute the remaining VE1 operation cards, including K26 codec views and
K35 complete candidates/options/selector/capacity semantics. Broader HTTP/security,
other services, feature/platform acceptance and PR #16 integration remain in the
[execution checklist](mock-fidelity-execution-checklist.md); no new decision is required
for K34 or the already approved VE1 policy.

### VE1 encoder delivery, 2026-09-11

All eight operation cards now route through the shared scoped encoder module.
K26 codec views and K35 candidates/selectors/options/capacity are implemented
within the [documented non-streaming model](../mock-server.md#622-encoder-configuration-contract).
Generic encoder options are a device-wide union; profiles use the existing logical
compatibility policy. No actual routing, RTP, signing, CBR or hardware capacity is
claimed. Optional rate absence retains prior values even on codec changes.

- Baseline runtime reproduction at 87cbf6c failed all three new regression tests:
  missing configuration incorrectly succeeded, late invalid quality changed state,
  and out-of-range bitrate was not adapted. RTK: 1789114140_cargo_test.log.
- tests/mock_video_encoder.rs covers both transports with exact Fault chains/reasons,
  full-state/hook refusal checks, qualified selectors, escaped identities,
  required Media1 fields and unmodeled interval/multicast/timeout refusal.
  Public roundtrips write every advertised Media2 frame rate and resolution for
  every seeded encoder/codec. Separate checks cover huge finite rate adaptation,
  signed bitrate bounds and H265/Media1 view refusal. Existing source and profile
  replay drivers now verify capacity/options recordings survive refusal and retire
  after commit. The all-profile compatibility policy is explicit, not physical routing.
- One full workspace/all-feature/no-fail-fast mutation campaign
  (1789115078_cargo_test.log) disabled bitrate clamping, Media1 codec guarding,
  source-capacity invalidation and profile-encoder-options invalidation.
  Eight runtime failures detected bitrate and both replay dependencies across
  four targets. Earlier bitrate assertions masked the codec mutation, so the
  campaign is not independent sensitivity evidence for that guard. All mutated
  sources were restored exactly before final gates.
- The same run exposed three obsolete fixture expectations outside those eight
  failures: capacity supplied VEC_1 rather than VSC_1; two cross-service tests
  wrote resolutions absent from VEC_1 options. These now use valid advertised
  settings while retaining exact readback and cross-service assertions.
- Removed 17 legacy reader calls. Inventory/self-tests pass: 159 Action sites,
  157 routes, 214 readers (199 production, 15 test; 63 production symbols).
  Token discrimination is 35 rows: 30 discriminating, 5 explicit blind cases.
- The external exporter adds 13 encoder exchanges across all eight operations.
  Strict Xerces XSD 1.1 passed 110/110 instances in external directory
  oxvif-profile-corpus-20260911-07: 55 exchanges, 29 operations, 41 successes,
  14 Faults. Explicit legacy structural validation passed. This is selected
  structural evidence, not complete ONVIF semantic conformance.
- Final workspace gates pass: all-feature 1,257 tests, default 1,154 tests,
  each with five existing ignored cases across 38 suites. Both locked all-target
  Clippy modes pass with -D warnings; formatting, diff whitespace, both strict
  workspace rustdocs, inventory/self-tests and 282 relative file links pass
  (link anchors not checked by that scan). Previous K34 commit 87cbf6c
  passed hosted CI 34577502721; that run does not include VE1.

Next subgroup: audio/metadata (K24/K25), then the remaining W10 and other service
work in the execution checklist. W04/W06/W07/security, independent corpus expansion,
feature/platform acceptance and PR #16 remain open. No release, install, main-branch
merge, contributor PR merge or live camera write is included in this delivery.
