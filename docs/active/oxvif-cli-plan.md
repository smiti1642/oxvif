# oxvif CLI — human and Agent operation surface plan

**Status:** active. Written 2026-08-26 from the product discussion that
established the package boundary, human/Agent contract, and named-device
registry. **Stage 0 landed as `9590663` on 2026-08-27. Stage 1 was completed on
2026-08-27, and the first Stage 2A fleet-inventory slice was completed the same
day.** The workspace, typed command/result contract,
named-device registry, Windows credential backend, human and Agent renderers,
and first read-only live-device operations are present. Command spelling that
is explicitly marked provisional may still change before the first release.

**Package:** crates.io package `oxvif-cli`, installed binary `oxvif`.

**Repository decision:** keep the CLI in this repository as a workspace member.
`cargo install oxvif-cli` depends on the crates.io package name, not on the CLI
having a separate Git repository. The library and CLI therefore remain
independently publishable without giving up atomic library/CLI changes and the
existing mock-based integration tests.

---

## 1. Product decision

The CLI is not merely a human-friendly wrapper around selected
`OnvifClient` methods. It is the stable ONVIF operation surface shared by:

- humans working interactively in a terminal;
- Agents invoking a local command and consuming structured results;
- CI and fleet automation;
- a possible future MCP server built on the same application layer.

The first release is **diagnostic-first**: discover, register, identify,
inspect, obtain media URIs, read PTZ state, and run health checks. Mutating
device operations follow only after the output contract, error taxonomy, and
safety model are stable.

The CLI's durable workflow scales from one camera to a fleet:

```text
discover scan -> save snapshot -> filter/enrich -> import plan/apply
-> organize as groups/views -> select one device or a set -> execute operation
```

A named-device registry is part of the MVP. Without it, every invocation must
repeat an address and credentials and the CLI remains a stateless example
runner instead of a useful operating environment.

---

## 2. Decisions taken

| # | Question | Decision |
|---|---|---|
| 1 | New repository? | **No.** Add `crates/oxvif-cli` to this repository. |
| 2 | Package and executable names? | Package `oxvif-cli`; executable `oxvif`. |
| 3 | Human-only CLI with JSON added later? | **No.** Human and Agent contracts are first-class from the start. |
| 4 | Separate Agent commands? | **No.** One command model; Agents request structured output and non-interactive behavior. |
| 5 | Persist known cameras? | **Yes.** A named-device registry is an MVP capability. |
| 6 | Stable reference key? | A machine-safe immutable device ID, separate from the mutable human display name. |
| 7 | Global active device for Agents? | **No.** Humans may use `use`; Agents should select explicitly with `--device`. |
| 8 | Store passwords in the registry? | **Never.** Store only a credential reference; secrets live in an OS credential store. |
| 9 | First-release writes? | Avoid them. Start with a high-value read-only surface. |
| 10 | MCP now? | Not in the MVP, but keep the application layer reusable so MCP does not reimplement behavior. |
| 11 | How are 200+ cameras organized? | Static Groups hold explicit membership; dynamic Views hold saved filters. The two must not be conflated. |
| 12 | How is one camera selected inside a Group? | `group-id/local-alias` resolves exactly one device while the immutable global device ID remains canonical. |
| 13 | Can discovery results be reused? | Yes. A named discovery snapshot preserves one scan until explicitly refreshed or removed. |
| 14 | Can filtered discoveries be registered in bulk? | Yes, through explicit `device import --plan` and `--apply`; scanning itself never mutates the registry. |
| 15 | How are credentials reused across many cameras? | Named credential profiles may be referenced by many devices. Groups never imply credential inheritance. |
| 16 | How does an installed Agent learn safe operation rules? | Root help points to embedded, version-matched `agent guide` and `agent prompt` commands; `describe` remains the machine contract. |
| 17 | May the pre-release structured contract change? | Yes. The UX correction raises stdout schema to v2 before the first public CLI release. |
| 18 | Which selectors may delete a registry device? | `device remove` requires the immutable global ID; read/live and reversible registry operations may use `group/local-alias`. |
| 19 | Where are discovery records persisted? | Registry v3 indexes snapshots stored as separate atomic files; v2 embedded snapshots migrate without data loss. |
| 20 | What does discovery do by default? | `discover scan` is ephemeral; `--save` is an explicit local write. Partial interface failure is a successful result with warnings; total failure is typed failure. |
| 21 | What is the first filter language? | Typed `field[:operator]=value`, `eq/neq/contains/prefix/in`, and explicit `--match all|any`; no implicit firmware version ordering. |

---

## 3. Repository and crate shape

The existing root remains the `oxvif` library package and becomes a workspace
root. The initial shape is:

```text
oxvif/
|-- Cargo.toml
|-- src/                         oxvif library
`-- crates/
    `-- oxvif-cli/
        |-- Cargo.toml           package = "oxvif-cli"
        `-- src/
            |-- lib.rs           reusable application surface
            |-- main.rs          thin CLI adapter
            |-- commands/
            |-- registry/
            |-- credential/
            |-- output/
            `-- error.rs
```

The CLI manifest publishes a differently named executable:

```toml
[package]
name = "oxvif-cli"

[[bin]]
name = "oxvif"
path = "src/main.rs"

[dependencies]
oxvif = { version = "0.15", path = "../.." }
```

The `version` is required for publication; `path` is the local workspace
override. Release order is library first, CLI second. Installation becomes:

```sh
cargo install oxvif-cli --locked
oxvif --version
```

`main.rs` is responsible only for argument parsing, constructing a typed
command request, invoking the application layer, rendering the result, and
returning an exit code. Argument-parser types must not leak into command
execution. A future MCP adapter consumes the same typed requests and results.

---

## 4. Named-device registry

### 4.1 Identity model

Every saved device has at least four distinct identifiers or labels:

| Field | Example | Semantics |
|---|---|---|
| `id` | `front-door` | Immutable, unique, machine-safe key used by Agents and scripts. |
| `name` | `前門攝影機` | Mutable display name for humans. |
| `device_uuid` | `uuid:abcd...` | ONVIF-reported physical-device identity when available. |
| `target` | `http://192.168.1.100/onvif/device_service` | Current connection endpoint; mutable and not identity. |

The ID grammar is deliberately narrow and shell-safe:

```text
[a-z0-9][a-z0-9_-]*
```

Changing a display name does not invalidate automation:

```sh
oxvif device rename front-door --name "大廳入口"
```

An address change updates `target`; it does not create a new device. A UUID or
serial-number mismatch at a previously known address must produce a warning
that the physical device may have been replaced.

### 4.2 Registry commands

The canonical registry surface is:

```sh
oxvif device add <id>
oxvif device list
oxvif device show <id>
oxvif device update <id>
oxvif device rename <id>
oxvif device remove <id>
oxvif device test <id>
oxvif device refresh <id>
```

Initial registration:

```sh
oxvif device add front-door \
  --name "前門攝影機" \
  --target 192.168.1.100 \
  --username admin \
  --password-stdin
```

Discovery can feed registration without copying endpoint details by hand:

```sh
oxvif discover --output json
oxvif device add front-door --from-discovery uuid:abcd
```

Interactive discovery may offer to save a selected result. Under
`--non-interactive`, it must never prompt and must require an explicit result
identity and device ID.

### 4.3 Selection and conda-like convenience

Humans may select a current device:

```sh
oxvif use front-door
oxvif current
oxvif device info
oxvif health check
```

Explicit selection is always available and is the required Agent practice:

```sh
oxvif --device front-door device info
oxvif --device front-door media stream-uri
oxvif --device front-door ptz status
```

Ambient global state is unsafe for concurrent Agents: one Agent can run
`use front-door` while another runs `use warehouse`. Target resolution must
therefore have a documented precedence:

```text
--device
-> OXVIF_DEVICE
-> project-local current device, if supported
-> user current device
-> structured MISSING_TARGET error
```

An Agent should normally use `--device` even when a current device exists.
Tests must cover two concurrent invocations selecting different devices and
prove that neither reads or changes the other's target.

### 4.4 Stored shape

The user registry lives in the platform-appropriate application configuration
directory. On Windows this resolves beneath `%APPDATA%\oxvif`; code must not
hard-code that path for other platforms. A versioned TOML representation may
look like:

```toml
version = 1
current_device = "front-door"

[devices.front-door]
name = "前門攝影機"
target = "http://192.168.1.100/onvif/device_service"
device_uuid = "uuid:abcd"
manufacturer = "Hikvision"
model = "DS-2CD2043G2"
serial_number = "redacted-example"
credential_ref = "oxvif/device/front-door"
tags = ["entrance", "outdoor"]
```

Rules:

- The registry never contains a password, token, digest, or URL-embedded
  credential.
- `credential_ref` identifies a secret in the OS credential store; it is not
  itself secret.
- Manufacturer, model, endpoint, service URLs, and last-seen information are
  cached observations. `device refresh` revalidates them.
- Import/export excludes credentials. Export is safe to inspect and commit
  only after explicit redaction tests pass.
- Registry writes are atomic. A crash must not truncate the inventory.
- The file has a schema version and migrations; incompatible newer versions
  fail clearly instead of being overwritten.

### 4.5 Credentials

The preferred enrollment flow accepts a username as ordinary configuration
and a password through stdin:

```sh
oxvif device add front-door --username admin --password-stdin
```

Environment variables may support ephemeral automation:

```text
OXVIF_USERNAME
OXVIF_PASSWORD
```

Passing `--auth admin:password` is not the documented default because command
arguments appear in shell history and process listings. Renderer, trace, and
error tests must prove that credentials cannot appear in stdout, stderr,
registry files, or diagnostic bundles.

The credential backend is an abstraction. The first supported OS backend may
be Windows, but command and registry models cannot encode a Windows-only API.

### 4.6 Fleet inventory, Groups, Views, and discovery snapshots

The inventory model keeps four concepts distinct:

| Concept | Meaning | Membership changes automatically? |
|---|---|---|
| Device | One registered physical camera with an immutable global ID. | No. |
| Group | An explicitly managed static set, such as a site or floor. | No. |
| View | A named filter evaluated against current registered-device metadata. | Yes. |
| Discovery snapshot | The preserved result of one network scan, including unregistered devices. | Only when explicitly refreshed. |

A Group may assign a unique local alias to each member. `--device
taipei-f1/cam-023` resolves exactly one camera; `--group taipei-f1` resolves a
set and is accepted only by commands with defined batch semantics. A device
may belong to multiple Groups and have a different local alias in each. Global
device IDs remain immutable and are the canonical identity returned to Agents.

Discovery, enrichment, and registration are separate operations:

```sh
oxvif discover scan --save factory-scan
oxvif discover list factory-scan --filter ip-cidr=192.168.20.0/24
oxvif discover enrich factory-scan --credential-profile factory-admin
oxvif device import --from factory-scan --filter manufacturer=GeoVision \
  --group taipei-f1 --credential-profile factory-admin --plan
oxvif device import --from factory-scan --filter manufacturer=GeoVision \
  --group taipei-f1 --credential-profile factory-admin --apply
```

Native discovery filters may use interface, endpoint/IP, UUID, ONVIF type, and
scope because WS-Discovery supplies those fields. Manufacturer, model,
firmware, serial number, capabilities, and health are enriched fields and must
not appear available until authenticated follow-up calls populate them. Human
flags and Agent requests compile to the same typed filter expression.

A named credential profile stores one secret in the OS credential backend and
can be referenced by many devices. The registry stores only the profile
reference and username. A device may override its profile explicitly. Groups
never provide implicit credential inheritance because a device can belong to
multiple Groups with conflicting defaults.

`discover scan`, `discover list`, and `discover enrich` never add registry
devices. Bulk import always produces a deterministic plan before an explicit
apply. The plan includes proposed global IDs, Group-local aliases, matched and
skipped records, duplicate UUIDs, endpoint conflicts, and credential
references, but never secret material.

#### Stage 2A import/enrichment contract — locked 2026-08-27

The next implementation slice closes the discovery-to-inventory loop with
these rules:

1. `discover enrich SNAPSHOT --credential-profile PROFILE [--filter ...]`
   authenticates each selected usable XAddr with bounded concurrency and
   atomically replaces that snapshot. Per-device failures are warnings; total
   failure is typed. Secrets remain in the native credential backend and never
   enter snapshot JSON.
2. `device import --from SNAPSHOT ... --plan` is read-only and returns every
   source record as `create`, `already_present`, `filtered_out`, or `conflict`.
   Ordering, proposed IDs, aliases, names, and targets are deterministic.
3. IDs prefer the WS-Discovery UUID, then the endpoint, then the target host;
   they are normalized to `[a-z0-9][a-z0-9_-]*`. Display names prefer the
   ONVIF name scope, enriched model, target host, then the proposed ID.
4. Existing UUID or normalized-target matches are `already_present` and are
   never silently overwritten. An occupied proposed ID, duplicate identity,
   invalid/missing target, Group-alias collision, or incompatible credential
   assignment is an explicit conflict.
5. A plan carries a SHA-256 fingerprint. `--apply` requires
   `--expect-plan FINGERPRINT`, recomputes and validates the plan while holding
   the registry lock, rejects stale/conflicting plans before any mutation, and
   creates devices plus Group membership in one atomic registry replacement.
6. Applying again is safe: the next plan reports imported devices as
   `already_present`. Import never deletes or updates an existing device.

Canonical flow:

```sh
oxvif discover enrich factory-scan --credential-profile factory-admin
oxvif device import --from factory-scan --filter manufacturer=GeoVision \
  --group taipei-f1 --credential-profile factory-admin --plan --output json
oxvif device import --from factory-scan --filter manufacturer=GeoVision \
  --group taipei-f1 --credential-profile factory-admin --apply \
  --expect-plan sha256:...
```

---

## 5. Operational command model

Commands are organized by user-facing ONVIF domain rather than exposing SOAP
method names as the primary interface:

```sh
oxvif discover
oxvif device info
oxvif device capabilities
oxvif device services
oxvif media profiles
oxvif media stream-uri
oxvif media snapshot-uri
oxvif ptz status
oxvif ptz presets
oxvif health check
```

All target-taking commands accept a saved ID via `--device`. Direct targets
remain important for one-shot use:

```sh
oxvif --target 192.168.1.100 device info
oxvif --target http://192.168.1.100/onvif/device_service device info
```

`--device` and `--target` are mutually exclusive. A bare IP may be normalized
or resolved, but the resolved endpoint must be reported in result metadata;
the CLI must not silently hide an incorrect endpoint guess.

The implementation should extract reusable behavior from the existing
examples rather than rename `examples/camera.rs` into the production binary.
That example currently mixes parsing, connection setup, behavior, and human
rendering and is a source of operations to migrate, not the target
architecture.

---

## 6. Human and Agent contract

There is one semantic command surface. Presentation and interaction are
explicit policies:

```text
--output table|json|jsonl
--non-interactive
--timeout <duration>
--retries <count>
--verbose
--quiet
```

There is no vague `--agent` switch. Agents request `--output json` or
`--output jsonl` plus `--non-interactive`. Non-interactive execution fails
immediately with a structured error if required input or confirmation is
missing.

### 6.1 Self-description

Agents must not need to scrape human help text:

```sh
oxvif describe --output json
oxvif describe media.stream-uri --output json
```

Description output includes the command's argument schema, result schema,
authentication requirement, risk level, retry behavior, and whether it can
change device state.

### 6.2 Stable result envelope

Every structured success uses the same top-level contract:

```json
{
  "schema_version": "3",
  "ok": true,
  "data": {},
  "warnings": [],
  "meta": {
    "device_id": "front-door",
    "target": "http://192.168.1.100/onvif/device_service",
    "elapsed_ms": 142
  }
}
```

Errors are data, not prose requiring interpretation:

```json
{
  "schema_version": "3",
  "ok": false,
  "error": {
    "code": "AUTH_CLOCK_SKEW",
    "message": "Device clock differs by 312 seconds",
    "retryable": true,
    "suggested_action": "Retry with clock synchronization enabled"
  }
}
```

Human-readable data goes to stdout, diagnostics to stderr. Structured mode
must remain valid JSON even when verbose diagnostics are enabled. Exit codes
are documented and stable; an Agent never needs to parse an English error to
distinguish authentication, reachability, unsupported service, invalid input,
or a dangerous operation requiring authorization.

JSONL is required for discovery and future fleet operations so partial results
can stream without waiting for every target.

---

## 7. Safety model for later writes

Each command declares one of three risk classes:

| Risk | Examples | Default policy |
|---|---|---|
| read | info, profiles, health, PTZ status | Allowed. |
| write | preset changes, imaging settings | Explicit write authorization. |
| dangerous | network changes, reboot, factory reset, firmware | Plan/apply plus explicit confirmation. |

Dangerous commands should produce a typed plan before application. A plan
records the target's stable identity, before/after values, expected disconnect
or reboot, reversibility, expiry, and a plan ID. It must not apply to a device
whose identity no longer matches.

Continuous PTZ motion must require a duration or another bounded stop policy so
an interrupted Agent cannot leave a camera moving indefinitely. Retryable
errors and idempotency rules must be part of the structured command contract
before write commands ship.

---

## 8. Delivery stages

### Stage 0 — workspace and contracts — complete 2026-08-26

- Convert the repository into a workspace without changing the published
  `oxvif` library package.
- Add `crates/oxvif-cli` with a thin binary and reusable library surface.
- Define typed command request/result, structured envelope, error codes, exit
  codes, output policy, and `describe` schema.
- Add snapshot tests for human, JSON, and error output.

**Exit:** `cargo run -p oxvif-cli -- --help`, `describe --output json`, and all
existing library feature combinations pass.

Delivered and verified:

- `cargo test -p oxvif-cli`: 10 tests passed across unit, binary-unit, and CLI
  integration suites.
- `cargo test -p oxvif`: 542 passed, 1 ignored under the library's default
  feature set.
- `cargo test --workspace --all-features`: 955 passed, 3 ignored, 0 failed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: no
  findings.
- `cargo package -p oxvif-cli --allow-dirty`: packaged 13 files and rebuilt the
  package against the crates.io `oxvif 0.15.0` dependency successfully.
- `cargo rustdoc -p oxvif-cli --lib -- -D warnings`: public API documentation
  built without warnings.
- `cargo fmt --all -- --check` and `git diff --check`: clean.

### Stage 1 — registry and credentials — complete 2026-08-27

- Implement versioned, atomic device-registry storage.
- Implement `device add/list/show/update/rename/remove/test/refresh`.
- Implement immutable IDs, mutable display names, UUID/serial replacement
  detection, and credential references.
- Implement `use`, `current`, `--device`, `--target`, and documented resolution
  precedence.
- Implement the credential abstraction and at least the target platform's
  secure backend.
- Add concurrency, migration, atomic-write, corruption, and secret-redaction
  tests.

**Exit:** a direct device endpoint can be saved once, selected by stable ID,
and reused without a password appearing in arguments or the registry.
Discovery feeds registration in Stage 2; making it a Stage 1 exit condition
would contradict the stage ordering.

Delivered:

- `devices.toml` schema version 1 under the platform user configuration
  directory, with `OXVIF_CONFIG_DIR` isolation for tests and containers.
- Cross-process `fs2` locking plus `atomic-write-file` replacement; eight
  concurrent CLI writers are covered by an integration test.
- Immutable `[a-z0-9][a-z0-9_-]*` IDs, mutable display name/target/tags, cached
  device information, and a serial-change replacement warning.
- `device add/list/show/update/rename/remove`, `use`, and `current`.
- Credential abstraction with a Windows Credential Manager backend and an
  in-memory test backend. The registry holds a credential reference and
  username, never the password.
- `device credential set/delete`; secret-bearing request types redact `Debug`
  and are intentionally not `Clone`.
- `device test/info/refresh` by explicit `--device`, direct `--target`,
  `OXVIF_DEVICE`, or current device, in that order where applicable.
- Timeouts, bounded retries, stable error codes, JSON/JSONL results, and target
  identity metadata on the first live-device vertical slice.
- A Windows real-device run stored the password in Credential Manager, removed
  password environment variables, then passed `use`, `device test`,
  `device info`, `device refresh`, `device list`, and `device remove` against a
  GeoVision GV-GDRN4800-2F. The temporary registry and credential were removed.

Verified after the final Stage 1 change:

- `cargo +1.88.0 check -p oxvif-cli`: clean at the corrected workspace MSRV.
- `cargo test -p oxvif-cli`: 22 passed across library, binary, and CLI process
  suites.
- `cargo test --workspace --all-features`: 967 passed, 3 ignored, 0 failed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: no
  findings.
- `cargo rustdoc -p oxvif-cli --lib -- -D warnings`: clean.
- `cargo package -p oxvif-cli --allow-dirty`: 15 files packaged and rebuilt
  successfully against the crates.io `oxvif 0.15.0` dependency.
- `cargo fmt --all -- --check` and `git diff --check`: clean.

### Stage 2A — discovery and fleet inventory foundation

- Add named discovery snapshots with explicit refresh/removal lifecycle.
- Add typed native and enriched filters with deterministic JSON rendering.
- Add static Groups, dynamic Views, and unique Group-local aliases.
- Resolve `--device <group-id>/<local-alias>` to exactly one canonical device;
  reserve `--group` and `--view` for commands with batch semantics.
- Add named credential profiles that many devices can reference without Group
  inheritance or duplicated secrets.
- Discovery enrichment and bulk `device import --plan/--apply`, including UUID
  deduplication, endpoint conflicts, ID/alias proposals, and redaction.
  **Implemented 2026-08-27** with bounded enrichment, SHA-256 plan binding,
  stale-plan rejection, and atomic registry apply.
- Test at least a 205-device mock inventory, concurrent readers/writers, stable
  filter ordering, duplicate identities, and interrupted imports.

**Exit:** a 205-device discovery can be saved, filtered, enriched, previewed,
imported, grouped, viewed dynamically, and resolved as either one exact camera
or an explicit set without leaking credentials or silently mutating inventory.

First slice delivered 2026-08-27:

- Registry schema v2 automatically loads schema v1 and persists the migration
  on the next atomic write. New maps remain deterministic `BTreeMap` data.
- Static Group CRUD and explicit member add/remove with unique local aliases.
  `group/local-alias` resolves exactly one canonical device for `use`, show,
  and live read operations. Removing a device cleans all Group memberships.
- Dynamic View CRUD and evaluation with typed `field=value` filters over ID,
  name, target, UUID, manufacturer, model, firmware, serial, tag, and IP CIDR.
- Named discovery snapshot scan/list/filter/remove/enrich. Native filters cover
  endpoint/UUID, type, scope, XAddr, and IP CIDR; enriched identity fields are
  populated through a native credential profile with per-device warnings.
- Reusable credential-profile set/list/show/delete and explicit per-device
  assignment. A profile secret lives once in the native credential store;
  deletion is refused while referenced and Groups never inherit credentials.
- Registry-load integrity checks reject missing current devices, dangling
  Group members, duplicate members, and missing credential profiles.
- A generated 205-device inventory test verifies deterministic dynamic View
  evaluation. Schema migration, Group cleanup/conflicts, snapshot
  sorting/deduplication/filtering, credential redaction, and CLI selection are
  covered by focused tests.
- A three-second real multicast smoke test found 190 devices, saved and listed
  the isolated snapshot, removed it, and deleted the temporary registry.

Verified after the first slice:

- `cargo +1.88.0 check -p oxvif-cli`: clean at the workspace MSRV.
- `cargo test -p oxvif-cli`: 49 passed across library, binary, and CLI process
  suites.
- `cargo test --workspace --all-features`: 994 passed, 3 ignored, 0 failed.
- `cargo clippy -p oxvif-cli --all-targets --no-deps -- -D warnings`: clean.
- `cargo rustdoc -p oxvif-cli --lib -- -D warnings`: clean.
- `cargo package -p oxvif-cli --allow-dirty --no-verify`: 17 files packaged.
  Final tarball verification requires publishing an oxvif release containing
  `probe_result`, `probe_result_on`, and `discovery_interfaces` before
  publishing `oxvif-cli`; workspace and MSRV builds use the local dependency.
- Isolated real-LAN verification: 194 observations produced 192 creates and
  two shared-target conflicts; apply rejected the conflicting plan with exit 4
  and left the inventory empty. Mock and CLI process tests cover successful
  apply, stale fingerprint rejection, idempotent re-plan, partial enrichment,
  total enrichment failure, and a deterministic 205-record inventory.

Stage 2A discovery lifecycle is complete: snapshots expose monotonically
increasing generations and scan-interface metadata; explicit refresh atomically
rescans an existing name; enrichment and refresh invalidate stale plans; and
versioned secret-free JSON overrides can deterministically replace proposed IDs
and Group-local aliases. Set resolution continues in Stage 3 with batch commands.

UX contract correction approved 2026-08-27:

- Add embedded `agent guide` and `agent prompt`; root help routes Agents to the
  guide and structured `describe` surface.
- Raise the structured stdout contract to schema v2 and report canonical IDs,
  original selectors, and explicit credential source/status.
- Make connection selectors/options consistent and stop advertising ignored
  global options. `device remove` remains global-ID-only by design.
- Raise the registry to v3 and atomically move snapshots out of `devices.toml`.
- Make discovery ephemeral unless `--save` is present, expose interface and
  partial-failure diagnostics, and merge duplicate observations.
- Add typed filter operators, all/any matching, View explanation output,
  Unicode-aware rendering, and complete Agent-readable command descriptors.

Implementation status 2026-08-27: the schema-v3 Agent contract, consistent
selectors, registry-v3 external snapshots, ephemeral discovery, explicit
interface selection, duplicate observation merge, typed View filters,
`--match all|any`, `view evaluate --explain`, credential status, readable fleet
output, and per-interface partial-failure warnings are implemented.

Discovery-to-inventory closure status 2026-08-27: authenticated bounded
`discover enrich`, deterministic per-record import proposals, filtered/existing/
conflict classifications, SHA-256 plan fingerprints, stale-plan rejection,
atomic device plus Group creation, credential-profile references, and
idempotent re-planning are implemented.

### Stage 2B — read-only diagnostic MVP

- Migrate discovery, device information, capabilities/services, media
  profiles, stream/snapshot URI, PTZ status/presets, and health checks through
  the application layer.
- Support table, JSON, and JSONL where appropriate.
- Run end-to-end against `MockTransport`, `MockServer`, and a multi-device
  mock fleet; no real camera is required for the release gate.

**Exit:** the ten-command diagnostic surface works by direct target, immutable
device ID, and `group/local-alias` in both human and non-interactive structured
modes. Set selection is enabled only where batch behavior is specified.

Implementation status 2026-08-27: single-target capabilities/services, Media1
profiles and credential-sanitized stream/snapshot URIs, PTZ status/presets, and
the default read-only health report all execute through the shared application
layer. MockServer covers the complete diagnostic path. Group/View set selection
and deterministic partial-success output are completed in Stage 3.

### Stage 3 — Agent hardening and fleet execution

- Stabilize schema version 3 and publish command descriptors.
- Add timeouts, bounded retries, retryability metadata, and cancellation
  behavior.
- Add bounded parallel health/inspection over `--group` and `--view`, including
  rate limits, deterministic JSONL, partial success, and aggregate exit codes.
- Test concurrent Agents using different explicit device IDs.

**Exit:** an Agent can discover capabilities, select a saved target, perform a
  diagnostic workflow, and recover from every expected failure without
reading human prose or encountering a prompt.

Implementation status 2026-08-27: Group/View selectors resolve to canonical
devices for every read-only diagnostic, jobs default to 16 and are capped at
64, completion order is normalized by device ID, JSONL emits per-device records
plus a final aggregate, partial success exits 6, and total failure is the typed
`FLEET_FAILED` error. Agent guide v3 documents this contract.

### Stage 4 — controlled writes

- Add write-risk metadata and explicit authorization.
- Add plan/apply for dangerous operations.
- Add idempotency and bounded PTZ movement.
- Begin with reversible operations; network, firmware, factory reset, and user
  management land only with dedicated integration tests and recovery notes.

### Stage 5 — optional MCP adapter

- Expose typed tools such as `discover_devices`, `inspect_device`,
  `list_media_profiles`, `get_stream_uri`, `get_ptz_status`,
  `plan_device_change`, and `apply_device_change`.
- Reuse the CLI application's requests, results, errors, credential lookup,
  and safety policy. MCP must not become a second implementation.

---

## 9. MVP release gate

The first crates.io release is ready only when all of the following are true:

- `cargo install oxvif-cli --locked` installs an `oxvif` executable.
- Existing `oxvif` default and all-feature tests remain green.
- Every MVP command works against the built-in mock without a physical camera.
- A saved device can be referenced by stable ID and its display name can change
  without breaking that reference.
- A 205-device mock inventory supports deterministic discovery filtering,
  import planning, static Group membership, dynamic Views, and exact
  `group/local-alias` resolution.
- Discovery never changes the registry; only explicit import apply does.
- Concurrent explicit selections do not share mutable current-device state.
- Registry files and all outputs pass credential redaction tests.
- `--non-interactive` never opens a GUI or waits for input.
- JSON/JSONL output validates against the published schema and contains no
  terminal decoration.
- Error code, retryability, target identity, and elapsed-time metadata are
  present for Agent consumers.
- Direct URL/IP operation remains available when persistence is unwanted.

---

## 10. Decisions closed by Stage 1

1. The workspace MSRV is Rust 1.88. The previous 1.85 declaration was stale:
   both existing let-chain syntax and `diqwest 3.2` require 1.88. Stage 1
   verifies the CLI with that exact compiler. `keyring 3.6.3` is retained as
   the tested Windows-native credential backend behind the CLI's own trait.
2. `use` is user-scoped in version 1. Project-local current-device state is
   deferred until a concrete workflow justifies its concurrency and Git rules.
3. A bare IP/host is normalized to the conventional
   `/onvif/device_service` path. The normalized endpoint is always returned in
   result metadata. Discovery-based resolution follows in Stage 2.
4. Registry writers take an OS-released `fs2` exclusive lock and replace the
   TOML atomically. The lock file may persist after exit; the OS lock does not,
   so no stale-lock deletion policy is needed.
5. Tags ship with the single-device registry. Static Groups, dynamic Views,
   Group-local aliases, discovery snapshots, reusable credential profiles, and
   import plan/apply move into Stage 2A because the known operating environment
   contains 205 cameras; a flat inventory is not an adequate discovery MVP.
6. Exit codes now distinguish invalid input, not found, conflict, missing
   target, registry/configuration, credentials, network/device connection, and
   internal failures.

## 11. Remaining decisions

1. Schema distribution format for `describe`: JSON Schema, the current smaller
   internal description format, or both.
2. Native credential backends and packaging policy for non-Windows platforms.
