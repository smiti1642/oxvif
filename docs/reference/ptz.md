# PTZ Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver20/ptz/wsdl/ptz.wsdl
- **Namespace:** `http://www.onvif.org/ver20/ptz/wsdl` (prefix `tptz`)
- **ONVIF Profile:** S (PTZ)
- **oxvif status:** ◐ implemented in `src/client/ptz.rs` (26 / 29 operations)

Coordinates use the ONVIF normalised range: pan/tilt `[-1.0, 1.0]`, zoom `[0.0, 1.0]`.

---

## Operations

| Operation | Purpose | In → Out | oxvif | method |
|-----------|---------|----------|:----:|--------|
| AbsoluteMove | move to absolute position | `AbsoluteMove` → `…Response` | ✓ | `ptz_absolute_move` |
| RelativeMove | move by offset | `RelativeMove` → `…Response` | ✓ | `ptz_relative_move` |
| ContinuousMove | start continuous move | `ContinuousMove` → `…Response` | ✓ | `ptz_continuous_move` |
| Stop | stop movement | `Stop` → `…Response` | ✓ | `ptz_stop` |
| GetStatus | current position + state | `GetStatus` → `…Response` | ✓ | `ptz_get_status` |
| GetPresets | list presets | `GetPresets` → `…Response` | ✓ | `ptz_get_presets` |
| SetPreset | save current as preset | `SetPreset` → `…Response` | ✓ | `ptz_set_preset` |
| GotoPreset | move to preset | `GotoPreset` → `…Response` | ✓ | `ptz_goto_preset` |
| RemovePreset | delete preset | `RemovePreset` → `…Response` | ✓ | `ptz_remove_preset` |
| GotoHomePosition | go to home | `GotoHomePosition` → `…Response` | ✓ | `ptz_goto_home_position` |
| SetHomePosition | set home | `SetHomePosition` → `…Response` | ✓ | `ptz_set_home_position` |
| GetConfigurations | list PTZ configs | `GetConfigurations` → `…Response` | ✓ | `ptz_get_configurations` |
| GetConfiguration | single config | `GetConfiguration` → `…Response` | ✓ | `ptz_get_configuration` |
| SetConfiguration | write config | `SetConfiguration` → `…Response` | ✓ | `ptz_set_configuration` |
| GetConfigurationOptions | config option ranges | `GetConfigurationOptions` → `…Response` | ✓ | `ptz_get_configuration_options` |
| GetNodes | list PTZ nodes | `GetNodes` → `…Response` | ✓ | `ptz_get_nodes` |
| GetNode | single node | `GetNode` → `…Response` | ✓ | `ptz_get_node` |
| GetCompatibleConfigurations | configs compatible w/ profile | `GetCompatibleConfigurations` → `…Response` | ✓ | `ptz_get_compatible_configurations` |
| GetServiceCapabilities | PTZ service capabilities | `GetServiceCapabilities` → `…Response` | ✓ | `ptz_get_service_capabilities` |
| SendAuxiliaryCommand | PTZ auxiliary command | `SendAuxiliaryCommand` → `…Response` | ✓ | `ptz_send_auxiliary_command` |
| GeoMove | move to a geolocation | `GeoMove` → `…Response` | — | — |
| GetPresetTours | list preset tours | `GetPresetTours` → `…Response` | ✓ | `ptz_get_preset_tours` |
| GetPresetTour | single preset tour | `GetPresetTour` → `…Response` | ✓ | `ptz_get_preset_tour` |
| GetPresetTourOptions | preset-tour option ranges | `GetPresetTourOptions` → `…Response` | ✓ | `ptz_get_preset_tour_options` |
| CreatePresetTour | create preset tour | `CreatePresetTour` → `…Response` | ✓ | `ptz_create_preset_tour` |
| ModifyPresetTour | modify preset tour | `ModifyPresetTour` → `…Response` | ✓ | `ptz_modify_preset_tour` |
| OperatePresetTour | start/stop/pause a tour | `OperatePresetTour` → `…Response` | ✓ | `ptz_operate_preset_tour` |
| RemovePresetTour | delete preset tour | `RemovePresetTour` → `…Response` | ✓ | `ptz_remove_preset_tour` |
| MoveAndStartTracking | move then auto-track | `MoveAndStartTracking` → `…Response` | — | — |

> Note: oxvif implements **both** auxiliary-command operations, and they are not
> interchangeable. `send_auxiliary_command` is the **Device** service one;
> `ptz_send_auxiliary_command` is this one, which is per-profile, carries
> `tt:AuxiliaryData` rather than `tt:AuxiliaryCommand`, and returns a response
> payload. Cameras that implement a wiper generally implement the PTZ one.

---

## Request / response detail (unimplemented only)

#### GetServiceCapabilities
- **Req:** _(empty)_  · **Resp:** `Capabilities` `tptz:Capabilities` [1]

`tptz:Capabilities` — all members are **attributes**, all optional. Complete set:

| Attribute | Type | Meaning |
|-----------|------|---------|
| `EFlip` | `xs:boolean` | E-Flip supported |
| `Reverse` | `xs:boolean` | reversing pan/tilt control direction supported |
| `GetCompatibleConfigurations` | `xs:boolean` | the `GetCompatibleConfigurations` command is supported |
| `MoveStatus` | `xs:boolean` | `PTZStatus` includes `MoveStatus` |
| `StatusPosition` | `xs:boolean` | `PTZStatus` includes `Position` |
| `MoveAndTrack` | `tt:StringList` | supported `MoveAndStartTracking` methods |

`MoveAndTrack` values come from `tt:MoveAndTrackMethod`: `PresetToken`,
`GeoLocation`, `PTZVector`, `ObjectID`. It is a *list-typed attribute* —
whitespace-separated, not repeated elements.

> `MoveStatus` and `StatusPosition` are the two most directly checkable claims
> in the whole capability surface: both are assertions about what a
> `GetStatus` response will contain, and oxvif already parses `GetStatus`
> (`PtzStatus::from_xml`). A camera that sets `MoveStatus` but returns no
> `MoveStatus` element is the canonical claim-vs-behaviour divergence.

#### SendAuxiliaryCommand
- **Req:** `ProfileToken` `tt:ReferenceToken` [1]; `AuxiliaryData` `tt:AuxiliaryData` [1]
- **Resp:** `AuxiliaryResponse` `tt:AuxiliaryData` [1]

#### GeoMove
- **Req:** `ProfileToken` `tt:ReferenceToken` [1]; `Target` `tt:GeoLocation` [1];
  `Speed` `tt:PTZSpeed` [0..1]; `AreaHeight` `xs:float` [0..1]; `AreaWidth` `xs:float` [0..1]
- **Resp:** _(empty)_

#### GetPresetTours
- **Req:** `ProfileToken` `tt:ReferenceToken` [1]  · **Resp:** `PresetTour` `tt:PresetTour` [0..*]

#### GetPresetTour
- **Req:** `ProfileToken` [1]; `PresetTourToken` `tt:ReferenceToken` [1]
- **Resp:** `PresetTour` `tt:PresetTour` [1]

#### GetPresetTourOptions
- **Req:** `ProfileToken` [1]; `PresetTourToken` `tt:ReferenceToken` [0..1]
- **Resp:** `Options` `tt:PTZPresetTourOptions` [1]

#### CreatePresetTour
- **Req:** `ProfileToken` `tt:ReferenceToken` [1]
- **Resp:** `PresetTourToken` `tt:ReferenceToken` [1]

#### ModifyPresetTour
- **Req:** `ProfileToken` [1]; `PresetTour` `tt:PresetTour` [1]  · **Resp:** _(empty)_

#### OperatePresetTour
- **Req:** `ProfileToken` [1]; `PresetTourToken` `tt:ReferenceToken` [1];
  `Operation` `tt:PTZPresetTourOperation` [1] (`Start|Stop|Pause|Extended`)
- **Resp:** _(empty)_

#### RemovePresetTour
- **Req:** `ProfileToken` [1]; `PresetTourToken` `tt:ReferenceToken` [1]  · **Resp:** _(empty)_

#### MoveAndStartTracking
- **Req:** `ProfileToken` `tt:ReferenceToken` [1]; `PresetToken` `tt:ReferenceToken` [0..1];
  `GeoLocation` `tt:GeoLocation` [0..1]; `TargetPosition` `tt:PTZVector` [0..1];
  `Speed` `tt:PTZSpeed` [0..1]; `ObjectID` `xs:integer` [0..1]
- **Resp:** _(empty)_

`tt:GeoLocation`: see onvif.xsd. (`tt:PTZSpeed`, `tt:PTZVector` in
[types.md](types.md).) The preset-tour family and `tt:AuxiliaryData` are
expanded below.

_Source: ptz.wsdl `<wsdl:types>` (fetched 2026-05)._

---

## Preset-tour types

PTZ-only, so they live here rather than in [types.md](types.md), despite the
`tt:` prefix.

### `tt:PresetTour`

| Member | Type | Card. | Notes |
|--------|------|:-----:|-------|
| `token` (attr) | `tt:ReferenceToken` | [0..1] | **optional in the schema** — see below |
| `Name` | `tt:Name` | [0..1] | |
| `Status` | `tt:PTZPresetTourStatus` | [1] | |
| `AutoStart` | `xs:boolean` | [1] | |
| `StartingCondition` | `tt:PTZPresetTourStartingCondition` | [1] | |
| `TourSpot` | `tt:PTZPresetTourSpot` | [0..*] | the ordered tour |
| `Extension` | `tt:PTZPresetTourExtension` | [0..1] | |

> **`token` is `[0..1]` here**, unlike `tt:PTZPreset/@token`. The CLAUDE.md rule
> "required fields must return `Result`" keys off the *schema* cardinality, so
> this one is genuinely `Option<String>` — do not reflexively
> `ok_or_else(missing("PresetTour/@token"))` it. On a `GetPresetTours` response
> every real device sends it; on a tour being *sent* to `ModifyPresetTour` it
> identifies the target. Decide per direction, and note that `Status`,
> `AutoStart` and `StartingCondition` **are** `[1]` and so are the fields that
> should hard-fail when absent.

### `tt:PTZPresetTourStatus`

`State` `tt:PTZPresetTourState` [1]; `CurrentTourSpot` `tt:PTZPresetTourSpot`
[0..1]; `Extension` [0..1].

`tt:PTZPresetTourState` = `Idle` | `Touring` | `Paused` | `Extended`.

### `tt:PTZPresetTourStartingCondition`

| Member | Type | Card. |
|--------|------|:-----:|
| `RandomPresetOrder` (attr) | `xs:boolean` | [0..1] |
| `RecurringTime` | `xs:int` | [0..1] |
| `RecurringDuration` | `xs:duration` | [0..1] |
| `Direction` | `tt:PTZPresetTourDirection` | [0..1] |
| `Extension` | — | [0..1] |

`tt:PTZPresetTourDirection` = `Forward` | `Backward` | `Extended`.

### `tt:PTZPresetTourSpot`

`PresetDetail` `tt:PTZPresetTourPresetDetail` [1]; `Speed` `tt:PTZSpeed` [0..1];
`StayTime` `xs:duration` [0..1]; `Extension` [0..1].

### `tt:PTZPresetTourPresetDetail`

An **`xs:choice`** — exactly one of:

- `PresetToken` `tt:ReferenceToken`
- `Home` `xs:boolean`
- `PTZPosition` `tt:PTZVector`
- `TypeExtension` `tt:PTZPresetTourTypeExtension`

> This is a choice, not a sequence of optionals. It maps to a Rust `enum`, and
> serialising more than one variant produces a schema-invalid request. It is
> the single most likely place to get `ModifyPresetTour` wrong.

### `tt:PTZPresetTourOptions`

`AutoStart` `xs:boolean` [1]; `StartingCondition`
`tt:PTZPresetTourStartingConditionOptions` [1]; `TourSpot`
`tt:PTZPresetTourSpotOptions` [1].

- **`…StartingConditionOptions`** — `RecurringTime` `tt:IntRange` [0..1];
  `RecurringDuration` `tt:DurationRange` [0..1]; `Direction`
  `tt:PTZPresetTourDirection` **[0..\*]** (the supported directions, repeated);
  `Extension` [0..1].
- **`…SpotOptions`** — `PresetDetail` `tt:PTZPresetTourPresetDetailOptions` [1];
  `StayTime` `tt:DurationRange` [1].

> Note the asymmetry: `Direction` is a single value in
> `StartingCondition` but a repeated list in `StartingConditionOptions`. Same
> element name, different cardinality, different Rust type.

### `tt:AuxiliaryData`

Not a complex type — an `xs:string` restricted to **`maxLength` 128**. Values
are vendor-namespaced strings such as `tt:Wiper|On`, `tt:IRLamp|Auto`. The
device-level `tds:MiscCapabilities/@AuxiliaryCommands` (see
[device.md](device.md)) is the discoverable list of what a given camera accepts.

_Source: onvif.xsd (fetched 2026-07-28)._
