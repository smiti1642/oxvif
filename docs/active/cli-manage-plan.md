# Guided CLI maintenance plan

[English](cli-manage-plan.md) | [繁體中文](cli-manage-plan_zh.md)

Status: implemented and locally verified, including independent critique; unreleased.
Built on the previous maintenance work while preserving
the newer develop sponsorship changes. No publishing, installation or camera writes.

| Section | Purpose |
| --- | --- |
| [Implementation](#implementation) | Ordered changes |
| [Acceptance](#acceptance) | Evidence and independent critique |
| [Results](#results) | Verification, critique and limits |

## Implementation

1. [x] Add a terminal-only `manage` entry point: choose saved/discovered devices,
   retain device/profile context, run diagnosis, inspect profiles, save snapshots,
   export/compare settings, inspect retained results and return without retyping IDs.
2. [x] Use shared application operations with a bounded session cache. Announce
   reuse versus reconnect; invalidate after failures and expire after 60 seconds.
   Never imply a cached session proves liveness or retry a file write automatically.
3. [x] Add reported encoding/resolution/FPS where available, explicit unavailable
   labels, bounded optional metadata queries and an `i` detail view. No primary/
   secondary-stream guessing, and no claim that configured FPS is measured FPS.
4. [x] Add a shared diagnostic assessment: primary observed issue, dependent
   untested checks, unimplemented limitations and actionable suggestions. Preserve
   v3 fields and exit semantics; expose the same facts to Agents.
5. [x] Synchronize bilingual documentation, README summaries, command metadata,
   Agent guide and Unreleased changelog. Reject manage in non-interactive/JSON mode.

## Acceptance

- Main agent: unit/mock/executable tests, terminal navigation/cancel/input/resize,
  failure recovery, cache expiry/reuse, no-clobber and structured output parity.
- Workspace default/all-feature tests and clippy, formatting and MSRV checks.
- After implementation and main-agent testing, launch one independent SubAgent to
  experience the CLI as a first-time human user. Prefer observed executable/terminal
  behavior over source inspection; use isolated local mocks, never real cameras.
- Address actionable critique, rerun relevant tests, record residual limitations.
- Real-camera and other-platform native release acceptance remain separate gates.

## Results

Windows x64, 2026-09-09:

- Workspace all-feature tests: **1,105 passed, 4 existing conditional ignores**.
- Workspace default tests: **1,025 passed, 4 existing conditional ignores**.
- Both workspace/all-target clippy variants (`-D warnings`), formatting, Rust 1.88
  workspace/all-target/all-feature check and warning-denied CLI rustdoc passed.
- Regression coverage includes profile metadata failure without losing profiles,
  session reuse/expiry/failure invalidation, no-clobber, export/diff, observational
  assessment, JSON refusal, Unicode/caret scrolling and password masking.
- Main-agent Windows ConPTY test: selected a saved device and Profile_2, diagnosed,
  saved a synthetic snapshot, cancelled the next path prompt and exited normally.
  A separate loopback stalled endpoint confirmed Escape cancels network waiting,
  returns to the workspace and marks the session for reconnect.
- Initial executable tests exposed Windows debug stack overflow from the enlarged
  async state. Heap-pinning the manage branch fixed it; full executable tests passed.
- A frozen executable was given to one independent SubAgent after main testing.
  It used CLI help and terminal behavior before reading implementation or manuals.
  It independently completed diagnosis, snapshot, export, comparison and cancellation.

| Critic finding | Resolution and targeted recheck |
| --- | --- |
| P2: Long paths hide filename/caret | Added horizontal scrolling and UTF-8-safe Left/Right/Home/End editing; rechecked in an 80-column terminal. |
| P2: Invalid destination clears input | Retain path; critic removed only an invalid middle directory and successfully saved without retyping the path. |
| P3: File-exists error talks about registry IDs | File-specific message and destination suggestion; no-clobber refusal rechecked. |
| P3: Details repeat the `-v` instruction | Removed redundant guidance when details are already rendered; stage timings retained. |
| Help: `--target` unexplained | Added IP/hostname/ONVIF URL and session-only behavior; help rechecked. |

The same critic verified all fixes against a second frozen executable and reported
no remaining blocker in that targeted review. This is **agent-simulated first use,
not a human user study**. No real cameras, LAN discovery, hardware credentials,
video decoding, actual terminal resize or non-Windows terminals were validated.
Real-camera interoperability and native three-platform release/installation gates
remain open. Mock processes were stopped; synthetic artifacts remain in ignored
`target/cli-manage-review-20260909/`. No Release/tag, remote branch, installed CLI,
camera configuration or saved user credential was changed.
