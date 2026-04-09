# oxvif Roadmap

This document tracks planned work for the oxvif library. Items are grouped by theme and roughly ordered by priority within each group.

---

## Short-term

### Analytics Service (Profile T §8.5/§8.6)

- [ ] `GetSupportedRules` — list supported rule types
- [ ] `GetRules` — list configured rules (motion zones, line crossing, etc.)
- [ ] `GetRuleOptions` — valid ranges for rule parameters
- [ ] `CreateRules` / `DeleteRules` / `ModifyRules`
- [ ] `GetSupportedAnalyticsModules`
- [ ] `GetAnalyticsModules`
- [ ] `CreateAnalyticsModules` / `DeleteAnalyticsModules`

### DeviceIO Service (Profile T §8.15/§8.16)

- [ ] `GetVideoSources` (DeviceIO variant)
- [ ] `GetAudioSources` / `GetAudioOutputs` (DeviceIO variant)
- [ ] `GetDigitalInputs` / `SetDigitalInputConfigurations`
- [ ] `GetRelayOutputs` / `SetRelayOutputState` (DeviceIO variant)
- [ ] `GetRelayOutputOptions`

### Audio decoder configuration options

- [ ] `GetAudioDecoderConfigurationOptions` (Media2)

---

## Medium-term

### Recording / Search advanced operations

- [ ] `SetRecordingConfiguration` — modify existing recording
- [ ] `SetTrackConfiguration` — modify track settings
- [ ] `GetRecordingOptions` — recording capacity limits
- [ ] `FindEvents` / `GetEventSearchResults` — event log search
- [ ] `FindPTZPosition` / `GetPTZPositionSearchResults` — PTZ position search
- [ ] Live-source job binding for recording jobs

### ONVIF Receiver Service

- [ ] `GetReceivers` — list configured RTSP input receivers
- [ ] `CreateReceiver` / `DeleteReceiver`
- [ ] `SetReceiver`

---

## Long-term

### TLS / HTTPS hardening

- [ ] Expose `reqwest::ClientBuilder` customisation (custom CA, client cert, `danger_accept_invalid_certs`)
- [ ] Document HTTPS camera setup

### Access Control / Door Control (Profile C/D)

- [ ] `GetAccessPointInfo` / `ExternalAuthorization`
- [ ] `GetDoorInfo` / `LockDoor` / `UnlockDoor`
- [ ] Credential / Schedule services

---

## Library / DX improvements

| Item | Notes |
|------|-------|
| `serde` feature flag | Opt-in `Serialize` / `Deserialize` on all public types |
| `tracing` integration | Instrument SOAP calls at `DEBUG` / `TRACE` level |
| Builder for `ImagingSettings` | Chainable setters to avoid cloning the full struct for one-field updates |
| `OnvifClient::from_discovered(device)` | Convenience constructor from `DiscoveredDevice` |
| Retry / timeout policy | Configurable per-request timeouts and automatic retry on transient errors |
| Benchmarks | `criterion` benchmarks for XML parsing hot paths |

---

## Completed

| Item | Version |
|------|---------|
| Device: capabilities, device info, date/time, services, hostname, NTP, reboot, scopes, users, network, DNS, gateway, relay, storage, system log/URIs, factory default, discovery mode | v0.1–v0.8 |
| Device: `SetNetworkDefaultGateway`, `SendAuxiliaryCommand` | develop |
| Media1: profiles, stream/snapshot URI, video/audio source + encoder configs, OSD | v0.1–v0.8 |
| Media2: profiles, stream/snapshot URI, video source/encoder configs + options + instances | v0.4–v0.8 |
| Media2: `AddConfiguration` / `RemoveConfiguration` (unified config binding) | develop |
| Media2: metadata configurations (Get/Set/Options) | develop |
| Media2: audio source/encoder/output/decoder configurations | develop |
| Media2: video source modes (Get/Set) | develop |
| PTZ: absolute/relative/continuous move, stop, presets, home, status, configs, nodes | v0.2–v0.8 |
| PTZ: `GetNode`, `GetCompatibleConfigurations` | develop |
| Imaging: settings, options, focus move/stop/status | v0.3–v0.8 |
| Events: pull-point subscription, poll, renew, unsubscribe, `event_stream` | v0.6–v0.8 |
| Events: WS-BaseNotification push (`subscribe` + `notification_listener`) | v0.8.5 |
| Events: `SetSynchronizationPoint` | develop |
| Recording: list, create/delete recordings + tracks + jobs, job state/mode | v0.8 |
| Search: find recordings, get results, end search, `search_recordings` | v0.8 |
| Replay: `GetReplayUri` | v0.8 |
| WS-Discovery: UDP multicast probe + passive Hello/Bye listening | v0.6–v0.8.5 |
| HTTP Digest Authentication (RFC 7616, Profile T §7.1) | develop |
| Published on crates.io | v0.1 |
