# EPIA2-27 bounded adversarial review / EPIA2-27限定敵対監査

## 日本語

### Snapshot・境界

対象はEpistesys repositoryのdirty working treeとHEAD `60cc71db2ba47517bbd209894033f9b55b3708f8`の合成である。単独HEADを実装済みsnapshotとは呼ばない。RepoSeiriの各queryは個別のbounded local projectionであり、自己参照を避けるため、この文書内にsource-session digestはpinせず、最終hand-offで最後の安定したdigestを報告する。

### Assurance-Compiler

- `assurance_doctor`: `ok=true`、deferred capabilities `none`、semantic digest `2d102a78fdecd3d325b5ce8c4ef0921fb390e86b7071b7634c1f7ade8823313a`。
- 22個の完全Rust unitをin-bandで個別compile。各unitは16KiB UTF-8上限内。全て`needs_evidence`、diagnostics 0。これらはsource-text static observationに限られ、crate/module依存、cfg/feature、Cargo build、proc macro、runtime、ホスト状態を含まない。
- `dgcl_final_output.rs`をrenderも実行し、`needs_evidence`のまま13個の外部symbol/type/module参照をunknownとして表示した。これはbounded-unit側に依存crateの証拠がないことを示すもので、missing implementationやrepository全体の結論へ読み替えない。

| Source unit | Bytes | Result | Input digest |
| --- | ---: | --- | --- |
| `scripts/lc631/crates/lc631-analysis/src/dgcl_final_output.rs` (whole module) | 7,682 | `needs_evidence`, 0 diagnostics | `sha256:584f8b5285076dbe099816119ce01aeae1fc1fe6805ac9bf77f43e76493917f4` |
| `evidence_lifecycle.rs::validate_dgcl_evidence_lease` | 1,997 | `needs_evidence`, 0 | `sha256:3792a82bf16fb3a5ca1959dcc8dee094b8b6b2c9319d43d2a81c178132dc2946` |
| `repair_loop.rs::permit_authorizes_repair` | 1,059 | `needs_evidence`, 0 | `sha256:ff7bc07d81dfe289693fdbe96e41936c8e1fa8860d49bcee3ad0739dedc74872` |
| `repair_loop.rs::application_matches_request` | 2,237 | `needs_evidence`, 0 | `sha256:dafb323ff939dd2bcdc4a9132582bf8708b3baaa0829e42358a1ca6cf5894023` |
| `repair_loop.rs::validation_preserves_unrelated_gaps` | 376 | `needs_evidence`, 0 | `sha256:ecb92a46c85e67125953da042bedbfe21756ddd5bf81261bb346407e23a41e46` |
| `closure_resume.rs::inspect_resume_with_anchor` | 1,665 | `needs_evidence`, 0 | `sha256:2799d525be13ec6fb05d335b1791d036dad8710ade4183a230e568af816e47a6` |
| `closure_resume.rs::validate_event_transition` | 1,568 | `needs_evidence`, 0 | `sha256:610fe9501a4ec323b0356a80660273e4de193ccda8adeb282e047ced2df576e9` |
| `distillation.rs::run_mutual_distillation` | 4,257 | `needs_evidence`, 0 | `sha256:76f775d0de39ca03abeab7932d9e95b87b6fed5a46f6ea35f57feea8b0008713` |
| `distillation.rs::apply_accepted_materializations` | 2,522 | `needs_evidence`, 0 | `sha256:10904a8ba049290e1faf9fc22e522c760f355a77ea550cb2b8bf65bee8fe6143` |
| `dgcl_gold.rs::evaluate_gold_decisions` | 8,780 | `needs_evidence`, 0 | `sha256:b69430f80b1bce3d576fcda3fd9ece14e9acb54ced4bd19792063944fa5c86d0` |
| `dgcl_gold.rs::validate_gold_provenance` | 6,237 | `needs_evidence`, 0 | `sha256:04a10c7954982c441ec075968e4ff8c76a1275d617b5a7072be96c61d9b750ab` |
| `lc631-host/src/lib.rs::host_output_stage_payload_digest` | 2,102 | `needs_evidence`, 0 | `sha256:7aed3e42319dc2ae919ece9dc18c5fbbabc20a38e6b8ae69d696226a95db6ddb` |
| `lc631-host/src/lib.rs::observe_host_output_stage_attested` | 1,847 | `needs_evidence`, 0 | `sha256:c3bfd7b4119eaebded1149d57233ae070eb712cff4f6613bad793b99dbacd921` |
| `kernel.rs::build_structural_kernel` | 2,213 | `needs_evidence`, 0 | `sha256:5a31ad8efc56a0fa496213fb1c5ec231621d389ef042d6334c1ba493485ec017` |
| `kernel.rs::distillation_kernel_digest` | 454 | `needs_evidence`, 0 | `sha256:ec2a93c79c14691b812cf158581f2e9f61d78cd8a473ad6450b4c938895fa4b6` |
| `geometry.rs::build_geometry_shadow` | 3,014 | `needs_evidence`, 0 | `sha256:b354c8472a1f7c467bdfb296c203071a02559f35b073861f7e06759299bd6133` |
| `evidence_lifecycle.rs::advance_dgcl_target_snapshot` | 1,342 | `needs_evidence`, 0 | `sha256:cf6c2b60cb5b1c81dd1f15e9cc98e94de43eae07c8a9b34fd764ffbc11ecd070` |
| `closure_resume.rs::verify_checkpoint_head_anchor` | 2,021 | `needs_evidence`, 0 | `sha256:190c155c142868cfe35831151c1e89bf5bb3ae7be8872be8b5bbdc2afb2a39fa` |
| `repair_loop.rs::verify_application_receipt` | 938 | `needs_evidence`, 0 | `sha256:45ae32b0ca3304564ae22ccac0f0b3eb4a98362897589ae4a6d801d3424fec58` |
| `repair_loop.rs::verify_validation_receipt` | 928 | `needs_evidence`, 0 | `sha256:eabe141aef800a3c7ed05c2d15b7a4292ba616f1bd195c60fccebca00c0801ef` |
| `dgcl_gold.rs::evaluate_gold` | 6,902 | `needs_evidence`, 0 | `sha256:904b5c8c36f7b9ef9b2bd41f05a217d188fe51de8553c1aa8c3117155c4b7ddb` |
| `lc631-host/src/lib.rs::host_output_stage_scope` | 296 | `needs_evidence`, 0 | `sha256:e51573ba310ff7eec82dfc02105fcbdcd465b3d3796262abc39326c61b999203` |

未監査: `run_dgcl_repair_loop`全体は16KiBを超えるため送信せず、CLI `run`全体、Cargo feature matrix、host callbacks、全部のclosure/analysis modules、repository graphも未投入。Assurance結果は人手のdiff/target testsを置換しない。

### RepoSeiri 1.1

- Profile `research`, scope `repository`; `summary`, `routes`, `linter`を照会。summary source-session digest `sha256:2a38a8f471a70bc9fbbc023da21db819ee486bb0293bc9e41bea27408b94eed4`はroutes/linter後のsummary再照会でも一致した。以降に監査/status文書を追加したので、このdigestはその追加前snapshotに対するもの。
- Summary: 28,170 entries、5,903 document events、13,762 facts、14 routes、63 content slots、2,258 capability nodes、52 selected documents、0 budget skips、0 diagnostics。Underclaim opportunity 1、overclaim risks 9、claims/findings 6/6、wording pattern matches 10、patch operations 0、writes files false。
- Routes: Docs `Overloaded`, Release `Routed`; Quickstart/Support/Intake/Contributing/Automation/Ownership/GovernanceはAbsent; Security/Lifecycle/LicenseはUnsafeToInvent。Missing route priorities 12: top Security (manual decision), Lifecycle (manual decision), Automation (guarded); these are route evidence, not directives to invent legal or security policy.
- Linter: 27 files, 3 generated surfaces, 6 suppressed boundary exceptions, zero wording findings. A clean wording lint is not a correctness or publication-readiness guarantee.

### 統合敵対所見

- Closure producer: production finalizer still produces only the route observation; producer coverage for structural/compiler/runtime-validation/acceptance remains partial. Normal coding closure is still held.
- Lifecycle/repair: snapshots are caller-supplied and not OS-isolated. Host filesystem confinement, live revocation polling and enforceable hard cancellation are absent; only permit scope and signed test adapter receipt contracts are observed.
- Replay/output: head verification is an API but CLI does not persist an external trusted head; CLI replay status stays Unanchored. Host precommit/postsend/sink stage API is not connected to real Codex callbacks or durable stage replay.
- Distillation: kernel overlays materialize only already-Verified source relations; no new independent evidence or semantic truth is created. Global backflow and CPU/GPU parity claims remain separate and unobserved.
- Evaluation and matrix: no actual GoldCorpus, decisions, rights review, holdout, independent adjudication, 3×3 grammar-family matrix, or full 16-mutation replay. RepoSeiri's 16 matrix map checks test symbol references only.
- Distribution: offline release binary runs in local profiles, but archive/package/remote CI/host pickup is not proven. Source version remains alpha.1.

## English

### Snapshot and evidence boundary

The target is the combination of HEAD `60cc71db2ba47517bbd209894033f9b55b3708f8` and the dirty working tree at review time; HEAD alone is not called the implementation snapshot. Version remains `6.3.2-alpha.1`. RepoSeiri queries are separate bounded local projections; to avoid a self-referential digest, this document does not pin a source-session digest, which will be reported in the final hand-off.

### Assurance-Compiler

- `assurance_doctor`: `ok=true`, deferred capabilities `none`, semantic digest `2d102a78fdecd3d325b5ce8c4ef0921fb390e86b7071b7634c1f7ade8823313a`.
- Compiled 22 complete Rust units in-band, each at most 16 KiB. Every surface was `needs_evidence` with zero diagnostics. These are source-text static observations, not crate-wide compilation evidence and do not cover module dependencies, cfg/features, Cargo builds, proc macros, runtime, or host state.
- Also rendered `dgcl_final_output.rs`; it remained `needs_evidence` and listed 13 external symbol/type/module references as unknown. This indicates dependencies were absent from the bounded-unit input; it is not a finding of missing implementation or a repository-wide conclusion.

| Source unit | Bytes | Result | Input digest |
| --- | ---: | --- | --- |
| `dgcl_final_output.rs` (whole module) | 7,682 | `needs_evidence`, 0 diagnostics | `sha256:584f8b5285076dbe099816119ce01aeae1fc1fe6805ac9bf77f43e76493917f4` |
| `evidence_lifecycle.rs::validate_dgcl_evidence_lease` | 1,997 | `needs_evidence`, 0 | `sha256:3792a82bf16fb3a5ca1959dcc8dee094b8b6b2c9319d43d2a81c178132dc2946` |
| `repair_loop.rs::permit_authorizes_repair` | 1,059 | `needs_evidence`, 0 | `sha256:ff7bc07d81dfe289693fdbe96e41936c8e1fa8860d49bcee3ad0739dedc74872` |
| `repair_loop.rs::application_matches_request` | 2,237 | `needs_evidence`, 0 | `sha256:dafb323ff939dd2bcdc4a9132582bf8708b3baaa0829e42358a1ca6cf5894023` |
| `repair_loop.rs::validation_preserves_unrelated_gaps` | 376 | `needs_evidence`, 0 | `sha256:ecb92a46c85e67125953da042bedbfe21756ddd5bf81261bb346407e23a41e46` |
| `closure_resume.rs::inspect_resume_with_anchor` | 1,665 | `needs_evidence`, 0 | `sha256:2799d525be13ec6fb05d335b1791d036dad8710ade4183a230e568af816e47a6` |
| `closure_resume.rs::validate_event_transition` | 1,568 | `needs_evidence`, 0 | `sha256:610fe9501a4ec323b0356a80660273e4de193ccda8adeb282e047ced2df576e9` |
| `distillation.rs::run_mutual_distillation` | 4,257 | `needs_evidence`, 0 | `sha256:76f775d0de39ca03abeab7932d9e95b87b6fed5a46f6ea35f57feea8b0008713` |
| `distillation.rs::apply_accepted_materializations` | 2,522 | `needs_evidence`, 0 | `sha256:10904a8ba049290e1faf9fc22e522c760f355a77ea550cb2b8bf65bee8fe6143` |
| `dgcl_gold.rs::evaluate_gold_decisions` | 8,780 | `needs_evidence`, 0 | `sha256:b69430f80b1bce3d576fcda3fd9ece14e9acb54ced4bd19792063944fa5c86d0` |
| `dgcl_gold.rs::validate_gold_provenance` | 6,237 | `needs_evidence`, 0 | `sha256:04a10c7954982c441ec075968e4ff8c76a1275d617b5a7072be96c61d9b750ab` |
| `lc631-host/src/lib.rs::host_output_stage_payload_digest` | 2,102 | `needs_evidence`, 0 | `sha256:7aed3e42319dc2ae919ece9dc18c5fbbabc20a38e6b8ae69d696226a95db6ddb` |
| `lc631-host/src/lib.rs::observe_host_output_stage_attested` | 1,847 | `needs_evidence`, 0 | `sha256:c3bfd7b4119eaebded1149d57233ae070eb712cff4f6613bad793b99dbacd921` |
| `kernel.rs::build_structural_kernel` | 2,213 | `needs_evidence`, 0 | `sha256:5a31ad8efc56a0fa496213fb1c5ec231621d389ef042d6334c1ba493485ec017` |
| `kernel.rs::distillation_kernel_digest` | 454 | `needs_evidence`, 0 | `sha256:ec2a93c79c14691b812cf158581f2e9f61d78cd8a473ad6450b4c938895fa4b6` |
| `geometry.rs::build_geometry_shadow` | 3,014 | `needs_evidence`, 0 | `sha256:b354c8472a1f7c467bdfb296c203071a02559f35b073861f7e06759299bd6133` |
| `evidence_lifecycle.rs::advance_dgcl_target_snapshot` | 1,342 | `needs_evidence`, 0 | `sha256:cf6c2b60cb5b1c81dd1f15e9cc98e94de43eae07c8a9b34fd764ffbc11ecd070` |
| `closure_resume.rs::verify_checkpoint_head_anchor` | 2,021 | `needs_evidence`, 0 | `sha256:190c155c142868cfe35831151c1e89bf5bb3ae7be8872be8b5bbdc2afb2a39fa` |
| `repair_loop.rs::verify_application_receipt` | 938 | `needs_evidence`, 0 | `sha256:45ae32b0ca3304564ae22ccac0f0b3eb4a98362897589ae4a6d801d3424fec58` |
| `repair_loop.rs::verify_validation_receipt` | 928 | `needs_evidence`, 0 | `sha256:eabe141aef800a3c7ed05c2d15b7a4292ba616f1bd195c60fccebca00c0801ef` |
| `dgcl_gold.rs::evaluate_gold` | 6,902 | `needs_evidence`, 0 | `sha256:904b5c8c36f7b9ef9b2bd41f05a217d188fe51de8553c1aa8c3117155c4b7ddb` |
| `lc631-host/src/lib.rs::host_output_stage_scope` | 296 | `needs_evidence`, 0 | `sha256:e51573ba310ff7eec82dfc02105fcbdcd465b3d3796262abc39326c61b999203` |

Unreviewed: the complete `run_dgcl_repair_loop` exceeds 16 KiB and was not submitted; the complete CLI `run` function, Cargo feature matrix, host callbacks, all closure/analysis modules, and repository graph were not submitted either. Assurance results do not replace human diff review or tests.

### RepoSeiri 1.1

- Profile `research`, scope `repository`; queries were summary, routes, and linter. Summary source-session digest `sha256:2a38a8f471a70bc9fbbc023da21db819ee486bb0293bc9e41bea27408b94eed4` remained the same on a summary rerun after routes/linter. Audit and status notes were added afterward, so this digest refers to the pre-addition snapshot.
- Summary: 28,170 entries, 5,903 document events, 13,762 evidence facts, 14 routes, 63 content slots, 2,258 capability nodes, 52 selected documents, no budget skips, zero diagnostics. Underclaim opportunities 1, overclaim risks 9, claims/findings 6/6, wording-pattern matches 10, patch operations 0, writes files false.
- Routes: Docs `Overloaded`, Release `Routed`; Quickstart/Support/Intake/Contributing/Automation/Ownership/Governance `Absent`; Security/Lifecycle/License `UnsafeToInvent`. Twelve route priorities were reported, led by Security (manual decision), Lifecycle (manual decision), and Automation (guarded). These are route observations, not directions to invent legal or security policy.
- Linter: 27 files, 3 generated surfaces, 6 suppressed boundary exceptions, zero wording findings. A clean wording lint is not a correctness or publication-readiness guarantee.

### Adversarial synthesis

- **Closure and producer wiring:** per-task closure binds source/plan/receipt/lifecycle, but the production finalizer still issues no producer classes beyond ProductionConnection; normal finalization remains Hold. EPIA2-14/17 release blockers.
- **Lifecycle and authority:** caller-supplied snapshot hashes are not OS snapshots. Repair permits check Edit/scope/source/expiry/revocation revision per attempt, but live revocation refresh, filesystem containment, hard timeout kill, and independent target-to-gap mapping are not connected.
- **Replay/output:** external trusted-head verification API and v2 checkpoint event phases exist, but the CLI does not persist/load a head outside the ledger; finalization reports `Unanchored`. Host output stage API is exercised only with test receipts; real Codex callbacks and durable stage replay are unobserved.
- **Distillation:** revisions now follow actual kernel overlay content. Only relations already Verified in the same source artifact are materialized; this creates no independent evidence or semantic truth. External evidence backflow across TLDG→DeepGrammar/Program IR/TL/closure remains unwired.
- **Evaluation/over-hold:** decision metrics and signed provenance verification APIs exist, but no GoldCorpus or decision set exists. Distinct signing keys are not equated with independent humans. EPIA2-24's 16 items are a test-reference manifest; full mutation replay, 3×3 grammar-family coverage, and independent holdout remain unmet.
- **Distribution:** offline release binary ran no-model and configured Stanza profiles from a temp cwd, but no source archive/package install, remote CI, or host pickup was observed. Model rights remain unreviewed. Source version remains alpha.1.
