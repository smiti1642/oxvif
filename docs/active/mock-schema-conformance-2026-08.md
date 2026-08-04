# The mock's XML does not match the schema — triage and plan

**Status** — investigated 2026-08-03, **sweep complete 2026-08-04. Every §5
work unit is done and all five kinds are at 0.** §3 fixed in `8091892`; the
checker's own defects fixed and the corpus re-dumped at that commit, so **the
numbers below are run 3's and run 2's 108 is superseded**. Written against a
working checker and the complete ONVIF schema set, not from memory.

The status line above used to end *"Nothing in the sweep is fixed yet."* That
was true when written and stopped being true at §5.0; it is replaced rather
than deleted, because the sentence is what a reader would have trusted.

**Zero on every kind does not mean the class is closed** — see §6. A type
carrying an `xs:any` suppresses `UNKNOWN-CHILD` for the whole type, and an
element whose children are all optional is schema-valid empty. Five of the
twelve client-facing bugs in §0 were found *in spite of* the counts — and the
eleventh was found with every kind already at 0, against a mock the checker
calls fully conformant. **This said "Four of the ten" until §5.10, and "Five of
the eleven" until §5.12.**

**It also listed a third category — "reads `xs:element` and never
`xs:attribute`" — until §5.11, which closed it.** The checker now reports
`MISSING-ATTR`, `UNKNOWN-ATTR`, `ATTR-AS-ELEMENT` and `ELEMENT-AS-ATTR`; all
four came out at 0 and all four were perturbed to prove they can move. What that
bought is one specific thing: `ATTR-AS-ELEMENT` is the only kind an `xs:any`
cannot suppress, so `GovLength` rendered as an element — §0.5, which moved
*nothing* when it was found — is now two rows. What it did not buy is the other
direction: this file still reads the mock's output and never the client's
parsing of it, which is how §0.11 was missed with every kind at 0.

The checker, the schema and the raw findings live in the sibling repository
`onvif-schema-lab` (local, never pushed) because of decision D2 in
[`schema-shape-plan-2026-08.md`](schema-shape-plan-2026-08.md) §4: nothing
derived from the schema enters this repository. **This file names elements in
*oxvif's own output* and says what is wrong with them; the schema evidence for
each stays in the lab.**

---

## 0. Blast radius, established before anything else

**Eleven client-facing bugs. The rest are mock fidelity.**

This line said *"Three client-facing bugs so far"* until §1.3's fourth landed,
*"Four client-facing bugs so far"* until §5.5's fifth did, *"Five"* until
§5.7's sixth did, *"Six"* until §5.8's seventh and eighth did — one work
unit, two types, so it moved by two — and **"Eight client-facing bugs so far"**
until §5.9's ninth and tenth did. It then read:

> **Ten client-facing bugs. The rest are mock fidelity.** […] The *"so far"* is
> dropped: §5 is complete, so this is the count for the sweep rather than a
> running total.

**Dropping the *"so far"* was the mistake, and it is the same mistake this
section keeps recording.** §5 being complete bounds what the *checker* still
reports; it does not bound the client, because the checker never reads the
client. The eleventh (§5.10) was named in §1.3 the whole time — the
`UsernameToken` paragraph established that `tt:SecurityCapabilities` declares
eight elements and that this was not one of them, and `SecurityCapabilities` in
`src/types/capabilities.rs` was left reading it. §5.6 fixed the mock and its own
commit message said in as many words that the client field was *"deliberately
not fixed, reported instead"*. So the count stood at ten with the eleventh
written down twice. That is (7)/(8)'s failure again — *a right classification
nobody scheduled* — and the rule §0 already states applies to a **commit
message** as much as to a subsection: when something concludes "client-facing",
it belongs in this count the same day, open or not.

This section originally read *"no finding below is a client-facing bug"*, and
that was wrong five times over — kept here rather than deleted, because every
exception was found by looking at something the sentence dismissed:

1. **Media2's audio encoder element name** — §3, fixed in `8091892`.
   `MediaProfile2::audio_encoder_token` had been `None` from every conformant
   device. It was flagged in §3 as "the only finding that could reach the
   client"; it did.
2. **`GetDigitalInputs` was sent to the wrong service** — §1.5, fixed in §5.0.
   It was hiding inside "54 unanchored roots, cause not yet established", which
   §2 explicitly declined to call a defect. Establishing the cause was what
   turned one of them into a defect.
3. **`set_storage_configuration` sent five elements in the wrong namespace in
   its *request body*** — §5.1a, fixed with §5.1. This one the checker cannot
   see at all: it reads the mock's responses and never a client request. It was
   found by asking why §4's predicted red never happened.
4. **`ImagingMoveOptions::from_xml` read three invented element names** — §1.3.
   All five of `imaging_get_move_options`'s ranges were `None` against a
   conformant camera. Recorded in §1.3 as *"Not fixed here — out of this work
   unit's scope"*, which is the fourth dismissal, and the fourth to be a defect.
   The checker cannot see this one either, for the *same* reason as (3) and a
   different one from what §0 argues below: it judges the mock's output, not
   oxvif's parsing of it. Fixing it moved no pin, which was checked rather than
   assumed.

5. **`VideoEncoderConfiguration2` read and wrote `GovLength` and `Profile` as
   child elements** — §1.3, §5.5. `tt:VideoEncoder2Configuration` declares both
   as `xs:attribute`, so both fields were `None` from every conformant device
   and both values were silently dropped by
   `set_video_encoder_configuration_media2`. §1.3 listed *"`Profile` inside a
   video encoder configuration"* among eight undeclared names with no comment,
   and §1 called the row *"not a new defect"* — true of the row, since it was
   the same defect counted at a second path, and read as *not a defect*.

6. **`MetadataConfigurationOptions::analytics_supported` read an element ONVIF
   declares nowhere** — §1.3, §5.7. `Options/Extension/AnalyticsSupported` is not
   in any of the fifteen files, in any form, so the field was `false` from every
   conformant device. §1.3 had it in the same eight-name list as (5), with the
   comment *"That one is a rename"* — a **positive** classification this time,
   not a dismissal, and wrong: there is no element at any level of
   `tt:MetadataConfigurationOptionsExtension` to rename it to. Both the mock and
   the unit fixture had been written to agree with the parser, which is (1)'s
   shape exactly.

7. **`VideoEncoderOptions2` read three `xs:attribute`s as elements, two of them
   `xs:list`-typed** — §1.3, §5.8. `tt:VideoEncoder2ConfigurationOptions`
   declares exactly `Encoding`, `QualityRange`, `ResolutionsAvailable` and
   `BitrateRange` as child elements; `GovLengthRange`, `FrameRatesSupported`
   and `ProfilesSupported` are attributes, so `gov_length_range` was `None` and
   `profiles` / `frame_rates` empty from every conformant device. It also
   carried a `frame_rate_range` field for an element the type does not declare
   at any level, and read `frame_rates` as `u32` where the schema says
   `xs:float`, so a device offering `12.5` fps lost that entry twice over.

8. **`VideoSourceConfigurationOptions::max_limit` read an element** — §1.3,
   §5.8. `MaximumNumberOfProfiles` is an `xs:attribute` of
   `tt:VideoSourceConfigurationOptions`; the field was `None` from every
   conformant device, on **both** Media1 and Media2, which return the same type.

9. **`SystemUris::from_xml` read a *type* name as an element name** — §1.3,
   §5.9. `tt:SystemLogUriList` declares one child element, `SystemLog`, *typed*
   `tt:SystemLogUri`. The parser walked `SystemLogUris/SystemLogUri/Uri`, so
   `system_log_uri` was `None` from every conformant device, and both the mock
   and `get_system_uris_xml` in `src/tests/client/device_tests.rs` had been
   written to agree with it — (1)'s shape a third time. §1.3 listed
   *"`SystemLogUri` and its `LogType`"* among the undeclared names and said
   nothing about the client.

10. **`VideoEncoderInstances::from_xml` iterated the wrong level** — §1.3, §5.9.
    `tr2:EncoderInstanceInfo` declares `Codec` repeated, and `Encoding` is a
    child *of* `Codec`; the parser iterated `children_named("Encoding")`, so
    `encodings` was empty while `total` still parsed — a half-populated struct,
    which is harder to notice than an error. The two levels sharing a name is
    what hid it, and `XmlNode` strips namespaces so the `tt:`/`tr2:` difference
    could not disambiguate them either.

11. **`SecurityCapabilities` read an undeclared element and modelled four of
    twelve members** — §1.3, §5.10. `username_token` read
    `Device/Security/UsernameToken`, which `tt:SecurityCapabilities` declares at
    no level — it is an `xs:attribute` of the service-level
    `tds:SecurityCapabilities` — so the public field was `false` from every
    conformant camera. On top of that the type modelled only four of the eight
    required elements, and **none** of the four its two `Extension` levels add
    (`TLS1.0`; then `Dot1X`, `SupportedEAPMethod` `[0..*]`,
    `RemoteUserHandling`), so four more facts a device states were unreadable.
    The field is removed and the eight missing members added.

12. **`OsdOptions::position_types` was empty from every conformant camera.**
    `tt:OSDConfigurationOptions` declares `PositionOption` as
    `type="xs:string" maxOccurs="unbounded"` — repeated plain strings.
    `from_xml` read a single `<PositionOption>` wrapper for nested `<Type>`
    children instead, the mock emitted that invented wrapper to match, and
    `apply_vendor_extensions` documented the *conformant* shape as a Genetec
    deviation. Only `OnvifSession` recovered the positions, by the path
    labelled a workaround. Found by §5.12's new `SIMPLE-TYPE-KIDS` kind.

**(11) is the only one of the twelve the checker was silent on in *both*
directions**, and that is worth stating plainly rather than as a caveat. (3) and
(4) were things it could not see; (5), (7), (8), (9) and (10) each cost it at
least one visible row, and (12) is what a *new* kind bought. Here §5.6 had
already made the mock conformant, so the run reported 0 on every kind while the
client read four of twelve members and one name that does not exist anywhere.
**A clean checker run is compatible with a client that reads almost nothing** —
the one-sidedness argued below, at its limit.

**(12) is the counter-example to that pessimism, and the reason the checker was
worth extending rather than retiring.** It was invisible for the same structural
reason as the rest — the walk stopped at `PositionOption` because `xs:string` is
not a complex type, so the invented subtree was skipped rather than judged — and
one rule using information the checker already had turned it into a row. Cost:
about twenty lines. That is a better return than any of the fixes below.

**(9) and (10) share a shape the first eight do not: the wrong name was a real
ONVIF name at a *different level or in a different role*.** (5), (7) and (8) were
elements that should have been attributes; (9) is a type name read as an element
name and (10) is a child name read as its own parent. Neither is a misspelling,
so no amount of care about spelling would have caught either — only reading the
declaration.

**(7) and (8) were both named in §1.3, correctly and in detail, and stayed
open for a work unit anyway.** That is the opposite failure from (6): not a
wrong classification but a right one nobody scheduled. §1.3 called (7) *"a
separate client-facing bug"* in those words and §0's count did not move,
because §0 counts bugs *fixed or being fixed* and §1.3 was writing about a bug
*found*. The two sections had no link between them. That is worth a rule of
its own: **when a subsection concludes "client-facing", it belongs in §0's
count the same day, open or not** — otherwise the blast-radius section
undercounts exactly the thing it exists to bound.

**The pattern across the first five is worth naming: each came from the sentence
that dismissed a category.** "No finding is client-facing", "54 unanchored
roots, cause not established", "the byte assertions will catch it", "out of
this work unit's scope", "not a new defect". A dismissal in this document has so
far been the best available index of where the next defect is.

**(6) is the first that came from a confident *assertion* instead.** *"That one
is a rename"* named the fix, so nobody re-derived it; it survived from triage
until the work unit opened. So the index is wider than "dismissals": it is any
sentence here that settles a question **without the schema open**, in either
direction. The already-stated rule two paragraphs below it — *"Read the
declaration before assuming an undeclared name wants renaming"* — was the
correct instruction and the same paragraph broke it.

**(5) adds a blind spot the other four do not have.** (3) and (4) are things the
checker cannot see; (5) is a thing it saw and *undercounted by construction*.
`GovLength` moved no counter in either direction — the name is a real `tt:`
element on `H264Configuration`, so `UNKNOWN-NAME` could not fire, and
`tt:VideoEncoder2Configuration` carries an `xs:any`, which suppresses
`UNKNOWN-CHILD` for the entire type. **A wildcarded type is invisible to the
child check, and a name that is real somewhere else is invisible to the name
check.** Half of this defect was reported only because the *other* half happened
to be a name declared nowhere.

**And (4) sharpens what "the checker cannot see it" means.** Two distinct blind
spots have now produced a bug each: it never reads a client *request* (3), and
it never reads the client's *parser* (4). Both are one-sided — the checker
compares the mock against the schema, and every other pair in the triangle
(client↔schema, client↔mock) is unwatched by it. §6's guard argument covers
client↔mock; nothing here covers client↔schema except a human with the WSDL
open.

So the safe reading of §0 is not "the client is fine" but **"the client is
unaffected by *namespace* and *order*, which is most of the set."** That much
is measured, not assumed:

- `XmlNode` is documented as *"a namespace-stripped XML node"* and drops the
  prefix (`src/soap/xml.rs:25-30`).
- Every lookup matches on the local name alone — `child()` at `xml.rs:46`,
  `children_named()` at `:76`, the `Body` search at `:267`.

So oxvif's parser is **namespace-blind and order-independent**. A response whose
elements are in the wrong namespace, or in the wrong sequence, parses
identically. Nothing a caller of this crate does is affected by any of it.

What *is* affected, and why this is still worth doing:

1. **The mock cannot prove the thing it exists to prove.** Its job is to stand
   in for a conformant device. A mock that emits documents no conformant device
   emits can only demonstrate that the client agrees with the mock — the exact
   failure `CLAUDE.md` already records four times as *"parser, fixture and mock
   agreeing with each other and with nothing else"*.
2. **`MockServer` is documented for other people's clients.** `src/mock/mod.rs`
   offers it "for cross-process / non-Rust clients". A conformant client
   resolving by qualified name finds nothing in the wrong namespace. That is a
   user-facing defect of the `mock-server` feature, not an internal one.

And the uncomfortable corollary, which belongs in the record: because the parser
is namespace-blind, **oxvif would also accept a wrong-namespace element from a
real camera**. That is robustness rather than a bug — but it means this class
can never be detected in the field either. The mock is the only place it can be
caught, which is why the mock being wrong matters more here than it would
elsewhere.

**Nothing here is a reason to delay 0.15.0** — except §1.5, which rode along
rather than waiting for the sweep.

This line used to call §1.5 *"a one-line client fix"*, contradicting §1.5's own
*"not purely a string change; scope it before writing it."* **§1.5 was right.**
Measured when it was done: the action string and the body prefix are two lines,
and the rest is a service that did not exist anywhere in the crate — a
`tmd` namespace binding, a dispatch route, a mock `GetServices` entry and a
`Capabilities/Extension/DeviceIO` block (whose five required counts are a
finding of their own), a session URL resolver, a `GetServices` fallback that
tolerates both spellings of `deviceIO`, and a breaking signature change. Eleven
files. The habit worth keeping is the one §1.5 used: **say what the fix needs
where the finding is recorded, and let the summary defer to it.**

---

## 1. What the checker reports

155 mock responses dumped at `8091892`, complete schema set (15 files, 1314
types, 1849 declared elements, 14 namespaces), 608 response roots anchored.

```
WRONG-NS          16     element declared by the parent, in another namespace
MISSING-REQUIRED  23     a required member absent
UNKNOWN-NAME      12     element not declared in the namespace it was emitted in
UNKNOWN-CHILD      6     element not declared by its parent's type
ORDER              6     children out of the declared sequence order
                 ───
                  63     distinct  (99 raw)
```

**After §5.0 this is 62 distinct, `UNKNOWN-NAME` 11; after §5.1, 46 distinct
with `WRONG-NS` 0; after the imaging slice of §5.2 + §5.3, 38 distinct —
`MISSING-REQUIRED` 21, `UNKNOWN-NAME` 8, `UNKNOWN-CHILD` 4, `ORDER` 5; after
the `GetProfiles` slice of §5.2 + §5.4, 32 distinct — `MISSING-REQUIRED` 16,
`UNKNOWN-NAME` **9**, `UNKNOWN-CHILD` 4, `ORDER` 3; after §5.5, 30 distinct —
`UNKNOWN-NAME` **7**, no other kind moved; after §5.6, 24 distinct —
`MISSING-REQUIRED` **11**, `UNKNOWN-NAME` **6**, `UNKNOWN-CHILD` 4, `ORDER` 3;
after §5.7, 16 distinct — `MISSING-REQUIRED` **7**, `UNKNOWN-NAME` **5**,
`UNKNOWN-CHILD` **3**, `ORDER` **1**; after §5.8, 14 distinct —
`UNKNOWN-NAME` **4**, `UNKNOWN-CHILD` **2**, the other two unmoved; after §5.9,
**0 distinct — every kind at 0**; after §5.10, still 0 on every kind, which is
the point of that unit rather than a footnote to it.**
`tests/mock_schema_shape.rs` `PINS` carries the live numbers; this
block is the baseline the sweep started from and is left as it was.

§5.9's fourteen rows were four independent groups, each perturbed on its own
against the all-zero pins:

| group | delta when the old output is put back |
|---|---|
| device — `ScopeDef`, `SystemLog` | `MISSING-REQUIRED` +1, `UNKNOWN-NAME` +3, `UNKNOWN-CHILD` +2 |
| recording / search required members | `MISSING-REQUIRED` +4 |
| PTZ `Spaces`, the two event dialects | `MISSING-REQUIRED` +2 |
| OSD order, Media2 encoder instances | `ORDER` +1, `UNKNOWN-NAME` +1 |

They sum to the 7 / 4 / 2 / 1 the group started from, and **no group opened a
row for another** — which is not automatic: every element added is a fresh
chance at a wrong namespace or position, and this group added a whole
`tt:PTZSpaces` subtree.

**`UNKNOWN-NAME` went up by one, and it is not a new defect.** Inlining the
Media2 configurations reuses `render_video_encoder`, the same helper the list
getter uses, so the `tt:Profile` element it already emitted now appears at a
second path and is counted a second time.
`tt:VideoEncoder2Configuration` declares `Profile` and `GovLength` as
**attributes**; `VideoEncoderConfiguration2::from_xml` parses both as child
elements, so closing it is a client change and belongs to §5.3, where it will
close both rows at once. The alternative — a second copy of the encoder body,
free to drift from the list getter — is the failure `CLAUDE.md` step 5b exists
to prevent, so the counted row was preferred to the duplicate renderer.

**That paragraph was right about the arithmetic and wrong about the reading.**
§5.5 landed the fix and both rows did close together, 9 → 7, exactly as
predicted — but *"it is not a new defect"* was taken downstream to mean *not a
defect*, and it was the fifth client-facing one (§0). It also landed in §5.5
rather than §5.3, because §5.3 shipped without it. The accurate form of the
sentence is: **the row was not new; the bug it named was never triaged.**

This paragraph used to end *"No other kind has moved."* True of §5.0 and §5.1,
and it stopped being true at the imaging slice: **eight rows across four kinds,
of which seven were one wrong element name.** `GetMoveOptions` rendered the
focus ranges as `PositionSpace` / `SpeedSpace`. One undeclared name reports in
three kinds at once — undeclared (`UNKNOWN-NAME`), not accepted by its parent
(`UNKNOWN-CHILD`), and the real required child therefore absent
(`MISSING-REQUIRED`) — so a fix that moves several counters at once is not
evidence of several fixes, and the reverse reading is the one to distrust.

The single row §5.0 removed is worth naming, because it is the whole argument
for the anchoring assertion that fix added: it was the `DigitalInputs` *child*.
The response **root** was the corpus's one unanchored non-fault root, and an
unanchored root is not judged at all — so the defect that mattered contributed
**zero** to every number in this table. Putting only the wrapper's namespace
back leaves all five counts identical and the test still fails, on the
anchoring assertion rather than a pin.

**This replaces run 2's 108, which was not a defect count.** Almost the whole
difference is rollup, not suppression: one element in the wrong namespace used
to produce three rows — undeclared where it was put, unknown to its parent, and
the displaced element now missing — and is one `WRONG-NS` row now. One row was
a genuine false positive (the `xs:choice` bug, §2). Details and the
perturbations that prove the rollup consolidates rather than silences are in
the lab's `NOTES.md`, run 3.

63 distinct is still not 63 *fixes*: `MISSING-REQUIRED` on the five Media2
`ConfigurationSet` members is one decision (inline the configurations), not
five.

### 1.1 The dominant class was on nobody's list: wrong namespace

**Fixed in §5.1 — the table there supersedes this one**, which named the
directions correctly but read as though the unit of the fix were the *element
name*. It is the *declaration*: two responses of one service can put the same
element name in two namespaces, and both events and recording do.

16 rows, and the direction is **not** uniformly "`tt:` where a service
namespace belongs" — which is what run 2 read like and what the fix would have
been written against:

| where | emitted | declared |
|---|---|---|
| storage configuration `Data` and its `LocalPath` / `StorageUri` / `User` / `User/UserName` | `tt:` | `tds:` |
| Media2 profile `Name`; `GetVideoEncoderInstances` → `Total`; `GetVideoSourceModes` → `Encodings`, `MaxFramerate`, `MaxResolution`, `Reboot` | `tt:` | `tr2:` |
| recording `JobItem` → `JobToken`, `JobConfiguration` | `trc:` | **`tt:`** |
| events `CurrentTime`, `TerminationTime`, `TopicExpressionDialect` | `tev:` | **WS-BaseNotification** |

The last two rows run the other way, and the events one leaves ONVIF's own
namespaces entirely. **A blanket "move it to the service namespace" sweep would
have introduced three new defects** while closing thirteen.

The three events rows were invisible until run 3 fetched `b-2.xsd` — they were
part of the "54 unanchored roots" that §2 declined to judge.

**Why nothing caught it:** `every_response_binds_the_prefixes_it_uses`
(`src/mock/`) asserts that each prefix the mock emits is *declared*. It says
nothing about whether the element belongs in the namespace that prefix names.
The test is doing exactly what it says and certifying the wrong property.
Strengthening it is part of the work, not a follow-up.

### 1.2 Sequence-order violations — six rows, five causes

`xs:sequence` order is significant. Confirmed by hand against the schema, not
only via the checker, for the first:

| response | what is out of place |
|---|---|
| ~~Media1 `GetProfiles` → `Profile`~~ **fixed** | the video encoder configuration is emitted before the audio source configuration; the schema has them the other way round |
| ~~Media2 `GetProfiles` → `ConfigurationSet`~~ **fixed** | the same inversion |
| ~~`GetOptions` → `ImagingOptions20`~~ **fixed** | badly scrambled — most members are in a different position. Only `WideDynamicRange` held its index; `BacklightCompensation` moved from last to first. **The schema order is not alphabetical, though it looks it** — `WideDynamicRange` precedes `WhiteBalance`, so sorting the members is a wrong fix that gets nine of ten right |
| ~~Media2 `GetMetadataConfigurations`~~ **fixed** (§5.7) | analytics emitted before PTZ status (**two rows**, one per configuration in the response — one cause) |
| ~~`GetOSDOptions` → `OSDTextOptions`~~ **fixed** (§5.9) | the font-size range is emitted last, the schema places it second |

**`ORDER` is now 0.** The one row left was the OSD text options, and moving
`FontSizeRange` from last to second is the whole fix — `tt:OSDTextOptions`
declares `Type` (repeated), `FontSizeRange`, `DateFormat`, `TimeFormat`,
`FontColor`, `BackgroundColor`, `Extension`, and the mock emits the first four.

A zero here proves less than the five rows above suggest. The rule fires only
when **two or more** children of a type are recognised: `GetVideoEncoderInstances`
had `Total` before the repeated wrapper, which is a real sequence violation, and
`ORDER` never saw it because the wrapper was misnamed and therefore not
recognised at all. §5.9 fixed both in one edit and only the OSD row moved.

Media1 `GetProfiles` is the most-used response in the crate.

The Media2 row used to read *"plus an `Audio` child where the schema names
`AudioEncoder`"*. That part is fixed (`8091892`); ~~the inversion is not~~ and
so is the inversion, in §5.4.

**"The same inversion" is a description of the symptom, not of the cause, and
the two rows had to be derived separately.** `tt:Profile` and
`tr2:ConfigurationSet` are different types declared in different schemas —
`onvif.xsd` and `media2.wsdl` — whose members do not even share names
(`AudioSourceConfiguration` against `AudioSource`). That they agree on placing
the audio source between the two video members is a fact about ONVIF, not
something either order licenses assuming of the other. Perturbed independently:
either one alone moves `ORDER` by exactly one.

### 1.3 Undeclared element names — the `AFModes` class again

12 `UNKNOWN-NAME` rows; the six that are not §1.5 or a duplicate of a
`UNKNOWN-CHILD` row below:

- ~~`ScopeAttribute` in `GetScopes`.~~ **Fixed** — §5.9. **The correct name was
  already in this repository**, in a comment 150 lines below the bug at
  `src/mock/services/device.rs:277`. The mock emitted the wrong one at `:124`
  and the client unit fixture agreed with it at
  `src/tests/client/device_tests.rs:403` and `:407`. The finding was right in
  every particular, including that the client is unaffected — oxvif's parser
  reads `ScopeItem` only. `tt:Scope` declares `ScopeDef` then `ScopeItem`, both
  `[1]`, so this closed a `MISSING-REQUIRED`, an `UNKNOWN-CHILD` and an
  `UNKNOWN-NAME` at once: three kinds, one name, the pattern §1 warns about.
- ~~Imaging `GetMoveOptions` emits space-style names (`PositionSpace`,
  `SpeedSpace`) under the absolute and continuous focus options where the
  schema declares a position and a speed. Same service and same class as the
  `tt:AFModes` defect fixed in 0.15.0.~~ **Fixed.** The finding was right; the
  classification was not. It is **not** the `AFModes` class — `AFModes` is a
  real ONVIF name at a deeper level, which is why reverting it moves
  `UNKNOWN-CHILD` alone. `PositionSpace` and `SpeedSpace` are declared nowhere
  in `tt:` for focus at all: the vocabulary is borrowed from PTZ, where a
  *space* is a URI naming a coordinate system. Reverting the three names moves
  three kinds — `UNKNOWN-NAME` 8 → 11, `UNKNOWN-CHILD` 4 → 6 and
  `MISSING-REQUIRED` 21 → 23, the last because the required `Position` and
  `Speed` are then absent.

  **`ImagingMoveOptions::from_xml` reads the same four invented names**
  (`src/types/imaging.rs`, `PositionSpace` / `SpeedSpace` / `DistanceSpace`),
  so every range it returns is `None` against a conformant device, and the unit
  fixture in `src/tests/client/imaging_tests.rs` was written to agree with it.
  Same shape as the Media2 `Audio` → `AudioEncoder` client bug of `8091892`.
  ~~**Not fixed here** — out of this work unit's scope, and it needs the fixture
  and the getter changed together.~~ **Fixed.** The finding was right in every
  particular; only the deferral is superseded. Three things measured when it was
  done that the finding did not say:

  - **"the same four invented names" is three, not four.** `PositionSpace`,
    `SpeedSpace` and `DistanceSpace` — the parser called `range("Absolute",
    "SpeedSpace")` and `range("Continuous", "SpeedSpace")`, two call sites of
    one name. Five *ranges* came from three wrong names.
  - **The rename alone does not settle the parse.** All three focus families are
    `[0..1]`, so an absent family is `None` — but each declares exactly one
    required range (`Absolute/Position`, `Relative/Distance`,
    `Continuous/Speed`) and only the two `Speed` members under Absolute and
    Relative are optional. A present family missing its required range is now
    `MissingField`, since `None` there is indistinguishable from "the device
    does not offer that move type".
  - **The real gap was that nothing connected the two sides.** The mock had
    already been corrected here, and no test noticed the client still
    disagreeing: the only mock-driven call was
    `s.imaging_get_move_options("VS_1").await.unwrap()` in
    `tests/mock_multi_sensor.rs`, asserting no field.
    `imaging_move_options_ranges_survive_the_round_trip` in
    `tests/mock_workflow.rs` now asserts the bounds, and reverting *either* side
    reddens it. Of the imaging getters this was the only one with the gap —
    settings, options, status and service capabilities are each asserted
    field-by-field against the mock elsewhere in `tests/`.
- ~~`AnalyticsSupported` under the Media2 metadata options extension;~~
  **fixed** — §5.7, and it was a client bug (§0.6) and a deletion, not the
  rename this list said below;
  ~~`MaximumNumberOfProfiles` under the video source configuration options;~~
  ~~`ProfilesSupported` under the video encoder configuration options;~~
  **both fixed** — §5.8, and both were client bugs (§0.7, §0.8), as the two
  bullets below had already established.
  ~~`Profile` inside a video encoder configuration;~~ ~~`Number` under an encoder
  instance;~~ ~~`UsernameToken` under the device security capabilities;~~
  ~~`SystemLogUri` and its `LogType` in `GetSystemUris`.~~ **The last two are
  fixed in §5.9, and both were client bugs — §0.9 and §0.10.** This list called
  them undeclared names and stopped there, which is the §0 failure again: the
  entry that names a bug correctly and never reaches the blast-radius count.

  Neither was a rename in the sense this list assumes. `SystemLogUri` **is** a
  declared ONVIF name — it is the `complexType` that `SystemLog` is typed as, so
  the mistake was reading a type name as an element name, and the `LogType` next
  to it should have been `Type`. `Number` **is** a declared `tr2:` element — on
  `tr2:EncoderInstance`, one level below where the mock put it — and it reported
  here only because the mock also placed it in `tt:`. Its parent
  `tr2:EncoderInstanceInfo` carries an `xs:any`, so three further errors in that
  one subtree (wrapper named `Encoding` instead of `Codec`, the whole subtree in
  `tt:`, `Total` first) moved **no** counter at all.

  **`UsernameToken` is fixed** — §5.6, and it was neither a rename nor a mock
  fidelity item: it was two *types* mixed into one element, and the mock's copy
  had been propping up an unsound health check. See the settled analysis below.

  **`Profile` is fixed** — §5.5, and it was a client bug (§0.5), not the mock
  fidelity item this flat list implied. `tt:VideoEncoder2Configuration` declares
  it as `xs:attribute`, together with `GovLength`, `AnchorFrameDistance`,
  `GuaranteedFrameRate`, `Signed` and `SecureStreamingProtocolAlgorithm`; the
  type's only child elements are `Name`, `UseCount` (inherited from
  `tt:ConfigurationEntity`, which also carries the required `token` attribute),
  `Encoding`, `Resolution`, `RateControl`, `Multicast` and `Quality`.

  **Two more names on this list are the same class**, parsed against the schema
  while §5.5 was being written, and neither is a rename:

  - `ProfilesSupported` is an `xs:attribute` of
    `tt:VideoEncoder2ConfigurationOptions`, of type `tt:StringAttrList` —
    which is `<xs:list itemType="xs:string"/>`, so **one attribute holds the
    whole space-separated list**. The mock's repeated `<tt:ProfilesSupported>`
    element and `VideoEncoderOptions2::profiles`' `children_named(…)` are both
    wrong, and unlike §5.5 the fix changes the *cardinality* of the parse, not
    only where it reads from. **Not fixed in §5.5** — it is a different type
    (`tt:VideoEncoder2ConfigurationOptions`, not
    `tt:VideoEncoder2Configuration`) and a separate client-facing bug.

    **Fixed in §5.8.** Two corrections to the sentence above. It said the fix
    *"would take `UNKNOWN-NAME` 7 → 6"* — true when written, stale by the time
    it was done: §5.6 and §5.7 had taken the kind to 5, and §5.8 measured
    5 → **4**. And *"the fix changes the cardinality of the parse, not only
    where it reads from"* understates the scope by four members: parsing the
    whole type against the schema found `GovLengthRange` and
    `FrameRatesSupported` misclassified the same way — invisible to the checker,
    because both are real `tt:` elements on the *Media1* options types — plus a
    `frame_rate_range` field for an element `tt:VideoEncoder2ConfigurationOptions`
    does not declare, and `frame_rates` typed `u32` against a `tt:FloatList`.
    **One visible row, six wrong members.** The lesson is the one §1.3 keeps
    restating: the row names a symptom, and the type has to be read out whole.
  - `MaximumNumberOfProfiles` is an `xs:attribute` of
    `tt:VideoSourceConfigurationOptions`. It is also a real *element*, on the
    unrelated `tt:ProfileCapabilities`, which is why it reports only as
    `UNKNOWN-CHILD` and never as `UNKNOWN-NAME` — the same double-declaration
    that hid `GovLength` completely.

    **Fixed in §5.8**, `UNKNOWN-CHILD` 3 → **2**. Media1 and Media2 return the
    same type from `GetVideoSourceConfigurationOptions`, so this was two
    renderers and one parser, and `video_source_options_max_profiles_is_an_attribute`
    asserts both services rather than the one the row happened to name.
  - `AnalyticsSupported` is declared **nowhere** in `tt:`, as element or
    attribute. ~~That one is a rename, and belongs to the class this section was
    named for.~~ **Wrong, and wrong in the direction this section keeps warning
    about.** Re-measured in §5.7 by parsing all fifteen files:
    `tt:MetadataConfigurationOptionsExtension` declares exactly `CompressionType`
    (`[0..*]`) and a further `Extension` typed
    `tt:MetadataConfigurationOptionsExtension2`, which is an
    `xs:any ##targetNamespace` and nothing else. **There is no element to rename
    it to at any level of the extension chain.** It is the `UsernameToken` shape,
    not the `AFModes` shape: a fact that belongs to a different operation —
    `GetCapabilities`' `tt:AnalyticsCapabilities/AnalyticsModuleSupport` — and
    the fix is to delete it. It was also the sixth client-facing bug, because
    `MetadataConfigurationOptions::from_xml` read the same invented name; §0's
    count is five no longer.

    The sentence directly above this list — *"Read the declaration before
    assuming an undeclared name wants renaming"* — was written about the three
    attributes and is right about this one too: **four of the eight names on this
    list were not renames, and this was the last one still filed as one.**

  **Read the declaration before assuming an undeclared name wants renaming.**
  Three of the eight names on this list are attributes, and the first two were
  triaged as misspellings for a fortnight.

**`UsernameToken` is settled — §5.6.** This section used to read:

> `UsernameToken` is worth a second look rather than a rename: the name plausibly
> belongs to the device *service* capabilities type, which is a different type in
> `devicemgmt.wsdl`. If so the mock is mixing the two types, and
> `src/health/checks.rs` cross-checks that attribute as one of its eighteen
> twice-stated ones, so it has an opinion either way.

The guess was right and the consequence was larger than "either way". Measured
against the schema set, parsing every one of the fifteen files:

- `tt:SecurityCapabilities` (`onvif.xsd`) declares **eight `xs:element`s** —
  `TLS1.1`, `TLS1.2`, `OnboardKeyGeneration`, `AccessPolicyConfig`,
  `X.509Token`, `SAMLToken`, `KerberosToken`, `RELToken` — then an
  `xs:any ##other` and an optional `Extension`. No `UsernameToken`.
- `tds:SecurityCapabilities` (`devicemgmt.wsdl`) declares **23
  `xs:attribute`s**, `UsernameToken` among them, and is the type
  `GetServiceCapabilities` answers with.
- `UsernameToken` appears **nowhere else in any of the fifteen files, in any
  form** — and it is not reachable through `SecurityCapabilitiesExtension`
  (`TLS1.0` + `Extension`) or `…Extension2` (`Dot1X`, `SupportedEAPMethod`,
  `RemoteUserHandling`) either.

So there is no element to rename it to. The mock drops it, and the fact keeps
its correct home in `resp_service_capabilities`, which already carries it as an
attribute.

**And the client kept reading it for four commits — §5.10.** The three bullets
above were written with the schema open and are all correct; what none of them
asked is what `src/types/capabilities.rs` does with the name.
`SecurityCapabilities::username_token` read `Device/Security/UsernameToken`, so
the public field was `false` from every conformant camera — the eleventh
client-facing bug, and §0's count did not move for it because §5.6's own commit
message filed it as *"deliberately not fixed, reported instead"* and nothing
carried that forward.

Two further facts the same three bullets contain and nobody read off them:

- the eight declared elements are `TLS1.1`, `TLS1.2`, `OnboardKeyGeneration`,
  `AccessPolicyConfig`, `X.509Token`, `SAMLToken`, `KerberosToken`, `RELToken`,
  and `SecurityCapabilities` modelled **four**;
- the extension chain is `TLS1.0`, then `Dot1X` / `SupportedEAPMethod` `[0..*]`
  / `RemoteUserHandling`, and it modelled **none** — the third bullet listed all
  four, as evidence that `UsernameToken` was not among them.

So the paragraph that settled the removal also enumerated eight members the
crate could not read, in the sentence used to rule one out. **A list written to
prove an absence is still a list of what is present.** All eight are modelled as
of §5.10, and seven of them are `xs:attribute`s of `tds:SecurityCapabilities`
too, which took `capability_cross_check` from seventeen twice-stated facts to
twenty-four.

**The health-check consequence is the part worth recording.**
`capability_cross_check` had paired `caps.device.security.username_token`
against `d.security.username_token` as one of its eighteen twice-stated facts.
It is not one: the device-level side reads an element that type never declares,
so on **every conformant camera** it is `false`, and the comparison can only
ever produce a spurious `service_only` — never the contradiction it exists to
find. It looked like it worked for exactly one reason: oxvif's mock emitted a
`<tt:UsernameToken>` no camera sends. Mock and check agreeing with each other
and with nothing else — the same shape as the Media2 `Audio` defect in §0.5,
one level up. The pair is removed; eighteen twice-stated attributes are
seventeen.

**Seventeen is now twenty-four — and the correction is not the one this
paragraph looks like it made.** Removing `UsernameToken` fixed a pair that was
never real. What it did not do is ask how many *real* pairs the check was
missing: `tt:SecurityCapabilities` and `tds:SecurityCapabilities` both declare
eleven security facts, and the check compared four, because the device-level
side had fields for four. §5.10 adds the other seven. Both numbers were counts
of *what the function compares*, read as counts of *what the two types both
declare*, which is the drift a corrected count is least likely to be re-examined
for.

### 1.4 Required members omitted

23 rows. After §1.1 fallout is discounted, the genuine omissions are:
~~device capabilities (system, security, events, recording, search)~~
(**fixed** — §5.6, all five, `MISSING-REQUIRED` 16 → 11), ~~`Scope`'s
`ScopeDef`, `GetRecordings` → `Tracks`, `EndSearch` → `Endpoint`, PTZ
configuration options → `Spaces`~~, ~~the metadata configuration's `Multicast` /
`SessionTimeout` and multicast `AutoStart` / `TTL`, the metadata PTZ-status
filter options~~ (**fixed** — §5.7, all four rows, `MISSING-REQUIRED` 11 → 7),
~~imaging focus position and speed~~ (**fixed** — they were
absent only because the invented `…Space` names stood in their place, so they
closed with §1.3's rename rather than as omissions in their own right), ~~the
event-properties
response, and recording source information — the last of which
`src/mock/state.rs` already **stores** and does not render, the same shape as
the `MTU` bug.~~

**All seven remaining rows are fixed in §5.9 — `MISSING-REQUIRED` 7 → 0.** Four
things measured that this paragraph did not say:

- **The `MTU` prediction was right, and understated.** `RecordingState` stored
  `source_id`, `source_name`, `location` and `description`, and
  `GetRecordingSearchResults` rendered `Name` alone — so it dropped three
  members it was already holding. But `Address` had **no field at all**, so
  `CreateRecording` read it out of the request and threw it away while
  `RecordingSourceInformation::address` kept parsing it. Storing-and-not-
  rendering was the smaller half.
- **A required member over an empty list means an empty wrapper, not no
  wrapper.** `GetRecordingsResponseItem/Tracks` is `[1]` and
  `tt:GetTracksResponseList` declares `Track` as `[0..*]`, so a recording
  holding nothing sends `<tt:Tracks/>`. Same for `Address`: the element is
  required, the *value* may be empty, and empty is what keeps
  `address: Option` observable as `None`.
- **`EndSearch` was not an empty response at all.** `search.wsdl` declares one
  required child, `Endpoint`, an `xs:dateTime`. The mock answered `resp_empty`
  and nothing noticed, because `end_search` returns `()` and only checks that
  the response element is present. It is now the third caller of
  `soap::security::unix_secs_to_iso8601`, so the mock has one clock.
- **`Spaces` is the one row where clearing the finding was the wrong fix.**
  `<tt:Spaces/>` satisfies `tt:PTZConfigurationOptions` and asserts the head
  supports no coordinate space — precisely the 0.15.0 `SupportedPTZSpaces`
  defect, on the sibling element. It is filled from the node the configuration
  drives, through the same helper `GetNodes` uses, so `PTZConfig_2` reports the
  four zoom slots and no pan/tilt ones. **A shape checker cannot tell those two
  fixes apart**, which is the §6 argument in one row.

~~Five of the 23 are the Media2 `ConfigurationSet` members
(`VideoSource`, `AudioSource`, `VideoEncoder`, `AudioEncoder`, `PTZ`), which
are **one** decision: `8091892` established that a conformant device inlines
the full configuration, and this mock renders a token-only reference. Fixing
that closes five rows at once and is the largest single change in the sweep.~~

**Done in §5.4 — `MISSING-REQUIRED` 21 → 16, exactly the five rows, from one
edit to `render_profile_media2`.** Two things the prediction did not say:

- **It is one decision but not one renderer.** Each member is inlined by the
  helper the corresponding *list* getter already used —
  `media::render_vsc_body`, `media::render_audio_source_config`,
  `render_video_encoder`, `render_audio_encoder_media2`,
  `ptz::render_config` — so a profile cannot describe a configuration
  differently from the getter that lists it. `render_vsc_body` and
  `render_video_encoder` needed only a `qname` parameter; the other three
  already had one.
- **Inlining is only worth having if the inlined copy tracks state.** Nothing in
  §1.4, and nothing the checker reports, distinguishes a renderer reading
  `DeviceState` from one emitting a plausible constant: both satisfy every
  required member. `a_config_write_shows_inside_both_services_profiles` in
  `tests/mock_media1_media2_agree.rs` is the assertion that can — it repoints
  `VSC_1` at the other sensor and re-reads it *through a profile* on both
  services.

### 1.5 `GetDigitalInputs` was sent to the wrong service — a client bug

**Fixed — see §5.0 for what it took and what it moved.** Everything below is
the finding as established, kept unchanged.

Not a mock finding. It surfaced from the "54 unanchored roots" of §2, which is
why establishing that cause came before acting on anything.

`GetDigitalInputsResponse` is declared **only** in the DeviceIO service's WSDL,
whose target namespace is `…/ver10/deviceIO/wsdl`, and whose binding gives the
action `…/ver10/deviceio/wsdl/GetDigitalInputs` (ONVIF's own casing
inconsistency, not a typo here). `devicemgmt.wsdl` declares neither the request
nor the response; its only occurrence of the string is prose inside an
enumeration's documentation.

`src/client/device.rs:707` sends `…/ver10/device/wsdl/GetDigitalInputs` with a
`tds:` body, to the device management endpoint. Against a real camera the device
service has no such operation.

What makes this a finding rather than a guess about service names:
`GetRelayOutputs` and `SetRelayOutputState` have their elements declared in
`devicemgmt.wsdl` and appear in **both** portTypes — DeviceIO reuses the `tds:`
elements. So oxvif is right about those two and wrong about this one, and the
difference is visible in the WSDLs.

**No oxvif test can see it**, and none could: the mock answers whatever action
it is asked for, so mock and client agree — the class `CLAUDE.md` records, once
more. The fix needs the DeviceIO endpoint from `GetServices`, so it is not
purely a string change; scope it before writing it.

---

## 2. The checker's own defects — all four fixed, lab `ac74fee`

Kept as a record of what run 2's output actually was, because the fixes are
what turned 108 into 63 and one of them turned an ignored line into §1.5.

| defect | effect | now |
|---|---|---|
| `xs:choice` walked as a sequence | demanded every alternative at once; the PTZ preset-tour row was a **false positive** | members share one position and one is enough. Two `xs:choice` in the whole set, no `xs:all`, no `xs:group ref` — counted, not assumed |
| no rollup | one wrong-namespace element produced three rows | one `WRONG-NS` row; identical rows across documents collapse with the count kept |
| `#anon880_global_…` type names | unreadable exactly where it mattered — the event responses are almost all inline types | `<GetEventPropertiesResponse>`, owner path for nested |
| 54 unanchored roots, cause unknown | **"not a mock defect until settled"** | settled: 50 SOAP faults, 3 WS-BaseNotification, **1 real client bug** (§1.5) |

Both behavioural fixes were perturbation-proved against the corpus rather than
accepted because a row disappeared — three mutations for the choice, two for
the rollup, each reverted and the baseline re-checked. The tables are in the
lab's `NOTES.md`, run 3.

**The residual, so the next reader knows what the tool still cannot see:**

- 50 of 155 responses are SOAP faults, so **a third of the corpus carries no
  shape evidence at all**. `tools/dump_responses.rs` has an override table
  keyed by operation name that needs finishing. Two of the five shape defects
  found before this tool existed lived in operations that fault here.
- 946 children are skipped for an unresolvable type — mostly builtins, but not
  audited.
- The corpus is a snapshot. **Re-dump before every run.** Run 2 was taken
  against output the mock no longer produced, and its numbers were quoted for
  a day before that was noticed.

---

## 3. Settled — `8091892`. It was not mock-only

The question was whether `CLAUDE.md` step 5b's *"Media2 emits token
references"* was design or error. `media2.wsdl` answers it: `tr2:ConfigurationSet`
types **every** member as the full configuration, so a conformant device inlines
it exactly as Media1 does. The design note was wrong.

Two consequences, both landed:

- The member is named `AudioEncoder`; `src/types/media.rs` read `Audio`, so
  `MediaProfile2::audio_encoder_token` had been `None` from every conformant
  device. Fixed. The existing test asserted that field and stayed green
  throughout, because its fixture had been written to match the parser.
- `CLAUDE.md` step 5b corrected, with the old claim quoted and a rule beside
  it: **check a shape claim against the WSDL before writing it down there.**

~~Still open, deliberately: the mock renders a token-only reference rather than
inlining. That is now a documented simplification rather than a claim about the
schema, and it is the five `MISSING-REQUIRED` rows of §1.4 — the largest single
change in the sweep.~~

**Closed in §5.4.** The mock inlines. A documented simplification is only
defensible while nothing depends on the omission, and something did:
`MediaProfile2::video_source_token` is read from a `SourceToken` *inside* the
video source configuration, so against this mock it was permanently `None` —
the field existed, was parsed, and could not be exercised. The same argument
that made `8091892` a client bug rather than a mock nicety applies one level
down: a mock that emits a shape no device produces cannot test the parser that
reads the shape devices do produce.

Third consequence of §3, only visible once the mock moved: **the unit fixtures
had been written to the token-only shape too**, and
`test_get_profiles_media2_parses_audio_ptz_tokens` carried a doc comment
promising *"the element names and prefixes here are the ones a conformant device
sends"*. The names were right and the nesting was not — the same
mock-and-fixture-agree-with-each-other failure `8091892` found one level up.
Both Media2 profile fixtures now carry the configurations, which is what makes
`video_source_token` assertable at all.

---

## 4. What will break when the mock's output moves

- ~~**167 byte-level `contains("<prefix:…")` assertions** across eight files.
  **68 of them assert on mock output** — 62 in `src/mock/state.rs`, 6 in
  `src/mock/dispatch.rs` — and every one that names a prefix that moves will go
  red. That is the intended signal, not collateral: those assertions exist
  because the client parser cannot see prefixes, so they are the only thing that
  can.~~

  **Measured when §5.1 landed: not one of them moved.** All sixteen
  `WRONG-NS` rows were fixed and the whole suite stayed green — 818 lib tests
  before and after, byte for byte.

  The counting was right and the inference was wrong. Of the 62 in
  `src/mock/state.rs`, 60 name a `tt:` element and **none of the 60 names an
  element that had to move.** They cluster on `GetSystemDateAndTime`,
  `GetHostname`, `GetNTP`, `GetNetworkProtocols`, the PTZ spaces — settled
  parts of the mock that nobody suspected — while every one of the sixteen
  defects lived in a nested element of a *service-declared* type that no
  assertion had ever named. An assertion count is not coverage of the thing
  about to change.

  The one assertion in the whole repository that named a moved element was
  `src/tests/client/device_tests.rs`, on a **client request body**, and it
  turned out to be asserting a second client-facing bug rather than guarding
  against one — see §5.1.

  **The imaging slice moved exactly one of the 62**, and it is the shape the
  struck bullet predicted for all of them:
  `imaging_move_options_fault_on_a_fixed_lens` in `src/mock/state.rs` asserted
  `contains("<tt:PositionSpace>")` — an assertion that existed to pin the mock's
  output and was pinning an invented name. So the count is not coverage, but
  the mechanism is real when an assertion happens to name the defect. Re-aimed
  at `<tt:Position>`, and it now also asserts that no `…Space>` element appears
  in a focus response at all, which is the thing that was actually wrong.
- The remaining 99 are on client-*emitted* request bodies and on fixture
  parsing, and are unaffected — except `src/tests/client/device_tests.rs:403`
  and `:407`, which encode `ScopeAttribute` in a fixture and must move with the
  mock.
- `tests/mock_action_snapshot.rs` records an ok/fault outcome per action, not
  bytes, so it is unaffected.
- `tests/mock_roundtrip.rs` and `tests/mock_token_discrimination.rs` go through
  the client, which is namespace-blind and order-independent, so they are
  unaffected. **That is the point: neither table could ever have caught any of
  this.**

---

## 5. Work units

Grouped so each lands in one file with one perturbation:

0. ~~**`GetDigitalInputs` → the DeviceIO service** (§1.5).~~ **Done.** A client
   fix, not a mock one, and the only unit that changes what a caller sees.

   `get_digital_inputs` takes a `deviceio_url` — **a breaking signature
   change**, and correct: the crate is stateless and every other non-device
   service already takes its endpoint. `OnvifSession::get_digital_inputs()` is
   unchanged for callers; it resolves the URL itself.

   What it needed beyond the two strings, since the summary in §0 had guessed
   "one line":

   - `tmd` → `…/ver10/deviceIO/wsdl` in `src/soap/envelope.rs`.
   - `OnvifService::is_device_io()`, **case-insensitive**. The WSDL's
     `targetNamespace` capitalises `deviceIO` and every soapAction it binds
     spells the same segment `deviceio`; firmware copies either, and an exact
     match would find the endpoint on only some devices.
   - `Capabilities::device_io` already existed and was parsed but never used;
     the session fills it from `GetServices` too, and errors with
     `MissingField("DeviceIO service URL")` when the device has none.
   - The mock advertises the endpoint in **both** discovery paths, dispatches
     `ver10/deviceio/wsdl/`, and renders the response in `tmd:`.
   - `tt:DeviceIOCapabilities` requires five counts beside `XAddr`. Adding the
     block with only `XAddr` raised `MISSING-REQUIRED` 23 → 24 — a new defect,
     introduced and caught inside one edit. They are filled from the seeded
     collections, and `VideoOutputs` is `0` because the mock models none.
   - **`tt:IO/InputConnectors` said 1 while `default_digital_inputs` seeds 2.**
     Pre-existing, found only because the DeviceIO block made the mock state
     the same fact a third time. Now 2.

   The guard: **every non-fault response root must anchor**, asserted separately
   from the pins. See §1 for why the pins could not have carried this.

   `docs/reference/deviceio.md` had said since 2026-05 that this operation
   belongs to DeviceIO. It was right for three months while the client was
   wrong — the `ScopeAttribute` shape exactly (§1.3), where the correct answer
   sat in the repository the whole time. It also claimed *"❌ not implemented"*,
   which is how a reader would have missed the contradiction. Both corrected.
   `OPERATIONS.md` had **no row at all** for an operation shipped in 0.9.9.
1. ~~**Namespace correctness, per service** — device, Media2, recording,
   events.~~ **Done. `WRONG-NS` 16 → 0, no other kind moved.**

   §1.1's warning held, and understated it. Four families, four directions:

   | family | was | is | why |
   |---|---|---|---|
   | storage (`Data`, `LocalPath`, `StorageUri`, `User`, `UserName`) | `tt:` | `tds:` | declared in `devicemgmt.wsdl`'s own qualified schema; **none of the five exists in `tt:` at all** |
   | Media2 (`Profiles/Name`, `Info/Total`, the four `VideoSourceMode` members) | `tt:` | `tr2:` | declared locally in `media2.wsdl` |
   | recording (`JobItem/JobToken`, `JobItem/JobConfiguration`) | `trc:` | **`tt:`** | `JobItem` is typed `tt:GetRecordingJobsResponseItem`, a complexType in `onvif.xsd` |
   | events (`CurrentTime`, `TerminationTime`, `TopicExpressionDialect`, `FixedTopicSet`) | `tev:` / `wstop:` | **`wsnt:`** | `ref="wsnt:…"` in `event.wsdl` |

   Two traps that a name-based sweep walks straight into, both inside a single
   file:

   - **`CurrentTime` and `TerminationTime` are `wsnt:` on
     `CreatePullPointSubscriptionResponse` and `tev:` on
     `PullMessagesResponse`.** `event.wsdl` declares the first pair by `ref` and
     the second pair locally. Same two names, same service, same file.
   - **`JobToken` and `JobConfiguration` are `tt:` on
     `GetRecordingJobsResponse` and `trc:` on `CreateRecordingJobResponse`**,
     for the same reason in reverse.

   Perturbed one family at a time: 5 / 6 / 2 / 3 rows come back, each on the
   assertion, each reverted green.

   **One of the four was not driven by the checker.** `wstop:FixedTopicSet` →
   `wsnt:` produces *no* `WRONG-NS` row when reverted, because `t-1.xsd` is not
   in the schema set so `wstop:` is an unknown namespace the checker skips. It
   showed up only as a name inside a `MISSING-REQUIRED` row's list, which does
   not move a count. It was found by reading `event.wsdl`. **The count going to
   zero does not mean the class is closed.**

1a. **`set_storage_configuration` sent the same five elements in `tt:` in its
   *request body*** — a second client-facing defect, against real cameras, and
   **structurally invisible to the checker**, which reads the mock's responses
   and never the client's requests. Fixed in the same commit; the two would
   otherwise have disagreed, which is the failure this whole exercise exists to
   remove.

   It was found by asking why §4's predicted red did not happen. That question
   is the reusable part: **when a guard you expected to fire stays silent, the
   silence is the finding.**
2. **Sequence order** — five renderers, six rows (§1.2). **Imaging done**
   (`ImagingOptions20`, `ORDER` 6 → 5); **both `GetProfiles` inversions done**
   with §5.4 (`ORDER` 5 → 3); **Media2 `GetMetadataConfigurations` done** with
   §5.7 (`ORDER` 3 → 1). **`GetOSDOptions` done** with §5.9 (`ORDER` 1 → **0**)
   — `tt:OSDTextOptions` places `FontSizeRange` second, between the repeated
   `Type` and the `DateFormat` list, and the mock emitted it last.

   *"five renderers, six rows"* was right as a count and misleading as a plan:
   the sixth renderer, `resp_video_encoder_instances`, also had a real sequence
   violation (`Total` before the repeated wrapper) and **`ORDER` never saw it**,
   because the wrapper was misnamed and an unrecognised child cannot be out of
   sequence. It closed in §5.9 as part of the name fix. The unit of an `ORDER`
   row is *"a renderer with at least two recognised children"*, not *"a renderer
   in the wrong order"*.
3. ~~**Undeclared names** — `ScopeAttribute`, the imaging focus options, the
   options extensions (§1.3).~~ **Done — `UNKNOWN-NAME` and `UNKNOWN-CHILD` both
   0.** §5.9 took the last three rows: `ScopeAttribute`, `SystemLogUri`/`LogType`
   and the encoder-instance `Number`. ~~Settle `UsernameToken` by type rather than
   renaming it.~~ **`UsernameToken` done** — §5.6; settling it by type was the
   right instruction and there was no element to rename it to.
   **`AnalyticsSupported` done** — §5.7, and it needed the same treatment for
   the same reason, which §1.3 had explicitly ruled out.
   **Imaging done** — the `GetMoveOptions` focus names, which were
   seven of the eight imaging rows and moved three kinds between them; see §1.3
   for why that is not the `AFModes` class it was filed as, and for the
   client-side half that is still open.

   The imaging slice was taken as one bucket across units 2 and 3 rather than
   by unit, because both live in `src/mock/services/imaging.rs` and share one
   perturbation run. It was perturbed in **two independent halves**: the focus
   names alone leave `ORDER` at 5, the order alone leaves the other three
   kinds unmoved, so neither half is resting on the other's evidence.
4. **Required members** (§1.4). ~~The Media2 `ConfigurationSet` inlining is five
   of the rows and is the largest change here~~; the rest are a renderer
   dropping state it already holds.

   **The inlining is done — see §5.4.** ~~Sixteen rows left, none of which is
   the `ConfigurationSet` family.~~ ~~**Eleven rows left**~~ ~~**Seven rows
   left**~~ **None left — §5.9 took the last seven, `MISSING-REQUIRED` 7 → 0.**

   *"the rest are a renderer dropping state it already holds"* held for four of
   those seven and failed for three, in three different ways: `Address` was
   state the renderer could not have held because the field did not exist,
   `EndSearch/Endpoint` is a value with no state behind it at all, and PTZ
   `Spaces` is the row where rendering *something* would have been worse than
   the omission. Reading the sentence as a plan would have produced a green
   checker and one new defect.

   §5.6 took the five device-capabilities rows, which were the next largest
   group and, unlike the `ConfigurationSet` family, were five *different* types
   rather than one decision; §5.7 took the four metadata rows, of which three
   were one renderer dropping members it could have rendered from state it
   already held — *"the rest are a renderer dropping state it already holds"*
   was right about those and wrong about the fourth, which was the options
   getter emitting a required element **empty**.
5.4 ~~**The two `GetProfiles` responses**~~ **Done. `MISSING-REQUIRED` 21 → 16,
   `ORDER` 5 → 3, `UNKNOWN-NAME` 8 → 9.**

   Taken as one bucket across units 2 and 4, on the same argument the imaging
   slice used: the seven rows live in two renderers that share one state
   snapshot, so they share one perturbation run. Perturbed in **three
   independent parts**, none resting on another's evidence:

   | put back | moves |
   |---|---|
   | Media2 token-only references | `MISSING-REQUIRED` 16 → 21, `UNKNOWN-NAME` 9 → 8, `ORDER` unchanged at 3 |
   | Media1 `tt:Profile` order | `ORDER` 3 → 4, nothing else |
   | Media2 `tr2:ConfigurationSet` order | `ORDER` 3 → 4, nothing else |

   Each failed on the pin assertion and reverted green. The first line is also
   the proof that the extra `UNKNOWN-NAME` row is caused by the inlining and by
   nothing else.

   **Nothing in the existing suite went red** when the mock's output moved —
   821 lib tests before and after — which is §4's lesson repeating: every
   assertion about a Media2 profile read a *token*, and tokens were the one
   thing the token-only shape got right. The gap was closed by asserting what
   only the inlined shape can show (`video_source_token`, on both services and
   in both unit fixtures) and by adding the state-tracking test named in §1.4.
5.5 ~~**`Profile` inside a video encoder configuration**~~ **Done.
   `UNKNOWN-NAME` 9 → 7, no other kind moved.** The fifth client-facing bug
   (§0.5): `tt:VideoEncoder2Configuration` declares `GovLength` and `Profile`
   as `xs:attribute`, and `VideoEncoderConfiguration2` read *and wrote* them as
   child elements. Fixed on both sides plus the mock, which had been written to
   agree with the parser.

   **The pin movement is the weakest evidence in this whole section, and it has
   to be said rather than left to the count.** Two rows closed and both were
   `Profile`. `GovLength` moved nothing in either direction, for two independent
   reasons — the name is a real element on `tt:H264Configuration`, so
   `UNKNOWN-NAME` cannot fire, and `tt:VideoEncoder2Configuration` has an
   `xs:any`, so `UNKNOWN-CHILD` is suppressed for the whole type. And the
   checker reads elements only: **a row disappearing proves the element is gone,
   never that the attribute replacing it is right.**

   So the verification is a test, not a number.
   `media2_encoder_gov_length_and_profile_are_attributes` in
   `tests/mock_workflow.rs` drives the client against the mock and asserts the
   values — `VEC_1` at 25/`Main`, `VEC_3` at 50/`High`, which disagree on both
   so no constant can satisfy them — then writes 90/`Baseline` through
   `set_video_encoder_configuration_media2` and reads it back. Same shape as
   `imaging_move_options_ranges_survive_the_round_trip`, and for the same reason
   §1.3 gives: **the real gap was that nothing connected the two sides.** The
   mock's four encoders all carried `gov_length: 25` until this unit, which
   would have let a token-blind renderer pass; they are 25 / 30 / 50 / 15 now.

   Perturbed in three parts, each red on an assertion and each reverted green:

   | put back | reddens, on which assertion |
   |---|---|
   | client `from_xml` element read | the workflow test — `VEC_1 GovLength`, `None` vs `Some(25)` |
   | mock `render_video_encoder` element form | the workflow test, *same* assertion and same values — it asserts agreement, so either side shows |
   | client `to_xml_body` element form | three: `set_video_encoder_configuration_media2_body_is_exact` on the body fragment, `test_video_encoder_configuration2_to_xml_body` on `GovLength="50"`, and the workflow test's read-back — `GovLength after Set`, `Some(25)` vs `Some(90)` |

   All three failed on an assertion, never on a compile error, and each reverted
   green. **The second row is the one worth having**: the first and third would
   both be caught by a client-only test, and only an agreement test reddens when
   the *mock* drifts.

   **`cargo test --all-features` did not show the third row's workflow failure.**
   Cargo fail-fasts on the first failing *target*, and the lib target runs first,
   so the two lib assertions masked the integration one. `CLAUDE.md` already says
   to run the batch unfiltered; the addition is that unfiltered is not the same
   as complete — use `--no-fail-fast`, or run the target directly, when checking
   *which* tests a perturbation reddens.

   The last row is why the mock's write path was tightened at the same time.
   `apply_video_encoder_write` used to take `extract_tag(body, "GovLength")`
   against the whole body, which accepts both the attribute form and the
   pre-0.15 element form — so a client that regressed would have round-tripped
   cleanly through the mock. It now reads the Media2 attribute or the element
   *inside* Media1's `<tt:H264>` / `<tt:H265>` block, which are the only two
   shapes either schema declares. **A lenient mock cannot be a guard.**
5.6 ~~**The `GetCapabilities` tree**~~ **Done. `MISSING-REQUIRED` 16 → 11,
   `UNKNOWN-NAME` 7 → 6, `UNKNOWN-CHILD` and `ORDER` unmoved.** Six rows in one
   renderer, `resp_capabilities` in `src/mock/services/device.rs`, taken as one
   bucket across units 3 and 4 on the same argument the imaging and
   `GetProfiles` slices used: one renderer, one state snapshot, one perturbation
   run.

   Perturbed in **two independent halves**, since the required members and the
   `UsernameToken` decision share nothing:

   | put back | moves |
   |---|---|
   | all five sets of required members | `MISSING-REQUIRED` 11 → 16, every other kind unchanged |
   | `<tt:UsernameToken>` | `UNKNOWN-NAME` 6 → 7, every other kind unchanged |

   Each failed on the pin assertion, not a compile error, and each reverted
   green.

   **Adding required members opened no new row, and that was not a given.**
   Twelve elements that had never been emitted are twelve fresh chances at a
   wrong namespace or a wrong position, and `SupportedVersions` carries two
   required children of its own (`tt:OnvifVersion` = `Major` + `Minor`). The
   thing that made it safe was reading the sequence order out of each type
   before writing, rather than appending: `TLS1.1` goes *first* in
   `tt:SecurityCapabilities` and `SAMLToken`/`KerberosToken`/`RELToken` last, so
   appending all four would have traded five `MISSING-REQUIRED` rows for an
   `ORDER` row.

   **The values are the mock's answers about itself, and three of them are
   `false` on purpose.** `ReceiverSource` is false because the mock serves no
   receiver service at all; `MetadataSearch` and
   `WSPausableSubscriptionManagerInterfaceSupport` are false because the mock
   implements neither, and both already said so in the matching
   `GetServiceCapabilities`. Every value that appears in both operations agrees
   with `resp_service_capabilities` / `resp_recording_service_capabilities` /
   `resp_search_service_capabilities` / `resp_event_service_capabilities` — the
   agreement `src/health/checks.rs` exists to police, and it now covers four
   more attributes (`TLS1.1`, `SAMLToken`, `KerberosToken`, `RELToken`) than it
   did. `MaxStringLength` is the one invented constant, and it is commented as
   such: nothing in `DeviceState` bounds a name.

   **One existing test went red, and it was the right one.**
   `the_mock_does_not_contradict_itself_between_the_two_capability_calls` in
   `src/health/mod.rs` asserts the mock states every cross-checked fact on both
   sides; dropping `<tt:UsernameToken>` made it report `1 stated only by the
   service`. That is the check correctly noticing that the pair was never real
   — see §1.3. Fixed by removing the unsound comparison, not by weakening the
   assertion: the test still demands `0 stated only by the service`, and its
   coverage floor was **raised** from `n >= 14` to the exact measured `n >= 17`,
   so losing any one capability block now fails it where a floor of 14 would
   have absorbed the loss of `<tt:Security>`.
5.7 ~~**The Media2 metadata family**~~ **Done. `MISSING-REQUIRED` 11 → 7,
   `ORDER` 3 → 1, `UNKNOWN-CHILD` 4 → 3, `UNKNOWN-NAME` 6 → 5.** Eight rows in
   two renderers in `src/mock/services/media2.rs`, taken as one bucket across
   units 2, 3 and 4 on the argument the imaging, `GetProfiles` and
   `GetCapabilities` slices used: one file, one state snapshot, one perturbation
   run.

   Perturbed in **two independent halves**, since the two renderers share only
   the state entry:

   | put back | moves |
   |---|---|
   | `render_metadata`'s order and its omitted `Multicast` / `SessionTimeout` | `MISSING-REQUIRED` 7 → 10, `ORDER` 1 → 3, nothing else |
   | the empty `<tt:PTZStatusFilterOptions/>` + `Extension/AnalyticsSupported` | `MISSING-REQUIRED` 7 → 8, `UNKNOWN-CHILD` 3 → 4, `UNKNOWN-NAME` 5 → 6 |

   Each failed on the pin assertion, not a compile error, and each reverted
   green. The split is also what shows the eight rows are 5 + 3 rather than one
   cause counted twice: the four `MISSING-REQUIRED` rows §1.4 lists together
   belong to two different renderers, three to one and one to the other.

   **Two of the eight rows were one element, and the count is again the weaker
   evidence.** `AnalyticsSupported` reported as undeclared *and* as not accepted
   by its parent; removing it moves both. What settles that removing it was
   right rather than a rename is §1.3's corrected entry — there is no element to
   rename it to — and the consequence is §0.6, a client field deleted. What
   asserts the replacement is `metadata_configs_differ_on_every_field` in
   `tests/mock_workflow.rs`, which reads both required booleans through the
   client for **both** tokens; the seeds invert them, so no constant satisfies
   it.

   **The comment that caused three of the rows is the finding worth keeping.**
   `render_metadata` said *"Multicast is genuinely optional in
   `tt:MetadataConfiguration`"* and emitted the block only for a configuration
   with an address. It is `[1]`. The comment was not a stale justification in
   the usual sense — it was never true — and it had been written to explain why
   the mock differed from the audio configurations right beside it, which do
   emit the block. `CLAUDE.md`'s *"a justification that outlives its premise is
   where to look for the next defect"* generalises: **a justification that
   explains why one renderer differs from its neighbour is worth checking even
   when it is new.**

   **No new row opened**, which is not automatic: `tt:MulticastConfiguration`
   has four required members of its own and `tt:IPAddress` a required `Type`,
   so eight elements never previously emitted here went in. Reading
   `tt:MetadataConfiguration`'s sequence first is what kept it clean — appending
   `Multicast` after `Analytics` would have been right and appending
   `SessionTimeout` after the extension members would not.

   **Media1 needed nothing, and that was checked rather than assumed.**
   `media1.wsdl` and `media2.wsdl` type their `GetMetadataConfigurationsResponse`
   `Configurations` identically (`tt:MetadataConfiguration`) and their `Options`
   identically (`tt:MetadataConfigurationOptions`), so `CLAUDE.md` step 5b would
   apply — but `dispatch_media` has no metadata arm at all and the crate has no
   Media1 metadata client method, so there is no second renderer to disagree and
   nothing for `tests/mock_media1_media2_agree.rs` to guard.

   5.9 ~~**The last fourteen rows**~~ **Done. `MISSING-REQUIRED` 7 → 0,
   `UNKNOWN-NAME` 4 → 0, `UNKNOWN-CHILD` 2 → 0, `ORDER` 1 → 0. §5 is complete.**

   Four groups in four files, each perturbed on its own; the deltas are in §1.
   Two were client bugs (§0.9, §0.10) and two more were rows where the checker
   could not have judged the fix either way.

   - **device** (`src/mock/services/device.rs`) — `ScopeAttribute` → `ScopeDef`,
     and `SystemLogUri`/`LogType` → `SystemLog`/`Type`. §4's predicted blast
     radius was exact: `src/tests/client/device_tests.rs:403` and `:407` had to
     move with the mock. The scope fixture's two entries now say `Fixed` and
     `Configurable` rather than `Fixed` twice, so the enumeration is a value an
     assertion could read rather than a constant.
   - **recording / search** (`src/mock/services/recording.rs`) — see §1.4. One
     new field on `RecordingEntry`, one `render_source` shared by both getters
     so the two views of one recording cannot disagree, an always-present
     `<tt:Tracks>`, and a real `EndSearchResponse`.
   - **PTZ and events** (`ptz.rs`, `events.rs`) — `render_spaces_body` factored
     out of `render_node` and reused, so `GetConfigurationOptions` reports the
     spaces of the node the configuration drives; the two required event
     dialects added, which sit either side of the optional
     `ProducerPropertiesFilterDialect` in `event.wsdl`'s sequence.
   - **media** (`media.rs`, `media2.rs`) — `FontSizeRange` to its declared
     position, and the whole `GetVideoEncoderInstances` subtree rebuilt:
     `tr2:Codec` wrapping `tr2:Encoding` + `tr2:Number`, then `tr2:Total`.

   **Three tests in `tests/mock_workflow.rs` assert what the counts cannot** —
   `recording_source_information_is_complete_and_per_recording`,
   `system_log_uri_is_reported` and
   `media2_encoder_instances_are_grouped_by_codec`. The first reads all five
   source members for **both** seeded recordings, which disagree on every one of
   them, and pins `Rec_002`'s empty `Address` as `None`; the other two drive the
   two client fixes end to end. Reverting either side of any of the three
   reddens it.

   5.10 **`SecurityCapabilities` — the eleventh client-facing bug, and the one
   the checker never had an opinion on. Every kind unmoved at 0.**

   This unit was opened *after* §5 was declared complete, which is itself the
   finding: §5.6 had made `<tt:Security>` conformant, so the run went to 0 with
   the client still reading `Device/Security/UsernameToken` — an element
   `tt:SecurityCapabilities` declares at no level — and modelling four of its
   twelve members. §0.11 and §1.3 carry the detail.

   - **`src/types/capabilities.rs`** — `username_token` removed (**breaking**);
     `TLS1.1`, `SAMLToken`, `KerberosToken`, `RELToken` added at the top level;
     `TLS1.0` from `Extension`; `Dot1X`, `SupportedEAPMethod` `[0..*]` and
     `RemoteUserHandling` from `Extension/Extension`. The three levels declare
     *different* members, so unlike the Media1 encoder options there is nothing
     to prefer or fall back to — each member is read at one fixed depth.
   - **`src/mock/services/device.rs`** — `<tt:Security>` gains both extension
     levels; `TLS1.1` and `Dot1X` are `true` on **both** operations, so the
     extension chain carries at least one value an absent extension cannot
     produce. `SupportedEAPMethods="13 21"` added to `tds:Security`: one
     `tt:IntList` attribute against two repeated elements, the same fact at two
     cardinalities.
   - **`src/health/checks.rs`** — seven new comparisons, 17 facts → 24. The
     device-level side finally has fields for the eleven security facts both
     types declare. `src/health/mod.rs`'s floor moves 17 → 24.
   - **`tests/mock_workflow.rs`** —
     `device_security_capabilities_include_both_extension_levels` is the
     deliverable, since the checker cannot see this class at all: it asserts all
     twelve members through a live `MockServer` and then re-asserts eleven of
     them against `GetServiceCapabilities`. Reverting the client parse reddens
     it on `TLS1.1`; reverting only the extension levels in the mock reddens it
     on `Dot1X lives in Extension/Extension`; both also redden the health
     self-consistency test on *"N stated only by the service"*.
5. ~~**Strengthen `every_response_binds_the_prefixes_it_uses`** so it asserts an
   element is in the namespace its type declares.~~ **Struck — this cannot be a
   separate unit.** Asserting that an element is in the namespace its *type*
   declares requires the schema, so this work unit *is* §6. The existing test
   keeps its weaker property, which is still worth having because it runs
   without the schema in every build; the stronger one only exists inside the
   schema-shape test.

   The consequence is an ordering change, made in §7: **the checker lands before
   the sweep, not after it.** It is not only the guard — it is the verifier for
   fixes that no other test in this repository can see.

---

## 6. The guard, and why it is not optional

Everything above was found by a tool that lives outside this repository and runs
by hand. Fixing the findings without landing the checker leaves the class
exactly as exposed as it was this morning — and this release already produced
the lesson that a number nothing asserts drifts.

**All ten pins now read 0, and that is the weakest the guard has ever been.**
A pin at 0 moves for a regression the checker can see, and this document has
recorded three whole categories it cannot. §5.11 closed the first of them — the
checker read `xs:element` and never `xs:attribute` (§0.5, §0.7, §0.8 were each
one visible row out of several members), and now reports four attribute kinds,
of which `ATTR-AS-ELEMENT` is the one that reaches those three. Two stand: a
type carrying an `xs:any` suppresses `UNKNOWN-CHILD` for the entire type (§0.5,
§0.10), and an element whose children are all optional is schema-valid empty
(§1.4's `Spaces`). A zero says *nothing the checker looks at is wrong*, which is
a much smaller claim than *the mock is conformant*. The
per-operation value assertions in `tests/mock_workflow.rs` are what carry the
rest, and they are named per unit in §5 for that reason.

The checker must land as `tests/mock_schema_shape.rs` per
[`schema-shape-plan-2026-08.md`](schema-shape-plan-2026-08.md) §3.3 — reading
the schema at run time from `$OXVIF_ONVIF_SCHEMA`, skipping loudly without it,
`#[ignore]`d, with a publishing-checklist line. Under decision D2 the checklist
is the only thing that makes it run, so the checklist line is part of the work
and not a follow-up.

---

## 7. Order of work

1. ~~**§3** — settle the Media2 configuration-set question.~~ **Done,
   `8091892`.** It was not mock-only; see §3.
2. ~~**§2** — fix the checker's own defects and establish what the 54
   unanchored roots are.~~ **Done, lab `ac74fee`.** Both gates it set were
   met: the finding list is now worth acting on, and 63 distinct is a number
   worth quoting. It also produced §1.5, a client bug that was sitting inside
   the line that said *"not a mock defect until that is settled"*.
3. ~~**§6 — land the checker, before any fix.**~~ **Done, `1e92fbc`.** It is the
   verifier as well as the guard: the client is namespace-blind and
   order-independent, so **no existing test in this repository can tell whether
   any of these fixes worked.** Sweeping first would mean hand-checking every one
   against a tool that lives in another repository, and calling it done on
   inspection. It landed with the publishing-checklist line in the same commit,
   as required.
4. ~~§5.0 → §5.1 → §5.2 → §5.3 → §5.4~~ **all done, and §5.5 through §5.9 with
   them — §5 is complete.** Each was its own commit, each verified by re-running
   the checker and each with the perturbation `CLAUDE.md` requires: put the old
   output back, and the schema-shape test must report it. §5.9 was four
   independent groups and needed four perturbations, one per group, because a
   single revert of all fourteen rows would not have shown which group owned
   which delta.

   **This list stopped at §5.4 and the sweep ran to §5.9.** The five extra units
   are not scope creep: §5.5, §5.7 and §5.8 each *became* a unit when a row this
   document had filed as mock fidelity turned out to be a client bug, and §5.6
   and §5.9 were the residue after the named groups closed. The plan's numbering
   was a plan for the rows it had triaged, not for the ones triage got wrong.
5. `CHANGELOG.md`: these are mock behaviour changes and belong in the entry,
   with §0's blast radius stated so a reader does not conclude the client was
   broken. Per the rule this release's audit produced, that is part of finishing
   the work rather than a step after it. **Done in each unit's own commit.**

   This item was numbered `6.` in a five-item list. Corrected rather than
   silently renumbered, because the gap is the sort of thing that makes a reader
   look for a missing step.
