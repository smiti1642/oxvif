# oxvif ONVIF protocol reference (`docs/`)

A per-service catalogue of ONVIF operations, transcribed from the official ONVIF WSDLs,
**for cross-reference while implementing oxvif** — it is not documentation of oxvif's own API
(that lives in the top-level [`README.md`](../README.md)) and is not compiled into the crate.

Each service file lists **every** operation the WSDL defines (including ones oxvif has not
implemented yet), marks oxvif coverage, and — for **unimplemented** operations — gives the
request/response field structure so a new method can be written against the real schema.

## Conventions

- **Status:** ✅ implemented · ◐ partial · ❌ not started.
- **Operation rows:** `✓` = oxvif has a method (named in the row); `—` = not yet.
- **Cardinality:** `[1]` required · `[0..1]` optional · `[0..*]` repeated · `[1..*]` one-or-more · `(attr)` attribute.
- Every file cites its **source WSDL/schema URL**. Anything not verified against the schema is
  marked `(unverified)` rather than guessed.
- oxvif coverage is taken from the top-level README status tables and `src/client/*.rs`.
- **Already-implemented operations are not field-expanded** — the code in `src/client/*.rs` and
  `src/types/*.rs` is the source of truth (avoids doc drift). Field detail is given only for
  operations oxvif has **not** built yet.
- **Cross-service types** (`tt:Config`, `tt:ReferenceToken`, `tt:PTZVector`, …) live once in
  [types.md](types.md); service files link there rather than repeating them.

## Services

### Core (streaming / recording — Profiles S, T, G, M)

| Service | File | WSDL | ns prefix | oxvif |
|---------|------|------|:---------:|:-----:|
| Device Management | [device.md](device.md) | `ver10/device/wsdl` | `tds` | ✅ |
| Media (Media1) | [media1.md](media1.md) | `ver10/media/wsdl` | `trt` | ✅ |
| Media2 | [media2.md](media2.md) | `ver20/media/wsdl` | `tr2` | ✅ |
| PTZ | [ptz.md](ptz.md) | `ver20/ptz/wsdl` | `tptz` | ✅ |
| Audio (in Media1/2) | [audio.md](audio.md) | `ver10/media` + `ver20/media` | `trt`/`tr2` | ✅ |
| Imaging | [imaging.md](imaging.md) | `ver20/imaging/wsdl` | `timg` | ✅ |
| OSD (in Media1/2) | [osd.md](osd.md) | `ver10/media` + `ver20/media` | `trt`/`tr2` | ✅ |
| Events | [events.md](events.md) | `ver10/events/wsdl` | `tev` / `wsnt` | ✅ |
| Recording Control | [recording.md](recording.md) | `ver10/recording.wsdl` | `trc` | ✅ |
| Recording Search | [search.md](search.md) | `ver10/search.wsdl` | `tse` | ✅ |
| Replay Control | [replay.md](replay.md) | `ver10/replay.wsdl` | `trp` | ✅ |
| WS-Discovery | [discovery.md](discovery.md) | WS-Discovery 1.1 | `d`/`dn` | ◐ |

### Not yet started (ROADMAP + Profiles A/C/D)

| Service | File | WSDL | ns prefix | Profile | oxvif |
|---------|------|------|:---------:|:-------:|:-----:|
| Analytics | [analytics.md](analytics.md) | `ver20/analytics/wsdl` | `tan` | T/M | ❌ |
| DeviceIO | [deviceio.md](deviceio.md) | `ver10/deviceio.wsdl` | `tmd` | T | ❌ |
| Receiver | [receiver.md](receiver.md) | `ver10/receiver.wsdl` | `trv` | — | ❌ |
| Access Control (PACS) | [accesscontrol.md](accesscontrol.md) | `ver10/pacs` | `tac` | C | ❌ |
| Door Control | [doorcontrol.md](doorcontrol.md) | `ver10/pacs` | `tdc` | C | ❌ |
| Credential | [credential.md](credential.md) | `ver10/credential/wsdl` | `tcr` | C | ❌ |
| Access Rules | [accessrules.md](accessrules.md) | `ver10/accessrules/wsdl` | `tar` | A | ❌ |
| Schedule | [schedule.md](schedule.md) | `ver10/schedule/wsdl` | `tsc` | A | ❌ |
| Authentication Behavior | [authenticationbehavior.md](authenticationbehavior.md) | `ver10/authenticationbehavior/wsdl` | `tab` | A | ❌ |

> Profiles: **S** video streaming · **T** advanced streaming (H.265, analytics, OSD) ·
> **G** recording & playback · **M** metadata/analytics · **A** access-rules/schedule ·
> **C** physical access control / door control · **D** access-control configuration.

---

## Attribution & licensing

These catalogues are **derived from the publicly published ONVIF® WSDL/XSD
schemas** (each file cites its source URL) purely as an implementation reference
for interoperability. They transcribe *interface facts* — operation names, field
names, types, and cardinality — and paraphrase each operation's purpose; they do
**not** reproduce the ONVIF specification prose, and no raw `.wsdl`/`.xsd` files
are redistributed here. This directory is excluded from the published crate
(`exclude = ["docs/"]` in `Cargo.toml`).

ONVIF® is a trademark of ONVIF, Inc. oxvif is an independent project and is
**not affiliated with, endorsed by, or certified by ONVIF**.
