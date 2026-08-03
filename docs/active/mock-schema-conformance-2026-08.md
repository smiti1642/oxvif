# The mock's XML does not match the schema — triage and plan

**Status** — investigated 2026-08-03. §3 fixed in `8091892`; the checker's own
defects fixed and the corpus re-dumped at that commit, so **the numbers below
are run 3's and run 2's 108 is superseded**. Nothing in the sweep is fixed yet.
Written against a working checker and the complete ONVIF schema set, not from
memory.

The checker, the schema and the raw findings live in the sibling repository
`onvif-schema-lab` (local, never pushed) because of decision D2 in
[`schema-shape-plan-2026-08.md`](schema-shape-plan-2026-08.md) §4: nothing
derived from the schema enters this repository. **This file names elements in
*oxvif's own output* and says what is wrong with them; the schema evidence for
each stays in the lab.**

---

## 0. Blast radius, established before anything else

**Two of the findings are client-facing bugs. The rest are mock fidelity.**

This section originally read *"no finding below is a client-facing bug"*, and
that was wrong twice over — kept here rather than deleted, because both
exceptions were found by looking at the two things the sentence dismissed:

1. **Media2's audio encoder element name** — §3, fixed in `8091892`.
   `MediaProfile2::audio_encoder_token` had been `None` from every conformant
   device. It was flagged in §3 as "the only finding that could reach the
   client"; it did.
2. **`GetDigitalInputs` is sent to the wrong service** — §1.5, *not* fixed. It
   was hiding inside "54 unanchored roots, cause not yet established", which
   §2 explicitly declined to call a defect. Establishing the cause was what
   turned one of them into a defect.

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

**Nothing here is a reason to delay 0.15.0** — except §1.5, which is a
one-line client fix and should ride along rather than wait for the sweep.

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
| Media1 `GetProfiles` → `Profile` | the video encoder configuration is emitted before the audio source configuration; the schema has them the other way round |
| Media2 `GetProfiles` → `ConfigurationSet` | the same inversion |
| `GetOptions` → `ImagingOptions20` | badly scrambled — most members are in a different position |
| Media2 `GetMetadataConfigurations` | analytics emitted before PTZ status (**two rows**, one per configuration in the response — one cause) |
| `GetOSDOptions` → `OSDTextOptions` | the font-size range is emitted last, the schema places it second |

Media1 `GetProfiles` is the most-used response in the crate.

The Media2 row used to read *"plus an `Audio` child where the schema names
`AudioEncoder`"*. That part is fixed (`8091892`); the inversion is not.

### 1.3 Undeclared element names — the `AFModes` class again

12 `UNKNOWN-NAME` rows; the six that are not §1.5 or a duplicate of a
`UNKNOWN-CHILD` row below:

- `ScopeAttribute` in `GetScopes`. **The correct name is already in this
  repository**, in a comment 150 lines below the bug at
  `src/mock/services/device.rs:277`. The mock emits the wrong one at `:124` and
  the client unit fixture agrees with it at
  `src/tests/client/device_tests.rs:403` and `:407`. oxvif's parser reads
  neither name — it takes `ScopeItem` only — so the client is unaffected.
- Imaging `GetMoveOptions` emits space-style names (`PositionSpace`,
  `SpeedSpace`) under the absolute and continuous focus options where the
  schema declares a position and a speed. Same service and same class as the
  `tt:AFModes` defect fixed in 0.15.0.
- `AnalyticsSupported` under the Media2 metadata options extension;
  `MaximumNumberOfProfiles` under the video source configuration options;
  `ProfilesSupported` under the video encoder configuration options; `Profile`
  inside a video encoder configuration; `Number` under an encoder instance;
  `UsernameToken` under the device security capabilities; `SystemLogUri` and
  its `LogType` in `GetSystemUris`.

`UsernameToken` is worth a second look rather than a rename: the name plausibly
belongs to the device *service* capabilities type, which is a different type in
`devicemgmt.wsdl`. If so the mock is mixing the two types, and
`src/health/checks.rs` cross-checks that attribute as one of its eighteen
twice-stated ones, so it has an opinion either way.

### 1.4 Required members omitted

23 rows. After §1.1 fallout is discounted, the genuine omissions are: device
capabilities (system, security, events, recording, search), `Scope`'s
`ScopeDef`, `GetRecordings` → `Tracks`, `EndSearch` → `Endpoint`, PTZ
configuration options → `Spaces`, the metadata configuration's `Multicast` /
`SessionTimeout` and multicast `AutoStart` / `TTL`, the metadata PTZ-status
filter options, imaging focus position and speed, the event-properties
response, and recording source information — the last of which
`src/mock/state.rs` already **stores** and does not render, the same shape as
the `MTU` bug.

Five of the 23 are the Media2 `ConfigurationSet` members
(`VideoSource`, `AudioSource`, `VideoEncoder`, `AudioEncoder`, `PTZ`), which
are **one** decision: `8091892` established that a conformant device inlines
the full configuration, and this mock renders a token-only reference. Fixing
that closes five rows at once and is the largest single change in the sweep.

### 1.5 `GetDigitalInputs` is sent to the wrong service — a client bug

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

Still open, deliberately: the mock renders a token-only reference rather than
inlining. That is now a documented simplification rather than a claim about the
schema, and it is the five `MISSING-REQUIRED` rows of §1.4 — the largest single
change in the sweep.

---

## 4. What will break when the mock's output moves

- **167 byte-level `contains("<prefix:…")` assertions** across eight files.
  **68 of them assert on mock output** — 62 in `src/mock/state.rs`, 6 in
  `src/mock/dispatch.rs` — and every one that names a prefix that moves will go
  red. That is the intended signal, not collateral: those assertions exist
  because the client parser cannot see prefixes, so they are the only thing that
  can.
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

0. **`GetDigitalInputs` → the DeviceIO service** (§1.5). A client fix, not a
   mock one, and the only unit that changes what a caller sees. Needs the
   DeviceIO endpoint resolved from `GetServices`, so it is not a string swap.
   Independent of everything else; do it first or in parallel.
1. **Namespace correctness, per service** — device, Media2, recording, events.
   "Render this subtree under the namespace its type declares" — and **read
   §1.1's table before writing any of it**, because three of the sixteen rows
   move the *other* way and a uniform sweep would break them.
2. **Sequence order** — five renderers, six rows (§1.2).
3. **Undeclared names** — `ScopeAttribute`, the imaging focus options, the
   options extensions (§1.3). Settle `UsernameToken` by type rather than
   renaming it.
4. **Required members** (§1.4). The Media2 `ConfigurationSet` inlining is five
   of the rows and is the largest change here; the rest are a renderer dropping
   state it already holds.
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
3. **§6 — land the checker, before any fix.** It is the verifier as well as the
   guard: the client is namespace-blind and order-independent, so **no existing
   test in this repository can tell whether any of these fixes worked.**
   Sweeping first would mean hand-checking every one against a tool that lives
   in another repository, and calling it done on inspection. Land it with the
   publishing-checklist line in the same commit.
4. §5.0 → §5.1 → §5.2 → §5.3 → §5.4, each its own commit, each verified by
   re-running the checker (**re-dump the corpus first**) and each with the
   perturbation `CLAUDE.md` requires: put the old output back, and the
   schema-shape test must report it.
6. `CHANGELOG.md`: these are mock behaviour changes and belong in the entry,
   with §0's blast radius stated so a reader does not conclude the client was
   broken. Per the rule this release's audit produced, that is part of finishing
   the work rather than a step after it.
