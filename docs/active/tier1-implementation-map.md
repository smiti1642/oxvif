# Tier 1 implementation map (0.15.0)

Status: **written 2026-07-28, not started.** Companion to
[service-capabilities-and-ptz-tours.md](service-capabilities-and-ptz-tours.md).

That document decides *what* and *why*. This one is the **對照表** — the
correspondence tables that make the writing mechanical: for every attribute in
every one of the twelve Stage 0 types, the exact WSDL name, the exact Rust field
name, the exact value the mock already emits, and the test that pins it.

Why it exists: the dominant failure mode in this stage is not a design mistake,
it is a **one-character mismatch that fails silently**. An attribute name wrong
by one letter parses as "absent" forever, and with `Option<bool>` fields that
reads as a legitimate `None`. No test fails. The camera looks like it declined
to answer. `AdaptablePreset` / `AdaptivePresets` (§2.6) is the case that
motivated the Stage 0 verification, and the same trap exists nine times over
here. Every table below is therefore three-column-minimum: **schema string →
Rust identifier → fixture value**, so a mismatch is visible by reading across
one row instead of by opening three files.

Read §1 before writing any code — it changes the quality gate.

---

## 1. Gate correction: `cargo test` does not run the mock tests

**This is a defect in the current workflow, found while writing this plan, and
it affects work already committed.**

`src/mock/` is behind `#[cfg(feature = "mock")]` (`src/lib.rs:239`) and the
crate has **no default features**. So the plain `cargo test` in the CLAUDE.md
quality gate never compiles the mock module. Measured on `1d224f4`:

| Command | lib tests collected |
|---------|--------------------:|
| `cargo test --lib` | 461 |
| `cargo test --all-features --lib` | 698 |

237 tests — including **all nine `GetServiceCapabilities` mock tests added in
`1d224f4`**, the whole reason that commit exists — are invisible to the gate as
written. Filtering proves it directly:

```
cargo test --lib every_service_answers               → 0 passed, 461 filtered out
cargo test --all-features --lib every_service_answers → 1 passed, 697 filtered out
```

**Binding for this work: the gate is**

```
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Both `--all-features` additions matter. `clippy --all-targets` without
`--all-features` lints the same 461-test subset, so a warning inside
`src/mock/` or `src/health/` does not fail the gate either.

Keep running the plain `cargo test` as well — a no-feature build breaking is
its own bug — but it is no longer sufficient on its own.

> **Update (2026-07-28, same day):** the plain `cargo test` did not merely skip
> the mock tests, it **did not compile**. `examples/conformance.rs` uses
> `oxvif::CapturingTransport`, which is `#[cfg(feature = "mock")]`, and had no
> `required-features` entry in `Cargo.toml`, so a bare `cargo test` failed with
> E0432 before running anything. Fixed by adding the entry; plain `cargo test`
> now builds and reports 463 lib tests. Both halves of the gate run again.

> Follow-up, out of scope here: CLAUDE.md's "Before every commit" block should
> be amended. Do that as its own docs commit, not folded into a Stage commit.

---

## 2. Global decisions (binding for every table below)

Each of these is a decision the tables assume. Changing one means revisiting
every table.

### 2.1 `Option<bool>`, never bare `bool`

Per plan §3.2. `None` = the attribute was absent; `Some(false)` = the device
said no. Health's claim-vs-behaviour checks are the entire point of Stage A and
they cannot work if the two collapse.

Consequence for the tables: a fixture row reading **`(omitted)`** must be
asserted as `None`, and that assertion is load-bearing — it is what proves the
parser is not defaulting.

### 2.2 List-typed attributes are `Vec<String>`, not `Option<Vec<String>>`

`tt:StringList` / `tt:StringAttrList` attributes (`MoveAndTrack`, `Encoding`,
`ConfigurationsSupported`, `AuxiliaryCommands`, `SupportedExportFileFormats`, …)
parse to `Vec<String>`, split on ASCII whitespace, **empty when absent**.

This is a deliberate exception to §2.1. For a list, "absent" and "present but
empty" mean the same thing — no items — so an `Option<Vec<_>>` would offer a
distinction with no meaning behind it and force every caller through a double
unwrap. Say this in the module doc comment so it does not read as an oversight.

### 2.3 Numeric attributes follow the schema type exactly

| Schema type | Rust | Appears in |
|-------------|------|------------|
| `xs:int` | `Option<u32>` | `NTP`, `MaxUsers`, `MaxPullPoints`, `MaxRecordingJobs`, `WebRTC`, … |
| `xs:float` | `Option<f32>` | `MaxRate`, `MaxTotalRate`, **`MaxRecordings`** |
| `tt:FloatList` | `Option<(f32, f32)>` | `SessionTimeoutRange` only |
| `xs:duration` | `Option<String>` | `BeforeEventLimit`, `AfterEventLimit` |
| `xs:anyURI` | `Option<String>` | `RTSPWebSocketUri` |

Two traps in that table:

- **`MaxRecordings` is `xs:float`**, not `xs:int`, in `trc:Capabilities`. It
  reads like a count and it is not. Type it `Option<f32>` and match the schema;
  do not "fix" it to `u32`.
- **`WebRTC` is `xs:int`**, not `xs:boolean` — it is a session count.
  `Some(0)` means "supported, zero concurrent sessions"; parsing it as a bool
  reports `0` as supported and loses the number. The mock deliberately sends
  `WebRTC="0"` (§3.4) to catch exactly this.

`xs:duration` stays a `String` to match the existing crate convention
(`src/types/recording.rs:231`, `src/client/recording.rs:250` — ISO 8601 strings
in and out). Do not introduce a duration type in this stage.

`SessionTimeoutRange` is the only `tt:FloatList`; it is a whitespace-separated
min/max **pair carried in the attribute**, not a `Min`/`Max` sub-tree. Parse to
`Option<(f32, f32)>`; a malformed or non-pair value yields `None`.

### 2.4 Dotted attribute names

Four attributes are not legal Rust identifiers:

| Schema attribute | Rust field |
|------------------|------------|
| `TLS1.0` | `tls1_0` |
| `TLS1.1` | `tls1_1` |
| `TLS1.2` | `tls1_2` |
| `X.509Token` | `x509_token` |

The **parser must still pass the dotted string** to `.attr()`. Precedent is
already in the tree: `src/types/capabilities.rs:341,344` does exactly this for
the device-level equivalents. Renaming the lookup string to match the Rust field
is the silent-failure bug this whole document exists to prevent.

### 2.5 Negative-sense flags keep their schema names

`DiscoveryNotSupported`, `NetworkConfigNotSupported`, `UserConfigNotSupported`
→ `discovery_not_supported`, `network_config_not_supported`,
`user_config_not_supported`.

Do **not** invert them to positive sense. Inverting makes `None` ambiguous:
absent means *supported* for these three and *unknown* for every other field in
the same struct, and one `!` in a parser would silently swap the meaning of a
health verdict. Keep the schema polarity and carry the awkwardness in a doc
comment.

### 2.6 `OnboardStorage` — the one attribute with a schema default

`trc:Capabilities/@OnboardStorage` has `default="true"` in the XSD. It is the
only defaulted attribute across all nine types.

**Decision: parse it like every other attribute — absent is `None`** — and say
in the field's doc comment that the schema default is `true`, so a caller
reading `None` should treat it as `true`.

Rationale: applying the default in the parser would make this the one field
where `None` is unreachable, breaking the §2.1 invariant that the whole struct
depends on, and it would erase the (real, common) difference between firmware
that answered and firmware that did not. The mock omits it deliberately so the
`None` path is the one under test.

### 2.7 Spellings that were verified and must not be "corrected"

Every one of these looks wrong and is right. Verified against the published
schema in Stage 0 (`00d73a2`). Changing any of them re-introduces a silent
parse failure:

| Correct | Plausible wrong form | Where |
|---------|---------------------|-------|
| `AdaptablePreset` | `AdaptivePresets` | `timg:Capabilities` |
| `tr2:Capabilities2` | `tr2:Capabilities` | media2 type name only — the *element* is still `Capabilities` |
| `MoveAndTrack` | `MoveAndStartTracking` | `tptz:Capabilities` (the latter is an operation name) |
| `EXICompression` | `EXICompressionSupported` | `trt:Capabilities` |
| `MetadataOverMQTT` | `MetadataOverMqtt` | `tev:Capabilities` |
| `NLSearch` | `NlSearch` | `tse:Capabilities` |
| `RTP_TCP`, `RTP_RTSP_TCP` | `RTPTcp` | Media1 streaming |

And one absence to preserve: **`tev:Capabilities` has no `WSPullPointSupport`**.
That name belongs to the device-level `tt:EventCapabilities`, which oxvif
already parses into `EventsCapabilities::ws_pull_point`
(`src/types/capabilities.rs:376`). Adding it to the service-capabilities struct
would create two fields with one name and different provenance — precisely the
confusion §3.1 of the parent plan sets out to avoid. The nearest question here
is answered by `MaxPullPoints`.

---

## 3. Stage A — `GetServiceCapabilities` × 9

### 3.0 File and symbol map

| Concern | File | Action |
|---------|------|--------|
| Types | `src/types/service_capabilities.rs` | **new** — 9 top-level structs + 7 nested |
| Module wiring | `src/types/mod.rs` | `mod service_capabilities;` + `pub use service_capabilities::*;` |
| Re-export | `src/lib.rs` | add all new type names to the public re-export list |
| Doc amendment | `src/types/capabilities.rs` | amend the doc comment of all 11 existing `*Capabilities` structs to name `GetCapabilities` as their source |
| Client methods | `src/client/{device,media,media2,ptz,imaging,events,recording}.rs` | 9 methods (recording.rs carries 3) |
| Session wrappers | `src/session.rs` | 9 wrappers |
| Mock | — | **already done** in `1d224f4` |
| Tests | `src/tests/client/{device,media,media2,ptz,imaging,events,recording}_tests.rs` | 18 tests |

Note the recording file carries **three** services (Recording, Search, Replay),
matching the existing `src/client/recording.rs` layout and the three-way
dispatcher split already landed in `src/mock/dispatch.rs`.

### 3.1 Struct inventory

```
DeviceServiceCapabilities          { network, security, system, misc }
  ├ DeviceNetworkCapabilities
  ├ DeviceSecurityCapabilities
  ├ DeviceSystemCapabilities
  └ DeviceMiscCapabilities
MediaServiceCapabilities           { …attrs, profile, streaming }
  ├ MediaProfileCapabilities
  └ MediaStreamingCapabilities
Media2ServiceCapabilities          { …attrs, profile, streaming }
  ├ Media2ProfileCapabilities
  └ Media2StreamingCapabilities
PtzServiceCapabilities             (flat)
ImagingServiceCapabilities         (flat)
EventsServiceCapabilities          (flat)
RecordingServiceCapabilities       (flat)
SearchServiceCapabilities          (flat)
ReplayServiceCapabilities          (flat)
```

Nested names are **prefixed** (`MediaStreamingCapabilities`, not
`StreamingCapabilities`) because `StreamingCapabilities` already exists in
`src/types/capabilities.rs:86` as the *device-level* type with a different field
set — three same-named types across device / Media1 / Media2 is the trap called
out in `docs/reference/media1.md:126`. One Rust struct must never be shared
between them.

Every struct derives `#[cfg_attr(feature = "serde", derive(Serialize,
Deserialize))] #[derive(Debug, Clone, Default)]`, matching the neighbouring
file.

### 3.2 Parse helpers

Four private helpers at the top of `src/types/service_capabilities.rs`. This
module is the first to parse **attributes** rather than child elements, so the
existing `xml_bool` / `xml_u32` / `xml_str` in `src/types/mod.rs` do not apply
and must not be reused or widened.

| Helper | Returns | Notes |
|--------|---------|-------|
| `attr_bool(n, name)` | `Option<bool>` | `"true"` or `"1"` → `true`; any other present value → `false`; absent → `None` |
| `attr_num<T: FromStr>(n, name)` | `Option<T>` | absent **or unparseable** → `None` |
| `attr_list(n, name)` | `Vec<String>` | `split_ascii_whitespace`; absent → empty |
| `attr_float_pair(n, name)` | `Option<(f32, f32)>` | exactly two parseable floats, else `None` |

`attr_bool` matching `xml_bool`'s `"true"`/`"1"` acceptance is deliberate —
`xs:boolean` permits both lexical forms and real firmware sends both.

### 3.3 Correspondence tables

The **fixture** column is what the mock in `1d224f4` already emits; it is also
what the client-test fixture must contain (§3.6). `(omitted)` means the
attribute is deliberately absent and the test asserts `None` / empty.

#### 3.3.1 `tds:DeviceServiceCapabilities` → `DeviceServiceCapabilities`

Children: `Network` [1], `Security` [1], `System` [1], `Misc` [0..1].
Mock: `src/mock/services/device.rs:719`.

**`Network` → `DeviceNetworkCapabilities`**

| Schema attr | Type | Rust field | Fixture |
|-------------|------|------------|---------|
| `IPFilter` | bool | `ip_filter` | `false` |
| `ZeroConfiguration` | bool | `zero_configuration` | `false` |
| `IPVersion6` | bool | `ip_version6` | `true` |
| `DynDNS` | bool | `dyn_dns` | `false` |
| `Dot11Configuration` | bool | `dot11_configuration` | `false` |
| `Dot1XConfigurations` | int | `dot1x_configurations` | `(omitted)` |
| `HostnameFromDHCP` | bool | `hostname_from_dhcp` | `false` |
| `NTP` | int | `ntp` | `1` |
| `DHCPv6` | bool | `dhcpv6` | `false` |

**`Security` → `DeviceSecurityCapabilities`**

| Schema attr | Type | Rust field | Fixture |
|-------------|------|------------|---------|
| `TLS1.0` | bool | `tls1_0` | `false` |
| `TLS1.1` | bool | `tls1_1` | `false` |
| `TLS1.2` | bool | `tls1_2` | `true` |
| `OnboardKeyGeneration` | bool | `onboard_key_generation` | `false` |
| `AccessPolicyConfig` | bool | `access_policy_config` | `false` |
| `DefaultAccessPolicy` | bool | `default_access_policy` | `false` |
| `Dot1X` | bool | `dot1x` | `false` |
| `RemoteUserHandling` | bool | `remote_user_handling` | `false` |
| `X.509Token` | bool | `x509_token` | `false` |
| `SAMLToken` | bool | `saml_token` | `false` |
| `KerberosToken` | bool | `kerberos_token` | `false` |
| `UsernameToken` | bool | `username_token` | `true` |
| `HttpDigest` | bool | `http_digest` | `true` |
| `RELToken` | bool | `rel_token` | `false` |
| `JsonWebToken` | bool | `json_web_token` | `(omitted)` |
| `MaxUsers` | int | `max_users` | `8` |
| `MaxUserNameLength` | int | `max_user_name_length` | `32` |
| `MaxPasswordLength` | int | `max_password_length` | `64` |
| `MaxPasswordHistory` | int | `max_password_history` | `(omitted)` |
| `MaxUserRoles` | int | `max_user_roles` | `(omitted)` |
| `SupportedEAPMethods` | `tt:IntList` | `supported_eap_methods: Vec<u32>` | `(omitted)` |
| `SecurityPolicies` | `tt:StringList` | `security_policies: Vec<String>` | `(omitted)` |
| `HashingAlgorithms` | `tt:StringList` | `hashing_algorithms: Vec<String>` | `(omitted)` |

`SupportedEAPMethods` is the only `tt:IntList` in the stage — `Vec<u32>`, same
whitespace split, unparseable entries dropped.

**`System` → `DeviceSystemCapabilities`**

| Schema attr | Type | Rust field | Fixture |
|-------------|------|------------|---------|
| `DiscoveryResolve` | bool | `discovery_resolve` | `false` |
| `DiscoveryBye` | bool | `discovery_bye` | `true` |
| `RemoteDiscovery` | bool | `remote_discovery` | `false` |
| `SystemBackup` | bool | `system_backup` | `false` |
| `SystemLogging` | bool | `system_logging` | `true` |
| `CloudFirmwareUpgrade` | bool | `cloud_firmware_upgrade` | `(omitted)` |
| `HttpFirmwareUpgrade` | bool | `http_firmware_upgrade` | `true` |
| `HttpSystemBackup` | bool | `http_system_backup` | `false` |
| `HttpSystemLogging` | bool | `http_system_logging` | `false` |
| `HttpSupportInformation` | bool | `http_support_information` | `false` |
| `StorageConfiguration` | bool | `storage_configuration` | `true` |
| `MaxStorageConfigurations` | int | `max_storage_configurations` | `2` |
| `GeoLocationEntries` | int | `geo_location_entries` | `(omitted)` |
| `AutoGeo` | `StringAttrList` | `auto_geo: Vec<String>` | `(omitted)` |
| `StorageTypesSupported` | `StringAttrList` | `storage_types_supported: Vec<String>` | `(omitted)` |
| `Addons` | `StringAttrList` | `addons: Vec<String>` | `(omitted)` |
| `StorageConfigurationRenewal` | bool | `storage_configuration_renewal` | `(omitted)` |
| `HardwareType` | string | `hardware_type: Option<String>` | `(omitted)` |
| `DiscoveryNotSupported` | bool | `discovery_not_supported` | `(omitted)` |
| `NetworkConfigNotSupported` | bool | `network_config_not_supported` | `(omitted)` |
| `UserConfigNotSupported` | bool | `user_config_not_supported` | `false` |

The three negative-sense flags (§2.5) are the last three rows. The mock sends
**one** of them and omits the other two on purpose: the test then pins
`Some(false)` next to `None` in the same struct, which is the cheapest possible
proof that the parser is not defaulting.

**`Misc` → `DeviceMiscCapabilities`** (element itself `[0..1]` → `Option<…>`)

| Schema attr | Type | Rust field | Fixture |
|-------------|------|------------|---------|
| `AuxiliaryCommands` | `StringAttrList` | `auxiliary_commands: Vec<String>` | `tt:Wiper\|On tt:Wiper\|Off tt:IRLamp\|On tt:IRLamp\|Off tt:IRLamp\|Auto` |

That fixture value is **5 elements**, and it is the discoverable list behind
Stage C. It contains `|` but no XML metacharacter, so it survives the round trip
unescaped — a useful accident to assert rather than to rely on silently.

#### 3.3.2 `trt:Capabilities` → `MediaServiceCapabilities`

Mock: `src/mock/services/media.rs:858`.

| Schema attr | Type | Rust field | Fixture |
|-------------|------|------------|---------|
| `SnapshotUri` | bool | `snapshot_uri` | `true` |
| `Rotation` | bool | `rotation` | `false` |
| `VideoSourceMode` | bool | `video_source_mode` | `false` |
| `OSD` | bool | `osd` | `true` |
| `TemporaryOSDText` | bool | `temporary_osd_text` | `false` |
| `EXICompression` | bool | `exi_compression` | `false` |

Children, **both required [1]** — the only service-capability type with required
children, so a missing child is a `SoapError::missing`, not a `None`:

| Child | Rust field | Attr | Rust | Fixture |
|-------|-----------|------|------|---------|
| `ProfileCapabilities` | `profile` | `MaximumNumberOfProfiles` | `maximum_number_of_profiles: Option<u32>` | `8` |
| `StreamingCapabilities` | `streaming` | `RTPMulticast` | `rtp_multicast` | `false` |
| | | `RTP_TCP` | `rtp_tcp` | `true` |
| | | `RTP_RTSP_TCP` | `rtp_rtsp_tcp` | `true` |
| | | `NonAggregateControl` | `non_aggregate_control` | `false` |
| | | `NoRTSPStreaming` | `no_rtsp_streaming` | `false` |

Missing-field paths: `Capabilities/ProfileCapabilities` and
`Capabilities/StreamingCapabilities` — these are the two negative tests for
Media1 that are not fault tests.

#### 3.3.3 `tr2:Capabilities2` → `Media2ServiceCapabilities`

Mock: `src/mock/services/media2.rs:352`. Response element is still
`Capabilities`; only the *type* name carries the `2` (§2.7).

| Schema attr | Type | Rust field | Fixture |
|-------------|------|------------|---------|
| `SnapshotUri` | bool | `snapshot_uri` | `true` |
| `Rotation` | bool | `rotation` | `false` |
| `VideoSourceMode` | bool | `video_source_mode` | **`true`** |
| `OSD` | bool | `osd` | `false` |
| `TemporaryOSDText` | bool | `temporary_osd_text` | `(omitted)` |
| `Mask` | bool | `mask` | `false` |
| `SourceMask` | bool | `source_mask` | `false` |
| `WebRTC` | **int** | `webrtc: Option<u32>` | `0` |
| `WebRTC_codecs` | `StringList` | `webrtc_codecs: Vec<String>` | `(omitted)` |

> `VideoSourceMode` is `true` here and `false` in Media1 (§3.3.2) **on purpose**.
> The two services share six attribute names; a copy-paste that wires Media2's
> parser to Media1's response, or one struct shared between them, produces a
> test that still passes on every other field. This single disagreement is the
> tripwire. Do not "harmonise" the fixtures.

Children:

| Child | Card. | Rust field | Attr | Rust | Fixture |
|-------|:-----:|-----------|------|------|---------|
| `ProfileCapabilities` | [1] | `profile` | `MaximumNumberOfProfiles` | `Option<u32>` | `8` |
| | | | `ConfigurationsSupported` | `Vec<String>` | 5 items¹ |
| `StreamingCapabilities` | [1] | `streaming` | `RTSPStreaming` | `Option<bool>` | `true` |
| | | | `RTPMulticast` | `Option<bool>` | `false` |
| | | | `RTP_RTSP_TCP` | `Option<bool>` | `true` |
| | | | `NonAggregateControl` | `Option<bool>` | `false` |
| | | | `AutoStartMulticast` | `Option<bool>` | `false` |
| | | | `SecureRTSPStreaming` | `Option<bool>` | `(omitted)` |
| | | | `RTSPWebSocketUri` | `Option<String>` | `(omitted)` |
| `AudioClipCapabilities` | [0..1] | `audio_clip: Option<…>` | — | — | `(omitted)` |
| `MulticastAudioDecoderCapabilities` | [0..1] | `multicast_audio_decoder: Option<…>` | — | — | `(omitted)` |

¹ `VideoSource VideoEncoder AudioSource AudioEncoder Metadata`

**Decision on the two `[0..1]` children:** model them as
`Option<()>`-equivalent presence markers — i.e. a unit-ish struct with no
fields — **or leave them out of the struct entirely** and note it in the module
docs. Recommend **leaving them out for 0.15**: their contents belong to the
audio-clip and multicast-audio-decoder operations, none of which oxvif
implements, so a field would carry a type whose members are unverified. Revisit
when audio clips are built (the 0.16 candidate). Record this as a deliberate
omission in the CHANGELOG, not as an oversight.

#### 3.3.4 `tptz:Capabilities` → `PtzServiceCapabilities`

Mock: `src/mock/services/ptz.rs:348`.

| Schema attr | Type | Rust field | Fixture |
|-------------|------|------------|---------|
| `EFlip` | bool | `eflip` | `(omitted)` |
| `Reverse` | bool | `reverse` | `(omitted)` |
| `GetCompatibleConfigurations` | bool | `get_compatible_configurations` | `true` |
| `MoveStatus` | bool | `move_status` | `true` |
| `StatusPosition` | bool | `status_position` | `true` |
| `MoveAndTrack` | `StringList` | `move_and_track: Vec<String>` | `PresetToken PTZVector` (2 items) |

`MoveAndTrack` values come from `tt:MoveAndTrackMethod` (`PresetToken`,
`GeoLocation`, `PTZVector`, `ObjectID`). Keep them as `String` — an enum would
be a guess at a set the schema may extend, and neither `GeoMove` nor
`MoveAndStartTracking` is in scope (parent plan §8.3).

`EFlip` / `Reverse` omitted is the `None` assertion for this service, and the
richest one in the stage: `move_status: Some(true)` sits next to
`eflip: None` in the same struct.

#### 3.3.5 `timg:Capabilities` → `ImagingServiceCapabilities`

Mock: `src/mock/services/imaging.rs:163`. Complete set is three attributes.

| Schema attr | Type | Rust field | Fixture |
|-------------|------|------------|---------|
| `ImageStabilization` | bool | `image_stabilization` | `false` |
| `Presets` | bool | `presets` | `false` |
| `AdaptablePreset` | bool | `adaptable_preset` | `false` |

All three `Some(false)`. This is the one service with no `(omitted)` row, so its
negative test carries the whole weight — see §3.6 for the extra spelling test.

#### 3.3.6 `tev:Capabilities` → `EventsServiceCapabilities`

Mock: `src/mock/services/events.rs:286`.

| Schema attr | Type | Rust field | Fixture |
|-------------|------|------------|---------|
| `WSSubscriptionPolicySupport` | bool | `ws_subscription_policy_support` | `true` |
| `WSPausableSubscriptionManagerInterfaceSupport` | bool | `ws_pausable_subscription_manager_interface_support` | `false` |
| `MaxNotificationProducers` | int | `max_notification_producers` | `4` |
| `MaxPullPoints` | int | `max_pull_points` | `4` |
| `PersistentNotificationStorage` | bool | `persistent_notification_storage` | `false` |
| `EventBrokerProtocols` | string | `event_broker_protocols: Option<String>` | `(omitted)` |
| `MaxEventBrokers` | int | `max_event_brokers` | `0` |
| `MetadataOverMQTT` | bool | `metadata_over_mqtt` | `false` |

`EventBrokerProtocols` is `xs:string` in the schema even though its content is
space-separated (`mqtt mqtts`). Keep it `Option<String>` — matching the schema
type — rather than splitting it as if it were a `StringList`. It is the one
place in the stage where the *content* looks like a list and the *type* is not.

`MaxEventBrokers` fixture is `0`, i.e. `Some(0)`, not `None`. A parser that maps
falsy to absent fails here, which is the point.

Events is also the odd one out in dispatch: its action URI carries a portType
segment **and** a `Request` suffix —
`…/events/wsdl/EventPortType/GetServiceCapabilitiesRequest`. The client method's
`ACTION` constant must match, and this is already pinned by the `CAPS` table in
`src/mock/dispatch.rs`.

#### 3.3.7 `trc:Capabilities` → `RecordingServiceCapabilities`

Mock: `src/mock/services/recording.rs:157`. Widest of the nine — 21 attributes.

| Schema attr | Type | Rust field | Fixture |
|-------------|------|------------|---------|
| `DynamicRecordings` | bool | `dynamic_recordings` | `true` |
| `DynamicTracks` | bool | `dynamic_tracks` | `true` |
| `Encoding` | `StringList` | `encoding: Vec<String>` | `H264 AAC` (2 items) |
| `MaxRate` | **float** | `max_rate: Option<f32>` | `4096` |
| `MaxTotalRate` | **float** | `max_total_rate: Option<f32>` | `8192` |
| `MaxRecordings` | **float** | `max_recordings: Option<f32>` | `2` |
| `MaxRecordingJobs` | int | `max_recording_jobs: Option<u32>` | `2` |
| `Options` | bool | `options` | `false` |
| `MetadataRecording` | bool | `metadata_recording` | `false` |
| `SupportedExportFileFormats` | `StringAttrList` | `supported_export_file_formats: Vec<String>` | `(omitted)` |
| `EventRecording` | bool | `event_recording` | `false` |
| `BeforeEventLimit` | duration | `before_event_limit: Option<String>` | `(omitted)` |
| `AfterEventLimit` | duration | `after_event_limit: Option<String>` | `(omitted)` |
| `SupportedTargetFormats` | `StringAttrList` | `supported_target_formats: Vec<String>` | `(omitted)` |
| `EncryptionEntryLimit` | int | `encryption_entry_limit` | `(omitted)` |
| `SupportedEncryptionModes` | `StringAttrList` | `supported_encryption_modes: Vec<String>` | `(omitted)` |
| `OverrideSegmentDuration` | bool | `override_segment_duration` | `(omitted)` |
| `AsymmetricEncryptionSupported` | bool | `asymmetric_encryption_supported` | `(omitted)` |
| `ScheduledRecording` | bool | `scheduled_recording` | `false` |
| `OnboardStorage` | bool | `onboard_storage` | `(omitted)` — see §2.6 |
| `SegmentExport` | bool | `segment_export` | `false` |

`MaxRecordings` fixture `2` parses as `2.0f32`. Assert `Some(2.0)` — if it were
typed `u32` the fixture would still pass, so the assertion that proves the type
is a fixture value with a fractional part. **Change the mock to
`MaxRecordings="2.5"`** as part of Stage A so the `f32` is actually pinned, and
perturb it to confirm. This is the one place this document asks for a mock edit.

#### 3.3.8 `tse:Capabilities` → `SearchServiceCapabilities`

Mock: `src/mock/services/recording.rs:178`.

| Schema attr | Type | Rust field | Fixture |
|-------------|------|------------|---------|
| `MetadataSearch` | bool | `metadata_search` | `false` |
| `GeneralStartEvents` | bool | `general_start_events` | `false` |
| `NLSearch` | bool | `nl_search` | `false` |
| `ImageSearch` | bool | `image_search` | `false` |

#### 3.3.9 `trp:Capabilities` → `ReplayServiceCapabilities`

Mock: `src/mock/services/recording.rs:194`.

| Schema attr | Type | Rust field | Fixture |
|-------------|------|------------|---------|
| `ReversePlayback` | bool | `reverse_playback` | `false` |
| `SessionTimeoutRange` | `FloatList` | `session_timeout_range: Option<(f32, f32)>` | `1.0 600.0` |
| `RTP_RTSP_TCP` | bool | `rtp_rtsp_tcp` | `true` |
| `RTSPWebSocketUri` | `anyURI` | `rtsp_web_socket_uri: Option<String>` | `(omitted)` |

`session_timeout_range` asserts `Some((1.0, 600.0))` — the only tuple assertion
in the stage, and the only one that catches a parser reading the attribute as a
single float.

### 3.4 Client method map

All nine take **no parameters beyond the service URL** and send an empty body.

| Method | File | Action URI | Response tag | Returns |
|--------|------|-----------|--------------|---------|
| `device_get_service_capabilities` | `client/device.rs` | `…/ver10/device/wsdl/GetServiceCapabilities` | `GetServiceCapabilitiesResponse` | `DeviceServiceCapabilities` |
| `media_get_service_capabilities` | `client/media.rs` | `…/ver10/media/wsdl/GetServiceCapabilities` | ↑ | `MediaServiceCapabilities` |
| `media2_get_service_capabilities` | `client/media2.rs` | `…/ver20/media/wsdl/GetServiceCapabilities` | ↑ | `Media2ServiceCapabilities` |
| `ptz_get_service_capabilities` | `client/ptz.rs` | `…/ver20/ptz/wsdl/GetServiceCapabilities` | ↑ | `PtzServiceCapabilities` |
| `imaging_get_service_capabilities` | `client/imaging.rs` | `…/ver20/imaging/wsdl/GetServiceCapabilities` | ↑ | `ImagingServiceCapabilities` |
| `events_get_service_capabilities` | `client/events.rs` | `…/ver10/events/wsdl/EventPortType/GetServiceCapabilitiesRequest` | ↑ | `EventsServiceCapabilities` |
| `recording_get_service_capabilities` | `client/recording.rs` | `…/ver10/recording/wsdl/GetServiceCapabilities` | ↑ | `RecordingServiceCapabilities` |
| `search_get_service_capabilities` | `client/recording.rs` | `…/ver10/search/wsdl/GetServiceCapabilities` | ↑ | `SearchServiceCapabilities` |
| `replay_get_service_capabilities` | `client/recording.rs` | `…/ver10/replay/wsdl/GetServiceCapabilities` | ↑ | `ReplayServiceCapabilities` |

Verify each URI against the `CAPS` table in `src/mock/dispatch.rs` before
writing the method — that table is already the tested source of truth, and the
events row is the one that will be got wrong from memory.

Request bodies are the service prefix + `GetServiceCapabilities`, self-closing:
`<tptz:GetServiceCapabilities/>`, `<tds:GetServiceCapabilities/>`, etc.
Precedent: `src/client/ptz.rs:295`.

Body shape of every method (imaging shown):

```rust
const ACTION: &str = "http://www.onvif.org/ver20/imaging/wsdl/GetServiceCapabilities";
const BODY: &str = "<timg:GetServiceCapabilities/>";
let xml = self.call(imaging_url, ACTION, BODY).await?;
let body_node = parse_soap_body(&xml)?;
let resp = find_response(&body_node, "GetServiceCapabilitiesResponse")?;
ImagingServiceCapabilities::from_xml(resp)
```

`from_xml` takes the **response node** (not the `Capabilities` child) and does
its own `.child("Capabilities").ok_or_else(|| SoapError::missing("Capabilities"))?`,
matching `Capabilities::from_xml` at `src/types/capabilities.rs:249`.

**No `xml_escape` needed anywhere in Stage A** — no method takes a string
parameter. That changes in Stages B and C.

### 3.5 Session wrapper map

| Wrapper | URL getter | Notes |
|---------|-----------|-------|
| `device_get_service_capabilities` | — | device URL is the client's own base |
| `media_get_service_capabilities` | `media_url()?` | |
| `media2_get_service_capabilities` | `media2_url()?` | |
| `ptz_get_service_capabilities` | `ptz_url()?` | |
| `imaging_get_service_capabilities` | `imaging_url()?` | |
| `events_get_service_capabilities` | `events_url()?` | |
| `recording_get_service_capabilities` | `recording_url()?` | |
| `search_get_service_capabilities` | `search_url()?` | |
| `replay_get_service_capabilities` | `replay_url()?` | |

Pattern at `src/session.rs:1202`. **No batched
`session.service_capabilities()`** — parent plan §8.1 defers that until the
health integration shows the real access pattern.

### 3.6 Test matrix

18 tests minimum: one positive + one negative per service. Fault codes and
reasons are **pre-allocated here** so that `code` and `reason` vary
independently across the suite (CLAUDE.md), which cannot be achieved by picking
them one test at a time. None collides with the 20 pairs already in the tree.

| Service | Positive test | Negative test | Fault code | Fault reason |
|---------|--------------|---------------|-----------|--------------|
| Device | `device_service_capabilities_parses_all_four_children` | `device_service_capabilities_fault` | `env:Sender` | `ActionNotSupported-tds-caps-6104` |
| Media1 | `media_service_capabilities_parses_required_children` | `media_service_capabilities_missing_streaming` | *(missing field)* | path `Capabilities/StreamingCapabilities` |
| Media2 | `media2_service_capabilities_parses_webrtc_count` | `media2_service_capabilities_fault` | `ter:ActionNotSupported` | `NoServiceCapabilities-tr2-2270` |
| PTZ | `ptz_service_capabilities_parses_move_and_track` | `ptz_service_capabilities_fault` | `env:Receiver` | `PtzCapsUnavailable-9318` |
| Imaging | `imaging_service_capabilities_parses_three_flags` | `imaging_service_capabilities_fault` | `ter:NotAuthorized` | `ImagingCapsDenied-5527` |
| Events | `events_service_capabilities_parses_counts` | `events_service_capabilities_fault` | `s:Receiver` | `EventCapsInternal-8043` |
| Recording | `recording_service_capabilities_parses_wide_attribute_set` | `recording_service_capabilities_fault` | `ter:InvalidArgVal` | `RecCapsBadRequest-3391` |
| Search | `search_service_capabilities_parses_four_flags` | `search_service_capabilities_fault` | `env:Sender` | `SearchCapsUnsupported-7712` |
| Replay | `replay_service_capabilities_parses_timeout_range` | `replay_service_capabilities_missing_capabilities` | *(missing field)* | path `Capabilities` |

Seven distinct codes across nine rows; nine distinct reasons. Two negatives are
missing-field rather than fault, which is what makes the `Capabilities`
`ok_or_else` path load-bearing at all.

**Assertions each positive must carry**, beyond a happy field:

| Service | The assertion that is doing the work |
|---------|-------------------------------------|
| Device | `user_config_not_supported == Some(false)` **and** `discovery_not_supported == None` in one test |
| Device | `misc.auxiliary_commands.len() == 5` and `[0] == "tt:Wiper\|On"` |
| Device | `tls1_2 == Some(true)` and `x509_token == Some(false)` — pins the dotted lookup strings |
| Media1 | `streaming.rtp_tcp == Some(true)` — the field Media2 does not have |
| Media2 | `webrtc == Some(0)` — pins `xs:int`, not bool |
| Media2 | `video_source_mode == Some(true)` — the Media1/Media2 tripwire (§3.3.3) |
| Media2 | `temporary_osd_text == None` while Media1's is `Some(false)` |
| PTZ | `move_and_track == ["PresetToken", "PTZVector"]` and `eflip == None` |
| Imaging | `adaptable_preset == Some(false)` — pins the verified spelling (§2.7) |
| Events | `max_event_brokers == Some(0)` — pins that `0` is not `None` |
| Recording | `max_recordings == Some(2.5)` — pins `f32` (requires the mock edit in §3.3.7) |
| Recording | `onboard_storage == None` — pins §2.6 |
| Replay | `session_timeout_range == Some((1.0, 600.0))` |

Client-test fixtures must be **byte-identical to the mock responder bodies**.
They cannot literally reuse them: `src/mock/` is feature-gated (§1) and the
client tests compile without it. Copy the XML and add a comment naming the
source function, so a future divergence is at least traceable:

```rust
// Byte-identical to `crate::mock::services::ptz::resp_ptz_service_capabilities`
// (src/mock/services/ptz.rs:348). Feature-gated there, so it cannot be reused
// directly — keep the two in sync by hand.
```

Also add **one integration test** in `tests/` driving the real `MockServer`
through the client for a single service (PTZ), so the copy and the original are
proven equal at least once rather than only by convention. It runs under
`--all-features` only, which §1 has already made the standing gate.

---

## 4. Stage B — PTZ preset tours (7 ops)

### 4.1 Type map

Into `src/types/ptz.rs`, following `PtzPreset` at `src/types/ptz.rs:23`.

| Schema type | Rust | Notes |
|-------------|------|-------|
| `tt:PresetTour` | `PtzPresetTour` | |
| `tt:PTZPresetTourStatus` | `PtzPresetTourStatus` | |
| `tt:PTZPresetTourState` | `PtzPresetTourState` (enum) | `Idle\|Touring\|Paused\|Extended` |
| `tt:PTZPresetTourStartingCondition` | `PtzPresetTourStartingCondition` | |
| `tt:PTZPresetTourDirection` | `PtzPresetTourDirection` (enum) | `Forward\|Backward\|Extended` |
| `tt:PTZPresetTourSpot` | `PtzPresetTourSpot` | |
| `tt:PTZPresetTourPresetDetail` | `PtzPresetTourPresetDetail` (**enum**) | `xs:choice` — see §4.3 |
| `tt:PTZPresetTourOptions` | `PtzPresetTourOptions` | |

`tt:PresetTour` field map:

| Member | Card. | Rust field | On absent |
|--------|:-----:|-----------|-----------|
| `@token` | **[0..1]** | `token: Option<String>` | `None` — **not** an error |
| `Name` | [0..1] | `name: Option<String>` | `None` |
| `Status` | [1] | `status: PtzPresetTourStatus` | `SoapError::missing("PresetTour/Status")` |
| `AutoStart` | [1] | `auto_start: bool` | `SoapError::missing("PresetTour/AutoStart")` |
| `StartingCondition` | [1] | `starting_condition: …` | `SoapError::missing("PresetTour/StartingCondition")` |
| `TourSpot` | [0..*] | `tour_spots: Vec<PtzPresetTourSpot>` | empty |
| `Extension` | [0..1] | *(not modelled)* | — |

> **`@token` is `[0..1]` here**, unlike `tt:PTZPreset/@token`. The CLAUDE.md
> "required fields must return `Result`" rule keys off *schema* cardinality, so
> this one is genuinely `Option<String>`. Do not reflexively
> `ok_or_else(missing("PresetTour/@token"))` it — that would reject a
> schema-valid response. The three fields that **must** hard-fail are `Status`,
> `AutoStart` and `StartingCondition`.

`StartingCondition`: `@RandomPresetOrder` `Option<bool>` (attribute),
`RecurringTime` `Option<u32>`, `RecurringDuration` `Option<String>` (ISO 8601,
per §2.3), `Direction` `Option<PtzPresetTourDirection>`.

`TourSpot`: `preset_detail` [1] (hard-fail), `speed` [0..1], `stay_time`
`Option<String>` [0..1].

`PtzPresetTourOptions`: `auto_start: bool` [1], `starting_condition` [1],
`tour_spot` [1] — all three required, all three hard-fail when absent.

> **Cardinality asymmetry, easy to get wrong:** `Direction` is a *single* value
> in `StartingCondition` and a *repeated* list `[0..*]` in
> `StartingConditionOptions`. Same element name, different Rust type
> (`Option<Direction>` vs `Vec<Direction>`).

### 4.2 Enum parsing

Both enums are ONVIF `xs:string` restrictions with an `Extended` member, so
they are open in practice. Model each as a Rust enum with an
`Extended`/`Unknown(String)` catch-all rather than failing on an unrecognised
value — a vendor string must not turn `GetPresetTours` into an `Err`. Decide
the exact catch-all shape when writing, but the rule is: **unknown value must
not be an error**.

### 4.3 `PresetDetail` is a choice, not a struct

```rust
pub enum PtzPresetTourPresetDetail {
    PresetToken(String),
    Home,
    Position(PtzVector),
    // TypeExtension deliberately unmodelled
}
```

`xs:choice` — exactly one. Serialising more than one variant produces a
schema-invalid `ModifyPresetTour`. This is the single most likely place to get
Stage B wrong, and a Rust enum is what makes the invalid state unrepresentable.
Parse order: `PresetToken`, then `Home`, then `PTZPosition`; if none is present
→ `SoapError::missing("PTZPresetTourPresetDetail")`.

### 4.4 Operation map

| Operation | Method | Request fields | Response |
|-----------|--------|---------------|----------|
| `GetPresetTours` | `ptz_get_preset_tours` | `ProfileToken` | `Vec<PtzPresetTour>` |
| `GetPresetTour` | `ptz_get_preset_tour` | `ProfileToken`, `PresetTourToken` | `PtzPresetTour` |
| `GetPresetTourOptions` | `ptz_get_preset_tour_options` | `ProfileToken`, `PresetTourToken` [0..1] | `PtzPresetTourOptions` |
| `CreatePresetTour` | `ptz_create_preset_tour` | `ProfileToken` | `String` (token) |
| `ModifyPresetTour` | `ptz_modify_preset_tour` | `ProfileToken`, `PresetTour` | `()` |
| `OperatePresetTour` | `ptz_operate_preset_tour` | `ProfileToken`, `PresetTourToken`, `Operation` | `()` |
| `RemovePresetTour` | `ptz_remove_preset_tour` | `ProfileToken`, `PresetTourToken` | `()` |

Action URIs: `http://www.onvif.org/ver20/ptz/wsdl/<Operation>`.

`Operation` is `Start|Stop|Pause|Extended` — take it as a typed enum on the way
*in* (it is a closed set the client chooses from, unlike the response enums it
does not have to survive vendor strings).

**Escaping:** every `&str` parameter goes through `xml_escape` — profile
tokens, tour tokens, and **every string field inside `PtzPresetTour::to_xml_body()`**
(`Name` above all — it is user-supplied). `ModifyPresetTour` is the only
operation in Tier 1 that writes structured user data, so it is the only place
this rule has real teeth.

### 4.5 Mock state

Unlike Stage A, tours need real state on `DeviceState` (`src/mock/state.rs`) —
a tour created by `CreatePresetTour` must come back from a later
`GetPresetTours`, or the mock is not an integration harness for the feature.
`OperatePresetTour` moves a status field, mirroring the existing
`SetRecordingJobMode` handler.

### 4.6 The one test Stage B cannot ship without

A `GetPresetTours` fixture with **one valid tour and one tour missing
`Status`**, asserting the error is `PresetTour/Status` — proving the
`vec_from_xml` closure propagates rather than returning the first tour and
dropping the second. That is the specific bug the CLAUDE.md `vec_from_xml` rule
exists to prevent, and only a two-element fixture catches it.

Use `Status` (not `@token`) as the omitted field, precisely because `@token` is
optional here (§4.1) and omitting it is *valid*.

---

## 5. Stage C — PTZ `SendAuxiliaryCommand` (1 op)

| | |
|---|---|
| Method | `ptz_send_auxiliary_command` |
| Action | `http://www.onvif.org/ver20/ptz/wsdl/SendAuxiliaryCommand` |
| Request | `ProfileToken` [1], `AuxiliaryData` [1] |
| Response | `AuxiliaryResponse` `tt:AuxiliaryData` [1] → `String` |

> **Name collision with an existing method.** `OnvifClient::send_auxiliary_command`
> already exists and is the **Device** operation (`src/session.rs:407`). The PTZ
> one is a different operation, on a different endpoint, and **returns a
> payload** where the Device one returns nothing. Cameras implement the wiper on
> the PTZ one. Keep both; the `ptz_` prefix is what distinguishes them, and both
> doc comments must name the other.

`tt:AuxiliaryData` is `xs:string` with `maxLength` 128. Keep the API `&str` in /
`String` out. **Do not invent an enum of commands** — the values are
vendor-namespaced (`tt:Wiper|On`, `tt:IRLamp|Auto`) and an enum would need a
breaking change every time a camera used a value not enumerated. Document
common values in the method doc comment instead.

`xml_escape` the command on the way in. The discoverable list of what a given
camera accepts is `DeviceServiceCapabilities.misc.auxiliary_commands` from
Stage A (§3.3.1) — **cross-reference the two doc comments**, since that link is
the only reason a caller can use this method without guessing.

`maxLength` 128 is **not** enforced client-side: the device rejects an
over-long value with a fault, and a client-side length check would be a second
source of truth that drifts. Note the limit in the doc comment.

---

## 6. Release deltas (SOP steps 6, 6a, 7)

Headers that go stale silently — the failure mode that got ROADMAP.md deleted.
Every one of these must move in the release commit:

| File | Current | After Tier 1 |
|------|---------|--------------|
| `docs/reference/ptz.md:8` | 18 / 29 | **26 / 29** |
| `docs/reference/media1.md:9` | ~31 of ~78 | ~32 |
| `docs/reference/media2.md:8` | ~26 of ~59 | ~27 |
| `docs/reference/events.md:9` | ~8 of ~16 | ~9 |
| `docs/reference/{imaging,recording,search,replay,device}.md` | — | +1 each |
| `README.md` | test count, op tables, install version | all three |
| `src/lib.rs` `//!` header | Profile coverage table, PTZ prose | both |
| `examples/camera.rs` | — | new command + `full_workflow()` section |
| `Cargo.toml` | 0.14.0 | 0.15.0 |
| `CHANGELOG.md` | — | new top entry |

Also flip the `—` to `✓` for `GetServiceCapabilities` in **nine** reference
tables and for the seven tour operations in `ptz.md`.

Move this plan and its parent to `docs/done/` at release, and update the tables
in `docs/README.md`.

---

## 7. Perturbation protocol

CLAUDE.md requires proving each assertion is load-bearing. For a batch this
size, per-test perturbation is impractical; the batch forms below are the
cheapest correct substitutes. **Run every one unfiltered and with
`--all-features`** (§1) — a `cargo test <filter>` run silently excludes the
integration crates, and a no-feature run silently excludes the mock.

| # | Mutation | Expected |
|---|----------|----------|
| 1 | `SoapError::missing()` ignores its argument | every missing-field negative reds; fault negatives stay green |
| 2 | fault parser in `src/soap/xml.rs` returns constant `code`/`reason` | every fault negative reds |
| 3 | `attr_bool` returns `Some(false)` instead of `None` when absent | every `(omitted)` assertion reds — this is the one that catches §2.1 |
| 4 | `attr_list` returns empty unconditionally | PTZ `move_and_track`, device `auxiliary_commands`, recording `encoding` red |
| 5 | Media2's parser wired to Media1's struct | `video_source_mode` red (§3.3.3 tripwire) |

Anything that stays green under all five is hollow and must be rewritten before
commit.

Perturbation 3 is the important one and it has no substitute: it is the only
check that the `Option<bool>` decision — the reason Stage A exists at all — is
actually observable in the tests rather than merely written in the types.

---

## 8. Commit sequence

Per CLAUDE.md, one commit per completed piece, gate (as corrected in §1) green
before each.

1. `docs(plan): Tier 1 implementation map` — this file.
2. `feat(types): per-service capabilities for all nine services` — Stage A
   types + `mod.rs` + `lib.rs` re-exports + the `capabilities.rs` doc
   amendment + the `MaxRecordings="2.5"` mock edit (§3.3.7).
3. `feat(client): GetServiceCapabilities on all nine services` — 9 methods +
   9 session wrappers + 18 tests + the one integration test.
4. `feat(ptz): preset tours` — Stage B, types + client + mock state + tests.
5. `feat(ptz): SendAuxiliaryCommand` — Stage C.
6. `chore(release): 0.15.0` — §6.

Splitting 2 from 3 keeps the type decisions reviewable on their own; they are
where every silent-failure trap in this document lives.
