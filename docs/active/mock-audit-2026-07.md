# Mock audit — 2026-07

**Status:** **Tiers 1 and 2 closed; Tiers 3 and 4 open.** Measured at `fa1cd91`
(0.15.0 unreleased); §9 tracks what has landed since.

Every line below is **measured**, not inferred: either a probe driving the real
`OnvifClient` against a real `MockServer` over HTTP, or a mechanical read of
`src/mock/dispatch.rs`. Where a claim comes from reading code rather than
running it, it says so.

Prompted by an external report (Media2's profile family ignored `DeviceState`,
fixed in `fa1cd91`). That report named one instance; this is the sweep for the
class.

**The original measurements are left as measured** — the "before" numbers in §2
and the probe output quoted in §3 and §4 are the evidence the tiering was built
on, and rewriting them to match today would destroy the record of what was
actually wrong. Each section says what was done underneath.

### Where it stands

| | |
|---|---|
| **Defects found** | 16 — 8 Tier 1, 2 Tier 2 families, plus item 1.8 which the property test added |
| **Defects fixed** | all of them |
| **Standing guards** | `mock_roundtrip.rs` (48 pairs), `mock_token_discrimination.rs` (28 rows), `mock_media1_media2_agree.rs` (10 tests), `dispatch.rs`'s routing test (157 actions) |
| **`Expect::Broken` rows** | **0** |
| **Still open** | Tier 3's four remaining declared stubs (§5) and Tier 4's PTZ coordinate spaces (§6) — both *declared*, both asserted, neither a lie |

The structural finding in §8 is the part to read before touching `src/mock/`.
The two property tables are how "deliberately static" is now written down; the
short version is `CLAUDE.md` step 5c.

---

## 1. Method

Three axes, each answering a question a mock user actually depends on.

| Axis | Question | How measured |
|---|---|---|
| **A. state-read** | Does this getter read `DeviceState` at all? | Seed a distinctive marker straight into the state via `server.device().modify(..)`, then read through the client over HTTP and look for it. |
| **B. round-trip** | Does `Set → Get` return what was set? | Call the setter through the client, re-read through the getter. |
| **C. token-discrim** | Does an operation given two *different* tokens return two *different* answers? | Ask with two tokens the fixture deliberately disagrees on. |

Axis A is the load-bearing one: a getter that ignores state cannot be fixed by
fixing the setter, and a setter that writes state nobody reads is equally
useless. Axis B is what a user notices; A and C are why.

The classification below combines axis A with a mechanical read of every write
arm in `dispatch.rs`:

| getter reads state | setter writes state | verdict |
|---|---|---|
| yes | yes | **OK** |
| yes | **no** | **LIE** — the getter looks live, writes vanish silently |
| no | no | **STUB** — the whole family is static; at least it is consistent |

**LIE is the dangerous cell**, and it is the cell the reported bug lived in.

---

## 2. Headline numbers

Measured before the Tier 1 wiring, and left as measured — the first four lines
are the "before" picture the rest of this document reasons about.

- **53** write operations in the dispatcher. **29 touch state, 24 do not.**
  Tier 1 moved **9 dispatch arms** off the unconditional-success helper —
  `resp_empty` arms in `dispatch.rs` went from **22 to 13**.
- **19** getters confirmed state-driven by probe. **4 families have no
  `DeviceState` field at all.**
- **26 of 27** PTZ handlers never receive the request body. (**All 18 that are
  per-profile now do**; the remaining 9 are node/config-addressed or static.)
- `grep -c recording src/mock/state.rs` → **0**. (Tier 2.2 added
  `RecordingState`; all eleven Recording/Search/Replay operations now use it.)
- **47** `Set → Get` pairs became a standing test (`tests/mock_roundtrip.rs`).
  27 round-tripped, 15 were defects, 5 were declared stubs. **After Tiers 1
  and 2: 42 round-trip, 0 defects, 5 declared stubs** — `Expect::Broken` has
  no rows left. **After the Storage and metadata fixes (§5): 48 pairs, 44
  round-trip, 3 declared stubs; the token table is 28 rows, 21
  discriminating.**

  *Correction.* This line read "40 round-trip … 5 declared stubs" until the
  Storage work, which does not sum to 47 and was simply wrong; the figure was
  42. It went unnoticed because no test asserts the prose — the floor in
  `mock_roundtrip.rs` was `>= 35`, loose enough to hold under either number.
  The floors are now `PAIRS.len() >= 45` and `round_tripped >= 40`, and the
  exact split is a comment beside them where a stale number is at least next
  to the thing that could contradict it.

---

## 3. Tier 1 — LIE: state exists, the write is discarded — **fixed**

The getter is state-driven (probe-confirmed), so a caller has every reason to
believe the write landed. It did not. **This is the reported bug's class.**

**All eight are wired as of the Tier 1 commit**, and every row is now
`Expect::Works` in `tests/mock_roundtrip.rs`. The table below is kept as the
record of what was wrong and how it was found; the test is what keeps it fixed.
Perturbing each new writer back to a no-op reddens exactly the rows it fixed —
and perturbing the *shared* source-config writer reddens **both** services,
which is the property CLAUDE.md step 5b asks for.

| # | Operation | Evidence | Fix |
|---|---|---|---|
| 1.1 | `SetVideoSourceConfiguration` (Media1) | probe: set name → `"VSConfig1" -> "VSConfig1"` | mirror `apply_video_encoder_write` |
| 1.2 | `SetVideoSourceConfiguration` (Media2) | probe: `-> "VSConfig2"` unchanged | share the same writer as 1.1 |
| 1.3 | `SetDiscoveryMode` | probe: `Discoverable -> set NonDiscoverable -> Discoverable`; `get_discovery_mode` **is** state-driven | one `state.modify` |
| 1.4 | `AddVideoEncoderConfiguration` | probe: bind VEC_2 to a fresh profile → `bound encoder = None` | write `ProfileEntry.video_encoder_config_token` |
| 1.5 | `RemoveVideoEncoderConfiguration` | dispatch read: `resp_empty`; getter state-driven | clear the same field |
| 1.6 | `AddVideoSourceConfiguration` / `RemoveVideoSourceConfiguration` | dispatch read: `resp_empty`; getter state-driven | as 1.4 / 1.5 |
| 1.7 | `AddConfiguration` / `RemoveConfiguration` (Media2) | dispatch read: `resp_empty`; `GetProfiles` now state-driven | share 1.4–1.6 |
| 1.8 | `SetNetworkInterfaces` — **drops `MTU`** | `tests/mock_roundtrip.rs`: `wrote Some(1420), read back Some(1500)` | one `if let` beside the four that are already there |

**1.8 is a different shape from the rest, and was not in the first draft of this
document.** `tests/mock_roundtrip.rs` found it on its own the first time it ran.
The handler is not missing — it reads `Enabled`, `FromDHCP`, `Address` and
`PrefixLength` out of the body and writes all four. It silently drops the fifth
field, `MTU`, which the client *does* send (`src/client/device.rs:474`) and which
`GetNetworkInterfaces` *does* report. **A partial write is worse than no write**:
the state log prints `[STATE] interface updated`, four of five fields land, and
every signal a reader has says the operation is wired.

Nothing in the hand audit could have caught this. Axis A asks "does the getter
read state" (yes), the dispatch read asks "does the arm take `state`" (yes), and
`grep` for `resp_empty` never names it. Only writing a value and reading it back
distinguishes a whole write from a partial one — which is the argument for the
property test being step 1 rather than a nicety.

**Size: small.** The state field exists in every case; this was wiring. 1.1–1.2
and 1.4–1.8 were each ~10 lines plus a test.

**Why it mattered beyond tidiness:** 1.4–1.7 meant **a profile could not be
built up on the mock**. Create a profile, add an encoder, read it back — still
empty. Any test of profile-assembly logic passed without exercising anything.

### Three decisions taken while wiring it, recorded because they are omissions

Per §6: a documented omission is a design decision, an undocumented one is a bug.

- **`Bounds/@x` and `@y` are read off the wire and dropped.**
  `VideoSourceConfigEntry` models a size, not an offset, and every renderer emits
  `x="0" y="0"`. There is no field to write. Said out loud in
  `apply_video_source_write`, precisely so it never reads like item 1.8.
- **A Media2 `AddConfiguration` with an unmodelled `Type`** (`Metadata`,
  `Analytics`, `PTZ`, `AudioOutput`, `AudioDecoder`) **faults** instead of
  reporting success. `ProfileEntry` has four slots and `MediaProfile2` exposes
  exactly those four, so a success there could never be observed — it would be
  the LIE cell reintroduced by the commit that removes it. The fault names the
  type.
- **Binding to a *fixed* profile is still allowed.** Real devices refuse. All
  four seeded mock profiles are fixed, so refusing would leave only freshly
  created profiles reachable and would flip two `tests/mock_action_snapshot.rs`
  rows from `ok` to `fault`. That belongs with the fidelity work in Tier 4.

---

## 4. Tier 2 — the state model cannot express the answer

Not missing wiring. The shape of `DeviceState` has nowhere to put the truth, so
these need a (small, local) model change before any handler can be correct.

### 2.1 PTZ is profile-blind, by construction — **fixed**

```rust
pub struct PtzState {
    pan: f32, tilt: f32, zoom: f32,     // one global position
    home_pan: f32, home_tilt: f32, home_zoom: f32,
    presets: Vec<PtzPreset>,            // one preset list
    tours: Vec<PtzTour>,
}
```

Probe:

```
GAP  ptz_get_status(Profile_1 vs Profile_3)    pan Some(0.77) vs Some(0.77)
GAP  ptz_get_presets(Profile_1 vs Profile_3)   2 vs 2 presets
```

Mechanical read: **26 of 27 PTZ dispatch arms do not receive `body`** (the
exception is `SendAuxiliaryCommand`, which reads the command, not the profile).
The client sends `ProfileToken` at **20** call sites.

So this is two defects stacked: the handlers cannot see the token, and even if
they could, the state has one position and one preset list for the whole device.

**This is the exact rule `CLAUDE.md` already states** — "PTZ (per-profile)" is
the third item in the multi-sensor checklist. 0.15 applied it to Media and
Imaging and left PTZ.

**Consequence:** any test that checks "my code addresses the right profile"
passes against a mock that ignores the profile entirely. Fails green.

**Size: medium.** Key `presets` and position by profile token; thread `body`
through the PTZ dispatcher.

#### What was done

`PtzState` is now `{ channels: BTreeMap<String, PtzChannel> }`, and `PtzChannel`
holds what `PtzState` used to hold whole. **Eighteen dispatch arms gained
`body`** and go through one `require_profile`, which faults on an absent or
unknown token rather than falling back to a default head — the fallback is
precisely what made a token-blind handler indistinguishable from a correct one.

**The four seeded heads deliberately disagree**, per `CLAUDE.md`'s rule that a
single-channel fixture cannot cover a per-channel feature. They differ in
position, in preset *count* (2 / 1 / 3 / 0), in preset *names*, and in whether
they have tours at all. `Profile_4` is empty on purpose: an empty preset list is
a legitimate device state and the only fixture that catches a renderer which
substitutes a default when it finds nothing.

`Stop` writes nothing — nothing is moving in the mock — but validates the token
anyway, so a caller cannot ship code against a head this device does not have.

Tests: 8 new in `tests/mock_multi_sensor.rs` (public API, over HTTP) and 4 new
unit tests. Perturbing `require_profile` to answer for `Profile_1` regardless of
what was asked reddens **12** tests; making it fall back silently instead of
faulting reddens the **3** negatives.

**Still profile-blind and still Tier 3:** `GetConfigurations`, `GetNodes`,
`GetConfigurationOptions` and `GetCompatibleConfigurations` — those are static
fixtures on both sides (§5), not wiring gaps.

### 2.2 Recording / Search / Replay have no state at all — **fixed**

```sh
grep -c recording src/mock/state.rs   # 0
```

Probe: `CreateRecording` returns `"Rec_new"`; `GetRecordings` still returns the
same 2 static entries and does not contain it. **Identical shape to the reported
Media2 `CreateProfile` bug**, in a different service.

Seven operations are affected (`CreateRecording`, `DeleteRecording`,
`CreateTrack`, `DeleteTrack`, `CreateRecordingJob`, `SetRecordingJobMode`,
`DeleteRecordingJob`).

**Consequence:** Profile G workflows cannot be tested at all. Worse, the
health check's `with_liveness_probes(true)` "genuinely exercises Profile G"
against this — so its Profile G verdict on the mock is measuring a facade.

**Size: medium.** Needs a `recordings: Vec<RecordingEntry>` family, mirroring
`ProfilesState`.

#### What was done

`RecordingState { recordings, jobs, next_* }` modelled on `ProfilesState`, and
**all eleven** Recording/Search/Replay operations read or write it. Unknown
tokens fault instead of being answered — `NoSuchRecording-DELREC-5701` and
siblings.

Two per-token operations moved from `Blind` to `Discriminates` in
`tests/mock_token_discrimination.rs`: `GetRecordingJobState` (the job's own mode)
and `GetReplayUri` (the URI now names the recording). Five rows in
`tests/mock_roundtrip.rs` moved from `Broken` to `Works`, which leaves that table
with **no `Broken` rows at all**.

**The fixture disagrees with itself**, same rule as everywhere else: `Rec_001`
carries a track and `Rec_002` carries none; `Job_001` is `Active` and `Job_002`
is `Idle`. With one job, `GetRecordingJobState` is indistinguishable from a
constant.

Three declared simplifications, per §6:

- **No per-search cursor.** `FindRecordings` hands out one token and
  `GetRecordingSearchResults` renders the whole current list against it. A real
  device pages and expires searches.
- **A freshly created recording has no time bounds**, so `Earliest`/`Latest` are
  omitted rather than faked — both are optional in `tt:RecordingInformation`, and
  the seeded ones do carry bounds, so the distinction is observable.
- **Deleting a recording deletes its jobs.** A job pointing at nothing is not a
  state a device would report.

Found while wiring it: `tests/mock_workflow.rs` was calling
`get_replay_uri("rec1", …)` — a token matching **nothing**, which the blind
handler answered anyway. The same shape as the `VideoSource_1` token reconciled
in 0.15, and it only surfaced because the handler started checking.

---

## 5. Tier 3 — consistent stubs

Getter *and* setter are static. The round-trip is broken, but nothing pretends
otherwise, and `Get` never claims to reflect a write. Lower priority — but each
is a family a user might reasonably expect to work.

| Family | Probe |
|---|---|
| Audio (Media1 + Media2): sources, source configs, encoder configs + options, `SetAudioEncoderConfiguration` | `get_audio_sources` — no `DeviceState` field; 1 static |
| PTZ configurations / nodes / options, `SetConfiguration` | `ptz_get_configurations` — no field; 1 static |
| ~~Storage configurations, `SetStorageConfiguration`~~ **fixed** | `get_storage_configurations` — no field; 1 static |
| ~~Media2 metadata configurations, `SetMetadataConfiguration`~~ **fixed** | dispatch read: static both sides |
| ~~Media2 `SetVideoSourceMode`~~ **fixed — now faults** | dispatch read: static |

### What was done — Storage

`DeviceState` gained `storage: Vec<StorageEntry>` with the exact field set
`crate::types::StorageConfiguration` parses, so every field the client can read
is one the mock can store. `GetStorageConfigurations` renders it;
`SetStorageConfiguration` creates when the token attribute is absent, updates
in place when it names an entry, and **faults when it names one that does not
exist** (`ter:InvalidArgVal` / `NoSuchStorage-STOR-5802`) — a device that
silently created an entry under an invented token would make a typo
indistinguishable from a successful update. An empty `Data/@type` is refused
too, with a different code (`env:Sender` / `NoStorageType-STOR-5801`), so
asserting both proves more than asserting either.

Three seeded entries **disagree on every optional field independently** —
`SD_01` (path, no URI, no credentials), `NAS_01` (everything populated),
`CIFS_01` (URI only). One entry would let a renderer that hard-codes
`LocalPath` and omits the rest pass just as well as a correct one.

This closes Tier 4's storage-credential item at the same time (see §6).

### What was done — Media2 metadata

`DeviceState` gained `metadata: Vec<MetadataEntry>`. All **three** operations
became state-driven, not just the two the row named:

- `GetMetadataConfigurations` renders state and honours the optional
  `ConfigurationToken`. It is a **filter**, so a token matching nothing gives
  an empty list rather than a fault — and the test asserts that difference,
  because the other two operations *do* fault.
- `GetMetadataConfigurationOptions` answers for the addressed configuration.
  Leaving it static would have reproduced the multi-sensor failure exactly: a
  live configurations getter beside an options getter answering for whichever
  configuration the fixture happened to describe. It also now emits
  `Options/Extension/AnalyticsSupported`, which
  `MetadataConfigurationOptions::from_xml` reads and the old fixture omitted
  entirely — so every caller saw `analytics_supported: false` no matter what.
  That is the `AFModes` class, found while wiring rather than by the §6 diff.
- `SetMetadataConfiguration` updates in place and faults on an unknown token.
  It writes `name` **and all three filter booleans**; a name-only write is the
  `MTU` shape from §5c, and the round-trip row inverts all three so a partial
  write cannot pass.

`analytics_supported` is deliberately not writable — it is a device capability
reported by the options getter, not part of `tt:MetadataConfiguration`, and the
client never sends it.

The two seeded configurations invert every boolean and disagree on multicast.
**Unlike Storage, the `Option` distinction here is real:**
`multicast_address` / `multicast_port` are `Option` on the parser, so
`MetaConf_2` omitting the block is observable from a client — measured, a
renderer that always emits multicast reddens the workflow test.

**A claim withdrawn.** The first draft of the renderer carried a comment saying
that omitting an empty element keeps "the device did not say" distinguishable
from "the device said empty". **It does not, and the mutation proved it:**
`StorageConfiguration` models these as `String` via `unwrap_or_default()`, so a
renderer changed to emit `<tt:LocalPath></tt:LocalPath>` reddens nothing. The
wire shape is still what a real device sends, but it is not a tested guarantee
and the comment now says so. Making it one means `Option<String>` on the
parser — a public API change, deliberately not folded into this commit.

---

## 6. Tier 4 — fidelity gaps (the `AFModes` class)

A parser field the mock never feeds. Neither side is wrong in isolation; they
are simply never compared, which is exactly how the `AFModes` misspelling
survived until 0.15.

Method: extracted the 317 element names the parsers in `src/types/` look for and
the 384 the mock emits, then diffed. 51 raw candidates; most are false positives
(attributes rather than elements, tags rendered from a variable, prefixes my
extractor missed). **Verified real:**

- **PTZ coordinate spaces and limits** — `PanTiltLimits`, `ZoomLimits`, and all
  eight `*PositionSpace` / `*VelocitySpace` URIs are never emitted. Those
  `PtzConfiguration` fields are `None` from the mock forever; their only
  exercise is a hand-written unit fixture.
- ~~**Storage credential fields** — `resp_storage_configurations()` is static
  and omits them.~~ **Fixed** with the Tier 3 Storage work above:
  `StorageUri` and `User/UserName` are now rendered from state, and the
  round-trip row asserts all four writable fields rather than `local_path`
  alone. Asserting only the path would have left this unproved, since the old
  static fixture already carried a `LocalPath`.

**Explicitly not a defect:** Media1 encoder options omit `H265`. That omission is
deliberate and carries a comment saying why (`services/media.rs:669` — it lives
at `Options/Extension/Extension/H265` and adding it changes what every caller
sees). Recorded here as the contrast: **a documented omission is a design
decision; an undocumented one is a bug.**

---

## 7. Confirmed sound

Worth stating, so this reads as an audit and not a complaint.

- **The state layer.** `DeviceState` is 25 flat serde fields; `MockState` is a
  lock with `read` / `modify` / `modify_returning` / `set_on_change`. Seeding,
  persistence and snapshot/restore all fall out of it. **Not one defect in this
  document is caused by the state model** — Tier 1's fields all already exist.
- **19 getters confirmed state-driven end to end**, including everything in
  Device, Network, Users, Media/Media2 sources + encoders, Imaging and OSD.
- **Token discrimination works** where it was implemented: imaging per
  `VideoSourceToken`, encoder options per `ConfigurationToken`, OSD per
  `ConfigurationToken` (`1 vs 0 osds`).
- **29 of 53 writes do persist**, including the whole Device/Network/User
  surface, OSD, PTZ presets and tours, and (as of `fa1cd91`) both services'
  profile and encoder families.

---

## 8. The structural finding

Every Tier 1 and Tier 2 defect has one root cause:

> **Nothing distinguishes "deliberately static" from "not wired up yet"** — not
> the type system, not the dispatch table, not the tests.

```rust
pub fn resp_profiles_media2() -> String   // was a bug
pub fn resp_audio_sources() -> String     // a perfectly fine stub
```

Handler arity is free-form, so `()`, `(state)` and `(state, body)` all compile
and the dispatch table records no intent. Telling the two apart requires reading
`DeviceState` and guessing at history — which is why five instances have now been
found from *outside* the project rather than by review.

**The fix is not a rewrite.** The state layer is sound and the invariant is
checkable from outside, which is where every one of these was found anyway. Three
table-driven property tests over the public API kill the class:

1. **Round-trip** — every `(Get, Set)` pair: set a distinctive value, get it back.
   **Landed as `tests/mock_roundtrip.rs`** (48 pairs). Each row declares
   `Works` / `Broken(audit §)` / `Static(audit §)`, and **all three arms are
   asserted** — wire a `Broken` row up and the test goes red telling you to move
   it, so the list cannot rot into the permanent blind spot an xfail list usually
   becomes. This is where "deliberately static" is finally written down. It
   confirmed 46 of the 47 classifications in this document on its first run and
   found the 47th (item 1.8).
2. **Token discrimination** — every token-taking operation: two tokens the
   fixture disagrees on must produce different answers. **Landed as
   `tests/mock_token_discrimination.rs`** (26 rows). Same contract as the
   round-trip table — `Discriminates` or `Blind(audit §)`, **both arms
   asserted** — over the public API and real HTTP. **19 discriminate, 7 are
   declared static** (17/9 when it landed; the recording pair moved after §4.2).
   It catches a bug the round-trip table cannot: a handler can persist state
   perfectly and still answer for the wrong channel.
3. **Cross-service agreement** — already landed as
   `tests/mock_media1_media2_agree.rs`.

Plus the non-test half: **make fixtures discriminating by construction**, as the
0.15 two-sensor and two-lens work did. A blind handler cannot accidentally be
right against a fixture whose channels disagree.

**Explicitly rejected:** rewriting to a schema-driven or typed-XML mock. The
ONVIF schema is enormous; it would trade this bug class for an endless
schema-fidelity chase and stall everything else.

---

## 9. Suggested order

| Step | Why first |
|---|---|
| ~~1. Property test **round-trip** (§8.1)~~ **done** — `tests/mock_roundtrip.rs` | It was both probe and guard. Produced the Tier 1 list mechanically, confirmed 46 of 47 hand classifications, and added item 1.8. |
| ~~2. Tier 1 wiring (§3)~~ **done** — all 8 items | Eight rows moved from `Broken` to `Works`; **35 of 47 pairs now round-trip**. Three new cross-service tests in `tests/mock_media1_media2_agree.rs` cover the bindings and the shared source-config writer. |
| ~~3. Tier 2.1 PTZ per-profile (§4.1)~~ **done** | `PtzState` is keyed by profile token; 18 dispatch arms gained `body`; the four seeded heads deliberately disagree. |
| ~~4. Property test **token discrimination** (§8.2)~~ **done** — `tests/mock_token_discrimination.rs` | 26 rows; 19 discriminate, 7 declared static. Guards step 3 and the 0.15 media/imaging work. |
| ~~5. Tier 2.2 recording state (§4.2)~~ **done** | `RecordingState` mirrors `ProfilesState`; all eleven operations wired. Unblocks Profile G testing, including the health check's own liveness chain. |
| 6. Tiers 3 and 4 | **In progress.** The decision to close them before shipping 0.15.0 was taken 2026-07-31. Storage is done; the rest are below. Until each lands it stays *declared* — every remaining Tier 3 family has a `Static` or `Blind` row asserting it is still a stub, so none can quietly become a lie. Fix one and the tables tell you to move the row. |

### What a Tier 3 fix costs

Originally recorded as "not recommended now", so the decision would not be
re-derived. That decision was **reversed on 2026-07-31**: 0.15.0 waits for
Tier 3 and Tier 4. The estimates stand; the recommendation does not.

| Family | Work | Status |
|---|---|---|
| Audio (both services) | A catalogue in `DeviceState` plus per-token getters — the same shape as `video_encoders`. The largest of the four. | open |
| PTZ configurations / nodes | A `PtzConfigEntry` list; would also close Tier 4's coordinate-space gap, since those fields live on `PtzConfiguration`. | open |
| Storage configurations | One `Vec<StorageEntry>`; smallest. | **done** — and it closed §6's storage item, as predicted |
| Media2 metadata configurations | One `Vec<MetadataEntry>`. | **done** — cost held, but it was three operations, not two |

The Storage estimate held: one `Vec<StorageEntry>`, one renderer, one write
handler. What it did *not* predict was the create-vs-update split in
`SetStorageConfiguration` (a token-less call means "create") — that needed a
second round-trip row, since one row cannot cover both paths.

`SetVideoSourceMode` and `SetRelayOutputState` are **not** on this list: no getter
exposes the value either writes, so there is no round-trip question to ask. See
`CLAUDE.md` step 5c.

**Correction (2026-07-31).** Grouping those two together was wrong, and acting
on step 5c's other half exposed it. Only `SetVideoSourceMode` was reporting a
success it could not back — it stored nothing, `GetVideoSourceModes` is static,
and `oxvif::VideoSourceMode` has no active-mode field, so **nothing in this
crate could ever have contradicted the claim**. It now faults
(`ter:ActionNotSupported` / `NotModelled-VSMODE-5813`).

`SetRelayOutputState` is a different case and needed no change: it validates the
token, writes `RelayOutputState::logical_state`, and emits an event. It is
observable — just not through `GetRelayOutputs`, which by spec does not return
live state. What the two shared was "no ONVIF getter", which turns out not to be
the property that matters. The property that matters is **whether any observable
consequence exists at all** — and the second write has one.
