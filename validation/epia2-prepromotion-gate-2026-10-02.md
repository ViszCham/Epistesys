# EPIA2-28 Pre-promotion gate / 昇格前gate

## 日本語

この文書は修復前snapshotの歴史的なHold記録です。同日の後続[PrePromotionFunctionalPass](epia2-prepromotion-functional-pass-2026-10-02.md)とscopeを分けて参照してください。以下の観測結果自体は書き換えません。

### 判定

**`PrePromotionFunctionalPass`ではありません。Labyrinth-Codex / Epistesysは`6.3.2-alpha.1`のまま維持します。** A2-G00〜13のうち必須implementation closureが未達のため、EPIA2-29 metadata promotion、alpha.2 identity gate、commit/push/merge、plugin登録は実行しません。

### 実行済みgate

- `cargo fmt --all --check` — pass
- `cargo check --workspace --locked` — pass
- `cargo test --workspace --no-default-features --locked` — pass
- `cargo test --workspace --locked` — pass
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — pass
- `cargo build --release --locked --offline` — pass
- configured Stanza local-cache path, debug and release `lc631-dgcl-run`, temp cwd — 2 language regions observed; pipeline/candidate schema valid; host send/output commit remained false
- five legacy/current JSON shapes plus repair/candidate/host stage contracts under Draft 2020-12 — pass
- RepoSeiri matrix reference checker — 16 unique IDs and referenced test symbols pass; it is a traceability check, not mutation execution
- RepoSeiri 1.1 `research/repository` summary/routes/linter — source session digest `sha256:098c58900eedaa36d353209d5e76d1e4af7f31004e72bfaffc0ecfe71cbd9f5e`; summary repeated after routes/linter returned the same digest; writes false
- Assurance-Compiler doctor — `ok=true`; 22 bounded complete Rust units, each ≤16 KiB, all `needs_evidence`, diagnostics 0; see [EPIA2-27 review](epia2-27-adversarial-review-2026-10-02.md)

### A2-G00〜14 状態

| Gate | 状態 | 理由 |
| --- | --- | --- |
| G00 baseline/snapshot | Partial | 初期baseline・dirty差分保持記録あり。現snapshotの全owner hunk/dependency digest再固定はrelease dossierで未完了。 |
| G01 source/span/identity/budget | Pass (bounded profile) | workspace testとsource/span/property回帰pass。全Markdown/Rust/自然言語の意味完全性はclaimしない。 |
| G02 grammar matrix | Partial | parser positive/negativeが多数pass。全declared family 3×3＋境界交差の一括matrix未完成。 |
| G03 authority non-amplification | Partial | host-scoped permit regressionはpass。全projection/host invocationのproduction-wide enforcement未接続。 |
| G04 configured worker/resource | Pass (local profile) | pinned Stanza JA/ENのoffline local worker、framing/timeout/pipe regressionを通過。OS-level network isolationは未観測。 |
| G05 artifact→IR→TL→closure→candidate | Partial | no-model/configured CLIで一pipeline candidateを出す。full action evidence producerとsemantic output realizationが不足。 |
| G06 evidence producers | Blocked | ProductionConnection以外のStructuralObservation/CompilerObserved/RuntimeValidation/AcceptanceOracle production producerが未接続。 |
| G07 16 disconnects＋修復 | Partial | 16ケースのmanifest/reference mapはあるが、全mutationsのoriginal→cut→repair→same-gap-restore harnessは未実行。 |
| G08 lifecycle/replay/repair | Partial | core lease/head/repair tests pass。external head persistence、live revocation refresh、production edit adapter、hard cancel未接続。 |
| G09 discrete↔continuous state | Partial | Verified relation overlayでkernel revision/next inputが変化。新しい独立evidence、全体backflow、GPU parity未接続。 |
| G10 standalone/host output | Blocked | exact JSON candidateはcaptureされるがImplementationClosure Hold。host callback/sink/replayはPending。 |
| G11 v1/v2 compatibility | Partial | legacy fixture、v1 exact checkpoint serialization、v2 typed phase schema pass。実旧consumer negotiation/decoder compatibility未観測。 |
| G12 Rust/package/privacy | Partial | no-default/default suite、configured temp-cwd run、offline release build pass。source distribution archive、remote CI、model rights review未完了。 |
| G13 adversarial P0/P1/docs | Blocked | EPIA2-27で複数production P0/P1を残存記録。RepoSeiri/Assuranceは監査補助であり修復ではない。 |
| G14 alpha.2 identity | Not run | G00〜13必須gate不成立。source versionを先行変更しない。 |

### 未達条件と次の安全な段階

1. evidence producer policyをProductionConnection以外へ実装接続し、同じsource/plan/target/lifecycleに束縛する。
2. decision observationが全open gapを保持しつつ、supported actionの実production closureを示す受入経路を接続する。condition truth、semantic correctness、output realizationは独立evidenceなしに閉じない。
3. 16 disconnectsを実際に一つずつ断線・修復し、対象gapだけ開閉するmatrixを実行する。grammar family別positive/negativeとover-hold分母を追加する。
4. trusted checkpoint headをledger外で保存・取得し、CLI resumeへ接続する。RepairAdapterには実host permit, snapshot, process supervisorを接続する。
5. exact candidateはhost callback/sink stage receiptsとdurable replayへ接続し、実host未観測はPendingのまま維持する。

これらを直した後にA2-G00〜13を再評価し、全gate pass後のみEPIA2-29へ進みます。今回は6.3.2-alpha.2 metadata promotionは未実施です。commit/push/merge/reinstall/restartも行っていません。

## English

This document is the historical Hold record for the pre-repair snapshot. Read it separately from the later same-day [PrePromotionFunctionalPass](epia2-prepromotion-functional-pass-2026-10-02.md). The observations below are not rewritten.

### Decision

**This is not `PrePromotionFunctionalPass`. Labyrinth-Codex / Epistesys remains `6.3.2-alpha.1`.** Required implementation-closure gates among A2-G00..13 remain unmet, so EPIA2-29 metadata promotion, alpha.2 identity gates, commit/push/merge, and plugin registration are not performed.

### Executed gates

- `cargo fmt --all --check` — passed
- `cargo check --workspace --locked` — passed
- `cargo test --workspace --no-default-features --locked` — passed
- `cargo test --workspace --locked` — passed
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — passed
- `cargo build --release --locked --offline` — passed
- Configured Stanza local-cache path and debug/release `lc631-dgcl-run` from a temporary cwd — two language regions observed; pipeline/candidate schemas valid; host send/output commit remained false
- Five legacy/current JSON shapes plus repair/candidate/host-stage contracts under Draft 2020-12 — passed
- RepoSeiri matrix reference checker — 16 unique IDs and referenced test symbols passed; this is traceability, not mutation execution
- RepoSeiri 1.1 `research/repository` summary/routes/linter — source-session digest `sha256:098c58900eedaa36d353209d5e76d1e4af7f31004e72bfaffc0ecfe71cbd9f5e`; a repeated summary after routes/linter returned the same digest; writes false
- Assurance-Compiler doctor — `ok=true`; 22 bounded complete Rust units, each ≤16 KiB, all `needs_evidence`, zero diagnostics; see [EPIA2-27 review](epia2-27-adversarial-review-2026-10-02.md)

### A2-G00..14 state

| Gate | State | Reason |
| --- | --- | --- |
| G00 baseline/snapshot | Partial | Initial baseline and dirty-diff preservation are recorded. A release dossier pinning all current owner hunks/dependency digests is not complete. |
| G01 source/span/identity/budget | Pass (bounded profile) | Workspace tests and source/span/property regressions pass. No semantic-completeness claim for all Markdown/Rust/natural language. |
| G02 grammar matrix | Partial | Many parser positive/negative cases pass. The complete 3×3-per-declared-family plus cross-feature matrix is not complete. |
| G03 authority non-amplification | Partial | Host-scoped permit regressions pass. Production-wide enforcement across all projections/host invocations is not connected. |
| G04 configured worker/resource | Pass (local profile) | Pinned Stanza JA/EN offline local worker and framing/timeout/pipe regressions pass. OS-level network isolation is unobserved. |
| G05 artifact→IR→TL→closure→candidate | Partial | No-model/configured CLI emits one pipeline candidate. Full action evidence producers and semantic output realization are missing. |
| G06 evidence producers | Blocked | Production producers other than ProductionConnection (StructuralObservation, CompilerObserved, RuntimeValidation, AcceptanceOracle) are not connected. |
| G07 16 disconnects + repair | Partial | A 16-case manifest/reference map exists, but the original→cut→repair→same-gap-restore harness has not been executed for every mutation. |
| G08 lifecycle/replay/repair | Partial | Core lease/head/repair tests pass. External head persistence, live revocation refresh, production edit adapter, and hard cancellation are not connected. |
| G09 discrete↔continuous state | Partial | Verified-relation overlay changes kernel revision/next input. New independent evidence, global backflow, and GPU parity are not connected. |
| G10 standalone/host output | Blocked | Exact JSON candidate is captured, but ImplementationClosure is Hold. Host callback/sink/replay remain Pending. |
| G11 v1/v2 compatibility | Partial | Legacy fixtures, exact v1 checkpoint serialization, and v2 typed-phase schema pass. Real old-consumer negotiation/decoder compatibility is unobserved. |
| G12 Rust/package/privacy | Partial | No-default/default suites, configured temp-cwd run, and offline release build pass. Source distribution archive, remote CI, and model-rights review are incomplete. |
| G13 adversarial P0/P1/docs | Blocked | EPIA2-27 retains multiple production P0/P1s. RepoSeiri/Assurance are audit aids, not repairs. |
| G14 alpha.2 identity | Not run | Required G00..13 gates are not satisfied. Source version is not changed ahead of them. |

### Unmet conditions and next safe stage

1. Connect producer policies beyond ProductionConnection and bind each receipt to the same source/plan/target/lifecycle.
2. Connect a production acceptance path that preserves every open gap while demonstrating supported-action closure. Condition truth, semantic correctness, and output realization remain open without independent evidence.
3. Execute all 16 disconnect mutations and repairs, verifying that only the targeted gap opens/closes; add grammar-family positives/negatives and over-hold denominators.
4. Persist/load the trusted checkpoint head outside the ledger and connect it to CLI resume. Connect a production repair adapter to host permits, snapshots, and a process supervisor.
5. Connect exact candidates to host callback/sink-stage receipts and durable replay; keep real-host observation Pending until observed.

After these are repaired, reevaluate A2-G00..13; proceed to EPIA2-29 only if every required gate passes. Alpha.2 metadata promotion was not performed. No commit/push/merge/reinstall/restart was performed.
