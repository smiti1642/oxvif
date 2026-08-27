# oxvif-cli

`oxvif-cli` is the human- and Agent-friendly command-line operation surface
for the [`oxvif`](https://crates.io/crates/oxvif) ONVIF client library.

The package installs an executable named `oxvif`:

```sh
cargo install oxvif-cli --locked
oxvif describe
oxvif describe --output json --non-interactive
oxvif agent guide --output json
```

Root help includes a short Agent onboarding hint. `agent guide` returns the
version-matched operational and security rules embedded in the installed
binary; `agent prompt` prints a compact prompt suitable for Agent instructions.
The structured stdout contract is currently schema version 2.

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
oxvif --timeout 3s discover scan
oxvif --timeout 3s discover scan --interface Ethernet --save factory-scan
oxvif discover snapshots
oxvif discover list factory-scan --filter ip-cidr=192.168.20.0/24
oxvif discover enrich factory-scan --credential-profile factory-admin \
  --filter ip-cidr=192.168.20.0/24 --jobs 16
oxvif discover remove factory-scan
```

`--interface` may be repeated and accepts a local interface name or IPv4
address. Scans are ephemeral unless `--save` is supplied. Discovery filters
include `endpoint`, `uuid`, `type`, `scope`, `xaddr`, `ip-cidr`, and enriched
identity fields. Enrichment uses bounded concurrency and writes only identity
metadata back to the snapshot; `discover scan` and `discover enrich` never add
registered devices.

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

Further discovery, health, media, and PTZ operations are tracked in the
[oxvif CLI plan](https://github.com/smiti1642/oxvif/blob/master/docs/active/oxvif-cli-plan.md).
