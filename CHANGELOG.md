# Changelog

All notable changes to oxvif are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [0.16.0] - Unreleased

Headline: **oxvif now ships the library support required by the new fleet-ready
`oxvif` CLI.** Discovery gains fallible, interface-aware APIs while the
workspace adds the separately publishable `oxvif-cli 0.1.0` package.

### Added

- `discovery::probe_result` and `discovery::probe_result_on` expose discovery
  failures instead of collapsing them into an empty device list.
- `discovery::discovery_interfaces` reports eligible local IPv4 interfaces so
  callers can select and diagnose multicast paths explicitly.
- Workspace package `oxvif-cli 0.1.0`, installed as `oxvif`, with named device
  inventory, Groups, Views, credential profiles, versioned discovery snapshots,
  fingerprinted import plan/apply, read-only ONVIF diagnostics, Agent guidance,
  and deterministic bounded fleet execution.
- Human CLI facade with secure `setup` and `auth`, concise `info`/`test`/
  `health`/media commands, positional saved-device selection, `--json` and
  `--jsonl` shorthands, bare ephemeral discovery, interactive media-profile
  selection, actionable ID suggestions, and generated shell completion.
- Purpose-built terminal renderers for profiles, capabilities, services, PTZ,
  health, and media URIs, while retaining full-fidelity JSON/JSONL output.
- Cross-platform CI for Linux, Windows, and macOS, including Rust 1.88 MSRV,
  security audit, binary smoke, package, and documentation gates; a manual
  artifact workflow prepares checksums and SPDX SBOMs before release.
- Community contribution/security/conduct guidance, issue and pull-request
  templates, compatibility-report redaction rules, and Dependabot coverage.
- Draft 2020-12 schemas for the schema-v3 success/error envelope and command
  descriptors, with JSON and fleet-JSONL validation in the CLI test gate.
- Native credential adapters for Windows Credential Manager, macOS Keychain,
  and Linux Secret Service, with one black-box lifecycle contract and dedicated
  native CI. Backend errors are sanitized, owned CLI password buffers zeroize
  on drop, and plaintext fallback is forbidden.

### Changed

- The legacy `discovery::probe` remains available and delegates to the new
  fallible implementation while preserving its empty-on-error behavior.
- The workspace release order is `oxvif 0.16.0` first, followed by
  `oxvif-cli 0.1.0`, so the CLI tarball verifies against crates.io.
- `Cargo.lock` is now tracked because the workspace ships the `oxvif` CLI;
  release builds, MSRV checks, and dependency audits therefore resolve the same
  dependency graph.
- CLI retry behavior now classifies typed ONVIF errors: transient transport
  failures use bounded exponential backoff and a per-attempt timeout, while
  authentication rejection, deterministic SOAP faults, invalid input, and
  serialization failures are not retried. Health and per-interface discovery
  honor the same global retry count.
- `-v` and `-vv` now emit sanitized command, retry-policy, outcome, and timing
  diagnostics to stderr without altering structured stdout.
- The CLI accepts repeatable `--ca-certificate <FILE>` PEM bundles through one
  shared HTTP transport path used by diagnostics, health, setup/refresh,
  enrichment, and fleet work. Invalid/empty bundles and private-key material
  fail before connection; certificate-chain and hostname verification remain
  enabled.
- The public command surface now has an exhaustive `CommandId`/`CommandSpec`
  catalogue. Canonical typed requests derive their names from it, descriptor
  order/identity is drift-checked, and every descriptor example must parse back
  to the declared canonical command in tests.
- All 61 Agent command descriptors now expose a named output kind,
  command-specific error set, semantic argument guidance, and a safe example
  instead of generic `object` metadata.
- `config path` reports resolved registry locations without writing them, and
  `config validate` parses the registry plus every indexed discovery snapshot
  for backup/restore and support diagnosis; unindexed snapshot files are
  reported as warnings and never deleted automatically.

### Security

- Updated `h2` from 0.4.15 to 0.4.16 to address `RUSTSEC-2026-0258`, and
  replaced the yanked `chacha20` 0.10.1 release with 0.10.2. The release CI now
  blocks on a clean `cargo audit` result.
- Added a private-HTTPS regression proving an untrusted test endpoint fails
  without its CA and succeeds only when that CA is explicitly supplied.

---

## [0.15.0] - 2026-08-03

Headline: **every service can now be asked what it can do, and the PTZ surface
gains guard tours.** Seventeen new operations — nine `GetServiceCapabilities`,
seven preset-tour operations, and the PTZ `SendAuxiliaryCommand` that cameras
actually implement. Alongside them, a fix for a silent parser defect found on a
real two-sensor camera, and a quality gate that was running less than
two-thirds of the test suite.

> **This entry is condensed.** The full record — schema citations, measured
> counts, and the perturbation that verified each fix — is in
> [`docs/releases/0.15.0.md`](https://github.com/smiti1642/oxvif/blob/master/docs/releases/0.15.0.md).

### Added

- **`GetServiceCapabilities` on all nine services.** New
  `src/types/service_capabilities.rs` with nine top-level types and eight
  nested ones, plus nine client methods and nine `OnvifSession` wrappers:
  `device_`, `media_`, `media2_`, `ptz_`, `imaging_`, `events_`,
  `recording_`, `search_` and `replay_get_service_capabilities`.

- **PTZ preset tours** — seven operations: `ptz_get_preset_tours`,
  `ptz_get_preset_tour`, `ptz_get_preset_tour_options`,
  `ptz_create_preset_tour`, `ptz_modify_preset_tour`,
  `ptz_operate_preset_tour`, `ptz_remove_preset_tour`, each with a session
  wrapper. Twelve new public types.

- **The health check now asks the nine `GetServiceCapabilities`, and checks
  the device against itself.** Ten new checks under `Category::Services`:
  `service_caps_{device,media,media2,ptz,imaging,events,recording,search,replay}`
  — Pass when the service answers, Skip when it is not advertised, Fail when
  it is advertised and refuses — plus `service_caps_self_consistent`.

- **`ptz_send_auxiliary_command`** — the **PTZ** service's
  `SendAuxiliaryCommand`, which is not the Device operation of the same name
  that oxvif already had. Different endpoint and namespace, different child
  elements — `ProfileToken` + `AuxiliaryData` answered by
  `AuxiliaryResponse`, where the Device one sends `AuxiliaryCommand` and
  reads `AuxiliaryCommandResponse` — scoped to a media profile, and it
  returns the device's answer. …

- Mock coverage for all seventeen operations. Preset tours are **stateful**
  — a tour created by `CreatePresetTour` comes back from a later
  `GetPresetTours`, and `OperatePresetTour` moves an observable state — so
  the mock is an integration harness for the feature rather than a fixture
  printer. New integration tests `tests/mock_service_capabilities.rs` and
  two cases in `tests/mock_workflow.rs`.

- **`tests/mock_schema_shape.rs` — the mock's XML, checked against the ONVIF
  schema.** Six shape defects were found during this release, every one of
  them by a human reading a schema file by hand, and one of them
  (`MediaProfile2::audio_encoder_token`, below) was a client bug that a
  green test had been asserting around. **Nothing else in the crate can see
  the class**: `XmlNode` is namespace-stripped and every lookup matches the
  local name only, so oxvif's parser is namespace-blind and
  order-independent — a response with every element in the wrong namespace,
  in the wrong order, parses identically. …

- **The mock emitted sixteen elements in the wrong XML namespace.** All
  sixteen fixed; the schema-shape check's `WRONG-NS` count is 0 and no other
  kind moved. **No caller is affected** — `XmlNode` is namespace-stripped,
  so oxvif's parser reads these identically either way. What was affected is
  the `mock-server` feature, which `src/mock/mod.rs` offers "for
  cross-process / non-Rust clients": a conformant client resolving by
  qualified name found nothing.

- **The mock's Imaging service emitted eight non-conformant rows, and seven
  of them were one wrong element name.** `GetMoveOptions` rendered the focus
  ranges as `tt:PositionSpace` and `tt:SpeedSpace`.
  `tt:AbsoluteFocusOptions` declares `Position` (required) then `Speed`
  (optional), and `tt:ContinuousFocusOptions` declares `Speed` alone,
  required — all three plain `tt:FloatRange`. …

- **The mock's Media2 `GetProfiles` sent a token and no body for every bound
  configuration, and both `GetProfiles` responses had audio and video the
  wrong way round.** Seven schema-shape rows, one renderer each way:
  `MISSING-REQUIRED` 21 → 16, `ORDER` 5 → 3.

- **The mock answers a DeviceIO endpoint.** `{base}/onvif/deviceio`,
  advertised in `GetCapabilities` (`Capabilities/Extension/DeviceIO`)
  **and** `GetServices`, dispatching `…/ver10/deviceio/wsdl/` and rendering
  `GetDigitalInputs` in `tmd:`. It shares one `DeviceState` with the device
  service, the way Media1 and Media2 do.

### Fixed

- **`get_system_uris().system_log_uri` was `None` from every conformant
  device.** `tt:SystemLogUriList` declares one child element,
  **`SystemLog`**, *typed* `tt:SystemLogUri` — and `SystemUris::from_xml`
  (`src/types/device.rs`) walked `SystemLogUris/SystemLogUri/Uri`, reading
  the type name as though it were the element name. `tt:SystemLogUri` itself
  declares `Type` then `Uri`; the mock spelled the first `LogType`.

- **`get_video_encoder_instances_media2().encodings` was empty from every
  conformant device, while `total` still parsed.** `tr2:EncoderInstanceInfo`
  declares `Codec` (`tr2:EncoderInstance`, `[0..*]`) then `Total`, and
  `tr2:EncoderInstance` declares `Encoding` then `Number`.
  `VideoEncoderInstances::from_xml` (`src/types/video.rs`) iterated
  `children_named("Encoding")` — the name of a child *of* `Codec`, one level
  down. …

- **The mock omitted seven required members, one undeclared name and one
  sequence order — the last fourteen schema-shape rows.** All five element
  counts are now **0** (the four attribute kinds came later, and are also
  0); the movement per group is recorded in `PINS`
  (`tests/mock_schema_shape.rs`) and in
  `docs/active/mock-schema-conformance-2026-08.md` §1, and each of the four
  groups was perturbed on its own.

- **Release documentation audit: four wrong statements, all corrected
  here.** Nothing in this repository asserts prose, so the surfaces below
  were checked mechanically against source rather than re-read.

- **`OnvifClient::get_osd_options().position_types` was empty from every
  conformant camera.** `tt:OSDConfigurationOptions` declares
  `PositionOption` as `type="xs:string" maxOccurs="unbounded"` — one element
  per position, text body. `OsdOptions::from_xml` instead read a single
  `<PositionOption>` wrapper for nested `<Type>` children, so the strict
  client path returned nothing; only `OnvifSession` recovered the positions,
  and only through `apply_vendor_extensions`, whose doc comment called the
  *conformant* shape a Genetec deviation from the spec.

- **The schema-shape checker gained a tenth kind, `SIMPLE-TYPE-KIDS`**, for
  an element the schema types as text that the mock filled with a tree.
  `Index` now indexes named `xs:simpleType`s, and `is_simple` answers only
  where the answer is certain — a built-in `xs:*` type or a named
  `xs:simpleType`; a type merely absent from the index is *unknown*, not
  simple, and stays silent.

- **The schema-shape checker reads `xs:attribute`.** It had five kinds, all
  about elements; it now has nine. `MISSING-ATTR` reports a `use="required"`
  attribute the mock omits, `UNKNOWN-ATTR` one the type does not declare,
  and — the two that matter — `ATTR-AS-ELEMENT` and `ELEMENT-AS-ATTR` report
  a name declared on the *other* side of the element/attribute line.

- **`VideoEncoderConfiguration2::gov_length` and `::profile` were `None`
  from every conformant Media2 camera, and
  `set_video_encoder_configuration_media2` silently discarded both.**
  `tt:VideoEncoder2Configuration` declares `GovLength` and `Profile` as
  `xs:attribute` — together with `AnchorFrameDistance`,
  `GuaranteedFrameRate`, `Signed`, `SecureStreamingProtocolAlgorithm`, and
  the required `token` it inherits from `tt:ConfigurationEntity`. …

- **`VideoEncoderOptions2` read three attributes as elements, two of them
  lists, and `VideoSourceConfigurationOptions::max_limit` read a fourth.**
  The seventh and eighth client-facing bugs of this sweep, and the same
  class as the `VideoEncoderConfiguration2` entry above — one type up, on
  the *options* siblings.

- **The mock's Media2 encoder options were identical on both sensors.**
  `resp_video_encoder_configuration_options_media2`
  (`src/mock/services/media2.rs`) rendered the same `GovLengthRange` and the
  same three H.264 profiles whichever channel was addressed, so an assertion
  reading either would have passed against a renderer that ignored the token
  — the coincidence the differing `resolutions` lists already exist to rule
  out, one field over. …

- **The mock's four video encoders all carried `gov_length: 25`.** Any
  assertion reading it therefore passed against a renderer that ignored the
  token, which is the coincidence the differing `resolutions` lists already
  exist to rule out. They are 25 / 30 / 50 / 15 now, against profiles Main /
  Main / High / Baseline, so `VEC_1` and `VEC_3` disagree on both.

- **The mock accepted a `SetVideoEncoderConfiguration` body no schema
  declares.** `apply_video_encoder_write` (`src/mock/services/media.rs`)
  took `extract_tag(body, "GovLength")` against the whole body and
  `extract_tag(body, "Profile")` ahead of the codec-specific names, so both
  the attribute form and the pre-0.15 element form round-tripped cleanly — a
  client that regressed would have been silently understood. …

- **The mock's `GetCapabilities` omitted required members from five
  capability types and emitted one element the schema declares on a
  different type.** Six schema-shape rows in one renderer,
  `resp_capabilities` (`src/mock/services/device.rs`): `MISSING-REQUIRED` 16
  → 11, `UNKNOWN-NAME` 7 → 6, `UNKNOWN-CHILD` and `ORDER` unmoved.

- **`<tt:UsernameToken>` was two ONVIF types mixed into one element, and it
  had been propping up an unsound health check.** `tt:SecurityCapabilities`
  (`onvif.xsd`) — the type the device-level `GetCapabilities` answers with —
  declares eight `xs:element`s and `UsernameToken` is not among them. The
  name exists in the schema set exactly once: as an **`xs:attribute`** on
  `tds:SecurityCapabilities` (`devicemgmt.wsdl`), the *service* capabilities
  type, where `resp_service_capabilities` already carries it correctly. …

- **`MetadataConfigurationOptions::analytics_supported` read an element
  ONVIF does not declare.** `MetadataConfigurationOptions::from_xml`
  (`src/types/media.rs`) read `Options/Extension/AnalyticsSupported`.
  Parsing all fifteen schema files: **`AnalyticsSupported` appears nowhere,
  as element or attribute**, and `tt:MetadataConfigurationOptionsExtension`
  declares exactly `CompressionType` (`[0..*]`) and a further `Extension`. …

- **The mock's Media2 metadata configurations were missing two required
  members and had two out of order.** `render_metadata`
  (`src/mock/services/media2.rs`) emitted `Analytics` before `PTZStatus`,
  which `tt:MetadataConfiguration`'s `xs:sequence` does not permit, and
  omitted `Multicast` and `SessionTimeout` entirely. Six schema-shape rows:
  `MISSING-REQUIRED` 11 → 7 (three of the four; the fourth was the options
  getter above) and `ORDER` 3 → 1, the last remaining `ORDER` row being
  `GetOSDOptions` — which the last bullet of this sweep then closed, taking
  `ORDER` to 0.

- **`imaging_get_move_options` returned `None` for all five of its ranges
  against every conformant camera.** `ImagingMoveOptions::from_xml`
  (`src/types/imaging.rs`) read `PositionSpace`, `SpeedSpace` and
  `DistanceSpace`. `tt:AbsoluteFocusOptions` declares `Position` then
  `Speed`, `tt:RelativeFocusOptions20` declares `Distance` then `Speed`, and
  `tt:ContinuousFocusOptions` declares `Speed` — all plain `tt:FloatRange`.
  …

- **`set_storage_configuration` sent five elements in the wrong XML
  namespace.** `Data`, `LocalPath`, `StorageUri`, `User` and `UserName` went
  out as `tt:`. `devicemgmt.wsdl` declares `StorageConfiguration`,
  `StorageConfigurationData` and `UserCredential` in its own
  `elementFormDefault="qualified"` schema, so all five are `tds:` — **and
  none of the five names exists in the `tt:` namespace at all.** This is a
  request body, so a device that validates it can reject the call or
  silently drop the fields; oxvif has sent it since storage support landed.

- **`GetDigitalInputs` was sent to the device service, which does not
  implement it.** `deviceio.wsdl` is the only WSDL declaring
  `GetDigitalInputs` and `GetDigitalInputsResponse`; `devicemgmt.wsdl`
  declares neither, and its one occurrence of the string is prose inside an
  enumeration's documentation. Since 0.9.9 oxvif sent
  `…/ver10/device/wsdl/GetDigitalInputs` with a `tds:` body to the device
  endpoint. …

- **The mock's Storage family was a single static fixture.**
  `GetStorageConfigurations` always answered one `SD_01` entry carrying a
  `LocalPath` and nothing else, and `SetStorageConfiguration` was an empty
  success that wrote nothing (`docs/active/mock-audit-2026-07.md` §5, Tier
  3). `DeviceState` gained `storage: Vec<StorageEntry>`; the getter renders
  it and the setter creates, updates or faults.

- **The mock's Media2 metadata family was static on all three operations.**
  `GetMetadataConfigurations`, `GetMetadataConfigurationOptions` and
  `SetMetadataConfiguration` were fixtures (audit §5, Tier 3). `DeviceState`
  gained `metadata: Vec<MetadataEntry>`; all three now read or write it, the
  two token-addressed operations fault on an unknown token, and the
  configurations getter honours its optional `ConfigurationToken` filter.

- **The mock's PTZ `GetStatus` reported a frozen `UtcTime`.** It was the
  literal `2026-04-23T00:00:00Z` — the second hardcoded clock in the mock,
  missed when the `2026-04-15` in `GetSystemDateAndTime` was fixed because
  the two live in different files. Measured at the time of the fix: 99 days
  in the past, growing by a day a day. It now uses
  `soap::security::unix_secs_to_iso8601`, the same conversion as the device
  clock and the WS-Security `Created` header.

- **`PtzConfiguration::default_abs_pan_tilt_space` was spelled wrong on both
  sides, so it never worked in either direction.** `onvif.xsd` names the
  element `DefaultAbsolutePantTiltPositionSpace` — `Pant`, with a double
  `t`. The typo is ONVIF's and it is normative. oxvif read and wrote
  `DefaultAbsolutePanTiltPositionSpace`, so the field came back `None` from
  every conformant device, and `ptz_set_configuration` sent an element no
  schema defines: a strict device rejects the whole request, a lenient one
  drops the field. …

- **The mock's whole audio family was six string literals, and the two
  services disagreed about the same tokens.** `ASC_1` was
  `AudioSourceConfig1` reading `AudioSource_1` on Media1 and
  `AudioSourceConfig` reading `AudioSrc_1` on Media2; `AEC_1` was
  `AudioEncoder` on one and `AudioEncoderConfig` on the other. One device,
  two answers, from two literals in two files — and nothing failed, because
  `tests/mock_media1_media2_agree.rs` had no audio row.

- **The mock's profile renderer dropped any audio configuration but `ASC_1`
  / `AEC_1`.** Both inline renderers were `match token { "ASC_1" =>
  <literal>, _ => String::new() }`, so a profile bound to any other token
  rendered nothing and said so nowhere. `Profile_1` now carries `ASC_1` +
  `AEC_1` from state, and both services must agree about it.

- **`MediaProfile2::audio_encoder_token` was `None` from every conformant
  Media2 camera.** `tr2:ConfigurationSet` names its audio encoder member
  `AudioEncoder`; `MediaProfile2::vec_from_xml` looked for `Audio`, so the
  lookup never matched anything a real device sends. Reading the profile's
  audio encoder binding over Media2 has never worked.

- **Media1 `SetAudioEncoderConfiguration` now refuses an incomplete body.**
  `ter:ConfigModify` / `IncompleteAudioEncoder-SETAEC-5715` when `Multicast`
  or `SessionTimeout` is missing — which is the body oxvif itself sent until
  this release. A mock that accepted it would be the one device on which the
  bug did not show.

- **`get_audio_encoder_configuration_options` returned one empty entry from
  every Media1 device.** The two services nest the response differently and
  the parser only knew Media2's shape:

- **`set_audio_encoder_configuration` sent an element no schema declares and
  omitted two that are required.** `tt:AudioEncoderConfiguration` sequences
  `Encoding, Bitrate, SampleRate, Multicast, SessionTimeout`; oxvif sent
  `Encoding, Bitrate, SampleRate, Channels`. `Channels` is not a member of
  either audio encoder type — it belongs to `tt:AudioSource` — and
  `Multicast` and `SessionTimeout` are **required**, so the request could
  not validate and a strict device would reject it outright. …

- **`AudioEncoderConfiguration::channels` reported mono for every device.**
  It parsed `Channels` with `.unwrap_or(1)` from an element the type does
  not have, so "the device said mono" and "the device said nothing" were the
  same answer. It is now `Option<u32>`, read when a vendor supplies it
  through the type's trailing `<xs:any>` and written back at the end of the
  sequence, which is the only place that wildcard admits it. (In the old
  body it happened to be last too — but only because the two required
  members that belong before it were missing; adding them is what made its
  position a question.) Use `AudioSource::channels` for the physical channel
  count.

- **The mock's PTZ nodes, configurations and coordinate spaces were six
  string literals.** `GetNodes`, `GetNode`, `GetConfigurations`,
  `GetConfiguration`, `GetCompatibleConfigurations` and
  `GetConfigurationOptions` were fixtures whose handlers did not receive the
  request body at all, so every token got the same single node and the same
  single configuration, and `SupportedPTZSpaces` was sent as
  `<tt:SupportedPTZSpaces/>` — an empty element, schema-valid, and a claim
  that the head supports no coordinate space whatever. …

- **The mock's `SetConfiguration` discarded the whole request body.** It was
  `resp_empty` in the dispatcher: the call reported success and
  `GetConfiguration` went on answering the fixture, so a get → modify → set
  → get round trip returned the old values and nothing failed. Every field
  `PtzConfiguration` can carry is now persisted, and an optional element the
  request *omits* is cleared rather than preserved — `SetConfiguration`
  replaces a configuration. …

- **Media2 `AddConfiguration` still rejected `Type="PTZ"`.** Its fault said
  there was "no state to write and no getter that could ever show the
  result" — true when it was written, and false the moment `ProfileEntry`
  gained a PTZ slot and both profile renderers started emitting it. A fault
  whose stated reason has quietly become false is worse than no fault. …

- **The mock's PTZ state was keyed by the wrong thing.** It was the media
  profile token, so `Profile_1` and `Profile_2` — the main and the sub
  stream of *one lens* — were two independent motors: moving one left the
  other reporting its old position. No camera does that. `PtzState.channels`
  is now keyed by **PTZ node token**, and a profile reaches a head the way
  ONVIF says it does:

- **Neither profile renderer emitted the PTZ binding.**
  `MediaProfile::ptz_config_token` and `MediaProfile2::ptz_config_token`
  were parsed and permanently `None`, because `ProfileEntry` had no slot and
  neither renderer emitted the element. Media1 now inlines
  `<tt:PTZConfiguration>` and Media2 ~~emits `<tr2:PTZ token="…"/>`~~
  inlines the same body as `<tr2:PTZ>` (`tr2:ConfigurationSet` types `PTZ`
  as `tt:PTZConfiguration` too, so both come from `ptz::render_config` — see
  the Media2 `GetProfiles` bullet under Added), both from the same state,
  with a new row in `tests/mock_media1_media2_agree.rs`. …

- **`ptz_set_configuration` silently dropped `PanTiltLimits` and
  `ZoomLimits`.** `PtzConfiguration::from_xml` reads both; `to_xml_body`
  never emitted either. A get → modify limits → set therefore changed
  nothing and returned `Ok(())`. Both are now written, in their schema
  position (after `DefaultPTZTimeout`, before `Extension`).

- **Media1 video encoder options nested inside `Extension` were silently
  dropped.** ONVIF extends a type by nesting a same-named element one level
  deeper, so the deeper copy is a superset; `XmlNode::child` returns the
  *first direct* child, so the parser read only the shallow one.
  Consequences on a real device: `BitrateRange` came back absent when the
  device had sent it, and **H265 could never be reported from Media1 at
  all** — it exists only at `Options/Extension/Extension/H265`, so no
  conformant device could have produced it through the old parser.

- The mock's `GetVideoEncoderConfigurationOptions` was emitting
  `BitrateRange` at the top level, a position the schema does not allow.
  That is half the reason the defect survived: the mock and the unit fixture
  agreed with each other and with nothing else. The mock now sends the
  nested shape a real device sends.

- **Media1 and Media2 answered differently for the same mock device.**
  (Reported by a C++ ONVIF test suite driving `MockServer`.) Media2's
  profile family took no `state` at all: `GetProfiles` returned a **string
  literal**, `CreateProfile` returned a literal token and wrote nothing, and
  the dispatcher answered `DeleteProfile` with an unconditional empty
  success. …

- **…and the same defect pointing the other way, in the encoder family.**
  Found by auditing every operation present in **both** dispatchers for
  whether it takes `state`: `SetVideoEncoderConfiguration` **wrote state on
  Media2 and was `resp_empty` on Media1**. So a Media1 encoder write
  reported success and changed nothing, while the identical Media2 call
  changed the device. …

- **Nine more mock writes reported success and wrote nothing.** The two
  above were reported; these came from sweeping the class.
  `SetDiscoveryMode`, both services' `SetVideoSourceConfiguration`, Media1's
  `Add`/`RemoveVideoEncoderConfiguration` and
  `Add`/`RemoveVideoSourceConfiguration`, and Media2's
  `AddConfiguration`/`RemoveConfiguration` were all `resp_empty` in the
  dispatcher, over getters that **are** state-driven. …

- **The mock's Recording, Search and Replay services had no state at all.**
  `grep -c recording src/mock/state.rs` was **0**. `CreateRecording`
  answered `Rec_new` and `GetRecordings` never listed it; `DeleteRecording`
  was an unconditional success that removed nothing; `CreateTrack` returned
  a token that attached to no recording; `SetRecordingJobMode` reported
  success and changed nothing; and `GetRecordingJobState` returned the same
  state for every job token. …

- **The mock's PTZ was profile-blind.** It held **one** position, **one**
  home position, **one** preset list and **one** tour list for the entire
  device, and **not one of its 27 PTZ dispatch arms read the
  `ProfileToken`** — the word appears zero times in the pre-0.15
  `src/mock/services/ptz.rs`. Sixteen arms never received the request body
  at all; the eleven that did took a preset token, a vector or an auxiliary
  command out of it and ignored the profile. …

- **`SetNetworkInterfaces` silently dropped `MTU`.** It read `Enabled`,
  `FromDHCP`, `Address` and `PrefixLength` from the request and wrote all
  four, and ignored the fifth — which the client does send and
  `GetNetworkInterfaces` does report. **A partial write is worse than no
  write**: the state log printed `[STATE] interface updated`, the dispatch
  arm took `state`, and grepping for `resp_empty` never named it.

- **The mock's clock was half-frozen.** `GetSystemDateAndTime` computed the
  time of day from `SystemTime::now()` but hardcoded
  `<tt:Year>2026</tt:Year> <tt:Month>4</tt:Month><tt:Day>15</tt:Day>`, so
  the reported timestamp drifted a day further into the past every day — 106
  days by the time it was noticed. The hours/minutes/seconds looked right,
  which is why nobody read the date.

- **A profile could be bound to an audio configuration that does not
  exist.** `ConfigKind::known_token` validated `VideoSource`, `VideoEncoder`
  and `PTZ` tokens against `DeviceState` and returned `true` unconditionally
  for `AudioSource` and `AudioEncoder`, justified by a comment saying the
  audio families were static fixtures with no catalogue to check against. …

- **Two warnings that only exist under a single feature.**
  `redact::scrub_url_userinfo` was dead code under `--features health` alone
  (the module is `mock` **or** `health`, but only the recorders use it), and
  `use crate::metamorph::SurfaceOp` in `record.rs`'s tests was unused under
  `--features metamorph` alone (its sole consumer is `#[cfg(feature =
  "mock-server")]`).

- **The mock's device-level `GetCapabilities` omitted
  `Device/{Network,System,IO,Security}` and
  `Events/WSSubscriptionPolicySupport` entirely.** Two silent consequences:

- **`SecurityCapabilities` read an element ONVIF does not declare, and
  modelled four of the twelve members it does.** `Capabilities::from_xml`
  (`src/types/capabilities.rs`) built `Device/Security` from five element
  reads. One of them, `UsernameToken`, is declared at **no level** of
  `tt:SecurityCapabilities` — see the `<tt:UsernameToken>` bullet above,
  which established exactly that and then fixed only the mock — so the
  public field was `false` from every conformant camera. …

- `examples/conformance.rs` uses `CapturingTransport`, which is behind the
  `mock` feature, and had no `required-features` entry. A bare `cargo test`
  therefore failed to **compile** — no test ran at all, and the quality gate
  reported nothing wrong.

### Changed

- **`oxvif::mock`'s module docs now state what the mock refuses.** 0.15 made
  the mock materially stricter — per-channel operations require their token
  and refuse an unknown one, a write either persists or faults, responses
  are namespace-well-formed, clocks are real, Media1 and Media2 share one
  state — and none of that was written anywhere a reader of the API docs
  would find it. …

- **The non-ONVIF features are documented example-first.** `health`, `mock`
  and `metamorph` each open with the shortest thing that works and a
  runnable snippet, with the reference detail moved below an explicit
  boundary. Previously the `health` section spent 156 lines on
  `ProfileState` semantics before showing anything a reader could run, and
  never mentioned that the mock can be the target — so trying the crate's
  flagship non-ONVIF feature appeared to require a camera.

- **`examples/healthcheck.rs` gained `--mock`.** One command, no hardware:

- **`src/metamorph/mod.rs` gained doc examples.** It had **none**, despite
  being the headline of two releases, so the docs.rs module page was prose
  only. Three now: clone-and-replay, parse verification + quirk diff, and
  the `DeviceAdapter` skin.

- **`src/health/mod.rs` gained a doc example that actually runs.** Its
  opening example points at a camera and stays `no_run`; the new second one
  drives a real `MockServer`, so `cargo test --doc` proves the documented
  usage works rather than merely compiling it. Doc tests: 42 at 0.15.0.

- **Dependencies brought current.** `base64` **0.22 → 0.23** (the only
  direct dep behind by a major) plus semver-compatible moves for
  `async-trait` 0.1.91, `futures-core` 0.3.33, `serde` 1.0.229, `serde_json`
  1.0.151, `socket2` 0.6.5, `thiserror` 2.0.19, `tokio` 1.53.1, and the
  dev-only `futures` / `toml`. `cargo outdated --depth 1` is now empty;
  `cargo audit` reports zero vulnerabilities across 245 crates.

- **The quality gate now uses `--all-features`.** This crate has no default
  features and `src/mock/` is feature-gated, so the previous `cargo clippy
  --all-targets` and `cargo test` collected only the non-mock subset —
  measured at 461 of 698 lib tests — and a warning inside `src/mock/`,
  `src/health/` or `src/metamorph/` failed nothing. The plain `cargo test`
  is kept as an additional line, because a no-feature build breaking is its
  own bug. …

- **…and a fifth line, `cargo clippy --all-targets -- -D warnings`.** The
  `--all-features` clippy above is a *different compilation*, and neither
  `cargo test` line carries `-D warnings`, so a warning existing only in the
  no-feature build was invisible to all four commands. Measured: an unused
  `use std::sync::Arc;` in `ptz_tests.rs` whose only consumer was
  `#[cfg(feature = "mock")]` passed the whole gate and shipped in 0.15.0. …

- **The mock's action coverage is now enforced, not remembered.** New
  `mock_handles_every_action_the_client_can_send` (`src/mock/dispatch.rs`)
  pulls every SOAP action URI out of `src/client/*.rs` with `include_str!`
  and asserts none falls through to the `Not implemented` fault — **157
  actions at 0.15.0, all routed.** `CLAUDE.md` step 5a has asked for a
  handler per new action since 0.9.6 with nothing checking it; a missing arm
  surfaced only as a `[WARN]` line on stderr of whichever test happened to
  hit it. …

- New coding rules in `CLAUDE.md` for two classes of silent failure found
  this release: **multi-sensor devices** (never omit the token on a
  per-channel query; a single-sensor fixture cannot cover a per-channel
  feature) and **data nested in `Extension` levels** (prefer the deepest
  node, and give the mock the nested shape rather than the flat one).

- `parse_space_range` in `src/types/ptz_config.rs` is now `pub(crate)`,
  reused by the preset-tour option types rather than duplicated.

- **The mock device is now a two-sensor camera.** `DeviceState` gained
  `video_sources`, `video_source_configs` and `video_encoders` (a `Vec`,
  where there used to be one `video_encoder`), holding two sensors, two
  source configs and four encoder configs — `VS_1`/`VS_2`, `VSC_1`/`VSC_2`,
  `VEC_1`…`VEC_4` — plus `Profile_3` and `Profile_4` so both lenses are
  reachable through a profile. …

- **The mock's Imaging service is now per-`VideoSourceToken`.** Every
  operation in that service carries the token and the mock ignored all seven
  of them. `DeviceState::imaging` became `imaging_sources:
  Vec<ImagingState>`, one entry per sensor, and `ImagingState` gained
  `source_token`, `focus_supported` and `level_max`.

- **Fixed: the mock spelled `tt:AFModes` where the schema has
  `tt:AutoFocusModes`.** `tt:FocusOptions20` has no `AFModes` element, so
  `ImagingOptions::focus_af_modes` came back **empty from the mock forever**
  and nothing noticed — the hand-written unit fixture in `imaging_tests.rs`
  spelled it correctly, and the two were never compared. Found by a new
  end-to-end test, not by review.

- **`VideoSource_1` is gone.** That token appeared in the mock's own imaging
  tests, the action snapshot, `mock_workflow.rs` and the mock's event
  payloads, and matched **no entry** in `video_sources`. It was harmless
  only because Imaging ignored the token. All now use `VS_1`.

- **One rendering path per configuration in the mock.** `VEC_2` used to be
  `H264_sub`/H264/640x480 when rendered inline in a profile and
  `SubStream`/JPEG/640x480 when rendered in the configuration list — three
  hardcoded copies of the same object that disagreed. All now render from
  the single state entry. Media1 and Media2 also no longer report different
  encodings for the same token.

- **Every mock `Set` now declares whether it round-trips.** New
  `tests/mock_roundtrip.rs`: **49 `Set → Get` pairs** in one table, public
  API only, over real HTTP, with a fresh `MockServer` per pair. Each row
  declares `Expect::Works`, `Expect::Broken(audit §)` — a real defect — or
  `Expect::Static(audit §)` — a deliberate stub. **All three arms are
  asserted**, so wiring a `Broken` row up turns the test red telling you to
  move it, and the list cannot rot into the permanent blind spot an xfail
  list usually becomes.

- **…and every token-taking operation now declares whether the token selects
  the answer.** New `tests/mock_token_discrimination.rs`: **34 rows** by the
  end of the release (26 when the table landed), each naming two tokens the
  fixture deliberately disagrees on, declaring `Expect::Discriminates` or
  `Expect::Blind(audit §)`. **28 discriminate, 6 are declared blind**,
  pinned as `(34, 28)`.

- **The operation coverage tables moved out of `README.md`** into a new
  top-level **`OPERATIONS.md`**. Ten tables and 104 rows were roughly 8% of
  the README and sat below everything a reader actually navigates to; the
  README's `## Implemented ONVIF operations` section is now a link. **The
  file is eleven tables and 105 rows now** — this release's own DeviceIO
  split added a table, and the release audit found ten implemented
  operations with no row at all (the six OSD calls, `SetScopes`,
  `SetSystemDateAndTime`, `GotoHomePosition`, `SetHomePosition`). …

### Breaking

- **`OnvifClient::get_digital_inputs` takes a `deviceio_url`.** It was
  `get_digital_inputs()`, addressed to the device service.
  `GetDigitalInputs` is a **DeviceIO** operation — see Fixed — and DeviceIO
  is a separate endpoint, so it now takes one the way every other non-device
  service in this crate does.

- **`SecurityCapabilities::username_token` is removed**, and eight fields
  are added. See Fixed. `tt:SecurityCapabilities` — the type the
  device-level `GetCapabilities` answers with — declares that name at no
  level, so the field was `false` from every conformant device and there is
  no element to repoint it at. The fact belongs to the *service*-level
  `tds:SecurityCapabilities`, where it is an `xs:attribute` this crate
  already parses.

- **`MetadataConfigurationOptions::analytics_supported` is removed**,
  replaced by `pan_tilt_status_supported` and `zoom_status_supported`. See
  Fixed: the field read `Options/Extension/AnalyticsSupported`, which no
  ONVIF schema declares, so it was `false` from every conformant device and
  there is no element to repoint it at. The two replacements are the members
  `tt:PTZStatusFilterOptions` requires.

- **`VideoEncoderOptions2::frame_rate_range` is removed and `::frame_rates`
  is now `Vec<f32>`.** See Fixed. `tt:VideoEncoder2ConfigurationOptions`
  declares no `FrameRateRange` at any level, so the field was `None` from
  every conformant device and there is no element to repoint it at; what
  Media2 offers instead is the discrete `FrameRatesSupported` list, whose
  item type is `xs:float`. …

- **`AudioEncoderConfiguration` gained two fields and changed a third.**
  `multicast: Option<MulticastConfiguration>` and `session_timeout:
  Option<String>` are new; `channels` is now `Option<u32>` instead of `u32`.
  See Fixed for why each. Struct-literal construction and any `cfg.channels`
  comparison need updating; `..Default::default()` is not available on this
  type, so the two new fields must be named.

- **`set_audio_encoder_configuration` sends a different request body**, on
  both services: `Multicast` and `SessionTimeout` are now emitted (Media1),
  the `Channels` element is no longer invented, and Media2 orders
  `Multicast` before `Bitrate` where Media1 orders it after `SampleRate`.
  **This is a change against real cameras, not only the mock** — and the
  previous body could not validate against the Media1 schema at all.

- **`ptz_set_configuration` now sends the ONVIF spelling
  `DefaultAbsolutePantTiltPositionSpace`,** and sends `PanTiltLimits` /
  `ZoomLimits`, which it previously dropped. See Fixed. Anything asserting
  on the exact request body — a proxy, a recorded fixture, a conformance
  harness — will see three new element names. **This is a change against
  real cameras, not only the mock.**

- **`ptz_set_configuration` can now return `Err` before it sends anything.**
  `OnvifError::InvalidArgument` when `pan_tilt_limits` is `Some` with
  `y_range: None`: `PanTiltLimits/Range` is a `tt:Space2DDescription` whose
  `YRange` is required, and there is no honest way to render it without one.
  Emitting the element anyway is the exact defect the bullet above removes,
  so the caller is told instead of the device. …

- **The mock is now a two-head PTZ device, and the second head is
  zoom-only.** It seeded four PTZ channels, one per profile; it now seeds
  two nodes (`PTZNode_1`, `PTZNode_2`) and two configurations
  (`PTZConfig_1`, `PTZConfig_2`). `Profile_1` and `Profile_2` share lens 1's
  head; `Profile_3` has lens 2's; `Profile_4` binds no PTZ configuration and
  **every PTZ operation on it now faults** with `ter:NoConfig` /
  `NoPTZConfig-…-5619` instead of answering for an empty channel invented on
  the spot.

- **The mock's `SetVideoSourceMode` now faults instead of succeeding.**
  `ter:ActionNotSupported` / `NotModelled-VSMODE-5813`. It previously
  answered `<tr2:Reboot>false</tr2:Reboot>` while storing nothing. Code that
  calls this against `MockTransport` or `MockServer` and unwraps will now
  fail; treat the fault as "the device does not support sensor-mode
  switching". …

- **`DeviceState` gained `metadata: Vec<MetadataEntry>`,** and the mock
  seeds **two** metadata configurations (`MetaConf_1`, `MetaConf_2`) where
  it previously reported one. `SetMetadataConfiguration` and
  `GetMetadataConfigurationOptions` now fault (`ter:NoConfig`) on a token
  that names no configuration.

- **`DeviceState` gained `storage: Vec<StorageEntry>`,** and the mock now
  seeds **three** storage entries (`SD_01`, `NAS_01`, `CIFS_01`) where it
  previously reported one. Code asserting
  `get_storage_configurations().len() == 1` will fail.

- **`config_token` is now required on five options getters.** The parameter
  changed from `Option<&str>` to `&str` on:
  `get_video_encoder_configuration_options`,
  `get_video_source_configuration_options`,
  `get_video_encoder_configuration_options_media2`,
  `get_video_source_configuration_options_media2`, and
  `get_audio_encoder_configuration_options_media2` — plus the matching
  `OnvifSession` wrappers. …

- **The mock faults on a per-channel request with no token**, rather than
  answering for a default channel: `env:Sender` /
  `NoConfigToken-VECOPT-5507` and siblings. Unknown tokens fault too, and
  the reason names the rejected token. Real devices vary here; the mock is
  strict on purpose, because the omission is a client bug that a permissive
  device hides.

- **`DeviceState::video_encoder` is now `video_encoders:
  Vec<VideoEncoderState>`**, and `VideoEncoderState` gained `source_token`
  and `resolutions`. Both new fields are `#[serde(default)]`, so a persisted
  state file still loads. Media2's `SetVideoEncoderConfiguration` now uses
  the posted token to *select* which channel to write instead of renaming
  the single global config.

- **`PtzState` is now `{ channels: BTreeMap<String, PtzChannel> }`.** The
  seven fields it held at 0.14.0 — `pan`/`tilt`/`zoom`, the three `home_*`
  and `presets` — moved to the new `oxvif::mock::state::PtzChannel`, which
  also carries the `tours` list added earlier in this release.

- **`DeviceState` gained `recording: RecordingState`**, and the Recording,
  Search and Replay services now refuse tokens that name nothing:
  `ter:NoRecording` / `NoSuchRecording-DELREC-5701: <token>` and siblings on
  `DeleteRecording`, `CreateTrack`, `DeleteTrack`, `CreateRecordingJob`,
  `SetRecordingJobMode`, `DeleteRecordingJob`, `GetRecordingJobState` and
  `GetReplayUri`. …

- **The mock faults on a PTZ request with no `ProfileToken`**, or with one
  that names no profile: `env:Sender` / `NoProfileToken-STATUS-5601: every
  PTZ operation is per-profile`, and `ter:NoProfile` /
  `NoSuchProfile-STATUS-5601: <token>`. Same rationale as the per-channel
  media faults above — answering for a default head hides a client bug.

## [0.14.0] - 2026-07-27

Headline: **the metamorph clone becomes something you steer, and the client gets a
correctness pass.** Recording a device is now a two-level selectable read surface —
pick whole service zones or individual `Get*` operations, with prerequisite
tracking, determinate progress and a per-operation outcome report — widening the
default clone from ~12 reads to the full non-destructive `Get*` surface. Alongside
it: six mock operations that never worked, a Media2 request carrying a Media1
prefix, an XML-escaping hole reachable from device-supplied data, and a fixture key
that silently discarded one of any two services sharing a canonical request. Each
fix was proved by a library mutation that had to redden the new test before it was
accepted.

### Breaking

- **`FixtureStore::lookup` now takes the SOAP action as its first argument**
  (`lookup(action, key_canon)`). Callers get `E0061` — deliberately, so the change
  cannot pass silently. There is **no compatibility shim and no deprecation
  cycle**: the pair `(action, key_canon)` is the store's real identity, and a
  key-only lookup cannot disambiguate two services that canonicalise identically.
- **`get_discovery_mode` returns `Err` where it used to return `Ok("")`.** A device
  that omits `<tds:DiscoveryMode>` now yields
  `SoapError::MissingField("GetDiscoveryModeResponse/DiscoveryMode")`. The
  signature is unchanged; callers that read `""` as "unknown mode" now see an
  error. The doc comment always claimed two possible values — `""` was a third the
  documentation denied.
- **`SweepReport::is_complete()` returns `false` for an empty report**, not `true`.
  A sweep that resolved no operation at all no longer reads as a successful sweep.
  A caller gating on it with an empty selection flips.

### Added — pick what to clone (`metamorph`)
- **`SurfaceGroup`** — the seven coarse service zones (identity, network, media,
  PTZ, imaging, events, media2). `ALL` / `label()` / `ops()` let a UI render the
  top level of a "pick what to clone" tree.
- **`SurfaceOp`** — the ~50 individual read operations, the fine-grained level a
  tester needs to reproduce a model-specific quirk on one command. `ALL` /
  `group()` / `action_name()`, plus **`requires()`** — the token-source
  prerequisite (e.g. `GetStreamUri` → `GetProfiles`), so the surface renders as a
  dependency tree and a child lights up its parent when ticked.
- **`SurfaceSelection`** — the user's literal picks (`none` / `all` /
  `recommended` / `from_groups` / `with` / `with_group`); the driver expands
  prerequisites internally, so selecting just `GetStreamUri` still yields a
  replayable clone.
- **`SweepReport`** / **`OpOutcome`** — a per-operation result:
  `Recorded` / `Failed` / `SkippedNoData` / `SkippedPrerequisite`. The "hard
  prerequisite" feedback distinguishes *this device has no such path* from *the
  command itself broke*. Query via `outcome()` / `recorded()` / `skipped()` /
  `is_complete()`.
- **`drive_surface(session, &selection)`** and **`record_surface(url, creds,
  label, &selection)`** — drive / record a chosen subset; both return a
  `SweepReport`. `record_surface` returns `(FixtureStore, SweepReport)`.

### Added — will oxvif parse this device (`metamorph`)
- **Parse verification** — `FixtureStore::verify_parsing()`
  runs oxvif's own typed parser over each recorded response and returns a
  `ParseReport` of `ParseVerdict`s (`ParseStatus::Parsed` + extracted value as
  JSON / `Failed` + parser error / `Unverified`). Answers "will oxvif choke on
  this device", catching value/type quirks the structural diff is blind to (a
  non-integer where an int is expected). Complements — does not replace — the
  structural `diff_against_synthetic` / `diff_details`; both share the
  `(action, key_canon)` key so a UI can join verdict + side-by-side SOAP diff.
  The `metamorph` feature now enables `serde` (was implicit via `mock`) so
  response types serialize to JSON.

### Added — device adapters (`metamorph`)
- **Raw escape hatch** — `DeviceAdapter::respond_raw(op,
  body)` lets an adapter answer any ONVIF operation the typed hooks
  (`identity` / `stream_uri` / `continuous_move`) don't cover, returning a full
  SOAP envelope or `None` to fall through to synthetic. Consulted only after the
  typed hooks decline. Public **`soap_body(xmlns, inner)`** builds the response
  envelope so implementers don't reach into internal mock helpers. Opens
  Persona C to arbitrary per-device operations without waiting on typed hooks
  landing upstream.

### Added — serde coverage
- **`DiscoveredDevice`** and **`DiscoveryEvent`** now derive
  `Serialize` / `Deserialize` under the `serde` feature. They were the only
  always-public data types the feature missed, so `discovery::probe` results
  could not be handed to a REST layer or persisted without a hand-cloned
  parallel struct. Reported by a downstream user.
- The metamorph **surface** and **adapter** data types now derive
  `Serialize` / `Deserialize`, matching `ParseReport` / `QuirkReport` which
  already did: `SurfaceOp`, `SurfaceGroup`, `SurfaceSelection`, `OpOutcome`,
  `SweepReport`, `DeviceIdentity`, `PtzVector`, `AdapterResult`. A UI can now
  persist which operations the user ticked and store a sweep result alongside
  the parse and quirk reports. Unconditional (the `metamorph` feature already
  requires `serde`).

### Added — progress for the long operations (`metamorph`)
- **`drive_surface_with_progress`**, **`record_surface_with_progress`**,
  **`FixtureStore::verify_parsing_with_progress`** and
  **`…::diff_against_synthetic_with_progress`** — determinate progress for the
  four long operations, so a UI can show a real bar instead of freezing for the
  length of a 52-operation sweep. The existing four functions are unchanged and
  delegate with a no-op callback.
- **`SweepProgress`** (`op` / `done` / `total`) and **`FixtureProgress`**
  (`action` / `key_canon` / `done` / `total`) carry the events. The callback is
  `Fn(..) + Send + Sync` so it can feed a channel from an async UI.
- For the sweep, `total` counts **selected operations after prerequisite
  expansion** — not HTTP requests, which are unknowable in advance because a
  per-token operation runs once per token the device returns. Each operation
  ticks exactly once, whether it was attempted or resolved as skipped.

### Added — reports as JSON, and baseline diffing (`metamorph`)
- **`QuirkReport::to_json` / `to_json_pretty`** and **`ParseReport::to_json` /
  `to_json_pretty`** — matching `HealthReport`, so a caller no longer has to pull
  in `serde_json` to persist a report. No new dependency (`metamorph` already
  requires it).
- **`QuirkReport::diff(&prev) -> QuirkDiff`** — compare a run against a saved
  baseline and see only what moved: `appeared` (newly quirky operations),
  `resolved` (no longer deviating), and `changed` (still quirky, but the
  deviating path sets shifted — detailed per side by `ChangedQuirk`). Mirrors
  `HealthReport::diff` / `ReportDiff`. Answers "did this firmware update change
  the device's quirks?" and "are these two same-model cameras quirk-identical?" —
  the practical question now that a full sweep covers 52 operations and
  hand-comparing two reports is not viable. Output is order-deterministic
  (entries keyed and sorted by `(action, key_canon)`, path lists set-sorted), so
  two runs over identical input serialise byte-identically and a JSON baseline
  can be diffed with ordinary text tools.
- `OperationQuirk` now also derives `PartialEq` / `Eq` (additive) so reports and
  diffs compare structurally.

### Changed
- `drive_standard_surface` now sweeps the full non-destructive `Get*` surface
  (per-profile / per-token reads, OSD, audio, PTZ, imaging, events, and Media2
  when advertised) instead of ~12 reads, and returns a `SweepReport`.
  `record_standard_surface` is unchanged (still returns `FixtureStore`).
- **`ParseStatus::Faulted`** — a recorded response carrying a well-formed SOAP
  `Fault` is now classified `Faulted` rather than `Failed`. A device answering
  `NotAuthorized` is behaving correctly; reporting it as "oxvif cannot parse
  this" was wrong and, when sweeping with a restricted account, buried the real
  interop failures under non-problems. `failures()` and `all_parsed()` keep
  their meaning ("oxvif choked") and exclude `Faulted`; the new **`faulted()`**
  iterator surfaces declined operations separately. `ParseStatus` is
  `#[non_exhaustive]`, so the new variant is source-compatible — but any caller
  counting `Failed` verdicts will see faults move out of that bucket.

### Fixed — the mock disagreed with oxvif's own parser
- **Six operations answered with element names that exist in no ONVIF WSDL.**
  `Add`/`RemoveVideoEncoderConfiguration`, `Add`/`RemoveVideoSourceConfiguration`
  shared a made-up `<trt:ConfigurationResponse/>`, and both imaging focus
  operations shared `<timg:ImagingResponse/>`, so all six failed against
  `MockTransport` with `UnexpectedResponse`. Each now has its own dispatch arm and
  its own real response element.
- **`GetOSD`**: the mock wrapped the entry in `<trt:OSDConfiguration>` (the
  schema *type*), but the WSDL element name — and what the client parser reads —
  is `<trt:OSD>`. The mock now emits `<trt:OSD>`, so a clone of the mock's own
  `GetOSD` parses.
- **`GetCompatibleConfigurations`**: the mock reused the `GetConfigurations`
  handler, answering with `<GetConfigurationsResponse>`; the client parser
  matches `<GetCompatibleConfigurationsResponse>`. Added a dedicated mock
  response with the correct wrapper element.
- Every one of these was a disagreement between the mock's own output and oxvif's
  own parser — the hand-written client-test fixtures never re-parsed the mock's
  bytes, so nothing was watching that seam. Real ONVIF devices return the
  schema-correct shapes and always parsed. All now have mock→client round-trip
  regression tests.

### Fixed — what went on the wire, and what got stored

- **`OnvifClient::with_credentials` silently discarded a transport installed by
  `with_transport`.** It assigned a fresh `HttpTransport` over the transport
  field unconditionally, so builder call order was load-bearing and invisible:
  `.with_credentials(u, p).with_transport(t)` kept `t`, while
  `.with_transport(t).with_credentials(u, p)` threw `t` away and went to the
  network — no warning, no `#[must_use]`, no compile error. A mock stopped
  mocking; a `CapturingTransport` stopped capturing, leaving an empty output
  directory and no error. The default transport is now resolved on first use
  rather than in the builders, so **the two orders are equivalent** and an
  installed transport is never replaced. HTTP Digest credentials still apply to
  the default transport; a custom transport is left to its own transport-level
  authentication, and the WS-Security header is added either way.
  `OnvifSession::builder` was never affected — it normalises the order in
  `build()` — so only direct `OnvifClient` users could hit this.
- **`set_audio_encoder_configuration_media2` sent a Media1 prefix.**
  `AudioEncoderConfiguration::to_xml_body()` hard-codes `trt:`, so a `tr2:`
  request carried a `<trt:Configuration>` child. Adds `to_xml_body_media2()`;
  Media1 output is unchanged and frozen by tests.
- **`xml_escape` was bypassed through `Display`.** `VideoEncoding::Other(String)`
  and `AudioEncoding::Other(String)` return the device's raw string, so four
  serialisers put an unescaped device-echoed encoding on the wire — a device
  reporting an encoding containing `&`, `<` or `"` produced malformed XML. All
  four sites now escape. Invisible to a `grep xml_escape` audit, which is why it
  survived.
- **Fixtures are keyed on `(action, key_canon)`, not the canonical request alone.**
  Media1's `<trt:GetProfiles/>` and Media2's `<tr2:GetProfiles/>` canonicalise
  identically, so one silently overwrote the other. **The on-disk format does not
  change** and old `fixtures.json` files keep loading; what an old clone cannot
  recover is the exchanges that were never written — 4 of the 64 in a recommended
  sweep. Those four now replay as an honest miss instead of returning the other
  service's envelope, so an un-re-recorded clone gets *less* wrong, not more.
  Re-recording is still recommended and is the only way to fill the gap.

### Security

- **`CapturingTransport` wrote credentials to disk, and the docs told you to
  commit them.** It wrote each SOAP request verbatim, so an authenticated
  capture put the WS-Security `UsernameToken` — username, nonce and password
  digest — into `<action>.req.xml`, and a `GetStreamUri` response put
  `rtsp://user:pass@host/…` into `<action>.resp.xml`. The module's stated
  workflow was "commit those fixtures under `tests/fixtures/<vendor>-<model>/`".

  It now redacts by default, using the same two transforms `HealthCheck`'s
  capture and metamorph's `FixtureStore` already applied: `<wsse:Password>` and
  `<wsse:Nonce>` are blanked to `[redacted]`, and `user:pass@` is stripped from
  URLs in responses. `<wsse:Username>` and `<wsu:Created>` are kept — not secret,
  and what makes a capture readable. **Only the bytes written to disk change;
  the device still receives the request unmodified.**

  Two opt-outs for the case that needs the untouched bytes — debugging
  WS-Security itself, where the digest is the evidence:
  `CapturingTransport::with_raw_requests()` and `with_raw_responses()`. They are
  independent, and a directory recorded with either holds a live credential.

  **If you committed captures recorded by an earlier version, treat those
  `*.req.xml` as leaked credentials and rotate the device account.** Re-recording
  with 0.14 produces redacted files; existing files are not rewritten. Note that
  replayed `GetStreamUri` results now carry no userinfo — a test asserting the
  full `rtsp://user:pass@…` form needs `with_raw_responses()`. Gated on the
  `mock` feature, which is not on by default.

- Internal: the redaction transforms had three near-identical copies (`health`,
  `metamorph`, and none in `fixtures`). They now live once in a crate-private
  `redact` module that all three use, so a fix reaches every recorder. Proved by
  neutralising both functions there and confirming all nine redaction tests
  across the three consumers turn red.

#### Advisory note — no change to this crate

Four advisories were open against transitive dependencies at release time, all
reached through `reqwest`:

- `quinn-proto` < 0.11.15 — RUSTSEC-2026-0185, 7.5 high: remote memory exhaustion
  via unbounded out-of-order stream reassembly.
- `rustls-webpki` < 0.103.13 — RUSTSEC-2026-0098 and -0099, name constraints
  incorrectly accepted for URI names and for wildcard certificates;
  RUSTSEC-2026-0104, reachable panic in CRL parsing.

**Nothing in this crate changed and nothing needed to.** `oxvif` never named those
crates; its `reqwest` requirement is a caret range that already permits the fixed
versions, so a fresh resolve picks them up. `Cargo.lock` is not tracked here — a
library does not ship one — so this release cannot pin them for you. **If your
lockfile predates 2026-07-26, run `cargo update -p quinn-proto -p rustls-webpki`;
upgrading `oxvif` alone will not do it.**

### Documentation

- **docs.rs was missing the entire `metamorph` module.** The crate has no
  default features, and `[package.metadata.docs.rs]` listed only
  `["mock-server", "health"]` — so every release since `metamorph` landed
  rendered without it, and the record / replay / clone surface (`FixtureStore`,
  `SurfaceOp`, `SurfaceSelection`, `DeviceAdapter`, parse verification, quirk
  diff) simply did not appear in the published docs. Now `all-features = true`,
  which cannot drift the same way when a future feature is added.
- **Every public item in `oxvif::types` is documented.** 159 response-struct
  fields, enum variants and constants had no doc comment — the types a caller
  actually reads back from a device. `types` now carries `#[warn(missing_docs)]`
  so the next undocumented field is visible at build time. The rest of the crate
  is not yet clean (164 items remain, 123 of them in the test-only
  `mock::state`); the lint is scoped rather than crate-wide to say so honestly
  instead of silencing it.

### Testing

No public API change, but the suite is materially different: 64 client methods
that reported as covered were not. 28 had no test asserting anything about their
outcome, and 36 had a negative test that could not fail for the reason it was
written — `assert!(res.is_err())` stays green when a `Fault` becomes an
`UnexpectedResponse`. All 64 now assert the payload: the fault's `code` and
`reason`, or the exact `MissingField` path. 706 → 734 tests, none removed or
renamed. Each batch was measured before and after with the same two library
mutations, so the improvement is a diffed set of failing test names, not a count.

`CLAUDE.md` now bans hollow tests outright and states how to prove a new
assertion is load-bearing.

Three further tests came with the `with_credentials` / `with_transport` fix
above, covering the interaction rather than either method alone: both builder
orders route through the installed transport, and setting credentials discards
a default transport that an earlier request had already built. Each was proved
by restoring the specific defect it targets and watching that one test — and
only that one — fail on its assertion.

---

## [0.13.0] - 2026-07-24

Headline: **metamorph — clone a real camera, replay it (in-process or over a
bound port), and diff its quirks** — plus opt-in `serde` on every response type,
ONVIF-schema fixes to the mock, correct Media2 discovery, and raw-SOAP capture
for failing health checks. Everything is additive and feature-gated; the ONVIF
client's public API is unchanged.

### Added (`metamorph` feature)
- **`metamorph` feature** (a superset of `mock`) and the `oxvif::metamorph`
  module — Persona B record/replay:
  - **`FixtureStore`** — a device's recorded SOAP exchanges as one
    `fixtures.json`, keyed by the **canonical, ephemera-masked request**, so
    `GetProfile(token=A)` and `(token=B)` never collide while MessageID / nonce
    / timestamps and the `wsa:To` endpoint don't fragment the key. Loads whole
    into memory; recorded requests have WS-Security `Password`/`Nonce` blanked.
  - **`ReplayResponder`** — answers reads from fixtures; writes pass through to
    the synthetic device state and invalidate that operation family's replay
    (coarse copy-on-write), so `Set → Get` round-trips.
  - **`MetamorphTransport`** — an in-process replay device (`Transport`), the
    client-drivable counterpart of `MockTransport`.
  - **`RecordingTransport`** — taps a live transport into a `FixtureStore`;
    `examples/metamorph_record.rs` is the recorder CLI.
  - **`record_standard_surface(url, credentials, label)`** — one call clones a
    camera's standard read surface into a `FixtureStore` (builds the session +
    recording tap internally), so a caller like oxdm needs no copy of the op
    list; `drive_standard_surface(&session)` exposes just the op list for callers
    that own the session. The example is now a thin wrapper over it.
  - Recorded fixtures now also strip `user:pass@` **URL credentials** (e.g. an
    `rtsp://` stream URI from `GetStreamUri`) from both request and response, so
    a saved `fixtures.json` carries no stream/snapshot password — not just the
    WS-Security ones.
  - `Fixture`, `FixtureStore`, `MetamorphTransport`, `ReplayResponder`,
    `RecordingTransport` are re-exported from the crate root.
  - **Persona C — adapter / skin**: implement the **`DeviceAdapter`** trait
    (`identity` + `stream_uri` required; `continuous_move` / `snapshot`
    optional) to put an ONVIF skin on a non-ONVIF (e.g. RTSP-only) device.
    **`AdapterResponder`** answers GetDeviceInformation / GetStreamUri /
    ContinuousMove from the adapter and falls through to the synthetic mock for
    everything else; **`AdapterTransport`** drives it in-process. See
    `examples/metamorph_adapter.rs`. `DeviceIdentity`, `PtzVector`,
    `AdapterResult` are re-exported too.
  - **Structural quirk diff** — **`FixtureStore::diff_against_synthetic`**
    replays each recorded request through the synthetic **reference** mock and
    diffs the two responses' element-path sets (the SOAP `Header` is excluded, so
    it reflects response *Body* shape), returning a serde-serialisable
    **`QuirkReport`** (`OperationQuirk` per drifting op: `only_in_clone` /
    `only_in_synthetic` paths). Surfaces where the real camera's response *shape*
    differs from what oxvif emits/expects — a proxy for "will oxvif parse this
    device", **not** an ONVIF-schema-conformance verdict. Structure only (not
    values). **`FixtureStore::diff_details` → `Vec<OperationDiff>`** additionally
    renders each operation's baseline and clone responses as aligned,
    pretty-printed XML for a git-style side-by-side diff, with transport
    ephemera, tokens, and IPv4/IPv6/MAC literals (incl. IPs inside URLs)
    normalised so instance-specific values don't show as differences.
    `FixtureStore::fixtures()` exposes the recorded set. `QuirkReport`,
    `OperationQuirk`, `OperationDiff` re-exported.

### Added (`metamorph-server` feature)
- **Serve a clone from a bound port — the "container".**
  `MockServer::builder().replay(FixtureStore).start()` turns the HTTP mock
  server into the recorded camera: reads replay the clone's real responses
  (quirks and all) via the replay responder spliced into the server's chain,
  while writes and unrecorded operations fall to synthetic `DeviceState` with the
  same coarse copy-on-write (`Set → Get` still round-trips). Any HTTP ONVIF
  client — oxdm, ONVIF Device Manager, Frigate — or oxvif's own `HealthCheck`
  can drive the clone at `device_url()`. The feature is just
  `metamorph` + `mock-server`. See `examples/metamorph_serve.rs`.

### Added (`mock` feature)
- **`mock::{Chain, Responder, RequestCtx}`** — the mock device now answers each
  request through an ordered chain of responders (fault → auth → synthetic by
  default). The trait is the stable seam personas extend; behaviour of the
  default pipeline is byte-for-byte unchanged.

### Added (`mock-server` feature)
- **WS-Discovery responder** — `MockServer::builder().discoverable(scopes)`
  makes a bound server answer WS-Discovery `Probe`s (best-effort :3702 +
  multicast), so a client (oxdm, ONVIF Device Manager, Frigate) finds it on the
  LAN. `mock::DiscoveryResponder` is also usable standalone.
- **Multi-device fleet** — `mock::Fleet` runs several independent `MockServer`s
  at once, each on its own ephemeral port with a distinct identity (hostname /
  model / serial). `Fleet::start(n)` for `n` default cameras or
  `Fleet::builder()` to mix in caller-seeded `DeviceState`s; `device_urls()`
  feeds a batch scanner directly. Dropping the fleet shuts every device down.
  See `examples/mock_fleet.rs` (metamorph M6).

### Added (`serde` feature)
- **`serde` feature** — derives `serde::Serialize` + `Deserialize` on every
  public response type in `oxvif::types`, so they can be returned directly from
  a REST handler (`axum::Json(session.ptz_get_presets(..).await?)`) or persisted
  as JSON, instead of hand-cloning parallel structs just to attach serde.
  Implemented with `#[cfg_attr(feature = "serde", derive(..))]`: **opt-in, with
  no new dependency and zero cost unless enabled.** Field names are the
  Rust-native snake_case identifiers (no `rename_all`). Resolves a
  user-reported gap.

### Added (`health` feature)
- **`HealthCheck::with_capture(true)`** records the raw request/response of every
  SOAP call that **fails** (a transport error or a SOAP Fault) into the new
  `HealthReport::captured` field (`Vec<CapturedExchange>`). Off by default.
  Successful — and therefore credential-bearing — requests are not stored, and
  the requests that are stored have their WS-Security `Password`/`Nonce` blanked,
  so a capture never carries credential-derivation material. Each entry keys on
  the SOAP action (`GetStreamUri`, …), keeping the latest failure per action.
- **`CapturedExchange`** exported from the crate root and `health`.

### Changed
- `HealthReport` gained a `captured: Vec<CapturedExchange>` field (additive;
  serialises only when non-empty, deserialises to empty when absent). Code that
  constructs `HealthReport` with a struct literal must add the field — in 0.x a
  minor bump is the SemVer signal for this.

### Fixed
- **Media2 is now discovered via `GetServices`** — Media2 is a GetServices-only
  service in the ONVIF spec, but the session build only read the *non-standard*
  Capabilities `Media2` extension, so standards-compliant cameras (which
  advertise Media2 only in `GetServices`) were mis-detected as having no Media2.
  The session build now fills `media2.url` from `GetServices` when
  `GetCapabilities` didn't provide it (dual-source, non-breaking — a device that
  does advertise it in Capabilities still works, that URL wins). `mock-server`'s
  non-standard `<tt:Media2>` in `GetCapabilities` was removed (it's advertised
  via the mock's `GetServices`, `ver20/media/wsdl`).
- **Mock read-surface responses are now ONVIF-schema conformant** (`mock`) — an
  audit against `onvif.xsd` fixed real violations that also skewed the clone
  quirk baseline: `GetNetworkInterfaces` emitted a boolean `<FromDHCP>` and
  omitted the required `<DHCP>` boolean (now `<Manual>` + `<DHCP>`, matching
  oxvif's own parser); video-encoder configs (`GetVideoEncoderConfigurations`
  and the `GetProfiles`-nested ones) omitted the required `Multicast` +
  `SessionTimeout` (added, in XSD order); `GetCapabilities` and
  `GetImagingSettings` children were out of `xs:sequence` order (reordered).

---

## [0.12.0] - 2026-07-09

Headline: **BREAKING — the HealthCheck report is reshaped so "couldn't verify"
is never mistaken for "non-conformant"**, plus opt-in active liveness probing, a
stronger Profile T assessment, and a negative auth-enforcement probe. In 0.x a
minor bump is the SemVer signal for a breaking change; only the `health`
feature's `HealthReport` JSON shape changes here — the rest of the crate (the
ONVIF client) is untouched. 0.11's health output was explicitly provisional (see
that release's note).

### Changed (breaking — `health` feature)
- **Profile assessment is an object, not a tuple.** `ProfileAssessment::profile_{s,t,g}`
  changed from `(ProfileVerdict, Vec<String>)` to `ProfileState { verdict, missing,
  unverified }`. JSON goes from `["Conformant", []]` to `{ "verdict": "conformant" }`.
- **`ProfileVerdict` gained `Inconclusive`** and now serialises lowercase
  (`conformant` / `partial` / `unsupported` / `inconclusive`). A profile whose
  required checks couldn't be tested (auth blocked / skipped), with nothing
  verified to fail, is now `Inconclusive` — not `Partial`. The ids behind a
  verdict are split into `missing` (verified fail) vs `unverified` (couldn't test),
  so an auth failure is never counted as a conformance failure.
- **`CheckStatus` `kind` serialises lowercase** (`pass` / `warn` / `fail` / `skip`).
- **`CheckResult::elapsed` is now `Option<Duration>`**; `elapsed_ms` serialises as
  `null` for a check that never ran (e.g. `Skip`) instead of an ambiguous `0`.

### Added (`health` feature)
- `health::ProfileState` — the per-profile `{ verdict, missing, unverified }`.
- `CheckError::is_auth()` — the single source of truth for "is this failure an
  authentication/authorization problem?", used by the assessment to route a
  check to `unverified` vs `missing` (so callers can't drift from oxvif's rule).
- **`HealthCheck::with_liveness_probes(bool)`** (default `false`) — opt-in active
  probing that verifies results actually *work*, not just that the device
  answered the SOAP call. When enabled:
  - `get_stream_uri` follows the resolved RTSP URI with a non-destructive RTSP
    `OPTIONS` reachability probe (a resolved URI is no guarantee the server
    answers). `200`/`401` count as reachable; otherwise the check downgrades to
    `Warn` with the reason.
  - `get_snapshot_uri` fetches the snapshot bytes and validates them as a real
    image (JPEG/PNG/BMP magic) — a 0-byte body or an HTML error page returned
    with a `200` is flagged, not counted as a pass. Authenticates with a manual
    HTTP Digest handshake that quotes `qop="auth"` (some Hikvision/Uniview
    firmware reject the unquoted `qop=auth` and answer with a non-image `200`),
    falling back to Basic auth when the device does not offer Digest.
  - the `recording` / `search` / `replay` checks genuinely exercise Profile G
    (recording list + recording search + replay-URI resolution) instead of
    reporting advertised-only presence, so the Profile G verdict reflects real
    behaviour.

  With liveness off, every check keeps its exact prior behaviour (URI-scheme
  check / advertised-only Profile G); no report-shape change.
- **Stronger Profile T assessment.** Two new checks gate the Profile T verdict so
  a Profile-S-only device is no longer read as near-T:
  - `media2` — whether the device advertises the Media2 (`ver20/media`) service,
    checked via **GetServices** as well as the GetCapabilities extension (many
    devices only list Media2 in the former).
  - `event_motion_topic` — whether `GetEventProperties` exposes a motion-alarm
    topic.
- **Verdicts distinguish "not supported", "couldn't verify" and "not
  applicable".** A required capability the device doesn't advertise is a
  definitive gap (`missing` → `Unsupported`/`Partial`); `Inconclusive` is
  reserved for genuinely untestable checks (auth-blocked); and a service that is
  present but had no data to exercise (a recording device with zero recordings
  to replay) is **excluded** from the verdict, so it neither fails nor clouds a
  device whose other checks all pass.
- **`HealthCheck::with_force_unsupported(bool)`** (default `false`) — force-verify
  services the device does *not* advertise. For each unadvertised profile-gating
  service (Media2, recording / search / replay) it tries a few conventional
  service URLs (derived from the device endpoint, plus the endpoint itself for
  single-endpoint devices) and actually calls the operation; one that responds is
  flagged `Warn` as **under-declared** — the mirror of "declares a profile but
  it's broken". Best-effort: vendors use non-standard paths, so a miss is not
  proof of absence.
- **Negative security probe (`auth_enforcement`, new `Category::Security`).**
  When credentials are supplied, a credential-free `GetDeviceInformation` call
  confirms the device actually enforces authentication. Serving device info
  anonymously is flagged as a security `Warn`; an auth rejection is the healthy
  `Pass`; anything else is `Skip` (undetermined).

- **`HealthReport::to_junit_xml()` / `to_junit_testsuite(name)`** — render a run as
  JUnit XML, the de-facto test-result format every CI and dashboard ingests. Each
  check is a `<testcase>` (`Fail` → `<failure>` carrying the ONVIF subcode as
  `type`, `Skip` → `<skipped>`, `Warn` → a passing case with a `<system-out>`
  note) and each profile verdict is a testcase too (`Partial`/`Unsupported` →
  failure, `Inconclusive` → skipped). `to_junit_testsuite` emits a bare
  `<testsuite>` for composing multi-device documents.

### Migration
- `HealthReport` JSON written by ≤0.11 (e.g. saved baselines) will not
  deserialize into the 0.12 shape — regenerate it by re-running the check.

## [0.11.0] - 2026-07-03

Headline: **structured error facts on the health report**, so a cross-brand
conformance corpus can group the same fault across vendors (by ONVIF subcode)
instead of re-parsing free-text reasons — and separate genuine device faults
from client-side preconditions.

### Added
- `health::CheckError` + `health::ErrorClass` — structured facts attached to a
  failing `CheckResult` via a new `error` field: `class`
  (`soap_fault` / `precondition` / `parse` / `http` / `invalid_argument`),
  `fault_code`, ONVIF `subcode` (e.g. `ter:NotAuthorized`), `reason`, and
  verbatim `detail`.
- `HealthReport::clock_skew_s` — the numeric device-vs-local clock skew
  (previously only formatted into the `system_date_time` check string).
- `HealthReport::declared_profiles` — the ONVIF profiles the device
  self-declares via its scopes (canonical letters, e.g. `["S", "T", "G"]`),
  read best-effort from `GetScopes`. Independent of the *assessed* `profiles`
  verdicts, so consumers can flag "declares Profile G but replay/search fail".
- SOAP fault parsing now extracts `Code/Subcode/Value` and `Detail`, which were
  previously discarded.

### Changed
- **Breaking (source):** `SoapError::Fault` gained `subcode: Option<String>`
  and `detail: Option<String>` fields. Its `Display` string is unchanged;
  exhaustive matches on the variant must add `..`.

### Security
- Upgraded `quick-xml` `0.39` → `0.41`, clearing RUSTSEC-2026-0194 (quadratic
  run time when checking a start tag for duplicate attribute names) and
  RUSTSEC-2026-0195 (unbounded namespace-declaration allocation in `NsReader`,
  a memory-exhaustion DoS). Migrated the two changed APIs used in
  `soap::xml`: `BytesText::xml_content()` → `xml10_content()` and the
  deprecated `Attribute::decode_and_unescape_value` →
  `decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)`.
- Refreshed the dependency tree (`cargo update`), pulling `quinn-proto`
  `0.11.15` (RUSTSEC-2026-0185, remote memory exhaustion) and `rand` `0.8.6`
  (RUSTSEC-2026-0097 unsoundness). `cargo audit` is now clean.

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
