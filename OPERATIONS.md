# Implemented ONVIF operations

Per-service coverage tables for [oxvif](README.md) — which ONVIF operations the
crate implements. Split out of `README.md` to keep the API guide readable; the
method signatures and usage examples for each operation stay there.

Every row below is implemented (`✓`). These tables are **oxvif's coverage, not
the full ONVIF surface** — an operation that does not appear here is not
implemented. For the protocol-side catalogue of every operation each service
defines, see
[`docs/reference/`](https://github.com/smiti1642/oxvif/tree/master/docs/reference)
— an absolute link because `docs/` is excluded from the published crate, so a
relative one is dead for anyone reading this from the registry tarball.

### Device Service

| Operation | Status |
|-----------|--------|
| `GetCapabilities` | ✓ |
| `GetServices` | ✓ |
| `GetServiceCapabilities` | ✓ |
| `GetDeviceInformation` | ✓ |
| `GetSystemDateAndTime` | ✓ |
| `GetHostname` / `SetHostname` | ✓ |
| `GetNTP` / `SetNTP` | ✓ |
| `SystemReboot` | ✓ |
| `GetScopes` | ✓ |
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

### Media Service (Media1)

| Operation | Status |
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

### Media2 Service

| Operation | Status |
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

| Operation | Status |
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

### Imaging Service

| Operation | Status |
|-----------|--------|
| `GetServiceCapabilities` | ✓ |
| `GetImagingSettings` / `SetImagingSettings` | ✓ |
| `GetOptions` | ✓ |
| `Move` / `Stop` / `GetMoveOptions` / `GetStatus` | ✓ |

### Events Service

| Operation | Status |
|-----------|--------|
| `GetServiceCapabilities` | ✓ |
| `GetEventProperties` | ✓ |
| `CreatePullPointSubscription` | ✓ |
| `PullMessages` | ✓ |
| `Renew` | ✓ |
| `Unsubscribe` | ✓ |
| `event_stream` (continuous poll stream) | ✓ |
| WS-BaseNotification push (`subscribe` + `notification_listener`) | ✓ |
| `SetSynchronizationPoint` | ✓ |

### Recording Service

| Operation | Status |
|-----------|--------|
| `GetServiceCapabilities` | ✓ |
| `GetRecordings` | ✓ |
| `CreateRecording` / `DeleteRecording` | ✓ |
| `CreateTrack` / `DeleteTrack` | ✓ |
| `GetRecordingJobs` | ✓ |
| `CreateRecordingJob` / `SetRecordingJobMode` / `DeleteRecordingJob` | ✓ |
| `GetRecordingJobState` | ✓ |

### Search Service

| Operation | Status |
|-----------|--------|
| `GetServiceCapabilities` | ✓ |
| `FindRecordings` | ✓ |
| `GetRecordingSearchResults` | ✓ |
| `EndSearch` | ✓ |

### Replay Service

| Operation | Status |
|-----------|--------|
| `GetServiceCapabilities` | ✓ |
| `GetReplayUri` | ✓ |

### WS-Discovery

| Operation | Status |
|-----------|--------|
| UDP multicast `Probe` | ✓ |
| `Hello` / `Bye` passive listening (`listen`) | ✓ |
