# Epistesys prospective evaluation plan — draft, not preregistered

## 日本語

### 状態と目的

これは将来の評価案であり、実行済み試験、独立査読、事前登録ではない。Epistesys `6.3.2-alpha.2`の実装検証と、研究効果の実証を分離する。GB-CC75は既知の歴史的／開発用データとして扱い、未使用holdoutとは呼ばない。[五つの研究課題](../research-hypotheses.md)に対して別々の測定を設ける。

### 実行前に固定する項目

1. モデル要求と観測可能なprovider identity、controller commit、全prompt、tool policy、token/call/retry/wall/cost予算。等予算比較と予算増加比較を分ける。
2. 問題族単位の開発／holdout分離、使用履歴、難度・条件数・依存深度・session長・状態量の独立定義。文字数だけを複雑性としない。
3. 主指標・副指標・推定対象・実用上の最小差・非劣性条件・多重比較・停止規則。標本数は独立pilotまたは明記した仮定によるpower設計から決め、結果後に変更しない。
4. 対応taskでの経路順序の無作為化／均衡化、同一ユーザーseed、独立session、時刻・provider更新の記録。介入に属するsystem prompt差を隠さず記録する。
5. 全予定unitの取得・保留・拒否・未完・timeout・採点不能を記録する。正しいHoldと誤ったHoldをgoldで区別し、全試行運用指標と条件付き内容指標を併記する。
6. 経路ラベル・peer answerを伏せた別系統Judgeと独立人間裁定。判定順序を均衡化し、基準別一致・不一致・曖昧さ・採点不能を保存する。
7. 限定した機構別ablationと予算matched baseline。TL／DeepGrammar／world／output gateの寄与は、この操作が識別できる範囲だけ述べる。
8. 結果を見る前にprotocol・analysis code・hash・登録先を固定し、登録時刻を記録する。その時点までは確認的研究として表示しない。

### 研究課題ごとのendpoint

| 課題 | 必要な直接評価 | GB-CC75だけでは不足する点 |
| --- | --- | --- |
| Hallucination containment | 検証可能な事実とunsupported claim、誤commit、検出漏れ、coverage | ルーブリック平均はhallucination率ではない |
| Constraint preservation | 条件別・全条件達成、否定／例外／依存、意図の独立gold | 平均が高くてもstrict failureが残る |
| Long-horizon integrity | 初期指示、状態更新、authority／evidence失効、未解決obligationの保持 | 単発promptは長期driftを測らない |
| Failure observability | 正しいClarify/Hold、偽保留、silent failure、risk–coverage | 未完回答と正しい保留を同一視できない |
| Reliable complexity frontier | 課題族・予算・信頼度目標を固定した、複雑性／深度／状態量／長さの境界 | 75固定問題の平均でfrontier移動を示せない |

### 反証と公開

同等予算で差が再現しない、独立Judgeで差が消える、strict達成が悪化する、過剰保留でcoverageが落ちる、長期driftが増える場合を負の結果として公開する。複数指標を一つの未校正scoreへ混ぜて成功としない。改善が見られても課題族・モデル・予算の外へ一般化しない。

公開前に権利・privacy・秘密情報を検査し、公開可能なraw回答・基準別判定・実行manifest・集計scriptを分けて提供する。公開できない範囲はavailability statementへ記載する。gold不在を自己採点やAssurance doctorで補ったことにしない。RepoSeiriは文書・route整合性、Assuranceは対応Rust単位の補助監査に限定し、研究効果の独立評価には使わない。

---

## English

### Status and purpose

This is a future evaluation proposal, not an executed trial, independent peer review, or preregistration. Separate implementation checks for Epistesys `6.3.2-alpha.2` from empirical research effects. Treat GB-CC75 as known historical/development data, not unused holdout. Define separate measurements for the [five research questions](../research-hypotheses.md).

### Freeze before execution

1. Requested model and observable provider identity, controller commit, all prompts, tool policy, and token/call/retry/wall/cost budgets. Separate equal-budget and increased-budget comparisons.
2. Task-family development/holdout separation, usage history, independent definitions of difficulty, condition count, dependency depth, session length, and state volume. Length alone is not complexity.
3. Primary/secondary metrics, estimands, smallest practically meaningful difference, noninferiority conditions, multiplicity, and stopping rules. Determine sample size from an independent pilot or power design with explicit assumptions, not after results.
4. Randomized/balanced route order on paired tasks, identical user seeds, independent sessions, timestamps/provider changes. Disclose system-prompt differences belonging to the intervention.
5. Record acquisition, holds, refusals, non-delivery, timeouts, and unscoreable outcomes for every planned unit. Use gold to distinguish correct and incorrect holds; report all-attempt operational metrics alongside conditional content metrics.
6. Other-family Judges and independent human adjudication blinded to route labels/peer answers. Balance grading order and preserve per-criterion agreements, disagreements, ambiguities, and unscoreable cases.
7. Bounded mechanism ablations and budget-matched baselines. Attribute TL/DeepGrammar/world/output-gate contributions only within what those interventions identify.
8. Freeze protocol, analysis code, hashes, registration venue, and registration time before observing results. Until then, do not present the study as confirmatory.

### Endpoints by research question

| Question | Required direct evaluation | Why GB-CC75 alone is insufficient |
| --- | --- | --- |
| Hallucination containment | Verifiable facts, unsupported claims, erroneous commitment, missed detection, coverage | Mean rubric adherence is not hallucination rate |
| Constraint preservation | Per/all-criteria achievement, negation/exceptions/dependencies, independent intent gold | High means can coexist with strict failure |
| Long-horizon integrity | Initial instructions, state updates, authority/evidence expiry, unresolved obligations | Single prompts do not measure long-horizon drift |
| Failure observability | Correct Clarify/Hold, false holds, silent failures, risk–coverage | Non-delivery and correct withholding are different |
| Reliable complexity frontier | Boundaries over complexity/depth/state/length with fixed task families, budgets, reliability targets | A fixed 75-task mean cannot establish frontier movement |

### Falsification and publication

Publish negative outcomes if differences fail to replicate at equal budget, vanish under independent Judges, reduce strict achievement, lower coverage through excess holds, or increase long-session drift. Do not combine heterogeneous metrics into one uncalibrated success score. Even positive results do not automatically generalize beyond tested task families, models, or budgets.

Before disclosure, inspect rights, privacy, and secrets; provide separable publishable raw answers, criterion verdicts, execution manifests, and aggregation scripts. State unavailable ranges explicitly. Self-grading or Assurance doctor cannot substitute for missing gold. Limit RepoSeiri to document/route consistency and Assurance to supported bounded Rust review, not independent evaluation of research effects.
