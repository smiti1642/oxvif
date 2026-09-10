# Mock 可信度逐操作清冊

[English](mock-fidelity-operation-ledger.md) | [繁體中文](mock-fidelity-operation-ledger_zh.md)

程式碼基準：`b134f73`，2026-09-10。本文件是專案原始碼清冊，不是 ONVIF
schema 目錄，也不代表符合規格。它列出 **10** 個正式 sub-dispatcher 的全部
**157** 個字面值路由分支；此數量不是已完整驗證的操作數，也不是 ONVIF 定義的全部操作數。

先閱讀[施工檢查表](mock-fidelity-execution-checklist_zh.md)；
政策與歷史證據保留於[主計畫](mock-fidelity-hardening-plan_zh.md)。
修改路由前，先開啟其對應列及工作項目。

| 章節 | 用途 |
| --- | --- |
| [追蹤契約](#追蹤契約) | 欄位意義及完成證據 |
| [device](#device) | 38 個路由分支；W13 |
| [device_io](#device-io) | 1 個路由分支；W13 |
| [media](#media) | 32 個路由分支；W10 |
| [media2](#media2) | 26 個路由分支；W10 |
| [ptz](#ptz) | 27 個路由分支；W11 |
| [imaging](#imaging) | 8 個路由分支；W12 |
| [events](#events) | 8 個路由分支；W15 |
| [recording](#recording) | 11 個路由分支；W14 |
| [search](#search) | 4 個路由分支；W14 |
| [replay](#replay) | 2 個路由分支；W14 |
| [維護方式](#維護方式) | 程式碼差異及變更流程 |

## 追蹤契約

- ID 為 `dispatcher.operation`，不只使用 Action 最後一段。保留 Events 實際的
  `Request` 後綴，不與 Media 的同步點操作混淆。
- Handler 與 arguments 取自目前 dispatch expression，用於定位呼叫位置，
  **不是行為分類**。傳入 `state` 不證明寫入有效；未傳入 `body` 是請求驗證
  的待查目標，不代表操作沒有請求欄位。
- Work 指向施工檢查表中的編號工作項目。
- C＝操作契約審查；R＝請求解析；F＝一般 Fault 映射；
  B＝行為分類與狀態語意；V＝驗證證據。
- `TODO` 表示尚未依本計畫驗收，不表示沒有既有測試。`PARTIAL` 表示僅有具名
  子範圍的證據。`DONE` 必須附 commit 與確切測試名稱的證據紀錄。
  `NA:<record>` 必須有不適用理由；未支援操作仍需拒絕測試。
- 兩個 DeleteProfile 列刻意保留部分完成狀態：scalar identity 與不存在／固定 profile
  Fault 修正不等於完整路由、全部 Fault 路徑或整個操作已驗收。P1 記錄選定巢狀
  Fault 證據。E1 指主計畫第一批實作
  `b134f73`、`tests/mock_request_identity.rs` 與 `src/mock/request.rs` 的測試。
- 每列均繼承施工檢查表的 **C01–C12 全部稽核面向**及開工／結案條件。
  施工前填妥操作工作卡；不得將空白欄位解讀為不需要處理。依 D3，規範欄位表與
  schema 衍生 corpus 保留於外部，只連結去識別化結論。
- P1 指[第一批 13 張工作卡](mock-fidelity-profile-preflight_zh.md)；部分契約審閱及
  known-gap 重現不代表驗收通過。
- 下列服務實作路徑可直接開啟；以 Handler 欄的確切 symbol 搜尋定位，不固定易失效的行號。

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
| `device.SendAuxiliaryCommand` | `device::resp_send_auxiliary_command` | `` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetSystemLog` | `device::resp_system_log` | `` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetRelayOutputs` | `device::resp_relay_outputs` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetRelayOutputState` | `device::handle_set_relay_output_state` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetRelayOutputSettings` | `device::handle_set_relay_output_settings` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetSystemFactoryDefault` | `resp_empty` | `"tds", "SetSystemFactoryDefaultResponse"` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetStorageConfigurations` | `device::resp_storage_configurations` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetStorageConfiguration` | `device::handle_set_storage_configuration` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetSystemUris` | `device::resp_system_uris` | `base` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.StartFirmwareUpgrade` | `device::resp_start_firmware_upgrade` | `base` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.StartSystemRestore` | `device::resp_start_system_restore` | `base` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.GetDiscoveryMode` | `device::resp_discovery_mode` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SetDiscoveryMode` | `device::handle_set_discovery_mode` | `state, body` | W13 | TODO | TODO | TODO | TODO | TODO | - |
| `device.SystemReboot` | `device::resp_system_reboot` | `` | W13 | TODO | TODO | TODO | TODO | TODO | - |

## device-io

[dispatch_device_io](../../src/mock/dispatch.rs) · [services/device.rs](../../src/mock/services/device.rs)

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `device_io.GetDigitalInputs` | `device::resp_digital_inputs` | `state` | W13 | TODO | TODO | TODO | TODO | TODO | - |

## media

[dispatch_media](../../src/mock/dispatch.rs) · [services/media.rs](../../src/mock/services/media.rs)

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `media.GetServiceCapabilities` | `media::resp_service_capabilities` | `` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetProfiles` | `media::resp_profiles` | `state` | W10 | PARTIAL | TODO | TODO | TODO | TODO | P1 |
| `media.GetProfile` | `media::resp_profile` | `state, body` | W10 | PARTIAL | TODO | TODO | TODO | TODO | P1 |
| `media.CreateProfile` | `media::handle_create_profile` | `state, body` | W10 | PARTIAL | TODO | TODO | TODO | TODO | P1 |
| `media.DeleteProfile` | `media::handle_delete_profile` | `state, body, effect` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | E1,P1 |
| `media.GetStreamUri` | `media::resp_stream_uri` | `` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetSnapshotUri` | `media::resp_snapshot_uri` | `base` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetVideoSources` | `media::resp_video_sources` | `state` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetVideoSourceConfigurations` | `media::resp_video_source_configurations` | `state` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetVideoSourceConfiguration` | `media::resp_video_source_configuration` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.SetVideoSourceConfiguration` | `media::handle_set_video_source_configuration` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetVideoSourceConfigurationOptions` | `media::resp_video_source_configuration_options` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetVideoEncoderConfigurations` | `media::resp_video_encoder_configurations` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetVideoEncoderConfiguration` | `media::resp_video_encoder_configuration` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.SetVideoEncoderConfiguration` | `media::handle_set_video_encoder_configuration` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetVideoEncoderConfigurationOptions` | `media::resp_video_encoder_configuration_options` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.AddVideoEncoderConfiguration` | `media::handle_add_video_encoder_configuration` | `state, body` | W10 | PARTIAL | TODO | TODO | TODO | TODO | P1 |
| `media.RemoveVideoEncoderConfiguration` | `media::handle_remove_video_encoder_configuration` | `state, body` | W10 | PARTIAL | TODO | TODO | TODO | TODO | P1 |
| `media.AddVideoSourceConfiguration` | `media::handle_add_video_source_configuration` | `state, body` | W10 | PARTIAL | TODO | TODO | TODO | TODO | P1 |
| `media.RemoveVideoSourceConfiguration` | `media::handle_remove_video_source_configuration` | `state, body` | W10 | PARTIAL | TODO | TODO | TODO | TODO | P1 |
| `media.GetAudioSources` | `media::resp_audio_sources` | `state` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetAudioSourceConfigurations` | `media::resp_audio_source_configurations` | `state` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetAudioEncoderConfiguration` | `media::resp_audio_encoder_configuration` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetAudioEncoderConfigurations` | `media::resp_audio_encoder_configurations` | `state` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.SetAudioEncoderConfiguration` | `media::handle_set_audio_encoder_configuration` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetAudioEncoderConfigurationOptions` | `media::resp_audio_encoder_configuration_options` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetOSD` | `media::resp_osd` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetOSDs` | `media::resp_osds` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.SetOSD` | `media::handle_set_osd` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.CreateOSD` | `media::handle_create_osd` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.DeleteOSD` | `media::handle_delete_osd` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media.GetOSDOptions` | `media::resp_osd_options` | `` | W10 | TODO | TODO | TODO | TODO | TODO | - |

## media2

[dispatch_media2](../../src/mock/dispatch.rs) · [services/media2.rs](../../src/mock/services/media2.rs)

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `media2.GetServiceCapabilities` | `media2::resp_service_capabilities_media2` | `` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetProfiles` | `media2::resp_profiles_media2` | `state` | W10 | PARTIAL | TODO | TODO | TODO | TODO | P1 |
| `media2.CreateProfile` | `media2::handle_create_profile_media2` | `state, body` | W10 | PARTIAL | TODO | TODO | TODO | TODO | P1 |
| `media2.DeleteProfile` | `media2::handle_delete_profile_media2` | `state, body, effect` | W10 | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL | E1,P1 |
| `media2.AddConfiguration` | `media2::handle_add_configuration_media2` | `state, body` | W10 | PARTIAL | TODO | TODO | TODO | TODO | P1 |
| `media2.RemoveConfiguration` | `media2::handle_remove_configuration_media2` | `state, body` | W10 | PARTIAL | TODO | TODO | TODO | TODO | P1 |
| `media2.GetStreamUri` | `media2::resp_stream_uri_media2` | `` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetSnapshotUri` | `media2::resp_snapshot_uri_media2` | `base` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetVideoSourceConfigurations` | `media2::resp_video_source_configurations_media2` | `state` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.SetVideoSourceConfiguration` | `media2::handle_set_video_source_configuration_media2` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetVideoSourceConfigurationOptions` | `media2::resp_video_source_configuration_options_media2` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetVideoEncoderConfigurations` | `media2::resp_video_encoder_configurations` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.SetVideoEncoderConfiguration` | `media2::handle_set_video_encoder_configuration` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetVideoEncoderConfigurationOptions` | `media2::resp_video_encoder_configuration_options_media2` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetVideoEncoderInstances` | `media2::resp_video_encoder_instances` | `` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetMetadataConfigurations` | `media2::resp_metadata_configurations` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.SetMetadataConfiguration` | `media2::handle_set_metadata_configuration` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetMetadataConfigurationOptions` | `media2::resp_metadata_configuration_options` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetAudioSourceConfigurations` | `media2::resp_audio_source_configurations_media2` | `state` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetAudioEncoderConfigurations` | `media2::resp_audio_encoder_configurations_media2` | `state` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetAudioEncoderConfigurationOptions` | `media2::resp_audio_encoder_configuration_options_media2` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.SetAudioEncoderConfiguration` | `media2::handle_set_audio_encoder_configuration_media2` | `state, body` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetAudioOutputConfigurations` | `media2::resp_audio_output_configurations` | `state` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetAudioDecoderConfigurations` | `media2::resp_audio_decoder_configurations` | `state` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.GetVideoSourceModes` | `media2::resp_video_source_modes` | `` | W10 | TODO | TODO | TODO | TODO | TODO | - |
| `media2.SetVideoSourceMode` | `media2::resp_set_video_source_mode` | `` | W10 | TODO | TODO | TODO | TODO | TODO | - |

## ptz

[dispatch_ptz](../../src/mock/dispatch.rs) · [services/ptz.rs](../../src/mock/services/ptz.rs)

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `ptz.GetServiceCapabilities` | `ptz::resp_ptz_service_capabilities` | `` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetStatus` | `ptz::resp_ptz_status` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetPresets` | `ptz::resp_ptz_presets` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.SetPreset` | `ptz::handle_ptz_set_preset` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.RemovePreset` | `ptz::handle_ptz_remove_preset` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GotoPreset` | `ptz::handle_ptz_goto_preset` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.AbsoluteMove` | `ptz::handle_ptz_absolute_move` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.RelativeMove` | `ptz::handle_ptz_relative_move` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.ContinuousMove` | `ptz::handle_ptz_continuous_move` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.Stop` | `ptz::handle_ptz_stop` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GotoHomePosition` | `ptz::handle_ptz_goto_home_position` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.SetHomePosition` | `ptz::handle_ptz_set_home_position` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetNodes` | `ptz::resp_ptz_nodes` | `state` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetNode` | `ptz::resp_ptz_node` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetConfigurations` | `ptz::resp_ptz_configurations` | `state` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetCompatibleConfigurations` | `ptz::resp_ptz_compatible_configurations` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetConfiguration` | `ptz::resp_ptz_configuration` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.SetConfiguration` | `ptz::handle_ptz_set_configuration` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetConfigurationOptions` | `ptz::resp_ptz_configuration_options` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetPresetTours` | `ptz::resp_ptz_preset_tours` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetPresetTour` | `ptz::resp_ptz_preset_tour` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.GetPresetTourOptions` | `ptz::resp_ptz_preset_tour_options` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.CreatePresetTour` | `ptz::handle_ptz_create_preset_tour` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.ModifyPresetTour` | `ptz::handle_ptz_modify_preset_tour` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.OperatePresetTour` | `ptz::handle_ptz_operate_preset_tour` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.RemovePresetTour` | `ptz::handle_ptz_remove_preset_tour` | `state, body` | W11 | TODO | TODO | TODO | TODO | TODO | - |
| `ptz.SendAuxiliaryCommand` | `ptz::handle_ptz_send_auxiliary_command` | `body` | W11 | TODO | TODO | TODO | TODO | TODO | - |

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
| `events.PullMessagesRequest` | `events::resp_pull_messages` | `state` | W15 | TODO | TODO | TODO | TODO | TODO | - |
| `events.SubscribeRequest` | `events::resp_subscribe` | `base` | W15 | TODO | TODO | TODO | TODO | TODO | - |
| `events.RenewRequest` | `events::resp_renew` | `` | W15 | TODO | TODO | TODO | TODO | TODO | - |
| `events.UnsubscribeRequest` | `resp_empty` | `"wsnt", "UnsubscribeResponse"` | W15 | TODO | TODO | TODO | TODO | TODO | - |
| `events.SetSynchronizationPointRequest` | `resp_empty` | `"tev", "SetSynchronizationPointResponse"` | W15 | TODO | TODO | TODO | TODO | TODO | - |

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
| `recording.GetRecordingJobState` | `recording::resp_recording_job_state` | `state, body` | W14 | TODO | TODO | TODO | TODO | TODO | - |

## search

[dispatch_search](../../src/mock/dispatch.rs) · [services/recording.rs](../../src/mock/services/recording.rs)

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `search.GetServiceCapabilities` | `recording::resp_search_service_capabilities` | `` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `search.FindRecordings` | `recording::resp_find_recordings` | `` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `search.GetRecordingSearchResults` | `recording::resp_recording_search_results` | `state` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `search.EndSearch` | `recording::resp_end_search` | `` | W14 | TODO | TODO | TODO | TODO | TODO | - |

## replay

[dispatch_replay](../../src/mock/dispatch.rs) · [services/recording.rs](../../src/mock/services/recording.rs)

| ID | Handler | Arguments | Work | C | R | F | B | V | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `replay.GetServiceCapabilities` | `recording::resp_replay_service_capabilities` | `` | W14 | TODO | TODO | TODO | TODO | TODO | - |
| `replay.GetReplayUri` | `recording::resp_replay_uri` | `state, body` | W14 | TODO | TODO | TODO | TODO | TODO | - |

## 維護方式

於 repository 根目錄執行：

```powershell
rtk powershell -NoProfile -File docs/active/check-mock-fidelity-inventory.ps1 -SelfTest
```

唯讀檢查器比對路由 key、handler symbol、arguments 及雙語追蹤欄位，
拒絕重複／空清冊並偵測不支援的 dispatch 語法。記憶體內自我測試不修改原始碼，
也不使用攝影機。這是原始碼形狀檢查，不是 Rust AST、call graph 或規範驗證器。
它不檢查 handler 內部，也不證明 Action URI 與 body 一致；
相關工作分別列於 W00／W02／W07。W22 的原始碼清冊子項已接入
`.github/workflows/ci.yml`，在 Windows 與 Linux 使用 PowerShell 7 執行
`-SelfTest`；`package` 依賴此檢查成功。目前尚未觀察託管 runner 的執行結果。
外部 schema CI 與儲存庫必要狀態檢查設定仍是獨立且未完成的驗收工作。

W22 本機證據（2026-09-10）：PowerShell 7 執行與 CI 相同的命令，16 個拒絕
控制案例及實際原始碼核對均通過。暫時修改 `media.GetProfiles` 清冊的 handler
後，程序以 exit 1 結束並指出對應 route target 錯誤；還原後回到 exit 0。
另已在本機檢查 YAML 解析及 package 依賴。這不是託管 Linux 執行證據，也沒有
變更分支保護設定。

新增、移除或改接路由時，必須在同一變更更新兩份表格及對應操作工作卡。
發現新的 helper、欄位或錯誤路徑時，即使此檢查通過，也須擴充工作卡及 W02
來源對照。不得只為消除失敗而提高計數、標記 DONE 或移除操作列；
移除操作須保留理由及替代 ID。
