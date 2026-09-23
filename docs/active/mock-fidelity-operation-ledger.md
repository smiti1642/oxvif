# Mock fidelity operation ledger

[English](mock-fidelity-operation-ledger.md) | [繁體中文](mock-fidelity-operation-ledger_zh.md)

## Current calibration (2026-09-22)

Source baseline: `80bcf14`. Use this section as the current work entry; older dated statuses/counts below remain historical evidence. Historical evidence retains its recorded revision; fresh calibration checks are summarized in the follow-up backlog. Scheduling belongs to the [follow-up backlog](post-0.17-backlog.md).

| Category | Disposition and evidence |
| --- | --- |
| Confirmed | Maintained inventory, not a standalone implementation backlog. The standing checker reconciles 159 routes, 161 Action sites and the reader index. The two B16 routes are implemented; their PARTIAL columns describe broader field/fault/effect audit coverage. |
| Remaining | Each row keeps its W owner and fidelity axes. W26 integration completion does not make actual synchronization effects modeled; broader Media work belongs to W10/W16/W17. Do not mark all PARTIAL rows DONE because the route exists. |
| Next step / exit criteria | Before modifying a route, open its W card; after the batch update the row and both language inventories, run [the checker](check-mock-fidelity-inventory.ps1), and link exact semantic tests. Inventory equality is a maintenance gate, not conformance acceptance. |

Initial source baseline: `b134f73`, 2026-09-10; rows include subsequent named batches.
This is a project-source inventory,
not an ONVIF schema catalogue or a claim of conformance. Including B16, it lists all **159**
literal route arms in the **10** production sub-dispatchers. These counts are
not the number of fully verified operations or all operations ONVIF defines.

Start with the [execution checklist](mock-fidelity-execution-checklist.md);
policy and historical evidence remain in the [main plan](mock-fidelity-hardening-plan.md).
Do not change a route before opening its row and the corresponding work package.

| Section | Purpose |
| --- | --- |
| [Current calibration](#current-calibration-2026-09-22) | Delivered scope, remaining work and next step |
| [Tracking contract](#tracking-contract) | Column meaning and completion evidence |
| [device](#device) | 38 route arms; W13 |
| [device_io](#device-io) | 1 route arms; W13 |
| [media](#media) | 33 route arms; W10/W26 |
| [media2](#media2) | 27 route arms; W10/W26 |
| [ptz](#ptz) | 27 route arms; W11 |
| [imaging](#imaging) | 8 route arms; W12 |
| [events](#events) | 8 route arms; W15 |
| [recording](#recording) | 11 route arms; W14 |
| [search](#search) | 4 route arms; W14 |
| [replay](#replay) | 2 route arms; W14 |
| [Maintenance](#maintenance) | Source drift and change protocol |

## Tracking contract

B16: [Media synchronization cards and evidence](../done/mock-fidelity-pr16-integration.md),
default refusal and explicitly selected receipts, not actual streaming.

K34 means the historical rate-only migration in the [VE1 plan](../done/mock-fidelity-video-encoder.md),
including dependent encoder/profile reads. VE1 names the later eight-operation
encoder delivery: complete candidates, selectors/options, codec views and capacity.
Cross-cutting programme acceptance remains separate; see the plan's dated evidence.

- ID is `dispatcher.operation`, not the last Action segment alone. Preserve
  Events' actual `Request` suffix; do not confuse it with Media synchronization.
- Handler and arguments are copied from the current dispatch expression.
  They identify the call site, **not** a fidelity classification. Passing
  `state` does not prove a write works; omitting `body` is a request-validation
  audit target, not evidence that the operation has no request fields.
- Work identifies the numbered work package in the execution checklist.
- C = operation contract review; R = request parsing; F = ordinary Fault mapping;
  B = behavior classification/state semantics; V = verification evidence.
- `TODO` means not accepted under this programme, not that existing tests do not
  exist. `PARTIAL` means only a named slice is proved. `DONE` requires an evidence
  record with commit and exact test names. `NA:<record>` requires an explicit
  justification; unsupported operations still need rejection tests.
- The two DeleteProfile rows deliberately remain partial: scalar identity and
  missing/fixed-profile Fault fixes do not settle full routing, all Fault paths
  or whole-operation acceptance. P1 records the selected nested Fault evidence.
  Evidence E1 is the main plan's first implementation slice at `b134f73`,
  `tests/mock_request_identity.rs` and `src/mock/request.rs` tests.
- Every row inherits **all** review axes C01–C12 and the readiness/closure
  gates in the execution checklist. Fill an operation card before implementation;
  no blank field may be interpreted as “not needed.” Normative field tables and
  schema-derived corpora stay external under D3. Link sanitized findings only.
- P1 refers to the [first 13 operation cards](mock-fidelity-profile-preflight.md).
  Their partial contract review and known-gap reproductions are not acceptance.
- Service implementation paths below are navigable. Locate the exact symbol
  shown in the Handler column; line numbers are intentionally not frozen.

A2: first receipt-only policy and effect boundaries; see [policy checkpoint](../done/mock-fidelity-ack-policy-preflight.md).

A3: policy migration for eight additional effect stubs; see [batch record](../done/mock-fidelity-ack-policy-preflight.md#remaining-effect-stub-batch).

PA1: profile assembly, capacity and reference-count subgroup; see [batch record](../done/mock-fidelity-profile-assembly.md). Full field/physical compatibility remains separate.

VS1: eight source read/write/options operations; see [batch evidence and limits](../done/mock-fidelity-video-source.md). Rows remain PARTIAL, not full schema/device-contract acceptance.

AM1 identifies the [15-operation audio/metadata subgroup](../done/mock-fidelity-audio-metadata.md).
The [0.17 cut](../done/release-0.17-cut.md) accepts selected R01–R08 contracts separately
from whole-programme C/R/F/B/V completion; no TODO/PARTIAL row is promoted by a
release or by matching inventory counts.

## device

[dispatch_device](../../src/mock/dispatch.rs) · [services/device.rs](../../src/mock/services/device.rs)

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `device.GetServiceCapabilities` | `device::resp_service_capabilities` | `` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetSystemDateAndTime` | `device::resp_system_date_and_time` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetSystemDateAndTime` | `device::handle_set_system_date_and_time` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetCapabilities` | `device::resp_capabilities` | `base` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetServices` | `device::resp_services` | `base` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetDeviceInformation` | `device::resp_device_info` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetHostname` | `device::resp_hostname` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetHostname` | `device::handle_set_hostname` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetNTP` | `device::resp_ntp` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetNTP` | `device::handle_set_ntp` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetDNS` | `device::resp_dns` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetDNS` | `device::handle_set_dns` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetScopes` | `device::resp_scopes` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetScopes` | `device::handle_set_scopes` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetUsers` | `device::resp_users` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.CreateUsers` | `device::handle_create_users` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.DeleteUsers` | `device::handle_delete_users` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetUser` | `device::handle_set_user` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetNetworkInterfaces` | `device::resp_network_interfaces` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetNetworkInterfaces` | `device::handle_set_network_interfaces` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetNetworkProtocols` | `device::resp_network_protocols` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetNetworkProtocols` | `device::handle_set_network_protocols` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetNetworkDefaultGateway` | `device::resp_network_default_gateway` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetNetworkDefaultGateway` | `device::handle_set_network_default_gateway` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SendAuxiliaryCommand` | `device::resp_send_auxiliary_command` | `` | W13 | TODO | PARTIAL | PARTIAL | PARTIAL | PARTIAL | A3 |
| `device.GetSystemLog` | `device::resp_system_log` | `` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetRelayOutputs` | `device::resp_relay_outputs` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetRelayOutputState` | `device::handle_set_relay_output_state` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetRelayOutputSettings` | `device::handle_set_relay_output_settings` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetSystemFactoryDefault` | `resp_empty` | `"tds", "SetSystemFactoryDefaultResponse"` | W13 | TODO | PARTIAL | PARTIAL | PARTIAL | PARTIAL | A2 |
| `device.GetStorageConfigurations` | `device::resp_storage_configurations` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetStorageConfiguration` | `device::handle_set_storage_configuration` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetSystemUris` | `device::resp_system_uris` | `base` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.StartFirmwareUpgrade` | `device::resp_start_firmware_upgrade` | `base` | W13 | TODO | PARTIAL | PARTIAL | PARTIAL | PARTIAL | A3 |
| `device.StartSystemRestore` | `device::resp_start_system_restore` | `base` | W13 | TODO | PARTIAL | PARTIAL | PARTIAL | PARTIAL | A3 |
| `device.GetDiscoveryMode` | `device::resp_discovery_mode` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetDiscoveryMode` | `device::handle_set_discovery_mode` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SystemReboot` | `device::resp_system_reboot` | `` | W13 | TODO | PARTIAL | PARTIAL | PARTIAL | PARTIAL | A3 |

## device-io

[dispatch_device_io](../../src/mock/dispatch.rs) · [services/device.rs](../../src/mock/services/device.rs)

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `device_io.GetDigitalInputs` | `device::resp_digital_inputs` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |

## media

P2 is the [paired profile identity migration](mock-fidelity-profile-preflight.md#media-profile-identity).
It does not close complete field/fault policy or every configuration token.
K30 subsequently supplies the bounded empty-profile-token policy; PA1 supplies
modeled assembly/capacity behavior. These slices remain partial operation acceptance.

[dispatch_media](../../src/mock/dispatch.rs) · [services/media.rs](../../src/mock/services/media.rs)

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `media.GetServiceCapabilities` | `media::resp_service_capabilities` | `` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetProfiles` | `media::resp_profiles` | `state` | W10 | PARTIAL | TODO | TODO | TODO | TODO | P1 |
| `media.GetProfile` | `media::resp_profile` | `state, operation` | W10 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | P1/P2 |
| `media.CreateProfile` | `media::handle_create_profile` | `state, operation, effect` | W10 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | P1/P2/PA1 |
| `media.DeleteProfile` | `media::handle_delete_profile` | `state, operation, effect` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | E1,P1/PA1 |
| `media.GetStreamUri` | `media::resp_stream_uri` | `` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetSnapshotUri` | `media::resp_snapshot_uri` | `base` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.SetSynchronizationPoint` | `media::handle_set_synchronization_point` | `state, operation, false` | W26 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | B16 |
| `media.GetVideoSources` | `media::resp_video_sources` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VS1 |
| `media.GetVideoSourceConfigurations` | `media::resp_video_source_configurations` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VS1 |
| `media.GetVideoSourceConfiguration` | `media::resp_video_source_configuration` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VS1 |
| `media.SetVideoSourceConfiguration` | `media::handle_set_video_source_configuration` | `state, operation, effect` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VS1 |
| `media.GetVideoSourceConfigurationOptions` | `media::resp_video_source_configuration_options` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VS1 |
| `media.GetVideoEncoderConfigurations` | `media::resp_video_encoder_configurations` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VE1 |
| `media.GetVideoEncoderConfiguration` | `media::resp_video_encoder_configuration` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VE1 |
| `media.SetVideoEncoderConfiguration` | `media::handle_set_video_encoder_configuration` | `state, operation, effect` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VE1 |
| `media.GetVideoEncoderConfigurationOptions` | `media::resp_video_encoder_configuration_options` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VE1 |
| `media.AddVideoEncoderConfiguration` | `media::handle_add_video_encoder_configuration` | `state, body, operation, effect` | W10 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | P1/P2/PA1 |
| `media.RemoveVideoEncoderConfiguration` | `media::handle_remove_video_encoder_configuration` | `state, operation, effect` | W10 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | P1/P2/PA1 |
| `media.AddVideoSourceConfiguration` | `media::handle_add_video_source_configuration` | `state, body, operation, effect` | W10 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | P1/P2/PA1 |
| `media.RemoveVideoSourceConfiguration` | `media::handle_remove_video_source_configuration` | `state, operation, effect` | W10 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | P1/P2/PA1 |
| `media.GetAudioSources` | `media::resp_audio_sources` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | AM1 |
| `media.GetAudioSourceConfigurations` | `media::resp_audio_source_configurations` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | AM1 |
| `media.GetAudioEncoderConfiguration` | `media::resp_audio_encoder_configuration` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | AM1 |
| `media.GetAudioEncoderConfigurations` | `media::resp_audio_encoder_configurations` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | AM1 |
| `media.SetAudioEncoderConfiguration` | `media::handle_set_audio_encoder_configuration` | `state, operation, effect` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | AM1 |
| `media.GetAudioEncoderConfigurationOptions` | `media::resp_audio_encoder_configuration_options` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | AM1 |
| `media.GetOSD` | `media::resp_osd` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | RS1 |
| `media.GetOSDs` | `osd::list` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | OS1 |
| `media.SetOSD` | `osd::set` | `state, operation, effect` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | OS1 |
| `media.CreateOSD` | `osd::create` | `state, operation, effect` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | OS1 |
| `media.DeleteOSD` | `osd::delete` | `state, operation, effect` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | OS1 |
| `media.GetOSDOptions` | `osd::options` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | OS1 |

## media2

[dispatch_media2](../../src/mock/dispatch.rs) · [services/media2.rs](../../src/mock/services/media2.rs)

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `media2.GetServiceCapabilities` | `media2::resp_service_capabilities_media2` | `` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetProfiles` | `media2::resp_profiles_media2` | `state, operation` | W10 | PARTIAL | TODO | TODO | TODO | TODO | P1 |
| `media2.CreateProfile` | `media2::handle_create_profile_media2` | `state, operation, effect` | W10 | PARTIAL | TODO | TODO | TODO | TODO | P1/PA1 |
| `media2.DeleteProfile` | `media2::handle_delete_profile_media2` | `state, operation, effect` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | E1,P1/PA1 |
| `media2.AddConfiguration` | `media2::handle_add_configuration_media2` | `state, body, operation, effect` | W10 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | P1/P2/PA1 |
| `media2.RemoveConfiguration` | `media2::handle_remove_configuration_media2` | `state, body, operation, effect` | W10 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | P1/P2/PA1 |
| `media2.GetStreamUri` | `media2::resp_stream_uri_media2` | `` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetSnapshotUri` | `media2::resp_snapshot_uri_media2` | `base` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.SetSynchronizationPoint` | `media::handle_set_synchronization_point` | `state, operation, true` | W26 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | B16 |
| `media2.GetVideoSourceConfigurations` | `media2::resp_video_source_configurations_media2` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VS1 |
| `media2.SetVideoSourceConfiguration` | `media2::handle_set_video_source_configuration_media2` | `state, operation, effect` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VS1 |
| `media2.GetVideoSourceConfigurationOptions` | `media2::resp_video_source_configuration_options_media2` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VS1 |
| `media2.GetVideoEncoderConfigurations` | `media2::resp_video_encoder_configurations` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VE1 |
| `media2.SetVideoEncoderConfiguration` | `media2::handle_set_video_encoder_configuration` | `state, operation, effect` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VE1 |
| `media2.GetVideoEncoderConfigurationOptions` | `media2::resp_video_encoder_configuration_options_media2` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VE1 |
| `media2.GetVideoEncoderInstances` | `media2::resp_video_encoder_instances` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | VE1 |
| `media2.GetMetadataConfigurations` | `media2::resp_metadata_configurations` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | AM1 |
| `media2.SetMetadataConfiguration` | `media2::handle_set_metadata_configuration` | `state, operation, effect` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | AM1 |
| `media2.GetMetadataConfigurationOptions` | `media2::resp_metadata_configuration_options` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | AM1 |
| `media2.GetAudioSourceConfigurations` | `media2::resp_audio_source_configurations_media2` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | AM1 |
| `media2.GetAudioEncoderConfigurations` | `media2::resp_audio_encoder_configurations_media2` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | AM1 |
| `media2.GetAudioEncoderConfigurationOptions` | `media2::resp_audio_encoder_configuration_options_media2` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | AM1 |
| `media2.SetAudioEncoderConfiguration` | `media2::handle_set_audio_encoder_configuration_media2` | `state, operation, effect` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | AM1 |
| `media2.GetAudioOutputConfigurations` | `media2::resp_audio_output_configurations` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | AM1 |
| `media2.GetAudioDecoderConfigurations` | `media2::resp_audio_decoder_configurations` | `state, operation` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | AM1 |
| `media2.GetVideoSourceModes` | `media2::resp_video_source_modes` | `` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.SetVideoSourceMode` | `media2::resp_set_video_source_mode` | `` | W10 | TODO | TODO | TODO | TODO | TODO | - |

## ptz

[dispatch_ptz](../../src/mock/dispatch.rs) · [services/ptz.rs](../../src/mock/services/ptz.rs)

PTZ1 refers to [scoped profile identity](mock-fidelity-profile-preflight.md#ptz-profile-identity),
not complete operation acceptance; all other fields and full Fault policy remain open.

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `ptz.GetServiceCapabilities` | `ptz::resp_ptz_service_capabilities` | `` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetStatus` | `ptz::resp_ptz_status` | `state, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.GetPresets` | `ptz::resp_ptz_presets` | `state, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.SetPreset` | `ptz::handle_ptz_set_preset` | `state, body, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.RemovePreset` | `ptz::handle_ptz_remove_preset` | `state, body, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.GotoPreset` | `ptz::handle_ptz_goto_preset` | `state, body, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.AbsoluteMove` | `ptz::handle_ptz_absolute_move` | `state, body, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.RelativeMove` | `ptz::handle_ptz_relative_move` | `state, body, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.ContinuousMove` | `ptz::handle_ptz_continuous_move` | `state, body, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.Stop` | `ptz::handle_ptz_stop` | `state, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.GotoHomePosition` | `ptz::handle_ptz_goto_home_position` | `state, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.SetHomePosition` | `ptz::handle_ptz_set_home_position` | `state, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.GetNodes` | `ptz::resp_ptz_nodes` | `state` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetNode` | `ptz::resp_ptz_node` | `state, operation` | W11 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | RS1 |
| `ptz.GetConfigurations` | `ptz::resp_ptz_configurations` | `state` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetCompatibleConfigurations` | `ptz::resp_ptz_compatible_configurations` | `state, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.GetConfiguration` | `ptz::resp_ptz_configuration` | `state, operation` | W11 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | RS1 |
| `ptz.SetConfiguration` | `ptz::handle_ptz_set_configuration` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetConfigurationOptions` | `ptz::resp_ptz_configuration_options` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetPresetTours` | `ptz::resp_ptz_preset_tours` | `state, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.GetPresetTour` | `ptz::resp_ptz_preset_tour` | `state, body, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.GetPresetTourOptions` | `ptz::resp_ptz_preset_tour_options` | `state, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.CreatePresetTour` | `ptz::handle_ptz_create_preset_tour` | `state, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.ModifyPresetTour` | `ptz::handle_ptz_modify_preset_tour` | `state, body, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.OperatePresetTour` | `ptz::handle_ptz_operate_preset_tour` | `state, body, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.RemovePresetTour` | `ptz::handle_ptz_remove_preset_tour` | `state, body, operation` | W11 | PARTIAL | PARTIAL | TODO | PARTIAL | PARTIAL | PTZ1 |
| `ptz.SendAuxiliaryCommand` | `ptz::handle_ptz_send_auxiliary_command` | `body` | W11 | TODO | PARTIAL | PARTIAL | PARTIAL | PARTIAL | A3 |

## imaging

[dispatch_imaging](../../src/mock/dispatch.rs) · [services/imaging.rs](../../src/mock/services/imaging.rs)

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `imaging.GetServiceCapabilities` | `imaging::resp_imaging_service_capabilities` | `` | W12 | TODO | TODO | TODO | TODO | TODO | - |
| `imaging.GetImagingSettings` | `imaging::resp_imaging_settings` | `state, body` | W12 | TODO | TODO | TODO | TODO | TODO | - |
| `imaging.SetImagingSettings` | `imaging::handle_set_imaging_settings` | `state, body` | W12 | TODO | TODO | TODO | TODO | TODO | - |
| `imaging.GetOptions` | `imaging::resp_imaging_options` | `state, body` | W12 | TODO | TODO | TODO | TODO | TODO | - |
| `imaging.GetStatus` | `imaging::resp_imaging_status` | `state, body` | W12 | TODO | TODO | TODO | TODO | TODO | - |
| `imaging.GetMoveOptions` | `imaging::resp_imaging_move_options` | `state, body` | W12 | TODO | TODO | TODO | TODO | TODO | - |
| `imaging.Move` | `imaging::handle_imaging_move` | `state, body` | W12 | TODO | TODO | TODO | TODO | TODO | - |
| `imaging.Stop` | `imaging::handle_imaging_stop` | `state, body` | W12 | TODO | TODO | TODO | TODO | TODO | - |

## events

[dispatch_events](../../src/mock/dispatch.rs) · [services/events.rs](../../src/mock/services/events.rs)

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `events.GetServiceCapabilitiesRequest` | `events::resp_event_service_capabilities` | `` | W15 | TODO | TODO | TODO | TODO | TODO | - |
| `events.GetEventPropertiesRequest` | `events::resp_event_properties` | `` | W15 | TODO | TODO | TODO | TODO | TODO | - |
| `events.CreatePullPointSubscriptionRequest` | `events::resp_create_pull_point_subscription` | `base, state, body` | W15 | TODO | TODO | TODO | TODO | TODO | - |
| `events.PullMessagesRequest` | `events::resp_pull_messages` | `state` | W15 | PARTIAL | TODO | TODO | PARTIAL | PARTIAL | EP1 |
| `events.SubscribeRequest` | `events::resp_subscribe` | `base` | W15 | TODO | PARTIAL | PARTIAL | PARTIAL | PARTIAL | A3 |
| `events.RenewRequest` | `events::resp_renew` | `` | W15 | TODO | PARTIAL | PARTIAL | PARTIAL | PARTIAL | A3 |
| `events.UnsubscribeRequest` | `resp_empty` | `"wsnt", "UnsubscribeResponse"` | W15 | TODO | PARTIAL | PARTIAL | PARTIAL | PARTIAL | A2 |
| `events.SetSynchronizationPointRequest` | `resp_empty` | `"tev", "SetSynchronizationPointResponse"` | W15 | TODO | PARTIAL | PARTIAL | PARTIAL | PARTIAL | A2 |

## recording

[dispatch_recording](../../src/mock/dispatch.rs) · [services/recording.rs](../../src/mock/services/recording.rs)

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `recording.GetServiceCapabilities` | `recording::resp_recording_service_capabilities` | `` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `recording.GetRecordings` | `recording::resp_recordings` | `state` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `recording.CreateRecording` | `recording::handle_create_recording` | `state, body` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `recording.DeleteRecording` | `recording::handle_delete_recording` | `state, body` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `recording.CreateTrack` | `recording::handle_create_track` | `state, body` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `recording.DeleteTrack` | `recording::handle_delete_track` | `state, body` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `recording.GetRecordingJobs` | `recording::resp_recording_jobs` | `state` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `recording.CreateRecordingJob` | `recording::handle_create_recording_job` | `state, body` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `recording.SetRecordingJobMode` | `recording::handle_set_recording_job_mode` | `state, body` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `recording.DeleteRecordingJob` | `recording::handle_delete_recording_job` | `state, body` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `recording.GetRecordingJobState` | `recording::resp_recording_job_state` | `state, operation` | W14 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | RS1 |

## search

[dispatch_search](../../src/mock/dispatch.rs) · [services/recording.rs](../../src/mock/services/recording.rs)

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `search.GetServiceCapabilities` | `recording::resp_search_service_capabilities` | `` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `search.FindRecordings` | `recording::resp_find_recordings` | `` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `search.GetRecordingSearchResults` | `recording::resp_recording_search_results` | `state` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `search.EndSearch` | `recording::resp_end_search` | `` | W14 | TODO | PARTIAL | PARTIAL | PARTIAL | PARTIAL | A3 |

## replay

[dispatch_replay](../../src/mock/dispatch.rs) · [services/recording.rs](../../src/mock/services/recording.rs)

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `replay.GetServiceCapabilities` | `recording::resp_replay_service_capabilities` | `` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `replay.GetReplayUri` | `recording::resp_replay_uri` | `state, body` | W14 | TODO | TODO | TODO | TODO | TODO | - |

## Maintenance

Run from the repository root:

```powershell
rtk powershell -NoProfile -File docs/active/check-mock-fidelity-inventory.ps1 -SelfTest
```

The read-only checker compares route keys, handler symbols, arguments and
bilingual tracking, plus literal Action declarations and the five reader spellings
indexed in the source audit. It rejects duplicates/empty inventories and supported
classes of source-shape drift. Its in-memory self-tests do not edit source or use a camera.
It is a source-shape check, not a Rust AST, call-graph or normative validator.
It does not audit handler semantics or prove runtime Action URI/body agreement.
W00/W02/W07 cover those separate checks. The W22 source-inventory sub-slice is
wired in `.github/workflows/ci.yml` for Windows and Linux, using PowerShell 7
and `-SelfTest`; `package` depends on its success. External-schema CI is also wired
for the selected corpus. Hosted results are revision-specific in the
[release evidence](../done/release-0.17-cut.md#evidence); this source-shape check does not
establish final-candidate CI acceptance or repository required-check settings.

Local W22 evidence (2026-09-10): PowerShell 7 executed the exact CI command;
all 16 rejection controls and real-source reconciliation passed. Temporarily
changing the `media.GetProfiles` ledger handler caused exit 1 with the precise
route-target diagnostic; restoring it returned exit 0. YAML parsing and the
package dependency were checked locally. This is not a hosted Linux run or a
branch-protection change.

A route addition/removal/retargeting must update both tables and the relevant
operation card in the same change. A newly discovered helper/field/error path
must extend the card and W02 source map even when this check stays green.
Do not increase a count, mark a row DONE or remove a row solely to silence a
failure. Preserve the reason and replacement ID for removed operations.

RS1: [scoped read selectors](../done/mock-fidelity-read-selectors.md).

EP1: [event pull consistency](../done/mock-fidelity-event-pull.md).

OS1 (2026-09-23): [bounded OSD CRUD](../done/mock-fidelity-osd-crud.md) adds scoped candidates, atomic per-source quotas, binding refusal and commit-only persistence/replay. Client coordinate/color/persistence XML and quota parsing are corrected. Selected external corpus: 198 instances / 56 operations. Background color, temporary text and arbitrary write extensions refuse explicitly; URI/source-mode and wider W10 work remain open.
