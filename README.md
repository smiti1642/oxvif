# oxvif

[![crates.io](https://img.shields.io/crates/v/oxvif.svg)](https://crates.io/crates/oxvif)
[![docs.rs](https://img.shields.io/docsrs/oxvif)](https://docs.rs/oxvif)
[![downloads](https://img.shields.io/crates/d/oxvif.svg)](https://crates.io/crates/oxvif)
[![license](https://img.shields.io/crates/l/oxvif.svg)](https://github.com/smiti1642/oxvif/blob/master/LICENSE)

Async Rust client library and command-line tooling for
[ONVIF](https://www.onvif.org/) IP cameras. oxvif covers discovery, device
management, Media1/Media2, PTZ, imaging, events, recording, search, replay,
health diagnostics, and camera-free testing.

## Why oxvif

- Async-first with `tokio` and `reqwest`.
- WS-Security `UsernameToken` and HTTP Digest authentication.
- WS-Discovery with fallible, interface-aware APIs.
- High-level `OnvifSession` URL caching or direct `OnvifClient` routing.
- Built-in stateful mock device and bound-port mock server.
- Metamorph tooling to clone, replay, and compare real camera behavior.
- Pure Rust XML parsing and no unsafe code.
- A read-only, human- and Agent-friendly `oxvif` CLI with deterministic JSON.

## Choose your interface

| Interface | Best for | Start here |
| --- | --- | --- |
| Rust library | Applications that need typed ONVIF access and full routing control. | [Library quick start](#library-quick-start) |
| `oxvif` CLI | Operators, diagnostics, CI, Agents, and fleet inventory. | [CLI overview](#command-line-interface) |
| Mock device | Tests that need ONVIF behavior without camera hardware. | [Testing without a camera](#testing-without-a-camera) |

## Installation

The current crates.io release is 0.15:

```toml
[dependencies]
oxvif = "0.15"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

The `develop` branch is preparing oxvif 0.16.0 and oxvif-cli 0.1.0. Until
those versions are publicly released and independently verified, use 0.15 for
crates.io consumers or pin an explicit source revision for evaluation.

## Library quick start

`OnvifSession` discovers and caches service URLs during construction:

```rust
use oxvif::{OnvifError, OnvifSession};

#[tokio::main]
async fn main() -> Result<(), OnvifError> {
    let session = OnvifSession::builder(
        "http://192.168.1.100/onvif/device_service",
    )
    .with_credentials("admin", "password")
    .with_clock_sync()
    .build()
    .await?;

    let profiles = session.get_profiles().await?;
    let uri = session.get_stream_uri(&profiles[0].token).await?;
    println!("RTSP: {}", uri.uri);
    Ok(())
}
```

Use `OnvifClient` instead when your application needs to supply every service
URL directly. The [complete library and feature guide](LIBRARY_GUIDE.md) covers
both interfaces, discovery, every service family, error handling, and advanced
features. The generated [Rust API documentation](https://docs.rs/oxvif) is the
authoritative method and type reference.

## Command-line interface

The separately publishable `oxvif-cli` package installs an executable named
`oxvif`. Its 0.1 ONVIF surface is intentionally read-only: operators and Agents
can discover devices, maintain local inventory, inspect device/media/PTZ state,
and run deterministic health and fleet diagnostics without changing camera
configuration.

For pre-release evaluation, install from the current checkout:

```sh
cargo install --path crates/oxvif-cli --locked
oxvif --help
oxvif setup front-door 192.168.1.100 --username admin
oxvif --device front-door device info --output json --non-interactive
```

Passwords remain in Windows Credential Manager, macOS Keychain, or Linux
Secret Service rather than the device registry. Private HTTPS trust anchors can
be supplied without disabling certificate or hostname verification.

Read the [complete CLI guide](docs/oxvif-cli.md), its
[Traditional Chinese version](docs/oxvif-cli.zh-TW.md), or the
[`oxvif-cli` package README](https://github.com/smiti1642/oxvif/blob/master/crates/oxvif-cli/README.md).

Signed APT and Homebrew packaging has passed non-publishing three-platform
staging, but no public APT repository or Homebrew tap exists yet. This README
will publish verified installation commands only after the release approval
and independent installation checks are complete.

## Feature overview

| Area | Highlights | Detailed reference |
| --- | --- | --- |
| Discovery and device | WS-Discovery, capabilities, services, identity, time, network, users, and I/O. | [Guide](LIBRARY_GUIDE.md#ws-discovery) |
| Media | Media1/Media2 profiles, H.264/H.265, audio, stream/snapshot URIs, and video source modes. | [Guide](LIBRARY_GUIDE.md#media-service-media1-methods) |
| PTZ and imaging | Move/stop, presets, tours, home, status, exposure, focus, IR cut, and OSD. | [Guide](LIBRARY_GUIDE.md#ptz-methods) |
| Events | Pull-point subscriptions, renew/unsubscribe, and continuous event streams. | [Guide](LIBRARY_GUIDE.md#events-service-methods) |
| Recording | Recording/job management, time/scope search, and replay URIs. | [Guide](LIBRARY_GUIDE.md#recording-service-methods) |
| Diagnostics | Optional health checks, parse-coverage detection, and conformance tooling. | [Guide](LIBRARY_GUIDE.md#health-check-health-feature) |
| Test tooling | Stateful in-process mock, HTTP mock server, fault injection, clone, and replay. | [Mock reference](docs/mock-server.md) |

For the exact implemented surface, use the
[per-service operation tables](OPERATIONS.md). An operation absent from those
tables is not claimed as implemented.

## Testing without a camera

Enable `mock` to run client tests against an in-process stateful ONVIF device:

```toml
[dev-dependencies]
oxvif = { version = "0.15", features = ["mock"] }
```

```rust
use std::sync::Arc;
use oxvif::{mock::MockTransport, OnvifClient};

#[tokio::test]
async fn updates_a_mock_camera() {
    let client = OnvifClient::new("http://mock")
        .with_transport(Arc::new(MockTransport::new()));

    client.set_hostname("lab-cam").await.unwrap();
    let hostname = client.get_hostname().await.unwrap();
    assert_eq!(hostname.name.as_deref(), Some("lab-cam"));
}
```

Use the `mock-server` feature when another process needs a real HTTP port. See
the [mock device reference](docs/mock-server.md) for routing, state, supported
operations, fault injection, and limitations.

## Documentation

| Document | Purpose |
| --- | --- |
| [Library and feature guide](LIBRARY_GUIDE.md) | Detailed library usage, service methods, health checks, Mock, and Metamorph. |
| [Rust API documentation](https://docs.rs/oxvif) | Authoritative public types and method signatures. |
| [CLI guide](docs/oxvif-cli.md) | Installation, commands, security, fleet workflows, structured output, and exit codes. |
| [CLI 使用指南](docs/oxvif-cli.zh-TW.md) | Traditional Chinese CLI guide. |
| [Implemented operations](OPERATIONS.md) | Exact per-service ONVIF coverage. |
| [Mock device reference](docs/mock-server.md) | Complete behavior and fidelity contract for the mock. |
| [Support boundaries](docs/support.md) | Versioned platform, security, compatibility, and commercial-claim limits. |
| [Changelog](CHANGELOG.md) | Release history and unreleased changes. |

## Project status

oxvif is suitable for application development, diagnostics, interoperability
testing, and controlled pilots. ONVIF devices vary significantly by vendor and
firmware, so compatibility claims are evidence-based rather than inferred from
the protocol profile alone. Sanitized reports from additional real cameras are
welcome.

The first public CLI release remains approval-gated. Pre-release verification
is documented in the [0.16.0 release notes](https://github.com/smiti1642/oxvif/blob/master/docs/releases/0.16.0.md).

## Contributing and security

Read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting changes. Use the
[compatibility report](https://github.com/smiti1642/oxvif/blob/master/.github/ISSUE_TEMPLATE/compatibility.yml) for camera
results and follow its redaction checklist.

Please report security issues privately according to
[SECURITY.md](SECURITY.md), not through a public issue.

## License

MIT — see [LICENSE](LICENSE).

ONVIF is a trademark of ONVIF, Inc. This project is not affiliated with or
endorsed by ONVIF, Inc.
