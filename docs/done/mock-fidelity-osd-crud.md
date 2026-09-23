# OSD CRUD batch OS1

[English](mock-fidelity-osd-crud.md) | [繁體中文](mock-fidelity-osd-crud_zh.md)

Status: DONE (2026-09-23). W01 cards recorded before handler edits, 2026-09-23,
baseline `34cccdb`. Bounded W10/W16/W19 work; no camera or video-rendering claim.

## Operation cards

All Actions use `http://www.onvif.org/ver10/media/wsdl/` plus the operation name.
`respond_with_effect` owns the parsed request; Media1 dispatch passes its selected
operation to the handlers below. Media2 advertises no OSD support and is unchanged.

| Ledger ID | Existing reader / target | State, faults and replay |
| --- | --- | --- |
| media.GetOSDs | media::resp_osds; fragment ConfigurationToken → direct optional qualified scalar | Validate existing unique source configuration; immutable filtered/all snapshot; unknown source NoConfig |
| media.GetOSD | media::resp_osd; existing RS1 selector and shared renderer | Preserve RS1 selection/tolerance and legacy unknown-token fault; exercise CRUD readback, not a new full read-fault audit |
| media.GetOSDOptions | media::resp_osd_options; ignored body → direct required ConfigurationToken | Validate source identity; same synthetic options for every modeled source; immutable read |
| media.CreateOSD | media::handle_create_osd; global OSD fragment → one qualified candidate | Validate modeled fields/source, check per-source total/type quota and allocate unique token under one write lock; Receiver/Action/MaxOSDs at capacity |
| media.SetOSD | media::handle_set_osd; global token/fragment → one qualified candidate | Existing unique OSD; immutable source binding must match, replacement/type quota excludes self; Sender/InvalidArgVal/NoConfig or ConfigModify |
| media.DeleteOSD | media::handle_delete_osd; fragment token → direct qualified scalar | Remove exactly one existing OSD under lock; unknown NoConfig; duplicate stored identity is invalid snapshot |

Reference review: [Media 24.12 §5.20](https://www.onvif.org/specs/srv/media/ONVIF-Media-Service-Spec.pdf)
defines source-associated capacity and operation faults. Pinned Media WSDL and
ONVIF XSD from `packaging/schema-sources.json` remain outside the checkout.
Their review also identified client position/color/persistence and text-order
wire discrepancies; the production client is corrected and real captured exchanges
are independently validated below.
No schema-derived fixture catalogue is added to the repository.

## Delivered contract

- Preserve the existing public state shape. Model Text/Image, named/custom
  positions, finite coordinates, text subtype/date/time/font/plain content and
  font color. Decode XML once; preserve literal identity and text.
- Reject unmodeled write children/attributes/extensions, background color and
  temporary text explicitly instead of reporting a discarded write as successful.
  Absent/true persistence uses the existing persistent-state hook; no standalone
  disk durability or physical overlay guarantee. Image URI is stored, never fetched.
- Require shape/multiplicity/scalar boundaries. Missing/duplicate/non-scalar fields
  use RequestError policy; unsupported fields use mock:UnmodeledEffect. Invalid
  modeled values use ConfigModify. Set cannot silently change its source binding.
- Total capacity applies to images too; text subtype limits apply per source.
  Check candidates excluding the replaced entry. Counter overflow/collision must
  never panic, duplicate an identity, consume an ID on rejection or partially write.
- Successful writes notify once and emit OsdCommitted; rejected writes notify zero
  times and emit no effect. Built-in replay invalidates Media1 GetOSD/GetOSDs only
  after commit; options/unrelated reads and standalone replay policy remain intact.

## Acceptance

C01–C04: ledger/readers, Action/operation ownership, alternate prefixes, Header and
extension decoys, duplicate/missing/wrong-namespace fields, attributes and literal
entity/CDATA text. C05–C06: full lifecycle, two distinct sources, independent
instances, complete-state/hook preservation, concurrent quota and unique tokens.
C07–C08: structured selected faults, typed client values and independently validated
wire captures including color, custom position, image and options. C09 reuses
existing auth/request resource gates. C10–C11 cover advertised options, persistence
hook and committed replay dependencies, not rendering/hardware. C12 requires one
unfiltered all-feature/no-fail-fast mutation campaign, exact restoration, five
workspace gates, inventory/link checks and external schema validation.

## Delivered results

- Five routes now use `services/osd.rs`: `list`, `options`, `create`, `set`
  and `delete`. The operation cards above preserve the pre-edit reader inventory.
  GetOSD retains the existing RS1 selector and shares the corrected renderer.
- Seven lifecycle/quota/counter/replay tests cover in-process and HTTP transports;
  client attribute precedence has a separate regression assertion. The token
  discrimination probe sorts quota-map entries before comparing Debug output,
  preserving every quota and avoiding randomized HashMap-order failures.
- The full unfiltered `cargo test --workspace --all-features --no-fail-fast`
  combined perturbation campaign produced seven expected failures in CRUD,
  capacity, replay and capture tests; exact candidate bytes were restored.
  Independently removing each color/persistence attribute read also fails its
  focused client assertion; restoring both returns the test to green.
- All five workspace gates pass: fmt, both all-target Clippy configurations with
  warnings denied, and all-feature/default tests (1359 / 1241 passed,
  including workspace doctests). Strict workspace rustdoc passes both modes.
- The selected external corpus contains 99 exchanges / 198 instances / 56
  operations, including 22 Fault responses. Strict pinned Xerces XSD 1.1
  validation passes all 198; all seven validator qualification controls pass.
  The initial capture rejected an empty ImageOption; adding a synthetic ImagePath
  reference fixed that actual wire defect before the successful fresh export.
  Schemas and captures remain outside the checkout. The reference is not an
  image download or rendering guarantee.
- Bilingual inventory self-tests and Markdown target/anchor checks pass.

OS1 is archived as a bounded completed batch. Background color, temporary text,
arbitrary extensions and a full read-fault audit remain outside its acceptance.
The [active execution checklist](../active/mock-fidelity-execution-checklist.md)
retains broader W10/W16/W19 work. Local validation does not claim hosted CI,
hardware conformance or ONVIF certification.
