# oxvif-cli

`oxvif-cli` is the human- and Agent-friendly command-line operation surface
for the [`oxvif`](https://crates.io/crates/oxvif) ONVIF client library.

For the complete operator and automation reference, read the
[CLI guide](https://github.com/smiti1642/oxvif/blob/master/docs/oxvif-cli.md)
or its
[Traditional Chinese version](https://github.com/smiti1642/oxvif/blob/master/docs/oxvif-cli_zh.md).

The package installs an executable named `oxvif`. Install version 0.16 from
crates.io:

```sh
cargo install oxvif-cli --locked
oxvif --version
```

Repository contributors can install the current workspace version instead:

```sh
cargo install --path crates/oxvif-cli --locked
oxvif describe
oxvif describe --output json --non-interactive
oxvif agent guide --output json
```

APT and Homebrew packages are distributed only through independently verified
channels listed in the project README. Until a native channel is listed, use
crates.io or a checksum-verified portable artifact from the matching GitHub Release.

Root help includes a short Agent onboarding hint. `agent guide` returns the
version-matched operational and security rules embedded in the installed
binary; `agent prompt` prints a compact prompt suitable for Agent instructions.
The structured stdout contract is schema version 3 for the 0.16 release.

## Human quick start

On Windows, macOS, or a Linux desktop session with Secret Service, onboard one
camera with a no-echo password prompt, live verification, native credential
storage, and current-device selection:

```sh
oxvif setup 192.168.1.100 --name "Front Door"
oxvif list
oxvif info
oxvif health
oxvif stream
```

`setup` suggests `front-door` as the immutable ID, then prompts for the ONVIF
username and password. Run `oxvif setup` with no target to discover devices and
choose one from the interactive browser. Automation uses the explicit form
`oxvif setup 192.168.1.100 --id front-door --username admin --password-stdin
--non-interactive`.

One-shot human commands accept an exact canonical ID or `group/local-alias`:

```sh
oxvif test front-door
oxvif profiles taipei-f1/cam-023
oxvif snapshot front-door --profile Profile_1
oxvif health --group taipei-f1 --jobs 16
```

`auth` securely updates a native credential, `list` shows saved cameras with
cached identity, `devices`/`groups`/`views` list inventory, and
`--json`/`--jsonl` abbreviate structured output selection.
These commands map to the same typed requests as the canonical namespaces.
Agents should continue using explicit selectors, canonical command paths,
structured output, and `--non-interactive`.

`oxvif list` is local and deterministic: it displays current selection, ID,
name, address, cached manufacturer/model, firmware, and serial number without
contacting cameras. `oxvif devices` remains compatible. Automation uses `oxvif
device list --output json --non-interactive` for the complete saved records.

## Named devices

Save an endpoint once under an immutable, Agent-safe ID:

```sh
oxvif device add front-door \
  --name "Front Door" \
  --target 192.168.1.100 \
  --tag entrance

oxvif device list
oxvif device show front-door
oxvif use front-door
oxvif current
```

The schema-v3 registry automatically reads schema v1/v2, is process-locked, and
is atomically replaced. Discovery records live in separate atomic files under
`snapshots/`, so ordinary inventory commands do not parse every scan. Its display
name, target, tags, and cached device metadata may change; the device ID does
not. Set `OXVIF_CONFIG_DIR` to isolate the registry for tests or containers.
Default paths and the stop-writers/copy-whole-directory backup and restore
procedure are documented in the
[full CLI manual](https://github.com/smiti1642/oxvif/blob/master/docs/oxvif-cli.md#device-inventory).

```sh
oxvif config path
oxvif config validate --output json --non-interactive
```

## Fleet inventory

Static Groups have explicit membership and a local alias for each member.
Dynamic Views are saved filters evaluated against current device metadata:

```sh
oxvif group create taipei-f1 --name "Taipei F1"
oxvif group member add taipei-f1 front-door --alias cam-023
oxvif device show taipei-f1/cam-023
oxvif use taipei-f1/cam-023

oxvif view create outdoor-geovision \
  --filter tag=outdoor \
  --filter manufacturer:contains=GeoVision \
  --match all
oxvif view evaluate outdoor-geovision --explain --output json
```

`group/local-alias` always resolves exactly one canonical device. Removing a
device removes its Group memberships; it does not remove the Groups or Views.
Device filter fields currently include `id`, `name`, `target`, `uuid`,
`manufacturer`, `model`, `firmware`, `serial`, `tag`, and `ip-cidr`. Operators
use `field[:operator]=value`: `eq`, `neq`, `contains`, `prefix`, and `in`.
Views combine clauses with `--match all` (default) or `--match any`.

WS-Discovery results can be retained and filtered without registering any
devices:

```sh
oxvif discover
oxvif --timeout 3s discover scan
oxvif --timeout 3s discover scan --interface Ethernet --save factory-scan
oxvif --timeout 3s discover refresh factory-scan --interface Ethernet
oxvif discover snapshots
oxvif discover list factory-scan --filter ip-cidr=192.168.20.0/24
oxvif discover list factory-scan --filter registration=saved
oxvif discover scan --filter registration=unregistered --query GeoVision --output json --non-interactive
oxvif discover enrich factory-scan --credential-profile factory-admin \
  --filter ip-cidr=192.168.20.0/24 --jobs 16
oxvif discover remove factory-scan
```

In an interactive terminal, `discover` opens a 12-row paged browser. Use
`j`/`k` or the arrow keys to move, `h`/`l` or Page Up/Page Down to change page,
`/` to search, `c` to clear the search, `i` to inspect full scrollable details,
and Enter or `a` to onboard the selected
unregistered device. A one-line elapsed-time status remains visible while the
scan runs, and differential synchronized frames reduce navigation flicker.
Selected-device setup stays inside the browser while collecting the device ID,
username, and masked password; Esc returns to the discovery list without saving.
The table labels every result as `SAVED`, `NEW`, or `INCOMPLETE`; press `r` for
saved devices, `n` for all unregistered devices, and `A` to restore all results.
Structured output, redirected output, and
`--non-interactive` retain the deterministic non-interactive result.

`--interface` may be repeated and accepts a local interface name or IPv4
address. Scans are ephemeral unless `--save` is supplied. Discovery filters
include `registration`, `endpoint`, `uuid`, `type`, `scope`, `xaddr`, `ip-cidr`,
and advertised or enriched identity fields. Registration filters accept
`saved`, `registered`, `new`, `unregistered`, and `incomplete`. Structured
records expose `registration_status`, optional `registered_device_id`, and
shared status counts. `discover scan`, `discover list`, and `discover refresh`
accept `--query` and use the same cross-field matcher as the browser's `/`
search. Enrichment uses bounded concurrency and writes only identity
metadata back to the snapshot; `discover scan` and `discover enrich` never add
registered devices.

Each saved snapshot exposes a monotonically increasing `generation` plus the
interfaces used for its latest scan. `discover refresh` atomically replaces the
record set, while enrich updates metadata; both increment the generation and
invalidate older import fingerprints.

Bulk import is a fingerprinted plan/apply workflow. A plan is read-only; apply
requires the exact fingerprint from a freshly reviewed plan and atomically
creates devices plus optional Group membership:

```sh
oxvif device import --from factory-scan \
  --filter manufacturer=GeoVision \
  --group taipei-f1 \
  --credential-profile factory-admin \
  --tag discovered \
  --plan --output json

oxvif device import --from factory-scan \
  --filter manufacturer=GeoVision \
  --group taipei-f1 \
  --credential-profile factory-admin \
  --tag discovered \
  --apply --expect-plan sha256:...
```

Exceptional IDs and Group-local aliases can be supplied without secrets in a
versioned JSON file (or via `--overrides-stdin`):

```json
{
  "version": 1,
  "devices": [
    { "endpoint": "urn:uuid:...", "id": "loading-bay", "alias": "cam-042" }
  ]
}
```

Pass it as `--overrides overrides.json` to both plan and apply. The normalized
override document and snapshot generation are included in the plan fingerprint.

## Read-only diagnostics

The 0.16 device surface is diagnostic-only. Every command accepts a saved device
through root `--device` or an ephemeral endpoint through command-level
`--target`; credentials are resolved from the saved device/profile or the
`OXVIF_USERNAME` and `OXVIF_PASSWORD` environment variables.

`--timeout` is a per-network-attempt bound. `--retries` repeats only transient
transport failures with bounded backoff; authentication rejection, invalid
input, deterministic SOAP faults, and parse/schema failures are not retried.
Discovery retries a failed selected interface without repeating interfaces that
already succeeded. `-v` and `-vv` write sanitized policy/timing diagnostics to
stderr only, so JSON and JSONL stdout remain parseable.

Authenticated sessions default to `--clock-sync auto`: the CLI reads device
time and adjusts only its own WS-Security timestamp offset. `always` also does
this for unauthenticated sessions; `never` disables it. No policy changes the
camera clock.

HTTPS cameras using a private CA can add one or more PEM certificates or bundles
with repeatable `--ca-certificate <FILE>`. The CLI merges those certificates
with platform trust roots for diagnostics, health, setup/refresh, enrichment,
and fleet work. Malformed/empty bundles and private-key material are rejected
before connecting; certificate-chain and hostname verification remain enabled.

```sh
oxvif --device front-door device capabilities --output json
oxvif --device front-door device services --output json
oxvif --device front-door media profiles --output json
oxvif --device front-door media stream-uri --profile Profile_1 --output json
oxvif --device front-door media snapshot-uri --profile Profile_1 --output json
oxvif --device front-door ptz status --profile Profile_1 --output json
oxvif --device front-door ptz presets --profile Profile_1 --output json
oxvif --timeout 20s --device front-door health check --output json
```

Returned stream and snapshot URLs have URI userinfo removed. The default health
check performs no write round-trip, liveness fetch, or raw exchange capture.

Group/View fleet diagnostics use bounded concurrency (16 jobs by default, 64
maximum) and always sort results by canonical device ID:

```sh
oxvif --group taipei-f1 --jobs 16 health check --output jsonl
oxvif --view outdoor-geovision media profiles --output json
```

JSONL emits one `fleet_item` line per device followed by one `fleet_summary`
line. Full success exits 0, partial success exits 6, and complete failure emits
the typed `FLEET_FAILED` error. No fleet selection relies on the ambient current
device.

Versioned Draft 2020-12 schemas for the structured envelope and command
descriptors ship under [`schema/`](schema/). Optional fields are additive;
removal, rename, type/meaning changes, or tag/exit semantic changes require a
new structured `schema_version`.

## Credentials

Passwords never enter `devices.toml`. On Windows they are stored in Windows
Credential Manager:

```sh
oxvif device credential set front-door \
  --username admin \
  --password-stdin

oxvif device credential delete front-door
```

One native secret can be shared through an explicitly assigned credential
profile. Groups never imply credential inheritance:

```sh
oxvif credential profile set factory-admin \
  --username admin \
  --password-stdin
oxvif device credential use-profile front-door factory-admin
oxvif credential profile list
```

`OXVIF_USERNAME` and `OXVIF_PASSWORD` support ephemeral automation and direct
targets. Avoid passing a password as a command-line argument or placing it in a
version-controlled file.

Native credential persistence uses Windows Credential Manager, macOS Keychain,
or Linux Secret Service over D-Bus. A missing, locked, or denied native backend
returns `CREDENTIAL_UNAVAILABLE`; oxvif never creates a plaintext credential
fallback. Headless/container automation can use trusted environment injection
for ephemeral operations without persisting the secret. The native lifecycle
contract passes on Windows x64, macOS Intel/Apple Silicon, and Ubuntu
x86_64/aarch64 CI; Linux CI separately proves the unavailable D-Bus mapping.

## Read-only device operations

```sh
oxvif --device front-door device test
oxvif --device front-door device info
oxvif device refresh front-door

# One-shot operation without saving the endpoint:
# With OXVIF_USERNAME/OXVIF_PASSWORD already injected by the environment:
oxvif device info --target 192.168.1.100 --output json --non-interactive
```

Agents should use explicit `--device`, `--output json`, and
`--non-interactive`. `use` and `current` are conveniences for interactive human
sessions; an Agent should not depend on ambient current-device state.

Generate static shell completion without accessing the registry or network:

```sh
oxvif completion bash
oxvif completion zsh
oxvif completion fish
oxvif completion powershell
```

Further discovery, health, media, and PTZ operations are tracked in the
[oxvif CLI plan](https://github.com/smiti1642/oxvif/blob/master/docs/active/oxvif-cli-plan.md).
