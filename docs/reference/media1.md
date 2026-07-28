# Media Service (Media1)

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).
> Audio operations are detailed in [audio.md](audio.md); OSD in [osd.md](osd.md).

- **WSDL:** https://www.onvif.org/ver10/media/wsdl/media.wsdl
- **Namespace:** `http://www.onvif.org/ver10/media/wsdl` (prefix `trt`)
- **ONVIF Profile:** S
- **oxvif status:** ◐ implemented in `src/client/media.rs` (~31 of ~78 operations)

oxvif covers profiles, stream/snapshot URIs, video source/encoder configs (get/set/options),
the video-encoder binding pair, audio basics, and OSD. The unimplemented bulk is the **regular
configuration family** repeated across 8 configuration kinds (see pattern below) plus multicast
streaming and the "compatible configurations" queries.

---

## Operations

### Profiles & streaming
| Operation | Purpose | oxvif | method |
|-----------|---------|:----:|--------|
| GetProfiles | list profiles | ✓ | `get_profiles` |
| GetProfile | single profile | ✓ | `get_profile` |
| CreateProfile | create profile | ✓ | `create_profile` |
| DeleteProfile | delete profile | ✓ | `delete_profile` |
| GetStreamUri | RTSP URI | ✓ | `get_stream_uri` |
| GetSnapshotUri | snapshot URI | ✓ | `get_snapshot_uri` |
| GetServiceCapabilities | media service capabilities | — | — |
| StartMulticastStreaming | begin multicast | — | — |
| StopMulticastStreaming | end multicast | — | — |
| SetSynchronizationPoint | force I-frame / config refresh | — | — |
| GetVideoSourceModes | sensor modes | — | — (see media2 `get_video_source_modes_media2`) |
| SetVideoSourceMode | switch sensor mode | — | — (see media2) |
| GetGuaranteedNumberOfVideoEncoderInstances | encoder capacity | — | — |

### Sources
| Operation | Purpose | oxvif | method |
|-----------|---------|:----:|--------|
| GetVideoSources | physical video inputs | ✓ | `get_video_sources` |
| GetAudioSources | physical audio inputs | ✓ | `get_audio_sources` |
| GetAudioOutputs | physical audio outputs | — | — |

### Configuration family (per kind)
For each configuration **kind** the WSDL repeats the same operation set. oxvif coverage by kind:

| Kind | Add/Remove to profile | GetConfigurations | GetConfiguration | SetConfiguration | GetConfigurationOptions | GetCompatible… |
|------|:---------------------:|:-----------------:|:----------------:|:----------------:|:-----------------------:|:--------------:|
| VideoSource | ✓ / ✓ | ✓ | ✓ | ✓ | ✓ | — |
| VideoEncoder | ✓ / ✓ | ✓ | ✓ | ✓ | ✓ | — |
| AudioSource | — / — | ✓ | — | — | — | — |
| AudioEncoder | — / — | ✓ | ✓ | ✓ | ✓ | — |
| VideoAnalytics | — / — | — | — | — | (n/a) | — |
| Metadata | — / — | — | — | — | — | — |
| AudioOutput | — / — | — | — | — | — | — |
| AudioDecoder | — / — | — | — | — | — | — |
| PTZ | — / — | (PTZ service) | — | — | — | — |

Operation names follow `Add<Kind>Configuration`, `Remove<Kind>Configuration`,
`Get<Kind>Configurations`, `Get<Kind>Configuration`, `Set<Kind>Configuration`,
`Get<Kind>ConfigurationOptions`, `GetCompatible<Kind>Configurations`.

### OSD
`GetOSDs` ✓ · `GetOSD` ✓ · `CreateOSD` ✓ · `SetOSD` ✓ · `DeleteOSD` ✓ · `GetOSDOptions` ✓ — see [osd.md](osd.md).

---

## Request / response patterns (unimplemented families)

These regular shapes (ONVIF core, `trt`/`tt`) let any unimplemented config op be built without
re-reading the WSDL — verify the exact config type against onvif.xsd when implementing.

- **`Add<Kind>Configuration`** — Req: `ProfileToken` `tt:ReferenceToken` [1],
  `ConfigurationToken` `tt:ReferenceToken` [1]; Resp: _(empty)_.
- **`Remove<Kind>Configuration`** — Req: `ProfileToken` `tt:ReferenceToken` [1]; Resp: _(empty)_.
- **`Get<Kind>Configurations`** — Req: _(empty)_; Resp: `Configurations` `tt:<Kind>Configuration` [0..*].
- **`Get<Kind>Configuration`** — Req: `ConfigurationToken` `tt:ReferenceToken` [1];
  Resp: `Configuration` `tt:<Kind>Configuration` [1].
- **`Set<Kind>Configuration`** — Req: `Configuration` `tt:<Kind>Configuration` [1],
  `ForcePersistence` `xs:boolean` [1]; Resp: _(empty)_.
- **`Get<Kind>ConfigurationOptions`** — Req: `ConfigurationToken` `tt:ReferenceToken` [0..1],
  `ProfileToken` `tt:ReferenceToken` [0..1]; Resp: `Options` `tt:<Kind>ConfigurationOptions` [1].
- **`GetCompatible<Kind>Configurations`** — Req: `ProfileToken` `tt:ReferenceToken` [1];
  Resp: `Configurations` `tt:<Kind>Configuration` [0..*].

Distinct streaming ops:

- **GetServiceCapabilities** — Req: _(empty)_; Resp: `Capabilities` `trt:Capabilities` [1].
  Full shape below.
- **StartMulticastStreaming / StopMulticastStreaming** — Req: `ProfileToken` `tt:ReferenceToken` [1];
  Resp: _(empty)_.
- **SetSynchronizationPoint** — Req: `ProfileToken` `tt:ReferenceToken` [1]; Resp: _(empty)_.
- **GetGuaranteedNumberOfVideoEncoderInstances** — Req: `ConfigurationToken` `tt:ReferenceToken` [1];
  Resp: `TotalNumber` `xs:int` [1], `JPEG`/`H264`/`MPEG4` `xs:int` [0..1] each.
- **GetAudioOutputs** — Req: _(empty)_; Resp: `AudioOutputs` `tt:AudioOutput` [0..*].

_Source: media.wsdl operation list (fetched 2026-05); family field shapes are stable ONVIF
core (`trt`/`tt`) — verify exact config types against onvif.xsd when implementing._

---

## `trt:Capabilities` (GetServiceCapabilities)

Unlike most service-capability types, Media1's has **two required child
elements** as well as attributes.

Attributes, all `xs:boolean`, all optional:

| Attribute | Meaning |
|-----------|---------|
| `SnapshotUri` | `GetSnapshotUri` supported |
| `Rotation` | rotation configurable on a video source |
| `VideoSourceMode` | `GetVideoSourceModes` / `SetVideoSourceMode` supported |
| `OSD` | OSD configuration supported |
| `TemporaryOSDText` | temporary OSD text supported |
| `EXICompression` | EXI-compressed SOAP supported |

Children:

- **`ProfileCapabilities`** `trt:ProfileCapabilities` [1] — attr
  `MaximumNumberOfProfiles` `xs:int` [0..1].
- **`StreamingCapabilities`** `trt:StreamingCapabilities` [1] — attrs, all
  `xs:boolean` [0..1]: `RTPMulticast`, `RTP_TCP`, `RTP_RTSP_TCP`,
  `NonAggregateControl`, `NoRTSPStreaming`.

> Do not confuse `trt:StreamingCapabilities` with the identically named
> `tt:StreamingCapabilities` that oxvif already parses into
> `StreamingCapabilities` from the *device-level* `GetCapabilities`. The
> device-level one has only `RTPMulticast` / `RTP_TCP` / `RTP_RTSP_TCP`; this
> one adds `NonAggregateControl` and `NoRTSPStreaming`. Media2's version
> (`tr2:StreamingCapabilities`) is different again — see
> [media2.md](media2.md).

_Source: media.wsdl `<wsdl:types>` (fetched 2026-07-28)._
