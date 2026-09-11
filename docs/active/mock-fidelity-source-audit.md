# Mock fidelity source audit

[English](mock-fidelity-source-audit.md) | [繁體中文](mock-fidelity-source-audit_zh.md)

Source baseline: `892aa94`, audited 2026-09-10. Work: W00 and W02.
This is measured project-source indexing, not a schema catalogue.
[Execution checklist](mock-fidelity-execution-checklist.md) · [Operation ledger](mock-fidelity-operation-ledger.md)

| Section | Purpose |
| --- | --- |
| [Results and boundaries](#results-and-boundaries) | What is and is not complete |
| [Action declarations](#action-declarations) | Full URI, method and route correspondence |
| [Reader call sites](#reader-call-sites) | Reproducible direct-call multiset |
| [Migration ownership](#migration-ownership) | Text/subtree/attribute/shared consumers |
| [New findings](#new-findings) | Evidence and outstanding work |
| [Reproduction and handoff](#reproduction-and-handoff) | Commands and next work |

## Results and boundaries

- 159 literal `const ACTION` declarations in `src/client/*.rs` plus
  `src/session.rs` map to 157 unique Action URIs and all 157 dispatch keys.
  Two Media2 encoder methods share one URI; session `get_osd_options` repeats
  its client's URI. No source route lacks a corresponding declaration.
- The session method is a direct request path, not merely a delegate. The old
  dispatch-test comment said it declared no Action; that statement was wrong.
- 255 direct occurrences of five reader spellings are indexed: 240 before
  top-level test modules and 15 inside those modules. The former span 76
  enclosing symbols, **not** 76 defective operations. This includes test-only
  `required_text`, canonicalization, discovery and an intentionally unused
  helper touch; it is not a count of legacy production bugs.
- W00 source reconciliation is complete for the current literal shapes.
  K06 synthetic alias routing is now repaired in the
  [pipeline checkpoint](mock-fidelity-pipeline-preflight.md#exact-action-routing);
  HTTP extraction, body agreement and replay remain separate work.
  W02 has a complete direct-call index for these five spellings, but is still
  PARTIAL: transitive call paths and per-field classifications are not all closed.
- These scanners are deliberately source-shape tools, not Rust AST parsers.
  Function ownership uses the preceding declaration; test scope uses the current
  top-level test-module convention. Imports/aliases, macros, multiline/generic
  invocation changes, closures and custom readers require manual review. A new
  method need not use a constant named ACTION; inspect request call sites too.
  Compare the index with `rg`, do not mistake equality for normative acceptance.

## Action declarations

Exact strings below come from client/session source, not official WSDL tables.
The checker compares the full URI **and** source method and then reconciles the
unique route-key set with dispatch. Source URI changes cannot be hidden behind
an unchanged last segment. This does not enforce those checks in runtime dispatch.

| Site | Source Action | Route ID |
| --- | --- | --- |
| `src/client/device.rs::device_get_service_capabilities` | `http://www.onvif.org/ver10/device/wsdl/GetServiceCapabilities` | `device.GetServiceCapabilities` |
| `src/client/device.rs::get_capabilities` | `http://www.onvif.org/ver10/device/wsdl/GetCapabilities` | `device.GetCapabilities` |
| `src/client/device.rs::get_services` | `http://www.onvif.org/ver10/device/wsdl/GetServices` | `device.GetServices` |
| `src/client/device.rs::get_system_date_and_time` | `http://www.onvif.org/ver10/device/wsdl/GetSystemDateAndTime` | `device.GetSystemDateAndTime` |
| `src/client/device.rs::set_system_date_and_time` | `http://www.onvif.org/ver10/device/wsdl/SetSystemDateAndTime` | `device.SetSystemDateAndTime` |
| `src/client/device.rs::get_device_info` | `http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation` | `device.GetDeviceInformation` |
| `src/client/device.rs::get_hostname` | `http://www.onvif.org/ver10/device/wsdl/GetHostname` | `device.GetHostname` |
| `src/client/device.rs::set_hostname` | `http://www.onvif.org/ver10/device/wsdl/SetHostname` | `device.SetHostname` |
| `src/client/device.rs::get_ntp` | `http://www.onvif.org/ver10/device/wsdl/GetNTP` | `device.GetNTP` |
| `src/client/device.rs::set_ntp` | `http://www.onvif.org/ver10/device/wsdl/SetNTP` | `device.SetNTP` |
| `src/client/device.rs::system_reboot` | `http://www.onvif.org/ver10/device/wsdl/SystemReboot` | `device.SystemReboot` |
| `src/client/device.rs::get_scopes` | `http://www.onvif.org/ver10/device/wsdl/GetScopes` | `device.GetScopes` |
| `src/client/device.rs::set_scopes` | `http://www.onvif.org/ver10/device/wsdl/SetScopes` | `device.SetScopes` |
| `src/client/device.rs::get_users` | `http://www.onvif.org/ver10/device/wsdl/GetUsers` | `device.GetUsers` |
| `src/client/device.rs::create_users` | `http://www.onvif.org/ver10/device/wsdl/CreateUsers` | `device.CreateUsers` |
| `src/client/device.rs::delete_users` | `http://www.onvif.org/ver10/device/wsdl/DeleteUsers` | `device.DeleteUsers` |
| `src/client/device.rs::set_user` | `http://www.onvif.org/ver10/device/wsdl/SetUser` | `device.SetUser` |
| `src/client/device.rs::get_network_interfaces` | `http://www.onvif.org/ver10/device/wsdl/GetNetworkInterfaces` | `device.GetNetworkInterfaces` |
| `src/client/device.rs::set_network_interfaces` | `http://www.onvif.org/ver10/device/wsdl/SetNetworkInterfaces` | `device.SetNetworkInterfaces` |
| `src/client/device.rs::get_network_protocols` | `http://www.onvif.org/ver10/device/wsdl/GetNetworkProtocols` | `device.GetNetworkProtocols` |
| `src/client/device.rs::get_dns` | `http://www.onvif.org/ver10/device/wsdl/GetDNS` | `device.GetDNS` |
| `src/client/device.rs::set_dns` | `http://www.onvif.org/ver10/device/wsdl/SetDNS` | `device.SetDNS` |
| `src/client/device.rs::get_network_default_gateway` | `http://www.onvif.org/ver10/device/wsdl/GetNetworkDefaultGateway` | `device.GetNetworkDefaultGateway` |
| `src/client/device.rs::set_network_default_gateway` | `http://www.onvif.org/ver10/device/wsdl/SetNetworkDefaultGateway` | `device.SetNetworkDefaultGateway` |
| `src/client/device.rs::send_auxiliary_command` | `http://www.onvif.org/ver10/device/wsdl/SendAuxiliaryCommand` | `device.SendAuxiliaryCommand` |
| `src/client/device.rs::get_system_log` | `http://www.onvif.org/ver10/device/wsdl/GetSystemLog` | `device.GetSystemLog` |
| `src/client/device.rs::get_relay_outputs` | `http://www.onvif.org/ver10/device/wsdl/GetRelayOutputs` | `device.GetRelayOutputs` |
| `src/client/device.rs::set_relay_output_settings` | `http://www.onvif.org/ver10/device/wsdl/SetRelayOutputSettings` | `device.SetRelayOutputSettings` |
| `src/client/device.rs::set_relay_output_state` | `http://www.onvif.org/ver10/device/wsdl/SetRelayOutputState` | `device.SetRelayOutputState` |
| `src/client/device.rs::get_digital_inputs` | `http://www.onvif.org/ver10/deviceio/wsdl/GetDigitalInputs` | `device_io.GetDigitalInputs` |
| `src/client/device.rs::set_network_protocols` | `http://www.onvif.org/ver10/device/wsdl/SetNetworkProtocols` | `device.SetNetworkProtocols` |
| `src/client/device.rs::set_system_factory_default` | `http://www.onvif.org/ver10/device/wsdl/SetSystemFactoryDefault` | `device.SetSystemFactoryDefault` |
| `src/client/device.rs::get_storage_configurations` | `http://www.onvif.org/ver10/device/wsdl/GetStorageConfigurations` | `device.GetStorageConfigurations` |
| `src/client/device.rs::set_storage_configuration` | `http://www.onvif.org/ver10/device/wsdl/SetStorageConfiguration` | `device.SetStorageConfiguration` |
| `src/client/device.rs::get_system_uris` | `http://www.onvif.org/ver10/device/wsdl/GetSystemUris` | `device.GetSystemUris` |
| `src/client/device.rs::start_firmware_upgrade` | `http://www.onvif.org/ver10/device/wsdl/StartFirmwareUpgrade` | `device.StartFirmwareUpgrade` |
| `src/client/device.rs::start_system_restore` | `http://www.onvif.org/ver10/device/wsdl/StartSystemRestore` | `device.StartSystemRestore` |
| `src/client/device.rs::get_discovery_mode` | `http://www.onvif.org/ver10/device/wsdl/GetDiscoveryMode` | `device.GetDiscoveryMode` |
| `src/client/device.rs::set_discovery_mode` | `http://www.onvif.org/ver10/device/wsdl/SetDiscoveryMode` | `device.SetDiscoveryMode` |
| `src/client/events.rs::events_get_service_capabilities` | `http://www.onvif.org/ver10/events/wsdl/EventPortType/GetServiceCapabilitiesRequest` | `events.GetServiceCapabilitiesRequest` |
| `src/client/events.rs::get_event_properties` | `http://www.onvif.org/ver10/events/wsdl/EventPortType/GetEventPropertiesRequest` | `events.GetEventPropertiesRequest` |
| `src/client/events.rs::create_pull_point_subscription` | `http://www.onvif.org/ver10/events/wsdl/EventPortType/CreatePullPointSubscriptionRequest` | `events.CreatePullPointSubscriptionRequest` |
| `src/client/events.rs::pull_messages` | `http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/PullMessagesRequest` | `events.PullMessagesRequest` |
| `src/client/events.rs::renew_subscription` | `http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/RenewRequest` | `events.RenewRequest` |
| `src/client/events.rs::set_synchronization_point` | `http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/SetSynchronizationPointRequest` | `events.SetSynchronizationPointRequest` |
| `src/client/events.rs::unsubscribe` | `http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/UnsubscribeRequest` | `events.UnsubscribeRequest` |
| `src/client/events.rs::subscribe` | `http://docs.oasis-open.org/wsn/bw-2/NotificationProducer/SubscribeRequest` | `events.SubscribeRequest` |
| `src/client/imaging.rs::imaging_get_service_capabilities` | `http://www.onvif.org/ver20/imaging/wsdl/GetServiceCapabilities` | `imaging.GetServiceCapabilities` |
| `src/client/imaging.rs::get_imaging_settings` | `http://www.onvif.org/ver20/imaging/wsdl/GetImagingSettings` | `imaging.GetImagingSettings` |
| `src/client/imaging.rs::set_imaging_settings` | `http://www.onvif.org/ver20/imaging/wsdl/SetImagingSettings` | `imaging.SetImagingSettings` |
| `src/client/imaging.rs::get_imaging_options` | `http://www.onvif.org/ver20/imaging/wsdl/GetOptions` | `imaging.GetOptions` |
| `src/client/imaging.rs::imaging_move` | `http://www.onvif.org/ver20/imaging/wsdl/Move` | `imaging.Move` |
| `src/client/imaging.rs::imaging_stop` | `http://www.onvif.org/ver20/imaging/wsdl/Stop` | `imaging.Stop` |
| `src/client/imaging.rs::imaging_get_move_options` | `http://www.onvif.org/ver20/imaging/wsdl/GetMoveOptions` | `imaging.GetMoveOptions` |
| `src/client/imaging.rs::imaging_get_status` | `http://www.onvif.org/ver20/imaging/wsdl/GetStatus` | `imaging.GetStatus` |
| `src/client/media.rs::media_get_service_capabilities` | `http://www.onvif.org/ver10/media/wsdl/GetServiceCapabilities` | `media.GetServiceCapabilities` |
| `src/client/media.rs::get_profiles` | `http://www.onvif.org/ver10/media/wsdl/GetProfiles` | `media.GetProfiles` |
| `src/client/media.rs::get_stream_uri` | `http://www.onvif.org/ver10/media/wsdl/GetStreamUri` | `media.GetStreamUri` |
| `src/client/media.rs::get_snapshot_uri` | `http://www.onvif.org/ver10/media/wsdl/GetSnapshotUri` | `media.GetSnapshotUri` |
| `src/client/media.rs::create_profile` | `http://www.onvif.org/ver10/media/wsdl/CreateProfile` | `media.CreateProfile` |
| `src/client/media.rs::delete_profile` | `http://www.onvif.org/ver10/media/wsdl/DeleteProfile` | `media.DeleteProfile` |
| `src/client/media.rs::get_profile` | `http://www.onvif.org/ver10/media/wsdl/GetProfile` | `media.GetProfile` |
| `src/client/media.rs::add_video_encoder_configuration` | `http://www.onvif.org/ver10/media/wsdl/AddVideoEncoderConfiguration` | `media.AddVideoEncoderConfiguration` |
| `src/client/media.rs::remove_video_encoder_configuration` | `http://www.onvif.org/ver10/media/wsdl/RemoveVideoEncoderConfiguration` | `media.RemoveVideoEncoderConfiguration` |
| `src/client/media.rs::add_video_source_configuration` | `http://www.onvif.org/ver10/media/wsdl/AddVideoSourceConfiguration` | `media.AddVideoSourceConfiguration` |
| `src/client/media.rs::remove_video_source_configuration` | `http://www.onvif.org/ver10/media/wsdl/RemoveVideoSourceConfiguration` | `media.RemoveVideoSourceConfiguration` |
| `src/client/media.rs::get_video_sources` | `http://www.onvif.org/ver10/media/wsdl/GetVideoSources` | `media.GetVideoSources` |
| `src/client/media.rs::get_video_source_configurations` | `http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfigurations` | `media.GetVideoSourceConfigurations` |
| `src/client/media.rs::get_video_source_configuration` | `http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfiguration` | `media.GetVideoSourceConfiguration` |
| `src/client/media.rs::set_video_source_configuration` | `http://www.onvif.org/ver10/media/wsdl/SetVideoSourceConfiguration` | `media.SetVideoSourceConfiguration` |
| `src/client/media.rs::get_video_source_configuration_options` | `http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfigurationOptions` | `media.GetVideoSourceConfigurationOptions` |
| `src/client/media.rs::get_video_encoder_configurations` | `http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfigurations` | `media.GetVideoEncoderConfigurations` |
| `src/client/media.rs::get_video_encoder_configuration` | `http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfiguration` | `media.GetVideoEncoderConfiguration` |
| `src/client/media.rs::set_video_encoder_configuration` | `http://www.onvif.org/ver10/media/wsdl/SetVideoEncoderConfiguration` | `media.SetVideoEncoderConfiguration` |
| `src/client/media.rs::get_video_encoder_configuration_options` | `http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfigurationOptions` | `media.GetVideoEncoderConfigurationOptions` |
| `src/client/media.rs::get_osds` | `http://www.onvif.org/ver10/media/wsdl/GetOSDs` | `media.GetOSDs` |
| `src/client/media.rs::get_osd` | `http://www.onvif.org/ver10/media/wsdl/GetOSD` | `media.GetOSD` |
| `src/client/media.rs::set_osd` | `http://www.onvif.org/ver10/media/wsdl/SetOSD` | `media.SetOSD` |
| `src/client/media.rs::create_osd` | `http://www.onvif.org/ver10/media/wsdl/CreateOSD` | `media.CreateOSD` |
| `src/client/media.rs::delete_osd` | `http://www.onvif.org/ver10/media/wsdl/DeleteOSD` | `media.DeleteOSD` |
| `src/client/media.rs::get_osd_options` | `http://www.onvif.org/ver10/media/wsdl/GetOSDOptions` | `media.GetOSDOptions` |
| `src/client/media.rs::get_audio_sources` | `http://www.onvif.org/ver10/media/wsdl/GetAudioSources` | `media.GetAudioSources` |
| `src/client/media.rs::get_audio_source_configurations` | `http://www.onvif.org/ver10/media/wsdl/GetAudioSourceConfigurations` | `media.GetAudioSourceConfigurations` |
| `src/client/media.rs::get_audio_encoder_configurations` | `http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfigurations` | `media.GetAudioEncoderConfigurations` |
| `src/client/media.rs::get_audio_encoder_configuration` | `http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfiguration` | `media.GetAudioEncoderConfiguration` |
| `src/client/media.rs::set_audio_encoder_configuration` | `http://www.onvif.org/ver10/media/wsdl/SetAudioEncoderConfiguration` | `media.SetAudioEncoderConfiguration` |
| `src/client/media.rs::get_audio_encoder_configuration_options` | `http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfigurationOptions` | `media.GetAudioEncoderConfigurationOptions` |
| `src/client/media2.rs::media2_get_service_capabilities` | `http://www.onvif.org/ver20/media/wsdl/GetServiceCapabilities` | `media2.GetServiceCapabilities` |
| `src/client/media2.rs::get_profiles_media2` | `http://www.onvif.org/ver20/media/wsdl/GetProfiles` | `media2.GetProfiles` |
| `src/client/media2.rs::get_stream_uri_media2` | `http://www.onvif.org/ver20/media/wsdl/GetStreamUri` | `media2.GetStreamUri` |
| `src/client/media2.rs::get_snapshot_uri_media2` | `http://www.onvif.org/ver20/media/wsdl/GetSnapshotUri` | `media2.GetSnapshotUri` |
| `src/client/media2.rs::get_video_source_configurations_media2` | `http://www.onvif.org/ver20/media/wsdl/GetVideoSourceConfigurations` | `media2.GetVideoSourceConfigurations` |
| `src/client/media2.rs::set_video_source_configuration_media2` | `http://www.onvif.org/ver20/media/wsdl/SetVideoSourceConfiguration` | `media2.SetVideoSourceConfiguration` |
| `src/client/media2.rs::get_video_source_configuration_options_media2` | `http://www.onvif.org/ver20/media/wsdl/GetVideoSourceConfigurationOptions` | `media2.GetVideoSourceConfigurationOptions` |
| `src/client/media2.rs::get_video_encoder_configurations_media2` | `http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurations` | `media2.GetVideoEncoderConfigurations` |
| `src/client/media2.rs::get_video_encoder_configuration_media2` | `http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurations` | `media2.GetVideoEncoderConfigurations` |
| `src/client/media2.rs::set_video_encoder_configuration_media2` | `http://www.onvif.org/ver20/media/wsdl/SetVideoEncoderConfiguration` | `media2.SetVideoEncoderConfiguration` |
| `src/client/media2.rs::get_video_encoder_configuration_options_media2` | `http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurationOptions` | `media2.GetVideoEncoderConfigurationOptions` |
| `src/client/media2.rs::get_video_encoder_instances_media2` | `http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderInstances` | `media2.GetVideoEncoderInstances` |
| `src/client/media2.rs::create_profile_media2` | `http://www.onvif.org/ver20/media/wsdl/CreateProfile` | `media2.CreateProfile` |
| `src/client/media2.rs::delete_profile_media2` | `http://www.onvif.org/ver20/media/wsdl/DeleteProfile` | `media2.DeleteProfile` |
| `src/client/media2.rs::add_configuration_media2` | `http://www.onvif.org/ver20/media/wsdl/AddConfiguration` | `media2.AddConfiguration` |
| `src/client/media2.rs::remove_configuration_media2` | `http://www.onvif.org/ver20/media/wsdl/RemoveConfiguration` | `media2.RemoveConfiguration` |
| `src/client/media2.rs::get_metadata_configurations_media2` | `http://www.onvif.org/ver20/media/wsdl/GetMetadataConfigurations` | `media2.GetMetadataConfigurations` |
| `src/client/media2.rs::set_metadata_configuration_media2` | `http://www.onvif.org/ver20/media/wsdl/SetMetadataConfiguration` | `media2.SetMetadataConfiguration` |
| `src/client/media2.rs::get_metadata_configuration_options_media2` | `http://www.onvif.org/ver20/media/wsdl/GetMetadataConfigurationOptions` | `media2.GetMetadataConfigurationOptions` |
| `src/client/media2.rs::get_audio_source_configurations_media2` | `http://www.onvif.org/ver20/media/wsdl/GetAudioSourceConfigurations` | `media2.GetAudioSourceConfigurations` |
| `src/client/media2.rs::get_audio_encoder_configurations_media2` | `http://www.onvif.org/ver20/media/wsdl/GetAudioEncoderConfigurations` | `media2.GetAudioEncoderConfigurations` |
| `src/client/media2.rs::get_audio_encoder_configuration_options_media2` | `http://www.onvif.org/ver20/media/wsdl/GetAudioEncoderConfigurationOptions` | `media2.GetAudioEncoderConfigurationOptions` |
| `src/client/media2.rs::set_audio_encoder_configuration_media2` | `http://www.onvif.org/ver20/media/wsdl/SetAudioEncoderConfiguration` | `media2.SetAudioEncoderConfiguration` |
| `src/client/media2.rs::get_audio_output_configurations_media2` | `http://www.onvif.org/ver20/media/wsdl/GetAudioOutputConfigurations` | `media2.GetAudioOutputConfigurations` |
| `src/client/media2.rs::get_audio_decoder_configurations_media2` | `http://www.onvif.org/ver20/media/wsdl/GetAudioDecoderConfigurations` | `media2.GetAudioDecoderConfigurations` |
| `src/client/media2.rs::get_video_source_modes_media2` | `http://www.onvif.org/ver20/media/wsdl/GetVideoSourceModes` | `media2.GetVideoSourceModes` |
| `src/client/media2.rs::set_video_source_mode_media2` | `http://www.onvif.org/ver20/media/wsdl/SetVideoSourceMode` | `media2.SetVideoSourceMode` |
| `src/client/ptz.rs::ptz_get_service_capabilities` | `http://www.onvif.org/ver20/ptz/wsdl/GetServiceCapabilities` | `ptz.GetServiceCapabilities` |
| `src/client/ptz.rs::ptz_absolute_move` | `http://www.onvif.org/ver20/ptz/wsdl/AbsoluteMove` | `ptz.AbsoluteMove` |
| `src/client/ptz.rs::ptz_relative_move` | `http://www.onvif.org/ver20/ptz/wsdl/RelativeMove` | `ptz.RelativeMove` |
| `src/client/ptz.rs::ptz_continuous_move` | `http://www.onvif.org/ver20/ptz/wsdl/ContinuousMove` | `ptz.ContinuousMove` |
| `src/client/ptz.rs::ptz_stop` | `http://www.onvif.org/ver20/ptz/wsdl/Stop` | `ptz.Stop` |
| `src/client/ptz.rs::ptz_get_presets` | `http://www.onvif.org/ver20/ptz/wsdl/GetPresets` | `ptz.GetPresets` |
| `src/client/ptz.rs::ptz_goto_preset` | `http://www.onvif.org/ver20/ptz/wsdl/GotoPreset` | `ptz.GotoPreset` |
| `src/client/ptz.rs::ptz_set_preset` | `http://www.onvif.org/ver20/ptz/wsdl/SetPreset` | `ptz.SetPreset` |
| `src/client/ptz.rs::ptz_remove_preset` | `http://www.onvif.org/ver20/ptz/wsdl/RemovePreset` | `ptz.RemovePreset` |
| `src/client/ptz.rs::ptz_get_status` | `http://www.onvif.org/ver20/ptz/wsdl/GetStatus` | `ptz.GetStatus` |
| `src/client/ptz.rs::ptz_goto_home_position` | `http://www.onvif.org/ver20/ptz/wsdl/GotoHomePosition` | `ptz.GotoHomePosition` |
| `src/client/ptz.rs::ptz_set_home_position` | `http://www.onvif.org/ver20/ptz/wsdl/SetHomePosition` | `ptz.SetHomePosition` |
| `src/client/ptz.rs::ptz_get_configurations` | `http://www.onvif.org/ver20/ptz/wsdl/GetConfigurations` | `ptz.GetConfigurations` |
| `src/client/ptz.rs::ptz_get_configuration` | `http://www.onvif.org/ver20/ptz/wsdl/GetConfiguration` | `ptz.GetConfiguration` |
| `src/client/ptz.rs::ptz_set_configuration` | `http://www.onvif.org/ver20/ptz/wsdl/SetConfiguration` | `ptz.SetConfiguration` |
| `src/client/ptz.rs::ptz_get_configuration_options` | `http://www.onvif.org/ver20/ptz/wsdl/GetConfigurationOptions` | `ptz.GetConfigurationOptions` |
| `src/client/ptz.rs::ptz_get_nodes` | `http://www.onvif.org/ver20/ptz/wsdl/GetNodes` | `ptz.GetNodes` |
| `src/client/ptz.rs::ptz_get_node` | `http://www.onvif.org/ver20/ptz/wsdl/GetNode` | `ptz.GetNode` |
| `src/client/ptz.rs::ptz_get_compatible_configurations` | `http://www.onvif.org/ver20/ptz/wsdl/GetCompatibleConfigurations` | `ptz.GetCompatibleConfigurations` |
| `src/client/ptz.rs::ptz_get_preset_tours` | `http://www.onvif.org/ver20/ptz/wsdl/GetPresetTours` | `ptz.GetPresetTours` |
| `src/client/ptz.rs::ptz_get_preset_tour` | `http://www.onvif.org/ver20/ptz/wsdl/GetPresetTour` | `ptz.GetPresetTour` |
| `src/client/ptz.rs::ptz_get_preset_tour_options` | `http://www.onvif.org/ver20/ptz/wsdl/GetPresetTourOptions` | `ptz.GetPresetTourOptions` |
| `src/client/ptz.rs::ptz_create_preset_tour` | `http://www.onvif.org/ver20/ptz/wsdl/CreatePresetTour` | `ptz.CreatePresetTour` |
| `src/client/ptz.rs::ptz_modify_preset_tour` | `http://www.onvif.org/ver20/ptz/wsdl/ModifyPresetTour` | `ptz.ModifyPresetTour` |
| `src/client/ptz.rs::ptz_operate_preset_tour` | `http://www.onvif.org/ver20/ptz/wsdl/OperatePresetTour` | `ptz.OperatePresetTour` |
| `src/client/ptz.rs::ptz_remove_preset_tour` | `http://www.onvif.org/ver20/ptz/wsdl/RemovePresetTour` | `ptz.RemovePresetTour` |
| `src/client/ptz.rs::ptz_send_auxiliary_command` | `http://www.onvif.org/ver20/ptz/wsdl/SendAuxiliaryCommand` | `ptz.SendAuxiliaryCommand` |
| `src/client/recording.rs::recording_get_service_capabilities` | `http://www.onvif.org/ver10/recording/wsdl/GetServiceCapabilities` | `recording.GetServiceCapabilities` |
| `src/client/recording.rs::search_get_service_capabilities` | `http://www.onvif.org/ver10/search/wsdl/GetServiceCapabilities` | `search.GetServiceCapabilities` |
| `src/client/recording.rs::replay_get_service_capabilities` | `http://www.onvif.org/ver10/replay/wsdl/GetServiceCapabilities` | `replay.GetServiceCapabilities` |
| `src/client/recording.rs::get_recordings` | `http://www.onvif.org/ver10/recording/wsdl/GetRecordings` | `recording.GetRecordings` |
| `src/client/recording.rs::create_recording` | `http://www.onvif.org/ver10/recording/wsdl/CreateRecording` | `recording.CreateRecording` |
| `src/client/recording.rs::delete_recording` | `http://www.onvif.org/ver10/recording/wsdl/DeleteRecording` | `recording.DeleteRecording` |
| `src/client/recording.rs::create_track` | `http://www.onvif.org/ver10/recording/wsdl/CreateTrack` | `recording.CreateTrack` |
| `src/client/recording.rs::delete_track` | `http://www.onvif.org/ver10/recording/wsdl/DeleteTrack` | `recording.DeleteTrack` |
| `src/client/recording.rs::get_recording_jobs` | `http://www.onvif.org/ver10/recording/wsdl/GetRecordingJobs` | `recording.GetRecordingJobs` |
| `src/client/recording.rs::create_recording_job` | `http://www.onvif.org/ver10/recording/wsdl/CreateRecordingJob` | `recording.CreateRecordingJob` |
| `src/client/recording.rs::set_recording_job_mode` | `http://www.onvif.org/ver10/recording/wsdl/SetRecordingJobMode` | `recording.SetRecordingJobMode` |
| `src/client/recording.rs::delete_recording_job` | `http://www.onvif.org/ver10/recording/wsdl/DeleteRecordingJob` | `recording.DeleteRecordingJob` |
| `src/client/recording.rs::get_recording_job_state` | `http://www.onvif.org/ver10/recording/wsdl/GetRecordingJobState` | `recording.GetRecordingJobState` |
| `src/client/recording.rs::find_recordings` | `http://www.onvif.org/ver10/search/wsdl/FindRecordings` | `search.FindRecordings` |
| `src/client/recording.rs::get_recording_search_results` | `http://www.onvif.org/ver10/search/wsdl/GetRecordingSearchResults` | `search.GetRecordingSearchResults` |
| `src/client/recording.rs::end_search` | `http://www.onvif.org/ver10/search/wsdl/EndSearch` | `search.EndSearch` |
| `src/client/recording.rs::get_replay_uri` | `http://www.onvif.org/ver10/replay/wsdl/GetReplayUri` | `replay.GetReplayUri` |
| `src/session.rs::get_osd_options` | `http://www.onvif.org/ver10/media/wsdl/GetOSDOptions` | `media.GetOSDOptions` |

## Reader call sites

Counts are per symbol/helper/scope, so moving a call or adding another occurrence
changes the multiset. Line numbers are not pinned. Open the file in the site key
and search its symbol. Helper definitions and whole-line comments are excluded.
Inline comments/string examples and unsupported syntax need manual inspection.

| Site | Reader | Scope:occurrences |
| --- | --- | --- |
| `src/mock/auth.rs::validate_ws_security` | `extract_tag` | `production:4` |
| `src/mock/canon.rs::canonicalize` | `XmlNode::parse` | `production:1` |
| `src/mock/discovery_responder.rs::probe_response` | `XmlNode::parse` | `production:1` |
| `src/mock/discovery_responder.rs::build_probe_match_round_trips_through_the_client_parser` | `XmlNode::parse` | `test:1` |
| `src/mock/discovery_responder.rs::unicast_probe_round_trip` | `XmlNode::parse` | `test:1` |
| `src/mock/request.rs::read` | `required_text` | `test:1` |
| `src/mock/request.rs::namespace_attribute_values_are_normalized_once` | `required_text` | `test:1` |
| `src/mock/xml_parse.rs::extract_simple_tag` | `extract_tag` | `test:1` |
| `src/mock/xml_parse.rs::extract_no_namespace` | `extract_tag` | `test:1` |
| `src/mock/xml_parse.rs::extract_nested` | `extract_tag` | `test:1` |
| `src/mock/xml_parse.rs::extract_tag_with_attributes` | `extract_tag` | `test:1` |
| `src/mock/xml_parse.rs::extract_nonce_with_encoding_type` | `extract_tag` | `test:1` |
| `src/mock/xml_parse.rs::extract_all` | `extract_all_tags` | `test:1` |
| `src/mock/xml_parse.rs::extract_missing` | `extract_tag` | `test:1` |
| `src/mock/xml_parse.rs::extract_from_full_soap_security_header` | `extract_tag` | `test:4` |
| `src/mock/services/device.rs::handle_set_discovery_mode` | `extract_tag` | `production:1` |
| `src/mock/services/device.rs::handle_set_hostname` | `extract_tag` | `production:1` |
| `src/mock/services/device.rs::handle_set_ntp` | `extract_all_tags` | `production:1` |
| `src/mock/services/device.rs::handle_set_ntp` | `extract_tag` | `production:1` |
| `src/mock/services/device.rs::handle_set_dns` | `extract_all_tags` | `production:1` |
| `src/mock/services/device.rs::handle_set_dns` | `extract_tag` | `production:1` |
| `src/mock/services/device.rs::handle_set_scopes` | `extract_all_tags` | `production:1` |
| `src/mock/services/device.rs::handle_set_system_date_and_time` | `extract_tag` | `production:2` |
| `src/mock/services/device.rs::handle_create_users` | `extract_tag` | `production:1` |
| `src/mock/services/device.rs::handle_create_users` | `extract_all_tags` | `production:3` |
| `src/mock/services/device.rs::handle_delete_users` | `extract_tag` | `production:1` |
| `src/mock/services/device.rs::handle_delete_users` | `extract_all_tags` | `production:1` |
| `src/mock/services/device.rs::handle_set_user` | `extract_tag` | `production:4` |
| `src/mock/services/device.rs::handle_set_network_interfaces` | `extract_tag` | `production:6` |
| `src/mock/services/device.rs::handle_set_network_protocols` | `extract_all_tags` | `production:3` |
| `src/mock/services/device.rs::handle_set_network_default_gateway` | `extract_all_tags` | `production:1` |
| `src/mock/services/device.rs::handle_set_relay_output_state` | `extract_tag` | `production:2` |
| `src/mock/services/device.rs::handle_set_relay_output_settings` | `extract_tag` | `production:4` |
| `src/mock/services/device.rs::handle_set_storage_configuration` | `extract_attr` | `production:2` |
| `src/mock/services/device.rs::handle_set_storage_configuration` | `extract_tag` | `production:3` |
| `src/mock/services/events.rs::resp_create_pull_point_subscription` | `extract_tag` | `production:1` |
| `src/mock/services/imaging.rs::lookup` | `extract_tag` | `production:1` |
| `src/mock/services/imaging.rs::handle_set_imaging_settings` | `extract_tag` | `production:11` |
| `src/mock/services/media.rs::resp_profile` | `extract_tag` | `production:2` |
| `src/mock/services/media.rs::handle_create_profile` | `extract_tag` | `production:2` |
| `src/mock/services/media.rs::apply_video_encoder_write` | `extract_attr` | `production:3` |
| `src/mock/services/media.rs::apply_video_encoder_write` | `extract_tag` | `production:12` |
| `src/mock/services/media.rs::apply_video_source_write` | `extract_attr` | `production:3` |
| `src/mock/services/media.rs::apply_video_source_write` | `extract_tag` | `production:2` |
| `src/mock/services/media.rs::bind_configuration` | `extract_tag` | `production:3` |
| `src/mock/services/media.rs::unbind_configuration` | `extract_tag` | `production:1` |
| `src/mock/services/media.rs::resp_video_encoder_configurations` | `extract_tag` | `production:1` |
| `src/mock/services/media.rs::resp_osds` | `extract_tag` | `production:2` |
| `src/mock/services/media.rs::require_config_token` | `extract_tag` | `production:1` |
| `src/mock/services/media.rs::resp_osd` | `extract_tag` | `production:2` |
| `src/mock/services/media.rs::handle_create_osd` | `extract_tag` | `production:1` |
| `src/mock/services/media.rs::handle_set_osd` | `extract_attr` | `production:1` |
| `src/mock/services/media.rs::handle_set_osd` | `extract_tag` | `production:1` |
| `src/mock/services/media.rs::handle_delete_osd` | `extract_tag` | `production:2` |
| `src/mock/services/media.rs::parse_osd_payload` | `extract_tag` | `production:11` |
| `src/mock/services/media.rs::parse_osd_payload` | `extract_attr` | `production:2` |
| `src/mock/services/media.rs::parse_osd_color` | `extract_tag` | `production:2` |
| `src/mock/services/media.rs::parse_osd_color` | `extract_attr` | `production:4` |
| `src/mock/services/media.rs::_force_use_extract_all` | `extract_all_tags` | `production:1` |
| `src/mock/services/media.rs::resp_audio_encoder_configuration` | `extract_tag` | `production:1` |
| `src/mock/services/media.rs::resp_audio_encoder_configuration_options` | `extract_tag` | `production:1` |
| `src/mock/services/media.rs::apply_audio_encoder_write` | `extract_attr` | `production:1` |
| `src/mock/services/media.rs::apply_audio_encoder_write` | `extract_tag` | `production:11` |
| `src/mock/services/media2.rs::require_config_token` | `extract_tag` | `production:1` |
| `src/mock/services/media2.rs::resp_video_encoder_configurations` | `extract_tag` | `production:1` |
| `src/mock/services/media2.rs::apply_media2_configuration` | `extract_tag` | `production:3` |
| `src/mock/services/media2.rs::apply_media2_configuration` | `extract_all_tags` | `production:1` |
| `src/mock/services/media2.rs::resp_metadata_configurations` | `extract_tag` | `production:1` |
| `src/mock/services/media2.rs::resp_metadata_configuration_options` | `extract_tag` | `production:1` |
| `src/mock/services/media2.rs::handle_set_metadata_configuration` | `extract_attr` | `production:1` |
| `src/mock/services/media2.rs::handle_set_metadata_configuration` | `extract_tag` | `production:5` |
| `src/mock/services/media2.rs::resp_audio_encoder_configuration_options_media2` | `extract_tag` | `production:1` |
| `src/mock/services/ptz.rs::require_profile` | `extract_tag` | `production:1` |
| `src/mock/services/ptz.rs::handle_ptz_set_preset` | `extract_tag` | `production:3` |
| `src/mock/services/ptz.rs::handle_ptz_remove_preset` | `extract_tag` | `production:2` |
| `src/mock/services/ptz.rs::handle_ptz_goto_preset` | `extract_tag` | `production:2` |
| `src/mock/services/ptz.rs::has_pan_tilt` | `extract_attr` | `production:2` |
| `src/mock/services/ptz.rs::handle_ptz_absolute_move` | `extract_tag` | `production:1` |
| `src/mock/services/ptz.rs::handle_ptz_absolute_move` | `extract_attr` | `production:3` |
| `src/mock/services/ptz.rs::handle_ptz_relative_move` | `extract_tag` | `production:1` |
| `src/mock/services/ptz.rs::handle_ptz_relative_move` | `extract_attr` | `production:3` |
| `src/mock/services/ptz.rs::handle_ptz_continuous_move` | `extract_tag` | `production:1` |
| `src/mock/services/ptz.rs::handle_ptz_continuous_move` | `extract_attr` | `production:3` |
| `src/mock/services/ptz.rs::resp_ptz_node` | `extract_tag` | `production:1` |
| `src/mock/services/ptz.rs::resp_ptz_configuration` | `extract_tag` | `production:1` |
| `src/mock/services/ptz.rs::resp_ptz_configuration_options` | `extract_tag` | `production:1` |
| `src/mock/services/ptz.rs::min_max` | `extract_tag` | `production:2` |
| `src/mock/services/ptz.rs::parse_limits` | `extract_tag` | `production:5` |
| `src/mock/services/ptz.rs::pan_tilt_attrs` | `extract_attr` | `production:2` |
| `src/mock/services/ptz.rs::apply_ptz_configuration` | `extract_attr` | `production:2` |
| `src/mock/services/ptz.rs::apply_ptz_configuration` | `extract_tag` | `production:11` |
| `src/mock/services/ptz.rs::resp_ptz_preset_tour` | `extract_tag` | `production:2` |
| `src/mock/services/ptz.rs::handle_ptz_modify_preset_tour` | `extract_tag` | `production:8` |
| `src/mock/services/ptz.rs::handle_ptz_modify_preset_tour` | `extract_attr` | `production:2` |
| `src/mock/services/ptz.rs::handle_ptz_modify_preset_tour` | `extract_all_tags` | `production:1` |
| `src/mock/services/ptz.rs::handle_ptz_operate_preset_tour` | `extract_tag` | `production:3` |
| `src/mock/services/ptz.rs::handle_ptz_remove_preset_tour` | `extract_tag` | `production:2` |
| `src/mock/services/ptz.rs::handle_ptz_send_auxiliary_command` | `extract_tag` | `production:2` |
| `src/mock/services/recording.rs::handle_create_recording` | `extract_tag` | `production:9` |
| `src/mock/services/recording.rs::handle_delete_recording` | `extract_tag` | `production:1` |
| `src/mock/services/recording.rs::handle_create_track` | `extract_tag` | `production:4` |
| `src/mock/services/recording.rs::handle_delete_track` | `extract_tag` | `production:2` |
| `src/mock/services/recording.rs::handle_create_recording_job` | `extract_tag` | `production:5` |
| `src/mock/services/recording.rs::handle_set_recording_job_mode` | `extract_tag` | `production:2` |
| `src/mock/services/recording.rs::handle_delete_recording_job` | `extract_tag` | `production:1` |
| `src/mock/services/recording.rs::resp_recording_job_state` | `extract_tag` | `production:1` |
| `src/mock/services/recording.rs::resp_replay_uri` | `extract_tag` | `production:1` |

## Migration ownership

| Owner | Direct consumers / use | Required transitive review |
| --- | --- | --- |
| W08 | `auth::validate_ws_security`: Username/Password/Nonce/Created scalar extraction | AuthResponder, exemption selector and Device user mutations; migrate within auth boundary, never by body-wide lookup |
| W19 | Legacy `canon::canonicalize` key DOM plus `request::recording_equivalent` secondary replay check | Fixture keys, masking, recording, adapter and invalidation remain separate from synthetic routing/fault policy; exact raw replay retained, selected identity collisions contained without rewriting stored keys |
| W17 | `discovery_responder::probe_reply`: DOM | UDP Probe matching, QName scope and input bounds; not the SOAP synthetic entry point |
| W10 | Media profile scalar selectors and create fields; DeleteProfile strict reader; `bind_configuration`/`unbind_configuration`; encoder/source/audio write helpers | Media2 wrappers reuse Media1 helpers; replace fragment contracts with parsed values/subtrees, not decode-and-reinsert strings |
| W10 | Media OSD helpers, colour/position attributes and nested TextString; configuration option selectors | Rendering and typed parsers, list filtering, quota/state; `_force_use_extract_all` is not a routed behavior |
| W10 | `media2::apply_media2_configuration`: repeated subtrees, Type/Token scalars, then synthesized fragment | Calls Media1 binding helpers per entry; inspect all validation before any writes and hook/invalidation behavior |
| W11 | PTZ selector scalars, nested operation/config/tour/space fragments and coordinate attributes | Profile-to-node resolution; `min_max`, range/vector readers, per-head slots and repeated tour spots |
| W12 | Imaging source selector and scalar setting reads | Source resolution and nested settings; current parse-failure/default behavior is not the target contract |
| W13 | Device scalar fields, repeated scopes/users/IP entries, storage/network/relay attributes/subtrees | Multi-entry validation, auth state, IO queue, change hooks; audit every field rather than replacing the helper mechanically |
| W14 | Recording configuration/source/job subtrees and token scalars | Track/job lifetime, counters, deletion dependencies; separate Search/Replay dispatch ownership |
| W15 | Event TopicExpression scalar extraction | Namespace context, dialect/filter state, subscription lifetime and queue semantics |
| W04/W23 | Test-only strict/legacy/discovery readers | Keep lexical helper unit tests distinct from independent protocol assertions; do not use legacy extraction to prove corrected wire output |

The work packages above own all indexed entries, including the non-routed
synthetic helper touch. Per-field meanings and every transitive wrapper remain
W01/W02 work; the index must not be marked as a finished migration.

## New findings

The follow-up [pipeline preflight](mock-fidelity-pipeline-preflight.md) records
selected transitive paths and K17's pre-success replay family invalidation,
owned by W19/W03/W18. It was reproduced and the built-in CreateProfile/DeleteProfile paths
now use explicit committed effects; other mutation/dependency paths remain open. It also
traces the K15 decode/render dependency; output-only escaping is not a complete
fix while legacy input readers store raw entity spelling; profile Names now pair both fixes.
K18 in that preflight also records missing profile read/write invalidation edges
and service identity in replay family keys. The built-in create/list mismatch is repaired;
selected creation/deletion/binding/profile-read edges are tested across services and transports,
while the complete dependency graph remains open.

| ID | Evidence | Disposition |
| --- | --- | --- |
| K27 — substitution contained, index still collides | `mock_replay_key_gaps` retains the six original stored-key collisions and adds body ephemera, mixed-content ordering and xsi:type namespace controls. Replay now rejects unconfirmed scoped identity matches and falls through to synthetic. | W19 partial: both transports and exact raw/qualified-header controls pass; index/file shape unchanged, overwritten recordings unrecoverable, full key migration and other QName/HTTP/protocol semantics remain open. |
| K28 | Recorded request URL credentials survived in `key_canon`; old persisted keys were loaded unchanged. Synthetic assertion failures reproduced both defects. | W19: scrub projected URL pairs for recording/replay/diff and legacy loaded/caller lookup keys; loading does not rewrite disk, normalization collisions remain last-write-wins. Raw-envelope redaction is still targeted, not a general secret detector. |
| K24 | Pinned output-type review finds Media2 audio renderers/options reuse Media1 codec labels, and shared writes store those labels without a service adapter. XSD string validity does not verify the different codec vocabulary. | W01/W10: review audio list/options, profile-inlined audio, shared writes and client cross-service expectations as one dependency set. Source-confirmed; discriminating wire/state reproduction pending. Do not fix rendering alone and break writes. |
| K25 | The audio encoder write helper stores requested Multicast AutoStart, while Media2 metadata derives it from address presence; the mock does not implement persistent streaming. External output-type notes identify the field's read-only effect meaning. | W01/W10/W17: reproduce read/write/capability disagreement, review read-only treatment and absent/present multicast before changing defaults. Source-confirmed, no real RTP effect verified. |
| K26 | `apply_video_encoder_write` stores Encoding verbatim and both video renderers echo the shared label. A Media2-only codec may therefore enter the Media1 view without a representability policy. | W01/W10/W17: reproduce H265 write/profile/list/options combinations, review both service contracts and choose an explicit shared-state view/refusal policy; do not silently convert codec identity or weaken schemas. Source-confirmed risk, wire reproduction pending. |
| K23 | Windows CI run 34471659927 (`2a488be`) failed `line_number_override_is_validated_but_never_changes_agent_or_plain_output`: bytewise JSON comparison included independently measured `meta.elapsed_ms` (0 versus 9). | W22 test-harness repair: require numeric timing, omit only that exact field from JSON equality, keep all other fields, stderr and plain-output checks; add deterministic timing/data/type controls. Do not alter CLI output or hide other metadata. |
| K12 | Source comment at `media::bind_configuration` describes binding a fixed profile as a mock deviation. Official Media1/Media2 §4.1 distinguish deletion from configuration changes. | Correct the comment and preserve legal binding; do not “repair” it by making fixed profiles immutable. References below. |
| K13 — fixed after baseline | `create_profile_in_state` checks uniqueness and inserts under one write lock, skipping occupied generated tokens without overflowing the persisted counter. | Both-service collision regression, boundary/full-state controls and concurrent explicit/generated allocations cover this state slice; capacity and other CreateProfile semantics remain open. |
| K14 — fixed after baseline | DeleteProfile now uses an explicit committed-outcome predicate; NotFound/Fixed do not notify, Deleted notifies once. | W18 partial; regression checks full state, both services, hook count and public-helper compatibility. Replay remains separate K17. |
| K15 — Name repaired | Both CreateProfile Name readers now decode scoped scalar text; both profile renderers escape Name once, including seeded state. Profile-token attributes and nested configuration text remain open. | W10; corrected seeded-markup control and both-transport create/read/state/refusal tests. This does not close the token or nested-renderer audit. |
| K16 — partial repair | Media2 profile-list selection is repaired in `c6af85b`; create reads Name only; binding validates and commits one complete value-based plan. | Late-invalid-token partial writes and selected read semantics have state/hook/HTTP controls. Create/name/binding Type=All/conflicts and broader field/output validation remain W01/W10 work. |

K23 is now repaired in the test harness only. Deterministic controls distinguish
timing-only changes from data/command changes and reject invalid timing types.
Broadening the comparison mask and bypassing type validation made both new
controls fail in a full-workspace all-feature no-fail-fast mutation run; restored
formatting, both Clippy modes, 1,181 all-feature and 1,097 default tests passed
(5 ignored, 21 suites each). No CLI production output or public schema changed;
the next hosted run must still confirm the repaired gate.

K12 reference conclusions were checked against
[Media1 v24.12 §4.1](https://www.onvif.org/specs/2412/ONVIF-Media-Service-Spec-v2412.pdf)
and [Media2 v26.06 §4.1](https://www.onvif.org/specs/2606/ONVIF-Media2-Service-Spec-v2606.pdf)
on 2026-09-10. No schema tables or copies are included here. These references do
not constitute an external hash-pinned schema acceptance run.

Initial checkpoint results: K12's comment is corrected; the fixed-profile Add/Remove
control passes in both services. `tests/mock_fidelity_known_gaps.rs` reproduces
K13 collision in both services, K14 failed-delete notification in both services,
K15 Name-as-markup in both profile renderers, and K16 partial Media2 binding after
a late invalid configuration token. At that checkpoint K13's concurrent race and
K16's other selector/create/name semantics were unverified. All four gap assertions were
perturbed and failed at the intended payload/state assertions, then restored.
The passing baseline is deliberately a record of defects, not four fixes.

K16's binding subcase now uses one locked validation/commit across all requested
slots; the old synthesized XML path has been removed. The original implementation
failed the replacement full-state regression. New in-process/HTTP controls cover
late unknown, wrong-family and empty tokens, complete success visible in both
Media views, fixed-profile changes, repeated removals and one callback observing
both committed slots. Missing mandatory values are checked before state lookup;
this changes error precedence for requests containing multiple distinct errors.
Other K16 semantics remain open; this is not full binding conformance acceptance.

K13 now has a corrected both-service collision regression and private helper
controls for the counter boundary, duplicate full-state/hook preservation and
concurrent generated/explicit allocations. The duplicate check and insertion
share the write lock; only a Created outcome notifies. The persisted u32 field
remains compatible as a wrapping search hint. This does not enforce the advertised
profile capacity or repair parsing, rendering, initial bindings or replay.

Subsequent K14 repair replaces that known-gap expectation with
`rejected_delete_preserves_state_and_hook_but_success_notifies`. The original
implementation failed the refusal control; suppressing all notifications then
failed the successful-delete count in a full all-feature no-fail-fast run.
Restored behavior keeps the public `modify_returning` notification contract.
This conditional notification helper does not implement rollback or change the
callback lock/reentrancy policy at that checkpoint. The subsequent
[state hook snapshot work](mock-fidelity-pipeline-preflight.md#state-hook-snapshot-work)
repairs the selected lock/snapshot defects; other W18/W19 boundaries remain open.

## Reproduction and handoff

```powershell
rtk powershell -NoProfile -File docs/active/check-mock-fidelity-inventory.ps1 -SelfTest
rtk rg -n 'const ACTION|\.call\(' src/client src/session.rs
rtk rg -n 'extract_tag|extract_all_tags|extract_attr|required_text|XmlNode::parse' src/mock
```

The checker now verifies both this source index and the operation ledger.
Its extra controls cover changed/missing/duplicate entries, unsupported Action
expressions, unknown Action families and extra URI segments; reader controls
distinguish production from test calls and preserve occurrence counts.

Next: complete W01 profile/binding cards and transitive W02 paths; reproduce
K13–K16 without real-camera writes, then satisfy W03–W06 before broad handler
migration. No change to runtime Action acceptance is claimed in this audit.

Verification at this checkpoint (Windows, isolated `target/mock-fidelity-build`):
formatting and both workspace/all-target Clippy configurations passed;
workspace all-features tests: 1,146 passed, 4 ignored; workspace default tests:
1,066 passed, 4 ignored. Totals include the four deliberately passing known-gap
reproductions, not four corrected invariants. The checker passed its original
ten and additional six rejection controls; 264 local links/anchors across ten
planning documents resolved. No external schema gate, other native OS run,
real-camera command, publication, merge or installed-binary update occurred.

Handoff: W00 DONE for literal source reconciliation; W01 IN-PROGRESS and W02
PARTIAL. The first 13 cards now exist and K13–K16 have the reproductions above;
next work is their remaining contract/design readiness, not repeating discovery.
