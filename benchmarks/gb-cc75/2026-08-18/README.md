# GB-CC75 predecessor aggregate bundle

## 日本語

2026-08-18の前身Labyrinth-Codexによる派生ベンチマークの選択済み公開集計である。Epistesys `6.3.2-alpha.2`の性能評価ではない。公式ComplexConstraints scorer／leaderboardと同等でもない。[研究本文](../../../docs/research/gb-cc75-study.md)と[方法](../../../docs/research/gb-cc75-methods-and-provenance.md)を先に読む。

### 数値と単位

主要65 QID：74.38%→94.10%、差+19.71ポイント、記録された95%区間+12.20〜+27.83ポイント。strictは167対応ペア：66/167＝39.52%→81/167＝48.50%、差+8.98ポイント。平均適合と全基準達成は異なる。75予定QIDすべての成功率ではない。

### ファイル

- `GB_CC75_FINAL_SUMMARY.json`：歴史的な最終集計とclaim boundary。
- `F3-qid-primary.csv`、`F3-paired-secondary.csv`、`F3-statistics-report.json`：点推定、歴史的な区間／検定、seed。
- `F1-missingness-incident-audit.json`、`F4-incident.csv`、`F4-missingness-bounds.csv`：欠測と限定bound。
- `F2-audit-selection-r3.json`、`F2-scoring-accuracy-audit-r2.json`、`F4-adjudication.csv`：事後選択した内部判定一致。ファイル名のaccuracyはlegacy名であり、独立採点精度の実証ではない。
- `F4-direct-collapse.csv`：歴史的な構造的未完了の分類。本文は同梱しない。
- `F4-primary-source-reference.json`：Surge AIへの帰属、固定revision、元license記録。
- `artifact-manifest.json`：選択12ファイルの移入元hashと移入後hash。READMEやmanifest自身を自己hashしない。

### 再計算と公開境界

repository rootで`python scripts/research/verify-gb-cc75.py`と`python scripts/research/test-gb-cc75.py`を実行する。新しいモデル呼出し、再採点、bootstrap／sign-flipの再生成は行わない。JSON値・CSV値を保持し、改行を正規化した。raw回答、private queue、credential、個人path、cacheは含まない。移入前のSHA manifestは他の文書も参照するため移入せず、選択bundle用manifestを新設した。

これは集計再計算に限定された公開surfaceであり、独立再採点、実行条件の完全な再実験、gold、provider attestation、性能証明ではない。dataset licenseをrepository全体へ適用しない。

---

## English

This is the selected published aggregate bundle for the predecessor Labyrinth-Codex adapted benchmark dated 2026-08-18, not a performance evaluation of Epistesys `6.3.2-alpha.2`. It is not equivalent to the official ComplexConstraints scorer/leaderboard. Read the [study](../../../docs/research/gb-cc75-study.md) and [methods](../../../docs/research/gb-cc75-methods-and-provenance.md) first.

### Values and units

Primary 65 QIDs: 74.38%→94.10%, difference +19.71 points, recorded 95% interval +12.20 to +27.83 points. Strict uses 167 pairs: 66/167=39.52%→81/167=48.50%, difference +8.98 points. Mean adherence and all-criteria achievement differ; neither is success across all 75 planned QIDs.

### Files

- `GB_CC75_FINAL_SUMMARY.json`: historical final aggregates and claim boundary.
- `F3-qid-primary.csv`, `F3-paired-secondary.csv`, `F3-statistics-report.json`: point estimates, historical intervals/tests, seeds.
- `F1-missingness-incident-audit.json`, `F4-incident.csv`, `F4-missingness-bounds.csv`: missingness and limited bounds.
- `F2-audit-selection-r3.json`, `F2-scoring-accuracy-audit-r2.json`, `F4-adjudication.csv`: post-result internal agreement. Accuracy in the legacy filename does not establish independent grading accuracy.
- `F4-direct-collapse.csv`: historical structural non-delivery labels; answer text is excluded.
- `F4-primary-source-reference.json`: Surge AI attribution, pinned revisions, recorded source license.
- `artifact-manifest.json`: source/imported hashes for the 12 selected files; README and the manifest itself are not self-hashed.

### Recomputation and disclosure boundary

From repository root run `python scripts/research/verify-gb-cc75.py` and `python scripts/research/test-gb-cc75.py`. No new model calls, regrading, or bootstrap/sign-flip regeneration occur. JSON/CSV values are preserved with normalized line endings. Raw answers, private queues, credentials, personal paths, and caches are excluded. The source SHA manifest references other documents and is not imported; a selected-bundle manifest is provided instead.

This public surface supports aggregate recomputation, not independent regrading, complete experimental reruns, gold, provider attestation, or performance proof. The dataset license is not applied to the entire repository.
