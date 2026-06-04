# SOAP fixture archive

Captured `(request, response)` pairs from real ONVIF cameras, used to drive
parser regression tests without needing the camera in CI.

## Layout

```
tests/fixtures/
  <vendor>-<model>/
    GetCapabilities.req.xml
    GetCapabilities.resp.xml
    GetProfiles.req.xml
    GetProfiles.resp.xml
    ...
```

One directory per `(vendor, model[, firmware])` combination. Filenames are
the **last URL segment of the SOAP action**, stripped to `[A-Za-z0-9_-]`:

| SOAP action                                                    | Basename             |
|----------------------------------------------------------------|----------------------|
| `http://www.onvif.org/ver10/device/wsdl/GetCapabilities`       | `GetCapabilities`    |
| `http://www.onvif.org/ver10/media/wsdl/GetProfiles`            | `GetProfiles`        |
| `http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurations` | `GetVideoEncoderConfigurations` |

`*.req.xml` is what oxvif sent; `*.resp.xml` is what the camera replied.
For replay, only the `.resp.xml` is consulted by `FixtureTransport`.

## Recording a new device

```sh
cargo run --example record_fixtures --features "mock health" -- \
    http://<camera-ip>/onvif/device_service <user> <pass> \
    tests/fixtures/<vendor>-<model>
```

The recorder drives the same read surface `HealthCheck::run` exercises
(device info, capabilities, profiles, streams, encoders, imaging, PTZ,
events, network, users) — enough to seed a useful fixture set for most
service-level parser tests.

## Replaying in tests

```rust
use oxvif::{FixtureTransport, OnvifClient};
use std::sync::Arc;

let fix = FixtureTransport::new("tests/fixtures/hikvision-ds-2cd2085");
let client = OnvifClient::new("http://replay")
    .with_transport(Arc::new(fix));
let caps = client.get_capabilities().await.unwrap();
// assertions follow…
```

If the captured fixture for a given action is missing, `FixtureTransport`
returns `TransportError::HttpStatus { status: 404, body: <path> }` —
useful for telling "I forgot to record this" apart from "the parser
rejected the wire shape".

## Privacy / hygiene

- Strip MAC addresses, serial numbers, and any credentials that leak via
  echoed-back URLs (the recorder does **not** auto-scrub).
- Hostnames inside `<tt:Address>` etc. are usually fine to keep but feel
  free to redact for shareable fixtures.
- Never commit a `*.req.xml` containing a real password (the recorder
  writes the body verbatim; WS-Security `UsernameToken` digests are
  one-way but the username is plaintext).
