# oxvif-cli

`oxvif-cli` is the human- and Agent-friendly command-line operation surface
for the [`oxvif`](https://crates.io/crates/oxvif) ONVIF client library.

The package installs an executable named `oxvif`:

```sh
cargo install oxvif-cli --locked
oxvif describe
oxvif describe --output json --non-interactive
```

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

The registry is versioned, process-locked, and atomically replaced. Its display
name, target, tags, and cached device metadata may change; the device ID does
not. Set `OXVIF_CONFIG_DIR` to isolate the registry for tests or containers.

## Credentials

Passwords never enter `devices.toml`. On Windows they are stored in Windows
Credential Manager:

```sh
oxvif device credential set front-door \
  --username admin \
  --password-stdin

oxvif device credential delete front-door
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
