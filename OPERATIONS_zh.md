# 已實作的 ONVIF 操作

[English](OPERATIONS.md) | **繁體中文**

本文件依服務列出 [oxvif](README_zh.md) 已實作的 ONVIF 操作。方法簽章與使用範例請參閱[函式庫與功能指南](LIBRARY_GUIDE_zh.md)及 [docs.rs](https://docs.rs/oxvif)。

下表列出的每一項操作均已實作（`✓`）。這些表格表示 **oxvif 的涵蓋範圍，而非完整的 ONVIF 操作介面**；未出現在本文件中的操作即尚未實作。若要查閱各項服務於通訊協定層定義的完整操作目錄，請參閱 [`docs/reference/`](https://github.com/smiti1642/oxvif/tree/master/docs/reference)。此處使用絕對連結，是因為發布至 registry 的 crate 不包含 `docs/` 目錄，使用相對連結將無法由套件內容存取。

## 服務索引

| 服務 | 涵蓋範圍表格 |
| --- | --- |
| Device | [Device Service](#device-service) |
| Device I/O | [DeviceIO Service](#deviceio-service) |
| Media1 | [Media Service](#media-servicemedia1) |
| Media2 | [Media2 Service](#media2-service) |
| PTZ | [PTZ Service](#ptz-service) |
| Imaging | [Imaging Service](#imaging-service) |
| Events | [Events Service](#events-service) |
| Recording | [Recording Service](#recording-service) |
| Search | [Search Service](#search-service) |
| Replay | [Replay Service](#replay-service) |
| Discovery | [WS-Discovery](#ws-discovery) |

### Device Service

| 操作 | 狀態 |
|-----------|--------|
| `GetCapabilities` | ✓ |
| `GetServices` | ✓ |
| `GetServiceCapabilities` | ✓ |
| `GetDeviceInformation` | ✓ |
| `GetSystemDateAndTime` / `SetSystemDateAndTime` | ✓ |
| `GetHostname` / `SetHostname` | ✓ |
| `GetNTP` / `SetNTP` | ✓ |
| `SystemReboot` | ✓ |
| `GetScopes` / `SetScopes` | ✓ |
| `GetUsers` / `CreateUsers` / `DeleteUsers` / `SetUser` | ✓ |
| `GetNetworkInterfaces` / `SetNetworkInterfaces` | ✓ |
| `GetNetworkProtocols` / `SetNetworkProtocols` | ✓ |
| `GetDNS` / `SetDNS` | ✓ |
| `GetNetworkDefaultGateway` / `SetNetworkDefaultGateway` | ✓ |
| `GetDiscoveryMode` / `SetDiscoveryMode` | ✓ |
| `SendAuxiliaryCommand` | ✓ |
| `GetSystemLog` | ✓ |
| `GetSystemUris` | ✓ |
| `SetSystemFactoryDefault` | ✓ |
| `StartFirmwareUpgrade` | ✓ |
| `StartSystemRestore` | ✓ |
| `GetRelayOutputs` / `SetRelayOutputState` / `SetRelayOutputSettings` | ✓ |
| `GetStorageConfigurations` / `SetStorageConfiguration` | ✓ |

`GetRelayOutputs`、`SetRelayOutputState` 與 `SetRelayOutputSettings` 刻意列於此處，而非 DeviceIO。`deviceio.wsdl` 使用 **device service** 的 element 定義這三項 message，並將其繫結至兩種 portType，因此將請求傳送至 device endpoint 符合規範。`GetDigitalInputs` 的情況不同，詳見下一節。

### DeviceIO Service

| 操作 | 狀態 |
|-----------|--------|
| `GetDigitalInputs` | ✓ |

此操作使用獨立 endpoint；該 endpoint 可由 `Capabilities.device_io` 或 `OnvifService::is_device_io()` 探索，並傳入 `get_digital_inputs`。直到 0.15.0 前，本文件都未列出這項操作；然而 crate 自 0.9.9 起即已實作，但當時是透過未宣告該操作的 device service 呼叫。

### Media Service（Media1）

| 操作 | 狀態 |
|-----------|--------|
| `GetServiceCapabilities` | ✓ |
| `GetProfiles` / `GetProfile` | ✓ |
| `CreateProfile` / `DeleteProfile` | ✓ |
| `AddVideoEncoderConfiguration` / `RemoveVideoEncoderConfiguration` | ✓ |
| `AddVideoSourceConfiguration` / `RemoveVideoSourceConfiguration` | ✓ |
| `GetStreamUri` | ✓ |
| `GetSnapshotUri` | ✓ |
| `GetVideoSources` | ✓ |
| `GetVideoSourceConfigurations` / `GetVideoSourceConfiguration` | ✓ |
| `SetVideoSourceConfiguration` | ✓ |
| `GetVideoSourceConfigurationOptions` | ✓ |
| `GetVideoEncoderConfigurations` / `GetVideoEncoderConfiguration` | ✓ |
| `SetVideoEncoderConfiguration` | ✓ |
| `GetVideoEncoderConfigurationOptions` | ✓ |
| `GetAudioSources` | ✓ |
| `GetAudioSourceConfigurations` | ✓ |
| `GetAudioEncoderConfigurations` / `GetAudioEncoderConfiguration` | ✓ |
| `SetAudioEncoderConfiguration` | ✓ |
| `GetAudioEncoderConfigurationOptions` | ✓ |
| `GetOSDs` / `GetOSD` | ✓ |
| `GetOSDOptions` | ✓ |
| `CreateOSD` / `SetOSD` / `DeleteOSD` | ✓ |

### Media2 Service

| 操作 | 狀態 |
|-----------|--------|
| `GetServiceCapabilities` | ✓ |
| `GetProfiles` | ✓ |
| `CreateProfile` / `DeleteProfile` | ✓ |
| `GetStreamUri` / `GetSnapshotUri` | ✓ |
| `GetVideoSourceConfigurations` / `SetVideoSourceConfiguration` | ✓ |
| `GetVideoSourceConfigurationOptions` | ✓ |
| `GetVideoEncoderConfigurations` / `GetVideoEncoderConfiguration` | ✓ |
| `SetVideoEncoderConfiguration` | ✓ |
| `GetVideoEncoderConfigurationOptions` | ✓ |
| `GetVideoEncoderInstances` | ✓ |
| `AddConfiguration` / `RemoveConfiguration` | ✓ |
| `GetMetadataConfigurations` / `SetMetadataConfiguration` | ✓ |
| `GetMetadataConfigurationOptions` | ✓ |
| `GetAudioSourceConfigurations` | ✓ |
| `GetAudioEncoderConfigurations` / `SetAudioEncoderConfiguration` | ✓ |
| `GetAudioEncoderConfigurationOptions` | ✓ |
| `GetAudioOutputConfigurations` | ✓ |
| `GetAudioDecoderConfigurations` | ✓ |
| `GetVideoSourceModes` / `SetVideoSourceMode` | ✓ |

### PTZ Service

| 操作 | 狀態 |
|-----------|--------|
| `GetServiceCapabilities` | ✓ |
| `AbsoluteMove` / `RelativeMove` / `ContinuousMove` | ✓ |
| `Stop` | ✓ |
| `GetPresets` / `GotoPreset` | ✓ |
| `SetPreset` / `RemovePreset` | ✓ |
| `GetStatus` | ✓ |
| `GetConfigurations` / `GetConfiguration` | ✓ |
| `SetConfiguration` / `GetConfigurationOptions` | ✓ |
| `GetNodes` / `GetNode` | ✓ |
| `GetCompatibleConfigurations` | ✓ |
| `GetPresetTours` / `GetPresetTour` | ✓ |
| `GetPresetTourOptions` | ✓ |
| `CreatePresetTour` / `ModifyPresetTour` | ✓ |
| `OperatePresetTour` / `RemovePresetTour` | ✓ |
| `SendAuxiliaryCommand` | ✓ |
| `GotoHomePosition` / `SetHomePosition` | ✓ |

### Imaging Service

| 操作 | 狀態 |
|-----------|--------|
| `GetServiceCapabilities` | ✓ |
| `GetImagingSettings` / `SetImagingSettings` | ✓ |
| `GetOptions` | ✓ |
| `Move` / `Stop` / `GetMoveOptions` / `GetStatus` | ✓ |

### Events Service

| 操作 | 狀態 |
|-----------|--------|
| `GetServiceCapabilities` | ✓ |
| `GetEventProperties` | ✓ |
| `CreatePullPointSubscription` | ✓ |
| `PullMessages` | ✓ |
| `Renew` | ✓ |
| `Unsubscribe` | ✓ |
| `event_stream`（持續輪詢 stream） | ✓ |
| WS-BaseNotification push（`subscribe` + `notification_listener`） | ✓ |
| `SetSynchronizationPoint` | ✓ |

### Recording Service

| 操作 | 狀態 |
|-----------|--------|
| `GetServiceCapabilities` | ✓ |
| `GetRecordings` | ✓ |
| `CreateRecording` / `DeleteRecording` | ✓ |
| `CreateTrack` / `DeleteTrack` | ✓ |
| `GetRecordingJobs` | ✓ |
| `CreateRecordingJob` / `SetRecordingJobMode` / `DeleteRecordingJob` | ✓ |
| `GetRecordingJobState` | ✓ |

### Search Service

| 操作 | 狀態 |
|-----------|--------|
| `GetServiceCapabilities` | ✓ |
| `FindRecordings` | ✓ |
| `GetRecordingSearchResults` | ✓ |
| `EndSearch` | ✓ |

### Replay Service

| 操作 | 狀態 |
|-----------|--------|
| `GetServiceCapabilities` | ✓ |
| `GetReplayUri` | ✓ |

### WS-Discovery

| 操作 | 狀態 |
|-----------|--------|
| UDP multicast `Probe` | ✓ |
| `Hello` / `Bye` 被動監聽（`listen`） | ✓ |
