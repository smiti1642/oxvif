# oxvif

Async Rust client library for the [ONVIF](https://www.onvif.org/) IP camera protocol.

```
UDP multicast ──► discovery::probe() ──► Vec<DiscoveredDevice>
                                                  │
                                                  ▼ XAddr
SOAP/HTTP ──────► OnvifClient ──► Device  (capabilities, hostname, NTP, reboot)
                             ──► Media1   (profiles, RTSP/snapshot URIs, encoder configs)
                             ──► Media2   (H.265 native, flat encoder config)
                             ──► PTZ      (move, stop, presets, status)
                             ──► Imaging  (brightness, contrast, exposure, IR cut)
                             ──► Events   (subscribe, pull, renew, unsubscribe)
```

- Async-first (`tokio` + `reqwest`)
- WS-Security `UsernameToken` with `PasswordDigest` (ONVIF Profile S §5.12)
- WS-Discovery via UDP multicast (`239.255.255.250:3702`)
- Mockable transport — unit-test without a real camera
- No unsafe code; pure Rust XML parsing via `quick-xml`
- 181 unit tests + 9 doc tests

---

## Quick start

```rust
use oxvif::{OnvifClient, OnvifError};

#[tokio::main]
async fn main() -> Result<(), OnvifError> {
    let client = OnvifClient::new("http://192.168.1.100/onvif/device_service")
        .with_credentials("admin", "password");

    // 1. Discover service URLs
    let caps = client.get_capabilities().await?;
    let media_url = caps.media.url.as_deref().unwrap();

    // 2. List media profiles
    let profiles = client.get_profiles(media_url).await?;

    // 3. Get RTSP URI for the first profile
    let uri = client.get_stream_uri(media_url, &profiles[0].token).await?;
    println!("RTSP: {}", uri.uri);

    Ok(())
}
```

---

## Installation

```toml
[dependencies]
oxvif = { path = "." }   # local path until published to crates.io
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

---

## `OnvifClient`

The main entry point. Stateless and cheaply cloneable — safe to wrap in `Arc` and share across threads.

### Constructors and builder methods

| Method | Description |
|--------|-------------|
| `OnvifClient::new(device_url)` | Connect to device at `device_url` (e.g. `http://192.168.1.100/onvif/device_service`) |
| `.with_credentials(username, password)` | Enable WS-Security `UsernameToken` authentication |
| `.with_utc_offset(offset_secs: i64)` | Adjust WS-Security timestamp if device clock differs from local UTC |
| `.with_transport(Arc<dyn Transport>)` | Replace the default HTTP transport (used for unit testing) |

```rust
// Sync device clock before sending authenticated requests
let client = OnvifClient::new("http://192.168.1.100/onvif/device_service");
let dt = client.get_system_date_and_time().await?;
let client = client
    .with_credentials("admin", "secret")
    .with_utc_offset(dt.utc_offset_secs());
```

---

## WS-Discovery

Find ONVIF cameras on your local network without knowing their IP addresses.

```rust
use std::time::Duration;
use oxvif::discovery;

let devices = discovery::probe(Duration::from_secs(3)).await;

for d in &devices {
    println!("Found: {}", d.endpoint);
    for addr in &d.xaddrs {
        println!("  XAddr: {addr}");          // use this as device_url
    }
    for scope in &d.scopes {
        println!("  Scope: {scope}");         // e.g. "onvif://www.onvif.org/name/Camera1"
    }
}
```

**`DiscoveredDevice` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `endpoint` | `String` | Unique endpoint URN (e.g. `uuid:...`) |
| `types` | `Vec<String>` | WS-Discovery types (e.g. `NetworkVideoTransmitter`) |
| `scopes` | `Vec<String>` | ONVIF scopes (name, location, hardware, etc.) |
| `xaddrs` | `Vec<String>` | Device service URLs — pass the first to `OnvifClient::new` |

`probe` returns an empty `Vec` on I/O errors; it never panics.

---

## Device Service methods

### `get_capabilities() -> Result<Capabilities, OnvifError>`

Retrieves all service endpoint URLs and feature flags. **Always call this first.**

```rust
let caps = client.get_capabilities().await?;

caps.device.url        // Device management service
caps.media.url         // Media service (profiles / stream URIs)
caps.ptz_url           // PTZ service
caps.events.url        // Events service
caps.imaging_url       // Imaging service
caps.analytics.url     // Analytics service
caps.media2_url        // Media2 service (None on many cameras — use GetServices)

caps.device.system.firmware_upgrade
caps.device.security.username_token
caps.media.streaming.rtp_rtsp_tcp
caps.events.ws_pull_point
```

### `get_services() -> Result<Vec<OnvifService>, OnvifError>`

Use as a fallback when `caps.media2_url` is `None`:

```rust
let caps = client.get_capabilities().await?;
let media2_url = caps.media2_url.clone().or_else(|| {
    client.get_services().await.ok()?
        .into_iter()
        .find(|s| s.is_media2())
        .map(|s| s.url)
});
```

### `get_system_date_and_time() -> Result<SystemDateTime, OnvifError>`

Retrieves the device clock. Compute the offset to keep WS-Security timestamps in sync.

```rust
let dt = client.get_system_date_and_time().await?;
let offset = dt.utc_offset_secs();   // device_utc − local_utc
```

### `get_device_info() -> Result<DeviceInfo, OnvifError>`

```rust
let info = client.get_device_info().await?;
// info.manufacturer, info.model, info.firmware_version, info.serial_number
```

### Hostname methods

| Method | Description |
|--------|-------------|
| `get_hostname()` | Returns `Hostname { from_dhcp: bool, name: Option<String> }` |
| `set_hostname(name: &str)` | Set a static hostname |

### NTP methods

| Method | Description |
|--------|-------------|
| `get_ntp()` | Returns `NtpInfo { from_dhcp: bool, servers: Vec<String> }` |
| `set_ntp(from_dhcp: bool, servers: &[&str])` | Configure NTP servers |

### `system_reboot() -> Result<String, OnvifError>`

Initiates a device reboot. Returns the device's informational message.

---

## Media Service (Media1) methods

All Media1 methods use `media_url` from `caps.media.url`.

### Profile management

| Method | Returns | Description |
|--------|---------|-------------|
| `get_profiles(media_url)` | `Vec<MediaProfile>` | List all profiles |
| `get_profile(media_url, token)` | `MediaProfile` | Get a single profile |
| `create_profile(media_url, name, token)` | `MediaProfile` | Create a new empty profile |
| `delete_profile(media_url, token)` | `()` | Delete a non-fixed profile |
| `add_video_encoder_configuration(media_url, profile_token, config_token)` | `()` | Bind encoder config to profile |
| `remove_video_encoder_configuration(media_url, profile_token)` | `()` | Unbind encoder config |
| `add_video_source_configuration(media_url, profile_token, config_token)` | `()` | Bind video source to profile |
| `remove_video_source_configuration(media_url, profile_token)` | `()` | Unbind video source |

### Streaming

```rust
let profiles = client.get_profiles(&media_url).await?;

let rtsp = client.get_stream_uri(&media_url, &profiles[0].token).await?;
println!("RTSP: {}", rtsp.uri);

let snap = client.get_snapshot_uri(&media_url, &profiles[0].token).await?;
println!("Snapshot: {}", snap.uri);
```

### Video source and encoder configurations

| Method | Description |
|--------|-------------|
| `get_video_sources(media_url)` | Physical video inputs |
| `get_video_source_configurations(media_url)` | Crop/position window configs |
| `get_video_source_configuration(media_url, token)` | Single VSC by token |
| `set_video_source_configuration(media_url, config)` | Write VSC back to device |
| `get_video_source_configuration_options(media_url, token)` | Valid bounds ranges |
| `get_video_encoder_configurations(media_url)` | Codec / resolution / bitrate configs |
| `get_video_encoder_configuration(media_url, token)` | Single VEC by token |
| `set_video_encoder_configuration(media_url, config)` | Write VEC back to device |
| `get_video_encoder_configuration_options(media_url, token)` | Valid resolution/bitrate/fps ranges |

```rust
let mut enc = client.get_video_encoder_configuration(media_url, &token).await?;
if let Some(rc) = enc.rate_control.as_mut() {
    rc.bitrate_limit = 2048;   // 2 Mbps
}
client.set_video_encoder_configuration(media_url, &enc).await?;
```

---

## Media2 methods

Media2 (`ver20/media/wsdl`) is the successor to Media1, with native H.265 support and a simplified encoder config structure. All Media2 methods use `media2_url`.

### Media1 vs Media2 key differences

| Feature | Media1 | Media2 |
|---------|--------|--------|
| H.265 | Via `Other(String)` | Native `VideoEncoding::H265` |
| Encoder config | Nested `H264`/`H265` sub-struct | Flat — `gov_length` and `profile` at top level |
| `GetStreamUri` response | `<MediaUri>` wrapper | Just `<Uri>` string |
| Write operations | Require `<ForcePersistence>true` | No `ForcePersistence` |

### Media2 method reference

| Method | Returns | Description |
|--------|---------|-------------|
| `get_profiles_media2(url)` | `Vec<MediaProfile2>` | List profiles |
| `get_stream_uri_media2(url, token)` | `String` | RTSP URI |
| `get_snapshot_uri_media2(url, token)` | `String` | HTTP snapshot URI |
| `get_video_source_configurations_media2(url)` | `Vec<VideoSourceConfiguration>` | |
| `set_video_source_configuration_media2(url, config)` | `()` | |
| `get_video_source_configuration_options_media2(url, token)` | `VideoSourceConfigurationOptions` | |
| `get_video_encoder_configurations_media2(url)` | `Vec<VideoEncoderConfiguration2>` | Flat H.265-capable config |
| `get_video_encoder_configuration_media2(url, token)` | `VideoEncoderConfiguration2` | |
| `set_video_encoder_configuration_media2(url, config)` | `()` | |
| `get_video_encoder_configuration_options_media2(url, token)` | `VideoEncoderConfigurationOptions2` | |
| `get_video_encoder_instances_media2(url, config_token)` | `VideoEncoderInstances` | Encoder capacity |
| `create_profile_media2(url, name)` | `String` | Create profile, returns new token |
| `delete_profile_media2(url, token)` | `()` | |

---

## PTZ methods

All PTZ methods use `ptz_url` from `caps.ptz_url`. Coordinates use the ONVIF normalised range: pan/tilt `[-1.0, 1.0]`, zoom `[0.0, 1.0]`.

| Method | Description |
|--------|-------------|
| `ptz_absolute_move(ptz_url, profile_token, pan, tilt, zoom)` | Move to an absolute position |
| `ptz_relative_move(ptz_url, profile_token, pan, tilt, zoom)` | Move by an offset |
| `ptz_continuous_move(ptz_url, profile_token, pan, tilt, zoom)` | Start continuous movement |
| `ptz_stop(ptz_url, profile_token)` | Stop all movement |
| `ptz_get_presets(ptz_url, profile_token)` | List all saved preset positions |
| `ptz_goto_preset(ptz_url, profile_token, preset_token)` | Move to a saved preset |
| `ptz_set_preset(ptz_url, profile_token, name, token)` | Save current position as preset |
| `ptz_remove_preset(ptz_url, profile_token, preset_token)` | Delete a preset |
| `ptz_get_status(ptz_url, profile_token)` | Current pan/tilt/zoom position and move state |

```rust
// Save current position
let token = client.ptz_set_preset(ptz_url, &profile, Some("Entrance"), None).await?;

// Query position
let status = client.ptz_get_status(ptz_url, &profile).await?;
println!("pan={:?} tilt={:?} zoom={:?} state={}",
    status.pan, status.tilt, status.zoom, status.pan_tilt_status);
```

**`PtzStatus` fields:** `pan`, `tilt`, `zoom` (`Option<f32>`), `pan_tilt_status`, `zoom_status` (`String` — `"IDLE"` or `"MOVING"`).

---

## Imaging Service methods

All imaging methods use `imaging_url` from `caps.imaging_url` and require a `video_source_token`.

| Method | Description |
|--------|-------------|
| `get_imaging_settings(imaging_url, source_token)` | Current brightness, contrast, IR cut, white balance, exposure |
| `set_imaging_settings(imaging_url, source_token, settings)` | Write modified settings back |
| `get_imaging_options(imaging_url, source_token)` | Valid ranges for each setting |

```rust
let mut s = client.get_imaging_settings(&imaging_url, &source_token).await?;
s.brightness = Some(70.0);
s.ir_cut_filter = Some("AUTO".into());
client.set_imaging_settings(&imaging_url, &source_token, &s).await?;
```

**`ImagingSettings` fields:** `brightness`, `color_saturation`, `contrast`, `sharpness` (`Option<f32>`), `ir_cut_filter`, `white_balance_mode`, `exposure_mode` (`Option<String>`).

---

## Events Service methods

ONVIF Events use a pull-point subscription model. All operations start with `events_url` from `caps.events.url`.

```rust
// 1. Discover available topics
let props = client.get_event_properties(&events_url).await?;
for topic in &props.topics {
    println!("Topic: {topic}");    // e.g. "VideoSource/MotionAlarm"
}

// 2. Subscribe
let sub = client.create_pull_point_subscription(
    &events_url,
    None,           // filter: None = all topics
    Some("PT60S"),  // expire after 60 seconds
).await?;
println!("Subscription URL: {}", sub.reference_url);

// 3. Poll for events
let msgs = client.pull_messages(&sub.reference_url, "PT5S", 50).await?;
for m in &msgs {
    println!("[{}] {} — data={:?}", m.utc_time, m.topic, m.data);
}

// 4. Extend subscription
let new_time = client.renew_subscription(&sub.reference_url, "PT60S").await?;

// 5. Cancel
client.unsubscribe(&sub.reference_url).await?;
```

**`PullPointSubscription` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `reference_url` | `String` | Endpoint for `pull_messages`, `renew_subscription`, `unsubscribe` |
| `termination_time` | `String` | ISO-8601 timestamp when the subscription expires |

**`NotificationMessage` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `topic` | `String` | Event topic path (e.g. `tns1:VideoSource/MotionAlarm`) |
| `utc_time` | `String` | Event timestamp from `Message/@UtcTime` |
| `source` | `HashMap<String, String>` | Source `SimpleItem` pairs (e.g. `VideoSourceToken = "VideoSource_1"`) |
| `data` | `HashMap<String, String>` | Data `SimpleItem` pairs (e.g. `IsMotion = "true"`) |

**`EventProperties` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `topics` | `Vec<String>` | Flattened topic paths (e.g. `"VideoSource/MotionAlarm"`, `"RuleEngine/Cell/Motion"`) |

---

## Error handling

All API methods return `Result<T, OnvifError>`:

```rust
pub enum OnvifError {
    Transport(TransportError),  // network / TLS / unexpected HTTP status
    Soap(SoapError),            // parse failure, missing field, or SOAP Fault
}
```

```rust
use oxvif::error::OnvifError;
use oxvif::soap::SoapError;
use oxvif::transport::TransportError;

match client.get_capabilities().await {
    Ok(caps) => { /* use caps */ }
    Err(OnvifError::Transport(TransportError::Http(e))) => eprintln!("Network: {e}"),
    Err(OnvifError::Transport(TransportError::HttpStatus { status, body })) => {
        eprintln!("HTTP {status}: {body}");
    }
    Err(OnvifError::Soap(SoapError::Fault { code, reason })) => {
        eprintln!("SOAP Fault [{code}]: {reason}");
    }
    Err(e) => eprintln!("Other: {e}"),
}
```

> HTTP 500 is treated as `Ok` so the SOAP layer can parse the `<s:Fault>` detail.

---

## Testing without a real camera

Implement the `Transport` trait to inject any response:

```rust
use oxvif::transport::{Transport, TransportError};
use async_trait::async_trait;
use std::sync::Arc;

struct MockTransport { xml: String }

#[async_trait]
impl Transport for MockTransport {
    async fn soap_post(&self, _url: &str, _action: &str, _body: String)
        -> Result<String, TransportError>
    {
        Ok(self.xml.clone())
    }
}

let client = OnvifClient::new("http://ignored")
    .with_transport(Arc::new(MockTransport { xml: MY_FIXTURE_XML.into() }));
```

```sh
cargo test
```

---

## Running the built-in examples

```sh
cp .env.example .env   # fill in ONVIF_URL, ONVIF_USERNAME, ONVIF_PASSWORD
```

```sh
cargo run -- full-workflow          # end-to-end: all implemented operations
cargo run -- device-info            # manufacturer, model, firmware
cargo run -- device-management      # hostname, NTP, GetServices
cargo run -- stream-uris            # tabular RTSP URI listing
cargo run -- snapshot-uris          # tabular HTTP snapshot URI listing
cargo run -- system-datetime        # device clock and UTC offset
cargo run -- ptz-presets            # list all PTZ presets
cargo run -- ptz-status             # current pan/tilt/zoom position
cargo run -- video-config           # video sources, encoder configs (Media1)
cargo run -- video-config-media2    # H.265 encoder configs (Media2)
cargo run -- imaging                # brightness, contrast, exposure settings
cargo run -- events                 # subscribe, pull, renew, unsubscribe
cargo run -- discovery              # WS-Discovery UDP multicast probe
cargo run -- error-handling         # typed error variant matching demo
```

---

## Project structure

```
src/
├── lib.rs               Public API surface and re-exports
├── client.rs            OnvifClient — all ONVIF operations
├── discovery.rs         WS-Discovery UDP multicast probe
├── error.rs             OnvifError unified error type
├── transport.rs         Transport trait + HttpTransport (reqwest + rustls)
├── soap/
│   ├── mod.rs
│   ├── envelope.rs      SOAP 1.2 envelope builder
│   ├── security.rs      WS-Security UsernameToken / PasswordDigest
│   ├── xml.rs           Namespace-stripping XML parser (XmlNode)
│   └── error.rs         SoapError
├── types/
│   ├── mod.rs           XML helper functions
│   ├── capabilities.rs  Capabilities, service sub-structs
│   ├── device.rs        DeviceInfo, SystemDateTime, Hostname, NtpInfo
│   ├── events.rs        PullPointSubscription, NotificationMessage, EventProperties
│   ├── imaging.rs       ImagingSettings, ImagingOptions
│   ├── media.rs         MediaProfile, StreamUri, SnapshotUri
│   ├── ptz.rs           PtzPreset, PtzStatus
│   └── video.rs         VideoSource, VideoEncoder configs and options
└── tests/
    ├── client_tests.rs  181 unit tests covering all client methods
    └── types_tests.rs   XML parsing unit tests
```

---

## Implemented ONVIF operations

### Device Service

| Operation | Status |
|-----------|--------|
| `GetCapabilities` | ✓ |
| `GetServices` | ✓ |
| `GetDeviceInformation` | ✓ |
| `GetSystemDateAndTime` | ✓ |
| `GetHostname` / `SetHostname` | ✓ |
| `GetNTP` / `SetNTP` | ✓ |
| `SystemReboot` | ✓ |

### Media Service (Media1)

| Operation | Status |
|-----------|--------|
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
| Audio source / encoder operations | — |

### Media2 Service

| Operation | Status |
|-----------|--------|
| `GetProfiles` | ✓ |
| `CreateProfile` / `DeleteProfile` | ✓ |
| `GetStreamUri` / `GetSnapshotUri` | ✓ |
| `GetVideoSourceConfigurations` / `SetVideoSourceConfiguration` | ✓ |
| `GetVideoSourceConfigurationOptions` | ✓ |
| `GetVideoEncoderConfigurations` / `GetVideoEncoderConfiguration` | ✓ |
| `SetVideoEncoderConfiguration` | ✓ |
| `GetVideoEncoderConfigurationOptions` | ✓ |
| `GetVideoEncoderInstances` | ✓ |

### PTZ Service

| Operation | Status |
|-----------|--------|
| `AbsoluteMove` / `RelativeMove` / `ContinuousMove` | ✓ |
| `Stop` | ✓ |
| `GetPresets` / `GotoPreset` | ✓ |
| `SetPreset` / `RemovePreset` | ✓ |
| `GetStatus` | ✓ |
| `GetConfigurations` / `SetConfiguration` / `GetConfigurationOptions` | — |

### Imaging Service

| Operation | Status |
|-----------|--------|
| `GetImagingSettings` / `SetImagingSettings` | ✓ |
| `GetOptions` | ✓ |
| `Move` (focus/iris) | — |

### Events Service

| Operation | Status |
|-----------|--------|
| `GetEventProperties` | ✓ |
| `CreatePullPointSubscription` | ✓ |
| `PullMessages` | ✓ |
| `Renew` | ✓ |
| `Unsubscribe` | ✓ |
| WS-BaseNotification push (Subscribe) | — |

### WS-Discovery

| Operation | Status |
|-----------|--------|
| UDP multicast `Probe` | ✓ |
| `Hello` / `Bye` passive listening | — |

---

## License

MIT
