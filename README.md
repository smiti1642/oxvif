# oxvif

Async Rust client library for the [ONVIF](https://www.onvif.org/) IP camera protocol.

```
UDP multicast ──► discovery::probe() ──► Vec<DiscoveredDevice>
                                                  │
                                                  ▼ XAddr
                      OnvifSession ─── caches service URLs, delegates every call
                           │
SOAP/HTTP ──────►  OnvifClient ──► Device    (capabilities, hostname, NTP, reboot)
                           ──► Media1    (profiles, RTSP/snapshot URIs, video + audio configs)
                           ──► Media2    (H.265, metadata, audio, video source modes)
                           ──► PTZ       (move, stop, presets, home, status, configurations, nodes)
                           ──► Imaging   (brightness, contrast, exposure, IR cut, focus move/stop)
                           ──► OSD       (create, read, update, delete on-screen display elements)
                           ──► Events    (subscribe, pull, renew, unsubscribe, continuous stream)
                           ──► Recording (list, create/delete recordings and recording jobs)
                           ──► Search    (find recordings by time/scope)
                           ──► Replay    (RTSP URI for playback)
```

- Async-first (`tokio` + `reqwest`)
- WS-Security `UsernameToken` with `PasswordDigest` (ONVIF Profile S §5.12)
- HTTP Digest Authentication (RFC 7616, ONVIF Profile T §7.1)
- WS-Discovery via UDP multicast (`239.255.255.250:3702`)
- Mockable transport, plus a built-in mock ONVIF device (`mock` / `mock-server` features) — unit-test client code without a real camera
- No unsafe code; pure Rust XML parsing via `quick-xml`
- Optional, scriptable device health check with parse-coverage detection (`health` feature), plus a `conformance` example that validates the parsers against real cameras
- Hundreds of unit + doc tests, including the in-process mock device, the health checks, and scrubbed real-camera regression captures

---

## Quick start

Two ways to use oxvif — pick whichever suits your workflow.

### `OnvifSession` — URL caching handled for you

```rust
use oxvif::{OnvifSession, OnvifError};

#[tokio::main]
async fn main() -> Result<(), OnvifError> {
    let session = OnvifSession::builder("http://192.168.1.100/onvif/device_service")
        .with_credentials("admin", "password")
        .with_clock_sync()   // syncs WS-Security timestamp with device clock
        .build()
        .await?;

    let profiles = session.get_profiles().await?;
    let uri = session.get_stream_uri(&profiles[0].token).await?;
    println!("RTSP: {}", uri.uri);
    Ok(())
}
```

### `OnvifClient` — direct control, you manage service URLs

```rust
use oxvif::{OnvifClient, OnvifError};

#[tokio::main]
async fn main() -> Result<(), OnvifError> {
    let client = OnvifClient::new("http://192.168.1.100/onvif/device_service")
        .with_credentials("admin", "password");

    let caps = client.get_capabilities().await?;
    let media_url = caps.media.url.unwrap();

    let profiles = client.get_profiles(&media_url).await?;
    let uri = client.get_stream_uri(&media_url, &profiles[0].token).await?;
    println!("RTSP: {}", uri.uri);
    Ok(())
}
```

`OnvifSession` calls `GetCapabilities` once on `build()` and caches all service URLs — no URL arguments needed for individual methods. `OnvifClient` is stateless; you forward the URL yourself for full routing control.

### Testing — drive a mock device, no camera needed

Enable the `mock` feature and point a client at a built-in, stateful mock ONVIF
device — no network, no hardware. Ideal for unit tests.

```toml
[dev-dependencies]
oxvif = { version = "0.9.8", features = ["mock"] }
```

```rust
use std::sync::Arc;
use oxvif::{OnvifClient, mock::MockTransport};

#[tokio::test]
async fn talks_to_a_mock_camera() {
    let client = OnvifClient::new("http://mock")
        .with_transport(Arc::new(MockTransport::new()));

    client.set_hostname("lab-cam").await.unwrap();
    let h = client.get_hostname().await.unwrap();   // Set → Get round-trips
    assert_eq!(h.name.as_deref(), Some("lab-cam"));
}
```

Need a real bound port instead? The `mock-server` feature adds `MockServer::start()`.
See [Testing without a real camera](#testing-without-a-real-camera) for details.

---

## Installation

```toml
[dependencies]
oxvif = "0.9.8"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

---

## `OnvifSession`

`OnvifSession` calls `GetCapabilities` once at construction, caches all service
URLs internally, and exposes every operation as a one-liner — no URL parameters
needed anywhere.

### Building a session

```rust
use oxvif::{OnvifSession, OnvifError};

#[tokio::main]
async fn main() -> Result<(), OnvifError> {
    let session = OnvifSession::builder("http://192.168.1.100/onvif/device_service")
        .with_credentials("admin", "password")
        .with_clock_sync()   // syncs WS-Security timestamp with device clock
        .build()
        .await?;

    // Capabilities are already cached — no extra round-trip
    let caps = session.capabilities();

    let profiles = session.get_profiles().await?;
    let uri      = session.get_stream_uri(&profiles[0].token).await?;
    println!("RTSP: {}", uri.uri);

    let status = session.ptz_get_status(&profiles[0].token).await?;
    println!("Pan: {:?}  Tilt: {:?}", status.pan, status.tilt);

    Ok(())
}
```

### Builder methods

| Method | Description |
|--------|-------------|
| `OnvifSession::builder(device_url)` | Start building a session |
| `.with_credentials(username, password)` | Enable WS-Security `UsernameToken` authentication |
| `.with_clock_sync()` | Call `GetSystemDateAndTime` first and apply UTC offset — prevents auth failures on devices with clock skew |
| `.with_transport(transport)` | Replace HTTP transport (for unit testing) |
| `.build().await` | Connect, sync clock (if set), call `GetCapabilities`, return `OnvifSession` |

### Session accessors

| Method | Description |
|--------|-------------|
| `session.capabilities()` | Returns the cached `&Capabilities` — no network call |
| `session.client()` | Access the underlying `&OnvifClient` directly (e.g. for custom transport or fine-grained URL routing) |

`OnvifSession` delegates every `OnvifClient` method — the full method list is in the
sections below (Device, Media, PTZ, Imaging, OSD, Events, Recording, Search, Replay).

---

## `OnvifClient`

Stateless and cheaply cloneable — safe to wrap in `Arc` and share across threads.
You manage the service URLs (obtained from `get_capabilities()` or `get_services()`),
which gives you full control over per-call routing.

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

> **`OnvifSession` does this for you for Profile G.** Some cameras advertise the
> recording / search / replay services only via `GetServices`, not the
> `GetCapabilities` extension. `OnvifSession::build` fills any missing one from
> `GetServices` automatically, so `get_recordings` / `search_recordings` /
> `get_replay_uri` work on those devices without a manual fallback.

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

### `get_scopes() -> Result<Vec<String>, OnvifError>`

Returns the device's scope URIs — strings that describe the device's name,
location, hardware model, and capabilities
(e.g. `"onvif://www.onvif.org/name/Camera1"`). Completes Profile S coverage.

```rust
let scopes = client.get_scopes().await?;
for s in &scopes {
    println!("{s}");
}
```

### User management

| Method | Description |
|--------|-------------|
| `get_users()` | List all configured user accounts (usernames + access levels) |
| `create_users(users)` | Create accounts — `users` is `&[(&str, &str, &str)]` (username, password, level) |
| `delete_users(usernames)` | Delete accounts by username |
| `set_user(username, password, level)` | Modify an existing account; `password = None` leaves it unchanged |

### Network configuration

| Method | Description |
|--------|-------------|
| `get_network_interfaces()` | List interfaces with IP/MAC/MTU info → `Vec<NetworkInterface>` |
| `set_network_interfaces(token, enabled, addr, prefix, from_dhcp)` | Update IPv4 config; returns `RebootNeeded: bool` |
| `get_network_protocols()` | List enabled protocols (HTTP/HTTPS/RTSP, ports) → `Vec<NetworkProtocol>` |
| `set_network_protocols(protocols)` | Enable/disable protocols — `protocols` is `&[(&str, bool, &[u32])]` |
| `get_dns()` | DNS servers + DHCP flag → `DnsInformation` |
| `set_dns(from_dhcp, servers)` | Set DNS servers |
| `get_network_default_gateway()` | Default gateway addresses → `NetworkGateway` |
| `get_discovery_mode()` | Current WS-Discovery mode (`"Discoverable"` / `"NonDiscoverable"`) |
| `set_discovery_mode(mode)` | Change WS-Discovery mode |

### System & I/O

| Method | Description |
|--------|-------------|
| `get_system_log(log_type)` | Retrieve device log (`"System"` or `"Access"`) → `SystemLog` |
| `get_system_uris()` | Syslog / support-info / system-backup download URIs → `SystemUris` |
| `set_system_factory_default(default_type)` | Factory reset — `"Hard"` (full) or `"Soft"` (keep network) |
| `start_firmware_upgrade()` | Begin firmware upgrade (upload-URI flow) → `FirmwareUpgradeStart` (upload URI + timing) |
| `start_system_restore()` | Begin system restore (upload-URI flow) → `SystemRestoreStart` |
| `get_relay_outputs()` | List relay output ports → `Vec<RelayOutput>` |
| `set_relay_output_state(token, state)` | Set relay electrical state (`"active"` / `"inactive"`) |
| `set_relay_output_settings(token, mode, delay, idle)` | Configure relay mode/delay/idle-state |
| `get_storage_configurations()` | List SD/NAS storage locations → `Vec<StorageConfiguration>` |
| `set_storage_configuration(token, ...)` | Create or update a storage configuration entry |

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
| `ptz_get_configurations(ptz_url)` | List all PTZ configurations |
| `ptz_get_configuration(ptz_url, token)` | Single PTZ configuration by token |
| `ptz_set_configuration(ptz_url, config, force_persist)` | Write PTZ configuration back to device |
| `ptz_get_configuration_options(ptz_url, token)` | Valid timeout ranges for a PTZ configuration |
| `ptz_get_nodes(ptz_url)` | List PTZ nodes (capabilities, preset count, home support) |
| `ptz_goto_home_position(ptz_url, profile_token, speed)` | Move to the configured home position |
| `ptz_set_home_position(ptz_url, profile_token)` | Save current position as home |

```rust
// Save current position
let token = client.ptz_set_preset(ptz_url, &profile, Some("Entrance"), None).await?;

// Query position
let status = client.ptz_get_status(ptz_url, &profile).await?;
println!("pan={:?} tilt={:?} zoom={:?} state={}",
    status.pan, status.tilt, status.zoom, status.pan_tilt_status);
```

**`PtzStatus` fields:** `pan`, `tilt`, `zoom` (`Option<f32>`), `pan_tilt_status`, `zoom_status` (`String` — `"IDLE"` or `"MOVING"`), `utc_time` (`Option<String>`), `error` (`Option<String>` — device fault description if any).

---

## Audio Service methods

All audio methods use `media_url` from `caps.media.url`.

| Method | Returns | Description |
|--------|---------|-------------|
| `get_audio_sources(media_url)` | `Vec<AudioSource>` | Physical audio inputs (microphones) |
| `get_audio_source_configurations(media_url)` | `Vec<AudioSourceConfiguration>` | Audio source configs |
| `get_audio_encoder_configurations(media_url)` | `Vec<AudioEncoderConfiguration>` | Codec / bitrate / sample rate configs |
| `get_audio_encoder_configuration(media_url, token)` | `AudioEncoderConfiguration` | Single config by token |
| `set_audio_encoder_configuration(media_url, config)` | `()` | Write config back to device |
| `get_audio_encoder_configuration_options(media_url, token)` | `AudioEncoderConfigurationOptions` | Valid encoding / bitrate / sample rate options |

```rust
let sources = client.get_audio_sources(&media_url).await?;
println!("Audio inputs: {}", sources.len());

let mut enc = client.get_audio_encoder_configuration(&media_url, &token).await?;
enc.bitrate = 128;
client.set_audio_encoder_configuration(&media_url, &enc).await?;
```

**`AudioEncoderConfiguration` fields:** `token`, `name`, `use_count`, `encoding` (`AudioEncoding`), `bitrate` (kbps), `sample_rate` (kHz), `channels` (`Option<u32>`).

**`AudioEncoding` variants:** `G711`, `G726`, `Aac`, `Other(String)`.

---

## Imaging Service methods

All imaging methods use `imaging_url` from `caps.imaging_url` and require a `video_source_token`.

| Method | Description |
|--------|-------------|
| `get_imaging_settings(imaging_url, source_token)` | Current brightness, contrast, IR cut, white balance, exposure |
| `set_imaging_settings(imaging_url, source_token, settings)` | Write modified settings back |
| `get_imaging_options(imaging_url, source_token)` | Valid ranges for each setting |
| `imaging_get_status(imaging_url, source_token)` | Current focus position and move state |
| `imaging_get_move_options(imaging_url, source_token)` | Valid focus movement ranges |
| `imaging_move(imaging_url, source_token, focus)` | Move focus: `FocusMove::Absolute`, `Relative`, or `Continuous` |
| `imaging_stop(imaging_url, source_token)` | Stop ongoing focus movement |

```rust
let mut s = client.get_imaging_settings(&imaging_url, &source_token).await?;
s.brightness = Some(70.0);
s.ir_cut_filter = Some("AUTO".into());
client.set_imaging_settings(&imaging_url, &source_token, &s).await?;
```

**`ImagingSettings` fields:** `brightness`, `color_saturation`, `contrast`, `sharpness`, `focus_default_speed`, `wide_dynamic_range_level` (`Option<f32>`); `ir_cut_filter`, `white_balance_mode`, `exposure_mode`, `backlight_compensation`, `focus_mode`, `wide_dynamic_range_mode`, `image_stabilization_mode`, `tone_compensation_mode` (`Option<String>`).

```rust
// Move focus to an absolute position
client.imaging_move(&imaging_url, &source_token,
    &FocusMove::Absolute { position: 0.5, speed: None }).await?;

// Start continuous autofocus sweep
client.imaging_move(&imaging_url, &source_token,
    &FocusMove::Continuous { speed: 0.3 }).await?;
client.imaging_stop(&imaging_url, &source_token).await?;

// Query focus state
let status = client.imaging_get_status(&imaging_url, &source_token).await?;
println!("focus={:?}  state={}", status.focus_position, status.focus_move_status);
```

---

## OSD Service methods

On-screen display (OSD) elements overlay text or images on the video stream. All OSD methods use `media_url` from `caps.media.url`.

| Method | Returns | Description |
|--------|---------|-------------|
| `get_osds(media_url, config_token)` | `Vec<OsdConfiguration>` | List all OSD elements (pass `None` for all) |
| `get_osd(media_url, osd_token)` | `OsdConfiguration` | Get a single OSD by token |
| `set_osd(media_url, osd)` | `()` | Update an existing OSD |
| `create_osd(media_url, osd)` | `String` | Create a new OSD, returns its token |
| `delete_osd(media_url, osd_token)` | `()` | Delete an OSD element |
| `get_osd_options(media_url, config_token)` | `OsdOptions` | Valid OSD types and position options |

```rust
use oxvif::{OsdConfiguration, OsdPosition, OsdTextString};

// Create a date/time overlay in the upper-left corner
let osd = OsdConfiguration {
    token: String::new(),                           // empty = device assigns token
    video_source_config_token: vsc_token.clone(),
    type_: "Text".into(),
    position: OsdPosition { type_: "UpperLeft".into(), x: None, y: None },
    text_string: Some(OsdTextString {
        type_: "DateAndTime".into(),
        date_format: Some("MM/DD/YYYY".into()),
        time_format: Some("HH:mm:ss".into()),
        plain_text: None,
        font_size: Some(28),
        font_color: None,
        background_color: None,
        is_persistent_text: None,
    }),
    image_path: None,
};
let token = client.create_osd(&media_url, &osd).await?;
println!("Created OSD token: {token}");

// List all OSDs
let osds = client.get_osds(&media_url, None).await?;
for o in &osds {
    println!("[{}] type={} position={}", o.token, o.type_, o.position.type_);
}
```

**`OsdConfiguration` fields:** `token`, `video_source_config_token`, `type_` (`"Text"` or `"Image"`), `position` (`OsdPosition`), `text_string` (`Option<OsdTextString>`), `image_path` (`Option<String>`).

**`OsdTextString` fields:** `type_`, `plain_text`, `date_format`, `time_format` (`Option<String>`), `font_size` (`Option<u32>`), `font_color`, `background_color` (`Option<OsdColor>`), `is_persistent_text` (`Option<bool>`). `OsdColor` carries `x`/`y`/`z` channel values, optional `colorspace` URI, and `transparent` level.

**`OsdOptions` fields:** `max_osd` (`u32`), `types` (`Vec<String>`), `position_types` (`Vec<String>`), `text_types` (`Vec<String>`).

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

### Continuous event stream

`event_stream` wraps the polling loop into an infinite async `Stream` — each item
is one `NotificationMessage`. Use `futures::StreamExt::take` or a `select!` block
to bound it, and call `unsubscribe` when done.

```rust
use futures::StreamExt as _;

let sub = client.create_pull_point_subscription(&events_url, None, Some("PT60S")).await?;
let mut stream = client.event_stream(&sub.reference_url, "PT5S", 10);

while let Some(Ok(msg)) = stream.next().await {
    println!("[{}] {} {:?}", msg.utc_time, msg.topic, msg.data);
}
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

## Recording Service methods

Access and manage recordings stored on the device (NVR/DVR). Obtain `recording_url`
from `get_services()` — namespace `http://www.onvif.org/ver10/recording/wsdl`.

### Read operations

| Method | Returns | Description |
|--------|---------|-------------|
| `get_recordings(recording_url)` | `Vec<RecordingItem>` | List all stored recordings |
| `get_recording_jobs(recording_url)` | `Vec<RecordingJob>` | List all recording jobs |
| `get_recording_job_state(recording_url, job_token)` | `RecordingJobState` | Current active state of a job |

### Write operations

| Method | Returns | Description |
|--------|---------|-------------|
| `create_recording(recording_url, config)` | `String` | Create a new recording entry (`config: &RecordingConfiguration`), returns token |
| `delete_recording(recording_url, recording_token)` | `()` | Delete a recording and all its tracks |
| `create_track(recording_url, recording_token, track_type, description)` | `String` | Add a track to a recording, returns track token |
| `delete_track(recording_url, recording_token, track_token)` | `()` | Remove a track from a recording |
| `create_recording_job(recording_url, config)` | `String` | Create a new recording job (`config: &RecordingJobConfiguration`), returns job token |
| `set_recording_job_mode(recording_url, job_token, mode)` | `()` | Set job mode (`"Active"` or `"Idle"`) |
| `delete_recording_job(recording_url, job_token)` | `()` | Delete a recording job |

```rust
// Create a recording and start a job
let config = RecordingConfiguration {
    source_name: "Camera1".into(),
    source_id: "src1".into(),
    location: "Front door".into(),
    description: "Front door camera".into(),
    content: String::new(),
    maximum_retention_time: "PT0S".into(),
};
let rec_token = client.create_recording(&recording_url, &config).await?;

let track_token = client.create_track(
    &recording_url, &rec_token, "Video", "Main stream"
).await?;

let job_config = RecordingJobConfiguration {
    recording_token: rec_token.clone(),
    mode: "Active".into(),
    priority: 1,
    source_token: "VideoSourceToken_0".into(),
};
let job_token = client.create_recording_job(&recording_url, &job_config).await?;
println!("Job token: {job_token}");

// Check job state
let state = client.get_recording_job_state(&recording_url, &job_token).await?;
println!("Active state: {}", state.active_state);
```

**`RecordingItem` fields:** `token`, `source` (`RecordingSourceInformation`), `content`, `tracks` (`Vec<RecordingTrack>`).

**`RecordingSourceInformation` fields:** `source_id`, `name`, `location`, `description` (`String`), `address` (`Option<String>` — network address of the source device).

**`RecordingTrack` fields:** `token`, `track_type` (`"Video"`, `"Audio"`, `"Metadata"`), `description` (`String`), `data_from`, `data_to` (`Option<String>` ISO-8601 — time bounds of recorded data in this track).

**`RecordingJob` fields:** `token`, `recording_token`, `mode`, `priority` (`u32`), `source_token`.

**`RecordingJobState` fields:** `recording_token`, `active_state` (`"Active"`, `"Idle"`, or device-specific string).

---

## Search Service methods

Search through stored recordings. Obtain `search_url` from `get_services()` —
namespace `http://www.onvif.org/ver10/search/wsdl`.

| Method | Returns | Description |
|--------|---------|-------------|
| `find_recordings(search_url, max_matches, keep_alive)` | `String` (search token) | Start an async recording search |
| `get_recording_search_results(search_url, token, max_results, wait_time)` | `FindRecordingResults` | Poll results (call until `search_state == "Completed"`) |
| `end_search(search_url, token)` | `()` | Release search session on device |

```rust
// Find all recordings, collect results, play back the first one
let search_url   = /* from get_services() */;
let replay_url   = /* from get_services() */;

let token = client.find_recordings(&search_url, None, "PT60S").await?;

let results = loop {
    let r = client.get_recording_search_results(&search_url, &token, 100, "PT5S").await?;
    if r.search_state == "Completed" { break r; }
};
client.end_search(&search_url, &token).await?;

for rec in &results.recording_information {
    println!("[{}] {} — {} to {}",
        rec.recording_token, rec.source_name,
        rec.earliest_recording.as_deref().unwrap_or("?"),
        rec.latest_recording.as_deref().unwrap_or("?"));
}
```

**`FindRecordingResults` fields:** `search_state` (`"Queued"`, `"Searching"`, `"Completed"`), `recording_information` (`Vec<RecordingInformation>`).

**`RecordingInformation` fields:** `recording_token`, `source_name`, `earliest_recording`, `latest_recording`, `content`, `recording_status`.

---

## Replay Service methods

Stream a stored recording over RTSP. Obtain `replay_url` from `get_services()` —
namespace `http://www.onvif.org/ver10/replay/wsdl`.

| Method | Returns | Description |
|--------|---------|-------------|
| `get_replay_uri(replay_url, recording_token, stream_type, protocol)` | `String` | RTSP URI for playback |

```rust
let uri = client.get_replay_uri(
    &replay_url,
    &rec.recording_token,
    "RTP-Unicast",
    "RTSP",
).await?;
println!("Playback: {uri}");
// Open in VLC: vlc "{uri}"
```

---

## Health check (`health` feature)

A fast, scriptable conformance check — point it at a camera and get a
Pass/Warn/Fail/Skip report with a Profile S/T/G assessment. A readable
alternative to the official ONVIF Device Test Tool. Opt in with the `health`
feature; it is pure library code over `OnvifSession` (no extra dependencies).

```toml
oxvif = { version = "0.9.8", features = ["health"] }
```

```rust
use oxvif::health::HealthCheck;

let report = HealthCheck::new("http://192.168.1.100/onvif/device_service")
    .with_credentials("admin", "password")
    .run()
    .await;

println!("{report}"); // readable summary: per-check status + timings + profile verdict
```

Checks run concurrently and are read-only by default. `HealthReport` exposes the
individual `CheckResult`s (`status`, `category`, timing) and a
`ProfileAssessment` if you want to inspect results programmatically rather than
printing. See `examples/healthcheck.rs` (`cargo run --example healthcheck
--features health`).

**Parse coverage.** The report also includes a `Category::Coverage` dimension:
for a curated set of list operations it compares how many items the parser
returned against how many item elements the device actually sent, and warns when
the parser silently dropped data (the bug class where a wrong element name yields
an empty result with no error). It catches *list-emptying*; it does not catch
scalar *field-defaulting* — for that, validate against real hardware with the
**`conformance`** example (the mirror of `mock_server`):

```sh
cargo run --example conformance --features mock -- devices.txt
```

It points oxvif at a list of real cameras, dumps each raw SOAP response, and
prints a parsed summary so silent-parse mismatches stand out for review.

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

### `oxvif::mock` — test without a real camera

Depending on a physical IP camera in unit tests is painful, and every vendor's
ONVIF differs. Enable the **`mock`** feature for a built-in, **stateful** mock
ONVIF device (Set persists, Get reflects it) covering every operation oxvif
implements. There are two ways to wire it up.

```toml
[dev-dependencies]
oxvif = { version = "0.9.8", features = ["mock"] }           # MockTransport
# oxvif = { version = "0.9.8", features = ["mock-server"] }  # adds MockServer
```

**1. `MockTransport` — embedded in the client** (in-process, no sockets, no axum):

```rust
use std::sync::Arc;
use oxvif::{OnvifClient, mock::MockTransport};

#[tokio::test]
async fn embedded_mock() {
    let client = OnvifClient::new("http://mock")
        .with_transport(Arc::new(MockTransport::new()));

    client.set_hostname("lab-cam").await.unwrap();
    let h = client.get_hostname().await.unwrap();   // Set → Get round-trips
    assert_eq!(h.name.as_deref(), Some("lab-cam"));
}
```

Or keep the `MockTransport` as its own named handle instead of constructing it
inline — then you can seed/inspect its state and arm faults directly on it. It's
`Clone`, and every clone shares one device state, so hand a clone to the client:

```rust
use std::sync::Arc;
use oxvif::OnvifClient;
use oxvif::mock::MockTransport;

#[tokio::test]
async fn standalone_transport_handle() {
    let mock = MockTransport::new();                 // an independent object

    // Drive the mock directly — no client needed:
    mock.device().modify(|s| s.hostname = "seeded-cam".into());      // seed state
    mock.inject_fault("GetProfiles", "ter:NotAuthorized", "denied"); // arm one error

    // Share it with a client (clone — both sides see the same state):
    let client = OnvifClient::new("http://mock").with_transport(Arc::new(mock.clone()));

    assert_eq!(
        client.get_hostname().await.unwrap().name.as_deref(),
        Some("seeded-cam"),
    );
    assert!(client.get_profiles("http://mock/media").await.is_err()); // consumes the fault

    // Inspect, off the handle, what the client changed:
    client.set_hostname("after").await.unwrap();
    assert_eq!(mock.device().read().hostname, "after");
}
```

**2. `MockServer` — a standalone server you connect to over real HTTP** (needs
the `mock-server` feature). Start it on its own port and point an *ordinary*
`OnvifClient` / `OnvifSession` at it — nothing is injected into the client, so
the real HTTP transport (and, optionally, WS-Security) is exercised end-to-end:

```rust
use oxvif::{OnvifSession, mock::MockServer};

#[tokio::test]
async fn standalone_server() -> Result<(), oxvif::OnvifError> {
    let server = MockServer::start().await.unwrap();   // ephemeral 127.0.0.1 port

    // A normal session talking to the mock over HTTP — no transport swap.
    let session = OnvifSession::builder(server.device_url()).build().await?;
    assert_eq!(session.get_device_info().await?.manufacturer, "oxvif-mock");

    // Arm an error for the next GetProfiles to test your error handling.
    server.inject_fault("GetProfiles", "ter:NotAuthorized", "denied");
    assert!(session.get_profiles().await.is_err());
    Ok(())
}   // server shuts down when dropped
```

Both default to **no authentication** (frictionless tests) — call `.with_auth()`
(`MockTransport`) / `.enforce_auth(true)` (`MockServer::builder()`) to exercise
WS-Security. State is in-memory; opt into persistence via `MockState::set_on_change`.

### `oxvif::mock` API reference

**`MockTransport`** (`mock` feature) — in-process `Transport`; `Clone` + `Default`;
pass via `OnvifClient::with_transport(Arc::new(..))`:

| Method | Description |
|--------|-------------|
| `MockTransport::new()` | Default device, auth off |
| `MockTransport::with_state(MockState)` | Build from a seeded state |
| `.with_auth()` | Enforce WS-Security (builder-style, consumes `self`) |
| `.device() -> &MockState` | Seed / inspect device state |
| `.inject_fault(suffix, code, reason)` | Arm a single-shot SOAP Fault for the next matching action |
| `.clear_faults()` | Drop all queued faults |

**`MockServer`** (`mock-server` feature) — bound-port HTTP server; shuts down on drop:

| Method | Description |
|--------|-------------|
| `MockServer::start().await` | Start on an ephemeral port (auth off) → `io::Result<MockServer>` |
| `MockServer::builder()` | `.port(u16)` · `.initial_state(DeviceState)` · `.on_change(hook)` · `.enforce_auth(bool)` · `.start().await` |
| `.device_url()` / `.base_url()` / `.port()` | Connection info for `OnvifClient` / `OnvifSession` |
| `.device()` / `.inject_fault(..)` / `.clear_faults()` | Same as `MockTransport` |

HTTP extras: `GET /mock/snapshot.jpg`, `POST /admin/inject_fault?action=&code=&reason=`, `POST /admin/clear_faults`.

**`MockState`** — shared device state (seed / assert / persist):

| Method | Description |
|--------|-------------|
| `MockState::new()` / `::with_state(DeviceState)` | Create |
| `.read()` | Read guard over `DeviceState`, for assertions |
| `.modify(\|s\| ..)` / `.modify_returning(\|s\| ..)` | Mutate the state |
| `.set_on_change(hook)` | Fire a callback after each mutation — the persistence seam |

`DeviceState` is `serde`-serializable for snapshot/restore; the library itself
never writes to disk.

### Standalone mock server (`cargo run`)

The `examples/mock_server` binary wraps `oxvif::mock::MockServer` with TOML file
persistence (state survives restarts) — handy for manual testing and OxDM:

```sh
# Terminal 1 — start the mock server (default port 18080); needs the feature
cargo run --example mock_server --features mock-server

# Terminal 2 — run any example against it (no credentials required)
ONVIF_URL=http://127.0.0.1:18080/onvif/device \
cargo run --example camera -- full-workflow
```

It serves `GET /mock/snapshot.jpg` (a test-pattern image) and persists Set
operations to `~/.oxvif/mock_device.toml`. The mock engine's unit tests run with
`cargo test --features mock-server`.

#### Using with OxDM

The mock server is designed to work with [OxDM](https://github.com/smiti1642/oxdm),
the Dioxus-based ONVIF Device Manager:

```sh
# Terminal 1 — start mock server
cd oxvif && cargo run --example mock_server --features mock-server

# Terminal 2 — start OxDM
cd oxdm && dx serve --platform desktop
```

In OxDM:
1. Click the **Manual** tab in the device list
2. Click **Add** and enter `127.0.0.1:18080` (auto-completes to full ONVIF URL)
3. No credentials needed for the mock server — leave empty
4. The device appears with snapshot thumbnails refreshing every 3 seconds
5. Settings tabs (Identification, Network, Time, Users, Maintenance) show
   live data from the mock server's stateful device service

Or copy `.env.example` to `.env` and set `ONVIF_URL=http://127.0.0.1:18080/onvif/device`
so examples pick it up automatically via `dotenvy`.

### Unit test transport mock

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
cargo run --example camera -- full-workflow          # end-to-end: all implemented operations
cargo run --example camera -- session                # same workflow via OnvifSession API
cargo run --example camera -- device-info            # manufacturer, model, firmware
cargo run --example camera -- device-management      # hostname, NTP, GetServices
cargo run --example camera -- stream-uris            # tabular RTSP URI listing
cargo run --example camera -- snapshot-uris          # tabular HTTP snapshot URI listing
cargo run --example camera -- system-datetime        # device clock and UTC offset
cargo run --example camera -- ptz-presets            # list all PTZ presets
cargo run --example camera -- ptz-status             # current pan/tilt/zoom position
cargo run --example camera -- ptz-config             # PTZ configurations and nodes
cargo run --example camera -- ptz-home               # go to / set PTZ home position
cargo run --example camera -- audio                  # audio sources and encoder configs
cargo run --example camera -- imaging-focus          # focus status, move options, move/stop
cargo run --example camera -- osd                    # on-screen display elements (list, create, delete)
cargo run --example camera -- video-config           # video sources, encoder configs (Media1)
cargo run --example camera -- video-config-media2    # H.265 encoder configs (Media2)
cargo run --example camera -- imaging                # brightness, contrast, exposure settings
cargo run --example camera -- events                 # subscribe, pull, renew, unsubscribe
cargo run --example camera -- event-stream           # continuous event stream via event_stream()
cargo run --example camera -- recording              # list recordings, search, get replay URI
cargo run --example camera -- recording-jobs         # recording jobs: list, create, set mode, delete
cargo run --example camera -- users                  # list, create, delete device user accounts
cargo run --example camera -- network-config         # interfaces, protocols, DNS, gateway
cargo run --example camera -- relay-outputs          # list relay outputs and trigger state change
cargo run --example camera -- storage                # list storage configurations (SD/NAS)
cargo run --example camera -- discovery-mode         # show and toggle WS-Discovery mode
cargo run --example camera -- discovery              # WS-Discovery UDP multicast probe
cargo run --example camera -- error-handling         # typed error variant matching demo
cargo run --example camera -- healthcheck            # quick reachability + auth check
```

#### Direct device targeting with `--ip` and `--auth`

Instead of using a `.env` file, you can pass the device address and credentials
directly on the command line:

```sh
cargo run --example camera -- --ip 192.168.1.100 --auth admin:password device-info
cargo run --example camera -- --ip 192.168.1.100 healthcheck
```

To run without a real camera, start the mock server first — see
[Testing without a real camera](#testing-without-a-real-camera).

### Mock server

```sh
# Default port 18080; pass a port number to override (needs the feature)
cargo run --example mock_server --features mock-server
cargo run --example mock_server --features mock-server -- 19090

# Run the mock engine's unit tests
cargo test --features mock-server
```

### Conformance check (real devices)

```sh
# Validate the parsers against a list of real cameras; flags silent-parse gaps.
# The device-list file (pipe-delimited: name | url | user | pass) holds
# credentials — keep it out of version control.
cargo run --example conformance --features mock -- devices.txt
```

---

## Project structure

```
src/
├── lib.rs               Public API surface and re-exports
├── client/
│   ├── mod.rs           OnvifClient — constructor and builder methods
│   ├── device.rs        Device service methods
│   ├── events.rs        Events service methods (incl. event_stream)
│   ├── imaging.rs       Imaging service methods
│   ├── media.rs         Media1 service methods
│   ├── media2.rs        Media2 service methods
│   ├── ptz.rs           PTZ service methods
│   └── recording.rs     Recording / Search / Replay service methods
├── session.rs           OnvifSession — convenience wrapper with cached service URLs
├── discovery.rs         WS-Discovery UDP multicast probe
├── error.rs             OnvifError unified error type
├── transport.rs         Transport trait + HttpTransport (reqwest + rustls)
├── fixtures.rs          CapturingTransport / FixtureTransport — record-and-replay test seam
├── soap/
│   ├── mod.rs
│   ├── envelope.rs      SOAP 1.2 envelope builder
│   ├── security.rs      WS-Security UsernameToken / PasswordDigest
│   ├── xml.rs           Namespace-stripping XML parser (XmlNode)
│   └── error.rs         SoapError
├── types/
│   ├── mod.rs           XML helper functions (xml_escape, xml_str, …)
│   ├── audio.rs         AudioSource, AudioEncoderConfiguration, AudioEncoding
│   ├── capabilities.rs  Capabilities, service sub-structs
│   ├── device.rs        DeviceInfo, NetworkInterfaceConfig, SystemDateTime, Hostname, NtpInfo, StorageConfiguration
│   ├── events.rs        PullPointSubscription, NotificationMessage, EventProperties
│   ├── imaging.rs       ImagingSettings, ImagingOptions, ImagingStatus
│   ├── media.rs         MediaProfile, MediaProfile2, StreamUri, SnapshotUri
│   ├── osd.rs           OsdConfiguration, OsdTextString, OsdColor, OsdOptions
│   ├── ptz.rs           PtzPreset, PtzStatus
│   ├── ptz_config.rs    PtzConfiguration, PtzConfigurationOptions, PtzNode, PtzSpeed
│   ├── recording.rs     RecordingItem, RecordingJob, RecordingJobConfiguration, RecordingJobState
│   └── video.rs         VideoSource, VideoEncoder configs and options
├── mock/                In-process mock ONVIF device (mock / mock-server features)
│   ├── mod.rs           Public surface — MockTransport, MockServer, MockState, DeviceState
│   ├── transport.rs     MockTransport — Transport impl, zero-network
│   ├── server.rs        MockServer — axum bound-port server (mock-server only)
│   ├── state.rs         Stateful DeviceState — Set persists, Get reflects
│   ├── dispatch.rs      SOAP action routing to per-service handlers
│   ├── services/        device / media / media2 / ptz / imaging / events / recording handlers
│   ├── auth.rs          Optional WS-Security enforcement
│   ├── fault_injection.rs  Single-shot SOAP Fault queue per action
│   ├── helpers.rs       SOAP envelope helpers
│   ├── snapshot.rs      Test-pattern JPEG generator (GET /mock/snapshot.jpg)
│   ├── font.rs          5×7 bitmap font used by the snapshot generator
│   └── xml_parse.rs     Request body tag extraction
├── health/              Read-only ONVIF health / conformance check (health feature)
│   ├── mod.rs           HealthCheck builder + HealthReport public surface
│   ├── checks.rs        Individual check implementations (connectivity, services, …)
│   └── report.rs        HealthReport / CheckResult / ReportDiff types (Serde + diff)
└── tests/
    ├── client_tests.rs  unit tests covering all client methods
    ├── session_tests.rs unit tests for OnvifSession builder and delegates
    └── types_tests.rs   XML parsing unit tests
examples/
├── camera.rs            Live camera integration examples (all commands)
├── mock_server/         Thin wrapper over oxvif::mock::MockServer with TOML persistence
│   └── main.rs          Entry point (--features mock-server)
├── healthcheck.rs       Scriptable health/conformance check + --json + --baseline (--features health)
├── record_fixtures.rs   Capture every SOAP exchange against a live device for replay (--features mock,health)
├── conformance.rs       Validate the parsers against a fleet of real cameras; flags silent-parse gaps (--features mock)
├── probe_unicast.rs     One-shot unicast WS-Discovery probe to a specific host
├── odm_compat.rs        ODM compatibility integration test
└── write_workflow.rs    Write-operation workflow with embedded mock server
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
| `AbsoluteMove` / `RelativeMove` / `ContinuousMove` | ✓ |
| `Stop` | ✓ |
| `GetPresets` / `GotoPreset` | ✓ |
| `SetPreset` / `RemovePreset` | ✓ |
| `GetStatus` | ✓ |
| `GetConfigurations` / `GetConfiguration` | ✓ |
| `SetConfiguration` / `GetConfigurationOptions` | ✓ |
| `GetNodes` / `GetNode` | ✓ |
| `GetCompatibleConfigurations` | ✓ |

### Imaging Service

| Operation | Status |
|-----------|--------|
| `GetImagingSettings` / `SetImagingSettings` | ✓ |
| `GetOptions` | ✓ |
| `Move` / `Stop` / `GetMoveOptions` / `GetStatus` | ✓ |

### Events Service

| Operation | Status |
|-----------|--------|
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
| `GetRecordings` | ✓ |
| `CreateRecording` / `DeleteRecording` | ✓ |
| `CreateTrack` / `DeleteTrack` | ✓ |
| `GetRecordingJobs` | ✓ |
| `CreateRecordingJob` / `SetRecordingJobMode` / `DeleteRecordingJob` | ✓ |
| `GetRecordingJobState` | ✓ |

### Search Service

| Operation | Status |
|-----------|--------|
| `FindRecordings` | ✓ |
| `GetRecordingSearchResults` | ✓ |
| `EndSearch` | ✓ |

### Replay Service

| Operation | Status |
|-----------|--------|
| `GetReplayUri` | ✓ |

### WS-Discovery

| Operation | Status |
|-----------|--------|
| UDP multicast `Probe` | ✓ |
| `Hello` / `Bye` passive listening (`listen`) | ✓ |

---

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history.

---

## License

MIT
