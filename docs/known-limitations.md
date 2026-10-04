# Epistesys verification limits and residual conditions

## 日本語

この文書は`6.3.2-alpha.2`の未観測・未実証範囲を、次に必要な確認と対応付けます。実装経路の存在、宣言profileの局所検証、研究効果、配布、実host観測は別の状態です。[用語と境界](system-architecture.md)と[最終検証](../validation/epia2-alpha2-release-verification-2026-10-02.md)を参照してください。

### 残存条件

| 領域 | 現在の境界 | 必要な確認 |
| --- | --- | --- |
| 研究効果 | hallucination containment、制約保持、長期整合性、risk–coverage、運用範囲の拡張は未実証 | 独立gold、未使用holdout、同等予算、反復・欠測・劣化を含む評価 |
| 構文・意味 | controlled grammar外の完全理解は対象外。exact source roundtripは意味的等価性ではない | 対応文法別の独立評価、configured parser/modelの実観測 |
| 権限・証拠 | 外部receiptとcaller/host trust rootに依存。local署名は実hostや人間の独立性を証明しない | issuer/key provenance、失効・scope・snapshotの確認 |
| Coding・build | 宣言profileの検証・修復・再開を実装。任意build world、opaque build script/proc macro、OS sandboxは未保証 | dependency/build条件と実行環境の個別検証 |
| 再開・永続化 | journalと別管理headの不一致を検出。joint rollback、同一administrator改変、全OSのdirectory fsyncは範囲外 | 運用上のhead分離、key保護、OS別durability試験 |
| GPU | local materializationと数値経路を実装。parity、readback、timing、overlap、occupancy、device-lossは別観測 | 対象backend・device・workload別の測定 |
| Media・外部解析 | source/backend/model/license/method/budgetの条件付き候補・観測 | parser、vision、ASR等の登録・license・receipt・実backend試験 |
| 出力・host | candidate、send、sink、replayのadapterを実装。実callback／deliveryはPendingHostObservation | fresh host pickup、stage receipt、実sink、revocationの観測 |
| 配布 | source-only。sourceから生成したbinaryと配布監査済みbinaryは異なる | absolute build path監査、別cwd起動、clean-machine条件、配布検証 |
| 補助監査 | local tests・hash・Assurance statusは限定範囲の材料 | 対象revision、未監査単位、独立runtime観測の確認 |

### 前身の評価を現alphaへ移転しない

[GB-CC75](research/gb-cc75-study.md)は前身の探索的観測です。主要65 QID／採点済み167対応ペアへの選択、機構を含むJudge、未確定の生成設定・予算、未同梱raw回答という制約があります。公開CSVの再計算は独立再採点や再実験ではありません。[方法論監査](research/gb-cc75-adversarial-audit.md)の実験課題を文書変更だけで解決したとはしません。

初回alpha.1はv6.3.1のcloneでした。alpha.2はDGCLのcontrolled grammar→Program IR→TL→要求別completion、実Cargo観測、署名evidence、実ファイル修復、journal/head付き再開、構造蒸留の下流接続、consumer negotiationを追加しています。検証範囲は[DGCL実行契約](dgcl-operating-profile-and-closure.md)に限定します。

### 状態・数値の解釈

- `Clarify`、parse defect、Program IR／host output未bindingは、入力や証拠の未解決状態です。
- world doctorの2,048 evaluationはlocal materializationであり、2,048 LLM呼出しではありません。
- schema適合は証拠の真正性を、GPU利用率はoccupancyを、source hashは意味的正確性を表しません。
- commit、push、tag、plugin install、host reloadは、解析・検証結果から自動的には許可されません。

ResearchEvaluation=`PendingNoCorpus`とhost観測=`PendingHostObservation`を維持します。今後はsource/build条件、schema互換性、receipt/replay分離、独立評価をそれぞれ再確認し、Epistesys-7の計画と現alphaの成果を分離します。

## English

This document maps unobserved/unproven scope in `6.3.2-alpha.2` to required checks. Implementation presence, declared-profile local verification, research effects, distribution, and real-host observation are distinct states. See [terms/boundaries](system-architecture.md) and [final verification](../validation/epia2-alpha2-release-verification-2026-10-02.md).

### Residual conditions

| Area | Current boundary | Required checks |
| --- | --- | --- |
| Research effects | Hallucination containment, constraint preservation, long-horizon integrity, risk–coverage, and operating-range expansion are unproven | Independent gold, unused holdouts, matched budgets, repetitions/missingness/regressions |
| Syntax/semantics | Complete interpretation outside controlled grammar is excluded; exact roundtrip is not semantic equivalence | Independent grammar-specific evaluation and observed configured parsers/models |
| Authority/evidence | Depend on external receipts/caller-host trust roots; local signatures do not prove real-host or human independence | Issuer/key provenance, revocation, scope, snapshot verification |
| Coding/build | Implement declared-profile validation/repair/resume, not arbitrary build worlds, opaque scripts/proc macros, or OS sandbox guarantees | Individual dependency/build/environment checks |
| Resume/persistence | Detect journal/separate-head mismatch; joint rollback, same-administrator mutation, and universal directory fsync are excluded | Operational head separation, key custody, OS-specific durability tests |
| GPU | Implement local materialization/numeric paths; parity/readback/timing/overlap/occupancy/device-loss are separate observations | Backend/device/workload-specific measurement |
| Media/external analysis | Conditional source/backend/model/license/method/budget-bound candidates/observations | Parser/vision/ASR registration, licensing, receipts, actual backend tests |
| Output/host | Implement candidate/send/sink/replay adapters; real callbacks/delivery remain PendingHostObservation | Fresh pickup, stage receipts, real sinks, revocation observations |
| Distribution | Source-only; locally generated and distribution-audited binaries differ | Absolute build-path audits, other-cwd startup, clean-machine conditions, distribution checks |
| Advisory audits | Local tests/hashes/Assurance statuses are bounded material | Revision, unreviewed units, independent runtime observations |

### No transfer of predecessor evaluation to current alpha

[GB-CC75](research/gb-cc75-study.md) is exploratory predecessor evidence. Limitations include selection to 65 primary QIDs/167 scored pairs, mechanism-containing Judges, unresolved generator settings/budgets, and excluded raw answers. Public-CSV recomputation is neither independent regrading nor experimental replication. Documentation alone does not resolve experiments in the [methodological audit](research/gb-cc75-adversarial-audit.md).

Initial alpha.1 was a v6.3.1 clone. Alpha.2 adds controlled grammar→Program IR→TL→per-requirement completion, real Cargo observations, signed evidence, file repair, journal/head-backed resume, downstream structural distillation, and consumer negotiation. Verification remains scoped by the [DGCL contract](dgcl-operating-profile-and-closure.md).

### Interpreting states and quantities

- `Clarify`, parse defects, and unbound Program IR/host output denote unresolved input/evidence.
- World-doctor 2,048 evaluations are local materialization, not 2,048 LLM calls.
- Schema validity is not evidence authenticity; GPU utilization is not occupancy; source hashes are not semantic accuracy.
- Parsing/verification results do not automatically authorize commit/push/tag/plugin install/host reload.

Retain ResearchEvaluation=`PendingNoCorpus` and host observation=`PendingHostObservation`. Recheck source/build conditions, schemas, receipt/replay separation, and independent evaluation separately; distinguish Epistesys-7 plans from current-alpha results.
