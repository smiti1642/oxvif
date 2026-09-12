# Basic multi-camera Mock plan

[English](mock-fleet-basic-plan.md) | [繁體中文](mock-fleet-basic-plan_zh.md)

Status: IMPLEMENTING, authorized 2026-09-12. The first milestone is simple
multi-camera startup and configuration for VMS testing. B1 init/check and three
focused configuration tests pass. B2 HTTP serving, explicit network settings,
member isolation and awaited cleanup pass five Fleet tests plus scoped Clippy.
Shared discovery is still explicitly unavailable until B3.
No release inclusion or 64/256-device stability result is claimed.

| Section | Purpose |
| --- | --- |
| [Scope](#scope) | First milestone and exclusions |
| [Operator workflow](#operator-workflow) | Proposed commands and configuration |
| [Runtime rules](#runtime-rules) | Identity, networking and failure behavior |
| [Work batches](#work-batches) | Files, deliverables and completion checks |
| [Acceptance](#acceptance) | Bounded automated and VMS checks |
| [Decisions](#decisions) | Defaults and conditions requiring another decision |

## Scope

Provide one persistent foreground process running independently configurable
virtual cameras. Reuse `MockServer` and `Fleet`; do not create a process supervisor
or one OS process per camera. Start with four devices, allow explicitly configured
fleets up to 256, and report the actual successfully started device count.

Include basic shared WS-Discovery, because VMS onboarding should not require
manually entering every URL. Default to loopback HTTP without discovery;
LAN HTTP and multicast advertisement require explicit configuration.

Exclude RTSP/video generation, long-duration load orchestration, automatic failure
scenarios, hot reload, an administration TUI, host IP aliases/firewall changes,
containers, and a new published crate. Metamorph brand mixtures are a later stage:
retain per-device configuration boundaries, but do not add unused fixture options
or claim brand emulation now. Existing HTTP Digest and WS-Discovery matching
limitations must remain visible rather than becoming implicit conformance claims.

## Operator workflow

Add a long-running `mock_fleet_serve` example behind `mock-server`, preserving
the existing `mock_server` and short-lived `mock_fleet` example behavior. This
does not add mock-serving dependencies to the installed `oxvif` CLI.

Proposed commands — **not available in the current build**:

```sh
cargo run --example mock_fleet_serve --features mock-server -- init lab.toml --count 4 --base-port 18080
cargo run --example mock_fleet_serve --features mock-server -- check lab.toml
cargo run --example mock_fleet_serve --features mock-server -- serve lab.toml
```

`init` writes an editable versioned TOML manifest without overwriting an existing
file. It generates stable, distinct UUIDs, serials, names and sequential ports;
`--count 64` or `256` changes generation size, not a verified capacity claim.
`check` validates configuration and referenced state files without binding ports
or sending probes. `serve` prints device IDs/URLs and discovery status, remains
running, and shuts down all owned listeners on Ctrl+C.

Manifest contract to implement:

| Location | Fields / semantics |
| --- | --- |
| Root | `schema_version = 1`, `bind_ip` (default `127.0.0.1`), optional concrete `advertise_ip`, `discovery` (default false), `discovery_interface` when discovery is enabled |
| Each `[[devices]]` | Unique `id`, persisted `uuid`, fixed `port`, `name`, `manufacturer`, `model`, `serial_number`, optional `scopes` and `state_file` |
| State source | Optional existing `DeviceState` TOML, relative to the manifest; manifest identity fields override loaded identity, with precedence documented |

Reject unknown fields, unsupported schema versions, empty/duplicate identities,
duplicate/out-of-range ports, and invalid referenced state files. Never silently
replace a malformed state file with factory defaults. Explicit device entries
are authoritative; do not introduce a second count/template expansion system.

Settings take effect at startup. Runtime ONVIF writes affect only that device's
in-memory state; restarting reloads the manifest/state file. First-stage fleet
serving never rewrites source configuration or state files. Existing single-device
persistence remains unchanged. Any later write-back requires its own design.

## Runtime rules

- Add compatible bind/advertise configuration to `MockServerBuilder`; preserve
  current loopback defaults. Fleet mode uses one IP and different ports initially.
  Advertise reachable concrete addresses, never `0.0.0.0` or another host's loopback.
- Require explicit non-loopback `bind_ip` for LAN access. Wildcard binding requires
  a concrete `advertise_ip`; discovery requires an explicitly selected local IPv4
  multicast interface. Fail conflicting combinations with actionable errors.
  No automatic firewall changes, IP configuration or interface guessing.
- These are test services, not production authentication boundaries. Preserve
  existing test-auth behavior and print its actual state at startup; never print
  passwords or present unauthenticated LAN exposure as secure.
- One responder owns UDP 3702 and advertises every started member. Do not start
  256 independent listeners or rely on socket reuse. Keep per-device UUIDs, HTTP
  state and responses isolated. Correlate responses to probes; bound response size
  and work rather than assembling one oversized fleet datagram.
- Before changing discovery behavior, compare the affected response/identity/type
  and scope handling against official ONVIF/WS-Discovery references. Carry forward
  or explicitly refuse unsupported matching; do not silently widen the claim.
- Validate the full manifest before startup. If any requested HTTP bind or discovery
  startup fails, roll back owned listeners and exit nonzero. Do not report a partial
  fleet as successful. Do not terminate unrelated processes occupying those ports.
- Advertise only ready HTTP endpoints; stop advertising when shutting down the
  fleet. Do not claim Hello/Bye, dynamic membership or full lifecycle conformance
  unless implemented and separately verified.

## Work batches

| Batch | Main files / deliverable | Completion check |
| --- | --- | --- |
| B1 Configuration and runner | New `examples/mock_fleet_serve/` parser/runner; `Cargo.toml` example feature gate | Init/check contracts, stable identities, no clobber, malformed/duplicate input rejected; old examples unchanged |
| B2 HTTP fleet and networking | `src/mock/fleet.rs`, `src/mock/server.rs`, exports only where needed | Reuse compatible existing APIs; per-device state/ports and explicit bind/advertise; failed startup rollback and Ctrl+C cleanup |
| B3 Shared discovery | `src/mock/discovery_responder.rs` and fleet integration | One listener advertises multiple ready members; exact identities/URLs, matching boundaries, bounded responses and clean shutdown |
| B4 Acceptance and documentation | Focused fleet/discovery integration tests; paired Mock guide and CLI maintenance links where relevant | Four-device VMS/CLI smoke, optional 64/256 checks, final batched gates; only then document implemented features in changelog/release notes |

Commit each coherent completed batch with its verification and limitations. Run
focused tests during construction; run the combined default/all-feature workspace
tests, all-target Clippy, formatting and strict rustdoc once at final acceptance,
not after every helper or prose edit. Re-run the affected gate after a later code fix.
Keep protocol, state and UI scopes distinct; no unrelated mock-fidelity work enters
this milestone. Reconcile release-cut tracking after implementation, not in advance.

## Acceptance

1. Four configured cameras remain running simultaneously; discovery yields four
   distinct identities and every advertised URL answers `GetDeviceInformation`
   with that device's expected metadata. Repeated probes do not multiply identities.
2. A permitted test-state change on one member does not alter another. Restart
   retains configured UUIDs/ports and reloads original state, as documented.
3. Invalid manifests, occupied HTTP/UDP ports and invalid interfaces produce
   explicit failure without leftover owned listeners or modified input files.
   Check Ctrl+C and subsequent port reuse with deterministic lifecycle tests.
4. Exercise exact multi-member discovery responses through loopback unicast in
   ordinary tests. Run real LAN multicast and VMS onboarding separately on an
   authorized interface; unicast success is not multicast/platform acceptance.
5. Opt-in 64/256-device runs measure startup, distinct discovery, concurrent basic
   reads with timeouts, failures and resource use. Record host and concurrency;
   inspect VMS device identity handling. These are capacity smoke tests, not
   long-duration stability, streaming, event-subscription or brand-compatibility claims.
6. Verify native Windows/Linux/macOS build/network lifecycle where available.
   Record missing native VMS/multicast checks rather than declaring them passed.

## Decisions

Proceed with the small design above; no further choice blocks writing the plan.
Single-process, same-IP/different-port operation and startup-only settings are
implementation defaults, not promises that every VMS supports that topology.

Ask again before adding multiple host IPs if the target VMS deduplicates by IP,
implementing RTSP, enabling persistent runtime write-back, mixing Metamorph fixtures,
or including the feature in a particular release. Confirm the authorized lab
interface and target VMS before LAN acceptance, not by editing host networking.

This is the narrowed first slice of F11 in the
[backlog](post-0.17-backlog.md), following the [CLI re-entry discussion](cli-0.17-reentry.md#mock-discussion).
