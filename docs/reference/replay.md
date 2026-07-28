# Replay Control Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver10/replay.wsdl
- **Namespace:** `http://www.onvif.org/ver10/replay/wsdl` (prefix `trp`)
- **ONVIF Profile:** G
- **oxvif status:** ◐ implemented in `src/client/recording.rs` (1 of 4 operations)

---

## Operations

| Operation | Purpose | oxvif | method |
|-----------|---------|:----:|--------|
| GetReplayUri | RTSP URI for stored recording playback | ✓ | `get_replay_uri` |
| GetServiceCapabilities | replay service capabilities | — | — |
| GetReplayConfiguration | read replay config | — | — |
| SetReplayConfiguration | modify replay config | — | — |

---

## Request / response detail (unimplemented)

#### GetServiceCapabilities
- **Req:** _(empty)_ · **Resp:** `Capabilities` `trp:Capabilities` [1]

`trp:Capabilities` — all members are **attributes**, all optional. Complete set:

| Attribute | Type | Meaning |
|-----------|------|---------|
| `ReversePlayback` | `xs:boolean` | reverse playback supported |
| `SessionTimeoutRange` | `tt:FloatList` | supported session-timeout range, as a whitespace-separated min/max pair |
| `RTP_RTSP_TCP` | `xs:boolean` | RTP/RTSP/TCP delivery supported for replay |
| `RTSPWebSocketUri` | `xs:anyURI` | endpoint for RTSP-over-WebSocket, if supported |

Note `SessionTimeoutRange` is a `tt:FloatList` — a *list-typed attribute*, not a
child element; parse by splitting on whitespace, not by reading a `Min`/`Max`
sub-tree.

#### GetReplayConfiguration
- **Req:** _(empty)_ · **Resp:** `Configuration` `tt:ReplayConfiguration` [1]

#### SetReplayConfiguration
- **Req:** `Configuration` `tt:ReplayConfiguration` [1] · **Resp:** _(empty)_

**`tt:ReplayConfiguration`** — `SessionTimeout` `xs:duration` [1] (+ extensions).

For reference, the implemented op:
**GetReplayUri** — Req: `StreamSetup` `tt:StreamSetup` [1]; `RecordingToken` `tt:ReferenceToken` [1];
Resp: `Uri` `xs:anyURI` [1].

_Source: replay.wsdl `<wsdl:types>` (fetched 2026-05)._
