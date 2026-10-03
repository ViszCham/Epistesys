# GB-CC75 exploratory failure analysis

## 日本語

### 平均と完全達成の乖離

主要平均94.10%に対し、Labyrinth経路の全基準達成は81/167＝48.50%である。小さい基準漏れでもstrictは失敗するため、平均の改善を完成の保証と混同しない。210基準インスタンスが未達であり、32勝・20同点・13敗のQID分布は改善の非一様性を示す。[pair CSV](../../benchmarks/gb-cc75/2026-08-18/F3-paired-secondary.csv)と[QID CSV](../../benchmarks/gb-cc75/2026-08-18/F3-qid-primary.csv)から確認できる。

### 構造的未完了と回帰

記録にはDirect≤25%かつLabyrinth≥75%の25ペア・18 QIDがあり、そのうち21ペアは500文字以下の処理予告／meta preamble型の未完回答として歴史的に分類された。7ペアはDirect scoreが0%、30ペアは差が50ポイント以上、同閾値の逆向き事例は0だった。この分類は後ろ向きの内容分析であり、公開bundleのanswer hashだけから本文分類を独立確認できない。

Labyrinth側にも34/167ペア、13/65 QIDの負の差がある。負のQID一覧を隠さずCSVから再計算する。追加説明、形式逸脱、否定制約の漏出、部分欠落、解釈競合は改善対象の候補であり、独立した因果診断ではない。

### taxonomyと未同梱subgroupの扱い

歴史的な詳細報告はoutput contamination、exact format、primary/alternative interpretation、negative-constraint leak、partial omissionという重複可能な事後分類を用いた。またImplicit群のstrict passはDirect 19.23%、Labyrinth 15.38%、差-3.85ポイントと報告された。この群ラベル・全分類入力は本bundleに含まないため、同値を新しい検証済みsubgroup結果として再計算・宣伝しない。平均改善が全指示種別のstrict改善を含意しないことの、由来を限定した歴史的報告として扱う。

次の評価ではラベル定義、複数ラベル、裁定方法、annotatorの独立性、一致度、未分類状態を事前に固定する。失敗rationaleもJudge生成であるため、別の独立evidenceではない。事後taxonomyを確認的検定やTL機序の証明へ昇格しない。

### 運用上のfailureとの分離

内容scoreと取得／採点のfailureは別の層である。採点済み回答だけで「崩壊なし」と主張しない。Labyrinthの取得非適格30/225、Directの40/225、Judgeの10未採点packetを別に保持する。非適格の根本原因や総費用は公開集計から識別できない。正しいHoldと誤った拒否／未完回答も別ラベルで評価する必要がある。

[方法](gb-cc75-methods-and-provenance.md)／[敵対監査](gb-cc75-adversarial-audit.md)／[前向き評価案](epistesys-prospective-evaluation-plan.md)。

---

## English

### Mean adherence versus complete achievement

Against a primary mean of 94.10%, the Labyrinth route passed all criteria on 81/167=48.50% of scored pairs. Even small omissions fail strict pass; mean improvement is not guaranteed completion. There were 210 failed criterion instances, and the QID distribution of 32 wins, 20 ties, and 13 losses demonstrates nonuniform improvement. See the [pair CSV](../../benchmarks/gb-cc75/2026-08-18/F3-paired-secondary.csv) and [QID CSV](../../benchmarks/gb-cc75/2026-08-18/F3-qid-primary.csv).

### Structural non-delivery and regressions

Records include 25 pairs across 18 QIDs with Direct≤25% and Labyrinth≥75%; 21 were historically classified as incomplete process-promises/meta-preambles of at most 500 characters. Direct scored 0% on seven pairs; 30 pairs differed by at least 50 points, with zero reverse cases at the same threshold. This is retrospective content analysis; answer hashes in the public bundle cannot independently establish the textual classification.

Labyrinth also had negative differences on 34/167 pairs and 13/65 QIDs. Recompute rather than conceal negative-QID lists. Extra explanation, format deviations, leaked negative constraints, partial omissions, and competing interpretations are improvement candidates, not independent causal diagnoses.

### Taxonomy and unpublished subgroups

The historical detailed report used overlapping post hoc categories: output contamination, exact format, primary/alternative interpretation, negative-constraint leak, and partial omission. It also reported Implicit strict pass of 19.23% for Direct and 15.38% for Labyrinth, a -3.85-point difference. Subgroup labels and complete classification inputs are not included here; do not recompute or advertise these as newly verified subgroup results. Treat this as an attributed historical report illustrating that mean improvement does not entail strict improvement for every instruction category.

Future evaluation should freeze label definitions, multilabel policy, adjudication, annotator independence, agreement, and unclassified states. Judge-generated failure rationales are not independent evidence. Post hoc taxonomies are neither confirmatory tests nor proof of the TL mechanism.

### Separation from operational failures

Content scores and acquisition/grading failures are different layers. Do not claim no collapse by examining only scored answers. Retain Labyrinth acquisition ineligibility of 30/225, Direct 40/225, and ten unscored Judge packets separately. Public aggregates cannot identify ineligibility root causes or total costs. Correct holds and erroneous refusals/non-delivery also require separate labels.

See [methods](gb-cc75-methods-and-provenance.md) / [adversarial audit](gb-cc75-adversarial-audit.md) / [prospective plan](epistesys-prospective-evaluation-plan.md).
