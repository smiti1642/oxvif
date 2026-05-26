# Imaging Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver20/imaging/wsdl/imaging.wsdl
- **Namespace:** `http://www.onvif.org/ver20/imaging/wsdl` (prefix `timg`)
- **ONVIF Profile:** T
- **oxvif status:** ◐ implemented in `src/client/imaging.rs` (7 / 11 operations)

---

## Operations

| Operation | Purpose | In → Out | oxvif | method |
|-----------|---------|----------|:----:|--------|
| GetImagingSettings | current brightness/contrast/exposure/etc. | `GetImagingSettings` → `…Response` | ✓ | `get_imaging_settings` |
| SetImagingSettings | apply imaging settings | `SetImagingSettings` → `…Response` | ✓ | `set_imaging_settings` |
| GetOptions | valid ranges for settings | `GetOptions` → `…Response` | ✓ | `get_imaging_options` |
| Move | focus move (abs/rel/continuous) | `Move` → `…Response` | ✓ | `imaging_move` |
| Stop | stop focus move | `Stop` → `…Response` | ✓ | `imaging_stop` |
| GetStatus | focus position + move state | `GetStatus` → `…Response` | ✓ | `imaging_get_status` |
| GetMoveOptions | valid focus move ranges | `GetMoveOptions` → `…Response` | ✓ | `imaging_get_move_options` |
| GetServiceCapabilities | imaging service capabilities | `GetServiceCapabilities` → `…Response` | — | — |
| GetPresets | list manufacturer imaging presets | `GetPresets` → `…Response` | — | — |
| GetCurrentPreset | currently-applied imaging preset | `GetCurrentPreset` → `…Response` | — | — |
| SetCurrentPreset | apply an imaging preset | `SetCurrentPreset` → `…Response` | — | — |

---

## Request / response detail (unimplemented only)

#### GetServiceCapabilities
- **Req:** _(empty)_
- **Resp:** `Capabilities` `timg:Capabilities` [1] — attrs incl. `ImageStabilization`, `Presets`,
  `AdaptablePreset` (bool, optional).

#### GetPresets
- **Req:** `VideoSourceToken` `tt:ReferenceToken` [1]
- **Resp:** `Preset` `timg:ImagingPreset` [1..*]

#### GetCurrentPreset
- **Req:** `VideoSourceToken` `tt:ReferenceToken` [1]
- **Resp:** `Preset` `timg:ImagingPreset` [0..1]

#### SetCurrentPreset
- **Req:** `VideoSourceToken` `tt:ReferenceToken` [1]; `PresetToken` `tt:ReferenceToken` [1]
- **Resp:** _(empty)_

**`timg:ImagingPreset`** — attrs `token` `tt:ReferenceToken` (req), `type` `xs:string` (req);
child `Name` `tt:Name` [1].

_Source: imaging.wsdl `<wsdl:types>` (fetched 2026-05)._
