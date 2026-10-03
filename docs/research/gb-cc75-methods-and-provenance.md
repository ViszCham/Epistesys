# GB-CC75 methods and provenance

## 日本語

### データと適応

Complex Constraints Benchmark Set（Surge AI、`surgeai/ComplexConstraints`）への帰属を保持する。dataset revisionは`e9625c6f635f42b72cb85a04c2be64746f945126`、元CSV SHA-256は`3e2e5776af7f21a91e0fb9ac770b10aff525b8db5da114bcad72190f3b3ec413`、75問・1,559基準である。歴史的source manifestはCC-BY-4.0を記録する。今回bundleはdataset本文やharness codeを転載せず、選択済み公開集計を移入する。datasetのlicenseをEpistesys codeや全成果物のlicenseへ自動的に拡張しない。

公式harness revisionは`9ef1dd73d20030be94bdc399fd1c6ef7970f68d1`。単一completion・tool/agentなし・基準ごとの採点という[仕様](https://github.com/surge-ai/complex-constraints/blob/9ef1dd73d20030be94bdc399fd1c6ef7970f68d1/README.md)に対し、本研究は3取得epoch・2経路・適格性・重複再利用・独自Judgeを追加した。公式結果との数値比較は行わない。

### 実行設定の証拠段階

歴史的run IDは`20260816T174216284-grok-bench-embedded`、最終集計日は2026-08-18である。経路名は記録上のDirect GrokとGrok＋Labyrinthであり、現在のalpha.2を指さない。

| 項目 | 記録されている状態 | 限界 |
| --- | --- | --- |
| 生成側の正確なモデル・controller版・request設定 | 本公開bundleでは未確定 | Judgeの版を代用しない。provider側の実状態も未証明 |
| Judge | profile `CODEX-LABYRINTH-A-B-JUDGE-v1`、要求モデル`gpt-5.6-sol`、effort `xhigh`、Labyrinth `6.2.4` | モデルはCLI指定の記録でありprovider attestationではない |
| Judgeの盲検 | manifestはroute・epoch・peer answer・original prompt・QIDの非提示を指定 | styleからの推測や実際の全packetでの遵守は本bundleで独立検証できない |
| 採点単位 | 一つの回答の全基準を一つのfresh sessionへ渡す | 公式の基準一つずつの採点と異なる。sessionの分離は誤差独立性ではない |
| 重複 | 同QID・同answer SHA-256の判定を再利用 | 380適格route unit→370固有packet。反復判定の独立性を主張しない |
| retry | manifestは技術・schema・evidence failureに最大2試行、各1800秒、逐次実行を指定 | 内容が低得点だから選び直す設計ではないが、全provider処理は未観測 |
| 予算・順序・ツール | 経路間の等価性は未確定 | token、call、時間、費用、再試行、順序の完全な因果統制はない |

上のJudge設定はローカルmanifestの選択項目を今回監査した記述であり、manifest原本は同梱しない。正式な再実験には原本・prompt・version pinの公開可能性を再点検する必要がある。

### 選択と欠測のフロー

計画は75×3×2＝450 route unit、225対応ペア。歴史的分析記録ではDirect適格185/225、Labyrinth適格195/225、両適格171、片側のみ38、両非適格16だった。片側の適格性だけでそのQIDの全取得成功とは扱わない。

適格route unit 380は370固有Judge packetへ統合され、360採点成功・10技術的未採点となった。packet数とペア数は異なる。225→171→167は対応ペアのフローである。75→67→65はQIDのフローで、対応適格epochを持たない8問と、採点欠測の2問を区別する。

対応適格epochのない8 QIDはCIF-012、CIF-018、CIF-033、CIF-046、CIF-053、CIF-057、CIF-058、CIF-061。採点欠測で主要から外れる2 QIDはCIF-039とCIF-040。主要採用条件は「全適格対応epochに有効scoreがある」であり、全3 epochの取得成功を要求しない。適格性の判定実装・全理由・時点は公開集計だけでは再構成できず、選択biasは残る。

技術的未採点のordinal 197〜206は連続するが、根本原因は確定していない。後日回復は原本へ混ぜず、元hashへ結び付いた別recovery laneとする。今回回復・新規生成・再採点は行っていない。

### 公開bundleと再現可能性

[bundle](../../benchmarks/gb-cc75/2026-08-18/README.md)は前身repositoryで既に公開用に選択された12集計・監査ファイルを採用する。raw回答、private queue、credentials、cache、絶対host path、非開示のプロジェクト情報は採用しない。移入時に改行を正規化し、JSON値・CSV値を変更しない。`artifact-manifest.json`は移入元hashと本bundleのhashを区別する。

点推定の再計算は可能。基準の妥当性確認、回答の独立再採点、provider条件の再実験は不可。歴史的bootstrap／sign-flipのseedと出力は保存するが、RNG・実装・入力全体を再実行したとの主張はしない。新scriptの失敗検出は集計整合性の検査であり採点の正しさではない。

原本のhash凍結は現在byteの内部整合性を示すにとどまり、独立timestamp、過去非改変、provider状態、著作権上の全権利処理を証明しない。公開のscopeは選択bundleに限定する。[統計仕様](gb-cc75-statistical-analysis.md)／[監査](gb-cc75-adversarial-audit.md)。

---

## English

### Data and adaptation

Retain attribution to the Complex Constraints Benchmark Set (Surge AI, `surgeai/ComplexConstraints`). Dataset revision is `e9625c6f635f42b72cb85a04c2be64746f945126`, source CSV SHA-256 is `3e2e5776af7f21a91e0fb9ac770b10aff525b8db5da114bcad72190f3b3ec413`, with 75 tasks and 1,559 criteria. The historical source manifest records CC-BY-4.0. This bundle imports selected published aggregates, not dataset text or harness code. Do not extend the dataset license automatically to Epistesys code or every artifact.

Official harness revision is `9ef1dd73d20030be94bdc399fd1c6ef7970f68d1`. Relative to its [single-completion, no-tool/agent, criterion-level scoring specification](https://github.com/surge-ai/complex-constraints/blob/9ef1dd73d20030be94bdc399fd1c6ef7970f68d1/README.md), this study adds three acquisition epochs, two routes, eligibility, duplicate reuse, and a custom Judge. No numerical comparison with official results is made.

### Evidence levels for execution settings

Historical run ID is `20260816T174216284-grok-bench-embedded`; final aggregation date is 2026-08-18. Direct Grok and Grok plus Labyrinth are recorded historical route labels, not the current alpha.2.

| Item | Recorded state | Limitation |
| --- | --- | --- |
| Exact generator model/controller version/request settings | Unresolved in this public bundle | Do not substitute Judge version; provider reality is unproven |
| Judge | Profile `CODEX-LABYRINTH-A-B-JUDGE-v1`, requested model `gpt-5.6-sol`, effort `xhigh`, Labyrinth `6.2.4` | Model identity is a CLI request record, not provider attestation |
| Judge blinding | Manifest specifies withholding route, epoch, peer answer, original prompt, QID | Style leakage and actual compliance on all packets are not independently verifiable here |
| Grading unit | All criteria for one answer in one fresh session | Different from official one-criterion-at-a-time grading; separate sessions do not imply independent errors |
| Duplicates | Verdict reuse for identical QID and answer SHA-256 | 380 eligible route units→370 unique packets; no claim of independent repeated judgments |
| Retry | Manifest specifies at most 2 attempts for technical/schema/evidence failure, 1800 seconds each, sequential execution | Not designed to select new low-scoring answers, but complete provider processing remains unobserved |
| Budget/order/tools | Cross-route equivalence unresolved | No complete causal control of tokens, calls, time, cost, retries, or order |

Judge settings above describe selected local-manifest fields inspected in this audit; the original manifests are excluded. A formal rerun requires further disclosure review of originals, prompts, and version pins.

### Selection and missingness flow

The plan was 75×3×2=450 route units and 225 pairs. Historical analysis records report Direct eligibility 185/225, Labyrinth 195/225, both eligible 171, only one eligible 38, neither eligible 16. One-sided eligibility does not establish successful acquisition of every epoch for that QID.

The 380 eligible route units became 370 unique Judge packets: 360 successfully scored and ten technically unscored. Packet counts and pair counts differ. The pair flow is 225→171→167. The QID flow is 75→67→65, separating eight tasks without paired-eligible epochs from two with scoring missingness.

The eight QIDs without paired-eligible epochs are CIF-012, CIF-018, CIF-033, CIF-046, CIF-053, CIF-057, CIF-058, CIF-061. The two excluded from primary due to scoring missingness are CIF-039 and CIF-040. Primary eligibility means valid scores on every eligible paired epoch, not successful acquisition on all three epochs. Public aggregates cannot reconstruct the eligibility implementation, full reasons, or timing; selection bias remains.

Technically unscored ordinals 197–206 are consecutive, but their root cause is unidentified. Later recovery must remain a separate lane bound to original hashes rather than mixed into the original. No recovery, new generation, or regrading was performed here.

### Public bundle and reproducibility

The [bundle](../../benchmarks/gb-cc75/2026-08-18/README.md) adopts 12 aggregate/audit files already selected for publication in the predecessor repository. Raw answers, private queues, credentials, caches, absolute host paths, and non-disclosable project material are excluded. Import normalizes line endings without changing JSON/CSV values. `artifact-manifest.json` distinguishes source hashes from this bundle's hashes.

Point estimates can be recomputed. Criterion validity, independent answer regrading, and reruns under provider conditions cannot. Historical bootstrap/sign-flip seeds and outputs are preserved without claiming reruns of the RNG, implementation, or complete inputs. The new script's rejection tests check aggregate consistency, not grading truth.

Original hash freezing establishes current-byte internal consistency, not independent timestamps, historic non-tampering, provider state, or comprehensive rights clearance. Publication scope is limited to the selected bundle. See [statistics](gb-cc75-statistical-analysis.md) / [audit](gb-cc75-adversarial-audit.md).
