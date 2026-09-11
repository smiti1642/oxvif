# Mock 可信度原始碼盤點

[English](mock-fidelity-source-audit.md) | [繁體中文](mock-fidelity-source-audit_zh.md)

來源基準：`892aa94`，2026-09-10 盤點。工作：W00、W02。
這是實際量測的專案原始碼索引，不是 schema 目錄。
[施工檢查表](mock-fidelity-execution-checklist_zh.md) · [逐操作清冊](mock-fidelity-operation-ledger_zh.md)

| 章節 | 用途 |
| --- | --- |
| [結果與邊界](#結果與邊界) | 已完成與尚未完成的範圍 |
| [Action 宣告](#action-宣告) | 完整 URI、方法及路由對應 |
| [Reader 呼叫位置](#reader-呼叫位置) | 可重現的直接呼叫多重集合 |
| [遷移責任](#遷移責任) | 文字／子樹／attribute／共用消費端 |
| [新發現](#新發現) | 證據及待辦工作 |
| [重現與交接](#重現與交接) | 命令與下一項工作 |

## 結果與邊界

- `src/client/*.rs` 加上 `src/session.rs` 共 159 個字面值 `const ACTION`
  宣告，對應 157 個不同 Action URI，涵蓋全部 157 個 dispatch key。
  兩個 Media2 encoder 方法共用一個 URI；session `get_osd_options` 重複 client
  URI。沒有缺乏對應宣告的來源路由。
- Session 方法是直接 request 路徑，不只是 delegate。舊 dispatch test 註解稱它
  沒有宣告 Action，該敘述不正確。
- 五種 reader 拼法共 191 個直接呼叫：176 個位於頂層 test module 之前，15 個
  位於其中。前者分布於 56 個 enclosing symbol（AM1 重新量測），**不是** 56 個有缺陷的操作。
  其中包含 test-only `required_text`、canonicalization、discovery 及刻意未使用的
  helper touch，不是舊 parser 正式缺陷的數量。
- W00 已完成目前字面值形式的來源核對；K06 synthetic 別名路由已於
  [管線檢查點](mock-fidelity-pipeline-preflight_zh.md#完整-action-路由)修正；
  HTTP 擷取、body 一致性及 replay 仍為獨立工作。W02 已有這五種拼法的完整直接呼叫索引，但仍為
  PARTIAL：間接呼叫路徑與逐欄位分類尚未全部結案。
- 掃描器是原始碼形狀工具，不是 Rust AST parser。函式歸屬採前一個宣告；
  test scope 依目前頂層 test module 慣例。Import／alias、macro、跨行／generic
  invocation 變化、closure、自訂 reader 仍需人工檢查。新方法不一定使用名為
  ACTION 的常數，因此 request 呼叫位置也須核對。以 `rg` 交叉檢查，不把索引
  相等當成規範驗收。

## Action 宣告

下表精確字串來自 client／session 原始碼，不是官方 WSDL 表。
檢查器比對完整 URI **及**來源方法，再核對唯一 route key 集合與 dispatch。
不能因最後一段相同就忽略來源 URI 的變更；此檢查不會在 runtime dispatch 強制執行。

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

## Reader 呼叫位置

按 symbol／helper／scope 計數；移動呼叫或增加次數會改變多重集合。
不固定行號；開啟 site key 中的檔案並搜尋 symbol。排除 helper 定義及整行註解。
行內註解／字串範例及不支援的語法仍須人工檢查。

| Site | Reader | Scope:occurrences |
| --- | --- | --- |
| `src/mock/canon.rs::canonicalize` | `XmlNode::parse` | `production:1` |
| `src/mock/discovery_responder.rs::build_probe_match_round_trips_through_the_client_parser` | `XmlNode::parse` | `test:1` |
| `src/mock/discovery_responder.rs::probe_response` | `XmlNode::parse` | `production:1` |
| `src/mock/discovery_responder.rs::unicast_probe_round_trip` | `XmlNode::parse` | `test:1` |
| `src/mock/request.rs::namespace_attribute_values_are_normalized_once` | `required_text` | `test:1` |
| `src/mock/request.rs::read` | `required_text` | `test:1` |
| `src/mock/services/device.rs::handle_create_users` | `extract_all_tags` | `production:3` |
| `src/mock/services/device.rs::handle_create_users` | `extract_tag` | `production:1` |
| `src/mock/services/device.rs::handle_delete_users` | `extract_all_tags` | `production:1` |
| `src/mock/services/device.rs::handle_delete_users` | `extract_tag` | `production:1` |
| `src/mock/services/device.rs::handle_set_discovery_mode` | `extract_tag` | `production:1` |
| `src/mock/services/device.rs::handle_set_dns` | `extract_all_tags` | `production:1` |
| `src/mock/services/device.rs::handle_set_dns` | `extract_tag` | `production:1` |
| `src/mock/services/device.rs::handle_set_hostname` | `extract_tag` | `production:1` |
| `src/mock/services/device.rs::handle_set_network_default_gateway` | `extract_all_tags` | `production:1` |
| `src/mock/services/device.rs::handle_set_network_interfaces` | `extract_tag` | `production:6` |
| `src/mock/services/device.rs::handle_set_network_protocols` | `extract_all_tags` | `production:3` |
| `src/mock/services/device.rs::handle_set_ntp` | `extract_all_tags` | `production:1` |
| `src/mock/services/device.rs::handle_set_ntp` | `extract_tag` | `production:1` |
| `src/mock/services/device.rs::handle_set_relay_output_settings` | `extract_tag` | `production:4` |
| `src/mock/services/device.rs::handle_set_relay_output_state` | `extract_tag` | `production:2` |
| `src/mock/services/device.rs::handle_set_scopes` | `extract_all_tags` | `production:1` |
| `src/mock/services/device.rs::handle_set_storage_configuration` | `extract_attr` | `production:2` |
| `src/mock/services/device.rs::handle_set_storage_configuration` | `extract_tag` | `production:3` |
| `src/mock/services/device.rs::handle_set_system_date_and_time` | `extract_tag` | `production:2` |
| `src/mock/services/device.rs::handle_set_user` | `extract_tag` | `production:4` |
| `src/mock/services/events.rs::resp_create_pull_point_subscription` | `extract_tag` | `production:1` |
| `src/mock/services/imaging.rs::handle_set_imaging_settings` | `extract_tag` | `production:11` |
| `src/mock/services/imaging.rs::lookup` | `extract_tag` | `production:1` |
| `src/mock/services/media.rs::_force_use_extract_all` | `extract_all_tags` | `production:1` |
| `src/mock/services/media.rs::handle_create_osd` | `extract_tag` | `production:1` |
| `src/mock/services/media.rs::handle_delete_osd` | `extract_tag` | `production:2` |
| `src/mock/services/media.rs::handle_set_osd` | `extract_attr` | `production:1` |
| `src/mock/services/media.rs::handle_set_osd` | `extract_tag` | `production:1` |
| `src/mock/services/media.rs::parse_osd_color` | `extract_attr` | `production:4` |
| `src/mock/services/media.rs::parse_osd_color` | `extract_tag` | `production:2` |
| `src/mock/services/media.rs::parse_osd_payload` | `extract_attr` | `production:2` |
| `src/mock/services/media.rs::parse_osd_payload` | `extract_tag` | `production:11` |
| `src/mock/services/media.rs::resp_osd` | `extract_tag` | `production:2` |
| `src/mock/services/media.rs::resp_osds` | `extract_tag` | `production:2` |
| `src/mock/services/ptz.rs::apply_ptz_configuration` | `extract_attr` | `production:2` |
| `src/mock/services/ptz.rs::apply_ptz_configuration` | `extract_tag` | `production:11` |
| `src/mock/services/ptz.rs::handle_ptz_absolute_move` | `extract_attr` | `production:3` |
| `src/mock/services/ptz.rs::handle_ptz_absolute_move` | `extract_tag` | `production:1` |
| `src/mock/services/ptz.rs::handle_ptz_continuous_move` | `extract_attr` | `production:3` |
| `src/mock/services/ptz.rs::handle_ptz_continuous_move` | `extract_tag` | `production:1` |
| `src/mock/services/ptz.rs::handle_ptz_goto_preset` | `extract_tag` | `production:2` |
| `src/mock/services/ptz.rs::handle_ptz_modify_preset_tour` | `extract_all_tags` | `production:1` |
| `src/mock/services/ptz.rs::handle_ptz_modify_preset_tour` | `extract_attr` | `production:2` |
| `src/mock/services/ptz.rs::handle_ptz_modify_preset_tour` | `extract_tag` | `production:8` |
| `src/mock/services/ptz.rs::handle_ptz_operate_preset_tour` | `extract_tag` | `production:3` |
| `src/mock/services/ptz.rs::handle_ptz_relative_move` | `extract_attr` | `production:3` |
| `src/mock/services/ptz.rs::handle_ptz_relative_move` | `extract_tag` | `production:1` |
| `src/mock/services/ptz.rs::handle_ptz_remove_preset_tour` | `extract_tag` | `production:2` |
| `src/mock/services/ptz.rs::handle_ptz_remove_preset` | `extract_tag` | `production:2` |
| `src/mock/services/ptz.rs::handle_ptz_send_auxiliary_command` | `extract_tag` | `production:2` |
| `src/mock/services/ptz.rs::handle_ptz_set_preset` | `extract_tag` | `production:3` |
| `src/mock/services/ptz.rs::has_pan_tilt` | `extract_attr` | `production:2` |
| `src/mock/services/ptz.rs::min_max` | `extract_tag` | `production:2` |
| `src/mock/services/ptz.rs::pan_tilt_attrs` | `extract_attr` | `production:2` |
| `src/mock/services/ptz.rs::parse_limits` | `extract_tag` | `production:5` |
| `src/mock/services/ptz.rs::resp_ptz_configuration_options` | `extract_tag` | `production:1` |
| `src/mock/services/ptz.rs::resp_ptz_configuration` | `extract_tag` | `production:1` |
| `src/mock/services/ptz.rs::resp_ptz_node` | `extract_tag` | `production:1` |
| `src/mock/services/ptz.rs::resp_ptz_preset_tour` | `extract_tag` | `production:2` |
| `src/mock/services/recording.rs::handle_create_recording_job` | `extract_tag` | `production:5` |
| `src/mock/services/recording.rs::handle_create_recording` | `extract_tag` | `production:9` |
| `src/mock/services/recording.rs::handle_create_track` | `extract_tag` | `production:4` |
| `src/mock/services/recording.rs::handle_delete_recording_job` | `extract_tag` | `production:1` |
| `src/mock/services/recording.rs::handle_delete_recording` | `extract_tag` | `production:1` |
| `src/mock/services/recording.rs::handle_delete_track` | `extract_tag` | `production:2` |
| `src/mock/services/recording.rs::handle_set_recording_job_mode` | `extract_tag` | `production:2` |
| `src/mock/services/recording.rs::resp_recording_job_state` | `extract_tag` | `production:1` |
| `src/mock/services/recording.rs::resp_replay_uri` | `extract_tag` | `production:1` |
| `src/mock/xml_parse.rs::extract_all` | `extract_all_tags` | `test:1` |
| `src/mock/xml_parse.rs::extract_from_full_soap_security_header` | `extract_tag` | `test:4` |
| `src/mock/xml_parse.rs::extract_missing` | `extract_tag` | `test:1` |
| `src/mock/xml_parse.rs::extract_nested` | `extract_tag` | `test:1` |
| `src/mock/xml_parse.rs::extract_no_namespace` | `extract_tag` | `test:1` |
| `src/mock/xml_parse.rs::extract_nonce_with_encoding_type` | `extract_tag` | `test:1` |
| `src/mock/xml_parse.rs::extract_simple_tag` | `extract_tag` | `test:1` |
| `src/mock/xml_parse.rs::extract_tag_with_attributes` | `extract_tag` | `test:1` |

## 遷移責任

| 負責工作 | 直接消費端／用途 | 必須追蹤的間接路徑 |
| --- | --- | --- |
| W08 | `auth::validate_ws_security`：Username／Password／Nonce／Created scalar | AuthResponder、豁免 selector、Device users；於 auth 邊界遷移，不可使用整個 body 同名欄位搜尋 |
| W19 | Legacy `canon::canonicalize` key DOM，加上 `request::recording_equivalent` 第二層 replay 檢查 | Fixture key、masking、recording、adapter、invalidation 與 synthetic routing／fault policy 分離；保留 exact raw replay，不改 stored key 即限制選定身分碰撞 |
| W17 | `discovery_responder::probe_reply`：DOM | UDP Probe matching、QName scope、輸入限制；不是 SOAP synthetic 入口 |
| W10 | Media profile scalar selector／create、DeleteProfile strict reader、`bind_configuration`／`unbind_configuration`、scoped `video_source` subtree 契約及其餘 encoder／audio write helper | Media2 wrapper 共用 Media1 helper；以 parsed value／subtree 取代 fragment 契約，不做解碼再插回 XML |
| W10 | Media OSD helper、color／position attribute、巢狀 TextString、configuration options selector | Rendering／typed parser、list filter、quota／state；`_force_use_extract_all` 不是 routed behavior |
| W10 | `media2::configuration_plan`：create／add／remove 共用 scoped 重複 Type／Token reference | 完整 value plan、選填改名與引用計數原子提交；見 [組裝批次](mock-fidelity-profile-assembly_zh.md)。巢狀 configuration writer 仍屬後續工作。 |
| W11 | PTZ selector scalar、巢狀 operation／config／tour／space fragment、座標 attribute | Profile-to-node、`min_max`、range／vector reader、per-head slot、重複 tour spot |
| W12 | Imaging source selector、scalar settings | Source resolution、巢狀 settings；目前 parse failure／default 行為不是目標契約 |
| W13 | Device scalar、重複 scope／user／IP entry、storage／network／relay attribute／子樹 | 多筆驗證、auth state、IO queue、change hook；逐欄位稽核，不機械式更換 helper |
| W14 | Recording configuration／source／job 子樹與 token scalar | Track／job 生命週期、counter、刪除相依；Search／Replay dispatch 責任分開 |
| W15 | Event TopicExpression scalar | Namespace context、dialect／filter state、subscription 生命週期及 queue |
| W04／W23 | Test-only strict／legacy／discovery reader | 詞法 helper unit test 不等同獨立協定 assertion；不以舊擷取器證明修正後的 wire 正確 |

上述工作涵蓋全部索引項目，包含未路由的 synthetic helper touch；
逐欄位意義及所有間接 wrapper 仍屬 W01／W02，不得把索引完成標為遷移完成。

## 新發現

後續[管線開工核對](mock-fidelity-pipeline-preflight_zh.md) 記錄選定的間接路徑及
K17：成功前就使 replay family 失效，由 W19／W03／W18 負責。此問題已重現，內建
CreateProfile／DeleteProfile 路徑已改用明確的 committed effect；其他 mutation／相依路徑仍待完成。
亦追蹤 K15 的 decode／render 依賴；舊 input reader 仍儲存 raw entity 拼寫時，
只修改輸出 escaping 不是完整修正；profile Name 現已成對修正。
同文件 K18 另記錄 replay family key 缺少 profile 讀寫失效依賴與 service identity；
內建 create／list 不一致已修正，選定建立／刪除／binding／profile-read 依賴已有跨服務及 transport 測試；
完整相依圖仍待完成。

| ID | 證據 | 處置 |
| --- | --- | --- |
| K31–K33 — VS1 已修正選定路徑 | 舊碼重現 crop 導致 options 上限縮小及錯誤末欄位仍成功。整批擾動證實 source 選擇與拒絕後完整狀態斷言可辨識缺陷。 | W01／W10／W17–W19：完整 scoped candidate 原子提交、sensor-derived options、escaped identity 與成功後 replay 失效。兩種 transport、70 份外部 instance 及限制詳見 [VS1](mock-fidelity-video-source_zh.md)；scalar attribute、任意匯入 snapshot 與實體 routing 仍未完整建模。 |
| K29 — 共用 escaping 已修正 | `types::xml_escape` 處理 markup，卻在 attribute 中也直接輸出 CR／LF／tab；新增 wire／value assertion 與兩種 transport 的 profile Name 案例均在舊 helper 失敗。 | W03／W10／W06 共用表示子批次：輸出 numeric character reference，保留一般值的 borrowed 路徑與 literal Name state／read；raw renderer、token-reader 閉合、schema 欄位限制與 invalid XML character 仍為獨立項目。 |
| K30 — 空 profile 政策已修正 | 兩種 transport 均重現 client parse failure 後已提交空 token state。明確空值的 Create／read selector 現在於 effect 前以 Sender／mock:RequestPolicy 拒絕；含空 seed 的 list 回傳 Receiver，保留 snapshot 及有效個別讀取。 | W01／W10／W06 有界 mock 政策，不是規範上的 xs:string 限制。Raw／client state、hook、配置、replay 及獨立政策 Fault 控制記於 profile preflight；其他 seed 與欄位限制仍待處理。 |
| K27 — 已限制回覆替換，index 仍碰撞 | `mock_replay_key_gaps` 保留原本六種 stored-key collision，增加 body ephemera、mixed-content ordering 與 xsi:type namespace 控制；replay 現在拒絕未確認的 scoped identity match，轉入 synthetic。 | W19 部分完成：兩種 transport 及 exact raw／qualified-header 控制通過；index／檔案格式不變，被覆蓋錄製無法恢復；完整 key 遷移及其他 QName／HTTP／protocol 語意仍待完成。 |
| K28 | 已錄製 request 的 URL 憑證仍存於 `key_canon`，舊檔 key 載入時也未清除。虛構資料的 assertion failure 已重現兩項缺陷。 | W19：recording／replay／diff 投影與舊檔／caller lookup key 均清除 URL 帳密；載入不覆寫磁碟，正規化碰撞維持 last-write-wins。Raw envelope 仍僅針對指定格式去除憑證，並非通用秘密偵測器。 |
| K24 | Media2 音訊編碼名稱與 Media1 不同。 | AM1: G711/PCMU, AAC/MP4A-LATM; ONVIF G726 + bitrate. [AM1](mock-fidelity-audio-metadata_zh.md) |
| K25 | AutoStart 曾由位址推測或直接寫入。 | AM1: 唯讀輸入驗證後忽略；非串流預設 false。 |
| K26 — 選定 codec view 已修正 | VE1 涵蓋 H265 寫入／回讀及 Media1 頂層拒絕；兩種 profile renderer 共用相同可表示性防護。 | W01／W10／W17：保留 codec 身分，拒絕不相容 Media1 view；不靜默轉換或放寬 schema。詳見 VE1 證據及合成模型限制。 |
| K34 — rate 契約已修正 | 公開 client 重現 12.5 → 0；無效幀率文字亦變成零，共用整數 mock storage 掩蓋不一致。 | 已核准下一個 minor 的 `f32` 遷移、嚴格 present rate 解析、outbound／serde 有限值防護、共用 mock rate、Media1 view 拒絕及成功後 replay 失效。詳見 [VE1 證據](mock-fidelity-video-encoder_zh.md)。 |
| K35 — 選定 encoder 契約已修正 | 八項 VE1 路由已使用 scoped 完整 candidate／selector、共用 options／write 限制及 source-configuration capacity；移除 17 個 legacy reader 呼叫。 | [VE1 證據](mock-fidelity-video-encoder_zh.md)：原子拒絕、調整、轉義身分、option 回讀及 replay 相依性。串流、實體 routing 及全程式符合性不在此模型範圍。 |
| K36 / D4 | 2026-09-11 已核准 metadata 公開型別遷移。 | AM1: 完整 multicast 與 session timeout；修正序列化順序，明確記錄未建模欄位。 |
| K37 | AM1 外部 corpus 08 拒絕音訊 options 的多值 Items；client 原先僅讀取第一個 Items。 | AM1: 逐一輸出整數，讀取全部 Items；corpus 09 的 148 份 XML 通過 Xerces。 |
| K23 | Windows CI run 34471659927（`2a488be`）的 `line_number_override_is_validated_but_never_changes_agent_or_plain_output` 失敗：逐位元 JSON 比較包含各自量測的 `meta.elapsed_ms`（0 與 9）。 | W22 測試框架修正：要求耗時為數字，JSON 相等比較只排除該確切欄位；保留其他全部欄位、stderr 及純文字輸出檢查，加入確定性的耗時／資料／型別控制。不改 CLI 輸出，不遮蔽其他 metadata。 |
| K12 | `media::bind_configuration` 註解將 fixed profile 綁定描述為 mock 偏差；官方 Media1／Media2 §4.1 區分刪除限制與 configuration 變更。 | 修正註解並保留合法綁定，不可把 fixed profile 改成完全不可修改；參考資料如下。 |
| K13 — 基準後已修正 | `create_profile_in_state` 在同一 write lock 內檢查唯一性及新增，跳過已用的自動 token，並避免持久化 counter 溢位。 | 兩服務碰撞回歸、邊界／完整 state 控制及明確／自動 token 併發配置涵蓋此 state 批次；容量及其他 CreateProfile 語意仍待完成。 |
| K14 — 基準後已修正 | DeleteProfile 使用明確的 committed-outcome predicate；NotFound／Fixed 不通知，Deleted 通知一次。 | W18 部分完成；回歸檢查完整 state、兩服務、hook 次數及公開 helper 相容性。Replay 另由 K17 追蹤。 |
| K15 — Name 與選定 profile-token 路徑已修正 | 兩個 CreateProfile Name reader 解碼 scalar；Media1 Create／GetProfile 及六個 binding 入口採用 scoped profile 身分。兩個 profile renderer 將 Name 與 token attribute 轉義一次。 | W10；P2 workflow／refusal 控制及 PTZ1 涵蓋選定 token 路徑。K30 已完成空 token 政策；A1 涵蓋 typed adapter 身分與可表示移動參數。Recorded key、其他 adapter 選項及巢狀 configuration 文字仍未結案。 |
| K16 — 部分修正 | Media2 profile-list 選擇已於 `c6af85b` 修正；create 只讀 Name；binding 驗證並提交完整、以值表示的 plan。 | 後筆無效 token 部分寫入及選定讀取語意具 state／hook／HTTP 控制；create／name／binding Type=All／conflict 及廣泛欄位／輸出驗證仍屬 W01／W10。 |

K23 現僅修正測試框架。確定性控制可區分純耗時變動與資料／command 變動，
並拒絕錯誤耗時型別。擴大比較遮罩及繞過型別驗證後，完整 workspace、
all-feature、no-fail-fast 擾動執行中的兩項新控制均失敗；還原後格式、
兩種 Clippy、全功能 1,181 項與預設 1,097 項測試通過（各 5 ignored、
21 suites）。CLI production 輸出及公開 schema 均未變更；仍須由下一輪
託管 CI 確認 gate 已恢復。

K12 已於 2026-09-10 核對
[Media1 v24.12 §4.1](https://www.onvif.org/specs/2412/ONVIF-Media-Service-Spec-v2412.pdf) 與
[Media2 v26.06 §4.1](https://www.onvif.org/specs/2606/ONVIF-Media2-Service-Spec-v2606.pdf)。
不納入 schema 表或副本；這些參考資料不代表已執行外部固定 hash 的 schema 驗收。

初始檢查點結果：K12 註解已修正，兩種服務的 fixed profile Add／Remove 控制通過。
`tests/mock_fidelity_known_gaps.rs` 重現兩服務的 K13 token 碰撞、K14 拒絕刪除
仍 notify、兩種 profile renderer 的 K15 Name-as-markup，以及 K16 Media2
後筆 configuration token 無效時留下前筆 binding。當時 K13 併發競爭與 K16 其他
selector／create／name 語意尚未驗證。擾動四項 gap assertion，均在目標
payload／state assertion 失敗，還原後通過。通過的基準是缺陷紀錄，不是四項修復。

K16 binding 子案例現以單一 lock 驗證並提交全部請求 slot；原本合成 XML 的
路徑已移除。原實作在替換後的完整 state 回歸失敗。新增 in-process／HTTP
控制涵蓋後筆未知、錯誤 family 及空 token、兩 Media view 可見的完整成功結果、
fixed profile 變更、重複移除，以及單次 callback 觀察到兩個已提交 slot。
必要值缺漏會先於 state lookup 檢查，因此同時具有多種錯誤的請求，其錯誤優先
順序會改變。其他 K16 語意仍待處理；這不是完整 binding 符合性驗收。

K13 現已有兩服務的正確碰撞回歸，以及 counter 邊界、重複請求完整 state／hook
保留、明確／自動 token 併發配置的私有 helper 控制。重複檢查及新增共用 write
lock；僅 Created 結果通知。持久化 u32 欄位維持相容，作為可環回的搜尋起點。
這不代表已強制公告的 profile 容量，亦未修正解析、輸出、初始 binding 或 replay。

後續 K14 修正已將該缺陷預期替換為
`rejected_delete_preserves_state_and_hook_but_success_notifies`。原實作在拒絕控制
失敗；停用全部通知後，完整全部功能 no-fail-fast 執行則在成功刪除次數斷言失敗。
還原後保留公開 `modify_returning` 的通知契約。此條件式通知 helper 不提供 rollback，
在該檢查點亦未更動 callback lock／reentrancy policy。後續
[state hook 快照工作](mock-fidelity-pipeline-preflight_zh.md#state-hook-快照工作)
修正選定鎖定／快照缺陷，其他 W18／W19 邊界仍待完成。

## 重現與交接

```powershell
rtk powershell -NoProfile -File docs/active/check-mock-fidelity-inventory.ps1 -SelfTest
rtk rg -n 'const ACTION|\.call\(' src/client src/session.rs
rtk rg -n 'extract_tag|extract_all_tags|extract_attr|required_text|XmlNode::parse' src/mock
```

檢查器現在同時核對本來源索引及逐操作清冊。新增控制涵蓋變更／缺少／重複項目、
不支援的 Action expression、未知 Action family、多餘 URI segment；
reader 控制區分 production／test 並保留呼叫次數。

下一步：完成 W01 profile／binding 工作卡與 W02 間接路徑；不寫入實機地重現
K13–K16，滿足 W03–W06 後才廣泛遷移 handler。本盤點不宣稱已改變 runtime
Action 接受行為。

本檢查點驗證（Windows，隔離 `target/mock-fidelity-build`）：formatting 與兩種
workspace／all-target Clippy 通過；workspace all-features 1,146 passed／4 ignored，
workspace default 1,066 passed／4 ignored。總數包含四項刻意通過的 known-gap
重現，不是四項已修正 invariant。檢查器原有十項與新增六項拒絕控制通過；十份規劃
文件的 264 個本機連結／anchor 可解析。未執行外部 schema gate、其他 OS 原生
測試、實機命令、發布、合併或更新已安裝 binary。

交接：W00 字面值來源核對 DONE；W01 IN-PROGRESS、W02 PARTIAL。
第一批 13 張工作卡已建立，K13–K16 已有上述重現；下一工作是剩餘契約／設計
開工條件，不需重複 discovery。
