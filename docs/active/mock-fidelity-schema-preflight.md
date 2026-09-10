# Mock schema verification preflight

[English](mock-fidelity-schema-preflight.md) | [繁體中文](mock-fidelity-schema-preflight_zh.md)

W20/W21 checkpoint, 2026-09-10. The full programme remains in progress.

| Section | Purpose |
| --- | --- |
| [Structural checker](#structural-checker) | Delivered W20 scope |
| [Verification evidence](#verification-evidence) | Positive and rejection controls |
| [External validator evaluation](#external-validator-evaluation) | Tool selection and K19 |
| [Reproducible tooling](#reproducible-tooling) | Pinned retrieval, offline resolution and generic controls |
| [Independent Xerces backend](#independent-xerces-backend) | All-service compilation and qualification |
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

The complete URL/hash manifest is now `packaging/schema-sources.json`; the table
above retains the original selection experiment's representative identifiers.
Tool references: [xmlschema API](https://xmlschema.readthedocs.io/en/stable/api.html),
[XSD 1.1 support](https://xmlschema.readthedocs.io/en/stable/features.html),
[lxml validation](https://lxml.de/validation.html).

## Reproducible tooling

`packaging/verify_schemas.py` separates `fetch`, `check`, `compile` and `validate`.
The 23-source manifest contains URL/hash metadata only. The retrieved documents,
venv, instance corpus and any derived data must remain outside the checkout and
packages. Resource directories inside or above the checkout are refused. Fetch
uses HTTPS, size limits, exact hashes and exclusive creation; existing mismatched
files are not overwritten. Sources are never rewritten to satisfy a validator.

Verification checks all hashes and declared XSD/WSDL dependency locations before
compilation. A directory layout mirroring source URL paths preserves relative
imports. The URI mapper and file-only opener jointly require an exact pinned
absolute destination; there is no network handler or schema fallback. XML DTDs,
including UTF-16 declarations, are rejected. Versioned `xmlschema`/`elementpath`
wheels are hash-pinned in `packaging/schema-requirements.txt`; these are verification
dependencies only. Instance location hints cannot load additional schemas.

Use a dedicated external virtual environment; substitute its interpreter for
`python` below. Replace the resource path with an absolute external directory
(`C:/Temp/oxvif-schema-check` on Windows is also valid).

```text
python -m pip install --require-hashes --only-binary=:all: -r packaging/schema-requirements.txt
python -m unittest discover -s packaging -p test_verify_schemas.py -v
python packaging/verify_schemas.py fetch --root /absolute/external/oxvif-schema-check
python packaging/verify_schemas.py check --root /absolute/external/oxvif-schema-check
python packaging/verify_schemas.py compile --root /absolute/external/oxvif-schema-check
```

Local evidence: fresh hash-verified wheel installation succeeded; retrieval and
offline closure checks passed for 23 files, 12 independent schema roots and 30
declared dependency edges. Fifteen generic tests cover path/hash/closure/DTD
rejection, no-network compilation, scoped QName and scalar values, cardinality,
ordering, required attributes, inherited WSDL namespace bindings, dependency
versions, explicit root anchors and sanitized failures.
Removing the digest and instance checks made the digest control and all seven
invalid-instance controls fail; both mutations were restored. All 19 packaging
tests then passed. These generic fixtures are project-authored, not ONVIF-derived.

**K21 — Python backend limitation:** strict compilation with warnings
treated as errors reports `XMLSchemaTypeTableWarning` in the Device source.
Separate compilation reproduced it for Device; the other nine ONVIF service
roots and the two supporting WSDL roots compiled without warnings. This is an
external validator/schema qualification issue, not a demonstrated mock defect.
Do not suppress the warning, edit the schema or count the failing full command
as a pass. The earlier SOAP/Media experiment does not establish all-service support.
The Python full-catalogue instance gate has consequently not run. The independent
Xerces backend below resolves the compiler-selection blocker without weakening
this Python diagnostic or modifying the sources.

The `validate` command additionally requires `--corpus` pointing to an external
directory with `cases.json`: format 1 and a nonempty `cases` list, each containing
a unique simple `file` name and an explicit expected `root` expanded name. It
requires both a matching root and a schema declaration, then validates strictly;
missing files, duplicates, path escapes and invalid instances fail. Corpus
generation and operation coverage reconciliation remain to be implemented.
It does not certify request semantics, effects, WSDL bindings or device behavior.

The initial tooling checkpoint added Windows/Linux generic controls. Their current
extension and the separate source-compilation job are described below; neither is
W22's full mock-instance gate.

## Independent Xerces backend

`packaging/verify_schemas_xerces.py` and `SchemaVerifier.java` provide an independent
XSD 1.1 backend using [Apache Xerces-J 2.12.2's XSD 1.1 distribution](https://xerces.apache.org/xerces2-j/).
The downloaded archive SHA-512 and the four required JAR SHA-256 values are pinned
in `packaging/xerces-validator.json`. Only those verified members are extracted;
official schemas and the validator distribution are not packaged in oxvif.
A JDK 17+ executable runs the adapter as source; no system installation or PATH
modification is required. The local comparison used an external portable Temurin
17.0.20.1+1 JDK, with its archive SHA-256 independently checked against Adoptium
metadata (`e53a79c3c3d86865bd7e787903884331068e71321714ffd44f145785affc7cb0`).

The adapter retains strict schema full-checking and treats warnings/errors as
failure. An exact catalogue resolver never returns null to request a default
lookup. WSDL extraction preserves inherited namespace bindings and descendant
rebindings, retaining the original owner's directory for relative imports;
temporary derived files are external and removed after the run. Source files are
not rewritten. Instance parsing disables DTDs, external entities and XInclude;
expected roots are checked before validation. Error reports contain only stable
exception/constraint identifiers, never schema excerpts or input values.

Local result: the same 23-file closure compiled as one schema set with 12 explicit
roots and no warnings. This establishes an available independent compilation path
for K21, **not** proof that Python's warning is false or that the mock is conformant.
No Python warning was suppressed. The five explicit qualification tests include
a valid imported-schema instance, eight invalid value/QName/shape/root variants,
an invalid schema, DTD/import rejection and a location-hint control. Rejections
assert sanitized constraint identifiers. Removing the Java instance-validator
call made all seven schema-invalid instance controls fail; it was then restored.
Three additional schema-free controls check extraction scope, metadata injection
and dependency tampering. All 22 packaging tests passed before this checkpoint's
final gates; required backend qualification never silently skips missing tools.

After the source `fetch`/`check` commands above, use the qualified compiler:

```text
python packaging/verify_schemas_xerces.py fetch-tool --tool-root /absolute/external/oxvif-xerces
python packaging/qualify_xerces.py --tool-root /absolute/external/oxvif-xerces --java /absolute/jdk/bin/java
python packaging/verify_schemas_xerces.py compile --root /absolute/external/oxvif-schema-check --tool-root /absolute/external/oxvif-xerces --java /absolute/jdk/bin/java
```

`validate` uses the same external `--corpus` format described above. Corpus
generation and operation coverage remain open; compiling official schema files
does not validate any mock exchange. The earlier `verify_schemas.py compile`
command remains a Python-backend diagnostic expected to expose K21.

Windows/Linux CI now runs generic controls and independent Xerces qualification,
followed by a separate **Official schema compilation** job with fixed source
hashes, external directories and no uploaded artifacts. Both gate packaging;
neither should be reported as full operation/corpus acceptance. Prior CI
[34461384194](https://github.com/smiti1642/oxvif/actions/runs/34461384194) passed all
25 jobs for `4fdd9f2`; it predates the Xerces adapter and new compilation jobs.

## Next work

W20 remains PARTIAL: audit unresolved/wildcard accounting, QName-valued Fault text,
and expand the corpus beyond fragment probes. W21 remains PARTIAL: complete the
positive/negative envelope, payload and Fault instances from the mock corpus and
qualify the selected path against actual emitted exchanges. Then extend W22's
source-compilation job into the full fail-closed instance
CI job. Do not substitute this tool
experiment for P-B's per-operation field/Fault/semantic review.

No maintainer decision is required for this diagnostic checkpoint. If the chosen
gate cannot retain the approved D3 distribution or strict-validation boundaries,
escalate before weakening them. PR #16 integration still depends on the reviewed
Fault and D2 policy paths; its existing acknowledgment-only mock is not accepted.
