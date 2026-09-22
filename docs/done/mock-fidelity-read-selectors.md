# Scoped read selector batch RS1

[English](mock-fidelity-read-selectors.md) | [繁體中文](mock-fidelity-read-selectors_zh.md)

Status: DONE for the bounded RS1 batch. Source baseline `4a9d0a5`, 2026-09-22. W04 shared
request migration, contributing bounded selector coverage to W10/W11/W14.

| Section | Purpose |
| --- | --- |
| [Cards](#cards) | W01 review before handler edits |
| [Acceptance](#acceptance) | C01–C12 applicability and remaining scope |

## Cards

All four reads route through `dispatch::respond_with_effect`, its already parsed
`Request::operation` and the service dispatcher. Actions use the operation's
service namespace plus `/` plus operation name; the HTTP service endpoint and
in-process transport must agree. The existing client/session methods retain
their signatures. Selection is an immutable snapshot, with no state write,
replay invalidation or on-change hook. Normal replay hits retain their recorded
responses; RS1 changes synthetic dispatch only.

| Ledger ID / namespace | Client / handler / current readers | Target and renderer boundary |
| --- | --- | --- |
| `media.GetOSD` / `http://www.onvif.org/ver10/media/wsdl` | `get_osd`; `media::resp_osd`; nested `extract_tag(GetOSD)` then `extract_tag(OSDToken)` | Read one direct qualified scalar `OSDToken`; preserve literal decoded identity and permitted extension scope; `render_osd_entry`, singular OSD wrapper; unknown selector retains legacy fault |
| `ptz.GetNode` / `http://www.onvif.org/ver20/ptz/wsdl` | `ptz_get_node`; `ptz::resp_ptz_node`; global `extract_tag(NodeToken)` | Direct qualified scalar `NodeToken`; existing `render_node`; missing/empty and unknown fault messages retained |
| `ptz.GetConfiguration` / same PTZ namespace | `ptz_get_configuration`; `ptz::resp_ptz_configuration`; global `extract_tag(PTZConfigurationToken)` | Direct qualified scalar `PTZConfigurationToken`; existing `render_config`; missing/empty and unknown faults retained |
| `recording.GetRecordingJobState` / `http://www.onvif.org/ver10/recording/wsdl` | `get_recording_job_state`; `recording::resp_recording_job_state`; global `extract_tag(JobToken)` | Direct qualified scalar `JobToken`; select job's recording token/mode; preserve missing/unknown legacy faults; escape selected strings in response |

Reference review: external resources were fetched and SHA-256 verified against
`packaging/schema-sources.json` (23 resources), outside the checkout. Reviewed
the named request declarations in [Media WSDL](https://www.onvif.org/ver10/media/wsdl/media.wsdl),
[PTZ WSDL](https://www.onvif.org/ver20/ptz/wsdl/ptz.wsdl) and
[Recording WSDL](https://www.onvif.org/ver10/recording.wsdl), pinned retrieval
2026-09-10. The selectors belong to their operation scope; a Header, foreign
namespace or extension descendant cannot supply them. Media's extension scope
must not be collapsed into the required selector. This is a selector migration,
not complete request-shape or fault conformance: unrelated children retain current
tolerance. Empty/absent selectors preserve the existing operation-specific error;
duplicate or non-scalar selectors use the existing `RequestError` InvalidArgs
policy. Unknown-token legacy Code layout remains open for a fault migration.

## Acceptance

RS1-WIRE finding: the first external candidate corpus rejects GetOSD with
`cvc-complex-type.2.4.a`. `render_osd_text` puts PlainText ahead of other modeled
text members; `render_osd_entry` also omits the image container. These are shared
GetOSD/GetOSDs renderers. Correct both, capture project-authored text/image cases,
then rerun external validation. Decoded identities also require OSD/PTZ string
escaping. Shared rendering fixes do not establish acceptance of write handlers.

- C01/C02: dispatch/Action unchanged, one owning parse; update the ledger arguments
  and reader inventory in both languages.
- C03/C04: alternate/default prefixes, Header/foreign/Extension decoys, duplicate
  and nested selectors, XML entities and CDATA decoded once. Full child order,
  unrelated members and maximum token lengths remain outside RS1.
- C05/C06: two different objects, unknown and wrong-family tokens, state/hook
  preservation and instance isolation on both transports; no mutation lifecycle
  or motion/recording simulation claim.
- C07/C08: verify selected literal values and structured malformed-selector
  refusal; independent external validation for selected positive responses and
  generic refusals. Legacy unknown-token faults remain explicitly unaudited.
- C09: shared auth/resource gates unchanged; reuse their standing tests; this
  batch adds no authentication bypass or new resource policy.
- C10/C11: static/modelled reads retain their existing capability limitations;
  exercise real typed clients and raw requests, no client API change.
- C12: one batch sensitivity campaign, restored five gates and external corpus
  evidence before completion. Neither RS1 nor route counts close W10/W11/W14.

## Results (2026-09-22)

Four routes now consume direct qualified decoded selectors. Two transport tests
cover decoys, duplicates, nested fields, missing/unknown identities, CDATA,
two distinct objects, instance isolation, full-state preservation and hook counts.
The five client captures also assert selected values, including escaped OSD
text and image paths. Shared OSD/PTZ renderers escape strings; recording state
escapes recording identity/mode. OSD text ordering and image nesting were corrected
after the first external export failed. Request-shape tolerance and legacy
operation-specific faults remain outside this completion claim.

The combined mutation campaign disabled duplicate rejection and selected renderer
escaping. The unfiltered workspace all-feature/no-fail-fast run failed, including
both new transport tests and the new capture test; original bytes were restored.
The first 168-instance export exposed the OSD defect; the corrected 170-instance
export passes pinned strict Xerces XSD 1.1 with payload anchors (85 exchanges,
50 operations, 64 successes and 21 existing refusals). All seven backend
qualification controls pass. Python's known K21 compilation warning remains a
failure; it was not suppressed. New malformed-selector responses are covered by
standing exact-fault tests, not added to the schema-valid-request corpus.

Both inventory self-test families pass: 159 routes, 161 Action sites, 190 indexed
reader calls (five fewer). The restored workspace formatting, both Clippy and
test configurations are the final commit gates. Broader W04/W10/W11/W14 and
W20–W23 remain PARTIAL; no new hosted CI or release is claimed.

Final acceptance: workspace all-features 1,346 passed; default 1,230 passed; seven ignored in each, 42 suites. Both all-target Clippy modes, formatting, strict rustdoc modes, inventory controls and local documentation links pass.
