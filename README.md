# oxvif

Async Rust client library for the [ONVIF](https://www.onvif.org/) IP camera protocol.

```
SOAP/HTTP ──► OnvifClient ──► Capabilities / DeviceInfo
                          ──► Vec<MediaProfile>
                          ──► StreamUri / SnapshotUri
                          ──► SystemDateTime
                          ──► PTZ (move / stop / presets)
```

- Async-first (`tokio` + `reqwest`)
- WS-Security `UsernameToken` with `PasswordDigest` (ONVIF Profile S §5.12)
- Mockable transport — unit-test without a real camera
- No unsafe code; pure Rust XML parsing via `quick-xml`
- LF line endings enforced via `.gitattributes`

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

Add to `Cargo.toml`:

```toml
[dependencies]
oxvif = { path = "." }   # local path until published to crates.io
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

---

## `OnvifClient`

The main entry point. Stateless and cheaply cloneable — safe to wrap in `Arc` and share across threads.

### Constructors

| Method | Description |
|--------|-------------|
| `OnvifClient::new(device_url)` | Connect to device at `device_url` (e.g. `http://192.168.1.100/onvif/device_service`) |

### Builder methods

| Method | Description |
|--------|-------------|
| `.with_credentials(username, password)` | Enable WS-Security `UsernameToken` authentication |
| `.with_utc_offset(offset_secs: i64)` | Adjust WS-Security timestamp if device clock differs from local UTC |
| `.with_transport(Arc<dyn Transport>)` | Replace the default HTTP transport (used for unit testing) |

### Example

```rust
// Sync device clock before sending authenticated requests
let client = OnvifClient::new("http://192.168.1.100/onvif/device_service");
let dt = client.get_system_date_and_time().await?;
let client = client
    .with_credentials("admin", "secret")
    .with_utc_offset(dt.utc_offset_secs());
```

---

## API methods

### `get_capabilities() -> Result<Capabilities, OnvifError>`

Retrieves all service endpoint URLs and feature flags from the device.
**Always call this first** — the returned URLs are required by all subsequent calls.

```rust
let caps = client.get_capabilities().await?;

// Service URLs
caps.device.url        // Device management service
caps.media.url         // Media service (profiles / stream URIs)
caps.ptz_url           // PTZ service
caps.events.url        // Events service
caps.imaging_url       // Imaging service
caps.analytics.url     // Analytics service

// Device capabilities
caps.device.network.ip_version6
caps.device.system.firmware_upgrade
caps.device.security.username_token

// Media capabilities
caps.media.streaming.rtp_rtsp_tcp
caps.media.streaming.rtp_multicast
caps.media.max_profiles          // Option<u32>

// Events capabilities
caps.events.ws_pull_point
caps.events.ws_subscription_policy
```

---

### `get_system_date_and_time() -> Result<SystemDateTime, OnvifError>`

Retrieves the device clock. Use the result to calibrate WS-Security timestamps.

```rust
let dt = client.get_system_date_and_time().await?;

dt.utc_unix          // Option<i64>  Unix timestamp of the device UTC clock
dt.timezone          // String       POSIX timezone (e.g. "CST-8")
dt.daylight_savings  // bool

// Compute offset and apply before sending authenticated requests
let offset = dt.utc_offset_secs();   // device_utc − local_utc
let client = client.with_credentials("admin", "pass")
                   .with_utc_offset(offset);
```

---

### `get_device_info() -> Result<DeviceInfo, OnvifError>`

Returns hardware and firmware metadata. Many cameras expose this without authentication.

```rust
let info = client.get_device_info().await?;

info.manufacturer    // e.g. "Hikvision"
info.model           // e.g. "DS-2CD2085G1-I"
info.firmware_version
info.serial_number
info.hardware_id
```

---

### `get_profiles(media_url) -> Result<Vec<MediaProfile>, OnvifError>`

Lists all media profiles. Each profile represents a stream configuration (resolution, codec, frame rate, etc.).

```rust
let profiles = client.get_profiles(&caps.media.url.unwrap()).await?;

for p in &profiles {
    println!("{} — token: {}, fixed: {}", p.name, p.token, p.fixed);
}
```

**`MediaProfile` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `token` | `String` | Opaque identifier; pass to `get_stream_uri` / `get_snapshot_uri` / PTZ methods |
| `name` | `String` | Human-readable name (e.g. `"mainStream"`) |
| `fixed` | `bool` | `true` = profile cannot be deleted |

---

### `get_stream_uri(media_url, profile_token) -> Result<StreamUri, OnvifError>`

Returns the RTSP URI for the given media profile.

```rust
let uri = client.get_stream_uri(media_url, &profiles[0].token).await?;

uri.uri                    // e.g. "rtsp://192.168.1.100:554/Streaming/Channels/101"
uri.invalid_after_connect  // true = URI is one-time use
uri.invalid_after_reboot   // true = URI expires on reboot
uri.timeout                // ISO 8601 duration, e.g. "PT60S" ("PT0S" = no expiry)
```

Play with VLC or ffmpeg:

```sh
ffplay "rtsp://192.168.1.100:554/Streaming/Channels/101"
```

---

### `get_snapshot_uri(media_url, profile_token) -> Result<SnapshotUri, OnvifError>`

Returns the HTTP URL for fetching a JPEG snapshot from the given media profile.

```rust
let snap = client.get_snapshot_uri(media_url, &profiles[0].token).await?;

snap.uri                    // e.g. "http://192.168.1.100/onvif/snapshot?channel=1"
snap.invalid_after_connect  // true = URI is one-time use
snap.invalid_after_reboot   // true = URI expires on reboot
snap.timeout                // ISO 8601 expiry duration
```

Fetch the snapshot:

```sh
curl -o snapshot.jpg "http://192.168.1.100/onvif/snapshot?channel=1"
```

---

## PTZ methods

All PTZ methods take `ptz_url` from `caps.ptz_url`.
Coordinates use the ONVIF normalised range: pan/tilt `[-1.0, 1.0]`, zoom `[0.0, 1.0]`.

### `ptz_absolute_move(ptz_url, profile_token, pan, tilt, zoom)`

Move to an absolute position.

```rust
// Centre frame, half zoom
client.ptz_absolute_move(ptz_url, &token, 0.0, 0.0, 0.5).await?;
```

### `ptz_relative_move(ptz_url, profile_token, pan, tilt, zoom)`

Move by an offset from the current position.

```rust
// Pan right slightly
client.ptz_relative_move(ptz_url, &token, 0.1, 0.0, 0.0).await?;
```

### `ptz_continuous_move(ptz_url, profile_token, pan, tilt, zoom)`

Start continuous movement at the given velocity. Call `ptz_stop` to halt.

```rust
client.ptz_continuous_move(ptz_url, &token, 0.5, 0.0, 0.0).await?;
// ... wait ...
client.ptz_stop(ptz_url, &token).await?;
```

### `ptz_stop(ptz_url, profile_token)`

Stop all pan, tilt, and zoom movement.

### `ptz_get_presets(ptz_url, profile_token) -> Result<Vec<PtzPreset>, OnvifError>`

List all saved preset positions.

```rust
let presets = client.ptz_get_presets(ptz_url, &token).await?;
for p in &presets {
    println!("[{}] {} — pan/tilt: {:?}, zoom: {:?}", p.token, p.name, p.pan_tilt, p.zoom);
}
```

**`PtzPreset` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `token` | `String` | Opaque identifier; pass to `ptz_goto_preset` |
| `name` | `String` | Human-readable preset name |
| `pan_tilt` | `Option<(f32, f32)>` | Stored pan (x) and tilt (y), or `None` if absent |
| `zoom` | `Option<f32>` | Stored zoom, or `None` if absent |

### `ptz_goto_preset(ptz_url, profile_token, preset_token)`

Move to a saved preset position.

```rust
client.ptz_goto_preset(ptz_url, &profile_token, &presets[0].token).await?;
```

---

## Error handling

All API methods return `Result<T, OnvifError>`. The error has two variants:

```rust
pub enum OnvifError {
    Transport(TransportError),  // network / TLS / unexpected HTTP status
    Soap(SoapError),            // parse failure, missing field, or SOAP Fault
}
```

### Full match example

```rust
use oxvif::error::OnvifError;
use oxvif::soap::SoapError;
use oxvif::transport::TransportError;

match client.get_capabilities().await {
    Ok(caps) => { /* use caps */ }

    // Network unreachable, TLS error, timeout, etc.
    Err(OnvifError::Transport(TransportError::Http(e))) => {
        eprintln!("Network error: {e}");
    }

    // Server replied with 401, 403, 404, etc.
    Err(OnvifError::Transport(TransportError::HttpStatus { status, body })) => {
        eprintln!("HTTP {status}: {body}");
    }

    // Device returned a SOAP <s:Fault> (wrong credentials, unsupported operation)
    Err(OnvifError::Soap(SoapError::Fault { code, reason })) => {
        eprintln!("SOAP Fault [{code}]: {reason}");
    }

    Err(e) => eprintln!("Other error: {e}"),
}
```

### `TransportError` variants

| Variant | Trigger |
|---------|---------|
| `Http(reqwest::Error)` | Network failure, TLS handshake error, timeout |
| `HttpStatus { status, body }` | HTTP response other than 200 or 500 |

> HTTP 500 is passed through as `Ok` so the SOAP layer can extract the `<s:Fault>` detail.

### `SoapError` variants

| Variant | Meaning |
|---------|---------|
| `XmlParse(String)` | Malformed XML in the response |
| `MissingBody` | Response envelope has no `<s:Body>` |
| `MissingField(&'static str)` | Expected XML element was absent |
| `UnexpectedResponse(String)` | Response element name did not match |
| `Fault { code, reason }` | Device returned `<s:Fault>` |
| `InvalidValue(String)` | A field value could not be parsed |

---

## Testing without a real camera

Implement the `Transport` trait to inject any response you like:

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

let caps = client.get_capabilities().await.unwrap();
```

Run the built-in unit tests:

```sh
cargo test
```

---

## Running the built-in examples

**Step 1** — copy the example env file and fill in your camera details:

```sh
cp .env.example .env
```

`.env` contents:

```sh
ONVIF_URL=http://192.168.1.100/onvif/device_service
ONVIF_USERNAME=admin
ONVIF_PASSWORD=your_password
```

`.env` is listed in `.gitignore` and will never be committed.

**Step 2** — run an example:

```sh
cargo run -- full-workflow     # capabilities → profiles → RTSP URIs
cargo run -- device-info       # manufacturer, model, firmware
cargo run -- stream-uris       # tabular RTSP URI listing
cargo run -- snapshot-uris     # tabular HTTP snapshot URI listing
cargo run -- system-datetime   # device clock and UTC offset
cargo run -- ptz-presets       # list all PTZ presets (requires PTZ camera)
cargo run -- error-handling    # typed error matching demo
```

`ONVIF_USERNAME` and `ONVIF_PASSWORD` are optional (default: `admin` / empty).

---

## Project structure

```
src/
├── lib.rs            Public API surface and crate-level docs
├── client.rs         OnvifClient — all ONVIF operations
├── types.rs          Response structs (Capabilities, DeviceInfo, …)
├── error.rs          OnvifError unified error type
├── transport.rs      Transport trait + HttpTransport (reqwest + rustls)
└── soap/
    ├── envelope.rs   SOAP 1.2 envelope builder
    ├── security.rs   WS-Security UsernameToken / PasswordDigest
    ├── xml.rs        Namespace-stripping XML parser (XmlNode)
    └── error.rs      SoapError
```

---

## Implemented ONVIF operations

| Operation | Service | Status |
|-----------|---------|--------|
| `GetCapabilities` | Device | ✓ |
| `GetDeviceInformation` | Device | ✓ |
| `GetSystemDateAndTime` | Device | ✓ |
| `GetProfiles` | Media | ✓ |
| `GetStreamUri` | Media | ✓ |
| `GetSnapshotUri` | Media | ✓ |
| `AbsoluteMove` | PTZ | ✓ |
| `RelativeMove` | PTZ | ✓ |
| `ContinuousMove` | PTZ | ✓ |
| `Stop` | PTZ | ✓ |
| `GetPresets` | PTZ | ✓ |
| `GotoPreset` | PTZ | ✓ |
| Events (Subscribe / Pull) | Events | planned |
| `GetVideoEncoderConfigurations` | Media | planned |
| WS-Discovery | UDP multicast | planned |

---

## License

MIT
