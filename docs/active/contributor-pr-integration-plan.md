# Contributor PR integration plan

[English](contributor-pr-integration-plan.md) | [繁體中文](contributor-pr-integration-plan_zh.md)

Date: 2026-09-11. Local review baseline: `ad564cb` on
`codex/mock-fidelity-hardening`. Status: **IN-PROGRESS; implementation authorized
on 2026-09-11**. Candidate branch: `codex/contributor-pr-integration`, based on
`ad564cb`; its hardening ancestry must not be shipped under the feature-only scope.
Existing PR checklists are author reports, not maintainer acceptance.

| Section | Purpose |
| --- | --- |
| [Scope and revisions](#scope-and-revisions) | Fixed contributions and boundaries |
| [Decisions](#decisions) | Recommended defaults and escalation conditions |
| [B17 notification origin](#b17-notification-origin) | Compatible API and deterministic listener tests |
| [B16 media synchronization](#b16-media-synchronization) | Reusable client and truthful mock policy |
| [B14 dependencies](#b14-dependencies) | Independent dependency acceptance |
| [Verification cadence](#verification-cadence) | Batch gates and evidence reuse |
| [Integration and attribution](#integration-and-attribution) | Branch ancestry, credit and rollback |
| [Delivery checklist](#delivery-checklist) | Documents and finite completion conditions |
| [Execution record](#execution-record) | Measured outcomes and remaining evidence |

## Scope and revisions

| Batch | Reviewed head | Target observed on 2026-09-11 | Disposition |
| --- | --- | --- | --- |
| B17 / [PR #17](https://github.com/smiti1642/oxvif/pull/17) | `89f8125863668006dda395f0fd54253ab5a4aac5` | `develop`, `9dccf9df4099183d7b1edbb1941c7610ab27dfc0` | Keep peer capture; redesign the public surface and tests |
| B16 / [PR #16](https://github.com/smiti1642/oxvif/pull/16) | `3db6459b03b6977b3a41b6055ee3f8ac9f42b049` | `master`, `9dccf9df4099183d7b1edbb1941c7610ab27dfc0` | Keep client/session contribution; replace legacy mock handler |
| B14 / [PR #14](https://github.com/smiti1642/oxvif/pull/14) | `7c4db43efe4dbc28f8ed2093f5d08fde5eeb5114` | `master`, `03b8d30dd8a78a6224a13035978f7fa94ae5f9e1` | Review separately; old green CI is not combined-tree evidence |

Re-fetch heads, bases and checks at batch entry and before integration. A changed
head requires an incremental review and renewed affected evidence. PR #14's title
and body disagree on the update count; use the actual manifest/lock diff.
Do not interpret an empty check list (#16/#17 at review) as passing CI.

The maintainer explicitly included #14 in this round. The order is B14, B17,
then B16, with independent commits: establish the dependency baseline first so
later code is not accepted against immediately obsolete dependency inputs.
Do not wait for all W00–W25 work to
design these features. Conversely, completing these features does not complete
the mock-fidelity programme or authorize shipping unfinished prerequisites.
No release, version bump, installation, new CLI command, contributor-branch force
push, PR comment/closure or main-branch merge is part of this planning change.

## Decisions

| ID | Recommended implementation decision | Escalate only if |
| --- | --- | --- |
| I1 | Preserve `NotificationMessage`, its struct-literal construction, serde shape and the existing listener signature | A breaking replacement is necessary rather than an additive API |
| I2 | Add `ReceivedNotification { message, peer }` and an opt-in listener; `peer` is a TCP `SocketAddr`, not authenticated camera identity | A public identity/trust/proxy-header mechanism is requested |
| I3 | Use existing exact-operation acknowledgment policy for Media1 and Media2 separately; default refusal, opt-in receipt only | Real stream generation or different fidelity defaults are requested |
| I4 | Keep existing Events synchronization semantics/names separate; retain the two proposed Media client method names | API naming conflicts are found in the actual integration base |
| I5 | Preserve contributor credit, add maintainer corrections as a cohesive tested commit | Contributor coordination or remote actions need new authority |
| I6 | Keep dependency updates independent and retain Rust 1.88, CLI schema v3 and existing behavior contracts | An update requires MSRV/schema changes or unrelated migration |

These defaults avoid another open-ended design cycle. Implementation authorization
can accept this plan as a unit; no additional product decision is currently needed.

## B17 notification origin

Owner at execution: maintainer. Status: TODO. No dependency on W15 completion.

### Files and API

- `src/types/events.rs`, `src/types/mod.rs`, `src/lib.rs`: export the new wrapper
  without adding a field to `NotificationMessage`. Proposed wrapper fields are
  `message: NotificationMessage` and `peer: std::net::SocketAddr`; push connections
  always have a peer. Do not add this field to the ONVIF parser or to PullMessages.
- `src/client/events.rs`: add async
  `notification_listener_with_peer(bind_addr) -> std::io::Result<...stream...>`.
  Bind before returning; callers can observe bind errors and know it is ready.
  Factor a private helper accepting an already-bound `TcpListener`. Tests bind
  port zero and retain ownership rather than releasing and reacquiring a port.
- Preserve the old synchronous `notification_listener` return type and payload.
  Share parsing/dispatch and map the wrapper to `message` for legacy users;
  document that the old API retains its existing bind-error limitations.
- Use the actual `accept()` address; never replace it with `Forwarded`,
  `X-Forwarded-For`, XML Source fields or reverse DNS. Do not log addresses by
  default. The wrapper has no automatic serde derive in this initial scope;
  legacy event JSON remains unchanged, and explicit address export is caller-owned.
- Bound listener/connection task lifetime to stream lifetime: stop accepting
  and cancel/join owned connection tasks on receiver drop. Preserve existing
  message/body limits. Do not turn this change into a new HTTP/TLS/authentication
  server or claim that the existing listener is hardened for Internet exposure.

### Acceptance cases

| ID | Required discriminating evidence |
| --- | --- |
| N01 | Loopback sender `local_addr()` equals delivered `peer`; assert topic/time/source/data too |
| N02 | Two simultaneous senders with identical XML remain distinguishable by full socket address; correlate each message explicitly |
| N03 | Several notifications in one POST all retain the same peer and distinct payloads; do not assume cross-connection arrival order |
| N04 | Legacy listener and PullMessages retain payload behavior; an external consumer still compiles an old struct literal, and serde JSON stays identical |
| N05 | Occupied bind address returns the OS error from the new async API; startup uses readiness/owned listener, no fixed sleep or free-port race |
| N06 | Drop stream with an idle accept and with an incomplete connection; prove owned tasks terminate within a bounded timeout |
| N07 | IPv4 passes; IPv6 loopback passes where available, otherwise explicit unavailable evidence rather than a silent success |
| N08 | Valid namespace-declared Notify fixture; misleading proxy headers cannot override the TCP peer; malformed input yields no invented event |

Primary tests: `src/tests/client/events_tests.rs`; propose a public API regression
target `tests/notification_origin.rs` if needed. Use existing transport helpers for
PullMessages and isolated downstream compilation for struct-literal compatibility.
Mutations: substitute a wrong peer and mix up per-connection attribution; intended
N01/N02 assertions must fail. Restore exactly before acceptance.

Documentation must distinguish source port from ONVIF service port, explain
NAT/proxy limitations, and state that the address is local transport metadata,
not an ONVIF wire extension or a camera authentication mechanism.

## B16 media synchronization

Owner at execution: maintainer. Status: TODO; maps to W26. See the
[detailed PR review](mock-fidelity-pr16-integration.md). This plan supersedes its
old prerequisite ordering, not its unresolved findings. The baseline now contains
scoped requests, structured faults, profile lookup and `AckOnlyOperation`; verify
the actual transitive dependencies instead of reimplementing these facilities.

### Files and behavior

- Reuse `src/client/media.rs`, `media2.rs` and `src/session.rs` contributions:
  `media_set_synchronization_point` and `set_synchronization_point_media2`.
  Keep complete service-specific Actions, escaped tokens and response checks.
- Replace the proposed `src/mock/services/media.rs` text-search handler with a
  scoped request implementation; add distinct `src/mock/dispatch.rs` routes.
  Complete two operation cards before coding, with the existing C01–C12 axes.
- Extend `src/mock/policy.rs` with distinct Media1/Media2 identities. The Events
  variant cannot enable either. Preserve common identity/auth/policy precedence:
  default unmodeled-effect refusal can precede operation field checks; opt-in
  requests must still pass scoped field/profile validation before acknowledgment.
- For opted-in requests reject absent/duplicate/mislocated/wrong-namespace tokens
  and unknown profiles with reviewed structured faults. Decode text exactly once;
  preserve significant token whitespace. Inspect one coherent state snapshot.
- Both refusal and opt-in acknowledgment leave state, hooks, event queues and
  replay retirement unchanged. Do not claim a commit, I-frame, PTZ refresh or RTP
  delivery. Explicit raw/recorded fixtures retain their documented precedence;
  verify this separately from the synthetic default policy.
- Extend `tests/mock_ack_policy.rs`, client Media test files, action snapshot,
  workflow, token discrimination, replay and corpus tests as applicable. Remove
  unconditional-success expectations rather than making the default permissive.

### Acceptance cases

| ID | Required discriminating evidence |
| --- | --- |
| S01 | Each client emits the exact Action/body/endpoint, including escaped and Unicode tokens; session selects the correct service |
| S02 | Each client preserves exact SOAP Fault payloads and rejects the wrong response wrapper; no bare `is_err()` assertions |
| S03 | Default Media1 and Media2 calls refuse; opt-in only enables its selected operation, never the other Media service or Events |
| S04 | Opt-in known/unknown/two-profile/special-token cases, duplicate and namespace/Header/Extension decoys produce exact outcomes |
| S05 | Full state snapshots, hook counts, queues and recorded read results remain unchanged on refusal and acknowledgment |
| S06 | In-process and HTTP behavior agree; adapters/replay do not bypass synthetic policy unintentionally; separate raw-fixture controls |
| S07 | Independent pinned external request/response/Fault validation passes; intended success/control and refusal instances are counted separately |
| S08 | Inventory reconciles new client Actions, routes/readers/cards and bilingual operation tables; no Media success claim leaks into Events |

Mutations: misroute one Action, bypass policy, corrupt an echoed Fault or select a
decoy token. The planned subgroup campaign must fail at the intended assertions.
No mock test can prove a real I-frame. Optional real-camera validation requires
separate permission for a synchronization request and observation of the affected
stream; CLI discovery or an HTTP success alone is not that evidence.

Recheck official Media1/Media2 Service documents and WSDL with the pinned-source
policy; store normative tables, downloaded sources and schema-derived artifacts
outside the repository. Document profile-associated stream synchronization, not
only video I-frames. Do not infer normative Fault mappings from the old mock.

## B14 dependencies

Status: TODO, included in this round and independently reviewable. See the
[maintenance policy](dependency-maintenance-plan.md); its prior acceptance covers
earlier PRs, not #14. Inspect the current `Cargo.lock` and
`crates/oxvif-cli/Cargo.toml` diff against the chosen base, including transitive
updates and already-applied versions. Do not replay an obsolete lockfile wholesale.

The reviewed PR changes only those two files, not Rust source or workflows.
The manifest edits are CLI **dev-dependencies**: jsonschema 0.52.1 to 0.53.0 and
shlex 1.3 to 2.0. The lockfile also updates production dependencies, including
futures/futures-core, thiserror and toml, and their transitive graph. Dependency-only
therefore does not mean test-only or automatically behavior-compatible.

Review thiserror, futures/futures-core, toml, jsonschema and shlex changes against
upstream releases and actual call sites. In particular check shlex's major-version
API removals, TOML configuration round trips, JSON Schema acceptance/rejection,
async behavior and Rust 1.88 resolution. Record security/license notices and any
new feature defaults. No opportunistic upgrade of unrelated packages.

Acceptance: locked build/tests, both Clippy modes, MSRV, `cargo audit`, reviewed
`cargo outdated`, affected CLI config/schema tests, XML feature-unification guard,
and current native CI. Run package dry-run and non-publishing staging on the final
shipping candidate when packaging inputs changed; this does not authorize install
on the user's system or publication. Never suppress an advisory to pass the gate.

## Verification cadence

Follow the [approved batch cadence](mock-fidelity-execution-checklist.md#batch-and-verification-cadence).
Each batch finishes source, targeted tests and paired documentation before its
acceptance run and cohesive commit. Do not run the whole workspace per helper.

1. Record exact base/head/toolchain/dependency/features and reuse matching green
   baseline evidence. Use compilation and focused tests while implementing.
2. One planned sensitivity campaign per code subgroup; use an unfiltered
   all-feature `--no-fail-fast` run as required by repository policy. Record which
   assertions actually failed; an earlier failure does not prove later checks.
3. Restore mutations, then run the per-code-commit gates once on the candidate:

   ```text
   rtk cargo fmt --all --check
   rtk cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
   rtk cargo clippy --locked --workspace --all-targets -- -D warnings
   rtk cargo test --locked --workspace --all-features --no-fail-fast
   rtk cargo test --locked --workspace --no-fail-fast
   ```

4. For changed public docs run both strict rustdoc configurations and doctests.
   For B16 run the inventory self-tests and external schema corpus/shape checks;
   for B17 independent XML inspection must reject an undeclared-prefix fixture.
   Schema success is structural evidence, not ONVIF certification.
5. At integration run Rust 1.88, individual supported feature Clippy checks and
   downstream feature-unification controls. A workspace default run can unify
   features through the CLI: include a truly default-feature library consumer.
6. Obtain Windows/Linux/macOS CI on the exact combined candidate. The current
   workflow only auto-runs selected branch names; for a new integration branch use
   its existing manual dispatch once authorized, not a new maintainer PR merely
   to trigger CI. Unavailable/running checks remain NOT-RUN/PENDING, never PASS.

After a failure, fix and rerun affected checks first, then establish final gates
for the changed candidate. Identical final-code evidence may be reused with its
original SHA/tooling context; dependency changes invalidate affected earlier gates.
Do not add full release/staging matrices to every small correction.

## Integration and attribution

1. Work only after implementation authorization. Inspect local/remote ancestry and
   dirty files. B17 can use `codex/pr17-notification-origin` from current `develop`
   without importing hardening. B16 can use `codex/pr16-media-sync` from accepted
   hardening prerequisites; B14 uses its own maintenance branch. Names are planned.
2. Preserve original authors/commit provenance where code is reused. Prefer a
   credited import followed by a maintainer correction, gating the complete batch
   before committing; if history is squashed or selectively rewritten, retain
   PR references and appropriate contributor attribution. Never forge approval.
3. Inspect the complete proposed merge diff and ancestry, not just feature files.
   A branch based on unfinished hardening cannot be merged wholesale into master
   under this feature plan. Either its base is independently accepted, a minimal
   prerequisite set is separately reviewed, or main-branch integration waits.
   B17 and B14 need not wait for B16 in that case.
4. Prefer one forward integration direction into `develop`, validate the exact
   candidate, then synchronize `master` only with explicit merge authority and
   accepted ancestry. Recheck any target movement and test conflict resolutions.
5. Do not open a replacement maintainer PR by default. If the original PR can be
   safely updated/merged with authorization, preserve that route; otherwise explain
   the credited implementation in a remote comment and close as superseded only
   after actual integration, with separate authorization for those remote actions.
6. Preserve branches/evidence until verified. Roll back a bad integrated batch by
   an explicit revert commit, not force-push/reset or deletion of contributor work.
   No automatic release, cargo publish, tag, system install or branch deletion.

## Delivery checklist

| Item | Status | Required closure |
| --- | --- | --- |
| B17 | TODO | N01–N08 evidence, compatible public API, lifecycle limits and paired guide examples |
| B16 / W26 | TODO | S01–S08 evidence, complete cards, explicit acknowledgment boundary |
| B14 | LOCAL-PASS | Dependency review, audit, Clippy/tests, MSRV and XML controls pass; hosted CI pending |
| Combined candidate | TODO | Exact SHA and ancestry accepted; final tests/CI actually complete |
| Remote integration | NOT AUTHORIZED | Explicit authority, credited commits and verified target branches |

Update each batch's public method/type rustdoc, `src/lib.rs`, CHANGELOG Unreleased
and `LIBRARY_GUIDE.md` / `_zh.md`. B16 also updates `OPERATIONS.md` / `_zh.md`,
Media reference pairs, mock-server pairs, affected examples, operation ledger,
source inventory, W26 and the detailed review. README receives only a short link
if needed, not API details. New CLI commands and their human/Agent schemas are
out of scope; any incidental CLI behavior change is a regression to investigate.

Long documents retain navigation tables and reciprocal language links. Keep
planning separate from release claims; do not put proposed features under Added
until implemented. Final report lists accepted commits, exact test/check outcomes,
unavailable evidence, compatibility changes and deferred unrelated findings.
Completion of B17/B16 requires no newly discovered unrelated service repairs.

## Execution record

B14 candidate reproduces the reviewed dependency versions without replacing the
current lockfile with the older branch's lockfile. No Rust code or workflow changed.
Only `shlex::split` is used, in CLI example-parsing tests; removed quote/join and
mutable-deref APIs are not used. jsonschema still disables default features.
The reviewed packages declare MIT or MIT/Apache-2.0 licenses and MSRVs no greater
than 1.85; actual Rust 1.88 verification remains required, not inferred from these
declarations. Upstream notes: [shlex](https://github.com/comex/rust-shlex/blob/master/CHANGELOG.md),
[jsonschema](https://github.com/Stranger6667/jsonschema/blob/master/CHANGELOG.md),
[futures](https://github.com/rust-lang/futures-rs/blob/main/CHANGELOG.md).

`cargo audit` passed against 410 locked dependencies on 2026-09-11.
`cargo outdated --workspace --root-deps-only` was reviewed: newer dirs, reqwest,
tokio-rustls, toml, ipnet, jsonschema and keyring remain outside #14's bounded update.
The reported obsolete keyring feature names concern candidate keyring 4, not a
change to the currently retained keyring 3.6.3.

B14 local gates passed: 1,272 all-feature tests and 1,167 workspace-default tests,
each with five existing ignored tests across 39 suites; both Clippy modes and fmt.
Rust 1.88 locked workspace/all-target/all-feature check passed. The independent
XML consumer passed all three cases with encoding off and on. No new production
logic/assertions were introduced, so no new mutation campaign was needed for this
dependency-only batch. Hosted combined-candidate acceptance remains pending.
