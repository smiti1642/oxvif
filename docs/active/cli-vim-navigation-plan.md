# Reusable CLI Vim-style navigation plan

[English](cli-vim-navigation-plan.md) | [繁體中文](cli-vim-navigation-plan_zh.md)

Status: planned; implementation has not started. This document does not announce
an available feature. Baseline: `e7c8250`, including centered camera columns and
Ctrl+D / Ctrl+U half-page movement. Scope: human-facing terminal navigation only.

| Section | Purpose |
| --- | --- |
| [Objectives and boundaries](#objectives-and-boundaries) | Deliverables and exclusions |
| [Interaction contract](#interaction-contract) | Keys, counts and pending sequences |
| [Relative numbering and viewport](#relative-numbering-and-viewport) | Stable identity and display behavior |
| [Reusable architecture](#reusable-architecture) | Core, adapter and application responsibilities |
| [Implementation milestones](#implementation-milestones) | Ordered work and exit criteria |
| [Verification](#verification) | Automated tests and terminal acceptance |
| [Documentation and delivery](#documentation-and-delivery) | Documentation, packaging and release boundaries |

## Objectives and boundaries

1. Provide consistent `gg`, `G`, counted movement and hybrid relative numbering in
   manage menus, standalone profile selection, Discovery, and scrollable results.
2. Preserve arrows, Home/End, full-page and half-page navigation. Keep input fields,
   passwords and search isolated from navigation sequences.
3. Establish a deterministic, independently testable navigation core that another
   Rust CLI can reuse without importing ONVIF, a registry, networking or a terminal
   backend. This is a Vim-inspired navigation subset, not a Vim emulator.

Do not add editing operators, macros, registers, key remapping, persistent UI
preferences, a new search engine or additional camera operations. Existing search
remains available; adding search to manage is a separate feature. No Agent schema,
exit-code, command-selection, credential, or camera-write behavior changes.

## Interaction contract

`n` is a positive decimal count. List operations address the current filtered order,
not the registry's device ID or the original discovery record number.

| Input | Result |
| --- | --- |
| `j` / `k`, Down / Up | Move down/up one item or display line. |
| `nj` / `nk`, count + Down / Up | Move down/up `n` items or display lines. |
| `gg`, Home | Go to the beginning. A single `g` waits for another key. |
| `G`, End | Go to the end. |
| `nG`, `ngg` | Go to the one-based ordinal `n`; clamp beyond the final entry. |
| PgDown / PgUp | Move a full viewport; a count multiplies that movement. |
| Ctrl+D / Ctrl+U | Move half a viewport, rounded down with a minimum of one; a count multiplies that movement. It does not persistently change the page size. |
| `h` / `l` in Discovery | Preserve existing backward/forward full-page aliases; do not add these aliases to text input or other screens. |
| Esc with a pending count or `g` | Clear the sequence without leaving the screen. |
| Esc without a pending sequence | Preserve the screen's existing return/cancel behavior. |
| Enter, `i`, `q`, `/`, filter keys | Preserve existing screen actions when no navigation sequence is pending. |
| Ctrl+C | Preserve immediate cancellation/exit behavior, even with a pending sequence. |

Counts on `h` / `l`, Home/End and application actions are outside the initial
grammar. Plain `0` does not start a count; zero may extend a count such as `10j`.
Counts have a maximum of six digits (`999999`). Further digits are rejected with
a short status hint; the accepted prefix remains visible and cancellable. Arithmetic
must saturate before clamping; even a very large count must not loop per item.

Show the pending sequence, such as `12` or `12g`, in a bounded status area. No timing
deadline is needed: slow typing must work. Esc explicitly clears it. An unsupported
sequence, such as `3i` or `gq`, clears the prefix and consumes the failing key with a
brief hint; it must not accidentally open details, select an item or exit. Enter with
an unfinished prefix follows the same rule. Standalone unhandled keys remain the
screen's responsibility.

Only navigation mode feeds the parser. The terminal adapter ignores key-release
events and repeated digit/`g` prefix events; held movement keys may repeat. In search,
username, password and path input, `gg`, digits and `j`/`k` remain text, and Ctrl+U
retains its current clear operation. Clear pending state on mode/screen changes,
filter changes, replacement/reordering of data, cancellation and terminal resize.
Returning from details must not restore an old count.

Compatibility note: Discovery currently accepts a single `g` for the first entry.
It will require `gg`; Home remains an immediate alternative. Document this deliberate
change rather than silently maintaining an ambiguous single-`g` alias.

## Relative numbering and viewport

- Use a hybrid gutter: the selected item shows its one-based absolute ordinal;
  other visible items show their absolute distance from the selection. For example,
  a row seven places below the cursor displays `7` and can be reached with `7j`.
- Keep `>` as the selection marker. Navigation ordinals are not device IDs. Existing
  device IDs, original discovery record numbers and registration information remain
  available; label their role separately from the navigation gutter.
- Derive gutter width from the full filtered item count, not the visible page. Keep
  centered data columns and separator positions stable during navigation. Subtract
  the gutter from the content width before Unicode-safe truncation/wrapping.
- Maintain selection and viewport origin separately. Single-item/counted movement
  scrolls only when necessary to reveal the selection. Full/half-page movement shifts
  both by the requested distance, clamped at boundaries. `gg`/`G` reveal the first/last
  entry; absolute jumps reveal their destination. Do not wrap at either end.
- For read-only text viewers, the first visible display line is the relative-number
  anchor, not an editable cursor. Counts address wrapped display lines; `G` shows the
  last page, and `nG` places the requested line at the top where space permits.
- Filtering restarts the ordinal space over the matching items and clears pending
  input. A changed dataset retains selection by stable identity when available;
  otherwise clamp the existing ordinal, with no automatic item activation.
- Empty lists show an explicit empty state, no fictional item `1`, and no selectable
  row. Short terminals prioritize a visible selected item and return/cancel guidance;
  hide decorative/relative-gutter detail before hiding essential content. Resize must
  recompute row/column budgets and keep a valid selection. Text reflow may change line
  ordinals and must clear pending sequences; it need not preserve an exact text anchor.

## Reusable architecture

Use three boundaries; the type names below are provisional, not published API.

| Layer | Responsibility | Must not own |
| --- | --- | --- |
| Pure core: `crates/oxvif-cli/src/navigation.rs` | Backend-neutral key model, pending count/`g`, parse outcomes, navigation intents, bounded viewport calculations, relative-number values | crossterm events, terminal I/O, ONVIF, credentials, registry or async work |
| Terminal adapter: `interactive.rs` | Map crossterm events, route input modes, render gutters/status/help, honor terminal size | Independent copies of the count parser or movement arithmetic |
| Application: `manage.rs` and Discovery handlers | Supply items/stable identity and preserve select/details/search/add/cancel semantics | Interpretation of Vim sequences or storage of passwords in navigation state |

The core returns explicit outcomes: pending input, navigation action, consumed
cancellation/invalid sequence, or unhandled key. Navigation actions express relative
movement, first/last and absolute destinations. Hosts supply item count and viewport
size; the core returns clamped state without accessing the item payloads. Keep list
selection and text scrolling policies explicit rather than relying on ONVIF-specific
conditions.

Initially compile the module inside the existing CLI binary, without changing the
workspace/package graph or publishing a new public API. Add a small backend-free
test harness that compiles the same source with plain `rustc --test`; this proves
the core does not depend on the rest of oxvif. Test the actual shared source, not a
copied implementation. Existing package file-inclusion rules must cover new source
and tests.

Future extraction is a separate milestone: move the proven core to a standalone
crate with its own name, version, MIT attribution, README and API compatibility
policy. Reuse through an adapter in a second CLI should precede claims of general
adoption. Do not add an unpublished path-only runtime dependency to `oxvif-cli`;
agree on package naming and publish order before such an extraction.

## Implementation milestones

1. [ ] **M1 — Core and grammar.** Implement the pure parser, bounded calculations
   and relative-number helpers. Pin the contract above with table-driven tests and
   prove the backend-free harness compiles and passes. No camera/terminal access.
2. [ ] **M2 — Adapter and mode boundaries.** Add one event adapter, pending-sequence
   hints and reset rules. Replace duplicated navigation handling rather than placing
   another parser alongside it. Verify Esc, Ctrl+C, repeat/release and input isolation.
3. [ ] **M3 — Selectable lists.** Integrate manage's saved/discovered device menus,
   action menus, profile menus, standalone profile selection, and Discovery's list.
   Add the gutter and viewport policy while preserving aligned columns and stable
   selection. Confirm the saved/new status and existing actions are unchanged.
4. [ ] **M4 — Text viewers and constrained screens.** Integrate manage results/item
   details and Discovery details. Account for wrapped-line numbering, scroll limits,
   status/footer height, resizing and compact terminals. Forms/progress-only screens
   must not acquire a navigation cursor or consume text as navigation.
5. [ ] **M5 — Verification and documentation.** Run the gates below; synchronize
   bilingual user documentation and help. Record observed results and limitations,
   then make a scoped local commit. No release or system installation in this phase.
6. [ ] **M6 — Future standalone crate (deferred).** Revisit only after oxvif acceptance
   and a concrete second-CLI consumer; obtain direction on package/publication scope.

M1–M5 define completion for the initial implementation. M6 is explicitly not a
requirement for that delivery and must not be reported as completed by it.

## Verification

| Layer | Required evidence |
| --- | --- |
| Parser | `gg`, `G`, `12j`, `5k`, `42G`, `42gg`, multi-digit zeros, invalid/overlong prefixes, Esc twice, Ctrl+C, case/modifier handling, reset events and unsupported count/action combinations |
| Movement | Empty/single/large lists, odd/one/zero viewport sizes, both boundaries, saturated counts, absolute destinations, full/half-page operations and changed datasets |
| Rendering | Correct absolute/relative numbers, stable gutter and data-column widths across pages, Unicode, no control-character leakage, pending hints, empty states and selected-item visibility |
| Mode isolation | Search clearing, typing `123ggjk` in input, masked passwords, paste without navigation execution, mode changes, returning from details, and unchanged cancel behavior |
| Reuse | Compile and test the same core with plain Rust tooling, without Cargo/ONVIF/crossterm dependencies |
| Automation | Existing JSON/JSONL, non-interactive and redirected command tests stay unchanged and pass; no gutter, ANSI, key hints or prompt leakage |

Perturb count handling, `gg` completion or relative-number calculation and confirm
the relevant assertions fail; restore before the full gates. Do not rely only on
successful compilation or tests that merely assert an error exists.

Run workspace formatting, default/all-feature all-target clippy with `-D warnings`,
default/all-feature tests, and the Rust 1.88 compatibility check. If a public rustdoc
surface or intra-doc link changes, also run the appropriate warning-denied docs gate.

Use an isolated registry with at least 40 synthetic devices and both short/long,
ASCII/Unicode names; do not use real credentials or scan a real network. In an actual
Windows terminal, exercise `gg`, `G`, `7j`, `3k`, `21G`, half/full-page keys, invalid
prefix cancellation, details return, input clearing and resize. Use loopback mocks
only if a workflow requires data. Preserve any user-running CLI process; build/copy
to a separate test artifact when Windows locks its executable.

Record expected versus observed selection/viewport and clean terminal restoration.
Native macOS/Linux terminal checks are required before claiming three-platform UX
acceptance; Windows plus pure-core tests alone do not establish that. If independent
first-use critique is requested for implementation, perform it after main-agent
testing and distinguish agent-simulated critique from a real human study.

## Documentation and delivery

- During planning, add this bilingual plan and its documentation-index entry only.
  Do not advertise the proposed keys as implemented in README or CHANGELOG.
- During implementation, update `docs/cli-maintenance.md` and its `_zh` counterpart,
  `docs/oxvif-cli.md` and its `_zh` counterpart, on-screen hints, and Unreleased
  CHANGELOG. Explicitly describe relative versus identity numbers, text-viewer anchors,
  counted half-page semantics, `g` → `gg`, and input-mode exceptions.
- Keep the project README concise; add only a short navigation-guide link if needed.
  Agent documentation should retain the non-interactive boundary; this feature alone
  does not require a schema/Agent guide version bump or new machine command.
- Keep plan status and unchecked milestones accurate. Record test evidence and any
  remaining platform gates before declaring M1–M5 complete.
- No crate publication, GitHub Release/tag, push, merge, package-manager submission,
  system CLI replacement or new external dependency is authorized by this plan.
  Release and installation remain separately controlled operations.
