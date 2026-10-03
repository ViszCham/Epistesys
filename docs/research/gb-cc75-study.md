# GB-CC75: a retrospective exploratory paired study of predecessor control routes

## 日本語

### 要旨

複雑な指示への適合を外部制御経路が支援し得るかを調べるため、前身Labyrinth-Codexに関するGB-CC75の歴史的記録を整理した。これはSurge AIのComplexConstraints由来の75問を用いた、3取得epoch・2経路の後ろ向き探索的な対応付き評価であり、無作為化された確認的試験ではない。2026-08-18に確定した独自Judgeの集計において、主要解析の65 QIDの等重み平均ルーブリック適合度はDirect Grok 74.38%、Grok＋Labyrinth 94.10%、差+19.71パーセントポイントだった。記録されたQID bootstrap 95%区間は+12.20〜+27.83ポイントだった。一方、採点済み167ペアの全基準達成率は39.52%から48.50%への変化であり、QID平均では13問でLabyrinth経路が下回った。欠測、採用条件、Judge依存、経路間予算差、版の移転を限界として残す。結果は前身の限定観測であり、Epistesys alpha.2の改善、特定機構の因果効果、hallucinationや長期sessionの効果を実証しない。

### 1. 問題設定と研究問い

Epistesysは、制約・権限・状態・証拠を保持し、未裏付けのcandidateを検証済み状態と区別する制御系を研究する。GB-CC75は、そのうち複雑な指示のルーブリック適合を扱う前身の経験的観測である。理論上の機構が存在することと、利用結果が改善することは異なる。

- RQ1：採点可能な対応QIDで、経路間の平均適合度はどう異なるか。
- RQ2：平均適合度と全基準同時達成は同じ改善を示すか。
- RQ3：負の差、欠測、構造的未完了はどこに残るか。
- RQ4：この観測から支持できる範囲と、次の独立検証が必要な範囲は何か。

### 2. 設計と解析対象

元の問題集合は75問・1,559固有基準である。GB-CC75は3取得epoch、Direct Grok／Grok＋Labyrinth、独自の適格性判定、重複回答の判定再利用、独自Judgeを追加した。公式scorer／leaderboardと同等ではない。[方法と由来](gb-cc75-methods-and-provenance.md)に既知・未確定の設定を分けて記録する。

計画225対応epochペアのうち、両経路適格171ペア、両経路採点済み167ペアだった。問題単位では75→対応適格67→主要65 QIDとなる。主要解析は「すべての適格対応epochが採点済み」のQIDを等重みで平均する。各QIDに3成功epochがあることや、全取得が成功したことを意味しない。反復ペアと基準は独立問題数ではない。[統計仕様](gb-cc75-statistical-analysis.md)を参照する。

### 3. 結果

| 解析／集計単位 | Direct Grok | Grok＋Labyrinth | 差 | 歴史的な95%区間 |
| --- | ---: | ---: | ---: | --- |
| 主要：65 QID等重み平均 | 74.38% | 94.10% | +19.71ポイント | +12.20〜+27.83ポイント |
| 副次：167ペア平均、65 QIDクラスタ | 79.17% | 94.03% | +14.86ポイント | +8.83〜+21.63ポイント |
| 記述：基準インスタンス加重 | 80.69%（2841/3521） | 94.04%（3311/3521） | +13.35ポイント | — |
| 記述：回答の全基準達成 | 39.52%（66/167） | 48.50%（81/167） | +8.98ポイント | — |

主要解析では32勝・20同点・13敗、副次では55勝・78同点・34敗だった。相対向上26.50%は主要平均に対する派生値であり、正答率や知能の向上率ではない。差は丸め前に計算する。3,521は採点済みペアに現れる各経路の基準インスタンス数であり、元の固有基準数ではない。

10技術的未採点packetはCIF-039／CIF-040に集中し、4適格ペアと2主要QIDが失われた。欠測を0点化せず、recovery laneは実行していない。67対応適格QID中の2欠測だけを[-1, 1]で補う差のboundは+16.14〜+22.11ポイントである。75予定QIDすべてのboundではなく、取得非適格やJudge誤差を覆わない。

同一モデル系の内部再点検は事後選択した56基準で56一致・0不一致だった。独立した人間goldによる採点精度推定ではない。[失敗分析](gb-cc75-failure-analysis.md)に負の差と分類上の限界を記載する。

### 4. 妥当性への脅威

構成概念：ルーブリック適合は意味正確性、hallucination、authority安全性、長期保持の代替ではない。内的妥当性：生成設定・総予算・順序の完全な統制は本公開bundleから確定できず、個別機構の効果を識別できない。測定：ラベル盲検設計でもLabyrinthを含むJudgeの系統誤差は残る。選択：主要値は適格・採点済み条件に依存する。統計：問題集合は固定された選定集合であり、独立無作為標本ではない。外的妥当性：単一の歴史的実行から別モデル、別問題族、Epistesys alpha.2へ移転できない。

[敵対的方法論監査](gb-cc75-adversarial-audit.md)のA01〜A12を、説明上の対処と未解決の実験課題に分けて保持する。p値、hash、build/test passはこれらの脅威を解消しない。

### 5. 考察と結論

この実行ではLabyrinth経路の平均適合度が明確に高かった一方、完全遵守や均一な改善は成立していない。これはconstraint-preserving controlの研究を動機付ける限定的な観測であり、設計理論やTLの因果機序の証明ではない。Epistesysでは同等予算、独立採点、未使用holdout、運用失敗と保留を含む[前向き評価](epistesys-prospective-evaluation-plan.md)によって検証する。これは評価案であり、実行済み・事前登録済みの試験ではない。

### 6. 成果物と参照

[公開集計bundle](../../benchmarks/gb-cc75/2026-08-18/README.md)から点推定を再計算できる。raw回答・全基準別判定・生成manifestは同梱せず、独立再採点・provider再実験は再現できない。歴史的区間・検定を保持するが、今回の検証scriptはそれらを再生成しない。

- Surge AI：[固定harness仕様](https://github.com/surge-ai/complex-constraints/blob/9ef1dd73d20030be94bdc399fd1c6ef7970f68d1/README.md)、[固定dataset revision](https://huggingface.co/datasets/surgeai/ComplexConstraints/tree/e9625c6f635f42b72cb85a04c2be64746f945126)。
- Zheng et al. (2023)：[Judging LLM-as-a-Judge with MT-Bench and Chatbot Arena](https://arxiv.org/abs/2306.05685)。Judgeの既知の限界に関する一次研究であり、本実行でのbias発生を証明するものではない。

---

## English

### Abstract

To investigate whether external control routes can support adherence to complex instructions, this report organizes historical GB-CC75 records for the predecessor Labyrinth-Codex. It is a retrospective exploratory paired evaluation with three acquisition epochs and two routes on 75 tasks adapted from Surge AI's ComplexConstraints, not a randomized confirmatory trial. Under the custom Judge aggregates finalized on 2026-08-18, equal-QID-weighted mean rubric adherence across 65 primary QIDs was 74.38% for Direct Grok and 94.10% for Grok plus Labyrinth, a +19.71-percentage-point difference. The recorded QID-bootstrap 95% interval was +12.20 to +27.83 points. However, all-criteria pass across 167 scored pairs changed from 39.52% to 48.50%, and the Labyrinth route scored lower on 13 QIDs. Missingness, eligibility selection, Judge dependence, route-budget differences, and version transportability remain limitations. These are bounded predecessor observations, not evidence of alpha.2 improvement, causal effects of specific mechanisms, hallucination reduction, or long-session benefits.

### 1. Problem and research questions

Epistesys studies control systems retaining constraints, authority, state, and evidence while separating unsupported candidates from validated states. GB-CC75 is predecessor empirical evidence concerning rubric adherence on complex instructions. The existence of a proposed mechanism and improved user outcomes are different propositions.

- RQ1: How does mean adherence differ between routes on scoreable paired QIDs?
- RQ2: Do mean adherence and simultaneous all-criteria achievement improve similarly?
- RQ3: Where do negative differences, missingness, and structural non-delivery remain?
- RQ4: What is supported by these observations, and what needs independent evaluation?

### 2. Design and analysis population

The source set contains 75 tasks and 1,559 unique criteria. GB-CC75 adds three acquisition epochs, Direct Grok/Grok plus Labyrinth, custom eligibility, duplicate-answer verdict reuse, and a custom Judge. It is not equivalent to the official scorer/leaderboard. [Methods/provenance](gb-cc75-methods-and-provenance.md) separates known and unresolved settings.

Of 225 planned paired epochs, 171 were eligible on both routes and 167 scored on both routes. At task level the flow is 75→67 paired-eligible→65 primary QIDs. The primary analysis equally weights QIDs with valid scores for every eligible paired epoch. This does not require three successful epochs per QID or success on every acquisition. Repeated pairs and criteria are not independent task counts. See the [statistical specification](gb-cc75-statistical-analysis.md).

### 3. Results

| Analysis / aggregation unit | Direct Grok | Grok plus Labyrinth | Difference | Historical 95% interval |
| --- | ---: | ---: | ---: | --- |
| Primary: 65 equally weighted QID means | 74.38% | 94.10% | +19.71 points | +12.20 to +27.83 points |
| Secondary: 167 pair means, 65 QID clusters | 79.17% | 94.03% | +14.86 points | +8.83 to +21.63 points |
| Descriptive: criterion-instance weighted | 80.69% (2841/3521) | 94.04% (3311/3521) | +13.35 points | — |
| Descriptive: all-criteria pass per answer | 39.52% (66/167) | 48.50% (81/167) | +8.98 points | — |

The primary comparison had 32 wins, 20 ties, and 13 losses; the secondary had 55 wins, 78 ties, and 34 losses. Relative uplift of 26.50% is derived from the primary mean, not task accuracy or intelligence improvement. Differences are calculated before rounding. The 3,521 count is criterion instances per route across scored pairs, not unique source criteria.

Ten technically unscored packets concentrated in CIF-039/CIF-040 lost four eligible pairs and two primary QIDs. Missingness was not zero-imputed; no recovery lane was executed. Bounding only the two missing QIDs among 67 paired-eligible QIDs to [-1, 1] yields +16.14 to +22.11 points. This is not a bound over all 75 planned QIDs and does not cover acquisition ineligibility or Judge error.

A same-model-family internal review found 56 agreements and zero disagreements on 56 post-result selected criteria. It is not a grading-accuracy estimate against independent human gold. The [failure analysis](gb-cc75-failure-analysis.md) retains negative differences and labeling limitations.

### 4. Threats to validity

Construct validity: rubric adherence is not a proxy for semantic correctness, hallucination, authority safety, or long-session retention. Internal validity: this public bundle cannot establish complete control of generator settings, total budgets, or order, and cannot identify mechanism effects. Measurement: a Labyrinth-based Judge may retain systematic error despite label-blinding design. Selection: primary estimates condition on eligibility and successful scoring. Statistical validity: tasks are a fixed curated set, not an independent random population sample. External validity: a single historical run cannot establish transfer to other models, task families, or Epistesys alpha.2.

Keep A01–A12 in the [adversarial methodological audit](gb-cc75-adversarial-audit.md), distinguishing documentation corrections from unresolved experimental questions. P-values, hashes, and build/test passes do not resolve these threats.

### 5. Discussion and conclusion

The Labyrinth route had clearly higher mean adherence in this run, but complete compliance and uniform improvement were not established. This bounded observation motivates constraint-preserving control research; it does not prove a design theory or the causal mechanism of TL. Epistesys will test these questions using the [prospective plan](epistesys-prospective-evaluation-plan.md), including matched budgets, independent grading, unused holdouts, operational failures, and holds. This is a proposed plan, not an executed or preregistered trial.

### 6. Artifacts and references

The [public aggregate bundle](../../benchmarks/gb-cc75/2026-08-18/README.md) supports point-estimate recomputation. Raw answers, complete criterion-level verdicts, and generator manifests are excluded; independent regrading and provider reruns are not reproducible from this bundle. Historical intervals/tests are retained, not regenerated by the new verification script.

- Surge AI: [pinned harness specification](https://github.com/surge-ai/complex-constraints/blob/9ef1dd73d20030be94bdc399fd1c6ef7970f68d1/README.md), [pinned dataset revision](https://huggingface.co/datasets/surgeai/ComplexConstraints/tree/e9625c6f635f42b72cb85a04c2be64746f945126).
- Zheng et al. (2023): [Judging LLM-as-a-Judge with MT-Bench and Chatbot Arena](https://arxiv.org/abs/2306.05685). Primary research on Judge limitations, not proof that a particular bias occurred in this run.
