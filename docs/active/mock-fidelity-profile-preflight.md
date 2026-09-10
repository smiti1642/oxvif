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

## Scope and identities

The table below is the per-operation current-behavior part of each card.
All 13 cards inherit the shared-path, reference and C01–C12 sections below.
Inputs describe existing source, **not** a normative field table or approved
defaults. Action URI and source method are individually recorded in the source
index; handlers are in `src/mock/services/media.rs` or `media2.rs`, dispatch in
`src/mock/dispatch.rs`. Full URI must match its indexed value, not just its tail.
Both client service URLs come from session service discovery; MockTransport
currently ignores URL and MockServer uses its catch-all POST route.

| Ledger ID | Client → handler | Current input extraction | State/renderer path | Current ordinary Fault codes (flat unless noted) |
| --- | --- | --- | --- | --- |
| `media.GetProfiles` | get_profiles → resp_profiles | No body consumed | profiles + catalogues → render_profile | No operation-specific branch in handler |
| `media.GetProfile` | get_profile → resp_profile | GetProfile fragment → ProfileToken; absent becomes empty | profiles + catalogues → render_profile | ter:NoProfile |
| `media.CreateProfile` | create_profile → handle_create_profile | Name defaults to Profile; optional Token; legacy fragment/text | create_profile_in_state → profiles, next_token_id → render_profile | ter:ProfileExists |
| `media.DeleteProfile` | delete_profile → handle_delete_profile | required_text(ProfileToken), strict scalar identity | delete_profile_in_state → profiles → empty response | Parse: env:Sender; missing/fixed: nested s:Sender (review below) |
| `media.AddVideoSourceConfiguration` | add_video_source_configuration → handle_add_video_source_configuration | ProfileToken; ConfigurationToken with Token fallback | bind_configuration(VideoSource) → profile slot | env:Sender / ter:NoProfile / ter:NoConfig |
| `media.RemoveVideoSourceConfiguration` | remove_video_source_configuration → handle_remove_video_source_configuration | ProfileToken; legacy scalar | unbind_configuration(VideoSource) → profile slot | env:Sender / ter:NoProfile |
| `media.AddVideoEncoderConfiguration` | add_video_encoder_configuration → handle_add_video_encoder_configuration | ProfileToken; ConfigurationToken with Token fallback | bind_configuration(VideoEncoder) → profile slot | env:Sender / ter:NoProfile / ter:NoConfig |
| `media.RemoveVideoEncoderConfiguration` | remove_video_encoder_configuration → handle_remove_video_encoder_configuration | ProfileToken; legacy scalar | unbind_configuration(VideoEncoder) → profile slot | env:Sender / ter:NoProfile |
| `media2.GetProfiles` | get_profiles_media2 → resp_profiles_media2 | No body consumed (Token/Type not read) | profiles + media::catalogues → render_profile_media2 | No operation-specific branch in handler |
| `media2.CreateProfile` | create_profile_media2 → handle_create_profile_media2 | Name defaults to Profile; Configuration not read | media::create_profile_in_state(None) → profiles, counter → Token response | ter:ProfileExists branch (currently unreachable with None) |
| `media2.DeleteProfile` | delete_profile_media2 → handle_delete_profile_media2 | required_text(Token), strict scalar identity | media::delete_profile_in_state → profiles → empty response | Parse: env:Sender; missing/fixed: nested s:Sender (review below) |
| `media2.AddConfiguration` | add_configuration_media2 → handle_add_configuration_media2 | ProfileToken; repeated Configuration/Type/Token; Name not read | apply_media2_configuration(add=true) → per-entry media::bind_configuration | env:Sender / ter:ConfigurationConflict / ter:NoProfile / ter:NoConfig |
| `media2.RemoveConfiguration` | remove_configuration_media2 → handle_remove_configuration_media2 | ProfileToken; repeated Configuration/Type/Token | apply_media2_configuration(add=false) → per-entry media::unbind_configuration | env:Sender / ter:ConfigurationConflict / ter:NoProfile |

## Shared paths and current behavior

See the [pipeline preflight](mock-fidelity-pipeline-preflight.md) for the
selected transitive closure, consumer map and P-A–P-E implementation order.
K17 tracks pre-success replay invalidation separately from K14's state hook.

- Request: session wrappers (and media-version preference/fallback) → client
  methods → `OnvifClient::call` envelope/security → MockTransport/MockServer →
  FaultResponder → AuthResponder → optional replay → SyntheticResponder →
  dispatch. These 13 client methods escape caller strings; old mock extraction
  does not decode them. Empty-body handlers bypass validation today.
- Scalar extraction: `xml_parse::{extract_tag,extract_all_tags,extract_attr}`
  locate local names and return trimmed/raw fragments. Delete uses
  `request::required_text` instead. Binding's synthesized per-entry fragment is
  not a standalone XML document; the future shared helper must take parsed
  identity/value objects instead of repeatedly parsing that fragment.
- Creation: explicit duplicate check precedes the write lock; generated counter
  values are not checked for collision. `ProfileEntry` is appended with all
  configuration slots None and fixed false. There is no modeled capacity check.
  Media2 ignores initial Configuration entries. K13 is reproduced for both
  services; the concurrency race remains source-derived/unreproduced.
- Deletion: `delete_profile_in_state` locates token under write lock, refuses
  fixed/missing, otherwise removes one profile. K14 is fixed through an explicit
  committed-outcome notification predicate; refusals preserve state and skip the
  hook, while successful deletion notifies once. This is not rollback or a change
  to public state-helper semantics. No profile-change event or reference-cascade audit has
  been completed; do not claim absence of those effects is conformant.
- Binding: `ConfigKind::{from_media2_type,known_token,slot}` select five modeled
  kinds. `bind_configuration` checks profile/config under separate read locks,
  then modifies one slot. `unbind_configuration` clears the slot after existence
  checks. Fixed does not prevent either. Media2 prechecks kinds but validates
  tokens while applying entries: K16 proves a later fault leaves an earlier
  binding. Unsupported kinds, Type=All, name-only updates and configuration
  conflicts need reviewed target behavior, not accidental fallbacks.
- Rendering: `catalogues` snapshots configurations separately from profile
  snapshot; `render_profile` / `render_profile_media2` inline VSC, encoder,
  audio source/encoder and PTZ render helpers. Profile name/token interpolation
  is currently raw; K15 proves Name becomes an XML child in both services.
  Further nested renderers' escaping and snapshot atomicity remain W10/W18.
- State hooks: `MockState::{modify,modify_returning,notify}` execute notify after
  writing, while the callback receives a read guard. Reentrancy/lock policy and
  failed-write replay invalidation require W18/W19; no external persistence is
  implemented by the mock itself.
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
device to omit configuration data. The mock ignores selectors and masked this.
The client now explicitly sends `Type=All`, without adding parameters or changing
return types; the session delegates to that method. Its existing field test now
records and asserts endpoint, complete Action and request selection. The old
request failed this assertion during a full all-feature no-fail-fast run.
Mock selector semantics and broader negative inputs remain W10/P-E work; XSD
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

| Axis | Existing evidence or exact next case | State |
| --- | --- | --- |
| C01 | Source index reconciles full Actions; add runtime alias/body/service mismatch rejection controls under W03/W07 | PARTIAL |
| C02 | `delete_profile_preserves_escaped_and_whitespace_identity`; K15 markup baseline; add literal name/token round trips on create/get/bind | PARTIAL |
| C03 | `delete_profile_rejects_ambiguous_or_mislocated_identity_without_mutation`; extend namespace/decoy controls to other 11 rows | PARTIAL |
| C04 | Add required/empty/duplicate/repeated/extension cases per field after external field review; preserve legal repeats | TODO |
| C05 | Existing `mock_token_discrimination` and `mock_media1_media2_agree`; add escaped tokens and wrong-family targets for all bindings | PARTIAL |
| C06 | K13/K16 remain reproduced; `rejected_delete_preserves_state_and_hook_but_success_notifies` now guards repaired K14 refusals, success and public-helper compatibility | PARTIAL |
| C07 | `known_gap_k15_profile_name_is_interpreted_as_markup`; complete nested renderer escaping and independent namespace/shape checks | GAP REPRODUCED |
| C08 | `unknown_token_fault_preserves_literal_text_and_state`; corpus checks missing/fixed DeleteProfile nested faults; other mappings/HTTP codes pending W05–W07 | PARTIAL |
| C09 | Generic parser limits covered only on migrated DeleteProfile; auth boundary/resource-limit coverage for other paths pending | TODO |
| C10 | Model limits and K12 corrected; `fixed_profile_configuration_remains_mutable_in_both_media_services` proves Add/Remove changes actual state on fixed profiles | PARTIAL |
| C11 | Selected DeleteProfile client/health first-subcode controls and K22 request selection verified; remaining consumer/CLI review pending W06 | PARTIAL |
| C12 | Audit assertions perturbed: all four known-gap tests failed at payload/state assertions; fixed-binding control also failed when expected attachment was inverted, then restored green | PARTIAL |

K13/K15/K16 tests in `tests/mock_fidelity_known_gaps.rs` deliberately assert current
defects like the existing Broken/Blind property tables. **Passing means reproduced,
not fixed.** Convert each to the corrected invariant and update its finding when
implementing the fix; never preserve a defect merely to restore green.

Readiness remains blocked by engineering work W02 transitive closure, W03/W04
parsed-input boundary, W05/W06 fault design, external per-field review and explicit
atomicity/capacity behavior. No new maintainer product decision has been identified.
Next scope is to finish these designs and split profile read/create/delete and
binding migration into separately verified commits. The initial source audit did
not change handlers; subsequent selected Fault and K14 notification repairs are
recorded above and do not close the broader migration prerequisites.
