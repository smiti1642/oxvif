# oxvif `docs/`

Project documentation that complements the crate API reference. Public guides
linked by the project README ship with the library package; development plans,
protocol transcriptions, and release records remain repository-only. Start
with the top-level [`README.md`](../README.md), the
[complete library guide](../LIBRARY_GUIDE.md), the
[complete CLI guide](oxvif-cli.md), the coverage tables in
[`OPERATIONS.md`](../OPERATIONS.md), or the library API on
[docs.rs](https://docs.rs/oxvif).

## Layout

| Path | What it holds |
|------|---------------|
| [`../README_zh.md`](../README_zh.md) | **專案首頁（繁體中文）** — the Traditional Chinese counterpart to the concise project README. |
| [`../LIBRARY_GUIDE.md`](../LIBRARY_GUIDE.md) | **Complete library and feature guide** — detailed session/client usage, service APIs, health checks, testing, Mock, and Metamorph, moved out of the concise project README. |
| [`../LIBRARY_GUIDE_zh.md`](../LIBRARY_GUIDE_zh.md) | **函式庫與功能指南（繁體中文）** — the Traditional Chinese counterpart to the complete library and feature guide. |
| [`../OPERATIONS.md`](../OPERATIONS.md) | **Implemented operation coverage** — exact per-service ONVIF coverage tables. |
| [`../OPERATIONS_zh.md`](../OPERATIONS_zh.md) | **已實作操作（繁體中文）** — the Traditional Chinese counterpart to the operation coverage tables. |
| [`reference/`](reference/) | **ONVIF protocol reference** — the WSDL/XSD transcriptions used while implementing oxvif. Stable facts, not plans. |
| [`active/`](active/) | **In-progress plans** — design docs / milestones for work that is under way or not yet finished. |
| [`done/release-0.17-approval.md`](done/release-0.17-approval.md) · [繁體中文](done/release-0.17-approval_zh.md) | Archived 0.17 approval packet; final publication and verification are recorded in `done/release-0.17-finalization.md`. |
| [`done/`](done/) | **Completed plans** — finished design/audit docs, kept as a record. |
| [`mock-server.md`](mock-server.md) | **Reference for `oxvif::mock`** — routing, envelope/namespace contracts, modeled state, seeded fixtures, operation behavior, request/response examples, faults and explicit simulation limits. The operation ledger owns current route counts; this reference is not a conformance certificate. |
| [`mock-server_zh.md`](mock-server_zh.md) | **Mock 裝置參考（繁體中文）** — the Traditional Chinese counterpart to the complete Mock behavior and fidelity contract. |
| [`mock-fleet.md`](mock-fleet.md) · [繁體中文](mock-fleet_zh.md) | Configurable multi-camera development runner: init/check/serve, explicit LAN discovery, state isolation and capacity-smoke limits. |
| [`oxvif-cli.md`](oxvif-cli.md) | **Complete CLI guide** — installation, human and Agent workflows, inventory, discovery, fleet execution, credentials, TLS, structured output, typed errors, and exit codes. |
| [`oxvif-cli_zh.md`](oxvif-cli_zh.md) | **CLI 使用指南（繁體中文）** — the Traditional Chinese companion to the complete CLI guide. |
| [`cli-maintenance.md`](cli-maintenance.md) · [繁體中文](cli-maintenance_zh.md) | CLI 0.17 image downloads, layered diagnosis, configuration inventory/diff, safety boundaries and manual acceptance. |
| [`dependency-pitfalls.md`](dependency-pitfalls.md) | Standing engineering guide (feature-unification footguns). Not a plan and not ONVIF reference, so it sits at the root; referenced from the release SOP in `CLAUDE.md`. |
| [`support.md`](support.md) | Versioned support boundaries for the Rust library and CLI beta, including OS, credential, schema, TLS, camera-evidence, and commercial-claim limits. |
| [`support_zh.md`](support_zh.md) | **支援與相容性政策（繁體中文）** — the Traditional Chinese counterpart to the support policy. |
| [`releases/`](releases/) | Version-specific release notes and verification evidence. Entries marked unreleased describe staging, not public availability. |
| [0.17 summary](releases/0.17.0.md) · [繁體中文](releases/0.17.0_zh.md) | 0.17 user-facing release notes and publication status. |
| [0.17 full changelog](releases/0.17.0-changelog.md) · [繁體中文](releases/0.17.0-changelog_zh.md) | CLI, Library, Mock, migration, evidence and limitations. |

A plan graduates from `active/` to `done/` when its scoped milestones are shipped.
A completed child batch may be archived while its broader parent programme remains
active. Keep explicit follow-up ownership and the original evidence limits; publication
alone does not turn unperformed hardware or native-terminal checks into passes.

## `reference/` — ONVIF protocol

A per-service catalogue of ONVIF operations transcribed from the official ONVIF
WSDLs, for cross-reference while implementing oxvif. See
[`reference/README.md`](reference/README.md) for the full service index,
conventions, and attribution/licensing.

## `active/` — in-progress plans

Reviewed 2026-09-22 against the recorded 0.17 publication and individual batch
evidence. The [post-0.17 backlog](active/post-0.17-backlog.md)
([繁體中文](active/post-0.17-backlog_zh.md)) owns follow-up scope;
the [0.17 cut](done/release-0.17-cut.md) is now a historical release record.

2026-09-22 歸檔與校準合計將 27 組已交付計畫／批次的 46 份 Markdown 文件，
以及一份原始審查 JSON 移至 `done/`。原 16 組 active 文件已逐份核對，
相依套件分組計畫補齊 GitHub 證據、clone 計畫補齊 G3 後結案；下表保留 14 組：
未執行的實機、原生終端與 VMS 驗證不會因發布或歸檔而視為通過。

| Doc | Status / scope |
| --- | --- |
| [`post-0.17-backlog.md`](active/post-0.17-backlog.md) · [繁體中文](active/post-0.17-backlog_zh.md) | Current follow-up index: remaining F01–F15 scope after 0.17 publication; delivered slices are distinguished from unfinished acceptance. |
| [`oxvif-cli-plan.md`](active/oxvif-cli-plan.md) | CLI product roadmap. The diagnostic CLI is published; later stages, device writes and product expansion are not completed by that release. |
| [`oxvif-cli-release-hardening-plan.md`](active/oxvif-cli-release-hardening-plan.md) | Broader reliability, security, support and commercial-pilot requirements; diagnostic-beta publication does not close the whole programme. |
| [`oxvif-cli-three-platform-distribution-plan.md`](active/oxvif-cli-three-platform-distribution-plan.md) | Native artifacts shipped. Durable project APT/tap operations, recovery/signing ownership and official-channel graduation remain separate work. |
| [`cli-vim-navigation-plan.md`](active/cli-vim-navigation-plan.md) · [繁體中文](active/cli-vim-navigation-plan_zh.md) | M1–M5 shipped in 0.17. M6 standalone navigation crate remains deferred; native terminal/IME evidence limits are retained. |
| [`metamorph.md`](active/metamorph.md) | M0–M3 and M5–M6 delivered; full M4 control plane/Persona A and M7 reference-value comparison remain incomplete. Structural diff and own-parser verification already exist. |
| [`mock-fleet-basic-plan.md`](active/mock-fleet-basic-plan.md) · [繁體中文](active/mock-fleet-basic-plan_zh.md) | Basic runner shipped in 0.17; actual LAN multicast/VMS and remaining native lifecycle acceptance are not established by loopback capacity smoke. |
| [`mock-fidelity-hardening-plan.md`](active/mock-fidelity-hardening-plan.md) · [繁體中文](active/mock-fidelity-hardening-plan_zh.md) | Active parent programme. Selected shipped slices do not complete service semantics, security, capability consistency or conformance. |
| [`mock-fidelity-execution-checklist.md`](active/mock-fidelity-execution-checklist.md) · [繁體中文](active/mock-fidelity-execution-checklist_zh.md) | Current W00–W26 work packages, operation cards, dependencies and remaining acceptance; retains PARTIAL/TODO dispositions. |
| [`mock-fidelity-operation-ledger.md`](active/mock-fidelity-operation-ledger.md) · [繁體中文](active/mock-fidelity-operation-ledger_zh.md) | Maintained source inventory of routes/readers and per-operation scope; used by the standing inventory checker. |
| [`mock-fidelity-source-audit.md`](active/mock-fidelity-source-audit.md) · [繁體中文](active/mock-fidelity-source-audit_zh.md) | Source-audit findings and migration ownership supporting unfinished batches; K27 repair does not close remaining W19 work. |
| [`mock-fidelity-profile-preflight.md`](active/mock-fidelity-profile-preflight.md) · [繁體中文](active/mock-fidelity-profile-preflight_zh.md) | Profile/binding readiness and remaining broad handler-migration controls; shipped profile slices do not close the entire preflight. |
| [`mock-fidelity-pipeline-preflight.md`](active/mock-fidelity-pipeline-preflight.md) · [繁體中文](active/mock-fidelity-pipeline-preflight_zh.md) | Unfinished request-chain/replay dependency work; historical K27 reproductions are retained with the later repair disposition. |
| [`mock-fidelity-schema-preflight.md`](active/mock-fidelity-schema-preflight.md) · [繁體中文](active/mock-fidelity-schema-preflight_zh.md) | Remaining W20/W21 schema-verification scope and corpus expansion; selected passing instances are not whole-programme acceptance. |

## `done/` — completed plans

The archived records preserve dated counts, revisions, authorization boundaries
and test limitations. Their archive notices identify the delivered scope and
supersede historical “unreleased” or pending-publication text. Immutable tag/commit
links still refer to their original paths. The
[0.17 review ledger](done/release-0.17-review-ledger.json) retains its original
input paths and hashes; it is historical evidence, not a current file inventory.

| Doc | Status / scope |
| --- | --- |
| [`audit-2026-05.md`](done/audit-2026-05.md) | 0.9.8: implemented-operation audit findings resolved. |
| [`metamorph-container-and-quirk-diff.md`](done/metamorph-container-and-quirk-diff.md) | 0.13: network clone container and structural quirk diff; broader Metamorph remains active. |
| [`metamorph-clone-in-oxdm.md`](done/metamorph-clone-in-oxdm.md) | Oxvif G1/G3 delivered, G2 superseded; G3 summary is unreleased. Broader M4/M7 and downstream UI remain separate. |
| [`refactor-2026-07.md`](done/refactor-2026-07.md) | 0.14: staged bug fixes and all 64 scoped coverage repairs completed. |
| [`stage4-ledger.md`](done/stage4-ledger.md) | Historical baseline supporting the completed 0.14 Stage 4 work; not a current coverage report. |
| [`mock-audit-2026-07.md`](done/mock-audit-2026-07.md) | 0.15: all four original audit tiers closed; later fidelity work has its own active programme. |
| [`ptz-wiring-plan-2026-07.md`](done/ptz-wiring-plan-2026-07.md) | 0.15: PTZ configurations, nodes and spaces; all three implementation stages delivered. |
| [`schema-shape-plan-2026-08.md`](done/schema-shape-plan-2026-08.md) | 0.15: structural checker delivered; completed execution and findings are recorded in the conformance sweep. |
| [`mock-schema-conformance-2026-08.md`](done/mock-schema-conformance-2026-08.md) | 0.15: bounded schema-shape sweep complete, with checker limitations retained. |
| [`service-capabilities-and-ptz-tours.md`](done/service-capabilities-and-ptz-tours.md) | 0.15: service capabilities and PTZ tours delivered. |
| [`tier1-implementation-map.md`](done/tier1-implementation-map.md) | 0.15: implementation mapping and verification record for capabilities/PTZ tours. |
| [`oxvif-cli-human-ux-plan.md`](done/oxvif-cli-human-ux-plan.md) | Completed CLI human-output and interaction work; broader CLI roadmap remains active. |
| [`cli-maintenance-workflows-plan.md`](done/cli-maintenance-workflows-plan.md) · [繁體中文](done/cli-maintenance-workflows-plan_zh.md) | 0.17: snapshot download, staged diagnosis and read-only inventory/diff delivered. |
| [`cli-maintenance-ux-plan.md`](done/cli-maintenance-ux-plan.md) · [繁體中文](done/cli-maintenance-ux-plan_zh.md) | 0.17: parameter placement, profile picker and human/Agent report refinements delivered. |
| [`cli-manage-plan.md`](done/cli-manage-plan.md) · [繁體中文](done/cli-manage-plan_zh.md) | 0.17: guided maintenance workspace and targeted first-use critique completed. |
| [`cli-workflow-continuity.md`](done/cli-workflow-continuity.md) · [繁體中文](done/cli-workflow-continuity_zh.md) | 0.17: continuous discovery onboarding, device workspaces, retained results and file workflows delivered. |
| [`cli-0.17-reentry.md`](done/cli-0.17-reentry.md) · [繁體中文](done/cli-0.17-reentry_zh.md) | 0.17: search/selection continuity, fixed-bottom status and bounded acceptance follow-ups delivered. |
| [`snapshot-auth-repair.md`](done/snapshot-auth-repair.md) · [繁體中文](done/snapshot-auth-repair_zh.md) | 0.17: shared snapshot HTTP/authentication repair; separate image-format investigation remains F10. |
| [`contributor-pr-integration-plan.md`](done/contributor-pr-integration-plan.md) · [繁體中文](done/contributor-pr-integration-plan_zh.md) | 0.17: credited adaptations of #14/#17/#16 delivered; this does not claim the original PRs were merged. |
| [`mock-fidelity-pr16-integration.md`](done/mock-fidelity-pr16-integration.md) · [繁體中文](done/mock-fidelity-pr16-integration_zh.md) | 0.17: two Media synchronization operations and B16 cards delivered with explicit receipt-only limits. |
| [`mock-fidelity-auth-preflight.md`](done/mock-fidelity-auth-preflight.md) · [繁體中文](done/mock-fidelity-auth-preflight_zh.md) | 0.17: bounded WS-Security parser migration delivered; freshness, replay protection and roles remain active. |
| [`mock-fidelity-ack-policy-preflight.md`](done/mock-fidelity-ack-policy-preflight.md) · [繁體中文](done/mock-fidelity-ack-policy-preflight_zh.md) | 0.17: classified acknowledgment-policy slices delivered; wider W16/W17 remain partial. |
| [`mock-fidelity-profile-assembly.md`](done/mock-fidelity-profile-assembly.md) · [繁體中文](done/mock-fidelity-profile-assembly_zh.md) | 0.17: bounded profile assembly batch delivered; full Media semantics remain active. |
| [`mock-fidelity-video-source.md`](done/mock-fidelity-video-source.md) · [繁體中文](done/mock-fidelity-video-source_zh.md) | 0.17: VS1 delivered, with operation and verification limits preserved. |
| [`mock-fidelity-video-encoder.md`](done/mock-fidelity-video-encoder.md) · [繁體中文](done/mock-fidelity-video-encoder_zh.md) | 0.17: VE1/K34 delivered, with broader encoder/fidelity work remaining active. |
| [`mock-fidelity-audio-metadata.md`](done/mock-fidelity-audio-metadata.md) · [繁體中文](done/mock-fidelity-audio-metadata_zh.md) | 0.17: AM1/D4 and its 15 operation cards delivered; broader W10 is not closed. |
| [`mock-fidelity-osd-crud.md`](done/mock-fidelity-osd-crud.md) · [繁體中文](done/mock-fidelity-osd-crud_zh.md) | OS1 completed 2026-09-23: bounded atomic OSD CRUD, client XML repair and independent wire validation; broader W10 remains active. |
| [`dependency-maintenance-plan.md`](done/dependency-maintenance-plan.md) · [繁體中文](done/dependency-maintenance-plan_zh.md) | Integration and subsequent grouped-update behavior verified on 2026-09-22. F13's six updates and rustls advisory fix are integrated locally; keyring migration remains separate. Current review evidence lives in [dependency pitfalls](dependency-pitfalls.md#post-017--reviewed-maintenance-batch-2026-09-22). |
| [`release-0.17-review.md`](done/release-0.17-review.md) · [繁體中文](done/release-0.17-review_zh.md) | Frozen candidate review and later repair evidence; companion JSON ledger is preserved byte-for-byte. |
| [`release-0.17-approval.md`](done/release-0.17-approval.md) · [繁體中文](done/release-0.17-approval_zh.md) | Historical approval packet; publication stop satisfied and 0.17 released. |
| [`release-0.17-cut.md`](done/release-0.17-cut.md) · [繁體中文](done/release-0.17-cut_zh.md) | Bounded 0.17 scope/acceptance record; no longer the active release gate. |
| [`release-0.17-finalization.md`](done/release-0.17-finalization.md) · [繁體中文](done/release-0.17-finalization_zh.md) | Final native CI, staging, package checks and successful public publication on 2026-09-14. |

---

Cross-references from source code, `Cargo.toml`, and `CLAUDE.md` point at the
current path of each doc. When you move a doc between buckets, update those
pointers too (grep the repo for the old path).
