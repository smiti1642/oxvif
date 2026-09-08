# CLI maintenance UX refinement

[English](cli-maintenance-ux-plan.md) | [繁體中文](cli-maintenance-ux-plan_zh.md)

Status: implementation complete and locally verified; unreleased. Real-device and
native release-candidate acceptance remain pending. No installed-binary replacement.

| Section | Purpose |
| --- | --- |
| [Implementation](#implementation) | Ordered work |
| [Acceptance](#acceptance) | Required evidence |
| [Evidence and limits](#evidence-and-limits) | Verification and remaining gates |

## Implementation

1. [x] Reproduce documented post-command selector/timeout failures in executable
   tests. Normalize root options on maintenance commands without capturing local
   discovery `--jobs`, profile values or arguments after `--`. Preserve old syntax.
2. [x] Reuse the diagnostic's existing session and profile query for an optional
   human selector. Show name/token, arrows/j/k, Enter and cancellation. Only a
   human terminal can invoke it; explicit bad tokens, fleet and Agent requests
   never trigger fallback selection. Preserve completed evidence on cancellation.
3. [x] Make human reports summary-first. Show actionable failures and aggregate
   deferred playback limitations; `-v` reveals stage timing/details. Render setting
   changes as field/before/after rows, distinguishing missing from null. Add slow
   operation progress only to interactive stderr and clear it around menus.
4. [x] Add structured profile candidates and selection reason codes, and reasons
   for untested stages. Preserve existing schema-v3 fields, stage error categories
   and exit semantics; refine errors with additive fields rather than silently
   renaming a published contract. Update Agent guide and descriptors.
5. [x] Update bilingual guides, navigation, README summaries and Changelog. Test
   relevant Markdown command examples against the real parser and local mocks.

## Acceptance

- Parser matrices: prefix/suffix/equal-form flags, missing values, conflicts,
  opaque option-like tokens, `--`, and local discovery jobs.
- Profile picker: one/many/zero, unknown token, query failure, cancel, no repeated
  session/profile calls, and no UI in JSON, redirected or non-interactive modes.
- Reports: compact success/failure, details, configuration differences and nulls;
  schema-valid single/fleet JSON/JSONL, including partial and total failure.
- Terminal key/resize tests and actual Windows PTY interaction against a local
  mock where supported; record limitations instead of claiming unperformed tests.
- Format, default/all-feature workspace clippy/tests and Rust 1.88 checks.
- Real camera and macOS/Linux native acceptance remain release gates, not inferred
  from local mocks. Only commit verified changes; do not publish a Release.

## Evidence and limits

Windows x64, 2026-09-08:

- Workspace all-feature tests: 1,101 passed, 4 existing conditional ignores.
- Workspace default tests: 1,021 passed, 4 existing conditional ignores.
- Default/all-feature workspace clippy (`-D warnings`), formatting and Rust 1.88
  workspace/all-target/all-feature check passed; CLI rustdoc passed with warnings denied.
- Parser tests cover documented bilingual maintenance examples, suffix/equal-form
  options, invalid/missing values, conflicts, opaque tokens and discovery-local jobs.
  Executable mock tests cover suffix device/timeout, summary/detail output,
  comparison, structured candidates and JSON without `--non-interactive`.
- Profile tests cover unique/multiple/empty results, explicit unknown tokens,
  query failure, cancellation and adapter failure. Faults armed in the picker
  remained available afterward, proving no second handshake/profile query.
- Windows ConPTY with native console handles: `j` then Enter selected `Profile_2`
  and completed with exit 0; `q` retained three passed stages and returned exit 20.
  A local stalled HTTP endpoint displayed one-second elapsed progress, cleared it
  and returned a timeout report with exit 20. RTK's default piped handles correctly
  selected non-interactive behavior; console handles were needed to exercise UI.
- Keyboard/page/bounds and narrow/Unicode/resized-layout behavior have unit tests.
  Actual terminal resize, all terminal emulators and macOS/Linux UI were not tested.
- An initial default build was blocked by Windows locking the running mock binary;
  stopping that owned process and rerunning the complete gate passed.

No real cameras, camera settings, installed binaries, remote branches, tags or
Releases were changed. Synthetic artifacts remain under ignored
`target/cli-ux-implementation-20260908/`. Real-camera and native release-candidate
installation acceptance still require the operator's release workflow.
