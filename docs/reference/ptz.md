# PTZ Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver20/ptz/wsdl/ptz.wsdl
- **Namespace:** `http://www.onvif.org/ver20/ptz/wsdl` (prefix `tptz`)
- **ONVIF Profile:** S (PTZ)
- **oxvif status:** ◐ implemented in `src/client/ptz.rs` (18 / 29 operations)

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
| GetServiceCapabilities | PTZ service capabilities | `GetServiceCapabilities` → `…Response` | — | — |
| SendAuxiliaryCommand | PTZ auxiliary command | `SendAuxiliaryCommand` → `…Response` | — | — |
| GeoMove | move to a geolocation | `GeoMove` → `…Response` | — | — |
| GetPresetTours | list preset tours | `GetPresetTours` → `…Response` | — | — |
| GetPresetTour | single preset tour | `GetPresetTour` → `…Response` | — | — |
| GetPresetTourOptions | preset-tour option ranges | `GetPresetTourOptions` → `…Response` | — | — |
| CreatePresetTour | create preset tour | `CreatePresetTour` → `…Response` | — | — |
| ModifyPresetTour | modify preset tour | `ModifyPresetTour` → `…Response` | — | — |
| OperatePresetTour | start/stop/pause a tour | `OperatePresetTour` → `…Response` | — | — |
| RemovePresetTour | delete preset tour | `RemovePresetTour` → `…Response` | — | — |
| MoveAndStartTracking | move then auto-track | `MoveAndStartTracking` → `…Response` | — | — |

> Note: oxvif's `send_auxiliary_command` is the **Device** service operation, not PTZ
> `SendAuxiliaryCommand` (which carries `tt:AuxiliaryData` and returns a response payload).

---

## Request / response detail (unimplemented only)

#### GetServiceCapabilities
- **Req:** _(empty)_  · **Resp:** `Capabilities` `tptz:Capabilities` [1]

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

Complex types `tt:PresetTour`, `tt:PTZPresetTourOptions`, `tt:GeoLocation`, `tt:AuxiliaryData`:
see ptz.wsdl `<wsdl:types>` / onvif.xsd. (`tt:PTZSpeed`, `tt:PTZVector` in [types.md](types.md).)

_Source: ptz.wsdl `<wsdl:types>` (fetched 2026-05)._
