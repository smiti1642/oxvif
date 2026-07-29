# Media2 Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver20/media/wsdl/media.wsdl
- **Namespace:** `http://www.onvif.org/ver20/media/wsdl` (prefix `tr2`)
- **ONVIF Profile:** T
- **oxvif status:** ◐ implemented in `src/client/media2.rs` (~27 of ~59 operations)

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
| GetServiceCapabilities | media2 capabilities | ✓ | `media2_get_service_capabilities` |
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

> The type really is named **`Capabilities2`**, not `Capabilities` — verified
> against ver20 media.wsdl, which defines no type named `Capabilities` at all.
> The response *element* is still `GetServiceCapabilitiesResponse/Capabilities`;
> only the type name carries the `2`.

Attributes, all optional:

| Attribute | Type | Meaning |
|-----------|------|---------|
| `SnapshotUri` | `xs:boolean` | `GetSnapshotUri` supported |
| `Rotation` | `xs:boolean` | rotation configurable on a video source |
| `VideoSourceMode` | `xs:boolean` | `GetVideoSourceModes` / `SetVideoSourceMode` supported |
| `OSD` | `xs:boolean` | OSD configuration supported |
| `TemporaryOSDText` | `xs:boolean` | temporary OSD text supported |
| `Mask` | `xs:boolean` | privacy masks supported |
| `SourceMask` | `xs:boolean` | masks defined on the video source (not the profile) |
| `WebRTC` | **`xs:int`** | number of simultaneous WebRTC sessions supported |
| `WebRTC_codecs` | `tt:StringList` | codecs offered over WebRTC |

> `WebRTC` is an **`xs:int`**, not a boolean — it is a session count. Parsing it
> as a bool loses the count and misreports `0` as "supported".

Children:

- **`ProfileCapabilities`** `tr2:ProfileCapabilities` [1] — attrs
  `MaximumNumberOfProfiles` `xs:int` [0..1], `ConfigurationsSupported`
  `tt:StringAttrList` [0..1].
- **`StreamingCapabilities`** `tr2:StreamingCapabilities` [1] — attrs
  `RTSPStreaming`, `RTPMulticast`, `RTP_RTSP_TCP`, `NonAggregateControl`,
  `AutoStartMulticast`, `SecureRTSPStreaming` (all `xs:boolean` [0..1]) and
  `RTSPWebSocketUri` `xs:anyURI` [0..1].
- **`AudioClipCapabilities`** `tr2:AudioClipCapabilities` [0..1].
- **`MulticastAudioDecoderCapabilities`** `tr2:MulticastAudioDecoderCapabilities` [0..1].

Media2's `StreamingCapabilities` is **not** the same shape as Media1's: it drops
`RTP_TCP` and `NoRTSPStreaming`, and adds `RTSPStreaming`, `AutoStartMulticast`,
`SecureRTSPStreaming` and `RTSPWebSocketUri`. Three same-named types across the
device / Media1 / Media2 levels — do not share one Rust struct between them.

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
