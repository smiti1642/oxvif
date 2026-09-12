# Configurable Mock Fleet

[English](mock-fleet.md) | [繁體中文](mock-fleet_zh.md)

Development feature; not included in the published 0.16.0 package. Run multiple
independent ONVIF test devices in one foreground process, using one IPv4 address
and different HTTP ports. This runner is a Rust example, not an installed `oxvif`
CLI command. It does not provide RTSP streams or a production security boundary.

| Section | Contents |
| --- | --- |
| [Quick start](#quick-start) | Generate, validate and serve four cameras |
| [Configuration](#configuration) | Stable identities and initial state |
| [LAN discovery](#lan-discovery) | Explicit interface and exposure settings |
| [Lifecycle and limits](#lifecycle-and-limits) | Cleanup, persistence and protocol boundaries |
| [Validation](#validation) | Reproducible checks and remaining acceptance |
| [References](#references) | Implementation plan and protocol sources |

## Quick start

From a checkout containing this example:

```sh
cargo run --example mock_fleet_serve --features mock-server -- init lab.toml --count 4 --base-port 18080
cargo run --example mock_fleet_serve --features mock-server -- check lab.toml
cargo run --example mock_fleet_serve --features mock-server -- serve lab.toml
```

`init` refuses to overwrite an existing file. `check` validates settings without
opening listeners. `serve` prints the actual device count, discovery/authentication
status and each device URL, then runs until Ctrl+C. Default settings allow only
local HTTP access and disable discovery. In another terminal, for example:

```sh
oxvif device info --target http://127.0.0.1:18080/onvif/device --json --non-interactive
```

Use the URLs printed by your runner. An installed CLI can query the example;
it does not need the `mock-server` feature. Stop the process before changing
settings. Reuse the same manifest to preserve identities on restart.

## Configuration

`init --count 64` or `--count 256` generates a larger manifest; this is not a
promise of VMS capacity. Counts must be 1–256 and the sequential port range must
fit in 1–65535. Every `[[devices]]` entry is authoritative; there is no additional
runtime template expansion.

| Field | Meaning |
| --- | --- |
| Root `schema_version` | Required; currently `1` |
| Root `bind_ip` | Local IPv4 HTTP interface; default `127.0.0.1` |
| Root `advertise_ip` | Optional concrete local IPv4 address; required with wildcard binding |
| Root `discovery` | Default `false`; enable the shared responder explicitly |
| Root `discovery_interface` | Required when discovery is enabled; must match the advertised, non-loopback local address |
| Device `id` | Unique ASCII identifier, used as the initial hostname |
| Device `uuid` | Persisted unique UUID used as the discovery endpoint identity |
| Device `port` | Unique fixed HTTP port |
| Device `name` | Name advertised through an encoded ONVIF name scope |
| Device `manufacturer`, `model`, `serial_number` | Device-information identity; serial numbers must be unique |
| Device `scopes` | Optional absolute scope URIs, advertised but not used for scope-filter matching |
| Device `state_file` | Optional existing `DeviceState` TOML; relative paths resolve from the manifest directory |

The manifest rejects unknown fields, invalid/duplicate identities and invalid
network combinations. Manifest and referenced state files are each limited to
4 MiB. A missing or malformed referenced file is an error, not a request to use
factory defaults. State files use the existing `DeviceState` parser.

Without `state_file`, factory state is used. After loading, the manifest overrides
hostname, manufacturer, model, serial number and scopes (including the generated
name scope). Other loaded fields are retained. Input files are read-only: runtime
ONVIF writes change only that member's memory and are discarded on restart.

## LAN discovery

Use only an isolated, authorized test network. The runner does **not enforce
authentication**, and exposing HTTP also exposes the mock control endpoints.
It never changes firewall rules or assigns host addresses.

Edit the generated root settings **before the first `[[devices]]` table**. Replace
the documentation address below with the IPv4 address already assigned to your
test interface; do not append a second copy of existing TOML keys:

```toml
schema_version = 1
bind_ip = "192.0.2.10"
advertise_ip = "192.0.2.10"
discovery = true
discovery_interface = "192.0.2.10"
```

Run `check`, then `serve`. One UDP 3702 listener advertises the ready HTTP members
with distinct UUIDs and their actual device URLs. A port conflict is fatal and
rolls back the fleet; it never terminates another process. The advertised IP must
be local and concrete. With a concrete bind address it must match that address;
`0.0.0.0` binding requires an explicit advertised address.

The responder supports basic, unscoped WS-Discovery Probe requests, including
namespace-qualified NetworkVideoTransmitter/Device type filters. **Nonempty Scopes
or explicit scope-matching attributes are declined.** If your VMS sends scoped
probes, it may discover nothing; test manual URL onboarding separately. A VMS
that identifies devices only by IP may also collapse this same-IP fleet.

## Lifecycle and limits

- All configuration is validated before startup. An HTTP bind or shared discovery
  startup failure shuts down already-created members and returns a nonzero exit.
- Ctrl+C stops discovery before awaiting HTTP shutdown. There is no hot reload,
  dynamic membership, automatic state write-back or process supervision.
- Shared discovery bounds packet sizes to 16 KiB, sends separate member responses
  and suppresses recent duplicate probes. This is not full WS-Discovery conformance:
  no scope matching, Hello/Bye, Resolve or persisted boot counter is implemented.
- The existing `mock_server` persistence example and short-lived `mock_fleet`
  example retain their behavior. Standalone `MockServerBuilder::discoverable`
  remains best-effort, with legacy matching; it does not share listeners. Use
  `FleetBuilder::discoverable` for the shared, fail-closed startup path.
- Library callers can configure members with `FleetBuilder::member`, enumerate
  `Fleet::device_urls`, and await `Fleet::shutdown`. Default HTTP remains loopback.
- No RTSP/video generation, HTTP Digest, multi-brand Metamorph configuration,
  multiple-host-IP management or long-duration stability guarantee is provided.
  General Mock fidelity limits still apply; see the [Mock reference](mock-server.md).

## Validation

The recorded Windows batch completed with **1,316 passed, 0 failed and 6 ignored**
under all features; the default-feature batch completed with **1,204 passed,
0 failed and 6 ignored**. Ignored tests are skipped, not failed or passed. The
capacity test was run separately and passed; the other five were not executed
in this batch. See the [six-item breakdown](active/mock-fleet-basic-plan.md#ignored-tests)
for prerequisites and the two unverified documentation examples. These counts
do not establish native LAN multicast, VMS or cross-platform acceptance.

Focused configuration and lifecycle tests cover invalid inputs, no-clobber,
independent state, occupied HTTP/UDP rollback, exact shared-probe identities/URLs,
duplicate handling and cleanup. The optional capacity check is deliberately ignored
in normal test runs:

```sh
cargo test --features mock-server --lib configured_fleet_capacity_smoke -- --ignored --nocapture
```

The CI test matrix separately runs the example's configuration tests with
`cargo test --example mock_fleet_serve --all-features --locked`; ordinary workspace
tests do not automatically execute tests inside examples.

It starts 64, then 256 devices on Windows in the recorded local run, collects
distinct loopback-unicast announcements and performs 64/256 concurrent basic
device-information reads with deadlines. It does not measure native multicast,
VMS onboarding, streaming, sustained load or peak memory/CPU. See the
[implementation evidence](active/mock-fleet-basic-plan.md#local-evidence).

For manual acceptance, start with four devices on an authorized interface. Check
that your VMS lists four distinct identities, connects to all four URLs, retains
their identity after restart, and reconnects after Ctrl+C/restart. Only then try
64 or 256 while recording VMS connection counts, failures and host resources.
Native LAN/VMS and Linux/macOS acceptance remain separate from local unit tests.

## References

The bounded discovery implementation was reviewed against
[ONVIF Core §7.3](https://www.onvif.org/specs/core/ONVIF-Core-Specification.pdf) and
[WS-Discovery April 2005 §2.4, §5.3 and Appendix I](https://specs.xmlsoap.org/ws/2005/04/discovery/ws-discovery.pdf).
These references do not certify the simplified responder. The remaining scope
and lifecycle gaps above are intentional first-stage exclusions.
