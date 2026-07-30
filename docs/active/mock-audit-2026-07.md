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

- **53** write operations in the dispatcher. **29 touch state, 24 do not.**
- **19** getters confirmed state-driven by probe. **4 families have no
  `DeviceState` field at all.**
- **26 of 27** PTZ handlers never receive the request body.
- `grep -c recording src/mock/state.rs` → **0**.

---

## 3. Tier 1 — LIE: state exists, the write is discarded

The getter is state-driven (probe-confirmed), so a caller has every reason to
believe the write landed. It did not. **This is the reported bug's class.**

| # | Operation | Evidence | Fix |
|---|---|---|---|
| 1.1 | `SetVideoSourceConfiguration` (Media1) | probe: set name → `"VSConfig1" -> "VSConfig1"` | mirror `apply_video_encoder_write` |
| 1.2 | `SetVideoSourceConfiguration` (Media2) | probe: `-> "VSConfig2"` unchanged | share the same writer as 1.1 |
| 1.3 | `SetDiscoveryMode` | probe: `Discoverable -> set NonDiscoverable -> Discoverable`; `get_discovery_mode` **is** state-driven | one `state.modify` |
| 1.4 | `AddVideoEncoderConfiguration` | probe: bind VEC_2 to a fresh profile → `bound encoder = None` | write `ProfileEntry.video_encoder_config_token` |
| 1.5 | `RemoveVideoEncoderConfiguration` | dispatch read: `resp_empty`; getter state-driven | clear the same field |
| 1.6 | `AddVideoSourceConfiguration` / `RemoveVideoSourceConfiguration` | dispatch read: `resp_empty`; getter state-driven | as 1.4 / 1.5 |
| 1.7 | `AddConfiguration` / `RemoveConfiguration` (Media2) | dispatch read: `resp_empty`; `GetProfiles` now state-driven | share 1.4–1.6 |

**Size: small.** The state field exists in every case; this is wiring. 1.1–1.2
and 1.4–1.7 are each ~10 lines plus a test.

**Why it matters beyond tidiness:** 1.4–1.7 mean **a profile cannot be built up
on the mock**. Create a profile, add an encoder, read it back — still empty. Any
test of profile-assembly logic passes without exercising anything.

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
   *This axis found Tier 1 on its own; as a standing test it also guards it.*
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
| 1. Property test **round-trip** (§8.1) | It is both probe and guard. Produces the authoritative Tier 1 list mechanically instead of by hand, and reddens today. |
| 2. Tier 1 wiring (§3) | Small, the fields exist, and step 1 turns green as they land. |
| 3. Tier 2.1 PTZ per-profile (§4.1) | Biggest source of green-when-wrong, and closes a rule we already wrote and left half-applied. |
| 4. Property test **token discrimination** (§8.2) | Guards step 3 and the 0.15 media/imaging work. |
| 5. Tier 2.2 recording state (§4.2) | Unblocks Profile G testing, including the health check's own liveness probe. |
| 6. Tiers 3 and 4 | Opportunistic. |
