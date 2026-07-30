# Mock audit — 2026-07

**Status:** open. Measured at `fa1cd91` (0.15.0 unreleased).

Every line below is **measured**, not inferred: either a probe driving the real
`OnvifClient` against a real `MockServer` over HTTP, or a mechanical read of
`src/mock/dispatch.rs`. Where a claim comes from reading code rather than
running it, it says so.

Prompted by an external report (Media2's profile family ignored `DeviceState`,
fixed in `fa1cd91`). That report named one instance; this is the sweep for the
class.

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
- **26 of 27** PTZ handlers never receive the request body.
- `grep -c recording src/mock/state.rs` → **0**.
- **47** `Set → Get` pairs are now a standing test
  (`tests/mock_roundtrip.rs`). 27 round-tripped, 15 were defects, 5 were
  declared stubs. **After Tier 1: 35 round-trip, 7 defects (all Tier 2), 5
  declared stubs.**

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

### 2.1 PTZ is profile-blind, by construction

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

### 2.2 Recording / Search / Replay have no state at all

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

---

## 5. Tier 3 — consistent stubs

Getter *and* setter are static. The round-trip is broken, but nothing pretends
otherwise, and `Get` never claims to reflect a write. Lower priority — but each
is a family a user might reasonably expect to work.

| Family | Probe |
|---|---|
| Audio (Media1 + Media2): sources, source configs, encoder configs + options, `SetAudioEncoderConfiguration` | `get_audio_sources` — no `DeviceState` field; 1 static |
| PTZ configurations / nodes / options, `SetConfiguration` | `ptz_get_configurations` — no field; 1 static |
| Storage configurations, `SetStorageConfiguration` | `get_storage_configurations` — no field; 1 static |
| Media2 metadata configurations, `SetMetadataConfiguration` | dispatch read: static both sides |
| Media2 `SetVideoSourceMode` | dispatch read: static |

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
- **Storage credential fields** — `resp_storage_configurations()` is static and
  omits them.

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
   **Landed as `tests/mock_roundtrip.rs`** (47 pairs). Each row declares
   `Works` / `Broken(audit §)` / `Static(audit §)`, and **all three arms are
   asserted** — wire a `Broken` row up and the test goes red telling you to move
   it, so the list cannot rot into the permanent blind spot an xfail list usually
   becomes. This is where "deliberately static" is finally written down. It
   confirmed 46 of the 47 classifications in this document on its first run and
   found the 47th (item 1.8).
2. **Token discrimination** — every token-taking operation: two tokens the
   fixture disagrees on must produce different answers.
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
| 3. Tier 2.1 PTZ per-profile (§4.1) | Biggest source of green-when-wrong, and closes a rule we already wrote and left half-applied. |
| 4. Property test **token discrimination** (§8.2) | Guards step 3 and the 0.15 media/imaging work. |
| 5. Tier 2.2 recording state (§4.2) | Unblocks Profile G testing, including the health check's own liveness probe. |
| 6. Tiers 3 and 4 | Opportunistic. |
