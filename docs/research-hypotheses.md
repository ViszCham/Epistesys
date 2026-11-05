# Epistesys：設計仮説と評価項目 / Design hypotheses and evaluation criteria

## 日本語

このページは、開発で必要になった仕組みが、期待した場面で本当に役立つかを確かめるための評価案です。たとえば、要求とテストを結び付ける機構を実装したことと、それを使うと実務で要求の見落としが減ることは、別々に確かめます。現在のソースは[Beta 6.9の保存版](epistesys-6.9.0-beta.1-source-snapshot-2026-11-05.md)です。以下の比較は、この版の性能を実証した結果ではありません。

### 用語と証拠の段階

測定そのものを確認したい場合は、[EpiMapLossの四軸](epimaploss-epirecompscore-formulas.md#map-ja)と[EpiRecompScoreのF1・F2](epimaploss-epirecompscore-formulas.md#score-ja)からお読みください。このページで扱うのは、その式を使うと開発結果が変わるかをどう評価するかです。

| 区分 | 意味 |
| --- | --- |
| 利用範囲の問い | どれだけ複雑な依頼や長い対話まで、あらかじめ決めた信頼性の基準を満たすか（reliable operating envelope / reliable complexity frontier） |
| 設計目標 | 制約・権限・状態・証拠の明示保持、defectの可観測性、commit前の再検証を実装する |
| 設計仮説 | 特定の機構が期待する効果を生むという、機構と結果の対応についての予想 |
| 比較評価で検証する仮説 | 同じモデル・課題・総予算で、誤った確定や制約の脱落が減り、目標の信頼性を満たす範囲が広がるか |
| 実装の検証 | 対象の版と条件を明示したCLI、schema、fixture、testの実行・検査。開発上の効果についての独立評価とは別 |
| 前身の観測 | GB-CC75の過去の探索的集計。Epistesysでの改善や機構の因果的な効果へ読み替えない |

### 開発で確かめたい五つのこと

| 開発上の関心 | 設計手段・仮説 | 今後の測定 |
| --- | --- | --- |
| Hallucination containment | evidence gateとcandidate/Validated分離が、裏付け不足のcommitを減らすか | unsupported claimと誤commit率、検出漏れ、回答coverage |
| Constraint preservation | obligation・exception・dependencyの明示保持と再検証が脱落を減らすか | 条件別充足率、全条件同時充足率、意図保持、回帰 |
| Long-horizon instruction integrity | revision・authority・未解決obligation・decision/evidence lineageの追跡がdriftを抑えるか | session長別の初期指示保持、状態整合性、古い権限や証拠の誤使用 |
| Failure observability | defectをClarify/Holdとして露出する設計がsilent failureを減らすか | 適切な保留、偽保留、検出精度、risk–coverage |
| Reliable complexity frontier | 上記の制御を組み合わせて目標信頼度を満たす範囲を広げられるか | 独立に定義した難度・依存深度・状態量・session長と総予算ごとの信頼性 |

長期対話では、記憶容量だけでなく、指示が今も有効か、何が未解決か、以前の検証結果を今のコードに使えるかが問題になります。比較する際には、指示の長さと条件数を別々に変えます。

### 前身の観測が支持する範囲

[GB-CC75](research/gb-cc75-study.md)は、前身システムで指示の制約がどれだけ保たれたかを調べた探索的な評価です。主要65 QIDで平均74.38%→94.10%（+19.71ポイント）を観測しましたが、167ペアの全基準達成は39.52%→48.50%で、13 QIDでは負の差でした。独自Judge、取得／採点選択、未確定予算、版の違いに由来する限界があります。hallucination、長期保持、authority安全性、frontier拡張は直接測定していません。

[敵対監査](research/gb-cc75-adversarial-audit.md)と[前向き評価案](research/epistesys-prospective-evaluation-plan.md)を接続し、設計仮説、前身の経験的結果、Epistesys固有の検証を混同しません。既知の75問を新しいholdoutとして再利用しません。

### 評価方針と現在の状態

Epistesys自体の比較ベンチマークと独立評価は未実施です。推論の改善、ハルシネーションの減少、複雑な指示や長い対話での優位性を示す結果はまだありません。build・test・CLIの成功だけでは、一般性能、SOTA、形式証明、ハルシネーションの防止、長期対話の頑健性は保証できません。

今後比較する場合は、モデル、課題、システム指示、ツール、出力上限、再試行、制御処理を含む総計算予算を明示します。独立採点、反復、順序の均衡化、欠測・失敗・保留の記録を用い、平均得点だけでなく全条件達成とrisk–coverageを調べます。機構が実装されていることと、その効果が再現することを別々に報告します。後続版の計画は現在の実装成果に含めません。

[README](../README.md) / [実装検証](clone-verification.md) / [既知の制約](known-limitations.md)

---

## English

This page lays out how to check whether mechanisms built for a development need actually help in that setting. For example, implementing a link between a requirement and a test is one question; whether using it leads developers to miss fewer requirements is another. The current source is the [Beta 6.9 checkpoint](epistesys-6.9.0-beta.1-source-snapshot-2026-11-05.md). The proposed comparisons below are not performance results for that version.

### Terms and evidence levels

For the measurements themselves, start with [the four EpiMapLoss axes](epimaploss-epirecompscore-formulas.md#map-en) and [EpiRecompScore F1 and F2](epimaploss-epirecompscore-formulas.md#score-en). This page asks how to evaluate whether using those formulas changes development outcomes.

| Category | Meaning |
| --- | --- |
| Operating-range question | How complex can a request become, or how long can a session run, while still meeting a stated reliability target (the reliable operating envelope / reliable complexity frontier)? |
| Design objective | Implement explicit constraint/authority/state/evidence retention, observable defects, and revalidation before commitment |
| Design hypothesis | A proposed mechanism-to-outcome relationship explaining how a mechanism could produce an expected effect |
| Hypothesis for comparative evaluation | Whether erroneous commitments or dropped constraints decrease and the range meeting the reliability target expands under the same model, tasks, and total budget |
| Implementation verification | CLI, schema, fixture, and test checks tied to a revision and its conditions, separate from independent evaluation of effects on development work |
| Predecessor observation | Historical exploratory GB-CC75 aggregates; not evidence of improvement in Epistesys or a causal effect of a mechanism |

### Five things to check in development

| Development concern | Design mechanism / hypothesis | Planned measurement |
| --- | --- | --- |
| Hallucination containment | Can evidence gates and candidate/Validated separation reduce unsupported commitments? | Unsupported claims, erroneous commitment rate, missed detections, response coverage |
| Constraint preservation | Can explicit obligation/exception/dependency retention and revalidation reduce omissions? | Per-criterion satisfaction, all-criteria satisfaction, intent preservation, regressions |
| Long-horizon instruction integrity | Can revision, authority, unresolved-obligation, and decision/evidence-lineage tracking reduce drift? | Initial-instruction retention, state consistency, misuse of stale authority/evidence by session length |
| Failure observability | Can exposing defects as Clarify/Hold reduce silent failures? | Appropriate holds, false holds, detection precision, risk–coverage |
| Reliable complexity frontier | Can combined control mechanisms expand coverage at a target reliability? | Reliability by independently defined difficulty, dependency depth, state volume, session length, and total budget |

In long sessions, memory capacity is only part of the problem. Are earlier instructions still in force? What remains unresolved? Do earlier test results still apply to the current code? A future comparison should vary instruction length and condition count separately.

### What predecessor observations support

[GB-CC75](research/gb-cc75-study.md) is an exploratory evaluation of instruction-constraint preservation in the predecessor system. Primary mean increased from 74.38% to 94.10% (+19.71 points) across 65 QIDs, but all-criteria pass across 167 pairs changed from 39.52% to 48.50%, with negative differences on 13 QIDs. The custom Judge, acquisition/scoring selection, unresolved budgets, and version differences limit what can be inferred. Hallucination, long retention, authority safety, and frontier expansion were not directly measured.

Connect the [adversarial audit](research/gb-cc75-adversarial-audit.md) and [prospective plan](research/epistesys-prospective-evaluation-plan.md) without conflating design hypotheses, predecessor empirical results, and Epistesys-specific evaluation. Do not reuse the known 75 tasks as a new holdout.

### Evaluation policy and current status

Comparative benchmarking and independent evaluation of Epistesys itself are pending. There is no result establishing improved reasoning, reduced hallucination, or better handling of complex instructions or long sessions. Build, test, and CLI success alone do not guarantee general performance, SOTA, formal proof, hallucination prevention, or long-session robustness.

Any future comparison should specify the model, tasks, system instructions, tools, output budgets, retries, and total compute including the controller. Independent grading, repetitions, balanced ordering, and records of missingness, failures, and holds will support evaluation of all-criteria achievement and risk–coverage alongside mean scores. Report implementation and reproducible effects separately. Plans for later versions are not achievements of the current implementation.

[README](../README.md) / [Implementation verification](clone-verification.md) / [Known limitations](known-limitations.md)
