# Epistesys research goals, design objectives, and hypotheses

## 日本語

Epistesysは、AIが定義された信頼性を維持して扱える指示複雑性・状態量・session長の範囲を明らかにし、その拡張可能性を検証する長期研究プロジェクトです。現在の6.3.2-alpha.1はv6.3.1由来のbaseline implementationであり、プロジェクトの最終到達点ではありません。

### 用語と証拠の段階

| 区分 | 意味 |
| --- | --- |
| 研究目標 | reliable operating envelope / reliable complexity frontierを特徴付け、拡張できるか明らかにする |
| 設計目標 | 制約・権限・状態・証拠の明示保持、defectの可観測性、commit前の再検証を実装する |
| 設計仮説 | 特定の機構が期待する効果を生むという、機構と結果の対応についての予想 |
| 研究仮説 | 同じモデル・課題・総予算で、誤ったcommit・制約脱落等が減り、目標信頼度を満たす範囲が広がるという検証対象 |
| 現在の観測 | CLI、schema、fixture、test等の実行・検査。研究効果の独立実証とは別 |

### 五つの研究課題

| 研究課題 | 設計手段・仮説 | 今後の測定 |
| --- | --- | --- |
| Hallucination containment | evidence gateとcandidate/Validated分離が、裏付け不足のcommitを減らすか | unsupported claimと誤commit率、検出漏れ、回答coverage |
| Constraint preservation | obligation・exception・dependencyの明示保持と再検証が脱落を減らすか | 条件別充足率、全条件同時充足率、意図保持、回帰 |
| Long-horizon instruction integrity | revision・authority・未解決obligation・decision/evidence lineageの追跡がdriftを抑えるか | session長別の初期指示保持、状態整合性、古い権限や証拠の誤使用 |
| Failure observability | defectをClarify/Holdとして露出する設計がsilent failureを減らすか | 適切な保留、偽保留、検出精度、risk–coverage |
| Reliable complexity frontier | 上記の制御を組み合わせて目標信頼度を満たす範囲を広げられるか | 独立に定義した難度・依存深度・状態量・session長と総予算ごとの信頼性 |

長期整合性は記憶容量だけの問題ではなく、指示の有効範囲、状態更新、未解決事項、証拠の由来を再検証する問題として扱います。指示の長さと条件数を同じ難度とみなさず、別々に操作します。

### 評価方針と現在の状態

全仮説はbenchmark pending / independent evaluation pendingです。現alphaはEpistesys固有のreasoning improvement、hallucination reduction、複雑指示への優位性、長期保持の改善を実証していません。build/test/CLI成功は一般性能、SOTA、formal proof、hallucination prevention、long-session robustnessを保証しません。

今後の比較ではモデル、task、system instruction、tool、出力上限、retry、controllerを含む総計算予算を明示します。独立採点、反復、順序の均衡化、欠測・失敗・保留の記録を用い、平均得点だけでなく全条件達成とrisk–coverageを調べます。実装機構が存在することと、その研究効果が再現することを別の証拠として報告します。Epistesys-7の計画を現alphaの実装成果に含めません。

[README](../README.md) / [実装検証](clone-verification.md) / [既知の制約](known-limitations.md)

---

## English

Epistesys is a long-term research project characterizing the instruction complexity, state volume, and session length over which AI can maintain defined reliability, and testing whether that range can expand. Version 6.3.2-alpha.1 is a baseline implementation derived from v6.3.1, not the project's endpoint.

### Terms and evidence levels

| Category | Meaning |
| --- | --- |
| Research goal | Characterize the reliable operating envelope / reliable complexity frontier and investigate its expansion |
| Design objective | Implement explicit constraint/authority/state/evidence retention, observable defects, and revalidation before commitment |
| Design hypothesis | A proposed mechanism-to-outcome relationship explaining how a mechanism could produce an expected effect |
| Research hypothesis | A testable prediction of fewer erroneous commitments or dropped constraints and wider target-reliability coverage under the same model, tasks, and total budget |
| Current observation | Execution/inspection of CLI, schema, fixture, and test surfaces, separate from independent demonstration of research effects |

### Five research questions

| Research question | Design mechanism / hypothesis | Planned measurement |
| --- | --- | --- |
| Hallucination containment | Can evidence gates and candidate/Validated separation reduce unsupported commitments? | Unsupported claims, erroneous commitment rate, missed detections, response coverage |
| Constraint preservation | Can explicit obligation/exception/dependency retention and revalidation reduce omissions? | Per-criterion satisfaction, all-criteria satisfaction, intent preservation, regressions |
| Long-horizon instruction integrity | Can revision, authority, unresolved-obligation, and decision/evidence-lineage tracking reduce drift? | Initial-instruction retention, state consistency, misuse of stale authority/evidence by session length |
| Failure observability | Can exposing defects as Clarify/Hold reduce silent failures? | Appropriate holds, false holds, detection precision, risk–coverage |
| Reliable complexity frontier | Can combined control mechanisms expand coverage at a target reliability? | Reliability by independently defined difficulty, dependency depth, state volume, session length, and total budget |

Long-horizon integrity concerns revalidation of instruction scope, state updates, unresolved items, and evidence provenance as well as memory capacity. Instruction length and condition count are manipulated separately rather than treated as equivalent difficulty.

### Evaluation policy and current status

All hypotheses remain benchmark pending / independent evaluation pending. The current alpha does not demonstrate Epistesys-specific reasoning improvement, hallucination reduction, complex-instruction superiority, or improved long-session retention. Build/test/CLI success does not guarantee general performance, SOTA, formal proof, hallucination prevention, or long-session robustness.

Future comparisons will specify model, tasks, system instructions, tools, output budgets, retries, and total compute including the controller. Independent grading, repetitions, balanced ordering, and records of missingness/failures/holds will support evaluation of all-criteria achievement and risk–coverage alongside mean scores. Mechanism existence and reproducible research effects will be reported as separate evidence. Epistesys-7 plans are not current-alpha implementation achievements.

[README](../README.md) / [Implementation verification](clone-verification.md) / [Known limitations](known-limitations.md)
