# PR 16 integration review

[English](mock-fidelity-pr16-integration.md) | [繁體中文](mock-fidelity-pr16-integration_zh.md)

2026-09-11 source review; W26 remains conditional, not integrated.

The [contributor integration plan](contributor-pr-integration-plan.md) now owns
execution order, current prerequisite checks, acceptance and branch boundaries for
#14/#17/#16. It supersedes the historical prerequisite sequence below; the review
findings remain open until implemented and verified.

| Section | Purpose |
| --- | --- |
| [Reviewed revision](#reviewed-revision) | Immutable head and incremental changes |
| [Disposition](#disposition) | Reusable code and required corrections |
| [Integration sequence](#integration-sequence) | Dependencies and acceptance |

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
- Operation/reference documentation is not completely bilingual in the diff:
  `OPERATIONS_zh.md` and the paired translated reference pages are absent. Public
  descriptions also need an explicit mock acknowledgment/effect boundary and
  should describe profile-associated streams, not only video I-frames.

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
