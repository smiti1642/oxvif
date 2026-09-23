# Autonomous remaining-plan execution

[English](autonomous-completion-schedule.md) | [繁體中文](autonomous-completion-schedule_zh.md)

Status: IN-PROGRESS. Authorized 2026-09-23, starting at `6ec107f`.
This is the execution queue for existing [F01–F15](post-0.17-backlog.md),
not another product roadmap or an assertion that the parent plans are complete.
The maintainer requested every remaining item be scheduled, feasible work completed,
blocked items skipped while other work continues, and one final consolidated report.

| Section | Purpose |
| --- | --- |
| [Execution policy](#execution-policy) | Completion, compatibility and delivery |
| [Queue](#queue) | All remaining work families and acceptance |
| [Evidence and blockers](#evidence-and-blockers) | Durable continuation record |

## Execution policy

- Work sequentially in independently testable service/product batches. Follow the
  existing W01 cards and C01–C12 controls before changing Mock handlers.
- Make routine implementation choices autonomously. A difficult implementation
  is not a blocker. Preserve public API compatibility; prefer private adapters.
- Keep external schemas/captures outside the checkout. Verify behavior, refusal
  preservation, appropriate mutation controls and both transports as applicable.
- Commit each completed batch after repository gates; update bilingual evidence,
  move only fully completed scoped plans to done, and fast-forward/push
  master and develop. No PR, force push, release, package publication or physical
  camera modification is included.
- Record exact unavailable inputs/platforms or unresolved incompatible product
  choices, finish independent portions, then continue another row. Never mark
  missing hardware evidence as passed or use it to defer unrelated coding.
- Execute continuously in this task; this is a dependency-ordered work queue,
  not a time-based automation. The mistakenly created heartbeat was deleted
  immediately after the maintainer clarified this on 2026-09-23. No routine
  completion notification per batch; report once all feasible work is finished,
  with every remaining blocker and the input needed to unblock it. Interactive
  execution may still expose tool progress.

## Queue

Rows are ordered by useful dependencies, not promised dates. TODO means review
and implementation remain; it is not a claim that the existing service is absent.

| ID | Existing owner | Status | Planned work and completion boundary |
| --- | --- | --- | --- |
| Q01 | F04 / W15 | PARTIAL: EP2 delivered | Events per-subscription endpoint identity, filter/queue isolation, bounded capacity, renew/expiry/unsubscribe; preserve RequestCtx construction; deterministic clock/lifecycle tests and independent wire validation |
| Q02 | F05 / W07–W09 | PARTIAL: AF1 delivered | Auth freshness, bounded atomic nonce-reuse rejection and expiry; HTTP binding/status/header policy; structured versus deliberately raw fault injection, roles and refusal preservation |
| Q03 | F01 / W10 | TODO | Remaining Media URI/source-mode/binding contracts and OSD extensions beyond OS1; explicit supported/refused fields with both-view consistency |
| Q04 | F02 / W11–W12 | TODO | PTZ spaces/configuration/presets/home/tours and Imaging/focus per-source modeled effects, limits and precise errors |
| Q05 | F03 / W13–W14 | TODO | Device/DeviceIO users/network/scopes/storage/relay contracts and Recording/Search/Replay lifetimes, termination and cascades; synthetic effects only |
| Q06 | F06 / W16–W19 | TODO, alongside Q01–Q05 | Remaining capability/effect/read dependencies, atomicity, instance isolation and committed replay invalidation; assess standalone replay/key design separately |
| Q07 | F07 / W20–W23 | TODO, alongside service batches | Remaining QName/wildcard/negative checks, external corpus and bounded fuzz/property tests; retain independent validation and coverage limits |
| Q08 | F15 / CLI hardening | TODO | Observability, clock-skew and registry durability, command descriptor reachability and executable examples; test recovery and preserve Agent output contracts |
| Q09 | F11 / Mock Fleet | TODO | Remaining discovery protocol and bounded lifecycle/load controls; inspect available native/LAN/VMS evidence and record only unavailable acceptance as blocked |
| Q10 | F14 / Metamorph M4/M7 | TODO | Local-owner mode transition and explicit-reference comparison candidates from the acceptance register; verify identity, ambiguity, failure preservation and teardown; record any unavoidable public API decision |
| Q11 | F08 / CLI roadmap | TODO | Evaluate and deliver bounded batch export, controlled-write preview/apply and RTSP/playback slices where existing product/platform constraints permit; do not execute writes on physical cameras |
| Q12 | F08/F12 / navigation | TODO | Existing native terminal/IME acceptance, cancellation/resize/restoration; assess separate crate against a real second consumer before extraction |
| Q13 | F09 / distribution | TODO | Complete local packaging/recovery/signature/lifecycle automation; inspect formal channel inputs and available native platforms; external admission needs ownership and signing evidence |
| Q14 | F10 / snapshot | TODO | Complete reproducible bounded diagnostic controls; actual camera-format root cause requires a sanitized failing response and transport metadata |
| Q15 | F13 / dependencies | TODO | Keyring migration compatibility, locked/denied/unavailable-store and recovery controls on available platforms; keep production dependency unchanged until its acceptance is met |
| Q16 | All parents / final audit | TODO | Reconcile every row with commits/tests and remaining inputs, archive eligible plans, verify branch synchronization and deliver one final report |

## Evidence and blockers

- Starting baseline: OS1 delivered at `6ec107f`; workspace clean, master/develop
  and both origin refs matched. Existing 1,359/1,241 test results are baseline
  evidence, not new runs for this queue.
- Q01 discovery: RequestCtx has public literal construction and no endpoint field.
  The implementation must carry transport endpoint identity through a private
  adapter rather than adding a required public field. Existing event state is
  per instance, but only one filter/queue exists; EP1 is retained as prior evidence.
- No newly encountered blocker has yet been classified. Existing missing input
  candidates are listed in [remaining acceptance](remaining-plan-acceptance.md);
  each row must inspect them before final disposition.

Q01 EP2 is delivered: [evidence](../done/mock-fidelity-event-lifecycle.md). Remaining W15 work is push/property synchronization, full-topic matching and normative WSNT Fault detail; these are not hardware blockers. Next independent batch: Q02 auth freshness and nonce replay protection.

AF1 (2026-09-23): [bounded auth freshness/replay](../done/mock-fidelity-auth-freshness.md) is delivered. Authenticated nonce admission is atomic and bounded, stale/future/replayed tokens refuse without device hooks, and clock rollback cannot reopen evicted intervals. External corpus: 224 XML / 60 Actions; AF1 request Security headers are removed for credential-free export. Roles, HTTP binding and structured/raw injection remain Q02 work.
