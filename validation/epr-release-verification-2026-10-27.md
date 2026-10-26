# EPR source-release verification

## 日本語

### 状態と判定単位

**候補検証中。alpha.3昇格・最終source release readinessは未成立。** 基線は`75500ed7a33a009f5e1ce813862dc0a7986372c9`、候補branchは`codex/epr-alpha3-20261027`。現段階のsource identityは`6.3.2-alpha.2`であり、最終gate成功前にversionを変更しない。

この記録は、EPR-00〜19の既知境界欠陥と追加反例に対するsource/build/CLIの観測を扱う。研究仮説の成立、独立semantic gold、未知欠陥ゼロ、OS-wide sandbox、実host activationを認証しない。[個別台帳](epr-remediation-ledger.v1.json)の未完了項目を、[残存条件](epr-residual-risk-ledger.v1.json)へ移して完了にしない。

### 昇格前の観測

| 観測 | 状態・範囲 |
| --- | --- |
| 日本語・文法・予算 | 原文byte span、反復参照、協調節、共有後処理予算、下流BudgetRejected/Clarifyの対象試験成功 |
| 権限・host lineage | secret Debug、source/parent/issuer、半開期限、code/quote、cross-seed stage、公開flag変更の対象試験成功 |
| 実Coding経路 | 登録driver/oracle、実Cargo metadata、正規connection、assertion shadow負例、実修復・再開・fresh evidenceの対象試験成功 |
| 実Media経路 | current private snapshotからdecoder/OpenCV/非空ASR候補を観測、39.24秒。`device=cpu`、faster-whisper 1.2.1。精度・CUDA推論の実証ではない |
| 実日英backend | pinしたruntime/requirementsの物理LF byteを復元し、configured Stanzaの対象試験成功、82.65秒。一般自然言語精度ではない |
| 実GPU | RTX 3060/Vulkanで2,048 rows、empty/diagonal/mask/ceilingの限定parityを観測。native completion、summary/mask/finalist/decision digestの範囲。全vector・occupancy・speedupは未成立 |
| 固定commit配布 | `cfe29f89403f44ed1cd55552cf652c7c11cb3425`のsource-only packageを別の新規一時rootへ展開し、release buildと実CLI 5件成功。214 files、members digest `sha256:5c9295707a7a5c043bf977bfdc0d2c788fa84dc99ddcf46856562cc106fe130f`。最終identityの配布検証ではない |
| CI負例 | [PR #6](https://github.com/ViszCham/Epistesys/pull/6)でRust-only sentinelを意図的に変更し、Windows/Linuxとも該当testで失敗を観測。負例PRはunmergedでclosed。失敗を成功に読み替えていない |
| CI正例 | [PR #5](https://github.com/ViszCham/Epistesys/pull/5)で確認中。Windows producerのpath identity差を調査中であり、local passで代用しない |

以前の全体gateの失敗も保持する。初回は長いtest operationで有限receiptを使い回して期限切れとなり、test hostによるoperation別の新規発行へ修正した。cold CIのcompiler fingerprint期限はtest-onlyの正例予算を修正し、期限切れ負例は維持した。別の全体gateでは追加したFaceDetectionに対する古い16-family期待値が失敗し、17-familyの網羅・重複・method ID検査へ修正した。これらの修正後、全体gateの再実行が必要である。

### 必須gate

| Gate | 最終snapshot/identity |
| --- | --- |
| Windows format/check/no-default/default/strict clippy/release | Pending |
| Linux check/no-default/default/strict clippy/release | Pending |
| release parser/numeric、実GPU、実Media、configured JA/EN | 最終identityで再確認待ち |
| production capture/schema/legacy consumer/増幅拒否 | Pending |
| fixed-commit fresh packageと実CLI修復・再開 | 最終identityで再確認待ち |
| current remote CI正例 | Pending |
| チーム内敵対監査A/B、追加反例の消込 | 最終source pinで再確認待ち |
| RepoSeiri同一source-sessionと日英整合 | 最終export待ち |
| version/manifest/current docs一致 | 昇格前のため未変更 |

通常のignoredな実device/model試験をportable試験成功へ混ぜない。必要な実GPU/日英/Mediaは明示的に別実行する。Windows Jobのcommitted-memory制限とUnix workerのper-process address-space制限を、RSS上限や完全隔離へ拡張しない。

### 補助監査と再現

[Assurance記録](epr-assurance-observations-2026-10-27.json)はdoctor一回と、16KiB以下の完全な選択Rust単位を保持する。current reviewの4単位は`blocked_with_diagnostics`、1単位は`needs_evidence`、すべてevidenceは`missing_or_unknown`。既存の中間hashも履歴として保持する。診断ゼロを証明やmerge approvalにしない。

[監査A](epr-adversarial-audit-a-2026-10-27.md)と[監査B](epr-adversarial-audit-b-2026-10-27.md)はチーム内の別担当レビューであり、独立第三者評価ではない。反例修正を担当した後の確認は自己検証も含み、全体統合gateとは別である。

主な再実行入口は`scripts/validate-epr-gates.ps1 -ConfiguredNlp -Gpu`、`scripts/test-epr-real-media.ps1`、`scripts/validate-dgcl-schema.py --captures <owned-capture-root>`、`scripts/new-alpha2-source-package.ps1 -BuildAndTest`。後者の互換名は保持する。モデル・FFmpeg・Pythonは明示的に用意し、weight・秘密鍵・trust root・個人host pathをreleaseへ同梱しない。実行logのcommand/exit/duration/output digestとsource snapshotを照合して初めて、該当gateの観測として扱う。

## English

### Status and unit of judgment

**Candidate verification in progress; alpha.3 promotion/final source readiness are not established.** Baseline: `75500ed7a33a009f5e1ce813862dc0a7986372c9`; candidate branch: `codex/epr-alpha3-20261027`. The current source identity remains `6.3.2-alpha.2`; do not change it before required promotion gates pass.

This record covers source/build/CLI observations for known EPR-00–19 boundary findings and related counterexamples. It certifies neither research hypotheses, independent semantic gold, zero unknown defects, OS-wide sandboxing, nor real-host activation. Do not discharge unresolved [individual findings](epr-remediation-ledger.v1.json) by moving them into the [residual ledger](epr-residual-risk-ledger.v1.json).

### Pre-promotion observations

| Observation | State/scope |
| --- | --- |
| Japanese/grammar/budgets | Source byte spans, repeated references, coordinated clauses, shared postprocessing work, downstream BudgetRejected/Clarify targeted tests passed |
| Authority/host lineage | Secret Debug, source/parent/issuer, half-open expiry, code/quotes, cross-seed stages, public-flag mutation targeted tests passed |
| Actual Coding path | Registered driver/oracle, actual Cargo metadata, positive connections, assertion-shadow negative, real repair/resume/fresh-evidence targeted tests passed |
| Actual Media path | Decoder/OpenCV/nonempty ASR candidates observed through current private snapshots, 39.24 seconds; `device=cpu`, faster-whisper 1.2.1. Not accuracy or CUDA inference evidence |
| Actual Japanese/English backend | Restored physical pinned LF runtime/requirements bytes; configured-Stanza target passed in 82.65 seconds. Not general natural-language accuracy |
| Actual GPU | RTX 3060/Vulkan observations cover 2,048 rows plus empty/diagonal/mask/ceiling cases, native completion, summaries/masks/finalists/decision digests. Full-vector equality/occupancy/speedup remain unestablished |
| Fixed-commit distribution | Source-only commit `cfe29f89403f44ed1cd55552cf652c7c11cb3425` expanded into a new separate temporary root; release build and five real CLI tests passed. 214 files; members digest `sha256:5c9295707a7a5c043bf977bfdc0d2c788fa84dc99ddcf46856562cc106fe130f`. Not final-identity distribution validation |
| CI negative | [PR #6](https://github.com/ViszCham/Epistesys/pull/6) deliberately changed a Rust-only sentinel; Windows/Linux failed in that test. Closed unmerged. Intentional failure is not relabeled success |
| CI positive | In progress in [PR #5](https://github.com/ViszCham/Epistesys/pull/5). Windows producer path identity is under investigation; local passes do not substitute for it |

Retain earlier whole-gate failures. A long fixture reused finite receipts beyond expiry; the test host now issues fresh operation-specific material. Cold compiler fingerprinting required a test-only positive budget correction, preserving expiry negatives. Another gate found a stale 16-family expectation after FaceDetection was added; it now checks all 17 families, duplicates, and method IDs. Complete gates must rerun after these corrections.

### Required gates

| Gate | Final snapshot/identity |
| --- | --- |
| Windows format/check/no-default/default/strict clippy/release | Pending |
| Linux check/no-default/default/strict clippy/release | Pending |
| Release parser/numerics, actual GPU/Media, configured JA/EN | Final-identity recheck pending |
| Production capture/schema/legacy consumers/amplification rejection | Pending |
| Fixed-commit fresh package and real CLI repair/resume | Final-identity recheck pending |
| Current remote CI positive | Pending |
| Team audits A/B/additional counterexample discharge | Final-source pin recheck pending |
| Same-session RepoSeiri/bilingual consistency | Final export pending |
| Version/manifest/current-doc agreement | Unchanged before promotion |

Do not mix ignored device/model tests into portable-test success. Execute required GPU/Japanese/English/Media observations separately. Windows Job committed memory and Unix worker per-process address-space constraints are not RSS bounds or complete isolation.

### Advisory review and reproduction

The [Assurance record](epr-assurance-observations-2026-10-27.json) retains one doctor observation and complete selected Rust units no larger than 16 KiB. Four current units are `blocked_with_diagnostics`, one is `needs_evidence`, and every evidence field remains `missing_or_unknown`. Preserve intermediate hashes as history. Zero diagnostics are neither proof nor merge approval.

[Audit A](epr-adversarial-audit-a-2026-10-27.md) and [audit B](epr-adversarial-audit-b-2026-10-27.md) are different-contributor team reviews, not independent third-party evaluation. Follow-up after implementing a correction includes self-verification and cannot replace integration gates.

Main reproduction entry points are `scripts/validate-epr-gates.ps1 -ConfiguredNlp -Gpu`, `scripts/test-epr-real-media.ps1`, `scripts/validate-dgcl-schema.py --captures <owned-capture-root>`, and `scripts/new-alpha2-source-package.ps1 -BuildAndTest` (retained compatibility name). Provision models/FFmpeg/Python explicitly; release packages exclude weights, private keys, trust roots, and personal host paths. Match command/exit/duration/output digests to the source snapshot before counting an observation for its gate.
