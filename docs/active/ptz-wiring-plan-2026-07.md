# PTZ configurations / nodes / spaces — implementation plan

**Status:** done. Written 2026-07-31, all three stages landed 2026-08-03
(`19250ec`, `1769850`, and the stage-2 commit). §3.5 and §6.4 record what the
plan did not anticipate; the rest stood.
**Blocks:** 0.15.0 (decision 2026-07-31 — the release waits for the mock).
**Closes:** [`mock-audit-2026-07.md`](mock-audit-2026-07.md) §5 Tier 3 (PTZ family)
and §6 Tier 4 (coordinate spaces and limits) — the last Tier 4 item after
Storage closed the credential one.

Every fact below was re-read from the source named beside it, or from the
official ONVIF schema downloaded during planning. **Nothing here is from
memory** — an earlier draft of this work carried three wrong recollections
(seeded hostname, `DeviceState` field count, round-trip tally), which is why
each claim carries its citation.

---

## 1. Decisions taken

| # | Question | Answer | Who |
|---|---|---|---|
| 1 | Key `PtzState` by node instead of profile, accepting the churn? | **Yes.** The mock's positioning is a model ONVIF IP camera; physical truth wins over fixture convenience. | user, 2026-07-31 |
| 2 | Is `to_xml_body` omitting the limits an oxvif bug, not a mock one? | **Yes** — and the spelling defect found alongside it is worse. §3.1. | verified |
| 3 | Make `GetConfigurationOptions` per-configuration | **Yes** | user |
| 4 | Emit `<tt:PTZConfiguration>` inside profiles | **Yes** | user |
| 5 | Move the table rows and re-pin the counts | **Yes** | user |
| A | Invert the `mock_multi_sensor.rs` PTZ premise (main + sub stream of one lens are **one** head) | **Yes** | user |
| B | A profile with no PTZ configuration → PTZ operations **fault** | **Yes**, the ONVIF-correct answer | user |
| C | A zoom-only node → reject `AbsoluteMove` carrying pan/tilt | **Yes** | user |

---

## 2. Verified starting state

### 2.1 What is static today

Seven of the 27 PTZ operations are fixtures whose handlers do not even receive
the request body — they are the only PTZ arms in
[`dispatch.rs:210-216`](../../src/mock/dispatch.rs) without a `body` argument:

| Operation | Handler | Defect |
|---|---|---|
| `GetNodes` | `resp_ptz_nodes()` | one node, `<tt:SupportedPTZSpaces/>` **empty** |
| `GetNode` | `resp_ptz_node()` | ignores `NodeToken`; same single node |
| `GetConfigurations` | `resp_ptz_configurations()` | one configuration |
| `GetConfiguration` | `resp_ptz_configuration()` | ignores `PTZConfigurationToken` |
| `GetCompatibleConfigurations` | `resp_ptz_compatible_configurations()` | ignores `ProfileToken` |
| `GetConfigurationOptions` | `resp_ptz_configuration_options()` | ignores the token; `PTZTimeout` only |
| `SetConfiguration` | `resp_empty("tptz", …)` | body discarded entirely |

### 2.2 Parser fields nothing feeds

`PtzConfiguration` ([`types/ptz_config.rs:73`](../../src/types/ptz_config.rs))
has 15 fields. The mock emits five (`token`, `Name`, `UseCount`, `NodeToken`,
`DefaultPTZTimeout`). Never emitted:

- the six `Default*Space` URIs
- `DefaultPTZSpeed`
- `PanTiltLimits`, `ZoomLimits`

`PtzNode` ([`types/ptz_config.rs:243`](../../src/types/ptz_config.rs)) — the
mock sends `<tt:SupportedPTZSpaces/>`, an empty element, so `pan_tilt_spaces`
and `zoom_spaces` are always empty `Vec`s, and `aux_commands` too. **The audit's
§6 listed the configuration gap and missed the node one.**

`MediaProfile::ptz_config_token`
([`types/media.rs:63`](../../src/types/media.rs)) and
`MediaProfile2::ptz_config_token` ([`types/media.rs:218`](../../src/types/media.rs))
are both parsed, but neither `media::render_profile`
([`services/media.rs:526`](../../src/mock/services/media.rs)) nor
`media2::render_profile_media2` ([`services/media2.rs:27`](../../src/mock/services/media2.rs))
emits a PTZ element, and `ProfileEntry`
([`state.rs:546`](../../src/mock/state.rs)) has no slot for one. **Not in the
audit at all** — found while planning this.

### 2.3 Official schema facts

From `onvif.xsd` (ONVIF, © 2008-2025, 422 KB, fetched from
`https://www.onvif.org/ver10/schema/onvif.xsd` during planning).

**`PTZConfiguration`** extends `tt:ConfigurationEntity` (which contributes
`Name`, `UseCount`, and the required `token` attribute). Its own sequence, in
order:

```
NodeToken                                required
DefaultAbsolutePantTiltPositionSpace     minOccurs=0   ← note the spelling
DefaultAbsoluteZoomPositionSpace         minOccurs=0
DefaultRelativePanTiltTranslationSpace   minOccurs=0
DefaultRelativeZoomTranslationSpace      minOccurs=0
DefaultContinuousPanTiltVelocitySpace    minOccurs=0
DefaultContinuousZoomVelocitySpace       minOccurs=0
DefaultPTZSpeed        tt:PTZSpeed       minOccurs=0
DefaultPTZTimeout      xs:duration       minOccurs=0
PanTiltLimits          tt:PanTiltLimits  minOccurs=0
ZoomLimits             tt:ZoomLimits     minOccurs=0
Extension                                minOccurs=0
```

Attributes: `MoveRamp`, `PresetRamp`, `PresetTourRamp`, all `xs:int`, all
optional. **oxvif parses none of the three.**

```
PanTiltLimits → Range : tt:Space2DDescription  = URI + XRange + YRange  (YRange required)
ZoomLimits    → Range : tt:Space1DDescription  = URI + XRange           (no YRange)
```

**`PTZNode`** extends `tt:DeviceEntity`. Sequence: `Name` (minOccurs=0),
`SupportedPTZSpaces` (**required**, type `tt:PTZSpaces`),
`MaximumNumberOfPresets`, `HomeSupported`, `AuxiliaryCommands`
(0..unbounded), `Extension`. Attributes: `FixedHomePosition`, **`GeoMove`** —
oxvif parses `FixedHomePosition` and not `GeoMove`.

**`tt:PTZSpaces`** has eight slots, each `minOccurs=0 maxOccurs=unbounded`:
`AbsolutePanTiltPositionSpace`, `AbsoluteZoomPositionSpace`,
`RelativePanTiltTranslationSpace`, `RelativeZoomTranslationSpace`,
`ContinuousPanTiltVelocitySpace`, `ContinuousZoomVelocitySpace`,
`PanTiltSpeedSpace`, `ZoomSpeedSpace`.

So the mock's `<tt:SupportedPTZSpaces/>` is *schema-valid* (all eight children
are optional) while asserting the node supports no space at all — which
contradicts the same node's `HomeSupported=true` and the mock accepting
`AbsoluteMove`. That self-contradiction is pre-existing, not introduced here.

`ptz.wsdl` (fetched from `https://www.onvif.org/ver20/ptz/wsdl/ptz.wsdl`, 60.6 KB)
declares **no `ter:` fault codes for any operation** — ONVIF keeps the fault
table in the specification prose, not the WSDL. So decision C cannot cite an
authoritative code; see §5.3 for what is used instead and why.

---

## 3. Stage 0 — the oxvif bug (independent of the mock)

**This stage does not touch `src/mock/`. It can be done first and alone,** and
it must come *before* the mock starts emitting spaces, or the mock and the
parser will disagree.

### 3.1 The spelling

The schema says `DefaultAbsolutePantTiltPositionSpace` — `Pant`, double t. It is
an ONVIF typo, and it is normative. **Only this one of the six is affected; oxvif
spells the other five correctly.**

| | current | consequence |
|---|---|---|
| read | `xml_str(node, "DefaultAbsolutePanTiltPositionSpace")` ([`ptz_config.rs:117`](../../src/types/ptz_config.rs)) | `default_abs_pan_tilt_space` is `None` from **every conformant device** |
| write | same spelling ([`ptz_config.rs:155`](../../src/types/ptz_config.rs)) | `SetConfiguration` carries an element no schema defines: strict devices reject the request, lenient ones drop the field |

It survived because the parser, the unit fixture
([`ptz_tests.rs:508`](../../src/tests/client/ptz_tests.rs)) and the mock all
agree with each other and with nothing else — the third instance of the shape
`CLAUDE.md` describes under "Data nested in `Extension` levels".

**Fix: read lenient, write strict.**

- `from_xml`: try `DefaultAbsolutePantTiltPositionSpace` first, fall back to
  `DefaultAbsolutePanTiltPositionSpace`. The fallback is one line and covers
  vendors who "corrected" ONVIF's typo.
- `to_xml_body`: emit the schema spelling only.

### 3.2 The missing limits

`to_xml_body` ([`ptz_config.rs:191`](../../src/types/ptz_config.rs)) omits
`PanTiltLimits` and `ZoomLimits`, which `from_xml` reads. A caller who does
get → modify limits → set loses them silently and gets `Ok(())`.

Insert both after `{timeout_el}`, before the close tag — the schema position
(§2.3). Shapes:

```xml
<tt:PanTiltLimits><tt:Range>
  <tt:URI>…</tt:URI>
  <tt:XRange><tt:Min>…</tt:Min><tt:Max>…</tt:Max></tt:XRange>
  <tt:YRange><tt:Min>…</tt:Min><tt:Max>…</tt:Max></tt:YRange>
</tt:Range></tt:PanTiltLimits>
<tt:ZoomLimits><tt:Range>
  <tt:URI>…</tt:URI>
  <tt:XRange><tt:Min>…</tt:Min><tt:Max>…</tt:Max></tt:XRange>
</tt:Range></tt:ZoomLimits>
```

`PtzSpaceRange.y_range` is `Option`; for `ZoomLimits` it must be omitted even if
`Some`, because `Space1DDescription` has no `YRange`.

The existing element order in `to_xml_body` already matches the schema
sequence — that part is correct and is not being changed.

### 3.3 Tests for stage 0

- Perturb: revert the spelling → the new "schema spelling is read" test must go
  red on the assertion.
- New negative-ish positive: a fixture using the **old** spelling still parses
  (pins the fallback). Without it the fallback is unexercised code.
- Round-trip at the type level: build a `PtzConfiguration` with both limits,
  `to_xml_body()`, re-parse with `from_xml`, assert every field survives.
  This is the assertion that would have caught both defects.
- `ptz_tests.rs:508`'s fixture moves to the schema spelling.

### 3.4 Deliberately out of scope for stage 0

`MoveRamp` / `PresetRamp` / `PresetTourRamp` on `PtzConfiguration`, and
`GeoMove` on `PtzNode`. Adding public fields is a breaking change and none of
them is a *silent data loss* the way the two above are. Record them in
`docs/mock-server.md` §13.2 instead.

### 3.5 Found while implementing: oxvif cannot drive a zoom-only head

**Not in the plan as written, and not fixed here.** Recorded 2026-08-03.

`ptz_absolute_move` ([`client/ptz.rs:36`](../../src/client/ptz.rs)),
`ptz_relative_move` (`:65`) and `ptz_continuous_move` (`:94`) all take
`pan, tilt, zoom: f32` and **always** emit a `<tt:PanTilt>` element. There is no
way to send a zoom-only vector.

Decision C makes that visible: `PTZNode_2` refuses any move carrying a pan/tilt
vector, so no oxvif method can move it. `tests/mock_multi_sensor.rs` positions
lens 2 with `GotoPreset` instead, which is honest but is a workaround.

This is a **real gap against real hardware**, not a mock artefact — a zoom-only
ONVIF head (a fixed-mount varifocal, a thermal channel) has exactly the same
schema, and oxvif would send it an element its `GetNodes` says it does not
support. The mock was hiding it, in the same way it hid the `Pant` spelling: the
mock accepted whatever oxvif sent, so oxvif and the mock agreed with each other
and with nothing else.

Deliberately out of scope for stages 0–2: closing it means new public methods
(`ptz_absolute_move_zoom`, or `Option<(f32, f32)>` parameters, which is
breaking), and that is a client decision, not a mock one.

---

## 4. Target model

### 4.1 State

```rust
// src/mock/state.rs
pub struct PtzNodeEntry {
    pub token: String,
    pub name: String,
    pub fixed_home_position: bool,
    pub home_supported: bool,
    pub max_presets: u32,
    pub aux_commands: Vec<String>,
    pub pan_tilt_spaces: Vec<SpaceEntry>,   // empty ⇒ zoom-only node
    pub zoom_spaces: Vec<SpaceEntry>,
}

pub struct SpaceEntry {           // renders as Space2D or Space1D by `y_range`
    pub kind: SpaceKind,          // which of the eight tt:PTZSpaces slots
    pub uri: String,
    pub x_range: (f32, f32),
    pub y_range: Option<(f32, f32)>,
}

pub struct PtzConfigEntry {
    pub token: String,
    pub name: String,
    pub use_count: u32,
    pub node_token: String,
    pub default_ptz_timeout: Option<String>,
    pub abs_pan_tilt_space: Option<String>,   // the six Default*Space URIs
    pub abs_zoom_space: Option<String>,
    pub rel_pan_tilt_space: Option<String>,
    pub rel_zoom_space: Option<String>,
    pub cont_pan_tilt_space: Option<String>,
    pub cont_zoom_space: Option<String>,
    pub default_speed_pan_tilt: Option<(f32, f32)>,
    pub default_speed_zoom: Option<f32>,
    pub pan_tilt_limits: Option<SpaceEntry>,  // read-only, see §5.2
    pub zoom_limits: Option<SpaceEntry>,
    pub timeout_min: String,                  // GetConfigurationOptions
    pub timeout_max: String,
}
```

`DeviceState` gains `ptz_nodes: Vec<PtzNodeEntry>` and
`ptz_configs: Vec<PtzConfigEntry>`, both with entries in the explicit `Default`
impl. `ProfileEntry` gains `ptz_config_token: Option<String>` with
`#[serde(default)]`, matching its four existing slots.

`PtzState.channels` changes key from **profile token** to **node token**.
`PtzState::channel` / `channel_mut` keep their signatures but take a node token;
a new resolver sits in front.

### 4.2 Resolution chain

```
ProfileToken → ProfileEntry.ptz_config_token → PtzConfigEntry.node_token → PtzChannel
```

Three failure points, each with its own fault:

| Failure | Code | Reason tag |
|---|---|---|
| no `ProfileToken` in the request | `env:Sender` | `NoProfileToken-…` (existing) |
| token names no profile | `ter:NoProfile` | `NoSuchProfile-…` (existing) |
| profile has no PTZ configuration | `ter:NoConfig` | `NoPTZConfig-…-5619` **(new, decision B)** |

`require_profile` ([`services/ptz.rs:26`](../../src/mock/services/ptz.rs)) grows
into `require_head`, returning the resolved node token. The `profile!` macro
follows. All 20 per-profile operations get the third failure mode for free.

### 4.3 Fixture

Two sensors, one head each — the physically honest mapping:

```
Profile_1 (VSC_1, main) ┐
                        ├→ PTZConfig_1 → PTZNode_1   full pan/tilt/zoom
Profile_2 (VSC_1, sub)  ┘
Profile_3 (VSC_2, main)  → PTZConfig_2 → PTZNode_2   zoom-only
Profile_4 (VSC_2, sub)   → (unbound)                 PTZ operations fault
```

**PTZNode_1** — `FixedHomePosition=false`, `HomeSupported=true`,
`MaximumNumberOfPresets=100`, aux commands present, all six space kinds plus
both speed spaces.

**PTZNode_2** — **no pan/tilt spaces at all**, `HomeSupported=false`,
`FixedHomePosition=true`, `MaximumNumberOfPresets=8`, no aux commands. This is
what makes decision C reachable through a real profile instead of needing a
fifth profile nobody uses. It is also consistent with the rest of the fixture,
where sensor 2 is the lesser lens (1280×720 cap, fixed focus).

**PTZConfig_1** — every `Default*Space` set, `DefaultPTZSpeed` set,
`PanTiltLimits` **and** `ZoomLimits` set, `DefaultPTZTimeout=PT10S`,
options `PT1S`/`PT60S`.

**PTZConfig_2** — only the three zoom spaces, **no `PanTiltLimits`**, only
`ZoomLimits`, no `DefaultPTZSpeed`, `DefaultPTZTimeout=PT30S`, options
`PT5S`/`PT30S`. The absent members are the point: they are the only way an
assertion can observe "the device did not say" as distinct from "the device said
a default", and `CLAUDE.md`'s batch mutation for `Option` fields depends on at
least one fixture exercising the `None` arm.

**Channel data moves with the node.** `PTZNode_1` inherits what `Profile_1` had
(presets `Home`, `Door`; one two-stop tour). `PTZNode_2` inherits what
`Profile_3` had (presets `Lobby`, `Dock`, `Roof`) **with pan/tilt zeroed**,
because a zoom-only head cannot hold a pan/tilt preset. Their zoom values
(`0.80`, `0.10`, `1.0`) already differ, so every existing count/name assertion
survives and the two heads still disagree on position.

`Profile_2`'s channel (`0.25 / -0.10 / 0.40`, preset `Gate`) and `Profile_4`'s
empty channel both disappear — the first because it is the same head as
`Profile_1`, the second because an unbound profile now faults. The "empty preset
list is a legitimate answer" case they covered is replaced by the unbound-profile
fault, which is a stronger statement.

---

## 5. Stage 1 — state and the read side

### 5.1 Handlers

| Handler | Becomes |
|---|---|
| `resp_ptz_nodes()` | `(state)` — renders every `PtzNodeEntry` with real `SupportedPTZSpaces` |
| `resp_ptz_node()` | `(state, body)` — resolves `NodeToken`; unknown → fault |
| `resp_ptz_configurations()` | `(state)` — renders every `PtzConfigEntry` |
| `resp_ptz_configuration()` | `(state, body)` — resolves `PTZConfigurationToken`; unknown → fault |
| `resp_ptz_compatible_configurations()` | `(state, body)` — resolves `ProfileToken` → the one bound configuration; unbound profile → empty list, **not** a fault (the operation asks "what is compatible", and "nothing" is a legitimate answer) |
| `resp_ptz_configuration_options()` | `(state, body)` — resolves `ConfigurationToken`; unknown → fault; per-configuration `PTZTimeout` |

Note the deliberate asymmetry on `GetCompatibleConfigurations`: it is the one
per-profile PTZ operation that must **not** apply decision B's fault, because an
empty compatible-set is exactly the answer a client uses to decide the profile is
not PTZ-capable. Faulting there would force every caller to treat a normal
condition as an error. This is the same distinction already drawn in
`docs/mock-server.md` §7.3 between a **filter** token and an **addressed** token.

### 5.2 Profiles carry their PTZ configuration

- `media::render_profile` — inline `<tt:PTZConfiguration token="…">` with the
  full body, matching the shape Media1 uses for the other configurations.
- `media2::render_profile_media2` — `<tr2:PTZ token="…"/>` inside
  `<tr2:Configurations>`, matching what `MediaProfile2::vec_from_xml` reads at
  [`types/media.rs:219`](../../src/types/media.rs) (`Configurations/PTZ@token`).
- `tests/mock_media1_media2_agree.rs` gains a row: both services must report the
  same `ptz_config_token` for the same profile.

### 5.3 Decision C — the zoom-only rejection

`handle_ptz_absolute_move` (and `RelativeMove`, `ContinuousMove` by the same
rule) resolves the head, then: **if the node has no pan/tilt space of the
relevant kind and the request carries a `PanTilt` vector, fault.** Strictly —
even `x="0" y="0"`, because the schema question is whether the vector is present,
not whether it is zero.

`ptz.wsdl` declares no fault codes (§2.3), so there is nothing to cite. Use
`ter:InvalidArgVal`, which is already in the mock's catalogue
(`docs/mock-server.md` §9.3) for "value outside the accepted set", with a
distinctive reason naming the node and the missing space. Record in the doc that
the code is the mock's choice and not quoted from ONVIF.

### 5.4 Test changes — the full list

Every site below was found by grepping for PTZ calls against `Profile_2`,
`Profile_3` and `Profile_4`; no test performs a move on `Profile_3` or
`Profile_4`, so decision C breaks nothing that exists.

**`tests/mock_multi_sensor.rs`**

| Test | Line | Change |
|---|---|---|
| `ptz_presets_answer_for_the_head_asked_about` | 428 | `Profile_4` returns 0 presets → now **faults**. Assert the fault. `Profile_1`=2, `Profile_3`=3 unchanged. |
| `moving_one_head_does_not_move_the_other` | 447 | unchanged — moves `Profile_1`, reads `Profile_3`, still two heads |
| `a_preset_stored_on_one_head_is_not_visible_on_another` | 469 | **inverted.** `Profile_2` and `Profile_1` are one head, so the preset *must* be visible; add `Profile_3` as the head that must not see it. Rename to say so. |
| `home_position_is_per_head` | 493 | rewrite with `Profile_1` / `Profile_3` |
| `preset_tours_are_per_head` | 527 | rewrite with `Profile_1` / `Profile_3` (`Profile_2` now ships `Profile_1`'s tour) |
| *(new)* | — | `main_and_sub_stream_of_one_lens_are_one_head` — the inverted premise as its own named test, so the property is stated positively and not only implied by a rewritten negative |

**`src/mock/state.rs`** (unit tests)

| Test | Line | Change |
|---|---|---|
| `ptz_set_preset_uses_current_position_and_returns_token` | 1662 | the "`Profile_2` must be untouched" assertion at 1682 becomes `Profile_3` |
| `ptz_preset_tours_are_per_profile` | 1740 | `Profile_2` → `Profile_3`; rename to `…_per_head` |
| `ptz_remove_preset_then_get` | 1687 | already uses `Profile_1` / `Profile_3` — unchanged |
| `ptz_without_a_profile_token_faults` | 1647 | unchanged; add the unbound-profile arm (decision B) |
| `default_ptz_channels` doc comment | 733 | rewrite — it currently explains a per-profile model |

**`tests/mock_token_discrimination.rs`** — `ptz/preset-tours` tokens change from
`("Profile_1", "Profile_2")` to `("Profile_1", "Profile_3")`. Left alone it
would silently become a Blind row that still claims `Discriminates`, and the
table would catch it — but catching it after the fact is worse than changing it
deliberately.

---

## 6. Stage 2 — `SetConfiguration` and the tables

### 6.1 What round-trips

`PtzConfiguration::to_xml_body` sends: token, `Name`, `UseCount`, `NodeToken`,
the six spaces, `DefaultPTZSpeed`, `DefaultPTZTimeout` — **and, after stage 0,
`PanTiltLimits` and `ZoomLimits` too.** So with stage 0 done first, every field
of `PtzConfigEntry` except `timeout_min` / `timeout_max` is round-trippable.
Those two belong to the *options* answer and have no place in a configuration,
which must be said in a comment at the write site — `CLAUDE.md` step 5c: a
documented omission is a design decision, an undocumented one is the `MTU` bug.

`force_persist` ([`client/ptz.rs:355`](../../src/client/ptz.rs)) is **not**
modelled. Real devices differ too widely on `ForcePersistence=false` for a
pretend model to be better than none. Record in `docs/mock-server.md` §13.3.

Writing a `NodeToken` that names no node must fault, not create a dangling
configuration.

### 6.2 `AddConfiguration(Type=PTZ)`

`ConfigKind` ([`services/media.rs:367`](../../src/mock/services/media.rs)) has
four variants and `from_media2_type` returns `None` for `PTZ`, so Media2's
`AddConfiguration` faults with `UnmodelledConfigType-CFG2-5542`. Its
justification ([`services/media2.rs:262`](../../src/mock/services/media2.rs))
reads *"there is no state to write and no getter that could ever show the
result"* — **that sentence becomes false** the moment `ProfileEntry` gains the
slot and the profile renderers emit it.

So this is not optional: either wire it or rewrite the justification. Wire it —
add `ConfigKind::Ptz`, its slot, and `known_token` validation against
`ptz_configs`. Media1 has no `AddPTZConfiguration` in oxvif
(`docs/reference/media1.md:57`), so this is Media2-only and creates no
Media1/Media2 divergence to audit.

### 6.3 Table and pin deltas

`tests/mock_roundtrip.rs`

| Row | From | To |
|---|---|---|
| `ptz/configuration` | `Static("audit §5")` | `Works` |
| `media2/add-ptz-config` | — | `Works` (new) |

`PAIRS.len()` 48 → **49**, declared-Works 45 → **47**. Pin
`assert_eq!((PAIRS.len(), declared_works), (49, 47))`.

`tests/mock_token_discrimination.rs`

| Row | From | To |
|---|---|---|
| `ptz/compatible-configs` | `Blind("audit §5 …")` | `Discriminates` |
| `ptz/configuration` | — | `Discriminates` (new — `GetConfiguration` by config token) |
| `ptz/node` | — | `Discriminates` (new — `GetNode` by node token) |
| `ptz/configuration-options` | — | `Discriminates` (new) |
| `ptz/preset-tours` | tokens `(Profile_1, Profile_2)` | `(Profile_1, Profile_3)` |

`ROWS.len()` 28 → **31**, declared-Discriminates 21 → **25**. Pin
`assert_eq!((ROWS.len(), declared_discriminating), (31, 25))`.

Both pins print the two documents that quote the counts; update
`docs/mock-server.md` §12 and `mock-audit-2026-07.md` §2 in the same commit.

### 6.4 What actually happened

**The token-table deltas above were all consumed by stage 1**, not stage 2 —
those three handlers stop being string literals in stage 1, so leaving their rows
out until stage 2 would have meant shipping a commit that knowingly left the
table incomplete. Stage 2's only table change is `tests/mock_roundtrip.rs`:
`ptz/configuration` `Static` → `Works`, one new `media2/add-ptz-configuration`
row, pin `(48, 45)` → `(49, 47)`.

**Stage 1 updated the audit's copy of the token counts and not
`docs/mock-server.md` §12's**, which still read 28/22 when stage 2 started. The
pin's failure message names both documents; it fired, and only the count that
had moved in the *other* table was read. Fixed here.

`docs/mock-server.md` §7.4 also still listed the whole configuration family as a
declared stub after stage 1 wired six of the seven operations. Fixed here.

---

## 7. Commit plan

Each stage is one commit and passes the full five-line gate on its own.

| # | Subject | Depends on |
|---|---|---|
| 0 | `fix(ptz)!: spell the absolute pan/tilt space the way ONVIF does, and send the limits` | — |
| 1 | `fix(mock)!: give PTZ nodes and configurations real state` | 0 |
| 2 | `fix(mock): persist SetConfiguration and bind PTZ to profiles` | 1 |

Stage 0 is independently useful and independently shippable; it is the only one
of the three that changes behaviour against a **real camera**.

---

## 8. Perturbations to run

Beyond the per-assertion ones:

- Make the space renderer emit nothing → every space/limits assertion must go
  red. Proves the Tier 4 fields are observed, not merely written.
- Make `require_head` ignore the resolved node and always return the first →
  every "two heads" assertion must go red, and the inverted "one head" tests
  must **stay green** (they would pass under a broken resolver too, which is
  exactly why §5.4 adds a positive test rather than relying on the inversions).
- Make the `Option` space fields render as `Some("")` where they are `None` →
  `PTZConfig_2`'s absent-member assertions must go red. This is `CLAUDE.md`'s
  `Option<bool>` batch mutation applied to the field family this stage adds.
- Restore the old spelling in `from_xml` → stage 0's schema-spelling test red,
  the legacy-spelling fallback test green.

---

## 9. Explicitly out of scope

- `MoveRamp` / `PresetRamp` / `PresetTourRamp` (`PTZConfiguration` attributes)
  and `GeoMove` (`PTZNode` attribute) — public API additions, §3.4.
- `PtzNode`'s flattening of eight space kinds into two `Vec`s. The information
  is recoverable from the URI, and un-flattening is a breaking change.
- `GeoMove` the *operation* — not implemented by oxvif
  (`docs/reference/ptz.md:38`).
- `ForcePersistence` semantics, §6.1.
- The Audio family, which is the only Tier 3 item left after this.
