# PR 16 integration review

[English](mock-fidelity-pr16-integration.md) | [繁體中文](mock-fidelity-pr16-integration_zh.md)

2026-09-11 source review; W26 implemented on the integration candidate; local gates pass, hosted CI pending.

The [contributor integration plan](contributor-pr-integration-plan.md) now owns
execution order, current prerequisite checks, acceptance and branch boundaries for
#14/#17/#16. It supersedes the historical prerequisite sequence below; the review
findings below describe the contributor head, not the rewritten candidate; current evidence is in that plan.

| Section | Purpose |
| --- | --- |
| [Reviewed revision](#reviewed-revision) | Immutable head and incremental changes |
| [Disposition](#disposition) | Reusable code and required corrections |
| [Integration sequence](#integration-sequence) | Dependencies and acceptance |
| [B16 operation cards](#b16-operation-cards) | Current bounded implementation contracts |

## Reviewed revision

[PR #16](https://github.com/smiti1642/oxvif/pull/16), by David Matthew Mattli
(`dmm`), remains open at `3db6459b03b6977b3a41b6055ee3f8ac9f42b049`.
Compared with previously reviewed `dc69e9aaf46c607d1cffa4b55a1e733ba323f5b1`,
the new head merges master's `9dccf9d` CLI line-number work. The incremental
changed-file list contains no Media implementation or test changes. The complete
current PR diff was reviewed again; no current-head execution is claimed here.
GitHub's observed merge state was UNSTABLE, not proof of an implementation defect
or successful CI. The contribution adds two Media operations, not a broad Media2
service expansion.

Official references rechecked: [Media1 24.12, section 5.18.1](https://www.onvif.org/specs/2412/ONVIF-Media-Service-Spec-v2412.pdf)
and [Media2 26.06, section 5.7.1](https://www.onvif.org/specs/2606/ONVIF-Media2-Service-Spec-v2606.pdf).
Normative payload/fault details remain in external review notes, not copied tables
or schema-derived fixtures in this repository.

## Disposition

- Client methods and session delegation are suitable integration candidates:
  complete Action strings, service-specific wrappers, escaped caller tokens and
  expected-response checks. Preserve the contributor's attribution when porting.
- The new mock handler still uses legacy `extract_tag` and compares raw escaped
  text with state tokens; it does not use the hardening branch's scoped parser.
  An existing-profile check is followed by unconditional acknowledgment, with no
  modeled stream effect. Do not transplant that implementation or call it faithful.
- Mock faults remain flat; use the reviewed structured serializer and preserve
  the existing client first-subcode contract. Generic malformed/ambiguous fields
  must fail before any state/effect observation.
- Two client positives check Action and escaped body, but the PR adds no dedicated
  negative client/fault assertions, session routing controls or mock token/state
  controls. The two workflow additions only unwrap success. An action snapshot
  and a schema-shape probe cannot prove media delivery or I-frame emission.
- Operation documentation is not completely bilingual in the diff:
  `OPERATIONS_zh.md` is not updated. Internal Media reference pages already had no
  translated counterparts in the base repository; that is not a contributor omission. Public
  descriptions also need an explicit mock acknowledgment/effect boundary and
  should describe profile-associated streams, not only video I-frames.

## B16 operation cards

Implementation authorized 2026-09-11; baseline `07a7d61`. Scoped requests, faults,
profile catalogues and exact-operation policy now exist. Official sections above
were rechecked for unknown-profile failures and associated-stream semantics.
Detailed normative material remains external; these are project behavior cards.

| Field | `media.SetSynchronizationPoint` | `media2.SetSynchronizationPoint` |
| --- | --- | --- |
| Action | `http://www.onvif.org/ver10/media/wsdl/SetSynchronizationPoint` | `http://www.onvif.org/ver20/media/wsdl/SetSynchronizationPoint` |
| Client/session | `media_set_synchronization_point`, Media1 endpoint | `set_synchronization_point_media2`, Media2 endpoint |
| Input | Unique scoped `ProfileToken`; decode once, opaque identity, no trimming | Same input in Media2 namespace, not local-name search |
| Code path | `services/media.rs::handle_set_synchronization_point`, borrowed Node, structured Fault, shared profile catalogue | Separate dispatch arm calling the checked helper with explicit service identity |
| Behavior | Default refusal; exact-operation opt-in acknowledgment after shape/profile validation | Separate Media2 opt-in, never enabled by Media1 or Events |
| Output/fault | Service-specific empty response; reviewed nested unknown-profile Fault; generic malformed request policy | Same classification, own response namespace |
| State/effect | One coherent profile read; no state writes, hooks, queues, RTP or replay retirement | Shared catalogue, no cross-service mutations |
| Replay | Recorded reads and explicit fault injection retain precedence; recorded write receipts are excluded and synthetic fallback obeys policy | Same, including identity controls |
| Compatibility | Additive methods, no default mock success promise | No implicit fallback to Media1 or Events |
| Tests/delivery | S01–S08, client tests, `tests/mock_media_sync.rs`, corpus, W26/inventory/paired docs | Same plus cross-service policy isolation |

C01–C05 map to S01/S04/S08; C06 to S05; C07/C08 to S02/S07. C09 reuses auth
and common request controls with no new exemption (test invalid auth cannot ack).
C10 maps to S03/S05; C11 to S01/S02/S06 (new CLI methods NA); C12 to both
transports, mutation and external validation. Neither card completes W10/W15.
Optional hardware actuation requires separate permission; no new encoder simulator.

## Integration sequence

1. Finish shared profile-token parsing, identity rendering and PTZ/adapter/replay
   dependency closure. Include K27 key collisions; synthetic-only success must
   not be presented as complete replay compatibility.
2. Implement W16's exact-operation policy: default refusal for unmodeled effects,
   explicit per-operation acknowledgment opt-in, no global legacy-success switch.
   Media1/Media2 identities must remain distinct from Events synchronization.
3. Recheck the PR head before any authorized integration. Port client/session
   contributions with attribution; independently implement the mock against the
   shared parser, structured fault and approved policy rather than copying the
   unconditional success handler. Do not silently merge the contributor PR.
4. Add each new operation to the source inventory, operation cards and coverage
   tables. Test literal/escaped/whitespace/duplicate/mislocated/absent tokens,
   unknown profiles, policy isolation, unchanged state/hooks, negative response
   payloads and both in-process/HTTP entry points. Verify tests fail under mutation.
5. Independently validate selected request/response instances with pinned external
   resources; run feature/MSRV/native-platform and ordinary quality gates. Update
   paired public docs, examples and changelog without claiming actual stream output.

No camera synchronization request was sent: this is an actuating operation, not
a read-only hardware probe. Real-stream acceptance, PR merge and release remain
outside the current read-only hardware/publish authorization.
