# oxvif CLI

Professional command-line operations for ONVIF devices, designed for both
interactive operators and automated Agents.

**English** | [繁體中文](oxvif-cli_zh.md)

## Quick navigation

| Section | Purpose |
| --- | --- |
| [Overview](#overview) | Scope, audience, and read-only boundary. |
| [Installation](#installation) | Source installation and public-channel status. |
| [Synopsis and global options](#synopsis) | Command shape, selectors, timeouts, retries, TLS, and output flags. |
| [Quick start](#quick-start) | Add a device, store credentials, and run the first diagnostic. |
| [Device inventory](#device-inventory) | Registry paths, backup, restore, and validation. |
| [Network discovery](#network-discovery) | Scans, interfaces, snapshots, filters, and enrichment. |
| [Bulk import](#fingerprinted-bulk-import) | Reviewable plan/apply workflow and fingerprints. |
| [Groups and Views](#groups-and-views) | Static and dynamic fleet selection. |
| [Credentials](#credentials) | Native stores, headless use, and secret-handling rules. |
| [Read-only diagnostics](#read-only-diagnostics) | Device, media, PTZ, health, and ephemeral targets. |
| [Fleet diagnostics](#fleet-diagnostics) | Bounded concurrency and deterministic aggregation. |
| [Agent contract](#agent-and-automation-contract) | Embedded guidance and command descriptors. |
| [Output and exit codes](#output-formats) | JSON/JSONL schemas, completion, and process status. |
| [Environment variables](#environment-variables) | Supported automation inputs and configuration paths. |

## Overview

`oxvif-cli` is the command-line interface for the [`oxvif`](../README.md)
ONVIF client library. The Cargo package is named `oxvif-cli`; the installed
executable is named `oxvif`.

The CLI provides:

- a local registry of devices identified by stable, operator-assigned IDs;
- retained WS-Discovery snapshots with filtering and enrichment;
- static Groups and dynamic Views for large camera fleets;
- read-only device, media, PTZ, and health diagnostics;
- native operating-system credential storage;
- secure interactive setup and concise daily-operation commands;
- deterministic JSON and JSONL contracts for Agents and automation; and
- typed errors with stable process exit codes.

The 0.1 ONVIF command surface is diagnostic-only. It reads device state but
does not modify device configuration. Registry, Group, View, credential, and
snapshot commands modify local CLI state only.

## Installation

Version 0.1 is not yet published on crates.io. Install the current workspace
version when developing or evaluating this repository:

```sh
cargo install --path crates/oxvif-cli --locked
oxvif --version
```

After the release is published and independently verified, the crates.io
installation command will be:

```sh
cargo install oxvif-cli --locked
```

Non-publishing release staging built all five declared native artifacts. It
also passed signed APT install/remove tests on amd64/aarch64 and Homebrew
formula/bottle tests on Intel and Apple Silicon. No public repository or tap
exists yet. Until the project
[`README`](../README.md#command-line-interface) names verified public channels,
use only the source installation above. The exact staging evidence is recorded
in the [0.16.0 release notes](releases/0.16.0.md#pre-release-verification).

Confirm the available command surface after installation:

```sh
oxvif --help
oxvif describe
```

## Synopsis

```text
oxvif [GLOBAL OPTIONS] <COMMAND> [COMMAND OPTIONS]
```

The primary command groups are:

| Command | Purpose |
| --- | --- |
| `setup`, `auth` | Securely onboard a device or update its native credential. |
| `info`, `test`, `health` | Run common diagnostics against a positional or current device. |
| `profiles`, `stream`, `snapshot` | Use common media operations without the canonical namespace. |
| `devices`, `groups`, `views` | List local inventory with concise plural commands. |
| `agent` | Print version-matched guidance for AI Agents. |
| `describe` | List commands or describe a command as human-readable text or structured data. |
| `device` | Manage saved devices and run device-level diagnostics. |
| `discover` | Scan the network and manage retained discovery snapshots. |
| `group` | Manage static device collections and Group-local aliases. |
| `view` | Manage dynamic, filter-based device collections. |
| `credential` | Manage reusable credentials outside the registry. |
| `media` | Inspect media profiles and obtain read-only media URIs. |
| `ptz` | Inspect PTZ status and presets without moving the camera. |
| `health` | Run read-only device health diagnostics. |
| `use`, `current` | Manage the ambient device selection for interactive sessions. |
| `completion` | Generate Bash, Zsh, Fish, or PowerShell completion. |

Use `oxvif <command> --help` for the authoritative syntax of an installed
version.

## Global options

| Option | Description |
| --- | --- |
| `--output table\|json\|jsonl` | Select terminal, JSON, or newline-delimited JSON output. The default is `table`. |
| `--json`, `--jsonl` | Human shorthands for `--output json` and `--output jsonl`. |
| `--device <ID>` | Select one saved device by canonical ID or `group/local-alias`. |
| `--group <ID>` | Select every explicit member of a static Group for fleet diagnostics. |
| `--view <ID>` | Select every current match of a dynamic View for fleet diagnostics. |
| `--jobs <N>` | Set fleet concurrency. The default is 16 and the maximum is 64. |
| `--non-interactive` | Disable prompts and GUI interaction; fail if required input is unavailable. |
| `--timeout <DURATION>` | Bound each network attempt. Supported units are `ms`, `s`, and `m`; the default is `10s`. |
| `--retries <N>` | Retry transient transport failures up to `N` times with bounded backoff. Authentication rejection, invalid input, deterministic SOAP faults, and parse/schema failures are not retried. The default is zero. Discovery retries each selected interface; it does not duplicate a successful full scan. |
| `--clock-sync <POLICY>` | WS-Security timestamp policy: `auto` (default) reads device time when credentials are present, `always` reads it for every session, and `never` disables client-side synchronization. This never changes the device clock. |
| `--ca-certificate <FILE>` | Add a PEM CA certificate or bundle to platform trust roots. Repeat for multiple bundles. Invalid/empty bundles and private keys are rejected; normal chain and hostname verification remain enabled. |
| `-v`, `--verbose` | Increase diagnostic verbosity; repeat for additional detail. |
| `-q`, `--quiet` | Suppress non-essential diagnostics. |

Root selectors such as `--device`, `--group`, and `--view` are written before
the command. An ephemeral `--target` is a command-level option and is written
after the command:

```sh
oxvif --device front-door device info --output json
oxvif device info --target 192.168.1.100 --output json
```

`--ca-certificate` applies consistently to setup/refresh, single-device
diagnostics, health, discovery enrichment, and fleet items. `-vv` reports only
the number of configured bundles; it does not print certificate contents. The
CLI intentionally has no insecure-certificate or hostname-bypass option.

## Quick start

Securely register, authenticate, verify, and select a device in one command:

```sh
oxvif setup front-door 192.168.1.100 --name "Front Door" --tag entrance --username admin
```

`setup` prompts for a password without echo, verifies the ONVIF connection
before writing local state, stores the secret in the native credential store,
and makes the device current. Daily operations are then concise:

```sh
oxvif info
oxvif test
oxvif health
oxvif profiles
oxvif stream
```

`front-door` remains the canonical ID even if its display name, target, tags,
or cached metadata later change. Passwords are never written to `devices.toml`.
Use `--no-verify` only when intentionally saving an unreachable device, and
`--no-use` when setup must not change the current selection.

The fully explicit canonical workflow remains available for scripts and
advanced composition:

```sh
oxvif device add front-door --target 192.168.1.100 --name "Front Door" --tag entrance
oxvif device credential set front-door --username admin --password-stdin
oxvif --device front-door device test --output json --non-interactive
```

## Device inventory

List and inspect saved devices:

```sh
oxvif devices
oxvif device show front-door
```

Interactive operators may select an ambient current device:

```sh
oxvif use front-door
oxvif current
```

Agents and unattended jobs should not rely on ambient state. They should pass
an explicit selector for every operation.

Set `OXVIF_CONFIG_DIR` to use an isolated registry, for example in tests,
containers, or independent Agent sessions. Registry updates are process-locked
and atomically replaced. Discovery records are stored separately under the
registry's `snapshots` directory.

Without an override, the configuration directory is:

| Platform | Directory |
| --- | --- |
| Windows | `%APPDATA%\oxvif` |
| Linux | `$XDG_CONFIG_HOME/oxvif`, or `$HOME/.config/oxvif` when `XDG_CONFIG_HOME` is unset |
| macOS | `$HOME/Library/Application Support/oxvif` |

The directory contains `devices.toml`, `devices.lock`, and retained records
under `snapshots/`. To back it up, first stop every oxvif process that may write
the registry, then copy the **entire directory** while preserving permissions.
Restore only while no oxvif process is running, and keep the failed/current
directory as a separately named rollback copy. `oxvif config path` reports the
resolved paths without writing them. After restore, run `oxvif config validate
--output json --non-interactive`; it parses the registry and every indexed
snapshot, and returns their counts. oxvif refuses to overwrite a registry whose
version or TOML structure it cannot validate. Unindexed `snapshots/*.json`
files produce an `ORPHANED_SNAPSHOT_FILE` warning and are never deleted
automatically; review backup and registry history before any manual cleanup.

## Network discovery

Run an ephemeral WS-Discovery scan:

```sh
oxvif discover
```

Bare `discover` is equivalent to an ephemeral `discover scan`; it never saves
a snapshot or registers a device. Human output includes row numbers, identity
metadata, current registration matches, and executable next-step examples.

Retain a named snapshot for later inspection:

```sh
oxvif --timeout 3s discover scan --save factory-scan
oxvif discover snapshots
oxvif discover list factory-scan
```

Limit discovery to one or more local interfaces. An interface value may be a
local interface name or IPv4 address:

```sh
oxvif --timeout 3s discover scan --interface Ethernet --interface 192.168.1.20 --save factory-scan
```

Filter a retained snapshot or replace its records with a new scan:

```sh
oxvif discover list factory-scan --filter ip-cidr=192.168.1.0/24
oxvif --timeout 3s discover refresh factory-scan
```

Each snapshot has a monotonically increasing `generation`. Refresh atomically
replaces the record set and increments the generation. A scan or refresh never
registers devices automatically.

### Snapshot enrichment

Discovery normally identifies endpoints without authenticating. Enrichment
uses ONVIF credentials to add identity metadata such as manufacturer, model,
firmware, and serial number:

```sh
oxvif credential profile set factory-admin --username admin --password-stdin
oxvif discover enrich factory-scan --credential-profile factory-admin --filter ip-cidr=192.168.1.0/24 --jobs 16
```

Enrichment uses bounded concurrency and writes only identity metadata back to
the snapshot. It does not register devices.

Discovery filters support `endpoint`, `uuid`, `type`, `scope`, `xaddr`,
`ip-cidr`, and enriched identity fields. The filter grammar is:

```text
field[:operator]=value
```

Supported operators are `eq`, `neq`, `contains`, `prefix`, and `in`.

## Fingerprinted bulk import

Bulk registration is a two-step plan/apply operation. Planning is read-only and
returns a fingerprint for the exact proposed change:

```sh
oxvif device import --from factory-scan --filter manufacturer=GeoVision --group taipei-f1 --credential-profile factory-admin --tag discovered --plan --output json
```

After reviewing the plan, apply the same inputs and supply its complete
fingerprint:

```sh
oxvif device import --from factory-scan --filter manufacturer=GeoVision --group taipei-f1 --credential-profile factory-admin --tag discovered --apply --expect-plan sha256:...
```

Always copy `sha256:...` from a fresh structured plan result. A changed
snapshot generation, filter, Group, tag, credential profile, or override file
invalidates the previous fingerprint. A mismatched apply is rejected before
the registry is modified.

Exceptional device IDs and Group-local aliases may be supplied in a versioned,
secret-free JSON document:

```json
{
  "version": 1,
  "devices": [
    {
      "endpoint": "urn:uuid:...",
      "id": "loading-bay",
      "alias": "cam-042"
    }
  ]
}
```

Pass the same document to both plan and apply:

```sh
oxvif device import --from factory-scan --overrides overrides.json --plan --output json
```

## Groups and Views

### Static Groups

A Group has explicit membership and is appropriate for physical sites, floors,
ownership boundaries, or maintenance batches:

```sh
oxvif group create taipei-f1 --name "Taipei F1"
oxvif group member add taipei-f1 front-door --alias cam-023
oxvif device show taipei-f1/cam-023
```

The qualified selector `taipei-f1/cam-023` resolves to exactly one canonical
device. Group-local aliases provide concise, unambiguous addressing in large
fleets.

### Dynamic Views

A View evaluates saved filters against current device metadata:

```sh
oxvif view create outdoor-geovision --filter tag=outdoor --filter manufacturer:contains=GeoVision --match all
oxvif view evaluate outdoor-geovision --explain --output json
```

Device filters support `id`, `name`, `target`, `uuid`, `manufacturer`, `model`,
`firmware`, `serial`, `tag`, and `ip-cidr`. Multiple clauses use `--match all`
by default; use `--match any` for disjunction. `--explain` reports why each
device did or did not match.

Groups never imply credential inheritance. Assign credentials explicitly to a
device or through a credential profile.

## Credentials

Store a device credential without placing the password in command arguments:

```sh
oxvif auth front-door --username admin
oxvif device credential set front-door --username admin --password-stdin
oxvif device credential delete front-door
```

`auth` uses a no-echo prompt in an interactive terminal. Under
`--non-interactive`, supply `--password-stdin` or inject `OXVIF_PASSWORD` from a
trusted execution environment; the CLI never falls back to a prompt.

Create one reusable native secret and explicitly assign it to devices:

```sh
oxvif credential profile set factory-admin --username admin --password-stdin
oxvif device credential use-profile front-door factory-admin
oxvif credential profile list
```

Secrets are stored in Windows Credential Manager, macOS Keychain, or a Linux
Secret Service provider in the current D-Bus session. The registry contains
only non-secret references. Native-backend errors are mapped to
`CREDENTIAL_UNAVAILABLE` without forwarding backend text that could contain an
account identifier. A missing, locked, or denied backend never causes a
plaintext fallback.

`OXVIF_USERNAME` and `OXVIF_PASSWORD` may be used for ephemeral automation and
direct targets when a trusted execution environment injects them. This is the
recommended non-persistent path for headless Linux/container sessions that do
not provide an unlocked Secret Service. The Windows backend contract is locally
verified; the first public release remains blocked until the same contract
passes native macOS and Linux CI.

Owned password buffers created by the CLI and loaded from the credential store
are zeroized when dropped. This reduces ordinary process-memory lifetime but is
not a guarantee that every copy disappears immediately: environment blocks,
operating-system APIs, allocator internals, crash dumps, and protocol-library
copies can retain secret bytes. Treat the running process and diagnostic dumps
as sensitive, disable unnecessary dumps, and prefer short-lived least-privilege
camera accounts.

Never place a password in a command argument, target URI, log, override file,
or version-controlled configuration.

## Read-only diagnostics

### Device

```sh
oxvif info front-door
oxvif test front-door
oxvif --device front-door device test
oxvif --device front-door device info --output json
oxvif --device front-door device capabilities --output json
oxvif --device front-door device services --output json
```

### Media

```sh
oxvif profiles front-door
oxvif stream front-door
oxvif snapshot front-door
oxvif --device front-door media profiles --output json
oxvif --device front-door media stream-uri --profile Profile_1 --output json
oxvif --device front-door media snapshot-uri --profile Profile_1 --output json
```

Returned stream and snapshot URIs have URI userinfo removed.

If a quick `stream` or `snapshot` command omits `--profile`, the only available
profile is selected automatically. Multiple profiles produce a terminal choice
for an interactive human; `--non-interactive` fails and lists the accepted
tokens instead of guessing. Canonical media commands continue to require an
explicit profile.

### PTZ

```sh
oxvif --device front-door ptz status --profile Profile_1 --output json
oxvif --device front-door ptz presets --profile Profile_1 --output json
```

These commands inspect PTZ state; they do not move the camera.

### Health

```sh
oxvif health front-door
oxvif --timeout 20s --device front-door health check --output json
```

The default health check does not perform a write round-trip, media liveness
fetch, or raw exchange capture.

### Ephemeral targets

Saved registration is optional for one-shot operations. Supply credentials
through the native store or securely injected environment variables:

```sh
oxvif device info --target 192.168.1.100 --output json --non-interactive
oxvif media profiles --target 192.168.1.100 --output json --non-interactive
```

## Fleet diagnostics

Use a Group or View selector to execute a diagnostic across multiple devices:

```sh
oxvif health --group taipei-f1 --jobs 16
oxvif health --view outdoor-geovision
oxvif --group taipei-f1 --jobs 16 health check --output jsonl --non-interactive
oxvif --view outdoor-geovision media profiles --output json --non-interactive
```

Fleet execution has bounded concurrency and always sorts results by canonical
device ID. JSONL emits one `fleet_item` record per device followed by one
`fleet_summary` record.

Fleet completion is represented as follows:

| Outcome | Exit code | Behavior |
| --- | ---: | --- |
| All items succeeded | `0` | Every result is successful. |
| Some items succeeded | `6` | Successful results are retained; inspect failed items and the final summary. |
| All items failed | `20` | The CLI emits the typed `FLEET_FAILED` error. |

Fleet selection never falls back to the ambient current device.

## Agent and automation contract

Root help contains a short Agent onboarding notice. Before operating devices,
an Agent should read the version-matched guide and command descriptors embedded
in the installed executable:

```sh
oxvif agent guide --output json
oxvif describe --output json --non-interactive
oxvif describe media.stream-uri --output json --non-interactive
```

The 0.1 structured output contract uses schema version 3. Automated callers
should:

1. discover the installed command surface with `describe`;
2. use an explicit device, Group, View, or direct target selector;
3. request `json` or `jsonl` and set `--non-interactive`;
4. enforce a suitable timeout and bounded fleet concurrency;
5. inspect both the process exit code and the structured error `code`; and
6. treat fleet exit `6` as partial success rather than discarding valid items.

`agent prompt` provides a compact prompt suitable for inclusion in Agent
instructions:

```sh
oxvif agent prompt
```

## Output formats

Schema version 3 is published with the CLI crate under `schema/` and attached
to release artifacts as `oxvif-envelope.schema.json` and
`command-descriptor.schema.json`. CI validates representative JSON and every
fleet JSONL line against the envelope schema, and validates all command
descriptors against the descriptor schema. Adding optional fields is
compatible; removing/renaming fields, changing their meaning/type, or changing
tag/exit semantics requires a new `schema_version`.

| Format | Intended use |
| --- | --- |
| `table` | Compact interactive terminal output. This is the default. |
| `json` | One stable structured success or error envelope. |
| `jsonl` | Streaming fleet output with one record per line and a final summary. |

Machine-readable stdout remains valid JSON or JSONL. Automation must consume
the structured fields rather than parse human-readable messages.

## Shell completion

Generate a completion script on stdout for installation by the user's shell or
package manager:

```sh
oxvif completion bash
oxvif completion zsh
oxvif completion fish
oxvif completion powershell
```

Completion generation performs no network or registry operation.

## Exit codes

| Code | Meaning |
| ---: | --- |
| `0` | Success. |
| `2` | Invalid argument. |
| `3` | Command, device, or resource not found. |
| `4` | Resource conflict, existing resource, resource in use, or import plan mismatch. |
| `5` | Missing target selector. |
| `6` | Fleet partial success. |
| `10` | Configuration or registry unavailable, corrupt, or unsupported. |
| `11` | Credential unavailable. |
| `20` | Device connection, discovery, or complete fleet failure. |
| `70` | Serialization or internal failure. |

These numeric values are stable automation interfaces. Structured errors also
contain a stable symbolic code such as `DEVICE_NOT_FOUND`,
`IMPORT_PLAN_MISMATCH`, or `FLEET_FAILED`.

## Environment variables

| Variable | Purpose |
| --- | --- |
| `OXVIF_CONFIG_DIR` | Override the local registry directory for isolation or testing. |
| `OXVIF_DEVICE` | Supply a default device for interactive use; explicit selection remains preferable for automation. |
| `OXVIF_USERNAME` | Supply a username for ephemeral automation or direct targets. |
| `OXVIF_PASSWORD` | Supply a password from a trusted execution environment. |

## Further reference

The installed executable is the authoritative reference for its version:

```sh
oxvif --help
oxvif device --help
oxvif describe --output json
oxvif describe health.check --output json
oxvif agent guide --output json
```

See the [`oxvif-cli` package README](../crates/oxvif-cli/README.md) for
package-level details. Switch to the [Traditional Chinese guide](oxvif-cli_zh.md)
for this document in Traditional Chinese.
