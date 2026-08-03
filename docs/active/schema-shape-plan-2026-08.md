# Checking the mock's XML against the ONVIF schema — plan

**Status** — investigated, not started. Written 2026-08-03 against a working
prototype, not from memory: every number below was measured, and the section
[What the prototype already found](#5-what-the-prototype-already-found) lists
candidate defects that still need confirming once the index is complete.

**Why** — `docs/active/mock-audit-2026-07.md` closed four tiers of *state*
defects. The class it never touched is *shape*: the mock writes XML as
hand-built strings, so it can emit a document no schema allows and every one of
the five gate lines stays green. Five instances have been found so far, each by
a human reading `onvif.xsd` one type at a time:

| found | what the mock emitted | what the schema says |
|---|---|---|
| 0.15.0 | `tt:AFModes` | `tt:AutoFocusModes` |
| 0.15.0 | `BitrateRange` at the top level of the options | only under `Extension` |
| 0.15.0 | Media1 options flat | wrapper + repeated entry |
| 0.15.0 | `DefaultAbsolutePanTiltPositionSpace` | `…Pant…`, double `t` |
| this investigation | `tt:ScopeAttribute` | `tt:ScopeDef` |

Five for five by hand is not a strategy. Nothing says the class is exhausted.

---

## 1. What was measured

### 1.1 `onvif.xsd` alone is not the schema

```
onvif.xsd   <xs:include schemaLocation="common.xsd"/>
            <xs:import> ×4, all with remote schemaLocation:
                w3.org/2005/05/xmlmime
                w3.org/2003/05/soap-envelope
                docs.oasis-open.org/wsn/b-2.xsd
                w3.org/2004/08/xop/include

grep -c 'complexType name="IntRange"' onvif.xsd   →  0
```

`IntRange`, `FloatRange`, `DurationRange`, `PTZVector`, `Vector2D` — the basic
types half the payload is made of — are all in **`common.xsd`**, which
`onvif.xsd` includes and which is not in the scratch copy this investigation
used.

And the *response wrapper* elements are in neither. `GetProfilesResponse` is
declared inline in `media1.wsdl`'s `<wsdl:types>` (verified), which imports
`onvif.xsd`. `onvif.xsd` declares only the `tt:` types.

**So the schema set for one response is: the service WSDL's inline schema +
`onvif.xsd` + `common.xsd`.** The four remote imports matter only for full
XSD validation (§6), not for the structural check proposed here.

### 1.2 `onvif.xsd` is structurally simple enough to index

| feature | count |
|---|---|
| named `complexType` (root level) | 459 |
| anonymous `complexType` | 11 |
| global `xs:element` | 31 |
| `xs:element` total | 1184 |
| `xs:extension` / `xs:complexContent` | 32 / 30 |
| `xs:choice` | **2** |
| `xs:any` | **262** |
| `xs:group ref` | **0** |
| named `simpleType` | 99 |

No group indirection, almost no choice. Base resolution (`xs:extension`) is 32
cases. **262 wildcards** is the number that shapes the design: a type with an
`xs:any` legally accepts unknown children, so "unknown child" can only be
reported for types that have none.

### 1.3 The corpus already exists

A throwaway test drove `MockTransport::soap_post` once per action URI extracted
from `src/client/*.rs` — the same extraction
`mock_handles_every_action_the_client_can_send` already does — with an empty
body plus a small override table for the token-taking operations:

```
155 responses dumped
 68 contain tt: elements
 48 are SOAP faults (missing/unknown token, or an operation needing a body)
```

Nothing new has to be built to produce the documents. The dumper is ~120 lines
and is kept in the session scratchpad; it should become part of the real test.

**The 48 faults are the gap to close first.** They are mostly per-channel
getters answered without a usable token, i.e. exactly the operations where two
of the five known defects lived (imaging options, video encoder options). The
override table needs finishing before the checker means much.

### 1.4 A prototype checker runs, and the coverage number is the headline

Index built from `onvif.xsd` alone: **470 types, 600 distinct element names.**

Two check strategies were prototyped against the 155 documents.

**By element name** (for each `tt:` element, is this name declared anywhere?):
32 findings, of which roughly 4 are genuine and the rest are names declared in
`common.xsd` or a WSDL the index does not have.

**Anchored** (resolve a type, then check children against its declared
sequence):

```
anchored subtrees:  237
skipped, type unresolvable: 1016      →  19% coverage
findings: 1 ORDER, 29 MISSING-REQUIRED
```

**19% is the honest number for what `onvif.xsd` alone can check**, and it is the
strongest argument in this document.

### 1.5 Two design errors the prototype exposed

**Anchoring by element name picks the wrong type.** `Brightness` is
`xs:float` inside `ImagingSettings20` and `FloatRange` inside `ImagingOptions20`.
Filtering candidates to "names I have a complexType for" silently discards the
builtin and anchors on `FloatRange`, so the checker demanded `Min`/`Max` inside
a plain float. Most of the 29 `MISSING-REQUIRED` findings are this bug —
`SourceToken (SourceReference)`, `Address (IPAddress)`, `State (AnalyticsState)`,
`StayTime (DurationRange)`, `IPv4Address (PrefixedIPv4Address)` are all the same
mistake.

**The fix is top-down anchoring**: start at the response element declared in the
WSDL, walk down carrying the declared type at each step. That is not an
optimisation — it is the only way to know a type. **It requires the WSDLs.**

**Anchoring loses the check that found the most.** Once anchoring is restricted
to resolvable subtrees, `tt:ScopeAttribute` stops being reported — its parent
`tds:Scopes` is not a `tt:` element, so nothing anchors it. The unknown-name
check is what caught `AFModes`, the `Pant` spelling and `ScopeAttribute`; it
must stay, and to be quiet it needs a *complete* index rather than a resolvable
one.

**Both checks are needed, and both need the same missing files.**

---

## 2. What the checks would and would not catch

Scored against the five known defects plus the ones this investigation found:

| defect | caught by | how |
|---|---|---|
| `tt:AFModes` | name check | no such element in the schema |
| `DefaultAbsolutePanTiltPositionSpace` | name check | same |
| `tt:ScopeAttribute` | name check | same |
| Media1 options missing a nesting level | anchored | `Encoding` under a type that does not declare it |
| `BitrateRange` at the top level | anchored | same |
| Media1 `Set` missing `Multicast`/`SessionTimeout` | anchored | required member absent |
| Media2 `Multicast` after `Bitrate` | anchored | order |
| **`<tt:SupportedPTZSpaces/>` empty** | **neither** | **it is schema-valid** |

Seven of eight. The eighth is the honest limit: an empty element whose children
are all `minOccurs="0"` is valid XML for a device that supports nothing.
Schema checking answers *is this document well-shaped*, never *does it mean
anything*.

Also out of scope, permanently:

- **Values.** `xs:anyURI`, ranges, enumerations of `simpleType`s — a structural
  index carries names and cardinality, not lexical spaces. §6 covers this.
- **Vendor divergence.** A conformant schema permits many shapes; each vendor
  picks a different one. That question belongs to `metamorph`
  (`FixtureStore::diff_against_synthetic`), which is **currently called by no
  test and no example**, and `tests/fixtures/` holds only a `README.md`. Worth
  its own plan; not this one.

---

## 3. Design

### 3.1 Index

Parse the schema set into:

```
types:      type name        -> { base, [ (child name, type, minOccurs, maxOccurs) ], has_wildcard }
elements:   element name     -> set of declaring types            (for the name check)
roots:      response element -> type                              (from each WSDL)
```

Resolution follows `xs:extension base` (32 cases) and treats a type as
wildcarded if it or any base contains `xs:any`. `xs:choice` (2 cases) is treated
as "any of these children, order unchecked" — cheaper than modelling it and
wrong in no measured case.

### 3.2 Checks

1. **Unknown element name** — every `tt:`-namespaced element the mock emits must
   be declared somewhere in the set. No anchoring needed, no false positives
   once the index is complete.
2. **Child not declared by parent** — top-down from a WSDL root. Skipped when
   the parent type has a wildcard.
3. **Order** — children must appear in declared sequence order.
4. **Missing required** — `minOccurs >= 1` children must be present.

Check 4 is the noisiest and the most valuable; expect to argue with it. The
`GetCapabilities` findings in §5 are all check 4.

### 3.3 Where it lives

`tests/mock_schema_shape.rs`, `#![cfg(feature = "mock")]`, black-box: it uses
`oxvif::mock::MockTransport` through the public `Transport` trait and
`include_str!`s the client sources exactly as `dispatch.rs` already does.
`quick-xml` is already a dev-dependency, so the XSD reader has no new dep.

A guard on the guard, in the shape `dispatch.rs` uses: assert the corpus
actually contains payload documents (`>= N` responses carrying `tt:` elements)
and that the index resolved a floor number of types. An index that silently
loads nothing makes every check vacuously true.

---

## 4. Open decisions — these block a start

**D1. How the missing schema files are obtained.** Needed, none present:

```
ver10/schema/common.xsd
ver10/device/wsdl/devicemgmt.wsdl
ver20/imaging/wsdl/imaging.wsdl
ver10/events/wsdl/event.wsdl
ver10/recording.wsdl   ver10/search.wsdl   ver10/replay.wsdl
```

Present in the session scratchpad: `onvif.xsd`, `media1.wsdl`, `media2.wsdl`,
`ptz.wsdl`. Downloading is an explicit-permission action and has not been done.

**D2. Whether anything derived from the schema is committed.** The schema is
© ONVIF 2008-2025. Two options:

- *(a) Nothing committed.* The test reads the schema set from a directory named
  by `OXVIF_ONVIF_SCHEMA`, and **skips loudly** when unset. Zero licensing
  question; the check runs only where someone has the files.
- *(b) A generated index committed* — element names, parent/child relations,
  cardinality. Structural facts rather than the schema text, and it puts the
  check in the daily gate. Whether that redistribution is acceptable is not a
  call this document makes.

(a) is the conservative default and what the rest of this plan assumes.

**D3. Gate or publish-time.** Under (a) the test is `#[ignore]`d and gets a line
in the publishing checklist, next to the per-feature warning sweep. Under (b) it
joins the five gate lines. A skipping test that reports success is a hollow test
by this project's own rule — under (a) it must print why it skipped and the
checklist line is what makes it run.

---

## 5. What the prototype already found

**Every row below is a candidate, not a confirmed defect.** Each was produced by
an index built from `onvif.xsd` alone; each needs re-checking once the set is
complete. They are recorded because a plan that reports no findings has not been
tested.

**5.1 `tt:ScopeAttribute` is not an ONVIF element.** `tt:Scope` declares
`ScopeDef` (`minOccurs=1`) and `ScopeItem` (`minOccurs=1`). The mock emits
`<tt:ScopeAttribute>Fixed</tt:ScopeAttribute>` at
`src/mock/services/device.rs:124`, and the unit fixture agrees with it at
`src/tests/client/device_tests.rs:403` and `:407`.

**The correct name is already in the repo**, in a comment 150 lines below the
bug: `src/mock/services/device.rs:277` — *"The GetScopesResponse format is
richer (`<Scopes><ScopeDef/><ScopeItem/></Scopes>`)"*. `grep -rn ScopeDef src/`
finds only that comment, so oxvif's own parser reads neither name — it takes
`ScopeItem` only. **Client unaffected; mock and fixture wrong.** Fifth instance
of the class, and the first where the answer was already written down.

**5.2 `OSDTextOptions` children are out of order.** Schema sequence is
`Type, FontSizeRange, DateFormat, TimeFormat, FontColor, BackgroundColor,
Extension`. The mock emits `Type ×4, DateFormat ×3, TimeFormat ×2,
FontSizeRange` — `FontSizeRange` last where the schema puts it second.

**5.3 `tt:SecurityCapabilities` — four required members omitted, and one member
emitted that the type does not have.** All eight are `minOccurs=1`: `TLS1.1`,
`TLS1.2`, `OnboardKeyGeneration`, `AccessPolicyConfig`, `X.509Token`,
`SAMLToken`, `KerberosToken`, `RELToken`. The mock omits `TLS1.1`, `SAMLToken`,
`KerberosToken`, `RELToken`, and emits `<tt:UsernameToken>`, for which
`grep -c 'name="UsernameToken"' onvif.xsd` is **0**.

**Needs `devicemgmt.wsdl` to settle.** `UsernameToken` is plausibly a member of
the *service*-capabilities type (`tds:SecurityCapabilities`, a different type
with attributes) rather than the device-level `tt:` one. If so the mock is
mixing the two. Note `src/health/checks.rs` cross-references `UsernameToken` as
one of its eighteen twice-stated attributes, so whichever way this lands, the
health check has an opinion about it.

**5.4 `tt:RecordingSourceInformation` — four of five required members omitted.**
Requires `SourceId`, `Name`, `Location`, `Description`, `Address`; the mock
emits `Name` only. `RecordingEntry` in `src/mock/state.rs` already **stores**
`source_id`, `location` and `description` — so this is a renderer dropping state
it holds, the same shape as the `MTU` bug, in a place the round-trip table does
not reach because there is no `Set`.

**5.5 Three more capability types with required members omitted** —
`SystemCapabilities` (`SupportedVersions`), `RecordingCapabilities`
(`ReceiverSource`, `MediaProfileSource`, `DynamicRecordings`, `DynamicTracks`,
`MaxStringLength`), `SearchCapabilities` (`MetadataSearch`).

---

## 6. Full XSD validation — deliberately not this

Real validation (`libxml2` via the `libxml` crate, or `xmllint --schema`) would
subsume every check here and add value ranges and types. It needs:

- the six-plus schema files with `schemaLocation` rewritten to local paths,
  including the four remote imports in §1.1;
- a C dependency or an external binary in the test environment;
- SOAP handling — `soap-envelope.xsd` types `Body` as `##any`, so the practical
  form is to validate the Body's first child element on its own.

Several times the cost of §3, and §2 scores the structural check at seven of the
eight known defects. **Do §3 first, and revisit only when it stops reporting
anything.**

---

## 7. Order of work

1. Settle **D1** and **D2**. Nothing below starts without the schema set.
2. Finish the corpus: extend the override table until the fault count is only
   the operations that genuinely refuse an empty request, and assert that floor.
3. Index reader + name check (check 1). Confirm or dismiss §5.1 first — it is
   the cheapest and it has a known answer.
4. Top-down anchoring + checks 2–4. Report coverage as a number, and assert a
   floor on it, or the check can rot into silence the way the 19% above would.
5. Triage §5.2–5.5 with the complete index; fix what survives.
6. Perturbation, per `CLAUDE.md`: reintroduce `tt:AFModes`, the `PanTilt`
   spelling and the flat Media1 options nesting one at a time. **Each must turn
   this test red.** A schema checker that does not catch the three defects that
   motivated it has not been tested.
