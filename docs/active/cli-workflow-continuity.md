# CLI workflow continuity plan

[English](cli-workflow-continuity.md) | [繁體中文](cli-workflow-continuity_zh.md)

Status: approved for implementation, 2026-09-13. Source baseline:
`71f0f437f60781393db98ab58dd95eee22fc1d90`, branch `feat/basic-mock-fleet`.
This plan follows direct source inspection, not earlier conversation summaries.
No version bump, system installation, release, CI dispatch or main-branch merge is
part of this authorization.

| Section | Purpose |
| --- | --- |
| [Source map](#source-map) | Verified gaps and existing building blocks |
| [Behavior contract](#behavior-contract) | State, security and navigation decisions |
| [Implementation batches](#implementation-batches) | Ordered construction and acceptance |
| [Validation and documentation](#validation-and-documentation) | Gates and handoff |
| [Progress](#progress) | Evidence recorded after execution |

## Source map

Line numbers below identify the baseline; use the named symbols after edits.

| Gap | Actual source | Required change |
| --- | --- | --- |
| Manage cannot add discovered cameras | [manage.rs](../../crates/oxvif-cli/src/manage.rs), `choose_device` (304); [interactive.rs](../../crates/oxvif-cli/src/interactive.rs), `BrowserState::handle_key` (1331), `DiscoverySelection` | Separate Enter/select from explicit a/add |
| Standalone add ends browser, including verification failure | [main.rs](../../crates/oxvif-cli/src/main.rs), `execute_and_emit` (1115), `setup_discovered_device` (1175); `browse_discovery`, `SetupForm::finish` | Keep terminal, form retry and discovery owner alive across setup |
| Switching devices discards state | `manage::run` (73–82) creates context/profile/result/viewport on every selection | Session-owned per-device workspace |
| Saved-device chooser cannot search | `choose_device` calls `Panel::menu_with_view` (735), which has no query handling | Reusable filtered menu with original-index mapping |
| Profile picker starts at first row | `manage::run` (167) calls `Panel::menu`, which constructs a default viewport | Restore selection by profile token after each live read |
| Direct failures cannot be reopened | `manage::execute` (252) reduces errors to `WorkflowOutcome::Failed` | Separate retained failure/cancellation evidence from last completed result |
| Address retry loses text; compare does not reuse exports | `choose_device` address branch, `destination` (276), `run` compare branch | Retained non-secret drafts and explicit last-export choice |
| Reopened reports lose position and cannot search | `Panel::show` (792), local `offset = 0` | Caller-owned searchable text view |

Existing code to preserve/reuse:

- [application.rs](../../crates/oxvif-cli/src/application.rs):
  `Application::preflight_setup`, `CommandRequest::DeviceSetup` execution,
  `rollback_setup`, `discovery_devices` (UUID/normalized-address registration
  projection). Setup verifies before synchronous local persistence and has rollback
  evidence tests. Do not duplicate persistence or reimplement identity matching.
- [maintenance.rs](../../crates/oxvif-cli/src/maintenance.rs): `ManagedDevice`,
  `set_credentials`, `disconnect`, 60-second connection expiry,
  `ManagedAction`, output no-clobber and `read_baseline`.
- [interactive.rs](../../crates/oxvif-cli/src/interactive.rs): terminal RAII,
  differential rendering, masked/zeroized setup fields, literal input handling,
  `DiscoverySelectionView` and context-sensitive Vim navigation.
- Existing tests: setup success/verification failure/native-secret conflict/race
  rollback/incomplete rollback in application; managed session/no-clobber and
  diagnostic cancellation in maintenance; discovery checkpoint, filters, Unicode,
  pending keys, resizing and form masking in interactive.

## Behavior contract

1. Manage Enter selects without saving; `a` explicitly opens setup. Standalone
   discovery retains Enter/a onboarding. Both stay in the same terminal after
   setup success/failure/cancellation. Form text states that submission verifies
   and saves credentials/device; manage must not change the global current device.
   Standalone preserves its existing explicit setup current-device behavior.
2. Retry retains device ID and username, clears the submitted password, and shows
   the actual application error. No automatic retries of local writes, overwrites,
   TLS bypass or alternate credentials. Cancellation before submission writes
   nothing. Cancelled in-flight setup must reconcile registry state before offering
   another attempt; incomplete rollback is not presented as clean cancellation.
3. After setup, reproject cached discovery records against the registry without
   multicast. Preserve query/registration filter and stable record identity. Under
   a NEW-only filter, a newly saved record disappears; select the next valid row,
   show saved confirmation, and never silently clear the filter.
4. A per-device workspace owns ManagedDevice, selected profile, action/profile
   viewport, last completed result, latest failure/cancellation, report views and
   non-secret path drafts. Key saved devices by ID plus normalized target, direct
   devices by normalized target; never share contexts by IP alone or merge saved
   and direct credentials implicitly. Reconnection expiry remains effective.
   Bound the number of retained contexts to 256; refuse an additional distinct
   context with a clear restart message instead of silently evicting state.
5. Credentials remain memory-only unless setup is explicitly submitted. Switching
   cameras never copies another camera's secrets; exiting manage drops all
   contexts. Registry target/credential configuration changes invalidate stale
   saved-device state before reuse. Native-secret edits outside the process still
   require restarting the workspace (existing documented behavior).
6. Saved-menu search matches ID, name, target and tags case-insensitively. Search
   and direct-address actions stay reachable with zero device matches. Preserve
   identity, not merely an index, across registry changes; transient Vim prefixes
   and search edit mode are not resumed. Search text is literal, not regex.
7. Profile reads remain live. After refresh select the previous token if present;
   a missing token clears the stale selection and requires an explicit choice,
   except the existing single-profile convenience. A failed read must not silently
   choose a different token.
8. Last completed result and latest failure/cancellation have separate menu entries.
   Searchable reports retain query/position when reopened; new content resets its
   view. Labels distinguish historical evidence from live camera status.
9. Invalid address/path input returns to the same populated editor. Retain
   non-secret drafts on ordinary back/re-entry. Never retain a password as a draft.
   Compare offers this device's last successfully written export, plus manual path;
   revalidate the file at execution and never treat snapshot paths as inventories.
10. No ONVIF protocol/parser/Mock behavior changes or Agent envelope changes.
    Human persistence uses the same existing typed setup command as Agents.

## Implementation batches

### B1 — Continuous discovery onboarding

- [ ] Factor reusable inline setup execution out of the shell-returning main helper;
  keep terminal ownership in Panel and use the shared Application setup command.
- [ ] Add manage a/add and continue standalone discovery after each setup.
- [ ] Expose/reuse local registration reprojection; preserve record identity and
  filters through success, failure, cancellation, duplicate IDs and unusable XAddr.
- [ ] Extend existing UI/application tests for explicit writes, password clearing,
  cached registration refresh and no accidental setup from pending/search keys.
- [ ] Run focused CLI tests, then commit this complete behavior batch.

### B2 — Device workspaces and searchable chooser

- [ ] Replace per-selection locals with bounded, identity-keyed workspaces;
  invalidate changed saved identities/configuration without cross-camera reuse.
- [ ] Add searchable saved chooser with fixed action rows and original-index mapping.
- [ ] Restore profile token/viewport and remove stale tokens after a successful read.
- [ ] Test A→B→A, same-IP distinct identities, 256-context boundary, empty filters,
  literal Vim-like queries, registry changes and profile removal/reordering.
- [ ] Run focused CLI tests, then commit.

### B3 — Results, retries and file workflow

- [ ] Retain structured direct failures/cancellations separately from completed data.
- [ ] Add reusable caller-owned text search/position for reopened results.
- [ ] Preserve address/output/compare drafts; keep invalid-input repair inline.
- [ ] Offer a confirmed successful export as comparison baseline; revalidate deleted,
  malformed or changed files and retain existing no-clobber rules.
- [ ] Test old-success/new-failure separation, cancellation, text no-match/Unicode/
  resize, corrected input, export→compare and absent/invalid baseline.
- [ ] Run focused CLI tests, then commit.

### B4 — Integration, documentation and handoff

- [ ] Run one CLI package suite and CLI all-target Clippy/fmt batch after B1–B3.
  Reuse existing application/maintenance coverage; no full 1300+ Mock rerun unless
  implementation crosses into core ONVIF or Mock code.
- [ ] Run actual Windows ConPTY journeys against isolated local registry and mock
  fixtures: add→retry→add another; manage search→add→operate→Back; A→B→A; 256 saved
  rows/search; missing profile; retained failure; export→compare; quit/restoration.
  Do not use production credentials or camera writes for UX acceptance.
- [ ] Document failures and limitations separately from passing evidence; compile
  alone is not terminal acceptance. Linux/macOS runtime acceptance remains CI/manual.
- [ ] Update paired CLI guide/maintenance docs, root unreleased changelog, detailed
  0.17 changelogs and active acceptance/release-cut records. Historical counts and
  published 0.16 sections stay unchanged. Refresh immutable public guide links only
  after a verified documentation commit exists.
- [ ] Commit/push feature-branch batches; hand off exact commit and manual checks.
  Do not merge, tag, publish or install a system release.

## Validation and documentation

Each batch adds scenario-focused tests to the nearest existing test module rather
than generating many redundant cases. Network fixtures must use isolated loopback
ports/configuration and fake credentials. Native credential-store success tests
remain explicit opt-in; record an environment skip honestly. Run unit tests before
terminal journeys and preserve sanitized reproducible terminal harnesses.

Acceptance requires both state-unit assertions and end-to-end UI orchestration:
an exact viewport checkpoint alone does not prove that the caller reuses it.
All new public behavior must have paired English/Traditional Chinese documentation,
reciprocal language links and section-link tables for long documents.

## Progress

- Planning complete: baseline and all source owners above inspected.
- B1–B4 not yet accepted. Evidence and commit IDs will be appended per batch.
- B1 implementation complete: shared Panel onboarding, manage a/add, standalone
  continuation and local registry reprojection. Binary tests: 54 passed; existing
  setup application scenarios: 5 passed; new local reprojection test: 1 passed.
  No terminal/native credential acceptance is claimed yet; that remains B4.
- B2 implementation complete: bounded workspaces, saved-menu search and token-based
  profile restoration. Saved-ID slots compare the full DeviceView (including target
  and credential configuration) before reuse; any changed view conservatively
  invalidates that slot. Binary tests: 57 passed, including A/B/A, same-address
  identities, registry target changes, 256-slot limit and literal search mapping.
- B3 implementation complete: separate retained issues, searchable persistent report
  views, non-secret input drafts and last-export baseline selection with shared
  local preflight. Binary tests: 59 passed; CLI all-target Clippy with warnings
  denied passed. B4 terminal journeys and final docs remain open.
