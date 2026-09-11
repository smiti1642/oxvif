# Audio and metadata migration

[English](audio-metadata.md) | [繁體中文](audio-metadata_zh.md)

Applies to the unreleased next minor version, not the published 0.16.0 API.

| Section | Purpose |
| --- | --- |
| [Metadata](#metadata) | Rust and JSON migration |
| [Audio](#audio) | Service vocabulary and option values |
| [Synthetic contract](#synthetic-contract) | Mock behavior and limits |
| [Verification](#verification) | Evidence and remaining work |

## Metadata

`MetadataConfiguration` replaces `multicast_address` and `multicast_port` with
`multicast: MulticastConfiguration` and `session_timeout: String`.
`mock::MetadataEntry` similarly uses `multicast: MulticastEntry` and
`session_timeout`. Update Rust struct literals and serialized snapshots.

A complete JSON value has this shape:

```json
{
  "token": "MetaConf_1",
  "name": "Entrance metadata",
  "use_count": 1,
  "analytics": true,
  "ptz_status": false,
  "ptz_position": true,
  "multicast": {
    "address": "239.0.1.10",
    "port": 40010,
    "ttl": 1,
    "auto_start": false
  },
  "session_timeout": "PT60S"
}
```

These are example values, not defaults to insert into old device data. Recover
TTL, AutoStart and timeout from a fresh device read or a known complete snapshot.
Old flattened-only JSON fails deserialization; missing information is not invented.
Mock entries additionally retain their two PTZ capability fields.

Reads require Name, UseCount, the complete multicast block and session timeout.
Malformed/duplicate modeled fields fail explicitly. The multicast address must
be an explicit valid IPv4/IPv6 address; the public type cannot represent an
address-less IPAddress block. Writes reject an empty token, malformed IP,
port above 65535, TTL above 255 or invalid nonnegative duration before transport.
The shared serializer now emits the matching IPv4/IPv6 element.

The setter emits PTZStatus before Analytics and includes multicast/session timeout.
Media2 deprecates/ignores the timeout, but it is still required on the wire.
AutoStart is a readonly persistent-stream indication, not a start command.

This is a modeled subset, **not an arbitrary lossless editor**. Events filters,
compression, analytics-engine configuration, sensor/geo/shape settings and vendor
extensions are not represented. Do not use a read-modify-write cycle to preserve
those unmodeled settings on a real camera.

## Audio

Media1 and Media2 codec strings are not interchangeable. Query options from the
target service before writing. Public `AudioEncoding::Other(String)` preserves
device-provided values; no automatic real-camera codec conversion was added.

The factory mock uses G711 as its explicit µ-law convention: its Media2 view is
PCMU. AAC becomes MP4A-LATM. G726 keeps the ONVIF G726 name, with the bitrate
selecting the variant. This does not claim that arbitrary Media1 G711 is µ-law.

Audio options now emit one integer per repeated Items element. The client reads
every repeated item; legacy whitespace-separated lists remain accepted.
This fixes the earlier loss of all but the first repeated item.

## Synthetic contract

All 15 audio/metadata operations in AM1 use scoped requests. Media2 list and
options selectors validate configuration/profile identity; unknown references
fault rather than returning an empty success. Generic options are the catalogue
union, not a promise that every configuration supports every combination.
All existing profiles are logically compatible; physical routing conflicts and
unsupported metadata/output/decoder profile bindings are not implemented.

Audio setters require a complete modeled candidate and a combination advertised
for that configuration. No video-specific bitrate adaptation is applied to audio.
Media1 requires multicast, session timeout and boolean ForcePersistence.
An omitted Media2 audio multicast retains shared settings and session timeout.
UseCount remains readonly.

Metadata setters store name, PTZ/analytics flags and multicast address/port/TTL.
Omitted optional flags retain their values; the required deprecated timeout is
validated but ignored. PTZ filtering is refused when neither PTZ capability exists.
AutoStart input is validated but ignored; the non-streaming mock reports false.
Imported snapshots claiming active persistent multicast refuse rendering.

Validation precedes one atomic commit/change notification. Refusal preserves all
state and replay recordings. Successful commits retire dependent configuration
and profile recordings, not unrelated services. Storage remains in memory with
an optional caller-owned persistence hook; no RTP or real-camera write is performed.

## Verification

See [AM1 execution evidence](active/mock-fidelity-audio-metadata.md) for measured
tests, external schema validation and limits. Structural validation is not ONVIF
certification or proof of real-device streaming behavior.
