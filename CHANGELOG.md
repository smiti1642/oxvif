# Changelog

All notable changes to oxvif are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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
