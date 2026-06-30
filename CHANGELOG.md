# Changelog

All notable changes to oxvif are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [0.10.0] - 2026-06-30

Headline: **real-camera correctness for Profile G and imaging.** Parsers and
service discovery had only ever been validated against hand-written mocks;
testing oxvif against a fleet of real cameras (3 vendors, incl. Hikvision and
Hanwha) surfaced several silent-parse bugs where a compliant device's data was
dropped with no error. All are fixed, with the real captures committed as
regression tests, plus a new parse-coverage health dimension and a
`conformance` example to keep the bug class from recurring.

### Fixed
- **Profile G `GetRecordings` parsed real cameras as empty.** The parser looked
  for a plural `RecordingItems` container, read the track token from a `@token`
  attribute, and read `TrackType`/`Description` as direct children of the track
  — but the ONVIF schema (and real GeoVision / Hanwha cameras) use a singular
  `RecordingItem`, a `TrackToken` child element, and `TrackType`/`Description`
  under the track's `Configuration`. `get_recordings` now returns the recordings
  and their video/audio/metadata tracks.
- **Profile G `GetRecordingSearchResults` never completed.** `SearchState` and
  `RecordingInformation` were read as direct children of the response, but the
  schema wraps them in `ResultList`; the state parsed as `"Unknown"`, so the
  search-poll loop spun out and returned nothing. Now read from `ResultList`,
  with a fallback for devices that omit the wrapper.
- **Imaging `GetOptions` exposure/gain/iris ranges parsed to `None`** on
  spec-compliant cameras. The Exposure20 *options* form reports
  `MinExposureTime`/`MaxExposureTime`, `MinGain`/`MaxGain`, `MinIris`/`MaxIris`
  (each a `{Min,Max}` range); the parser only looked for the legacy single
  `ExposureTime`/`Gain`/`Iris` element. `ImagingOptions.{exposure_time, gain,
  iris}_range` now collapse to the spec envelope `[Min{X}.Min, Max{X}.Max]`,
  keeping the legacy element as a fallback (confirmed across 8 cameras /
  2 vendors incl. Hikvision).

### Changed
- **Profile G service discovery falls back to `GetServices`.** Some cameras
  (GeoVision, ONVIF v25.x) advertise the recording / search / replay services
  only via `GetServices`, not the legacy `GetCapabilities` extension.
  `OnvifSession::build` now fills any missing recording / search / replay URL
  from `GetServices`, so `get_recordings` / `search_recordings` /
  `get_replay_uri` work on those devices (they previously failed with a
  `MissingField` error). The health check's Profile G verdict reflects this too.

### Added
- **Parse-coverage health dimension (`health` feature).** `HealthCheck` now
  compares the parsed item count against the number of item elements actually
  present in the raw response for a curated set of list operations (profiles,
  video encoders, users, network interfaces, PTZ nodes), emitting a warning
  under the new `Category::Coverage` when the parser silently dropped items.
  This catches the *list-emptying* bug class; it does **not** catch scalar
  *field-defaulting* (an optional value parsed to `None`) — committed fixtures
  and the `conformance` example guard that.
- **`recording_services` health check.** Reports recording / search / replay as
  advertised (Pass) or absent (Skip), so the Profile G verdict reflects real
  service availability instead of always reading `Unsupported`.
- **`conformance` example (`--features mock`).** The mirror of `mock_server`
  (which *simulates* a device): `conformance` points oxvif at a list of real
  devices, dumps each raw SOAP response, and prints a parsed summary so
  silent-parse mismatches stand out for review.
- **Real-camera regression tests.** Scrubbed captures (GeoVision GV-GBLF4813,
  Hikvision iDS-2CD7A26) added to the recording and imaging tests so the fixed
  shapes can't silently regress.

### Compatibility
- **Breaking:** `health::Category` gained a `Coverage` variant and is now
  `#[non_exhaustive]` — external code matching it must add a `_` arm. (The
  `#[non_exhaustive]` marker means future `Category` additions will be
  backwards-compatible.) `CheckStatus` and `ProfileVerdict` are unchanged and
  remain exhaustively matchable.

---

## [0.9.9] - 2026-06-11

Headline: **digital input read API** — `GetDigitalInputs` returns each
port's token + idle electrical state, completing the read side of the
Device-service IO surface that previously only exposed relay outputs.
Live input transitions still arrive via PullPoint subscription on the
`tns1:Device/Trigger/DigitalInput` topic (unchanged from 0.9.8).

### Added
- **`OnvifSession::get_digital_inputs()` / `OnvifClient::get_digital_inputs()`.**
  Returns `Vec<DigitalInput>` where each entry carries `token` and
  `idle_state` (`"closed"` / `"open"`, or empty string when the
  firmware omits the attribute). Mirrors the existing `get_relay_outputs`
  shape; no Set-side method is exposed because the Device service spec
  doesn't define one (per-input configuration is a real-camera vendor
  extension when it exists at all).
- New `DigitalInput` type re-exported from the crate root.

### Mock server
- **Stateful Relay/Input.** `MockState` now carries `relay_outputs`
  (two defaults: Bistable + Monostable) and `digital_inputs` (two
  defaults). `GetRelayOutputs` and `GetDigitalInputs` render from
  state instead of hardcoded XML; `SetRelayOutputState` and
  `SetRelayOutputSettings` mutate state and emit a SOAP Fault on
  unknown tokens.
- **PullPoint IO event topics.** `GetEventProperties` now advertises
  `tns1:Device/Trigger/DigitalInput` and `tns1:Device/Trigger/Relay`.
  `SetRelayOutputState` queues a `RelayOutput` event automatically;
  the queue is drained by the next `PullMessages` before the synthetic
  motion / rule cycle resumes.
- **`/mock/digital-input/:token/pulse` and `/set?state=...` REST hooks
  (`mock-server` feature).** Test-only endpoints that simulate physical
  input signals without an ONVIF SOAP wrapper (real cameras drive
  inputs through hardware, so there's no spec-level Set for them).
  Pulse queues an active→inactive pair; set queues one event in either
  direction. 404 on unknown token, 400 on missing `state` query.

### Fixed
- **Compile failure when `quick-xml/encoding` is enabled anywhere in the
  build graph.** quick-xml 0.39 cfg-gates `Attribute::unescape_value` away
  whenever its `encoding` feature is active; Cargo feature unification turns
  that feature on for the whole graph as soon as *any* sibling crate (e.g.
  `calamine`) requests it, so `oxvif` failed to build with
  `E0599: no method named unescape_value`. The XML attribute parser now goes
  through `Attribute::decode_and_unescape_value(reader.decoder())`, which is
  always available and decodes identically (the input is always a UTF-8
  `&str`). oxvif now builds with the `encoding` feature on or off.

---

## [0.9.8] - 2026-06-10

Headline: the **health check grows a memory** — JSON output plus a
`--baseline` diff mode that surfaces conformance regressions between
runs. Three audit items also land in this release: H265 is now rejected
at the Media1 boundary (it never belonged there), `SetNetworkInterfaces`
finally accepts IPv6 + MTU via a struct-shaped API, and `ImagingSettings`
gains write-side coverage for manual exposure / WB gains / focus limits.
Plus a record-and-replay `Transport` pair (`CapturingTransport` /
`FixtureTransport`) so the test suite can grow without a camera farm.

### Added
- **`HealthReport` JSON + baseline diff (`health` feature).**
  - `HealthReport::to_json()` / `to_json_pretty()` serialise the full
    report — every `CheckResult`, `ProfileAssessment`, timing, and
    status payload — to a stable JSON shape that round-trips back to
    `HealthReport` via `serde_json::from_str`.
  - `HealthReport::diff(&previous) -> ReportDiff` compares two runs and
    returns `{ flipped_to_fail, flipped_to_pass, new_checks,
    removed_checks, slowed }`. `slowed` carries `SlowedCheck { id,
    prev_ms, now_ms }` for any check that took ≥ 2× longer than the
    baseline (with a 5 ms noise floor).
  - The `healthcheck` example grows `--json` / `--json-pretty`
    (emit report to stdout as JSON for `> baseline.json`) and
    `--baseline <path>` (read a saved JSON report and print the diff)
    so it can be used as a scriptable regression gate in CI — exits
    non-zero if anything flipped to FAIL.
  - All report types now derive `Serialize`, `Deserialize`, `PartialEq`,
    `Eq` so consumers can embed them as struct fields, hash-keys, or
    UI-framework props without newtype wrappers. `verdict()` returns
    `Vec<String>` (was `Vec<&'static str>`) so it survives JSON
    round-trips.
- **`oxvif::fixtures` (`mock` + `health` features).** Two `Transport`
  implementations for offline testing:
  - `CapturingTransport<T>` wraps any inner `Transport` and writes every
    `(action, request_body, response_body)` triple under a directory as
    plain files — point it at a real camera once, get a reusable fixture
    set.
  - `FixtureTransport` is the replay side: it reads the same directory
    layout and serves responses keyed by action without touching the
    network. The two together let new B-track services (Analytics /
    DeviceIO / Receiver, etc.) get integration tests against real
    camera responses without requiring those cameras in CI.
  - New `examples/record_fixtures.rs` (`--features mock,health`) shows
    the typical capture flow against a live device.
- **`ImagingSettings` — manual exposure / WB / focus limits (writable).**
  Eight new optional fields, all serialised by `set_imaging_settings`
  when populated: `exposure_time`, `exposure_gain`, `exposure_iris`,
  `exposure_priority`, `wb_cr_gain`, `wb_cb_gain`, `focus_near_limit`,
  `focus_far_limit`. Existing callers see no behaviour change — these
  are pure additions on top of the auto-mode fields that already
  worked.

### Changed (breaking)
- **`OnvifClient::set_network_interfaces` / `OnvifSession::set_network_interfaces`
  now take a `&NetworkInterfaceConfig` struct** instead of the old
  positional `(token, enabled, dhcp, address, prefix_length, mtu)`
  signature. The new struct carries both an `IpStackConfig::v4` *and*
  `IpStackConfig::v6` (each with `enabled` / `from_dhcp` / `Option<ManualAddress>`)
  plus an `Option<u32>` MTU, so write-side IPv6 finally lines up with
  the read-side `NetworkInterface` parser shipped in 0.9.6. Migration
  for an IPv4-only caller is mechanical:
  ```rust
  // Before
  client.set_network_interfaces(&token, true, false, "192.0.2.10", 24, None).await?;
  // After
  client.set_network_interfaces(&NetworkInterfaceConfig {
      token: token.clone(),
      enabled: true,
      v4: IpStackConfig { enabled: true, from_dhcp: false,
          manual: Some(ManualAddress { address: "192.0.2.10".into(), prefix_length: 24 }) },
      v6: IpStackConfig::default(),
      mtu: None,
  }).await?;
  ```

### Fixed
- **`set_video_encoder_configuration` (Media1) now rejects H.265 up
  front** with `OnvifError::InvalidArgument(..)`. The Media1 schema
  pre-dates H.265 and has no field for `H265Configuration`; passing
  `VideoEncoding::H265` here silently produced an invalid request that
  some cameras coerced into H.264 and others rejected with vague
  faults. Use `set_video_encoder_configuration_media2` for H.265
  profiles.

### Docs
- `docs/audit-2026-05.md` updated — C1 (H265 Media1 reject), C2
  (network struct + IPv6), C3 (imaging manual write) are now marked
  fixed in 0.9.8.

## [0.9.7] - 2026-05-31

Headline: a fast, scriptable **device health check** (`oxvif::health`) — point
it at a camera and get a Pass/Warn/Fail/Skip conformance report with a Profile
S/T/G assessment, a readable alternative to the official ONVIF Device Test Tool.
Also adds the firmware-upgrade / system-restore upload-URI flow and corrects
two write-path XML bugs.

### Added
- **Health check — `oxvif::health`** (opt-in, behind the `health` feature; pure
  library code over `OnvifSession`, no extra dependencies).
  - `HealthCheck::new(url).with_credentials(..).run().await` returns a
    `HealthReport` of per-check `CheckResult`s (status + category + timing) plus
    a `ProfileAssessment` (S/T/G verdict). `Display` renders a readable summary.
  - Checks run concurrently and are read-only by default; opt into write/clock
    probes via the builder.
  - New `examples/healthcheck.rs` (`--features health`).
- **Device firmware / restore (upload-URI flow)** — `start_firmware_upgrade()`
  → `FirmwareUpgradeStart` and `start_system_restore()` → `SystemRestoreStart`.
  Each returns the upload URI + timing; the caller HTTP-POSTs the image/backup
  (the SOAP transport deliberately doesn't carry the binary payload).
- `PartialEq` derived on the video-encoder configuration types
  (`VideoEncoderConfiguration`, `VideoEncoderConfigurationOptions`, and related)
  so downstream code can diff configs without hand-written comparisons.

### Fixed
- `SetVideoEncoderConfiguration` and PTZ `SetConfiguration` produced malformed
  request XML that some cameras rejected — corrected the element nesting/order.

### Docs
- Recorded a read-path audit under `docs/audit-2026-05.md`.

## [0.9.6] - 2026-05-26

Headline: a **built-in mock ONVIF device** so downstream crates can unit-test
client code without a real camera — every vendor's ONVIF differs and depending
on a physical IP camera in tests is painful. Also rolls in the session-level
push `subscribe` and vendor-tolerant OSD parsing.

### Added
- **Built-in mock ONVIF device — `oxvif::mock`** (opt-in, behind features).
  Stateful (Set persists, Get reflects it) and covers every operation oxvif
  implements; state is in-memory (the library never writes to disk — opt into
  persistence via `MockState::set_on_change`).
  - `mock` feature → `MockTransport`, an in-process `Transport` (no sockets, no
    axum) — the fast unit-test path:
    `OnvifClient::new("http://mock").with_transport(Arc::new(MockTransport::new()))`.
  - `mock-server` feature → `MockServer`, a real axum HTTP server on an
    ephemeral port (`MockServer::start().await`), shutting down on drop — for
    cross-process / non-Rust clients.
  - Both default to no auth (call `.with_auth()` / `.enforce_auth(true)` to
    exercise WS-Security) and support `inject_fault(...)` for error-path tests.
  - `axum` / `serde` are optional deps enabled only by these features — the
    default build is unchanged and axum-free.
  - The `examples/mock_server` binary is now a thin wrapper over `MockServer`
    with TOML file persistence (`--features mock-server`); the mock engine moved
    from `examples/` into `src/mock/`. New `tests/mock_workflow.rs` drives one
    command from every service against a real `MockServer`.
- **`OnvifSession::subscribe`** — delegates the WS-BaseNotification push
  subscription that was previously only on `OnvifClient`.
- **`OsdOptions::max_per_text_type`.** New `HashMap<String, u32>`
  exposing the per-text-type quotas (`Plain`, `Date`, `Time`,
  `DateAndTime`) some cameras advertise via XML attributes on
  `<MaximumNumberOfOSDs>` (Genetec, recent Hikvision). Lets clients
  pre-validate `CreateOSD` calls against per-type limits instead of
  parsing opaque `ter:InvalidArgs` fault strings after the fact.
  **Populated only when fetched via `OnvifSession::get_osd_options`
  — `OnvifClient::get_osd_options` leaves it empty (spec-strict).**
- **`OnvifSession::get_osd_options` now layers vendor-extension
  parsing** on top of the spec-strict `OnvifClient` result. Two
  real-world shapes handled:
  - `<MaximumNumberOfOSDs Total="8" Plain="7" DateAndTime="1" .../>`
    — count from `Total` attribute when element body is empty, plus
    per-type quotas from named attributes.
  - `<PositionOption>UpperLeft</PositionOption>` flat siblings, when
    the textbook nested-`<Type>` shape produces nothing.

---

## [0.9.4] - 2026-05-04

### Fixed
- **OSD module: wrong wrapper element on the wire.** `CreateOSD` and
  `SetOSD` request bodies were emitting `<tt:OSDConfiguration>` as the
  wrapper, but the WSDL declares the element as `<trt:OSD>` (with
  *type* OSDConfiguration). Strict cameras (Hikvision, Dahua, Genetec,
  Uniview) rejected this with schema-validation faults like
  "occurrence constraint violation" or generic "Argument Value".
  Matching response parsers also looked for the wrong element names —
  `GetOSDsResponse` items are `<trt:OSDs>` (not `<OSDConfiguration>`),
  `GetOSDResponse` is `<trt:OSD>` — so cameras that actually had OSDs
  configured returned what looked like an empty list.

### Added
- **`OsdOptions` exposes `date_formats`, `time_formats`, and
  `font_size_range`** parsed from `<TextOption>`. ONVIF lets each
  camera define its own allowed date/time format strings (Hikvision
  uses tokens like `"24HourClock"`, Dahua uses `"hh:mm:ss tt"`, etc.)
  and font-size limits — sending values outside that set triggers
  `ter:InvalidArgs` on Create/SetOSD. Clients can now populate
  dropdowns from the camera's actual capabilities instead of guessing.
- **`NotificationMessage.property_operation`** — exposes the
  `Message/@PropertyOperation` attribute (`Initialized`, `Changed`,
  `Deleted`). Subscribers need this to distinguish state-init events
  fired at subscribe time from actual state changes.
- **`PartialEq` derived on the OSD types** so they can flow through
  framework prop-diffing layers (Dioxus, Yew) without a wrapper.
- **SOAP request/response trace logging in `OnvifClient::call`** —
  enable with `RUST_LOG=oxvif=trace` when chasing schema-validation
  faults from cameras that return a generic SOAP fault with no detail.

### Notes
All 375 library tests + 19 doctests pass. Changes are additive
except the OSD wire-format fix, which is a pure bug fix — code that
was working against lenient cameras keeps working, code that was
silently failing against strict ones starts working.

---

## [0.9.3] - 2026-04-17

### Changed
- **Dependencies bumped to latest stable** — keeps the lib.rs / docs.rs
  badges green. Zero source changes were required:
  - `socket2` 0.5 → 0.6 — oxvif already used the `_v4`-suffixed multicast
    methods that 0.6 makes mandatory, so the upgrade is API-compatible.
  - `tokio` 1.52.0 → 1.52.1 — upstream patch reverting a regression
    that caused `spawn_blocking` to hang under load.
  - `toml` 0.8 → 1.1 (dev-dep only, used by the `mock_server` example
    for state persistence). MSRV requirement (1.85) already met.

  All 420 tests (375 lib + 19 doc + 26 mock server) continue to pass.

---

## [0.9.2] - 2026-04-17

### Added
- `discovery::probe_unicast(ip, timeout)` — send a WS-Discovery `Probe`
  directly to a single known IP via unicast. Useful for "is this device
  still there" checks against a known address (e.g. user-added manual
  entries) and for cross-subnet detection where multicast cannot reach.
  Sends both `NetworkVideoTransmitter` and `Device` probes and
  deduplicates the responses by endpoint UUID, matching the behaviour
  of `probe` / `probe_rounds`.

### Fixed
- **XML entity decoding in SOAP response text (GeoVision snapshot URIs).**
  `XmlNode::parse` now handles `Event::GeneralRef` (quick-xml 0.39 emits
  each `&amp;` / `&lt;` / `&#65;` as a separate event) and accumulates
  text runs across events rather than overwriting on each `Event::Text`.
  GeoVision cameras return a `GetSnapshotUriResponse` with URIs like
  `http://host/cgi?skey=X&amp;action=update&amp;Snapshot=Video1.Stream1`
  — valid, RFC-compliant XML escaping. The old parser dropped every
  `Event::GeneralRef` and overwrote text on each `Event::Text`, so only
  the fragment after the last `&amp;` survived; the URI came out as
  `Snapshot=Video1.Stream1`, which the camera's web server rejected
  with 500. Decodes the five predefined named entities (`amp`, `lt`,
  `gt`, `quot`, `apos`) plus numeric character references
  (`&#NN;` / `&#xHH;`). Unknown entities are preserved verbatim as
  `&name;` so no content is silently lost. Affects every ONVIF response
  carrying `&`-escaped text — `StreamUri`, `SnapshotUri`, `Scopes`,
  `HostnameInformation`, custom metadata — not just GeoVision.

- **WS-Addressing namespace regression — restored ~80 missing devices.**
  `build_probe` now emits the legacy WS-Addressing 2004/08 namespace
  with `s:mustUnderstand="1"` on the `Action` and `To` headers and an
  explicit `<wsa:ReplyTo>` pointing at the WS-Addressing anonymous URI.
  The 0.9.0/0.9.1 probe used the modern 2005/08 namespace, which older
  Chinese OEM camera firmwares (Hikvision, Uniview, Dahua-family) silently
  reject — they ship with strict ONVIF 2008-era SOAP parsers that only
  recognise the 2004/08 wsa namespace. On a real heterogeneous LAN this
  regression cost roughly 80 of 195 devices. The new payload matches
  byte-for-byte what ODM (via WCF's `UdpDiscoveryEndpoint(WSDiscoveryApril2005)`)
  sends. WS-Discovery 1.1 / 2009 support — which would use the 2005/08
  wsa namespace — is deferred until both probes can be sent in parallel.
- **Reordered `Bye` no longer flaps a live device offline.** `listen()`
  now parses the `<wsd:AppSequence>` SOAP header (`InstanceId` /
  `MessageNumber` / optional `SequenceId`) and silently drops a `Bye`
  whose sequence is comparable to (same `InstanceId` and `SequenceId`
  as) one we have already seen but with an equal-or-lower
  `MessageNumber`. UDP multicast does not guarantee delivery order, so
  on noisy LANs an old departure could arrive after a fresh presence
  announcement and incorrectly remove a still-online device. Matches
  ODM's `NvtDiscovery.fs::process_offline` behaviour. `Hello` is never
  filtered — at worst a stale Hello resurfaces a live device, which is
  harmless. The `DiscoveryEvent` enum is unchanged: sequence handling
  is fully internal.
- **`probe_rounds` cancellation.** Per-NIC listener tasks are now
  spawned via `tokio::task::JoinSet` instead of `tokio::spawn`. When
  the surrounding future is dropped (e.g. caller wraps the call in
  `tokio::select!` and a timeout branch wins), every in-flight task is
  aborted instead of leaking until its own timeout elapses. Public
  API unchanged.

### Changed
- **Multicast TTL raised from 4 to 32** (`set_multicast_ttl_v4`). The
  previous value was tuned for a single LAN segment and silently lost
  devices on enterprise networks where the camera subnet is reached
  through one or two IGMP-routed hops (PIM/IGMP on a core switch). 32
  is a middle ground between the original 4 and ODM's "VPN workaround"
  TTL of 64 — large enough for typical campus topologies, small enough
  to respect the spec's intent that WS-Discovery stays close to the
  link.

---

## [0.9.1] - 2026-04-16

### Added
- `discovery::probe_rounds(rounds, timeout_per_round, interval)` — repeat
  the per-NIC WS-Discovery Probe `rounds` times with `interval` between
  them, deduplicating results across rounds. `rounds = 0` is a no-op;
  `rounds = 1` is equivalent to `probe()`.

### Fixed
- **Reliable discovery on heterogeneous LANs.** `probe()` on 0.9.0 could
  under-report by 30–40% against a real company network. A reference
  sweep with 195 live ONVIF devices returned 117. Three compounding
  causes, each now addressed:

  1. **Single-type probe filter.** `<wsd:Types>` is an AND match; the
     probe was filtered on `dn:NetworkVideoTransmitter` alone, so every
     device that advertised only `tds:Device` (many NVRs, doorbells,
     Profile T encoders, anything whose vendor shipped Device without
     Media) was silently ignored. `probe_once` now sends both probes
     per socket per round and merges by endpoint UUID — the same
     two-`FindCriteria` pattern as ODM's reference `NvtDiscovery.fs`.
  2. **Strict XML parser rejects real-world ProbeMatch responses.**
     Cameras that emit unescaped `&` in scope URIs, unclosed tags, or
     wrong-encoded CJK bytes had their entire datagram dropped by
     `XmlNode::parse`. The strict DOM parse is still the fast path for
     compliant devices; on `Err` a tolerant local-name scanner pulls out
     endpoint / types / scopes / xaddrs regardless of overall validity.
  3. **Lossy single-shot multicast.** Busy networks drop individual
     Probe packets. `probe_rounds` re-sends with cross-round dedup so
     downstream callers don't have to reimplement the per-NIC +
     `IP_MULTICAST_IF` plumbing just to get retry.

  Against the 195-device reference network: 0.9.0 found 117, 0.9.1
  finds 195.

### Tests
- 10 new: multi-round dedup + interval timing, `rounds = 0` no-op,
  strict parser rejects the malformed fixture (sanity),
  lenient parser recovers endpoint/types/scopes/xaddrs from it,
  drops ProbeMatch without an endpoint UUID,
  distinguishes `<ProbeMatch>` from `<ProbeMatches>`,
  NVT probe XML is well-formed and does not leak Device type,
  Device probe XML is well-formed with the correct `tds:` namespace,
  and an end-to-end check that `probe_once` actually puts both NVT and
  Device probes on the wire per round.

---

## [0.9.0] - 2026-04-15

### Added
- **HTTP Digest Authentication** — transport layer now supports HTTP Digest
  Auth (RFC 7616) as required by ONVIF Profile T §7.1
- **Profile T operations** — Device, Events, and PTZ mandatory operations for
  Profile T compliance
- **Media2 audio/metadata** — `GetAudioSourceConfigurations`,
  `GetAudioEncoderConfigurations`, `SetAudioEncoderConfiguration`,
  `GetAudioEncoderConfigurationOptions`, `GetAudioOutputConfigurations`,
  `GetAudioDecoderConfigurations`, `GetMetadataConfigurations`,
  `SetMetadataConfiguration`, `GetMetadataConfigurationOptions`,
  `AddConfiguration`, `RemoveConfiguration`
- **Healthcheck example** — new `healthcheck` subcommand for the camera
  example; `--ip` and `--auth` CLI flags for direct device targeting
- **Mock server** — refactored to multi-module architecture with stateful
  device service, file persistence, WS-Security auth, and snapshot endpoint

### Fixed
- **XML escape** — all user-supplied SOAP parameters are now XML-escaped
  before interpolation, preventing XML injection
- **MetadataConfiguration** — PTZFilter alignment corrected for Media2 service
- **MediaProfile `video_source_token`** — now correctly parses `<SourceToken>`
  child element instead of reading the wrong attribute
- **Transport** — HTTP 400 responses are now treated as SOAP Faults with
  structured error parsing instead of raw XML dump

### Breaking
- **`MediaProfile`** — added `video_source_config_token: Option<String>` field;
  code that constructs `MediaProfile` with struct literal syntax will need to
  include this new field

### Dependencies
- `if-addrs`: 0.10 -> 0.15 (major upgrade)
- `rand`: 0.10.0 -> 0.10.1 (fixes RUSTSEC-2026-0097)
- `rustls-webpki`: 0.103.10 -> 0.103.12 (fixes RUSTSEC-2026-0098)
- `tokio`: 1.51.0 -> 1.52.0

---

## [0.8.6] - 2026-04-08

### Fixed
- **XML injection** — all user-supplied string parameters (`consumer_url`,
  `filter`, `termination_time`, `timeout`, `keep_alive_timeout`, `wait_time`)
  in the Events and Recording services are now XML-escaped before
  interpolation into SOAP request bodies
- **XML injection in WS-Security** — the `username` field in the
  `UsernameToken` header is now XML-escaped
- **`get_osds` sent wrong XML element** — was sending `<OSDToken>` but
  ONVIF Media WSDL §5.14 specifies `<ConfigurationToken>` for the GetOSDs
  request; devices that ignored unknown elements were silently returning
  unfiltered results

### Changed
- `xml_escape()` now returns `Cow<str>` instead of `String`, avoiding
  allocation when the input contains no XML-special characters (the common
  case for tokens, ISO durations, and numeric values)
- Removed duplicate `xml_escape_url()` in `soap::envelope`; all code now
  uses the unified `xml_escape()` from `types`
- `parse_soap_body()` extracts the `<Body>` node via `swap_remove` instead
  of `.cloned()`, eliminating a deep clone of the entire SOAP body subtree
  on every ONVIF call
- `notification_listener()` now handles connections concurrently via
  `tokio::spawn` + `mpsc` channel (previously sequential)
- `notification_listener()` rejects notification bodies larger than 1 MiB
- WS-Discovery `probe_inner` mutex access uses `unwrap_or_else` to recover
  from poison instead of panicking
- WS-Discovery multicast address uses `const Ipv4Addr` instead of runtime
  `parse().unwrap()`

### Dependencies
- `tokio`: added `sync` feature (required for `mpsc` channel in
  `notification_listener`)

### Tests
- 11 new unit tests: `xml_escape` Cow behavior (5), XML escape security for
  profile token / consumer URL / username (3), `get_osds` sends correct
  `ConfigurationToken` element (2), `parse_soap_body` with header (1)

---

## [0.8.5] - 2026-04-06

### Added
- `discovery::listen()` — passive WS-Discovery listener; joins the ONVIF
  multicast group (`239.255.255.250:3702`) and collects `Hello` / `Bye`
  announcements for a configurable duration
- `DiscoveryEvent` enum (`Hello(DiscoveredDevice)` / `Bye { endpoint }`)
  returned by `listen()`
- `OnvifSession::subscribe()` + `notification_listener()` — WS-BaseNotification
  push subscription; spawns a minimal tokio TCP server so the device can POST
  `Notify` messages back to the consumer
- `PushSubscription` type returned by `subscribe()`
- `examples/camera` — new `discovery-listen` and `push-subscribe` sub-commands
- `examples/odm_compat` — runs all ODM v2.2.250 ONVIF APIs against a real
  camera and reports PASS / FAIL / SKIP / NOT_IMPL coverage summary
- Mock server handlers for Events service (`GetEventProperties`,
  `CreatePullPointSubscription`, `PullMessages`, `Subscribe`, `Renew`,
  `Unsubscribe`)

### Fixed
- **WS-Discovery multicast NIC selection on Windows** — without
  `IP_MULTICAST_IF` (`set_multicast_if_v4`) the OS routes the probe through
  its default multicast interface (often a Hyper-V or WSL virtual adapter)
  rather than the LAN NIC connected to the cameras. `probe_inner` now creates
  one `socket2` socket per interface, sets `IP_MULTICAST_IF` on each, and
  collects responses in parallel so cameras on any subnet are reachable.

### Dependencies
- Added `socket2 = "0.5"` (required for `IP_MULTICAST_IF`)

### Tests
- 7 new unit tests: subscribe action URI, filter body, SOAP fault path,
  `Hello` / `Bye` XML parsing, probe deduplication, garbage-response handling
- 3 end-to-end UDP tests for `probe_inner` (receive, dedup, garbage)

---

## [0.8.4] - 2026-04-05

### Fixed
- **ONVIF spec compliance — 11 parsing bugs corrected against official WSDL/XSD**
  - `NetworkInterface`: IPv4 address now reads `Config/DHCP` for DHCP flag and
    `Manual/Address` / `FromDHCP/Address` per spec (was misreading `FromDHCP` as
    boolean text → produced `ip=/0` against real devices)
  - `Capabilities`: `max_profiles` now reads from
    `Extension/ProfileCapabilities/MaximumNumberOfProfiles`
  - `StorageConfiguration`: removed non-spec `use_anonymous` / `storage_status`
    fields; now reads `Data type=` attribute, `LocalPath`, `StorageUri`,
    `Data/User/UserName` per spec
  - `SystemUris`: removed non-spec `firmware_upgrade_uri`; added `system_backup_uri`;
    `system_log_uri` now reads `SystemLogUris/SystemLogUri/Uri`;
    `support_info_uri` reads `SupportInfoUri` per spec
  - `RecordingConfiguration`: added `maximum_retention_time` field
  - `RecordingItem`: removed non-spec `earliest_recording`, `latest_recording`,
    `recording_status` fields; token now reads child element `RecordingToken`;
    source/content read from `Configuration/Source` and `Configuration/Content`
  - `RecordingJobState`: renamed `token` → `recording_token`; `active_state`
    now reads `State/State` (was `State/ActiveState`)
  - `FocusOptions20`: `focus_af_modes` reads `AutoFocusModes` (was `AFModes`);
    `focus_speed_range` reads `DefaultSpeed` (was `AutoFocusSpeed`)
  - `renew_subscription` / `unsubscribe` SOAP actions: corrected to OASIS-WSN
    namespace (`docs.oasis-open.org/wsn/bw-2/SubscriptionManager/…`)
  - `set_storage_configuration`: removed `use_anonymous` param; XML body now
    uses spec-compliant `<tt:Data type="…">` wrapper

### Tests
- Updated all affected fixtures and assertions in `client_tests.rs`,
  `session_tests.rs`, `types_tests.rs` to match spec-compliant XML
- Added `test_renew_subscription_uses_oasis_action_uri` and
  `test_unsubscribe_uses_oasis_action_uri`

---

## [0.8.3] - 2026-04-05

### Added
- `set_scopes(device_url, scopes)` — replace the device's scope list
- `set_system_date_and_time(device_url, req)` — set device clock;
  takes `SetDateTimeRequest` (manual or NTP, UTC offset, datetime fields)
- Both methods covered by handlers in `examples/mock_server.rs`
- Both methods demonstrated in `examples/write_workflow.rs`

### Fixed
- Broken intra-doc links in `events.rs`, `imaging.rs`, `types/device.rs`,
  `types/recording.rs`, `client/mod.rs` — resolves red version badge on lib.rs

---

## [0.8.2] - 2026-04-04

### Changed
- **Breaking API fixes (pre-1.0 cleanup)**
  - All service URLs unified to `caps.{service}.url` pattern
    (`caps.ptz.url`, `caps.imaging.url`, `caps.recording.url`, etc.)
  - `create_recording` now takes `&RecordingConfiguration` struct instead of
    6 positional `&str` arguments
- New convenience method: `search_recordings(search_url, max_matches)` —
  wraps the find → poll → end_search loop into a single call
- New re-exports: `PtzCapabilities`, `ImagingCapabilities`, `RecordingCapabilities`,
  `SearchCapabilities`, `ReplayCapabilities`, `Media2Capabilities`,
  `DeviceIoCapabilities`, `RecordingConfiguration`

### Fixed
- Stale `caps.*_url` references in doc comments across client modules

### Tests
- Added 12 missing tests: positive + negative for `delete_recording`,
  `delete_track`, `delete_recording_job`, `search_recordings`; negative tests
  for `create_recording_job`, `set_recording_job_mode`,
  `get_recording_search_results`, `end_search` (304 unit tests total)

---

## [0.8.1] - 2026-04-04

### Fixed
- README: project structure updated to reflect `client/` module directory
  (was incorrectly shown as `client.rs`); added missing `types/audio.rs`,
  `types/osd.rs`, `types/ptz_config.rs`, `examples/write_workflow.rs`
- README: running examples list now includes all 29 commands (13 were missing)
- README: removed residual `OnvifSession`-over-`OnvifClient` bias
  (`// recommended:` comment, `session.client()` description)
- `examples/mock_server.rs`: fixed axum 0.8 wildcard route syntax
  (`/*path` → `/{*path}`)

---

## [0.8.0] - 2026-04-04

### Added
- **Recording Service write operations** — 9 new methods completing Profile G write coverage:
  - `create_recording` / `delete_recording`
  - `create_track` / `delete_track`
  - `get_recording_jobs` / `create_recording_job` / `set_recording_job_mode` /
    `delete_recording_job` / `get_recording_job_state`
- New types: `RecordingJob`, `RecordingJobConfiguration`, `RecordingJobState`
- All 9 methods exposed on `OnvifSession` as convenience delegates
- All 9 methods covered by handlers in `examples/mock_server.rs`
- **Events Service** — `event_stream(subscription_url, timeout, max_messages)` wraps
  the `pull_messages` polling loop into an infinite `Pin<Box<dyn Stream<...>>>` —
  yields individual `NotificationMessage` items; errors stop the stream
- Added `trc` / `tse` / `trp` namespace declarations to the SOAP envelope — previously
  omitted, making recording/search/replay request bodies technically invalid XML
- New `async-stream = "0.3"` and `futures-core = "0.3"` runtime dependencies

### Changed
- Removed 38 low-value unit tests that only verified HTTP dispatch routing or duplicated
  SOAP Fault coverage without exercising response parsing (314 → 292 unit tests)

---

## [0.7.6] - 2026-04-04

### Changed
- Extended existing response/options types with remaining medium-priority ONVIF spec fields:
  - `PtzStatus`: `error` (`PTZStatus/Error`) — human-readable fault description
  - `VideoEncoderConfiguration`: `guaranteed_frame_rate` (`GuaranteedFrameRate` boolean);
    `to_xml_body` updated to serialise the flag
  - `StorageConfiguration`: `storage_status` (`StorageStatus`) — connection state string
  - `ImagingOptions`: 8 new fields covering exposure detail ranges
    (`exposure_time_range`, `gain_range`, `iris_range: Option<FloatRange>`),
    focus options (`focus_af_modes: Vec<String>`, `focus_speed_range`),
    WDR options (`wdr_level_range`, `wdr_modes`) and
    backlight compensation modes (`backlight_compensation_modes`)
- 8 new unit tests (306 → 314)

---

## [0.7.5] - 2026-04-04

### Changed
- Extended existing response types with ONVIF spec fields that were previously omitted (second batch):
  - `MediaProfile2`: `audio_source_token`, `audio_encoder_token`, `ptz_config_token`
    (`Configurations/AudioSource`, `Audio`, `PTZ/@token`)
  - `PtzConfiguration`: 6 default coordinate-space URI fields
    (`DefaultAbsolutePanTiltPositionSpace`, `DefaultAbsoluteZoomPositionSpace`,
    `DefaultRelativePanTiltTranslationSpace`, `DefaultRelativeZoomTranslationSpace`,
    `DefaultContinuousPanTiltVelocitySpace`, `DefaultContinuousZoomVelocitySpace`)
    + new `PtzSpeed` struct for `DefaultPTZSpeed` (`pan_tilt`/`zoom`)
    + `to_xml_body` updated to serialise all new fields
  - `ImagingSettings`: `focus_mode`, `focus_default_speed`, `wide_dynamic_range_mode`,
    `wide_dynamic_range_level`, `image_stabilization_mode`, `tone_compensation_mode`
    + `to_xml_body` updated
  - `RecordingTrack`: `data_from`, `data_to` (track time bounds)
  - `RecordingSourceInformation`: `address` (source device network address)
  - `OsdTextString`: new `OsdColor` struct (`x`/`y`/`z`/`colorspace`/`transparent`),
    `font_color`, `background_color`, `is_persistent_text` + `to_xml_body` updated
- New public types: `OsdColor`, `PtzSpeed`, `MulticastConfiguration`
- 5 new unit tests (301 → 306)

---

## [0.7.4] - 2026-04-04

### Changed
- Extended existing response types with ONVIF spec fields that were previously omitted (first batch):
  - `MediaProfile`: `video_source_token`, `video_encoder_token`, `audio_source_token`,
    `audio_encoder_token`, `ptz_config_token` (child element `@token` attributes)
  - `PtzNode`: `pan_tilt_spaces`, `zoom_spaces` (`Vec<PtzSpaceRange>` from `SupportedPTZSpaces`)
  - `PtzStatus`: `utc_time` (`PTZStatus/UtcTime`)
  - `AudioEncoderConfiguration`: `channels` (`Channels` element); `to_xml_body` updated
  - `DnsInformation`: `search_domains` (`Vec<String>` from `SearchDomain` elements)
  - `VideoEncoderConfiguration`: new `MulticastConfiguration` struct + `multicast` field
    (`Multicast/Address/IPv4Address`, `Port`, `TTL`, `AutoStart`); `to_xml_body` updated
  - `ImagingSettings`: `backlight_compensation` (`BacklightCompensation/Mode`); `to_xml_body` updated
  - `NetworkInterface`: `ipv6_enabled`, `ipv6_from_dhcp`, `ipv6_address`
    (`IPv6/Enabled`, `IPv6/Config/DHCP`, `IPv6/Config/Manual|LinkLocal/Address`)
- 9 new unit tests (292 → 301)

---

## [0.7.3] - 2026-04-03

### Changed
- Bumped all direct dependencies to latest versions:
  - `quick-xml` 0.36 → 0.39 (API: `BytesText::unescape()` replaced by `xml_content()`)
  - `sha1` 0.10 → 0.11
  - `rand` 0.8 → 0.10 (`thread_rng().fill_bytes()` replaced by `rng().fill_bytes()`)
  - `reqwest` 0.12 → 0.13 (`rustls-tls` feature replaced by `rustls` + `rustls-native-certs`)
  - `tokio` patch update to 1.51
  - `axum` (dev) 0.7 → 0.8

---

## [0.7.2] - 2026-04-03

### Changed
- Updated crate-level docs (`lib.rs`): architecture diagram now shows
  `OnvifSession` above `OnvifClient`; quick start rewritten to use
  `OnvifSession`; added `OnvifClient` low-level section; Device service
  list updated with all operations added in 0.6.0–0.7.0

---

## [0.7.1] - 2026-04-03

### Changed
- Expanded crate-level docs: added dedicated `OnvifSession` section to
  `README.md` with builder example, side-by-side comparison with
  `OnvifClient`, and method/accessor tables

---

## [0.7.0] - 2026-04-03

### Added
- **Device Service** — 8 additional operations completing device management coverage:
  - **Network protocols**: `set_network_protocols`
  - **Discovery**: `get_discovery_mode`, `set_discovery_mode`
  - **System**: `get_system_uris`, `set_system_factory_default`
  - **Relay config**: `set_relay_output_settings`
  - **Storage**: `get_storage_configurations`, `set_storage_configuration`
- New types: `StorageConfiguration`, `SystemUris`
- All 8 operations exposed on `OnvifSession` as convenience delegates
- All 8 operations covered by handlers in `examples/mock_server.rs`
- 16 new unit tests (292 total)
- `examples/camera.rs`: new `storage` and `discovery-mode` commands; extended
  `full_workflow` with sections 26–28 (storage, system URIs, discovery mode)

---

## [0.6.0] - 2026-04-03

### Added
- **Device Service** — 13 new operations for full device management:
  - **User management**: `get_users`, `create_users`, `delete_users`, `set_user`
  - **Network config**: `get_network_interfaces`, `set_network_interfaces`,
    `get_network_protocols`, `get_dns`, `set_dns`, `get_network_default_gateway`
  - **System**: `get_system_log`
  - **I/O**: `get_relay_outputs`, `set_relay_output_state`
- New types: `User`, `NetworkInterface`, `NetworkProtocol`, `DnsInformation`,
  `NetworkGateway`, `SystemLog`, `RelayOutput`
- All 13 operations exposed on `OnvifSession` as convenience delegates
- All 13 operations covered by handlers in `examples/mock_server.rs`
- 26 new unit tests (276 total)
- CLAUDE.md SOP: new rule requiring every new method to have a mock server handler

---

## [0.5.0] - 2026-04-03

### Added
- `OnvifSession` high-level convenience wrapper — calls `GetCapabilities` once at
  construction and caches service URLs so callers never need to pass endpoint URLs
  to individual methods; built via `OnvifSession::builder(...).with_clock_sync().build()`
- 20 new unit tests for `OnvifSession` (builder, missing-URL errors, delegate
  methods, accessors) in `src/tests/session_tests.rs`
- `examples/mock_server.rs` — stateless ONVIF HTTP mock server responding to
  every operation exercised by `full-workflow`; default port 18080

---

## [0.4.2] - 2026-04-02

### Fixed
- All `&str` parameters interpolated into SOAP request bodies are now
  XML-escaped via `xml_escape()` — previously token and identifier parameters
  in Media1, Media2, PTZ, Imaging, OSD, Recording, Search, and Replay methods
  were not escaped
- `RecordingTrack/@token` now returns `Err(SoapError::missing(...))` when the
  attribute is absent, instead of silently defaulting to an empty string
- `RecordingInformation::source_name` no longer falls back to reading from the
  parent node when `<Source>` is absent; returns empty string correctly
- `HttpTransport` now enforces a 10-second timeout on all requests
- `User-Agent` header now reflects the actual crate version via
  `env!("CARGO_PKG_VERSION")` instead of the hardcoded `"oxvif/0.1"`
- `<wsa:To>` WS-Addressing header is now included in every SOAP request,
  required by some strict ONVIF devices

---

## [0.4.1] - 2026-04-02

### Changed
- Expanded crate-level docs (`lib.rs`): ONVIF Profile coverage table,
  supported services list, updated Quick start with clock-sync step,
  added `MockTransport` doc-test example

---

## [0.4.0] - 2026-04-02

### Added
- **Device Service**: `get_scopes` — completes ONVIF Profile S coverage
- **Recording Service**: `get_recordings`
- **Search Service**: `find_recordings`, `get_recording_search_results`, `end_search`
- **Replay Service**: `get_replay_uri`
- New types: `RecordingItem`, `RecordingSourceInformation`, `RecordingTrack`,
  `RecordingInformation`, `FindRecordingResults`
- 12 new unit tests (228 total)

---

## [0.3.0] - 2026-04-02

### Added
- **PTZ Home**: `ptz_goto_home_position`, `ptz_set_home_position`
- **Imaging Focus**: `imaging_move` (`FocusMove::Absolute/Relative/Continuous`),
  `imaging_stop`, `imaging_get_move_options`, `imaging_get_status`
- **OSD Service**: `get_osds`, `get_osd`, `set_osd`, `create_osd`, `delete_osd`,
  `get_osd_options`
- New types: `FocusMove`, `ImagingStatus`, `ImagingMoveOptions`,
  `OsdConfiguration`, `OsdPosition`, `OsdTextString`, `OsdOptions`
- 16 new unit tests (positive + negative paths for all new methods)

---

## [0.2.0] - 2026-04-02

### Added
- **Audio Service**: `get_audio_sources`, `get_audio_source_configurations`,
  `get_audio_encoder_configurations`, `get_audio_encoder_configuration`,
  `set_audio_encoder_configuration`, `get_audio_encoder_configuration_options`
- **PTZ Configuration**: `ptz_get_configurations`, `ptz_get_configuration`,
  `ptz_set_configuration`, `ptz_get_configuration_options`, `ptz_get_nodes`
- New types: `AudioSource`, `AudioSourceConfiguration`, `AudioEncoding`,
  `AudioEncoderConfiguration`, `AudioEncoderConfigurationOptions`,
  `AudioEncoderOptions`, `PtzConfiguration`, `PtzConfigurationOptions`,
  `PtzNode`, `PtzSpaceRange`
- 13 new unit tests (positive + negative paths for all new methods)

---

## [0.1.3] - 2026-04-02

### Fixed
- `PtzPreset`, `VideoSource`, `VideoSourceConfiguration`,
  `VideoEncoderConfiguration`, and `VideoEncoderConfiguration2` now return
  `Err(SoapError::missing(...))` when the required `token` attribute is absent,
  instead of silently defaulting to an empty string

---

## [0.1.2] - 2026-04-02

### Fixed
- `MediaProfile::from_xml` and `MediaProfile2::vec_from_xml` now return
  `Err(SoapError::missing("Profile/@token"))` instead of silently using an
  empty string when the `token` attribute is absent
- All user-supplied strings passed into SOAP request bodies are now XML-escaped
  (`set_hostname`, `set_ntp`, `create_profile`, `ptz_set_preset`)
- String fields in `to_xml_body()` serialisers (`VideoSourceConfiguration`,
  `VideoEncoderConfiguration`, `VideoEncoderConfiguration2`,
  `ImagingSettings`) are now XML-escaped
- Replaced `stack.last_mut().unwrap()` in the XML parser with a safe `if let`,
  preventing a potential panic on malformed device responses
- Named the UDP receive buffer size constant (`UDP_MAX_SIZE = 65_535`) in
  `discovery.rs`

### Tests
- Added 8 negative-path tests covering malformed XML responses, SOAP Fault
  replies, missing required fields, and HTTP-level errors

---

## [0.1.1] - 2026-04-02

### Added
- `OnvifClient` now derives `Clone` — store one client and share it across
  async tasks without reconstructing
- `OnvifClient::device_url()` getter exposes the device service URL

---

## [0.1.0] - 2026-04-02

### Added
- Initial release
- Async ONVIF client (`OnvifClient`) with WS-Security `UsernameToken` /
  `PasswordDigest` authentication
- **Device Service**: `GetCapabilities`, `GetServices`, `GetDeviceInformation`,
  `GetSystemDateAndTime`, `GetHostname`/`SetHostname`, `GetNTP`/`SetNTP`,
  `SystemReboot`
- **Media1 Service**: profile management (`GetProfiles`, `GetProfile`,
  `CreateProfile`, `DeleteProfile`, add/remove video encoder/source
  configurations), `GetStreamUri`, `GetSnapshotUri`, full video source and
  encoder configuration read/write
- **Media2 Service**: `GetProfiles`, `CreateProfile`/`DeleteProfile`,
  `GetStreamUri`, `GetSnapshotUri`, video source and encoder configuration
  (native H.265 support), `GetVideoEncoderInstances`
- **PTZ Service**: `AbsoluteMove`, `RelativeMove`, `ContinuousMove`, `Stop`,
  `GetPresets`, `GotoPreset`, `SetPreset`, `RemovePreset`, `GetStatus`
- **Imaging Service**: `GetImagingSettings`, `SetImagingSettings`, `GetOptions`
- **Events Service**: `GetEventProperties`, `CreatePullPointSubscription`,
  `PullMessages`, `Renew`, `Unsubscribe`
- **WS-Discovery**: UDP multicast `Probe` with duplicate suppression
- Mockable `Transport` trait for unit testing without a real camera
- 181 unit tests + 9 doc tests
