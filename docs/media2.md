# Media2 Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver20/media/wsdl/media.wsdl
- **Namespace:** `http://www.onvif.org/ver20/media/wsdl` (prefix `tr2`)
- **ONVIF Profile:** T
- **oxvif status:** ◐ implemented in `src/client/media2.rs` (~26 of ~59 operations)

Media2 replaces Media1's per-kind binding ops with a single generic `AddConfiguration` /
`RemoveConfiguration` (a `Type` discriminator), flattens encoder configs, and drops
`ForcePersistence`. oxvif covers profiles, stream/snapshot, video/audio/metadata configs, encoder
instances, and video source modes. Unimplemented: privacy **masks**, **WebRTC**, **audio clips**,
multicast/EQ/decoder audio, analytics config, and OSD-via-Media2 (oxvif does OSD via Media1).

---

## Operations

### Profiles, configs, streaming
| Operation | Purpose | oxvif | method |
|-----------|---------|:----:|--------|
| CreateProfile | create profile | ✓ | `create_profile_media2` |
| GetProfiles | list profiles | ✓ | `get_profiles_media2` |
| DeleteProfile | delete profile | ✓ | `delete_profile_media2` |
| AddConfiguration | bind config(s) to profile | ✓ | `add_configuration_media2` |
| RemoveConfiguration | unbind config(s) | ✓ | `remove_configuration_media2` |
| GetStreamUri | RTSP URI | ✓ | `get_stream_uri_media2` |
| GetSnapshotUri | snapshot URI | ✓ | `get_snapshot_uri_media2` |
| GetVideoEncoderInstances | encoder capacity | ✓ | `get_video_encoder_instances_media2` |
| GetVideoSourceModes / SetVideoSourceMode | sensor modes | ✓ | `get_video_source_modes_media2` / `set_video_source_mode_media2` |
| GetServiceCapabilities | media2 capabilities | — | — |
| SetSynchronizationPoint | force I-frame / refresh | — | — |
| StartMulticastStreaming / StopMulticastStreaming | multicast control | — | — |

### Configuration get/set/options (Media2 shape)
Media2 `Get<Kind>Configurations` take optional `ConfigurationToken` + `ProfileToken` filters and
return arrays; `Set<Kind>Configuration` takes the config (no `ForcePersistence`);
`Get<Kind>ConfigurationOptions` take optional tokens.

| Kind | GetConfigurations | SetConfiguration | GetConfigurationOptions |
|------|:-----------------:|:----------------:|:-----------------------:|
| VideoSource | ✓ | ✓ | ✓ |
| VideoEncoder | ✓ | ✓ | ✓ |
| AudioSource | ✓ | — | — |
| AudioEncoder | ✓ | ✓ | ✓ |
| Metadata | ✓ | ✓ | ✓ |
| AudioOutput | ✓ | — | — |
| AudioDecoder | ✓ | — | — |
| Analytics | — | (n/a) | — |

(`GetVideoEncoderConfiguration` single-token lookup is implemented via `get_video_encoder_configuration_media2`.)
Also unimplemented: `SetEQPresetConfiguration`, `GetMulticastAudioDecoderConfigurations`,
`GetMulticastAudioDecoderConfigurationOptions`, `SetMulticastAudioDecoderConfiguration`.

### OSD (Media2) — all `—`; oxvif uses Media1
`GetOSDs`, `GetOSDOptions`, `SetOSD`, `CreateOSD`, `DeleteOSD` — see [osd.md](osd.md).

### Privacy masks · WebRTC · audio clips — all `—`
`GetMasks`, `GetMaskOptions`, `SetMask`, `CreateMask`, `DeleteMask`,
`GetWebRTCConfigurations`, `SetWebRTCConfigurations`,
`GetAudioClips`, `AddAudioClip`, `SetAudioClip`, `DeleteAudioClip`, `PlayAudioClip`, `GetPlayingAudioClips`.

---

## Request / response detail (unimplemented)

#### GetServiceCapabilities
- **Req:** _(empty)_ · **Resp:** `Capabilities` `tr2:Capabilities2` [1]

#### SetSynchronizationPoint / StartMulticastStreaming / StopMulticastStreaming
- **Req:** `ProfileToken` `tt:ReferenceToken` [1] · **Resp:** _(empty)_

#### GetAnalyticsConfigurations
- **Req:** `ConfigurationToken` `tt:ReferenceToken` [0..1]; `ProfileToken` `tt:ReferenceToken` [0..1]
- **Resp:** `Configurations` `tt:VideoAnalyticsConfiguration` [0..*]

### Privacy masks (Profile T)
#### GetMasks
- **Req:** `Token` `tt:ReferenceToken` [0..1]; `ConfigurationToken` `tt:ReferenceToken` [0..1]
- **Resp:** `Masks` `tr2:Mask` [0..*]

#### GetMaskOptions
- **Req:** `ConfigurationToken` `tt:ReferenceToken` [1] · **Resp:** `Options` `tr2:MaskOptions` [1]

#### SetMask
- **Req:** `Mask` `tr2:Mask` [1] · **Resp:** _(empty)_

#### CreateMask
- **Req:** `Mask` `tr2:Mask` [1] · **Resp:** `Token` `tt:ReferenceToken` [1]

#### DeleteMask
- **Req:** `Token` `tt:ReferenceToken` [1] · **Resp:** _(empty)_

### WebRTC
#### GetWebRTCConfigurations
- **Req:** _(empty)_ · **Resp:** `WebRTCConfiguration` `tr2:WebRTCConfiguration` [0..*]

#### SetWebRTCConfigurations
- **Req:** `WebRTCConfiguration` `tr2:WebRTCConfiguration` [0..*] · **Resp:** _(empty)_

### Audio clips
#### GetAudioClips
- **Req:** `Token` `tt:ReferenceToken` [0..1] · **Resp:** `AudioClipItem` `tr2:GetAudioClipsResponseItem` [0..*]

#### AddAudioClip
- **Req:** `Token` `tt:ReferenceToken` [0..1]; `Configuration` `tr2:AudioClip` [1]
- **Resp:** `Token` `tt:ReferenceToken` [1]; `UploadUri` `xs:anyURI` [1]; `ExpiryTime` `xs:dateTime` [1]

#### PlayAudioClip
- **Req:** `Token` `tt:ReferenceToken` [1]; `AudioOutputToken` `tt:ReferenceToken` [0..*];
  `Play` `xs:boolean` [1]; `RepeatCycles` `xs:int` [0..1] · **Resp:** _(empty)_

#### GetPlayingAudioClips
- **Req:** _(empty)_ · **Resp:** `PlayingAudioClips` `tr2:PlayingAudioClips` [0..*]

Complex types (`tr2:Mask`, `tr2:MaskOptions`, `tr2:WebRTCConfiguration`, `tr2:AudioClip`,
`tr2:Capabilities2`): see media2 wsdl `<wsdl:types>`.

_Source: media2 wsdl operation list + `<wsdl:types>` (fetched 2026-05)._
