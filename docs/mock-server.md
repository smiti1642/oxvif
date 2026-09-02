# oxvif mock ONVIF device — reference

**English** | [繁體中文](mock-server_zh.md)

A complete reference for `oxvif::mock`: the in-process [`MockTransport`] and the
bound-port [`MockServer`]. It documents what the mock answers, what it stores,
what you can change, and — explicitly — what it does **not** model.

Every statement here is checked against the code it describes, and the source
symbol is named so you can verify it. Where a behaviour is a deliberate
simplification rather than a fidelity claim, it says so: **an undocumented
omission is a bug, a documented one is a design decision.**

- **Audience** — anyone driving the mock: oxvif's own tests, a downstream Rust
  crate, or a non-Rust ONVIF client (Frigate, ODM, gSOAP, a C++ conformance
  suite) pointed at the bound port.
- **Version** — 0.16.0.
- **Feature flags** — `mock` for the transport, `mock-server` for the HTTP
  server. This crate has **no default features**; nothing below compiles
  without one of those two.

## Quick navigation

| Section | What it answers |
| --- | --- |
| [1. Quick start](#1-quick-start) | How do I run the in-process or HTTP mock? |
| [2. Request routing](#2-request-routing) | How are paths, service URLs, and namespaces dispatched? |
| [3. Envelope and namespace contract](#3-envelope-and-namespace-contract) | What SOAP/XML shape does the mock guarantee? |
| [4. Authentication](#4-authentication) | Which authentication behavior is modeled? |
| [5. State model](#5-state-model) | Which services share mutable state? |
| [6. Seeded fixture](#6-seeded-fixture) | What device, media, PTZ, audio, and recording data exists initially? |
| [7. Operation reference](#7-operation-reference) | Which operations are stateful, static, or unsupported? |
| [8. Worked examples](#8-worked-examples) | What do representative requests and responses look like? |
| [9. Error model](#9-error-model) | Which SOAP fault shapes and codes are emitted? |
| [10. Fault injection](#10-fault-injection-and-control-endpoints) | How can tests force transport and protocol failures? |
| [11. Changing the device](#11-changing-the-device) | How can a test customize state and responders? |
| [12. Guarantees](#12-what-is-guaranteed-and-by-which-test) | Which tests enforce each fidelity claim? |
| [13. Known limitations](#13-known-limitations) | What is deliberately simplified or not implemented? |
| [14. Extending the mock](#14-extending-the-mock) | How should contributors add behavior safely? |

---

## 1. Quick start

### 1.1 In-process transport (`feature = "mock"`)

No sockets, no `axum`, no runtime beyond the one the client already uses. This
is the fast path and what oxvif's own unit tests use.

```rust
use std::sync::Arc;
use oxvif::{OnvifClient, mock::MockTransport};

let client = OnvifClient::new("http://mock")
    .with_transport(Arc::new(MockTransport::new()));
let info = client.get_device_info().await?;
assert_eq!(info.manufacturer, "oxvif-mock");
```

`MockTransport` is cheap to clone and clones share one device state and one
fault queue (`src/mock/transport.rs`).

### 1.2 Bound-port HTTP server (`feature = "mock-server"`)

A real TCP listener, for when a test — or another process, or a non-Rust
client — needs an actual endpoint.

```rust
use oxvif::mock::MockServer;

let server = MockServer::start().await?;          // ephemeral 127.0.0.1 port
let client = oxvif::OnvifClient::new(server.device_url());
```

The server runs on a background task and **shuts down when the `MockServer` is
dropped** — keep the binding alive for as long as you need it. `MockServer::start()`
binds `127.0.0.1:0`; use `MockServer::builder().port(8080)` for a fixed port
(`src/mock/server.rs`).

### 1.3 Builder options

| Method | Default | Effect |
|---|---|---|
| `.port(u16)` | `0` (ephemeral) | TCP port to bind. |
| `.initial_state(DeviceState)` | factory defaults | Seed the whole device. |
| `.on_change(ChangeHook)` | none | Fired after every mutation — the seam for persistence. The server itself never touches the filesystem. |
| `.enforce_auth(bool)` | `false` | Require WS-Security `PasswordDigest`. |
| `.discoverable(Vec<String>)` | off | Answer WS-Discovery `Probe` on UDP `3702` with the given scopes. |
| `.replay(FixtureStore)` | none | Serve a recorded camera clone (`metamorph` feature). |

`.discoverable()` is **best-effort**: if the `:3702` bind fails (port in use,
sandboxed CI) the HTTP server still starts, just undiscoverable. At most one
discoverable server per host.

---

## 2. Request routing

### 2.1 The URL path is not used for routing

Every `POST` to any path is handled by one axum route, `/{*path}`
(`src/mock/server.rs`). Dispatch keys **entirely on the SOAP action**, which
SOAP 1.2 carries in the `Content-Type` header:

```
Content-Type: application/soap+xml; charset=utf-8; action="http://www.onvif.org/ver10/device/wsdl/GetHostname"
```

`helpers::extract_action` splits that out. Consequences worth knowing:

- Posting a Media action to `/onvif/device` works. The mock will not object.
- **A missing or malformed `action` parameter yields the "Not implemented"
  fault**, not a 404 and not a hint about the path.
- The service URLs the mock advertises (below) are cosmetic — they exist so a
  client that follows `GetCapabilities` behaves realistically.

### 2.2 Advertised service URLs

`GetCapabilities` and `GetServices` return these, relative to the server's base
(`src/mock/services/device.rs`):

| Service | XAddr |
|---|---|
| Device | `{base}/onvif/device` |
| Media (1) | `{base}/onvif/media` |
| Media2 | `{base}/onvif/media2` |
| PTZ | `{base}/onvif/ptz` |
| Imaging | `{base}/onvif/imaging` |
| Events | `{base}/onvif/events` |
| Recording | `{base}/onvif/recording` |
| Search | `{base}/onvif/search` |
| Replay | `{base}/onvif/replay` |

### 2.3 Namespace → dispatcher

`dispatch()` in `src/mock/dispatch.rs` selects a sub-dispatcher by action
namespace, **not** by the operation name — nine services share the operation
name `GetServiceCapabilities`, which is the whole reason:

| Action prefix | Dispatcher | Operations |
|---|---|---|
| `…/ver10/device/wsdl/` | `dispatch_device` | 38 |
| `…/ver10/deviceio/wsdl/` | `dispatch_device_io` | 1 |
| `…/ver10/media/wsdl/` | `dispatch_media` | 32 |
| `…/ver20/media/wsdl/` | `dispatch_media2` | 26 |
| `…/ver20/ptz/wsdl/` | `dispatch_ptz` | 27 |
| `…/ver20/imaging/wsdl/` | `dispatch_imaging` | 8 |
| `…/events/wsdl/` or `docs.oasis-open.org/wsn/` | `dispatch_events` | 8 |
| `…/ver10/recording/wsdl/` | `dispatch_recording` | 11 |
| `…/ver10/search/wsdl/` | `dispatch_search` | 4 |
| `…/ver10/replay/wsdl/` | `dispatch_replay` | 2 |

**157 operations total** — unchanged in 0.15.0; `GetDigitalInputs` moved from
`dispatch_device` to `dispatch_device_io`, it was not added. The `deviceio`
prefix is lowercase because that is how `deviceio.wsdl` spells its soapActions,
while the elements it declares are in `…/ver10/deviceIO/wsdl`.

Events is doubly irregular: its action URIs carry a
portType segment *and* a `Request` suffix, so its operation names are
`GetServiceCapabilitiesRequest`, `PullMessagesRequest`, and so on.

An action that matches no arm returns:

```xml
<s:Fault>
  <s:Code><s:Value>s:Receiver</s:Value></s:Code>
  <s:Reason><s:Text xml:lang="en">Not implemented: {action}</s:Text></s:Reason>
</s:Fault>
```

…and logs `[WARN] unhandled action:` to stderr.

---

## 3. Envelope and namespace contract

Every response is a SOAP 1.2 envelope built by `helpers::soap`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:tt="http://www.onvif.org/ver10/schema"
            {service namespace}>
  <s:Body>…</s:Body>
</s:Envelope>
```

`xmlns:s` and `xmlns:tt` are always present; each handler adds its own service
namespace. Two rules are now **mechanically enforced across all 157 operations**
(`src/mock/dispatch.rs`):

| Guard | Rule |
|---|---|
| `no_response_declares_an_attribute_twice` | No repeated attribute name in the envelope start-tag (XML 1.0 §3.1). |
| `every_response_binds_the_prefixes_it_uses` | Every element prefix used is declared somewhere in the document. |

**Both existed as real defects until 0.15.0**, and neither could fail a test
here: oxvif's `find_response` matches on *local* name and quick-xml enforces
neither rule, so the entire suite was green against XML a conforming parser
rejects. They were found by feeding captured responses to a strict parser.
If you consume the mock from gSOAP, `lxml`, or anything else namespace-strict,
this is the section that matters to you.

Namespace prefixes the mock emits, and their bindings (`helpers::namespace_for`):

| Prefix | Namespace |
|---|---|
| `tt` | `http://www.onvif.org/ver10/schema` |
| `tds` | `http://www.onvif.org/ver10/device/wsdl` |
| `trt` | `http://www.onvif.org/ver10/media/wsdl` |
| `tr2` | `http://www.onvif.org/ver20/media/wsdl` |
| `tptz` | `http://www.onvif.org/ver20/ptz/wsdl` |
| `timg` | `http://www.onvif.org/ver20/imaging/wsdl` |
| `trc` | `http://www.onvif.org/ver10/recording/wsdl` |
| `tse` | `http://www.onvif.org/ver10/search/wsdl` |
| `trp` | `http://www.onvif.org/ver10/replay/wsdl` |
| `tev` | `http://www.onvif.org/ver10/events/wsdl` |
| `wsnt` | `http://docs.oasis-open.org/wsn/b-2` |
| `tns1` | `http://www.onvif.org/ver10/topics` (declared on the element, in event topic sets) |

---

## 4. Authentication

Off by default, so a credential-less client works immediately. Enable with
`MockTransport::with_auth()` or `MockServerBuilder::enforce_auth(true)`.

When on, the mock validates WS-Security **`PasswordDigest`**
(`src/mock/auth.rs`) — the same `Base64(SHA1(nonce + created + password))`
construction a real device uses.

- **Seeded credentials**: `admin` / `admin` (Administrator) and
  `operator` / `operator` (Operator).
- **One action is exempt**: `GetSystemDateAndTime`. The ONVIF spec requires it
  to answer unauthenticated, because a client needs the device clock to build
  a valid digest in the first place.
- HTTP Digest is **not** implemented. A client configured for HTTP Digest only
  will fail against the mock even with correct credentials.

---

## 5. State model

`DeviceState` (`src/mock/state.rs`) is a flat serde struct — 35 fields: 32
persisted, 3 runtime-only. `MockState` wraps it in a lock exposing
`read()`, `modify()`, `modify_returning()` and `set_on_change()`.

Every persisted field carries `#[serde(default = …)]`, so a partial JSON
snapshot loads and the rest falls back to the factory fixture.

| Field | Type | Seeded | Written by |
|---|---|---|---|
| `info` | `DeviceInfo` | yes | — (read-only) |
| `hostname` | `String` | `"mock-camera"` | `SetHostname` |
| `hostname_from_dhcp` | `bool` | `false` | `SetHostname` |
| `users` | `Vec<MockUser>` | 2 | `CreateUsers`, `DeleteUsers`, `SetUser` |
| `scopes` | `Vec<String>` | yes | `SetScopes` |
| `timezone` | `String` | yes | `SetSystemDateAndTime` |
| `daylight_savings` | `bool` | `false` | `SetSystemDateAndTime` |
| `dns` | `DnsState` | yes | `SetDNS` |
| `ntp` | `NtpState` | yes | `SetNTP` |
| `gateway_ipv4` | `Vec<String>` | yes | `SetNetworkDefaultGateway` |
| `discovery_mode` | `String` | `"Discoverable"` | `SetDiscoveryMode` |
| `imaging_sources` | `Vec<ImagingState>` | 2 | `SetImagingSettings`, `Move`, `Stop` |
| `ptz` | `PtzState` | 2 channels, **keyed by PTZ node token** | 12 PTZ operations |
| `ptz_nodes` | `Vec<PtzNodeEntry>` | 2 | — (read-only) |
| `ptz_configs` | `Vec<PtzConfigEntry>` | 2 | `SetConfiguration` |
| `interface` | `NetworkInterfaceState` | yes | `SetNetworkInterfaces` |
| `protocols` | `Vec<NetworkProtocolState>` | yes | `SetNetworkProtocols` |
| `osd` | `OsdState` | yes | `CreateOSD`, `SetOSD`, `DeleteOSD` |
| `profiles` | `ProfilesState` | 4 | profile create/delete, config add/remove |
| `recording` | `RecordingState` | 2 recordings, 2 jobs | 8 Recording operations |
| `video_sources` | `Vec<VideoSourceEntry>` | 2 | — (read-only) |
| `video_source_configs` | `Vec<VideoSourceConfigEntry>` | 2 | `SetVideoSourceConfiguration` (both services) |
| `video_encoders` | `Vec<VideoEncoderState>` | 4 | `SetVideoEncoderConfiguration` (both services) |
| `relay_outputs` | `Vec<RelayOutputState>` | 2 | `SetRelayOutputState`, `SetRelayOutputSettings` |
| `digital_inputs` | `Vec<DigitalInputState>` | 2 | REST simulator only |
| `storage` | `Vec<StorageEntry>` | 3 | `SetStorageConfiguration` |
| `audio_sources` | `Vec<AudioSourceEntry>` | 2 | — (read-only) |
| `audio_source_configs` | `Vec<AudioSourceConfigEntry>` | 2 | — (read-only; no ONVIF setter in oxvif) |
| `audio_encoders` | `Vec<AudioEncoderEntry>` | 2 | `SetAudioEncoderConfiguration` (both services) |
| `audio_outputs` | `Vec<AudioOutputEntry>` | 1 | — (read-only) |
| `audio_decoders` | `Vec<AudioDecoderEntry>` | 1 | — (read-only) |
| `metadata` | `Vec<MetadataEntry>` | 2 | `SetMetadataConfiguration` |
| `event_seq` | `u64` | runtime | `PullMessages` |
| `event_filter` | `Option<Vec<String>>` | runtime | `CreatePullPointSubscription` |
| `pending_io_events` | `Vec<PendingIoEvent>` | runtime | REST simulator |

The last three are `#[serde(skip)]` — per-instance, never persisted.

### 5.1 Media1 and Media2 share one state

They are two views of one device. Any operation present in both dispatchers
reads and writes the same `DeviceState`; only the rendering differs (both
inline whole configurations, but Media1 lists them as siblings of `Name` where
Media2 groups them under `<tr2:Configurations>`, and two of the types differ).
This said *"Media2 emits token references"* until 0.15 — see §8.3.
`tests/mock_media1_media2_agree.rs`
is the standing guard, and the audit records two shipped bugs from getting
this wrong.

---

## 6. Seeded fixture

The factory device is a **two-sensor camera**. That is deliberate: a
single-channel fixture cannot distinguish a handler that honours a token from
one that ignores it, so every per-channel answer would look correct.

**The seed values disagree on purpose.** Where two entries could plausibly
carry the same value, they do not.

### 6.1 Identity

| Field | Value |
|---|---|
| Manufacturer | `oxvif-mock` |
| Model | `MockCam-1080p` |
| Firmware | `1.0.0` |
| Serial | `MOCK-0001` |
| Hardware ID | `1.0` |

### 6.2 Video chain

| Sensor | Source config | Encoder configs | Native resolution |
|---|---|---|---|
| `VS_1` | `VSC_1` (`VSConfig1`) | `VEC_1` `MainStream` 1920×1080, `VEC_2` `SubStream` 704×480 | 2592×1944 |
| `VS_2` | `VSC_2` (`VSConfig2`) | `VEC_3` `MainStream2` 1280×720, `VEC_4` | 1280×720 |

`VEC_1` advertises six resolutions up to 2592×1944; `VEC_3`'s list tops out at
1280×720. **Only `VS_1` advertises H.265.** An assertion that reads a
resolution list or an encoding set therefore fails if the handler answers for
the wrong channel.

### 6.3 Profiles

| Token | Name | Fixed | Source cfg | Encoder cfg | PTZ cfg | Audio cfg |
|---|---|---|---|---|---|---|
| `Profile_1` | `mainStream` | yes | `VSC_1` | `VEC_1` | `PTZConfig_1` | `ASC_1` + `AEC_1` |
| `Profile_2` | `subStream` | no | `VSC_1` | `VEC_2` | `PTZConfig_1` | *(none)* |
| `Profile_3` | `mainStream2` | yes | `VSC_2` | `VEC_3` | `PTZConfig_2` | *(none)* |
| `Profile_4` | `subStream2` | no | `VSC_2` | `VEC_4` | *(none)* | *(none)* |

`fixed="true"` profiles refuse deletion (`ter:DeletionOfFixedProfile`).

`Profile_4` binds no PTZ configuration on purpose: it is the fixture for a
profile that is **not PTZ-capable**, and every PTZ operation on it faults.

### 6.4 PTZ — two heads, one per lens

**A profile does not own a head.** It reaches one through a PTZ configuration:

```
ProfileToken → ProfileEntry.ptz_config_token → PtzConfigEntry.node_token → PtzChannel
```

So the main and the sub stream of one lens are **one** head, which is what a
camera does. Until 0.15 `PtzState` was keyed by *profile*, and moving
`Profile_1` left `Profile_2` reporting its old position.

| Node | Reached by | Spaces | Home | Fixed home | Max presets | Aux |
|---|---|---|---|---|---|---|
| `PTZNode_1` | `Profile_1`, `Profile_2` (lens 1) | all 8 | yes | no | 100 | 2 |
| `PTZNode_2` | `Profile_3` (lens 2) | **zoom only** (4) | no | yes | 8 | 0 |

| Node | Position (pan, tilt, zoom) | Presets | Tours |
|---|---|---|---|
| `PTZNode_1` | 0.0, 0.0, 0.0 | `Home`, `Door` | 1 |
| `PTZNode_2` | 0.0, 0.0, 0.80 | `Lobby`, `Dock`, `Roof` | 0 |

`PTZNode_2` declares **no pan/tilt space at all**, so `AbsoluteMove`,
`RelativeMove` and `ContinuousMove` on `Profile_3` are refused if the request
carries a `<tt:PanTilt>` element — even `x="0" y="0"`, because the question is
whether the vector is present. Its presets therefore carry zero pan and tilt;
they differ from each other, and from lens 1's, in **zoom**.

> **This makes lens 2 unreachable through oxvif's move API.**
> `ptz_absolute_move`, `ptz_relative_move` and `ptz_continuous_move` always emit
> a `<tt:PanTilt>` element. Use `GotoPreset` to position a zoom-only head. That
> is a gap in the *client* against real zoom-only hardware, not a mock quirk —
> `docs/active/ptz-wiring-plan-2026-07.md` §3.5.

### 6.4.1 PTZ configurations

| Token | Node | UseCount | Default spaces | DefaultPTZSpeed | PanTiltLimits | ZoomLimits | Timeout | Options |
|---|---|---|---|---|---|---|---|---|
| `PTZConfig_1` | `PTZNode_1` | 2 | all 6 | 0.5 / 0.5 / 0.5 | ±0.9 × ±0.7 | 0.0–1.0 | `PT10S` | `PT1S`–`PT60S` |
| `PTZConfig_2` | `PTZNode_2` | 1 | the 3 zoom ones | *(absent)* | *(absent)* | 0.1–0.95 | `PT30S` | `PT5S`–`PT30S` |

**The absences are load-bearing.** An `Option` field is only observable as
`None`-versus-`Some` if some fixture exercises each arm; `CLAUDE.md`'s batch
mutation for `Option` parse helpers has nothing to redden otherwise.

The absolute pan/tilt space element is spelled
`DefaultAbsolutePantTiltPositionSpace` — `Pant`, double `t`. That is ONVIF's own
typo in `onvif.xsd` and it is normative.

### 6.4.2 Audio — one catalogue, two shapes

Two of everything that can be addressed by a token, disagreeing on every value
an assertion reads.

| Source | Channels | | Source config | Name | UseCount | Reads |
|---|---|---|---|---|---|---|
| `AudioSrc_1` | 1 | | `ASC_1` | `AudioSourceConfig1` | 1 | `AudioSrc_1` |
| `AudioSrc_2` | 2 | | `ASC_2` | `AudioSourceConfig2` | 0 | `AudioSrc_2` |

| Encoder | Encoding | Bitrate | SampleRate | Multicast | SessionTimeout | Options |
|---|---|---|---|---|---|---|
| `AEC_1` | `G711` | 64 | 8 | `239.0.0.5:40002` ttl 5 | `PT60S` | 1 row: G711 |
| `AEC_2` | `AAC` | 128 | 48 | `239.0.0.6:40006` ttl 3 | `PT30S` | 2 rows: AAC, G726 |

Plus one `AOC_1` audio output (`AudioOut_1`, level 50) and one `ADC_1` decoder.

**`Multicast` and `SessionTimeout` are on both encoders on purpose.** Both are
*required* members of `tt:AudioEncoderConfiguration`, so a conformant Media1
device always sends them, and this mock is the conformant device.

`Profile_1` is the only profile with audio (§6.3). Every profile was unbound
while the family was a string literal, so both renderers emitted nothing and
agreed perfectly.

### 6.5 The two option shapes

`GetAudioEncoderConfigurationOptions` nests **differently on the two services**,
and the mock had it backwards on both:

```text
Media1  Response/Options   tt:AudioEncoderConfigurationOptions   ← a wrapper
                /Options   tt:AudioEncoderConfigurationOption    ← repeated, the entry
Media2  Response/Options   tt:AudioEncoder2ConfigurationOptions  ← repeated, IS the entry
```

Media1's response was flat (Media2's shape) and Media2's was wrapped (Media1's).
`AudioEncoderConfigurationOptions::from_xml` read only the flat one, so Media1
agreed with the parser and with no real device, and Media2 agreed with neither.
Both were fixed in 0.15; the parser now reads either.

**That makes a client-level test unable to tell the shapes apart**, which is why
`audio_options_use_media1_nesting_on_the_wire` and its Media2 twin assert raw
bytes. Same reason as the PTZ `Pant` spelling test, and found the same way — by
a perturbation that came back green.

### 6.6 Storage

| Token | Type | LocalPath | StorageUri | User |
|---|---|---|---|---|
| `SD_01` | `LocalStorage` | `/mnt/sd` | — | — |
| `NAS_01` | `NFS` | `/mnt/nas` | `nfs://192.168.1.50/records` | `recorder` |
| `CIFS_01` | `CIFS` | — | `smb://192.168.1.60/cam` | — |

Each optional field is present on some entries and absent on others, so an
assertion on any one of them can fail on its own.

### 6.7 Metadata (Media2)

| Token | Name | Analytics | PTZ status / position | Multicast | Pan/tilt · zoom status supported |
|---|---|---|---|---|---|
| `MetaConf_1` | `MetadataConfig` | true | false / true | `239.0.1.10:40010` | true · false |
| `MetaConf_2` | `MetadataMinimal` | false | true / false | *(no group)* | false · true |

`tt:MetadataConfiguration/Multicast` is **required**, so both configurations
send the block; `MetaConf_2` omits the optional `Address/IPv4Address` inside it
and reports `AutoStart` false, which is how a conformant device says "no group".
The last column is `Options/PTZStatusFilterOptions`, answered per token by
`GetMetadataConfigurationOptions`.

### 6.8 Recording

| Recording | Tracks | Bounds | Status |
|---|---|---|---|
| `Rec_001` | `VIDEO001` (Video) | 2026-01-01 → 2026-04-01 | `Stopped` |
| `Rec_002` | *(none)* | 2026-05-01 → 2026-06-01 | `Recording` |

| Job | Recording | Mode |
|---|---|---|
| `Job_001` | `Rec_001` | `Active` |
| `Job_002` | `Rec_002` | `Idle` |

### 6.9 I/O

`RelayOutput_1` (Bistable, idle closed), `RelayOutput_2` (Monostable, `PT1S`,
idle open); `DigitalInput_1` (idle closed), `DigitalInput_2` (idle open).

### 6.10 Users

`admin` / `admin` (Administrator), `operator` / `operator` (Operator).

---

## 7. Operation reference

**Legend** — ● state-backed (reads or writes `DeviceState`) · ○ static fixture
(same answer every time) · **T** answers per token, and the seeded fixture
makes two tokens disagree.

### 7.1 Device — 38 operations

| Operation | | Notes |
|---|---|---|
| `GetServiceCapabilities` | ○ | |
| `GetCapabilities`, `GetServices` | ○ | Emit the base URL. |
| `GetDeviceInformation` | ● | |
| `GetSystemDateAndTime` | ● | Clock is the **real current time**, all six components. |
| `SetSystemDateAndTime` | ● | Writes timezone + DST. |
| `GetHostname` / `SetHostname` | ● | |
| `GetNTP` / `SetNTP` | ● | |
| `GetDNS` / `SetDNS` | ● | |
| `GetScopes` / `SetScopes` | ● | |
| `GetUsers`, `CreateUsers`, `DeleteUsers`, `SetUser` | ● | |
| `GetNetworkInterfaces` / `SetNetworkInterfaces` | ● | Writes `Enabled`, `FromDHCP`, `Address`, `PrefixLength`, `MTU`. |
| `GetNetworkProtocols` / `SetNetworkProtocols` | ● | |
| `GetNetworkDefaultGateway` / `SetNetworkDefaultGateway` | ● | |
| `GetDiscoveryMode` / `SetDiscoveryMode` | ● | Only `Discoverable` / `NonDiscoverable` accepted. |
| `GetRelayOutputs`, `SetRelayOutputState`, `SetRelayOutputSettings` | ● | See §13 on `SetRelayOutputState`. Device-service operations even though DeviceIO binds them too — `deviceio.wsdl` types their messages with the `tds:` elements. |
| `GetStorageConfigurations` / `SetStorageConfiguration` | ● | Unknown token faults; token-less Set creates. |
| `SendAuxiliaryCommand`, `GetSystemLog`, `GetSystemUris`, `StartFirmwareUpgrade`, `StartSystemRestore`, `SystemReboot`, `SetSystemFactoryDefault` | ○ | Acknowledged, nothing modelled. |

### 7.1a DeviceIO — 1 operation

Answered at `{base}/onvif/deviceio`, advertised by both `GetCapabilities`
(`Capabilities/Extension/DeviceIO`) and `GetServices`. The action segment is
lowercase `deviceio`; the elements are in `…/ver10/deviceIO/wsdl`. Shares one
`DeviceState` with §7.1 — `dispatch_device_io` reaches the same
`services/device.rs` renderer.

| Operation | | Notes |
|---|---|---|
| `GetDigitalInputs` | ● | Driven by the REST simulator. Was answered at the device endpoint in `tds:` until 0.15.0. |

### 7.2 Media1 — 32 operations

| Operation | | Notes |
|---|---|---|
| `GetProfiles`, `GetProfile`, `CreateProfile`, `DeleteProfile` | ● | |
| `GetVideoSources`, `GetVideoSourceConfigurations` | ● | |
| `GetVideoSourceConfiguration` / `SetVideoSourceConfiguration` | ● **T** | Shared writer with Media2. |
| `GetVideoSourceConfigurationOptions` | ● **T** | |
| `GetVideoEncoderConfigurations`, `GetVideoEncoderConfiguration`, `SetVideoEncoderConfiguration` | ● **T** | |
| `GetVideoEncoderConfigurationOptions` | ● **T** | Nested `Extension` shape — see §13. |
| `AddVideoEncoderConfiguration`, `RemoveVideoEncoderConfiguration`, `AddVideoSourceConfiguration`, `RemoveVideoSourceConfiguration` | ● | Profile bindings, visible to Media2. |
| `GetOSD`, `GetOSDs`, `SetOSD`, `CreateOSD`, `DeleteOSD` | ● **T** | |
| `GetStreamUri`, `GetSnapshotUri` | ○ | One canned URI for every profile. |
| `GetAudioSources`, `GetAudioSourceConfigurations`, `GetAudioEncoderConfigurations` | ● | Whole-catalogue reads. |
| `GetAudioEncoderConfiguration`, `GetAudioEncoderConfigurationOptions` | ● **T** | Per configuration; unknown token faults. Options use Media1's **wrapped** nesting — §6.5. |
| `SetAudioEncoderConfiguration` | ● | **Refuses a body without `Multicast` or `SessionTimeout`** — both are required by `tt:AudioEncoderConfiguration`. §13.3. |
| `GetOSDOptions`, `GetServiceCapabilities` | ○ | |

### 7.3 Media2 — 26 operations

| Operation | | Notes |
|---|---|---|
| `GetProfiles`, `CreateProfile`, `DeleteProfile` | ● | `tr2:DeleteProfile` names its token element `Token`, not `ProfileToken`. |
| `AddConfiguration`, `RemoveConfiguration` | ● | Resolves **every** child's kind before writing any, so an unmodelled type cannot half-apply. |
| `GetVideoSourceConfigurations`, `SetVideoSourceConfiguration`, `GetVideoSourceConfigurationOptions` | ● **T** | |
| `GetVideoEncoderConfigurations`, `SetVideoEncoderConfiguration`, `GetVideoEncoderConfigurationOptions` | ● **T** | |
| `GetMetadataConfigurations` | ● **T** | `ConfigurationToken` is a **filter** — no match yields an empty list, not a fault. |
| `GetMetadataConfigurationOptions` | ● **T** | Addressed read — no match **faults**. |
| `SetMetadataConfiguration` | ● | Unknown token faults. |
| `GetStreamUri`, `GetSnapshotUri`, `GetVideoEncoderInstances` | ○ | |
| `GetAudioSourceConfigurations`, `GetAudioEncoderConfigurations`, `GetAudioOutputConfigurations`, `GetAudioDecoderConfigurations` | ● | Same state Media1 serves, in Media2's shapes. |
| `GetAudioEncoderConfigurationOptions` | ● **T** | Media2's **flat** nesting — §6.5. |
| `SetAudioEncoderConfiguration` | ● | Shares Media1's writer. Not required to carry `Multicast`, and cannot carry `SessionTimeout` — §13.3. |
| `GetVideoSourceModes` | ○ | Declared stub — §13. |
| `SetVideoSourceMode` | — | **Always faults** (`ter:ActionNotSupported`). The mock does not model sensor modes and will not claim it does — §13.1. |

### 7.4 PTZ — 27 operations

**Every per-profile operation requires `ProfileToken`.** A missing token faults
(`env:Sender` / `NoProfileToken-…`); a token naming no profile faults
(`ter:NoProfile` / `NoSuchProfile-…`); and a profile that binds no PTZ
configuration faults (`ter:NoConfig` / `NoPTZConfig-…-5619`), because it
addresses no head at all — §6.4.

| Operation | | Notes |
|---|---|---|
| `GetStatus`, `GetPresets`, `SetPreset`, `RemovePreset`, `GotoPreset` | ● **T** | |
| `AbsoluteMove`, `RelativeMove`, `ContinuousMove`, `Stop` | ● **T** | Moves are instantaneous; there is no motion model. |
| `GotoHomePosition`, `SetHomePosition` | ● **T** | |
| `GetPresetTours`, `GetPresetTour`, `GetPresetTourOptions`, `CreatePresetTour`, `ModifyPresetTour`, `OperatePresetTour`, `RemovePresetTour` | ● **T** | |
| `GetNodes`, `GetConfigurations` | ● | Whole-catalogue reads; no token to discriminate on. |
| `GetNode`, `GetConfiguration`, `GetConfigurationOptions`, `SetConfiguration` | ● **T** | Addressed by **node** or **configuration** token, not by profile. An unknown token faults. `GetConfigurationOptions` reports the coordinate spaces of the node its configuration drives, so `PTZConfig_2` answers with zoom slots only. |
| `GetCompatibleConfigurations` | ● **T** | The profile's bound configuration — or an **empty list**, not a fault, for a profile that is not PTZ-capable. |
| `SendAuxiliaryCommand`, `GetServiceCapabilities` | ○ | |

### 7.5 Imaging — 8 operations

`GetImagingSettings`, `SetImagingSettings`, `GetOptions`, `GetStatus`,
`GetMoveOptions`, `Move`, `Stop` are all ● **T**, keyed by
`VideoSourceToken` (`VS_1` / `VS_2`, which disagree).
`GetServiceCapabilities` is ○.

### 7.6 Events — 8 operations

| Operation | | Notes |
|---|---|---|
| `CreatePullPointSubscriptionRequest` | ● | Stores the topic filter. |
| `PullMessagesRequest` | ● | Emits a periodic synthetic stream plus any pending REST-injected I/O events; `event_seq` increments per call. |
| `GetEventPropertiesRequest` | ○ | Topic set. Declares `tns1` on the element. |
| `SubscribeRequest`, `RenewRequest`, `UnsubscribeRequest`, `SetSynchronizationPointRequest`, `GetServiceCapabilitiesRequest` | ○ | |

### 7.7 Recording / Search / Replay — 17 operations

| Operation | | Notes |
|---|---|---|
| `GetRecordings`, `CreateRecording`, `DeleteRecording`, `CreateTrack`, `DeleteTrack` | ● | Deleting a recording deletes its jobs. |
| `GetRecordingJobs`, `CreateRecordingJob`, `SetRecordingJobMode`, `DeleteRecordingJob`, `GetRecordingJobState` | ● **T** | |
| `GetRecordingSearchResults` | ● | |
| `GetReplayUri` | ● **T** | Faults on a token naming no recording. |
| `FindRecordings` | ○ | One search token; no cursor — see §13. |
| `EndSearch`, three × `GetServiceCapabilities` | ○ | `EndSearchResponse` carries the required `Endpoint`, read from the same clock as `GetSystemDateAndTime`. It was an empty body until 0.15.0. |

---

## 8. Worked examples

All captured from the real dispatcher and pretty-printed; the envelope
attributes and element order are verbatim. Request fragments show `<s:Body>`
only — the client's envelope declares fourteen prefixes and is elided.

### 8.1 A simple read

**Request** — action `http://www.onvif.org/ver10/device/wsdl/GetHostname`

```xml
<tds:GetHostname/>
```

**Response**

```xml
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:tt="http://www.onvif.org/ver10/schema"
            xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
  <s:Body>
    <tds:GetHostnameResponse>
      <tds:HostnameInformation>
        <tt:FromDHCP>false</tt:FromDHCP>
        <tt:Name>lobby-cam</tt:Name>
      </tds:HostnameInformation>
    </tds:GetHostnameResponse>
  </s:Body>
</s:Envelope>
```

### 8.2 A void write

**Request** — action `…/device/wsdl/SetHostname`

```xml
<tds:SetHostname><tds:Name>lobby-cam</tds:Name></tds:SetHostname>
```

**Response**

```xml
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:tt="http://www.onvif.org/ver10/schema"
            xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
  <s:Body><tds:SetHostnameResponse/></s:Body>
</s:Envelope>
```

That `xmlns:tds` declaration is new in 0.15.0. Before it, the prefix was
unbound and the document was not namespace-well-formed (§3).

### 8.3 Media1 vs Media2 — same state, two shapes

`trt:GetProfiles` inlines whole configurations:

```xml
<trt:Profiles token="Profile_1" fixed="true">
  <tt:Name>mainStream</tt:Name>
  <tt:VideoSourceConfiguration token="VSC_1">
    <tt:Name>VSConfig1</tt:Name>
    <tt:UseCount>2</tt:UseCount>
    <tt:SourceToken>VS_1</tt:SourceToken>
    <tt:Bounds x="0" y="0" width="2592" height="1944"/>
  </tt:VideoSourceConfiguration>
  <tt:VideoEncoderConfiguration token="VEC_1">
    <tt:Name>MainStream</tt:Name>
    <tt:UseCount>1</tt:UseCount>
    <tt:Encoding>H264</tt:Encoding>
    <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
    <tt:Quality>5</tt:Quality>
    <tt:RateControl>
      <tt:FrameRateLimit>25</tt:FrameRateLimit>
      <tt:EncodingInterval>1</tt:EncodingInterval>
      <tt:BitrateLimit>4096</tt:BitrateLimit>
    </tt:RateControl>
    <tt:H264><tt:GovLength>25</tt:GovLength><tt:H264Profile>Main</tt:H264Profile></tt:H264>
    <tt:Multicast>
      <tt:Address><tt:Type>IPv4</tt:Type><tt:IPv4Address>0.0.0.0</tt:IPv4Address></tt:Address>
      <tt:Port>0</tt:Port><tt:TTL>1</tt:TTL><tt:AutoStart>false</tt:AutoStart>
    </tt:Multicast>
    <tt:SessionTimeout>PT0S</tt:SessionTimeout>
  </tt:VideoEncoderConfiguration>
</trt:Profiles>
```

`tr2:GetProfiles` groups the same profile's configurations under one wrapper:

```xml
<tr2:Profiles token="Profile_1" fixed="true">
  <tr2:Name>mainStream</tr2:Name>
  <tr2:Configurations>
    <tr2:VideoSource token="VSC_1">
      <tt:Name>VSConfig1</tt:Name>
      <tt:UseCount>2</tt:UseCount>
      <tt:SourceToken>VS_1</tt:SourceToken>
      <tt:Bounds x="0" y="0" width="2592" height="1944"/>
    </tr2:VideoSource>
    <tr2:AudioSource token="ASC_1">…</tr2:AudioSource>
    <tr2:VideoEncoder token="VEC_1">…</tr2:VideoEncoder>
    <tr2:AudioEncoder token="AEC_1">…</tr2:AudioEncoder>
    <tr2:PTZ token="PTZConfig_1">…</tr2:PTZ>
  </tr2:Configurations>
</tr2:Profiles>
```

**This paragraph used to say `<tr2:Audio>` is the audio encoder reference.** It
is `<tr2:AudioEncoder>`; `Audio` was a name oxvif invented, which the parser,
this mock, a unit fixture and `CLAUDE.md` all agreed on and no device sends.
See the `Fixed` entry in `CHANGELOG.md` for 0.15.0.

**And it used to say the members carried a token and no body**, with the note
that ~~*"the token-only elements above are a simplification of this mock, not the
schema shape"*~~ and that `MediaProfile2::video_source_token` was ~~*"therefore
always `None` here"*~~. Both were true, and both stopped being true in 0.15:
`tr2:ConfigurationSet` types every member as the *full* configuration, so this
mock now inlines it as Media1 does, and `video_source_token` reports `VS_1`.

Two things the shape above still does not share with Media1:

- **The member names.** `VideoSource`, not `VideoSourceConfiguration` — and
  `Name` is `tr2:` here (declared locally in `media2.wsdl`) where Media1's is
  `tt:`.
- **Two of the types.** `VideoEncoder` is `tt:VideoEncoder2Configuration` and
  `AudioEncoder` is `tt:AudioEncoder2Configuration`, which have their own
  member sequences and no `SessionTimeout`. The other three members —
  `VideoSource`, `AudioSource`, `PTZ` — are the *same* types `tt:Profile`
  inlines, and are rendered by the same helpers.

  `tt:VideoEncoder2Configuration` also moves two members Media1 keeps as
  elements into **attributes** — the full opening tag is
  `<tr2:VideoEncoder token="VEC_1" GovLength="25" Profile="Main">`, where
  Media1 nests the same two facts as `<tt:H264><tt:GovLength>…</tt:GovLength>
  <tt:H264Profile>…</tt:H264Profile></tt:H264>`. This mock emitted them as
  `<tt:GovLength>` / `<tt:Profile>` children until 0.15, matching a client bug
  of the same shape; see the `Fixed` entry in `CHANGELOG.md` for 0.15.0.

**The sequence is `VideoSource, AudioSource, VideoEncoder, AudioEncoder,
Analytics, PTZ, …`** — audio source sits between the two video members, and
`tt:Profile` interleaves the same way. Two separate declarations that happen to
agree; neither licenses assuming the other.

### 8.4 Per-channel answers

The same operation with two tokens gives two answers. `VEC_1` (sensor `VS_1`):

```xml
<tt:H264>
  <tt:ResolutionsAvailable><tt:Width>2592</tt:Width><tt:Height>1520</tt:Height></tt:ResolutionsAvailable>
  <tt:ResolutionsAvailable><tt:Width>2560</tt:Width><tt:Height>1440</tt:Height></tt:ResolutionsAvailable>
  <tt:ResolutionsAvailable><tt:Width>2304</tt:Width><tt:Height>1296</tt:Height></tt:ResolutionsAvailable>
  <tt:ResolutionsAvailable><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:ResolutionsAvailable>
  <tt:ResolutionsAvailable><tt:Width>1280</tt:Width><tt:Height>720</tt:Height></tt:ResolutionsAvailable>
  …
</tt:H264>
```

`VEC_3` (sensor `VS_2`) returns a list topping out at 1280×720.

**The response also carries a nested `Extension` copy**, which is where ONVIF
puts the superset:

```
Options/H264                          no BitrateRange
Options/Extension/H264                adds BitrateRange
Options/Extension/Extension/H265      the only place H265 lives
```

A parser reading only the top level silently drops what the extension added.
Prefer the deepest node and fall back outward.

### 8.5 PTZ is per-head

`Profile_1`:

```xml
<tt:PanTilt x="0" y="0" space="http://www.onvif.org/ver10/tptz/PanTiltSpaces/PositionGenericSpace"/>
<tt:Zoom x="0" space="http://www.onvif.org/ver10/tptz/ZoomSpaces/PositionGenericSpace"/>
```

`Profile_3`:

```xml
<tt:PanTilt x="-0.6" y="0.35" space="http://www.onvif.org/ver10/tptz/PanTiltSpaces/PositionGenericSpace"/>
<tt:Zoom x="0.8" space="http://www.onvif.org/ver10/tptz/ZoomSpaces/PositionGenericSpace"/>
```

### 8.6 Optional elements are omitted, not blanked

`GetStorageConfigurations` over the three seeded entries:

```xml
<tds:StorageConfigurations token="SD_01">
  <tt:Data type="LocalStorage"><tt:LocalPath>/mnt/sd</tt:LocalPath></tt:Data>
</tds:StorageConfigurations>
<tds:StorageConfigurations token="NAS_01">
  <tt:Data type="NFS">
    <tt:LocalPath>/mnt/nas</tt:LocalPath>
    <tt:StorageUri>nfs://192.168.1.50/records</tt:StorageUri>
    <tt:User><tt:UserName>recorder</tt:UserName></tt:User>
  </tt:Data>
</tds:StorageConfigurations>
<tds:StorageConfigurations token="CIFS_01">
  <tt:Data type="CIFS"><tt:StorageUri>smb://192.168.1.60/cam</tt:StorageUri></tt:Data>
</tds:StorageConfigurations>
```

An absent value is an absent element. Note that `StorageConfiguration` in
oxvif parses these as `String` with `unwrap_or_default()`, so **an oxvif
client cannot distinguish omitted from empty here**; `MetadataConfiguration`'s
`multicast_address` is `Option` and reads the genuinely optional
`Multicast/Address/IPv4Address`, so there the distinction *is* visible.
(`multicast_port` is not a second instance: `Multicast/Port` is required, so the
mock sends `0` for a configuration with no group.)

### 8.7 A fault

**Request** — `DeleteRecording` with `Rec_999`

```xml
<trc:DeleteRecording><trc:RecordingToken>Rec_999</trc:RecordingToken></trc:DeleteRecording>
```

**Response**

```xml
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:tt="http://www.onvif.org/ver10/schema">
  <s:Body>
    <s:Fault>
      <s:Code><s:Value>ter:NoRecording</s:Value></s:Code>
      <s:Reason><s:Text xml:lang="en">NoSuchRecording-DELREC-5701: Rec_999</s:Text></s:Reason>
    </s:Fault>
  </s:Body>
</s:Envelope>
```

---

## 9. Error model

### 9.1 Shape

SOAP 1.2 `<s:Fault>` with `Code/Value` and `Reason/Text`
(`helpers::resp_soap_fault`). HTTP status is **200** — faults are transported
in the body, as ONVIF devices do.

### 9.2 Reason strings are tagged and unique

Every fault reason carries an operation tag and a numeric id:

```
NoSuchRecording-DELREC-5701: Rec_999
│              │      │      └─ the offending value
│              │      └──────── unique numeric id
│              └─────────────── operation abbreviation
└────────────────────────────── condition
```

This exists so a test can assert *which* operation faulted. A suite that
asserts only `is_err()` cannot tell `DeleteRecording` from `DeleteTrack`, and
oxvif's own testing rules ban that.

### 9.3 Codes in use

| Code | Meaning | Example reason tags |
|---|---|---|
| `env:Sender` | Malformed or missing required input | `NoProfileToken-STATUS-5601`, `NoStorageType-STOR-5801`, `InvalidDiscoveryMode-5551` |
| `ter:NoProfile` | Profile token names nothing | `NoSuchProfile-ABSMOVE-5606` |
| `ter:ProfileExists` | Duplicate profile token on create | |
| `ter:DeletionOfFixedProfile` | `DeleteProfile` on `fixed="true"` | |
| `ter:NoConfig` | Configuration token names nothing | `NoSuchMetadataConfig-SETMETA-5811` |
| `ter:ConfigurationConflict` | Media2 `AddConfiguration` type not modelled | `UnmodelledConfigType-CFG2-5542` |
| `ter:InvalidArgs` | Bad argument to a Media operation | |
| `ter:InvalidArgVal` | Value outside the accepted set | `NoSuchStorage-STOR-5802`, `BadJobMode-SETJOBMODE-5705` |
| `ter:NoRecording` / `ter:NoTrack` / `ter:NoJob` | Recording-family token names nothing | `NoSuchRecording-REPLAY-5709` |
| `ter:ActionNotSupported` | Routed, but deliberately not modelled — see §13.1 | `NotModelled-VSMODE-5813` |
| `s:Receiver` | Unrouted action | `Not implemented: {action}` |

**Known deviation.** The `ter:` and `env:` prefixes in `Code/Value` are QNames
but the mock does not declare those prefixes on the fault envelope. Element
prefixes are all bound (§3); these two live in *text content*, which no parser
resolves automatically, but a client that resolves fault-code QNames itself
will not be able to. Recorded rather than changed, because the correct
expansion is a design question — many real devices emit exactly these strings.

---

## 10. Fault injection and control endpoints

### 10.1 From Rust

```rust
server.inject_fault("GetProfiles", "ter:NoProfile", "injected");
// next action whose URI ends with "GetProfiles" faults, once
server.clear_faults();
```

Single-shot and consumed on first match. `MockTransport` has the same pair.

### 10.2 Over HTTP (`mock-server` only)

| Endpoint | Method | Parameters |
|---|---|---|
| `/admin/inject_fault` | POST | `action` (**required**), `code` (default `s:Receiver`), `reason` (default `Injected fault`) |
| `/admin/clear_faults` | POST | — |
| `/mock/snapshot.jpg` | GET | Generated JPEG |
| `/mock/digital-input/{token}/pulse` | POST | Fires an event, then reverts |
| `/mock/digital-input/{token}/set` | POST | Latches a state |

A missing `action` returns `400`. These exist so a non-Rust client can drive
failure paths.

**There is no authentication on `/admin`.** Bind the mock to loopback (the
default) and do not expose it.

---

## 11. Changing the device

Four seams, in increasing order of intrusiveness.

### 11.1 Mutate state directly

```rust
server.device().modify(|s| {
    s.info.model = "MyCam-4K".into();
    s.video_encoders[0].width = 3840;
});
```

`device()` returns the `MockState`; `read()` for assertions, `modify()` for
changes. This is how tests seed a scenario the ONVIF API cannot express.

### 11.2 Supply a whole `DeviceState`

```rust
let state: DeviceState = serde_json::from_str(&saved)?;   // `serde` feature
let server = MockServer::builder().initial_state(state).start().await?;
```

Every field has a serde default, so a partial JSON document is valid and
unspecified fields fall back to the factory fixture.

### 11.3 Persist on change

```rust
let server = MockServer::builder()
    .on_change(Arc::new(|s: &DeviceState| { /* write to disk */ }))
    .start().await?;
```

The library never touches the filesystem itself. This hook is the only seam.

### 11.4 Interpose a responder

`Chain` / `Responder` / `RequestCtx` (`src/mock/responder.rs`) let you splice
your own handler ahead of the synthetic dispatcher — the mechanism the
`metamorph` replay clone uses.

### 11.5 What you cannot change through the ONVIF API

These are read-only over SOAP; use §11.1:

- `info` — no ONVIF operation sets device information.
- `video_sources` — sensor geometry.
- `digital_inputs` — driven by the REST simulator only.
- `MetadataEntry::pan_tilt_status_supported` / `zoom_status_supported` — device
  capabilities, not part of `tt:MetadataConfiguration`.
- `MetadataEntry::multicast_address` / `multicast_port` —
  `MetadataConfiguration::to_xml_body` carries no `Multicast`, so no
  `SetMetadataConfiguration` can express them.
- `use_count` anywhere — derived from bindings on a real device.

---

## 12. What is guaranteed, and by which test

The mock's contract is enforced by property tests over the **public API only**,
each against a fresh server. If you depend on a behaviour, this is where to
check whether it is pinned or incidental.

| Guarantee | Test |
|---|---|
| Every action the client can send is routed (157) | `mock_handles_every_action_the_client_can_send` (`src/mock/dispatch.rs`) |
| No response repeats an attribute | `no_response_declares_an_attribute_twice` |
| No response uses an undeclared prefix | `every_response_binds_the_prefixes_it_uses` |
| Every `Set` either round-trips or is declared static (49 pairs) | `tests/mock_roundtrip.rs` |
| Every token-taking operation either discriminates or is declared blind (34 rows) | `tests/mock_token_discrimination.rs` |
| Media1 and Media2 never disagree about shared state | `tests/mock_media1_media2_agree.rs` |
| Per-sensor answers really differ | `tests/mock_multi_sensor.rs` |
| End-to-end flows | `tests/mock_workflow.rs` |
| The XML matches the ONVIF schema — namespaces, names, cardinality, sequence order | `tests/mock_schema_shape.rs` — **not a guarantee in the same sense**, see below |

The last row is weaker than the others and is listed so nobody mistakes it for a
gate. It is `#[ignore]`d and reads the ONVIF schema set at run time from
`$OXVIF_ONVIF_SCHEMA`, a directory outside the tree, because nothing derived
from that schema may enter this repository. A `CLAUDE.md` publishing-checklist
line is the only thing that runs it, and if it printed `SKIPPED` then nothing
was checked. It is nonetheless the only thing here that can see a wrong
namespace or a wrong sequence order at all: the client parser is
namespace-blind and order-independent, so every other row above passes just as
happily against XML no conformant device would emit.

**As of 0.15.0 all ten of its counts are 0.** That is not the same as "the
mock is conformant". A type carrying an `xs:any` suppresses its unknown-child
rule for the whole type; and an element whose children are all optional is
schema-valid empty — `<tt:Spaces/>` would have cleared a finding while claiming
the head supports no coordinate space. Five of the twelve client-facing bugs
the 0.15.0 sweep found were found *in spite of* the counts rather than by them,
and what asserts the part the counts miss is the per-operation value assertions
in `tests/mock_workflow.rs`.

**Five of those counts were nine only from the end of the sweep**: the checker
read `xs:element` and never `xs:attribute` until it gained `MISSING-ATTR`,
`UNKNOWN-ATTR`, `ATTR-AS-ELEMENT` and `ELEMENT-AS-ATTR`. All four read 0 on
their first run and all four were perturbed to prove they move.
`ATTR-AS-ELEMENT` is the one worth knowing: it is the only kind here that an
`xs:any` cannot suppress, because a name the type declares as an attribute is
not an *unknown* child.

The two tables are the important ones. Each row **declares its intent** —
`Works` / `Static(§)` for round-trip, `Discriminates` / `Blind(§)` for tokens —
and **all arms are asserted**. Wire a declared stub up and the test goes red
telling you to move the row, so the list cannot rot into a permanent blind
spot. Current state: 49 round-trip pairs (**49** working, **0** static, 0
known-broken) and 34 token rows (28 discriminating, 6 blind).

**Every `Set` on this mock now round-trips.** The last two static rows were the
audio encoder configurations, and wiring them emptied the audit's Tier 3. Both
`Expect::Static` and `Expect::Broken` are kept as arms with an `#[allow(dead_code)]`
and a comment saying why: they are the only place the distinction between
"deliberately fixture data" and "not wired up yet" can be *written down*, and
deleting an unused arm is how that distinction goes back to being nowhere.

*The token figures read "28 rows, 22 discriminating" until this was written.*
The PTZ node work took them to 31/25 and updated §6 and the audit but not this
paragraph. The pin in `tests/mock_token_discrimination.rs` names both documents
in its failure message for exactly this reason — it fired, and the message was
read only as far as the count that had changed in the *other* table.

---

## 13. Known limitations

Everything here is deliberate and asserted. None of it is a lie the mock tells:
where a family is static, the getter never claims to reflect a write.

### 13.1 Declared stubs — static on both sides

Pinned by a `Blind` row in `tests/mock_token_discrimination.rs`. Catalogued in
[`active/mock-audit-2026-07.md`](active/mock-audit-2026-07.md) §5.

**No `Static` row is left in `tests/mock_roundtrip.rs`** — the audio encoder
configurations were the last two, and every `Set` on this mock now round-trips.
What remains here is read-side.

| Family | What is missing |
|---|---|
| **Media2 `GetVideoSourceModes`** | Static — one mode (`Mode_1`) for every `VideoSourceToken`. `SetVideoSourceMode` is **not** a stub: it faults, see below. |
| **`GetStreamUri` / `GetSnapshotUri`**, both services | One canned URI for every profile. A real device gives each profile its own. |
| **Media1 `GetOSDOptions`**, **Media2 `GetVideoEncoderInstances`** | Static. |

### 13.2 Fidelity gaps — a parser field nothing feeds

Audit §6.

- **Four PTZ attributes are not in oxvif's types, so the mock has nowhere to
  put them.** `onvif.xsd` gives `tt:PTZConfiguration` the optional `xs:int`
  attributes `MoveRamp`, `PresetRamp` and `PresetTourRamp`, and `tt:PTZNode`
  the optional `GeoMove` — `oxvif::PtzConfiguration` and `oxvif::PtzNode` parse
  none of the four. Deliberate: adding public fields is a breaking change, and
  unlike the `Pant` spelling defect none of these causes *silent data loss*,
  because nothing in the crate ever claimed to carry them. Read from the
  schema during the PTZ planning, 2026-07-31.
- **`VideoSourceMode/@Enabled` is not in oxvif's type.** The ONVIF schema marks
  the active mode with it; `oxvif::VideoSourceMode` has `token`,
  `max_framerate`, `max_resolution_*`, `encodings` and `reboot` only. This is
  why `SetVideoSourceMode` has no getter to answer it, and closing the gap means
  a public-API change, not a mock change.

### 13.3 Documented simplifications

- **No motion model.** `AbsoluteMove` and friends update position
  instantaneously; `MoveStatus` is always `IDLE`. There is no timer, so a
  `Monostable` relay does not auto-revert either — use the REST pulse hook.
- **No search cursor.** `FindRecordings` hands out one token and
  `GetRecordingSearchResults` renders the whole current list against it. A real
  device pages and expires searches.
- **`Bounds/@x` and `@y` are read from the wire and dropped.**
  `VideoSourceConfigEntry` models a size, not an offset.
- **Media1 `SetAudioEncoderConfiguration` refuses a body without `Multicast` or
  `SessionTimeout`** (`ter:ConfigModify` / `IncompleteAudioEncoder-SETAEC-5715`).
  Both are *required* members of `tt:AudioEncoderConfiguration`, so a device
  validating the request rejects one that omits them — and oxvif omitted both
  until 0.15. Accepting it here would make the mock the one device on which the
  old, invalid request worked.
- **A Media2 `SetAudioEncoderConfiguration` never changes `SessionTimeout`.**
  `tt:AudioEncoder2Configuration` has no such member, so a Media2 write cannot
  express it; the stored value is preserved rather than cleared. `Multicast` is
  optional there and *is* written, including to `None`. `UseCount` and the
  options list are the device's, not the caller's, and are never written.
- **`SetConfiguration` ignores `ForcePersistence`.** The configuration is always
  stored as if `true`. Real devices differ too widely on what `false` means —
  some keep the change until reboot, some until the session ends, some ignore
  the flag — for a pretend model to be better than none. `UseCount` is likewise
  left alone: it is the device's count of profiles referencing the
  configuration, not a field a caller sets.
- **A freshly created recording has no time bounds**, so `Earliest` / `Latest`
  are omitted rather than faked. The seeded recordings do carry bounds, so the
  distinction is observable.
- **Deleting a recording deletes its jobs.** A job pointing at nothing is not
  a state a device would report.
- **Media1 encoder options omit `H265`.** Deliberate: it lives only at
  `Options/Extension/Extension/H265`, and adding it changes what every caller
  sees. Media2 does advertise H.265, on sensor `VS_1` only.
- **`SetRelayOutputState` writes `logical_state`, but no ONVIF getter returns
  it.** `GetRelayOutputs` per spec does not carry live state. The value is
  observable from Rust via `server.device().read()`, and it drives event
  emission.
- **`SetVideoSourceMode` faults rather than reporting a success it cannot
  back.** It answered `<tr2:Reboot>false</tr2:Reboot>` until 0.15 while storing
  nothing — and, uniquely, **no getter in this crate could ever contradict
  that**: `GetVideoSourceModes` is static, and oxvif's `VideoSourceMode` type
  carries no active-mode field. An unfalsifiable success is the one answer a
  mock must not give, so it now returns `ter:ActionNotSupported` /
  `NotModelled-VSMODE-5813`. If your code calls this operation, expect a fault
  and treat it as "unsupported on this device", not as a bug.

### 13.4 Protocol surface not implemented

- HTTP Digest authentication (WS-Security `PasswordDigest` only).
- RTSP. `GetStreamUri` returns a URI; nothing serves media at it.
- `Metadata`, `Analytics`, `AudioOutput` and `AudioDecoder` configuration types
  are rejected by Media2 `AddConfiguration` with
  `UnmodelledConfigType-CFG2-5542`, because `ProfileEntry` has no slot for them
  and `MediaProfile2` exposes none — so a success could never be observed.
  **`PTZ` was on that list until the PTZ family was wired**; it now binds like
  the other four, and `docs/active/ptz-wiring-plan-2026-07.md` §6.2 records why
  leaving it rejected was not an option once the slot existed.

---

## 14. Extending the mock

The full procedure is `CLAUDE.md` → *Adding a new ONVIF service*, step 5a–5c.
In short:

1. Add the action URI to the right `dispatch_*` arm in `src/mock/dispatch.rs`.
2. Add a `resp_<operation>()` or `handle_<operation>()` in
   `src/mock/services/<service>.rs`.
3. If the operation exists on **both** Media1 and Media2, it must read and
   write the same state. Put the state operation in `services/media.rs` and let
   each service render its own envelope — the shapes genuinely differ.
4. **Every `Set` needs a row in `tests/mock_roundtrip.rs`**, declaring `Works`,
   `Broken(audit §)` or `Static(audit §)`.
5. **Every token-taking operation needs a row in
   `tests/mock_token_discrimination.rs`**, declaring `Discriminates` or
   `Blind(audit §)`, naming two tokens the fixture disagrees on.

Steps 4 and 5 are not optional and `Broken` is a legitimate answer — what is
not legitimate is no row. Nothing else in the codebase distinguishes
"deliberately static" from "not wired up yet", which is how five instances of
that class reached users before the tables existed.

Routing is enforced automatically: a client method whose action has no dispatch
arm fails `mock_handles_every_action_the_client_can_send`. **Payload is not** —
give the handler a plausible response, because nothing checks that for you.
