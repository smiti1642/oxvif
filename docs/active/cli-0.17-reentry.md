# CLI 0.17 acceptance re-entry

[English](cli-0.17-reentry.md) | [繁體中文](cli-0.17-reentry_zh.md)

The operator reopened the CLI cut on 2026-09-12. Earlier CI remains evidence
for its exact commit, not these runtime changes. Versions remain 0.16.0 until
separate release approval; no publication or system installation is authorized here.

| Section | Purpose |
| --- | --- |
| [Implementation batch](#implementation-batch) | Scope and checks |
| [Search return follow-up](#search-return-follow-up) | Session cache and scoped regression evidence |
| [Mock discussion](#mock-discussion) | Proposal, not implemented functionality |
| [Acceptance](#acceptance) | Evidence and remaining gates |

## Implementation batch

1. Replace manage's generic discovery menu with the existing discovery browser.
   Verify `/`, `r`, `n`, uppercase `A`, `c`, literal search, empty results,
   details and original-record selection. Enter in manage selects a session;
   only standalone discovery retains the onboarding action.
2. Promote F12: distinguish the fixed-bottom status row with reverse video;
   keep help separate, hide idle pending input, show available context and
   elapsed BUSY time. Preserve bounded rendering, differential repaint and
   terminal restoration. Do not suggest a measured completion percentage.
3. Retain manage action/camera viewports across returns, including cancelled
   credentials; clamp them when list length or terminal size changes.
4. Update paired user/release documents. Run focused UI checks, Windows ConPTY
   acceptance and one default/all-feature workspace test/Clippy batch, then commit.

This does not introduce RTSP playback, camera writes, a public navigation crate,
new dependencies, Agent envelope changes or host/network configuration changes.

## Search return follow-up

On 2026-09-13, a reported Back/re-entry regression was reproduced against the
previous binary: returning from a discovered camera lost the browser, and reopening
search started another network scan. `manage` now owns its discovery results and
view for the session. Back restores both; uppercase `R` requests a new scan.
Successful scans reset the view, while cancellation or failure preserves it.
The cache is labelled, may become stale, and is discarded when `manage` exits.
Standalone `discover` retains state for details/setup cancellation, but submitting
setup ends the command; a new invocation remains a new scan.

Scoped validation passed: `cargo test -p oxvif-cli`, CLI all-target Clippy with
warnings denied, and Windows ConPTY against the updated debug executable. The
terminal check compared the complete filtered/scrolled screen after camera Back,
chooser re-entry and cancelled rescan; it also verified successful explicit rescan
resets the view, exit code zero and an unchanged isolated registry. Only read-only
LAN discovery was performed; no camera operations or credentials were used. Two
new unit tests cover checkpoint restoration and context-sensitive `R` handling.
This follow-up does not rerun or replace the historical workspace counts below;
cross-platform CI and release approval remain outstanding.

## Mock discussion

Current source distinguishes three cases:

- `examples/mock_server` serves loopback HTTP without discovery. Separate ports
  and configuration files allow multiple instances, but do not advertise them.
- `MockServerBuilder::discoverable` creates one UDP 3702 listener per device;
  bind failure logs a warning while HTTP continues. It is not a shared listener.
- `Fleet` now offers opt-in shared discovery through `FleetBuilder::discoverable`;
  the new `mock_fleet_serve` runner exposes startup configuration for this path.

Recommended first scope: one process owns multiple isolated devices and one
discovery responder. Each device needs an independent stable identity and state,
a reachable advertised service address, and defined shutdown/removal behavior.
Explicit interface/bind/advertise options should control LAN exposure; do not
silently expose an unauthenticated mock beyond loopback. Requested discovery
startup failure must be visible, not mistaken for full startup success.

Separate independently started processes require an explicit coordination model
or separate addresses. Merely enabling socket reuse is not an accepted design.
The operator subsequently narrowed the first milestone to simple multi-camera
startup and configuration for VMS use. The [basic Fleet plan](mock-fleet-basic-plan.md)
uses one process and same-IP/different-port endpoints as implementation defaults;
64/256 capacity checks are opt-in and Metamorph mixtures remain later work.
F11's basic implementation is now available in development source; see the
[operator guide](../mock-fleet.md). Native LAN/VMS acceptance and release placement
remain pending; the capacity smoke does not establish sustained VMS stability.

The first slice rejects duplicate identities/configuration, bounds response sizes,
checks qualified types and declines scope filters. Dynamic membership and multiple
advertised interfaces remain excluded; see the plan for the exact boundary.
Acceptance must discover multiple distinct devices and reach each advertised
HTTP endpoint, verify state isolation and disappearance after shutdown, and
report multicast/platform limitations independently from unicast test results.

## Acceptance

- Focused shared-browser and layout regressions: passed locally.
- One full workspace batch passed: all-feature tests 1,311 passed / 5 ignored;
  default tests 1,199 passed / 5 ignored (41 suites each). Both all-target Clippy
  configurations passed with `-D warnings`; formatting passed.
- Windows ConPTY passed actual manage network discovery, saved/new filters,
  search/no-match recovery, matching saved-ID and session-only new-device selection,
  half-page keys, resize, reverse-video bottom row, credential-cancel/camera menu
  position retention, snapshot cancellation and console restoration. A disposable
  registry was used without credentials; its files remained unchanged by the
  interactive workflows. Discovery is not snapshot/playback interoperability evidence.
- Reviewed runtime Git blobs: `interactive.rs`
  `3d879c3ab08143bdf3c9906cf0f28e1956c0b8ec`; `manage.rs`
  `171db25fe5d1255dd43715ac5c01d529bde474d8`. Tested Windows debug binary SHA-256:
  `4569efd15f6c9bbe4a5cfef005ea1d3d1da42d5bca20a07aa9c29206f04fcf47`.
- Native macOS/Linux terminal behavior and human IME composition: not validated
  by Windows key injection; remain explicit acceptance items.
- New-commit hosted CI and final release/version/package approval: pending.

See the [maintenance guide](../cli-maintenance.md#manual-acceptance) and
[release cut](release-0.17-cut.md). Do not mark the old frozen inventory as a
review of changed bytes; this re-entry is follow-up evidence.
