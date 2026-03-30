# oxvif

Async Rust client library for the [ONVIF](https://www.onvif.org/) IP camera protocol.

```
SOAP/HTTP ──► OnvifClient ──► Capabilities
                          ──► DeviceInfo
                          ──► Vec<MediaProfile>
                          ──► StreamUri
```

- Async-first (`tokio` + `reqwest`)
- WS-Security `UsernameToken` with `PasswordDigest` (ONVIF Profile S §5.12)
- Mockable transport — unit-test without a real camera
- No unsafe code; pure Rust XML parsing via `quick-xml`

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
    let media_url = caps.media_url.as_deref().unwrap();

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
| `.with_utc_offset(offset_secs: i64)` | Adjust WS-Security timestamp if device clock differs from local UTC. Obtain the offset from `GetSystemDateAndTime`. |
| `.with_transport(Arc<dyn Transport>)` | Replace the default HTTP transport (used for unit testing) |

### Example

```rust
let client = OnvifClient::new("http://192.168.1.100/onvif/device_service")
    .with_credentials("admin", "secret")
    .with_utc_offset(-5);   // device is 5 seconds behind local UTC
```

---

## API methods

### `get_capabilities() -> Result<Capabilities, OnvifError>`

Retrieves all service endpoint URLs from the device. **Always call this first** — the returned URLs are required by all subsequent media, PTZ, events, and imaging calls.

```rust
let caps = client.get_capabilities().await?;

// Returned fields — all Option<String>
caps.device_url    // Device management service
caps.media_url     // Media service (needed for profiles / stream URIs)
caps.ptz_url       // PTZ service
caps.events_url    // Events service
caps.imaging_url   // Imaging service
caps.analytics_url // Analytics service
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

Lists all media profiles available on the device. Each profile represents a stream configuration (resolution, codec, frame rate, etc.).

`media_url` must be the value from `caps.media_url`.

```rust
let profiles = client.get_profiles(&caps.media_url.unwrap()).await?;

for p in &profiles {
    println!("{} — token: {}, fixed: {}", p.name, p.token, p.fixed);
}
```

**`MediaProfile` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `token` | `String` | Opaque identifier; pass to `get_stream_uri` |
| `name` | `String` | Human-readable name (e.g. `"mainStream"`) |
| `fixed` | `bool` | `true` = profile cannot be deleted |

---

### `get_stream_uri(media_url, profile_token) -> Result<StreamUri, OnvifError>`

Returns the RTSP URI for the given media profile.

```rust
let uri = client.get_stream_uri(media_url, &profiles[0].token).await?;

uri.uri                    // e.g. "rtsp://192.168.1.100:554/Streaming/Channels/101"
uri.invalid_after_connect  // true = URI one-time use only
uri.invalid_after_reboot   // true = URI expires on reboot
uri.timeout                // ISO 8601 duration, e.g. "PT60S", "PT0S" = no expiry
```

Play with VLC or ffmpeg:

```sh
ffplay "rtsp://192.168.1.100:554/Streaming/Channels/101"
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
cargo run -- full-workflow    # capabilities → profiles → RTSP URIs
cargo run -- device-info      # manufacturer, model, firmware
cargo run -- stream-uris      # tabular RTSP URI listing
cargo run -- error-handling   # typed error matching demo
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
| `GetProfiles` | Media | ✓ |
| `GetStreamUri` | Media | ✓ |
| `GetSystemDateAndTime` | Device | planned |
| `GetSnapshotUri` | Media | planned |
| PTZ (Move / Stop / Preset) | PTZ | planned |
| Events (Subscribe / Pull) | Events | planned |
| WS-Discovery | UDP multicast | planned |

---

## License

MIT
