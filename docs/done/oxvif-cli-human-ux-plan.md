# oxvif CLI human UX plan

**Status:** completed 2026-08-28. Written after the `oxvif-cli 0.1` feature and
Agent contracts were completed, then implemented before the first crates.io
publication.

**Target:** additive human-facing UX improvements for `oxvif-cli 0.1.0`.

**Parent plan:** [oxvif CLI human and Agent operation surface](../active/oxvif-cli-plan.md).

---

## 1. Objective

Make common interactive workflows concise and discoverable without weakening
the explicit, deterministic command contract already built for Agents and
automation.

The CLI currently optimizes for unambiguous scripted execution:

```sh
oxvif device add front-door --target 192.168.1.100 --name "Front Door" --tag entrance
oxvif device credential set front-door --username admin --password-stdin
oxvif --device front-door device test
oxvif --device front-door device info --output json
```

That form remains the canonical automation interface. The human-facing layer
should make the equivalent first-run and daily workflows feel closer to:

```sh
oxvif setup front-door 192.168.1.100 --name "Front Door" --tag entrance
oxvif info
oxvif health
```

The improvement is a thin command facade over the existing typed application
requests. It must not create a second implementation of device operations.

## 2. Success criteria

The plan is complete when all of the following are true:

- A human can securely register, authenticate, verify, and select one device
  with one `setup` command.
- After selecting a current device, the most common read operations require a
  short top-level command such as `oxvif info` or `oxvif health`.
- A one-shot operation can select a saved device positionally, for example
  `oxvif info front-door`, without modifying ambient state.
- A bare `oxvif discover` performs a safe, ephemeral scan and presents useful
  next actions.
- Human credential entry uses a no-echo terminal prompt; passwords never enter
  arguments, registry files, output, or logs.
- Human errors explain the problem and provide one or more executable recovery
  commands.
- Existing canonical commands remain supported and retain their current
  structured request, result, error, and exit-code behavior.
- `--non-interactive` never prompts, opens a GUI, or silently uses a heuristic
  that requires a human decision.
- JSON and JSONL remain free of terminal decoration, prompts, progress bars,
  and human-only suggestions.
- Agent documentation continues to recommend canonical commands with explicit
  selectors, structured output, and `--non-interactive`.

## 3. Product principles

### 3.1 One application model, two invocation styles

Human shortcuts and canonical commands must resolve to the same typed request
before application execution:

```text
human alias ---------+
                     +--> typed request --> application --> typed result
canonical command ---+
```

Aliases may change parsing, prompting, defaults, and terminal presentation.
They may not fork credential resolution, ONVIF calls, registry behavior, error
classification, or result serialization.

### 3.2 Explicit automation, contextual interaction

- Humans may use the current device and secure terminal prompts.
- Agents should use `--device`, `--group`, `--view`, or `--target` explicitly.
- A human alias with no positional device may use the current device only in
  interactive mode.
- The same alias under `--non-interactive` fails with `MISSING_TARGET` unless
  an explicit selector is present.
- Fleet scope is never inferred from ambient state.

### 3.3 Safe defaults

- Bare discovery is ephemeral.
- Setup verifies credentials before persisting them.
- Read operations may use convenience defaults only when the choice is unique.
- Ambiguous choices prompt in an interactive TTY and fail with actionable
  guidance under `--non-interactive`.
- No device-mutating command is introduced by this plan.

### 3.4 Concise does not mean cryptic

Use complete words such as `info`, `health`, `profiles`, and `devices`. Do not
add opaque aliases such as `ls`, `ctx`, `prof`, or `rm` in the first release.
Shell completion is a better solution than accumulating abbreviations.

## 4. Command design

### 4.1 Canonical commands remain stable

The following forms remain documented for Agents and scripts:

```sh
oxvif --device front-door device info --output json --non-interactive
oxvif --device front-door media stream-uri --profile Profile_1 --output json --non-interactive
oxvif --group taipei-f1 --jobs 16 health check --output jsonl --non-interactive
```

No existing command is removed, renamed, or given different machine-readable
semantics.

### 4.2 Human quick commands

Add the following top-level commands as typed-request adapters:

| Human command | Canonical operation |
| --- | --- |
| `oxvif test [DEVICE]` | `device test` |
| `oxvif info [DEVICE]` | `device info` |
| `oxvif health [DEVICE]` | `health check` |
| `oxvif profiles [DEVICE]` | `media profiles` |
| `oxvif stream [DEVICE]` | `media stream-uri` |
| `oxvif snapshot [DEVICE]` | `media snapshot-uri` |
| `oxvif devices` | `device list` |
| `oxvif groups` | `group list` |
| `oxvif views` | `view list` |
| `oxvif auth DEVICE` | `device credential set` |

The optional positional `DEVICE` accepts a canonical device ID or exact
`group/local-alias`. It does not accept display names and never performs fuzzy
selection. This prevents a shortcut from becoming nondeterministic as the
inventory grows.

Fleet execution remains explicit:

```sh
oxvif health --group taipei-f1
oxvif health --view outdoor
```

`--group` and `--view` may be written after a quick command for human reading,
but they remain the same global selectors internally.

### 4.3 Human output shorthands

Add optional `--json` and `--jsonl` shorthands for `--output json` and
`--output jsonl`. They are mutually exclusive with each other and with an
explicit conflicting `--output` value.

The canonical `--output` option remains the documented Agent interface.

### 4.4 Help organization

Root help groups commands by intent:

1. **Quick operations** — `info`, `test`, `health`, `profiles`, `stream`, and
   `snapshot`.
2. **Inventory and discovery** — `setup`, `devices`, `discover`, `group`, and
   `view`.
3. **Advanced and automation** — canonical namespaces, `describe`, and
   `agent`.

Quick-command help must show the canonical equivalent. Canonical help may show
the shorter human form as an example, but must not redirect Agent guidance to
ambient state.

## 5. Secure setup and authentication UX

### 5.1 `setup`

Add:

```text
oxvif setup <DEVICE_ID> <TARGET> [--name <NAME>] [--tag <TAG>...] \
  [--username <USERNAME>] [--password-stdin] [--no-verify] [--no-use]
```

Default interactive behavior:

1. Validate and normalize the ID and target without writing state.
2. Fail before prompting if the ID or normalized target conflicts with the
   registry.
3. Prompt for a missing username.
4. Prompt for a password without echoing it.
5. Create an in-memory connection and verify the ONVIF device service.
6. Persist the native credential and registry entry only after verification.
7. Set the new device as current unless `--no-use` is supplied.
8. Print a concise device identity summary and the next useful command.

Under `--non-interactive`, missing username or password input is a typed error;
setup never falls back to a terminal prompt.

Example terminal result:

```text
Device front-door
  Target       http://192.168.1.100/onvif/device_service
  Credential   stored in Windows Credential Manager
  Connection   verified
  Manufacturer GeoVision
  Model        GV-BL5700

Current device: front-door
Next: oxvif info
```

### 5.2 Persistence and rollback

The registry and OS credential store do not provide one shared transaction.
Setup therefore uses the following order:

1. validate all local conflicts;
2. verify the live device with in-memory credentials;
3. write the native credential;
4. atomically add the registry entry; and
5. if registry persistence fails, delete only the credential created by this
   invocation and report whether cleanup succeeded.

Setup must detect and refuse to overwrite a pre-existing native credential in
the new device's credential slot. No pre-existing credential may be modified or
deleted during rollback. Unit tests must inject failure at every boundary.

`--no-verify` skips the live check but still validates all local inputs. Help
must state that this may save an unreachable or incorrectly authenticated
device.

### 5.3 `auth`

`oxvif auth front-door --username admin` prompts for a password without echo
and maps to the existing credential request. Automation uses:

```sh
oxvif auth front-door --username admin --password-stdin --non-interactive
```

If stdin is a TTY, `--password-stdin` continues to read stdin exactly as today;
the no-echo prompt is the default only when `--password-stdin` is absent and
interactive execution is allowed.

## 6. Discovery UX

### 6.1 Bare discovery

Make `oxvif discover` equivalent to ephemeral `oxvif discover scan`.
Subcommands such as `snapshots`, `list`, `refresh`, `enrich`, and `remove`
remain unchanged.

Bare discovery must never create a snapshot or registry device. Retention still
requires `--save`.

### 6.2 Human discovery table

The terminal renderer adds:

- a stable row number for the current output only;
- endpoint/IP, UUID suffix, manufacturer, model, and registration state when
  available;
- total matched count and active filters; and
- next-action examples for filtering, saving, enriching, or importing.

Row numbers are display aids, not persistent identities. Follow-up commands
must use endpoint, UUID, or a retained snapshot selection rather than assuming
that row `12` remains stable across scans.

Structured discovery output remains complete and unchanged except for additive
schema fields explicitly versioned through the normal contract process.

### 6.3 Large result sets

Do not silently truncate a 205-device scan. The first implementation prints all
matched rows and recommends filters. Pager support may be added later only when
it is TTY-aware, disabled by `--non-interactive`, and never affects structured
output.

## 7. Human diagnostics UX

### 7.1 Context visibility

When a quick command uses the current device, terminal output identifies it:

```text
Using current device: front-door (192.168.1.100)
```

This context line is human-only and is omitted when `--quiet` is set. JSON and
JSONL already carry target identity in metadata and receive no decoration.

### 7.2 Profile selection

For `stream` and `snapshot` without `--profile`:

- use the profile automatically when exactly one profile exists;
- prompt with token, name, encoding, and resolution when multiple profiles
  exist in an interactive TTY; and
- fail with `INVALID_ARGUMENT` plus the available profile tokens under
  `--non-interactive`.

The canonical `media stream-uri` and `media snapshot-uri` commands retain their
existing explicit profile behavior. Automatic profile selection belongs only
to the human quick commands.

### 7.3 Fleet summaries

Table output for Group/View diagnostics begins with scope and finishes with
counts for healthy/successful, warning, failed, and total devices. Failed rows
include the stable error code and a concise message.

JSONL ordering, `fleet_item`, `fleet_summary`, exit `6`, and `FLEET_FAILED`
remain unchanged.

## 8. Actionable errors

Human table errors should contain:

1. a concise problem statement;
2. the affected device or scope;
3. one likely reason when it is known; and
4. executable recovery commands.

Example:

```text
Error: no credential is configured for front-door.

Set one securely:
  oxvif auth front-door --username admin

Or assign an existing profile:
  oxvif device credential use-profile front-door factory-admin
```

Not-found errors may suggest close canonical IDs, but a suggestion is never
selected automatically. The structured error message may gain additive hints,
but its symbolic code, retryability, and exit code remain authoritative.

## 9. Shell completion

Add a `completion` command for Bash, Zsh, Fish, and PowerShell. Completion
includes commands, options, enum values, and static syntax. Dynamic device,
Group, View, snapshot, and credential-profile completion is deferred until it
can be implemented without network access, prompts, or observable registry
side effects.

Examples:

```sh
oxvif completion bash
oxvif completion zsh
oxvif completion fish
oxvif completion powershell
```

Generated completion output goes to stdout so installation remains controlled
by the user's shell environment or package manager.

## 10. Implementation stages

### Stage H1 — quick-command facade

- Add typed request adapters for `info`, `test`, `health`, `profiles`,
  `stream`, `snapshot`, `devices`, `groups`, and `views`.
- Support an optional positional single-device selector on quick commands.
- Allow `--group` and `--view` after `health` while preserving the existing
  global selector model.
- Add `--json` and `--jsonl` output shorthands.
- Reorganize root help and add alias equivalence to `describe` metadata.
- Update the English and Traditional Chinese CLI manuals.

**Exit:** every quick command produces the same typed application request as
its canonical equivalent, and all canonical parser/output snapshots remain
green.

### Stage H2 — secure interactive setup

- Introduce a prompt abstraction with a real TTY implementation and a fake
  test implementation.
- Add no-echo password input without changing the MSRV.
- Implement `setup` validation, live verification, persistence, rollback, and
  current-device selection.
- Add `auth` with secure prompting and non-interactive stdin support.
- Ensure Ctrl-C and EOF leave no partial registry or secret state.

**Exit:** one command securely onboards a mock device, injected failures prove
rollback behavior, and `--non-interactive` has zero prompt paths.

### Stage H3 — discovery and human rendering

- Make bare `discover` an ephemeral scan.
- Add registration-aware discovery columns and next-action hints.
- Add current-context notices for quick commands.
- Add actionable human error hints and deterministic close-ID suggestions.
- Add fleet table summaries without modifying JSONL.

**Exit:** a 205-device mock discovery remains complete, filterable, and
readable; terminal additions do not alter structured stdout.

### Stage H4 — profile choice and completion

- Implement quick-command profile auto-selection and TTY choice.
- Add `completion` generation for Bash, Zsh, Fish, and PowerShell.
- Add end-to-end documentation examples for interactive and Agent paths.

**Exit:** humans can obtain a stream URI without memorizing a profile token,
while non-interactive execution fails deterministically on ambiguity; generated
completion scripts pass shell-specific smoke checks where available.

## 11. Testing strategy

### Parser and mapping tests

- Snapshot root and subcommand help.
- Parse every human alias with current, positional, direct, Group, and View
  selectors where applicable.
- Assert alias and canonical invocations produce equivalent typed requests.
- Assert conflicting selectors and output shorthands fail before execution.

### Prompt tests

- Test prompts through an injected prompt trait rather than relying on a real
  terminal in unit tests.
- Cover accepted input, empty input, EOF, cancellation, invalid input, and
  secret redaction in `Debug` and errors.
- Prove that `--non-interactive` never calls the prompt implementation.

### Persistence tests

- Inject failures before credential write, after credential write, and during
  registry replacement.
- Verify cleanup deletes only secrets created by the current setup operation.
- Verify existing devices and credential profiles are unchanged after failure.

### Integration tests

- Run setup, info, profiles, stream, snapshot, and health against the built-in
  mock.
- Exercise discovery presentation with the existing 205-device fixture.
- Verify quick-command fleet partial success still exits `6` and emits the
  existing JSONL aggregate contract.
- Verify every structured output is valid JSON/JSONL and contains no ANSI,
  prompts, progress text, or credentials.

### Release gates

- `cargo test --workspace --all-features`
- `cargo +1.88.0 check --workspace --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- rustdoc with warnings denied
- `cargo install oxvif-cli --locked` from the packaged release artifacts
- Windows Credential Manager onboarding and rollback smoke test

## 12. Compatibility policy

- Canonical command paths remain supported for the entire `0.x` line unless a
  separately documented breaking release changes them.
- Human quick commands are additive public CLI surface and receive parser and
  help snapshots before release.
- Adding command descriptors does not by itself change structured schema
  version 3; changing descriptor or envelope shape does.
- Human-only table text is not a machine contract. JSON, JSONL, symbolic error
  codes, and numeric exit codes are machine contracts.
- Agent guide and examples continue to use canonical forms so shortcuts do not
  become an undocumented dependency for automation.

## 13. Explicit non-goals

- No device-mutating ONVIF operations.
- No full-screen TUI.
- No fuzzy automatic device selection.
- No implicit Group/View credential inheritance.
- No ambient fleet selection.
- No silent discovery truncation.
- No password command argument.
- No replacement of the existing registry, snapshot, filter, or plan/apply
  models.
- No MCP changes in this plan.

## 14. Decisions fixed by this plan

1. Existing hierarchical commands are canonical and remain stable.
2. Human UX is an additive facade that maps into the same typed requests.
3. `setup` is the one-command onboarding path; `auth` is the secure credential
   shortcut.
4. Quick operations accept an optional exact positional device selector.
5. Group/View fleet selection remains explicit through `--group` or `--view`.
6. Bare `discover` is an ephemeral scan and never writes local state.
7. Secure TTY prompting is allowed only outside `--non-interactive` execution.
8. Profile guessing is permitted only when the choice is unique; ambiguity
   prompts humans and fails automation.
9. Human improvements may enrich table output and error guidance but cannot
   contaminate or destabilize structured output.
10. The first implementation favors full words and shell completion over
    cryptic abbreviations.

## 15. Deferred decisions

1. Whether a future `oxvif shell` or full-screen TUI would materially improve
   workflows beyond aliases, completion, and ordinary shell composition.
2. Whether project-local current-device contexts are needed in addition to the
   existing user-scoped current selection.
3. Whether dynamic completion of local registry IDs is worth the latency,
   privacy, and shell portability cost.
4. Whether an optional TTY pager should be introduced for very large table
   output.

## 16. Implementation record

All four implementation stages were completed on 2026-08-28:

- **H1:** added top-level quick commands, exact positional device selection,
  fleet-selector normalization, `--json`/`--jsonl`, help and descriptor
  metadata, and bilingual documentation.
- **H2:** added secure no-echo prompting, `setup`, `auth`, preflight conflict
  checks, live verification, rollback, and deterministic non-interactive
  behavior.
- **H3:** made bare discovery ephemeral, added registration-aware human output,
  current-device context, actionable credential and close-ID hints, and retained
  the existing fleet summary contract.
- **H4:** added unique-profile auto-selection, interactive ambiguous-profile
  choice, deterministic non-interactive failure, and Bash, Zsh, Fish, and
  PowerShell completion generation.

Compatibility was preserved: quick commands resolve to the existing typed
requests, canonical command paths remain available, structured schema version 3
is unchanged, and discovery registration annotations are excluded from
JSON/JSONL serialization.

The root help presents quick commands prominently within Clap's command list
rather than introducing cosmetic heading groups. Profile prompts display the
token and name available from the current media-profile contract; encoder and
resolution can be added when those fields become part of that result. Dynamic
registry completion and a TTY pager remain deferred as described above.

Verification evidence:

- `cargo test -p oxvif-cli`: 78 passed.
- `cargo test --workspace --all-features`: 1,023 passed, 3 ignored.
- `cargo +1.88.0 check --workspace --all-features`: passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed.
- rustdoc for the workspace with warnings denied: passed; Cargo emits its known
  same-name lib/bin output-collision notice for `oxvif`.
- `cargo package -p oxvif-cli --list --allow-dirty`: the intended 17 package
  files were selected. Full package verification is correctly gated on
  publishing the `oxvif 0.16.0` dependency first.
- `cargo install --path crates/oxvif-cli --locked`: installed an `oxvif 0.1.0`
  executable, which generated the PowerShell completion script successfully.
