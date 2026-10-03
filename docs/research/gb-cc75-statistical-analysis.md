# GB-CC75 statistical specification and inference boundary

## 日本語

### 1. 集計単位とestimand

QIDをq、取得epochをe、経路をrとする。回答のscore `s[q,e,r]` は、その回答のpass基準数を基準数で割った[0, 1]の適合率である。`d[q,e] = s[q,e,Labyrinth] - s[q,e,Direct]`。主要集合Qは全適格対応epochが採点済みの65 QID、`E[q]`はそのQIDの適格対応epoch集合である。

```text
d[q]       = mean(d[q,e] for e in E[q])
Delta_QID  = mean(d[q] for q in Q)                 # QID等重み
Delta_pair = mean(d[q,e] for all 167 scored pairs) # 成功ペア等重み
```

これは全75問、全予定epoch、全失敗を含む運用上の効果ではない。`E[q]`の大きさが異なるので主要と副次の重みは違う。基準数加重の記述値とstrict passも別のestimandであり、都合のよい指標へすり替えない。

### 2. 点推定と反復の依存

主要差は+19.71ポイント、副次差は+14.86ポイント、基準インスタンス加重差は+13.35ポイント、strict差は+8.98ポイント。主要の相対向上は `Delta_QID / Direct_QID_mean`＝26.50%。パーセントポイントと相対%を区別し、知能・正答率・hallucination削減へ読み替えない。

3,521基準インスタンス／経路と167ペアは独立標本数ではない。QID内のepoch・criteria・重複判定に依存がある。主要ではQID内平均後にQIDを等重み化し、副次の不確実性はQIDクラスタを保持した。問題族内の依存やJudgeの共有誤差が完全に処理されたとは言わない。

### 3. 歴史的な不確実性出力

記録されたpercentile bootstrapは100,000再標本化、seed `5566811321892736085`。主要95%区間は+12.20〜+27.83ポイント、副次は+8.83〜+21.63ポイント。sign-flipは100,000試行、seed `4847648875745931313`、両解析の記録p値は`9.99990000099999e-6`。p=0とは書かず、Monte Carlo解像度付近の値として扱う。

対応付き設計だけでsign-flipの対称性／交換可能性仮定が正当化されるわけではない。無作為割当による因果的randomization inferenceでもない。固定された選定問題集合のQID再標本化を、あらゆる指示母集団に対する保証へ一般化しない。区間は取得選択、Judge系統誤差、版の移転を覆わない。事前登録と多重比較補正は本bundleから確立されていないため、主・副次という名称を確認的検定の証拠としない。

今回のscriptは点推定と整合性だけを再計算する。上の区間・p値は歴史的出力として照合し、独立再生成したとは記録しない。

### 4. 欠測とbound

主要の技術欠測boundは、対応適格67 QID中65観測・2欠測について、各未観測差が[-1, 1]という仮定の下で `(65 * Delta_QID ± 2) / 67` と計算する。副次は171適格ペア中167観測・4欠測について `(167 * Delta_pair ± 4) / 171`。それぞれ+16.14〜+22.11、+12.18〜+16.86ポイント。

これは信頼区間でも、75全予定QIDの感度分析でもない。対応適格epochのない8 QIDの取得選択、片側のみ適格の38ペア、両非適格16ペア、Judge相関誤差を覆わない。missingを勝手に0へ置換しない。技術欠測がランダムだったとも仮定しない。

### 5. 再計算と検証のscope

```powershell
python scripts/research/verify-gb-cc75.py
python scripts/research/test-gb-cc75.py
```

公開CSVから、ID重複、整数pass範囲、有限score、strict定義、ペア→QID平均、勝敗、主要／副次／加重／strict点推定、欠測boundを検査する。JSON／CSV間の整合性、移入後hashも検査する。負例では重複・範囲外・strict偽装・集計改変・manifest改変を拒否する。scriptの成功は独立gold、採点正確性、causal effect、provider再現ではない。

[方法](gb-cc75-methods-and-provenance.md)／[本文](gb-cc75-study.md)／[公開bundle](../../benchmarks/gb-cc75/2026-08-18/README.md)。

---

## English

### 1. Units and estimands

Let q denote QID, e acquisition epoch, and r route. Answer score `s[q,e,r]` is passed criteria divided by criterion count, in [0, 1]. Define `d[q,e] = s[q,e,Labyrinth] - s[q,e,Direct]`. Primary set Q contains 65 QIDs with valid scores on every eligible paired epoch; `E[q]` is that QID's eligible paired-epoch set.

```text
d[q]       = mean(d[q,e] for e in E[q])                 # within-QID mean
Delta_QID  = mean(d[q] for q in Q)                     # equal QID weights
Delta_pair = mean(d[q,e] for all 167 scored pairs)     # equal scored-pair weights
```

This is not an operational effect across all 75 tasks, planned epochs, and failures. Different `E[q]` sizes yield different primary and secondary weights. Criterion-weighted descriptive scores and strict pass are further estimands, not interchangeable alternatives selected for favorable results.

### 2. Point estimates and repeated-measure dependence

Primary difference is +19.71 points, secondary +14.86, criterion-instance-weighted +13.35, and strict +8.98. Primary relative uplift is `Delta_QID / Direct_QID_mean`=26.50%. Distinguish percentage points from relative percent; do not relabel them intelligence, task accuracy, or hallucination reduction.

The 3,521 criterion instances per route and 167 pairs are not independent sample counts. Epochs, criteria, and reused judgments within QIDs are dependent. Primary analysis averages within QID before equal weighting; secondary uncertainty retains QID clusters. This does not establish complete treatment of task-family dependence or shared Judge errors.

### 3. Historical uncertainty outputs

Recorded percentile bootstrap uses 100,000 resamples with seed `5566811321892736085`. Primary 95% interval is +12.20 to +27.83 points, secondary +8.83 to +21.63. Sign-flip uses 100,000 trials with seed `4847648875745931313`; both recorded p-values are `9.99990000099999e-6`. Do not report p=0; treat the value as near Monte Carlo resolution.

Pairing alone does not justify sign-flip symmetry/exchangeability assumptions. This is not causal randomization inference from randomized assignment. QID resampling of a fixed curated set is not a guarantee over all instruction populations. Intervals do not cover acquisition selection, systematic Judge error, or version transfer. Preregistration and multiplicity correction are not established by this bundle; primary/secondary names do not establish confirmatory testing.

The new script recomputes only point estimates and consistency. Intervals/p-values above are retained historical outputs, not independently regenerated results.

### 4. Missingness and bounds

The primary technical-missingness bound assumes each unobserved difference lies in [-1, 1] for 65 observed and two missing QIDs among 67 paired-eligible QIDs: `(65 * Delta_QID ± 2) / 67`. Secondary uses 167 observed and four missing pairs among 171 eligible pairs: `(167 * Delta_pair ± 4) / 171`. The ranges are +16.14 to +22.11 and +12.18 to +16.86 points respectively.

These are neither confidence intervals nor sensitivity analyses over all 75 planned QIDs. They exclude acquisition selection of eight QIDs with no paired-eligible epoch, 38 one-sided pairs, 16 neither-eligible pairs, and correlated Judge error. Do not silently replace missingness with zero or assume technical missingness was random.

### 5. Recomputation and verification scope

```powershell
python scripts/research/verify-gb-cc75.py
python scripts/research/test-gb-cc75.py
```

Public CSV checks cover duplicate identities, integer pass ranges, finite scores, strict definitions, pair-to-QID means, outcomes, primary/secondary/weighted/strict point estimates, and missingness bounds. JSON/CSV consistency and imported hashes are checked. Negative tests reject duplicates, out-of-range values, forged strict flags, changed aggregates, and changed manifests. Script success is not independent gold, grading accuracy, causal effect, or provider replication.

See [methods](gb-cc75-methods-and-provenance.md) / [study](gb-cc75-study.md) / [public bundle](../../benchmarks/gb-cc75/2026-08-18/README.md).
