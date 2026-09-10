# Mock schema verification preflight

[English](mock-fidelity-schema-preflight.md) | [繁體中文](mock-fidelity-schema-preflight_zh.md)

W20/W21 checkpoint, 2026-09-10. The full programme remains in progress.

| Section | Purpose |
| --- | --- |
| [Structural checker](#structural-checker) | Delivered W20 scope |
| [Verification evidence](#verification-evidence) | Positive and rejection controls |
| [External validator evaluation](#external-validator-evaluation) | Tool selection and K19 |
| [Next work](#next-work) | Remaining acceptance, not a completion claim |

## Structural checker

`tests/mock_schema_shape.rs` now retains namespace bindings at each node rather
than applying the final file-wide prefix map. This applies to emitted element and
attribute names and to schema `type`, `base`, and `ref` values. Bare attributes
remain unqualified; an unprefixed QName value uses its owner's default namespace,
not the schema's target namespace. Invalid/unbound names and duplicate expanded
attributes no longer silently become empty-namespace attributes or disappear.

The explicitly selected external test requires `OXVIF_ONVIF_SCHEMA` and anchored
SOAP 1.2 Envelope/Fault declarations. Missing resources fail; the test remains
ignored in ordinary schema-free runs. Every generated response must have a SOAP
Envelope, one Body and one payload. Envelope and payload are checked separately.
Fault payloads are no longer exempt from anchoring/structural checking. Counts
now distinguish successful payloads, actual Faults and recursively anchored nodes.

This remains a structural audit, not a complete XSD validator. It does not validate
Fault code lexical values or establish operation-specific error mappings. Wildcard,
unresolved-type, value, and corpus coverage still require W20/W21 work. The probe
corpus still uses legacy fragments; it is not evidence that client requests are valid.

## Verification evidence

Seven schema-free controls use project-authored synthetic names and schemas:
`checker_restores_namespace_scope_after_children_and_empty_elements`,
`checker_attributes_are_scoped_normalized_and_not_default_qualified`,
`checker_qname_values_use_owner_scope_not_the_final_file_map`,
`checker_schema_index_keeps_local_type_base_and_ref_bindings`,
`checker_rejects_unbound_or_duplicate_expanded_attributes`,
`checker_does_not_silently_replace_roots_or_accept_incomplete_documents`, and
`checker_payload_selection_rejects_wrong_envelopes_and_ambiguous_bodies`.
Each fixture was perturbed; all seven failed at their intended payload assertion,
then were restored. Missing-environment invocation also failed as intended.

External local structural run: Windows, Rust 1.97.0, 20 external files retrieved
2026-09-10; 158 responses, 108 success payloads, 50 Faults, 1,216 anchored nodes,
1,366 skipped children and 384 checked attributes. All ten existing finding pins
remain zero and were not changed. Replacing the ordinary Fault's Reason wrapper
with Detail produced 50 findings (one distinct missing-required finding); the
test failed at its pin assertion. Restoring the helper restored the original code.
Official resources and experimental scripts remain outside the checkout.
Local gate: formatting and both workspace Clippy modes passed; 1,162 all-feature
and 1,082 default tests passed (4 ignored each). Inventory reconciliation and
135 local document links passed. These are not full-validator results.

The previous checkpoint `e8fda59` passed all 23 jobs in
[CI run 34454688822](https://github.com/smiti1642/oxvif/actions/runs/34454688822),
including native Windows/Linux/macOS tests and smoke checks. That run predates
this checker change and is not its hosted acceptance or final M6 acceptance.

## External validator evaluation

K19 is an external-resource/tool compatibility finding, not a mock implementation
defect. The unchanged current Media source closure was tried in:

| Validator | Outcome |
| --- | --- |
| lxml 6.1.1 / libxml2 2.11.9, XSD 1.0 | SOAP schema compiled; Media1 closure rejected as nondeterministic |
| xmlschema 4.3.2 / elementpath 5.1.4, XSD 1.0 | SOAP compiled; both Media closures rejected for Unique Particle Attribution conflicts |
| xmlschema 4.3.2 / elementpath 5.1.4, XSD 1.1 | SOAP and both Media closures compiled with strict schema checking |

The XSD 1.1 result is a candidate tool configuration, **not** an XSD 1.0 pass or
instance-validation result. No schema was patched and no validator checks were
disabled. The Python packages were installed only in an external temporary venv;
the Rust runtime dependency graph and user installation are unchanged.

Representative retrieved SHA-256 identifiers:

| Official source | SHA-256 |
| --- | --- |
| [Media1 WSDL](https://www.onvif.org/ver10/media/wsdl/media.wsdl) | `bb4596f47ff0f907b8113fd2c86f649faa098df0e0df9f1c5f4de34f592de1eb` |
| [Media2 WSDL](https://www.onvif.org/ver20/media/wsdl/media.wsdl) | `0a1d59636910074ec4fb80b9a43f398ce208f4e38a6c7dff414eb6ddba3a1613` |
| [ONVIF schema](https://www.onvif.org/ver10/schema/onvif.xsd) | `1de6e9dd31a18a6773b611c4f7eaa0528f219098ddfc883b380802aa9a7ff647` |
| [Common schema](https://www.onvif.org/ver10/schema/common.xsd) | `d945394fe823febcd873ed8444ccfd73f3b5234d6dfbf0e151a33f5e611b5077` |
| [SOAP envelope](https://www.w3.org/2003/05/soap-envelope) | `3ae8caa9a74e83528cc0e1a59fc12784435e8e0086e890d0e1b04d881c079bec` |

These identifiers are not yet the complete reproducible closure manifest.
Tool references: [xmlschema API](https://xmlschema.readthedocs.io/en/stable/api.html),
[XSD 1.1 support](https://xmlschema.readthedocs.io/en/stable/features.html),
[lxml validation](https://lxml.de/validation.html).

## Next work

W20 remains PARTIAL: audit unresolved/wildcard accounting, QName-valued Fault text,
and expand the corpus beyond fragment probes. W21 remains PARTIAL: complete the
source/import catalogue, pin verification dependencies, enforce offline resolution
and qualify the candidate validator with positive/negative envelope, payload and
Fault instances. Then add W22's fail-closed CI job. Do not substitute this tool
experiment for P-B's per-operation field/Fault/semantic review.

No maintainer decision is required for this diagnostic checkpoint. If the chosen
gate cannot retain the approved D3 distribution or strict-validation boundaries,
escalate before weakening them. PR #16 integration still depends on the reviewed
Fault and D2 policy paths; its existing acknowledgment-only mock is not accepted.
