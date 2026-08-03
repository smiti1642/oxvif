# The mock's XML does not match the schema — triage and plan

**Status** — investigated 2026-08-03, nothing fixed. Written against a working
checker and the complete ONVIF schema set, not from memory.

The checker, the schema and the raw findings live in the sibling repository
`onvif-schema-lab` (local, never pushed) because of decision D2 in
[`schema-shape-plan-2026-08.md`](schema-shape-plan-2026-08.md) §4: nothing
derived from the schema enters this repository. **This file names elements in
*oxvif's own output* and says what is wrong with them; the schema evidence for
each stays in the lab.**

---

## 0. Blast radius, established before anything else

**No finding below is a client-facing bug.** Measured, not assumed:

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

**Nothing here is a reason to delay 0.15.0.**

---

## 1. What the checker reports

155 mock responses, complete schema set (11 files, 1856 types, 1664 declared
elements, 10 namespaces), 586 response roots anchored against 54 unanchored.

```
UNKNOWN-NAME      35     element not declared in the namespace it was emitted in
UNKNOWN-CHILD     17     element not declared by its parent's type
ORDER              6     children out of the declared sequence order
MISSING-REQUIRED  50     a required member absent
                 ───
                 108
```

**108 findings is not 108 defects.** One element emitted under the wrong prefix
produces up to three of them: it is undeclared where it was put
(`UNKNOWN-NAME`), its parent does not declare it (`UNKNOWN-CHILD`), and the
element the parent *did* expect is now absent (`MISSING-REQUIRED`).
Deduplicating to root causes is step 1 of the work, and until it is done no
count in this file should be quoted as a defect count.

### 1.1 The dominant class was on nobody's list: wrong namespace

The mock emits under `tt:` a set of elements the schema declares in the service
namespace (`tds:`, `tr2:`, `trc:`, `tev:`). Confirmed instances include `Data`
and its `User`/`UserName` children, `SystemLogUri`, `LogType`,
`MaximumNumberOfProfiles`, and most of the Media2 payload — `Name`, `Profile`,
`ProfilesSupported`, `Total`, `Number`, `MaxFramerate`, `MaxResolution`,
`Encodings`, `Reboot`, plus `JobToken`/`JobConfiguration` in Recording.

**Why nothing caught it:** `every_response_binds_the_prefixes_it_uses`
(`src/mock/`) asserts that each prefix the mock emits is *declared*. It says
nothing about whether the element belongs in the namespace that prefix names.
The test is doing exactly what it says and certifying the wrong property.
Strengthening it is part of the work, not a follow-up.

### 1.2 Six sequence-order violations

`xs:sequence` order is significant. Confirmed by hand against the schema, not
only via the checker, for the first:

| response | what is out of place |
|---|---|
| Media1 `GetProfiles` → `Profile` | the video encoder configuration is emitted before the audio source configuration; the schema has them the other way round |
| Media2 `GetProfiles` → `ConfigurationSet` | same inversion, plus an `Audio` child where the schema names `AudioEncoder` |
| `GetOptions` → `ImagingOptions20` | badly scrambled — most members are in a different position |
| Media2 `GetMetadataConfigurations` | analytics emitted before PTZ status |
| `GetOSDOptions` → `OSDTextOptions` | the font-size range is emitted last, the schema places it second |

Media1 `GetProfiles` is the most-used response in the crate.

### 1.3 Undeclared element names — the `AFModes` class again

- `ScopeAttribute` in `GetScopes`. **The correct name is already in this
  repository**, in a comment 150 lines below the bug at
  `src/mock/services/device.rs:277`. The mock emits the wrong one at `:124` and
  the client unit fixture agrees with it at
  `src/tests/client/device_tests.rs:403` and `:407`. oxvif's parser reads
  neither name — it takes `ScopeItem` only — so the client is unaffected.
- Imaging `GetMoveOptions` emits space-style names under the absolute and
  continuous focus options where the schema declares a position and a speed.
  Same service and same class as the `tt:AFModes` defect fixed in 0.15.0.
- `AnalyticsSupported` under the Media2 metadata options extension, and
  `MaximumNumberOfProfiles` under the video source configuration options on both
  services.

### 1.4 Required members omitted

Fifty rows, heavily overlapping §1.1. The ones that survive a first read as
genuine omissions rather than namespace fallout: device capabilities (system,
security, events, recording, search), `Scope`, storage configuration data,
imaging focus options, the two event-service responses, PTZ configuration
options, recording job items, and recording source information — the last of
which `src/mock/state.rs` already **stores** and does not render, the same shape
as the `MTU` bug.

---

## 2. The checker's own defects, so its output is read correctly

- **`xs:choice` is treated as a sequence.** `onvif.xsd` contains two; one of
  them is the PTZ preset-tour detail, so the checker demands all three
  alternatives at once. **That row is a false positive** and any other choice
  is too.
- **One root cause is reported up to three times** (§1). No rollup.
- Anonymous types appear in messages as `#anon880_global_…`, which is
  unreadable at the point where it matters most — the event service responses.
- 54 response roots are unanchored: no `xs:element` was found for them. Cause
  not yet established; could be a WSDL version difference or a checker gap.
  **Not a mock defect until that is settled.**

---

## 3. Settle this one first — it may not be mock-only

Media2's `ConfigurationSet/VideoSource` (and its siblings) resolve to the full
`tt:VideoSourceConfiguration` type, so a token-only reference is reported as
missing every required member. But `CLAUDE.md` step 5b states as *design* that
"Media2 emits token references" where Media1 inlines whole configurations, and
`src/mock/services/media2.rs` implements exactly that.

Either the checker resolves the wrong type here, or **that design note is wrong
and a conformant Media2 device inlines the configuration** — in which case this
is not mock-only: it changes what the client must be able to parse, and
`MediaProfile2` may be modelled on a shape no device sends.

This is the only finding in the set that could reach the client. Settle it
against the Media2 WSDL before any other work, because the answer decides
whether §5 is a tidy-up or a client change.

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

1. **Namespace correctness, per service** — device, Media2, recording, events.
   Each is "render this subtree under the namespace its type declares".
2. **Sequence order** — five renderers (§1.2).
3. **Undeclared names** — `ScopeAttribute`, the imaging focus options, the two
   options extensions (§1.3).
4. **Required members** — after deduplication, and only the ones that survive
   §2 and §1.1 fallout.
5. ~~**Strengthen `every_response_binds_the_prefixes_it_uses`** so it asserts an
   element is in the namespace its type declares.~~ **Struck — this cannot be a
   separate unit.** Asserting that an element is in the namespace its *type*
   declares requires the schema, so this work unit *is* §6. The existing test
   keeps its weaker property, which is still worth having because it runs
   without the schema in every build; the stronger one only exists inside the
   schema-shape test.

   The consequence is an ordering change, made in §7: **the checker lands before
   the sweep, not after it.** It is not only the guard — it is the verifier for
   108 fixes that no other test in this repository can see.

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

1. **§3** — settle the Media2 configuration-set question. It decides the scope
   of everything else.
2. **§2** — fix the checker's `xs:choice` handling, add rollup, and establish
   what the 54 unanchored roots are. Re-run. **Only then is the finding list
   worth acting on**, and only then is a defect count worth quoting.
3. **§6 — land the checker, before any fix.** It is the verifier as well as the
   guard: the client is namespace-blind and order-independent, so **no existing
   test in this repository can tell whether any of these 108 fixes worked.**
   Sweeping first would mean hand-checking every one against a tool that lives
   in another repository, and calling it done on inspection. Land it with the
   publishing-checklist line in the same commit.
4. §5.1 → §5.2 → §5.3 → §5.4, each its own commit, each verified by re-running
   the checker and each with the perturbation `CLAUDE.md` requires: put the old
   output back, and the schema-shape test must report it.
6. `CHANGELOG.md`: these are mock behaviour changes and belong in the entry,
   with §0's blast radius stated so a reader does not conclude the client was
   broken. Per the rule this release's audit produced, that is part of finishing
   the work rather than a step after it.
