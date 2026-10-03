# GB-CC75 adversarial methodological audit

## 日本語

### 監査の対象と順序

2026-10-03の文書改訂前に、Epistesysの基準commit `d74bdf020334d4efc2ea62267296c6a61fd046c1`、前身システムの公開集計、欠測・採点監査、ローカルのJudge manifestを点検した。本書は内部の敵対的方法論レビューであり、独立査読、統計的認証、論文採択、性能保証ではない。READMEの訴求を強める前に、何を観測し、何を推定できず、何を次に検証するかを分離した。

### 所見と採用した修正

| ID | 敵対的な問い／問題 | 今回の文書上の対処 | 残る検証 |
| --- | --- | --- | --- |
| A01 | 75問を評価したのに主要解析は65問ではないか | 計画75、対応適格67、主要65と225→171→167ペアを明記 | 取得選択を含む予定全単位の評価 |
| A02 | 94.10%は問題の正答率か | QID等重みの平均ルーブリック適合度と呼び、全基準達成48.50%を併記 | 独立基準による構成概念妥当性 |
| A03 | 反復・基準を独立標本として水増ししていないか | QID内平均とQIDクラスタを保持。167ペアと3,521基準インスタンスは独立問題数ではない | 問題族内の依存と未知母集団への一般化 |
| A04 | 信頼区間や小さいp値が系統誤差を覆っていないか | 固定問題集合と再標本化仮定に条件付け、Judge誤差・取得選択は区間外と記載 | 独立評価、仮定検査、別問題集合での再実験 |
| A05 | Labyrinthを含むJudgeがLabyrinth経路を有利にしていないか | manifestのラベル盲検設計を記録するが、機構上の依存は解消済みとしない | 別系統Judgeと独立人間裁定、盲検の実行確認 |
| A06 | 56/56一致は採点の正確さを証明するか | 事後選択・同一モデル系の判定一致と記載し、gold／採点精度へ昇格しない | 独立goldと代表性のある評価標本 |
| A07 | 改善はTLではなく追加プロンプト・予算の効果ではないか | 経路全体の観測差と機構の因果効果を分離 | 同等予算、順序制御、機構別ablation |
| A08 | Judgeの6.2.4を生成側の版と取り違えていないか | 生成側の正確な版・設定を未確定と表示。モデル指定とprovider実状態も分離 | 生成manifest・request・controller revisionの追加公開監査 |
| A09 | 負けた問題を平均で隠していないか | 32勝20同点13敗、strict pass、取得／採点失敗を同じ結果節へ配置 | 負の差の再現性と独立した失敗分類 |
| A10 | taxonomyやsubgroupは事前仮説だったのか | 探索的・事後的・重複可能と明示。未同梱の群ラベルで新しい推論をしない | ラベルを固定した未使用holdout |
| A11 | 前身の結果をalpha.2や長期sessionの効果に移転していないか | 前身の経験的観測、alpha.2の実装検証、将来の研究仮説を別区分にする | alpha.2固有、hallucination、長期interactionの直接評価 |
| A12 | hash一致を第三者の再現実験と呼んでいないか | 集計再計算、回答再採点、再実験を区別。公開範囲は選択済み集計のみ | raw回答・基準・実行設定の権利／privacy監査と追加公開 |

### 結論とclaim ladder

現在支持される結論は、この歴史的実行の採用条件と独自Judgeの下で、Labyrinth経路の主要・副次平均適合度がDirect経路より高かったことである。差の大きさを隠す必要はないが、全問題の完全遵守、一般能力、因果機序、hallucination削減、長期保持、Epistesys alpha.2の改善へ拡張しない。

1. 実装経路の存在・実行検査：現在のsource／CLI検証へ結び付ける。
2. 歴史的な採点結果：GB-CC75の凍結された集計へ結び付ける。
3. 独立した効果再現：未実施。
4. 特定機構の因果的寄与：未識別。
5. reliable operating envelopeの拡張：今後の研究仮説。

本改訂はA01〜A12の説明上の混同を修正する。実験上の限界を「解決済み」とはしない。[研究本文](gb-cc75-study.md)、[方法と由来](gb-cc75-methods-and-provenance.md)、[統計仕様](gb-cc75-statistical-analysis.md)、[将来の評価案](epistesys-prospective-evaluation-plan.md)へ接続する。

---

## English

### Scope and ordering

Before the documentation revision on 2026-10-03, the audit inspected Epistesys baseline commit `d74bdf020334d4efc2ea62267296c6a61fd046c1`, published predecessor aggregates, missingness/grading audits, and local Judge manifests. This is an internal adversarial methodological review, not independent peer review, statistical certification, publication acceptance, or a performance guarantee. Observations, unidentified quantities, and future tests were separated before strengthening README presentation.

### Findings and adopted corrections

| ID | Adversarial question / issue | Documentation treatment in this revision | Remaining evaluation |
| --- | --- | --- | --- |
| A01 | Why does a 75-task benchmark have a 65-task primary analysis? | State planned 75, paired-eligible 67, primary 65, and 225→171→167 pairs | Evaluation of all planned units including acquisition selection |
| A02 | Is 94.10% task accuracy? | Name it equal-QID-weighted mean rubric adherence; report all-criteria pass of 48.50% alongside it | Construct validity against independent criteria |
| A03 | Are repetitions/criteria inflated into independent samples? | Preserve within-QID means and QID clusters; 167 pairs and 3,521 criterion instances are not independent task counts | Task-family dependence and generalization to an unknown population |
| A04 | Do intervals or small p-values conceal systematic error? | Condition on the fixed tasks/resampling assumptions; exclude Judge error and acquisition selection from interval coverage | Independent evaluation, assumption checks, replication on other tasks |
| A05 | Does a Labyrinth-based Judge favor the Labyrinth route? | Record manifest-level label blinding without claiming that mechanism dependence is resolved | Other-family Judges, independent human adjudication, verified execution of blinding |
| A06 | Does 56/56 agreement establish grading accuracy? | Describe post-result, same-model-family agreement; do not promote it to gold or grading accuracy | Independent gold and a representative evaluation sample |
| A07 | Is the improvement due to extra prompts/budget rather than TL? | Separate observed whole-route differences from causal mechanism effects | Matched budgets, order control, mechanism ablations |
| A08 | Is the Judge's 6.2.4 identity confused with the generator version? | Mark exact generator version/settings unresolved; separate requested models from provider reality | Further disclosure audit of generator manifests/requests/controller revisions |
| A09 | Are losses concealed by the average? | Report 32 wins, 20 ties, 13 losses, strict pass, and acquisition/grading failures together | Replication of negative differences and independent failure labeling |
| A10 | Were taxonomies/subgroups specified before results? | Mark exploratory, post hoc, overlapping labels; do not infer new subgroup results from unpublished labels | Unused holdouts with frozen labels |
| A11 | Are predecessor results transferred to alpha.2 or long sessions? | Separate predecessor observations, alpha.2 implementation checks, and future hypotheses | Direct alpha.2, hallucination, and long-horizon evaluation |
| A12 | Is hash agreement called independent experimental replication? | Separate aggregate recomputation, answer regrading, and rerunning; disclose only selected aggregates | Rights/privacy review and further disclosure of raw answers/criteria/settings |

### Conclusion and claim ladder

The supported conclusion is that, under this historical run's eligibility rules and custom Judge, the Labyrinth route had higher primary and secondary mean adherence than the Direct route. The magnitude need not be hidden, but cannot be extended to complete compliance across tasks, general capability, causal mechanisms, hallucination reduction, long-session retention, or improvement in Epistesys alpha.2.

1. Implementation-path existence/execution: bind to current source/CLI checks.
2. Historical grading results: bind to frozen GB-CC75 aggregates.
3. Independently replicated effects: not performed.
4. Causal contributions of particular mechanisms: not identified.
5. Expansion of the reliable operating envelope: a future research hypothesis.

This revision corrects explanatory conflations A01–A12; it does not resolve experimental limitations. Follow the [study](gb-cc75-study.md), [methods/provenance](gb-cc75-methods-and-provenance.md), [statistical specification](gb-cc75-statistical-analysis.md), and [prospective plan](epistesys-prospective-evaluation-plan.md).
