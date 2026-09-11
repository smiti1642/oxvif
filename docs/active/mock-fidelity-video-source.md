# Video source batch VS1

[English](mock-fidelity-video-source.md) | [繁體中文](mock-fidelity-video-source_zh.md)

Baseline: `7e9b68f`, 2026-09-11. Owner: current hardening task. W01/W10/W17/W18/W19.

| Section | Purpose |
| --- | --- |
| [Operation cards](#operation-cards) | Complete source-selected subgroup |
| [Decisions](#decisions) | Reviewed contract and compatibility boundaries |
| [Verification](#verification) | Cases, mutation and gates |
| [Evidence](#evidence) | Actual results and next subgroup |

## Operation cards

Actions are the exact namespace below plus `/` and the operation suffix in each ID.
Media1 namespace: `http://www.onvif.org/ver10/media/wsdl`; Media2:
`http://www.onvif.org/ver20/media/wsdl`. Operation body uses that namespace and suffix.
Client entry points are in `src/client/media.rs` and `media2.rs`, with corresponding
session wrappers in `src/session.rs`; endpoint routing remains the shared synthetic
Action resolver, not a new HTTP path restriction.

| ID | Existing handler and source input | Output / target |
| --- | --- | --- |
| media.GetVideoSources | `media::resp_video_sources(state)`; ignores body | Physical source catalogue, unescaped tokens; validate empty operation, escape identities |
| media.GetVideoSourceConfigurations | `media::resp_video_source_configurations(state)`; ignores body | Whole configuration catalogue; validate empty operation, shared escaped renderer |
| media.GetVideoSourceConfiguration | `media::resp_video_source_configuration`; global required ConfigurationToken | One configuration or flat missing/unknown fault; direct unique scalar, structured NoConfig |
| media.GetVideoSourceConfigurationOptions | `media::resp_video_source_configuration_options`; global required ConfigurationToken, ProfileToken ignored | Bounds maxima currently come from mutable crop, literal profile limit 5; support optional selectors and sensor-derived ranges |
| media.SetVideoSourceConfiguration | `media::handle_set_video_source_configuration` → `apply_video_source_write`; global Configuration token, Name, SourceToken, Bounds width/height; ignores offsets/persistence | Partial mutation with separate existence lock, malformed numbers ignored; complete scoped candidate, atomic commit and explicit unsupported-setting refusal |
| media2.GetVideoSourceConfigurations | `media2::resp_video_source_configurations_media2(state)`; ignores both selectors | Duplicated unescaped catalogue renderer; scoped ConfigurationToken/ProfileToken and shared renderer |
| media2.GetVideoSourceConfigurationOptions | `media2::resp_video_source_configuration_options_media2`; same global-token/crop-limit defect | Same sensor-derived options contract, service-specific wrapper |
| media2.SetVideoSourceConfiguration | `media2::handle_set_video_source_configuration_media2` → shared writer | Same atomic state contract, without Media1 persistence member |

Shared paths: `VideoSourceConfigEntry`, `VideoSourceEntry`, `MockState::modify_returning_if`,
both profile renderers through `media::render_vsc_body`, `request::Node`, `fault.rs`,
`effect.rs`, built-in replay. Client `VideoSourceConfiguration` and `SourceBounds`
serialize the full modeled value; no public field/signature changes are planned.

Source-confirmed risks recorded before implementation: **K31** crop-dependent option
ceilings and unchecked/dropped bounds; **K32** global/missing/duplicate selector handling,
unescaped nested output and partial writes; **K33** refused source writes retire replay
before the synthetic result is known. Runtime reproduction and disposition are recorded below.

## Decisions

Reviewed primary references: [Media1 v24.12](https://www.onvif.org/specs/2412/ONVIF-Media-Service-Spec-v2412.pdf)
§5.3–5.4 and [Media2 v26.06](https://www.onvif.org/specs/2606/ONVIF-Media2-Service-Spec-v2606.pdf)
§5.2.2 and §5.3.2–5.3.4; pinned external schema manifest
`packaging/schema-sources.json`. Schema tables/derived fixtures stay outside the checkout.

- Parse once at dispatch; handlers consume the qualified operation and nested configuration.
  Preserve decoded identity/Name; reject duplicate scalar/configuration fields and malformed
  numeric values before touching state. UseCount is validated but not caller-writable.
- Require the complete modeled source configuration. Support only zero-origin crop storage.
  Nonzero offsets and unmodeled configuration settings must be explicitly refused, not silently
  discarded. Positive crop sizes may be adapted to the selected sensor's bounds; subsequent
  getters return the actual committed size. Unknown source references cannot be stored.
- Media1 persistence input is validated as a boolean; the mock only commits in-memory state
  and its existing caller-owned persistence hook. Do not promise an emulated reboot/disk effect.
- Empty list requests enumerate; Media2 explicit unknown configuration/profile references fault.
  Profile context is validated, but this model currently permits source reassignment for all
  profiles: it has no physical encoder-routing conflict model. Do not present that as a device
  compatibility guarantee. Generic/profile-only options aggregate available sources conservatively;
  explicit configuration options use its selected physical sensor, not its current crop size.
- Options advertise zero origin, positive dimensions, the modeled profile capacity and real
  source tokens. Arbitrary invalid caller-authored snapshots are not repaired by a read.
- Reviewed unknown configuration/profile and unassignable-setting refusals use structured
  Sender/InvalidArgVal with NoConfig, NoProfile or ConfigModify respectively. XML structural
  failures use the existing generic boundary policy; model limitations are labeled as such.
- Successful commits notify once and retire dependent source/profile/options recordings on
  built-in replay. Refusals preserve the entire state, hook count and recordings. Standalone
  replay construction, full HTTP binding, encoder settings and real media effects remain separate.

## Verification

`tests/mock_video_source.rs`: `source_fields_and_atomicity` (C01–C06, C09),
`source_selectors_and_options` (C01–C05/C07), paired HTTP wrappers (C10/C11),
`source_replay_commit_boundary` and its HTTP wrapper (C06/C11). Source snapshots,
distinct sensor limits, Unicode/escaped/whitespace identities, duplicate and nested decoys,
invalid final fields, source reassignment, read-only counts, clamp/readback, generic/profile
selectors, exact structured Faults and unchanged rejected writes are required assertions.
Existing multi-sensor, roundtrip, profile-assembly and cross-service agreement tests stay active.
Independent external source corpus must include successful reads/writes and refusals;
existing client-parser agreement alone is not namespace/XSD evidence (C07/C08).

First run focused old-code regressions. Implement the whole subgroup; run targeted checks
only where useful. One unfiltered workspace/all-feature/no-fail-fast mutation campaign will
perturb source selection/atomic commit, then restore exactly. Run the final five gates once,
strict docs and relevant external checks; fix/rerun affected failures rather than repeating
unaffected baselines. Update paired guides, CHANGELOG, ledgers, audit counts and this evidence.
No hardware write, release, installation or main-branch merge is authorized by this batch.

## Evidence

Implemented the eight cards through the shared qualified `video_source` helper;
full candidates validate before one state commit, readers/renderers preserve decoded
identities, physical-source options survive crop changes, and built-in replay retires
dependent reads only after success. Rows remain PARTIAL for the wider contract.

Verification on Windows, isolated build targets, restored source:

- Old-code regressions failed on invalid final bounds returning success and options
  shrinking with the crop. Existing two-sensor/client workflow controls were retained.
- One full workspace/all-feature/no-fail-fast campaign simultaneously ignored source
  selection and mutated state on refused input. Twelve runtime assertions failed over
  35 suites (log `1789111312_cargo_test.log`); source options, full-state refusals,
  replay and corpus controls were sensitive. Both perturbations were exactly restored.
  This combined campaign does not independently prove every field or concurrency rule.
- Final fmt and both workspace/all-target Clippy modes passed with warnings denied.
  All features: **1,238 passed, 5 ignored**; default: **1,137 passed, 5 ignored**,
  each over 35 suites. Final review corrected overlong selector faults to generic
  InvalidArgs rather than setter-specific ConfigModify; affected all-feature Clippy
  and tests passed again. Default behavior was unaffected.
- Both strict rustdoc modes passed. The selected external legacy shape check passed.
  Independent strict Xerces XSD 1.1 validated **70/70** external instances (35 exchanges,
  21 operations, ten Faults) in `oxvif-profile-corpus-20260911-05`.
- Inventory/self-tests passed: 159 Action sites, 157 routes and **233** direct readers
  (218 before top-level test modules, 15 inside; 68 enclosing production symbols).
  VS1 removed five legacy calls. These counts do not measure completed operations.

Broader scalar/operation-attribute rules, arbitrary imported snapshot validation,
physical routing compatibility, full HTTP binding and every ordinary Fault branch
remain open. This is not ONVIF certification, a native three-platform acceptance,
a real-camera write test or release approval. No system binary was replaced.
Next complete subgroup is encoder configuration/options/instances (the remaining eight video
operations); audio/metadata follows. VS1 does not close W10 or the whole programme.
