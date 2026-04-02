# oxvif Roadmap

This document tracks planned work for the oxvif library. Items are grouped by theme and roughly ordered by priority within each group.

---

## Short-term

### Audio support (Media1 / Media2)

The library currently covers all video-related operations but omits audio entirely.

- [ ] `GetAudioSources` — list physical audio inputs
- [ ] `GetAudioSourceConfigurations` / `GetAudioSourceConfiguration`
- [ ] `SetAudioSourceConfiguration`
- [ ] `GetAudioSourceConfigurationOptions`
- [ ] `GetAudioEncoderConfigurations` / `GetAudioEncoderConfiguration`
- [ ] `SetAudioEncoderConfiguration`
- [ ] `GetAudioEncoderConfigurationOptions`
- [ ] Media2 equivalents (`tr2:`)

### PTZ advanced configuration

Basic movement and presets are implemented. The configuration layer is missing.

- [ ] `GetConfigurations` — list all PTZ configs
- [ ] `GetConfiguration` — single config by token
- [ ] `SetConfiguration` — write back speed limits, zoom limits, etc.
- [ ] `GetConfigurationOptions` — valid ranges for config fields
- [ ] `GetNodes` — physical PTZ node description
- [ ] `GetCompatibleConfigurations` — valid configs for a given profile

### Imaging advanced controls

- [ ] `Move` — focus and iris motorised control
- [ ] `GetMoveOptions` — valid ranges for `Move`
- [ ] `GetStatus` — current focus/iris position
- [ ] `Stop` — halt ongoing focus/iris movement

---

## Medium-term

### WS-Discovery passive listening

The current `probe()` function is an active sender only.

- [ ] `Hello` listener — receive unsolicited announcements from cameras as they come online
- [ ] `Bye` listener — detect camera going offline
- [ ] Optional: maintain a live `DeviceRegistry` updated by Hello/Bye events

### Analytics Service

- [ ] `GetSupportedRules` — list supported rule types
- [ ] `GetRules` — list configured rules (motion zones, line crossing, etc.)
- [ ] `CreateRule` / `DeleteRule`
- [ ] `GetSupportedAnalyticsModules`
- [ ] `GetAnalyticsModules`
- [ ] `CreateAnalyticsModule` / `DeleteAnalyticsModule`

### Recording Service

- [ ] `GetRecordings` — list recordings with start/stop timestamps
- [ ] `GetRecordingInformation` — metadata for a single recording
- [ ] `CreateRecording` / `DeleteRecording`
- [ ] `CreateRecordingJob` / `DeleteRecordingJob`
- [ ] `GetRecordingJobs` / `GetRecordingJobState`

### Search Service

- [ ] `FindRecordings` — query recordings by time range and profile
- [ ] `GetRecordingSearchResults` — retrieve results from an ongoing search
- [ ] `FindEvents` — search the event log
- [ ] `GetEventSearchResults`

### Replay Service

- [ ] `GetReplayUri` — RTSP URI for playback of a stored recording
- [ ] `GetReplayConfiguration` / `SetReplayConfiguration`

---

## Long-term

### WS-BaseNotification push subscriptions

The Events service currently supports pull-point only. Some devices also support real-time push delivery.

- [ ] `Subscribe` (WS-BaseNotification) — register a push callback endpoint
- [ ] HTTP receiver for incoming `Notify` messages
- [ ] Tokio `mpsc` channel or async iterator API for event streaming

### DeviceIO Service

- [ ] `GetRelayOutputs` / `SetRelayOutputState` — control alarm relays
- [ ] `GetDigitalInputs` — read digital input states

### ONVIF Receiver Service

- [ ] `GetReceivers` — list configured RTSP input receivers
- [ ] `CreateReceiver` / `DeleteReceiver`
- [ ] `SetReceiver`

### TLS / HTTPS hardening

- [ ] Expose `reqwest::ClientBuilder` customisation (custom CA, client cert, `danger_accept_invalid_certs`)
- [ ] Document HTTPS camera setup
- [ ] CI smoke test against a self-signed camera

---

## Library / DX improvements

| Item | Notes |
|------|-------|
| Publish to crates.io | Choose final crate name, fill in metadata |
| `serde` feature flag | Opt-in `Serialize` / `Deserialize` on all public types |
| `tracing` integration | Instrument SOAP calls at `DEBUG` / `TRACE` level |
| Streaming event iterator | `async fn event_stream(...) -> impl Stream<Item = NotificationMessage>` via `tokio_stream` |
| Builder for `ImagingSettings` | Chainable setters to avoid cloning the full struct for one-field updates |
| `OnvifClient::from_discovered(device)` | Convenience constructor from `DiscoveredDevice` |
| Retry / timeout policy | Configurable per-request timeouts and automatic retry on transient errors |
| Benchmarks | `criterion` benchmarks for XML parsing hot paths |

---

## Completed

| Item | Commit |
|------|--------|
| Device: GetCapabilities, GetDeviceInformation, GetSystemDateAndTime, GetServices | `f0182f3` |
| Media1: GetProfiles, GetStreamUri, GetSnapshotUri | `f0182f3` |
| PTZ: AbsoluteMove, RelativeMove, ContinuousMove, Stop, GetPresets, GotoPreset | `4b8f23a` |
| Media1 + Media2: full video source and encoder configuration | `4b8f23a` |
| Media2: GetProfiles, GetStreamUri, GetSnapshotUri, GetVideoEncoderInstances, CreateProfile/DeleteProfile | `f0182f3` |
| GetServices with Media2 URL discovery fallback | `f0182f3` |
| Types split into `src/types/` module directory | `99d5407` |
| Unit tests moved to `src/tests/` | `451a103` |
| PTZ: SetPreset, RemovePreset, GetStatus | `a9f1456` |
| Media1: CreateProfile, DeleteProfile, GetProfile, Add/RemoveVideoEncoderConfiguration, Add/RemoveVideoSourceConfiguration | `a9f1456` |
| Device: GetHostname, SetHostname, GetNTP, SetNTP, SystemReboot | `a9f1456` |
| Imaging: GetImagingSettings, SetImagingSettings, GetOptions | `a9f1456` |
| WS-Discovery: UDP multicast Probe + DiscoveredDevice | `6da09a4` |
| Events: GetEventProperties, CreatePullPointSubscription, PullMessages, Renew, Unsubscribe | `6da09a4` |
