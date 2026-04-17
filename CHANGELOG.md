# Changelog

All notable changes to oxvif are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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
