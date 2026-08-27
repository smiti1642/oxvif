# oxvif-cli

`oxvif-cli` is the human- and Agent-friendly command-line operation surface
for the [`oxvif`](https://crates.io/crates/oxvif) ONVIF client library.

The package installs an executable named `oxvif`:

```sh
cargo install oxvif-cli --locked
oxvif describe
oxvif describe --output json --non-interactive
```

The initial development stage contains the typed command and output contracts.
Device discovery, the named-device registry, diagnostics, and media/PTZ
operations will land in the stages recorded in
the [oxvif CLI plan](https://github.com/smiti1642/oxvif/blob/master/docs/active/oxvif-cli-plan.md).
