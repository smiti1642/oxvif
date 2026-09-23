# Remaining-plan acceptance inputs

[English](remaining-plan-acceptance.md) | [繁體中文](remaining-plan-acceptance_zh.md)

2026-09-22. A supporting register for the existing plans, not another milestone.
Scheduling and authoritative status remain in [the backlog](post-0.17-backlog.md).
The local repository work is committed directly to `master`; no PR, release or
external publication is part of this work.

Status reviewed 2026-09-23: the selected local delivery pass through `2d430d8`
is **DONE**. This register remains **ACTIVE** for the outstanding inputs below;
the parent plans are not all complete. See the [closure review](post-0.17-backlog.md#closure-review-2026-09-23).

| Section | Purpose |
| --- | --- |
| [Work disposition](#work-disposition) | Every F family has a concrete next boundary |
| [Decision inputs](#decision-inputs) | Prepared contracts for deferred product choices |
| [Native evidence](#native-evidence) | Commands and evidence still requiring a host/operator |

## Work disposition

| Family | Local work / next acceptance boundary |
| --- | --- |
| F01/F02/F03 | [RS1](../done/mock-fidelity-read-selectors.md) repairs four Media/PTZ/Recording selectors and shared rendering. OSD CRUD, URI/source modes, motion/Imaging, Device/DeviceIO and recording/search lifecycle remain implementation work; they are not blocked solely by hardware. Start each next subgroup with W01 cards. |
| F04/F06 | [EP1](../done/mock-fidelity-event-pull.md) repairs queue filtering and atomic selection/filter snapshots. Per-subscription state and capability/replay consistency remain code work. Before endpoint identity is added, preserve the public RequestCtx construction contract or propose a compatible adapter. |
| F05 | Listener framing/deadline limits are delivered. The next mock-auth card must distinguish digest verification from freshness/replay: fixed injectable clock, accepted skew window, bounded nonce retention, duplicate nonce atomicity, expired-entry eviction, independent per-instance caches and rejected-request state preservation. Existing permissive auth is not evidence these protections exist. |
| F07 | RS1/EP1 add selected operation captures and external validation, with explicit payload anchors. Broader wildcard/QName/negative/fuzz coverage remains open; new independent instances must remain outside the checkout. |
| F08 | Prepared bounded product contracts below; RTSP playback, file batch export, camera writes, MCP and crate extraction remain deferred decisions. |
| F09 | Existing artifact/staging checks are usable. Public APT/tap ownership, signing and recovery inputs below are missing; platform lifecycle and submissions remain open. |
| F10 | `snapshot_probe` provides bounded offline evidence using the exact production signature function, without accepting modified image bytes. A sanitized failing camera body and transport metadata remain necessary to establish the observed camera's cause. |
| F11 | Fresh Windows loopback capacity acceptance on `cb25276`: 64/256 members and concurrent reads; all distinct identities/read results verified, zero failures. Startup/total: 360/932 ms and 1547/2378 ms. These are one smoke run, not latency ceilings, memory measurements, LAN/VMS or sustained-soak acceptance. |
| F12 | Windows ConPTY discover/manage/resize acceptance is rerun. The runner now waits for the completed resize frame and allows bounded fixture initialization. Human IME and native Linux/macOS remain open; see the record below. |
| F13 | Isolated native migration probe passes 3.6.3 ↔ 4.2.0 on Windows. Production stays on 3.6.3; other platforms, locked/denied/unavailable stores and reconnection remain open. |
| F14 | M4/M7 comparison and transition contracts below are prepared for review. No persona-switching or reference-value equivalence is claimed. |
| F15 | Health human `--details` is delivered. R2 observability/durability, descriptor reachability/examples and commercial acceptance remain code/design/evidence work. Native harness work supports these gates without closing R5. |

## Decision inputs

These are concrete proposed cuts, not silently approved new product scope.

- **Camera writes:** first candidate is one device's hostname. Preview records
  target identity, current and proposed values without credentials. Apply re-reads
  identity/current value, refuses a stale preview and sends at most one write.
  A timeout after transmission is an unknown outcome: read back before any retry;
  cancellation cannot promise rollback. Tests need stale identity/value, auth
  failure, disconnect before/after commit, partial readback and explicit recovery.
  Do not extend this to credentials, network settings, firmware or multi-device
  writes. A shipping decision and permission UX are still needed.
- **Batch file export:** first candidate is existing read-only inventory for an
  explicit fixed device selection, bounded concurrency, one new directory, unique
  filenames and a per-item outcome manifest. Never replace existing files; retain
  successful outputs on partial failure and report failed/cancelled entries.
  Choose selection/snapshot semantics before adding CLI parser paths.
- **RTSP/playback and navigation extraction:** decoder licensing/platform/resource
  budget and observable playback success need a separate choice. Navigation core
  remains internal until a second consumer and package/API/version are identified.
- **M7:** proposed first reference is an explicitly supplied fixture store.
  Compare only unambiguous exact operation/request identities; do not join solely
  on legacy canonical keys. Report missing/ambiguous/unparseable/unsupported
  separately from equal/different. Preserve raw evidence; volatile-field masking
  must be explicit, and changed semantic IDs must not silently disappear. Seed
  independent same-shape/different-value, changed namespace, key collision,
  missing-side, parse-failure and timestamp-only cases. This comparison cannot
  establish that a fixture is a correct physical-device reference.
- **M4:** proposed first transition is starting a new server from a validated
  synthetic/replay configuration, then swapping the owner-held handle only after
  startup succeeds. Failed startup keeps the old instance; the old endpoint is
  stopped explicitly after handover. Do not promise seamless same-port transition,
  implicit persistence, unauthenticated remote administration or write replay.
  Decide local owner API versus authenticated control endpoint before implementation.
- **Public distribution:** record APT base URL, suites/architectures, owner and
  recovery contact, signing fingerprint/expiry/rotation and retained downgrade
  versions; record tap repository owner and per-architecture formula/bottle hashes.
  With those actual values, exercise install → upgrade → downgrade/pin → remove,
  reject tampered/expired signatures and verify registry/credential retention policy.
  Existing CI-only keys and staging URLs must not become production defaults.

## Native evidence

Keyring probe (synthetic account only; a native store write is intentional):

```text
python -X utf8 packaging/check_keyring_migration.py --run-native
```

It creates an isolated Cargo project with exact keyring versions, leaves the
production lockfile untouched, and checks v3 create → v4 read/update → v3 rollback
read → v4 delete → v4 create → v3 read/delete → absence in both. Cleanup also runs
on ordinary error returns. Forced process/OS termination can interrupt cleanup;
use a disposable native session for acceptance. No existing account is enumerated
and raw backend errors/values are withheld. Windows used native-store 1.1.0;
the facade versus explicit-store production choice is still open.

Snapshot evidence:

```text
cargo run --example snapshot_probe --features health -- /path/to/captured-body
```

The report contains only byte count, recognized signature, trailing CR/LF count,
the signature after a diagnostic CR/LF trim, and `decoded=false`. It reads at most
16 MiB plus one guard byte; the source is untouched. This is not a decoder or an
image-acceptance policy change. Keep URL/query, cookies and authorization out of
shared evidence; record status, content type, byte length and authentication mode
separately after sanitization. No failing real camera sample is currently available.

Terminal/operator record for each OS and terminal:

| Required field | Evidence to capture |
| --- | --- |
| Environment | commit, executable hash, OS/build, terminal/version, keyboard layout, IME/version |
| Literal input | search/name/path/password: digits, `gg`, `j/k`, Unicode; composition confirm/cancel must not navigate |
| Navigation | pending count, escape, selection preservation, filter and mode changes |
| Resize | narrow/large window, complete repaint, cursor/selection and bottom status |
| Exit/recovery | Ctrl+C, operation cancellation, return-to-screen and terminal restoration |
| Result | pass/fail/not-run per row, sanitized reproduction; never replace human IME checks with injected ASCII |

Windows automated setup used pywinpty 3.0.5, pyte 0.8.2 and wcwidth 0.8.4 in an
ignored dependency directory. Initial manage startup and resize checks exposed
harness timing assumptions under build load. Standalone manage/resize reruns pass;
the adjusted runner retains the same screen, selection and restoration assertions.
Native LAN testing still needs an authorized interface and target VMS; no network
configuration or remote device state was changed by these checks.

Final Windows ConPTY run: all three modes (discover, manage, resize) pass together after the runner correction. The offline snapshot example unit test also passes; native human IME remains not-run.

The terminal host is Windows 10 build 19045; the tested CLI fixture executable's
SHA-256 is `692f987b6daed5b26a1df60c5104edc98243a1057902fcd8d075d5b987480996`.
The snapshot probe also passes binary checks for source preservation, oversize
refusal, missing/directory inputs and path redaction. Final combined workspace
acceptance: 1,351 all-feature / 1,235 default tests, seven ignored in each;
both all-target Clippy configurations and formatting pass. The 26 packaging
controls, explicit snapshot example test, 178-instance external corpus, native
credential probe and ConPTY/capacity runs are separate evidence, not additions
to the workspace count.

Delivery commits: `4a9d0a5` health details; `cb25276` RS1; `af1489f` EP1;
`c747297` offline snapshot probe; `6093d94` native keyring probe; `32d6f49`
terminal runner. Larger implementation items in the table remain open; this
register records one completed pass through every plan's feasible local work and
prepared inputs, not completion of everything that could eventually be coded.
