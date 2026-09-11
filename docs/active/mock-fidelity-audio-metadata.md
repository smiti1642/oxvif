# Audio and metadata batch AM1

[English](mock-fidelity-audio-metadata.md) | [繁體中文](mock-fidelity-audio-metadata_zh.md)

Baseline: 9016268, 2026-09-11. Owner: current hardening task.
Status: AM1 implemented and locally verified; hosted CI tracked after commit.
D4 was approved by the maintainer on 2026-09-11.
Scope: W01/W10/W17–W19; K24/K25/K36/K37. All 15 cards delivered as one AM1 batch.

| Section | Purpose |
| --- | --- |
| [Operation cards](#operation-cards) | Complete subgroup and shared dependencies |
| [Decisions](#decisions) | Approved defaults and new API boundary |
| [Verification](#verification) | One subgroup-level campaign and final gates |
| [Evidence](#evidence) | Reproductions and explicit pending work |

## Operation cards

All identities are project dispatch IDs, not a copied schema catalogue. Read the
paired operation ledger and the named current functions before implementation.

| ID | Current handler in services/media.rs or media2.rs | Target |
| --- | --- | --- |
| media.GetAudioSources | resp_audio_sources | Empty request; escaped bounded physical catalogue |
| media.GetAudioSourceConfigurations | resp_audio_source_configurations | Empty request; one snapshot and escaped source references |
| media.GetAudioEncoderConfiguration | resp_audio_encoder_configuration | Required scoped configuration selector; exact NoConfig |
| media.GetAudioEncoderConfigurations | resp_audio_encoder_configurations | Empty request; coherent Media1 codec view |
| media.SetAudioEncoderConfiguration | handle_set_audio_encoder_configuration / apply_audio_encoder_write | Complete candidate, readonly fields, options agreement, atomic commit |
| media.GetAudioEncoderConfigurationOptions | resp_audio_encoder_configuration_options | Configuration/profile/generic selectors; service-specific options |
| media2.GetAudioSourceConfigurations | resp_audio_source_configurations_media2 | Qualified optional selectors and logical compatibility |
| media2.GetAudioEncoderConfigurations | resp_audio_encoder_configurations_media2 | Qualified selectors; Media2 codec vocabulary |
| media2.SetAudioEncoderConfiguration | handle_set_audio_encoder_configuration_media2 | Service-specific codec input, complete candidate and shared state |
| media2.GetAudioEncoderConfigurationOptions | resp_audio_encoder_configuration_options_media2 | Selector validation and writable advertised codec/bitrate/rate combinations |
| media2.GetAudioOutputConfigurations | resp_audio_output_configurations | Scoped selectors, escaped output references and numeric views |
| media2.GetAudioDecoderConfigurations | resp_audio_decoder_configurations | Scoped selectors and escaped identity |
| media2.GetMetadataConfigurations | resp_metadata_configurations | Scoped selectors, exact missing-reference faults and complete modeled view |
| media2.SetMetadataConfiguration | handle_set_metadata_configuration | Complete candidate; no invented multicast values; refuse unsupported effects |
| media2.GetMetadataConfigurationOptions | resp_metadata_configuration_options | Generic/config/profile selectors and coherent PTZ filter support |

Shared consumers: profile renderers/catalogue snapshots in both Media services;
audio/metadata state entries and seed/persistence; types/audio.rs, types/media.rs,
MulticastConfiguration in types/video.rs; corresponding client/session methods,
replay committed-effect graph, action snapshots, roundtrip/token properties,
both-service agreement tests and external corpus.

## Decisions

D1–D3 remain in force: corrected next-minor defaults, explicit non-streaming
limits, external pinned schemas, no implicit camera write or publication.

- K24: Media2 audio must use media-subtype vocabulary, not blindly echo Media1
  labels. Keep factory-specific codec mapping explicit and distinguish G.711
  law and G.726 bitrate variants. Inspect client serialization and Other(String)
  preservation before choosing adapters; do not introduce a global lossy alias.
- K25: AutoStart is a readonly indication, not a request to start RTP. The synthetic
  device does not produce persistent streams. Do not infer it from a nonzero group
  address, mutate it through a configuration setter or claim a multicast effect.
- K36 / D4, **approved 2026-09-11**: the old public MetadataConfiguration contains only
  flattened multicast address/port, losing TTL/AutoStart and session timeout.
  Its setter places PTZStatus after Analytics and omits required closure fields.
  Correcting the mock alone breaks the public setter; inventing values is not a
  safe read-modify-write fix. Recommend a next-minor public type migration to
  preserve structured multicast settings and session timeout, strict required
  reads/writes, and explicit Rust/JSON migration notes. Prefer one source of truth
  over contradictory old/new writable fields. Media2 ignores the deprecated
  SessionTimeout value, but its required wire presence must still be handled.
  Inspect optional metadata fields and unsupported content separately; do not
  advertise arbitrary lossless roundtrips from a modeled subset.
- MulticastConfiguration currently reads IPv6 but serializes an IPv4 wrapper;
  audit this shared dependency with the approved public migration, not by hiding
  IPv6 inside the mock. Any additional public compatibility change is recorded
  before implementation.

## Verification

Start with source-authored discriminating failures against this baseline: audio
codec vocabulary; invalid late numeric fields preserving complete state/hooks;
metadata malformed request accepted despite external rejection; non-streaming
AutoStart and lost multicast information. Drive real client calls and independent
raw qualified requests through both in-process and HTTP transports.

Cover all 15 cards, positive and exact negative values, generic/config/profile
selectors, special-character identities, duplicates/wrong namespaces, supported
option combinations, read-only fields, absence versus explicit values, shared
profile rendering, malformed seeds and refusal/commit replay dependencies.
Retain existing properties, migrating only invalid fixture assumptions explicitly.

Extend the external corpus with all operations and representative refusals.
Use one whole-subgroup workspace/all-feature/no-fail-fast mutation campaign,
restore exactly, then final all/default Clippy/tests, formatting, strict docs,
inventory/link checks and external Xerces/legacy structural checks. Do not gate
individual helpers repeatedly. Update bilingual public docs, changelog, audit,
ledger and measured evidence; commit/push the subgroup only when complete.

## Evidence

VE1 is committed/pushed as 9016268; CI 34579594778 was in progress at this checkpoint.
Its local all/default workspace results are 1,257/1,154 passed, five ignored each;
110 external encoder/profile/source/rate XML instances passed. None is AM1 evidence.

At 9016268 the external source-authored offline diagnostic at
C:/Users/smiti/AppData/Local/Temp/oxvif-metadata-preflight-20260911 uses the real
OnvifClient to GetMetadataConfigurations then SetMetadataConfiguration through
MockTransport. Both return success without network or credentials.

Pinned Xerces XSD 1.1 results:
- Captured original exchanges: rejected with cvc-complex-type.2.4.a.
- Setter request with only PTZStatus ordering corrected: rejected with
  cvc-complex-type.2.4.b (incomplete content).
- Diagnostic setter with corrected order and multicast/session tail copied from
  the same synthetic getter: one instance passed. This is a structural control,
  not an implemented fix or proof of safe real-device semantics.

Official resources remain outside the checkout. Normative review:
[Media2 26.06](https://www.onvif.org/specs/2606/ONVIF-Media2-Service-Spec-v2606.pdf),
sections 5.2.5, 5.2.8 and 5.3; pinned external onvif.xsd metadata/multicast types
from packaging/schema-sources.json. Source confirms additional fields lost by the
public type. No live camera operation was performed.

Execution: replace flattened multicast fields with structured multicast plus
session_timeout in public metadata and mock snapshots. Old incomplete JSON must
be migrated explicitly; do not synthesize lost TTL/AutoStart values. Preserve
existing bool fields and document unmodeled optional metadata content separately.
Mock multicast stores configuration, never starts RTP; AutoStart is readonly.
Factory G711 maps explicitly to PCMU and AAC to MP4A-LATM. Pinned ONVIF
AudioEncodingMimeNames explicitly defines G726 as its name for bitrate variants;
AM1 therefore retains G726 plus bitrate on Media2, correcting the initial
IANA-only subtype proposal. Public Other(String) preserves device vocabulary.
No global real-device codec conversion is introduced.

Implement this complete subgroup, not a schema-only workaround.
Broader programme work remains in the execution checklist; this checkpoint does
not mark W10 or any overall milestone complete.

### AM1 delivery, 2026-09-11

- Implemented all 15 operation cards through services/audio_metadata.rs:
  scoped selectors and candidates, exact missing-reference Faults, shared
  codec/option/state views and atomic conditional commits. Profile renderers use
  the same audio view. Audio and metadata commits explicitly invalidate dependent
  replay reads; profile changes retire their profile-scoped configuration/options
  recordings. No new Action or public client method was added.
- D4 updates public MetadataConfiguration and mock MetadataEntry, client setter,
  IPv6 multicast rendering and existing fixtures. The paired
  [migration guide](../audio-metadata.md) names old Rust/JSON fields and unmodeled
  optional metadata; no invented default repairs or arbitrary lossless-edit claim.
- K37: independent corpus 08 rejected audio options (datatype validation).
  Options must emit repeated integer Items. The old client read only the first
  Items; it now retains all siblings and supports legacy whitespace-list input.
  Pinned ONVIF AudioEncodingMimeNames defines the G726 aggregate name, correcting
  the initial IANA-only subtype proposal without a global device codec alias.
- Before implementation, three discriminating runtime failures at 9016268
  (1789115957_cargo_test.log) reproduced vocabulary, false persistent streaming
  and invalid-write acceptance. The K36 external diagnostic remains historical.
- tests/mock_audio_metadata.rs covers in-process and HTTP scoped reads/options,
  all advertised audio combinations, atomic refusal and hooks, readonly fields,
  IPv6 storage, malformed seeds and both replay transports. Public client tests
  verify required paths, complete wire body, pre-transport rejection and explicit
  JSON migration. Existing roundtrip/token/cross-service tests retain their
  assertions with corrected supported values and service-specific codec views.
- One whole-workspace/all-feature/no-fail-fast mutation campaign,
  1789117105_cargo_test.log: TTL forced to 1, audio parser restricted to first
  Items, and audio/metadata replay invalidation disabled. Seven runtime failures
  in three targets caught TTL, list loss and both audio replay transports.
  Earlier audio assertions mask separate metadata-replay sensitivity, which is
  not claimed independently. All three mutations were restored exactly.
- External oxvif-corpus-20260911-09: strict Xerces XSD 1.1 passed 148/148
  instances (74 exchanges, 44 operations, 57 successes, 17 Faults); AM1 adds 19
  exchanges across all 15 cards. Corpus 08 remains the failed historical artifact.
  The explicit legacy structural check also passed: 169 responses, 110 successes,
  59 Faults, zero findings; 1518 skipped children are not validated coverage.
- Final workspace tests: all-features 1272 passed, default 1167 passed, each
  with five existing ignored cases across 39 suites. Both locked workspace/
  all-target Clippy modes and both strict workspace rustdocs pass. Formatting,
  diff whitespace and inventory/self-tests pass. The final negative assertion
  was tightened to exact Fault payload; its affected suite passed all 10 tests.
  All 337 relative file links in changed Markdown resolve; anchors are not checked.
- Removed 23 legacy reader calls: 159 Action sites, 157 routes, 191 remaining
  readers (176 production, 15 test; 56 production symbols). These are source
  inventory counts, not accepted ONVIF operations.
- Windows x64, rustc 1.97.0 (2d8144b78), cargo 1.97.0 (c980f4866).
  Schema manifest SHA256:
  65c7a558d9d96dda26eec5ef5fedaef33ae9c55e4ee00de23ca37a9650bdeb08.
  Official resources and generated validation artifacts remain outside the repo.

Reproduce using the checklist's locked workspace gates with
--target-dir target/mock-fidelity-build. The external exporter is
export_reviewed_batches_for_independent_validation in tests/mock_schema_corpus.rs
with OXVIF_MOCK_CORPUS naming a new absolute external directory. Run
packaging/verify_schemas_xerces.py validate with the pinned --root, --tool-root,
--java and --corpus described in the schema preflight. Missing resources or an
ignored export are not passes.

Next: remaining W10 URI/OSD/capability closure and subsequent service batches.
W04/W06/W07/security, expanded schema/semantic coverage, feature/platform
acceptance and PR #16 integration remain open. This subgroup does not close W10
or M0–M6. No live camera write, release/tag/publish, installation or main-branch
merge was performed. VE1 hosted CI 34579594778 passed; AM1 CI is separate.
