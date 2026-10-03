# GB-CC75 documentation and aggregate verification — 2026-10-03

## 日本語

### 対象と順序

基準commitは`d74bdf020334d4efc2ea62267296c6a61fd046c1`。作業開始時にtracked／untracked差分はなかった。文書改訂前に[敵対的方法論監査](../docs/research/gb-cc75-adversarial-audit.md)を行い、75／65問、対応単位、欠測、Judge依存、予算交絡、機構の因果性、版の移転、再現性を分離した。

更新範囲はREADME、文書入口、研究仮説、制約、研究文書、公開集計、再計算／文書check script、限定GitHub Actionsである。Rust runtime、public schema、fixture、version、plugin登録、host設定は変更しない。identityは`6.3.2-alpha.2`を維持する。

### 観測した検証

- fail-first：再計算module追加前のtest起動はmodule不存在で失敗。これは予定された観測であり、baseline failureではない。
- `python -B scripts/research/test-gb-cc75.py`：11テスト成功。重複ID、範囲外pass、非有限score、strict偽装、平均／summary／欠測改変、hash改変、manifest traversalを拒否。
- `python -B scripts/research/verify-gb-cc75.py`：選択12ファイルのhash、65 QID、167ペア、主要差+19.714733855657578、strict差+8.982035928143711ポイント、81 strict passを再計算・照合。主要の負のQIDは13件。
- `python -B scripts/research/verify-research-docs.py`：日英順序、表の数値、metric token、local linkの構文検査。意味上のclaim parityは別に手動で再読する。
- 移入検査：選択済み公開JSON／CSVの値を保持、改行を正規化、移入元／移入後hashを分離。raw回答、非公開queue、credentials、個人pathは移入しない。
- `git diff --check`とstage内容を確認。source／version差分を含めない。

歴史的bootstrap区間とsign-flip p値を保存するが、今回それらを独立再生成していない。新規モデル呼出し・Grok生成・再採点は0件である。aggregate checkはgold、採点正確性、因果効果、alpha.2性能を証明しない。

### 補助ツールと限界

RepoSeiri `1.1.0+codex.20260823031336`をresearch profile・repository scopeで用いた。summary／linter／governanceは文書とrouteの補助観測であり、品質保証や論文採択ではない。機械のoverclaim risk、Unknown、policy holdを性能欠陥や自動修正権限へ読み替えない。最終のsummary／linter／summaryは同一source-session digestを照合してからGit操作へ進む。

Assurance-Compiler doctorは`ok=true`だった。今回Rust単位は変更・監査しておらず、統計・文章・repository safety・merge approvalをAssuranceで検証したとはしない。

Epistesys alpha.2のrepository doctor、TL doctor、TLDG doctorは応答した。一方、元の日本語user seedをstdinへ渡したDGCL facadeは高資源消費で長時間終了せず、この作業が起動した当該プロセスだけを停止した。成功・completion・host観測として記録せず、別lineへfallbackしない。これは単一invocationの診断であり、一般性能や故障原因を確定する実験ではない。READMEの研究効果を補助ツールの成否から導かない。

### 手動claim reviewと統合条件

日英両版で、現在の実装観測、前身の採点観測、設計意図、将来の研究仮説を再読した。平均を正答率／完全遵守へ、56一致を独立goldへ、hashを外部実験再現へ、前身結果をalpha.2へ昇格しない。標語、既存実装説明、clone／source-only境界を保持し、現workspaceのcrate数だけ13へ訂正した。

GitHub Actionsは公開集計と文書のread-only検査であり、Rust全体CIや研究効果の試験ではない。remoteの成功はGitHub runで別に確認する。本書はmerge／activation receiptを事前に主張せず、version昇格・再install・host再起動も行わない。

---

## English

### Scope and order

Baseline commit is `d74bdf020334d4efc2ea62267296c6a61fd046c1`. No tracked/untracked changes existed at task start. The [adversarial methodological audit](../docs/research/gb-cc75-adversarial-audit.md) preceded documentation revision, separating 75/65 tasks, paired units, missingness, Judge dependence, budget confounding, causal mechanisms, version transport, and reproducibility.

Changes cover README, documentation entry points, hypotheses, limitations, research documents, public aggregates, recomputation/document-check scripts, and bounded GitHub Actions. Rust runtime, public schemas, fixtures, version, plugin registration, and host configuration are unchanged. Identity remains `6.3.2-alpha.2`.

### Observed checks

- Fail-first: tests failed with a missing module before adding the recomputation implementation. This was planned, not a baseline failure.
- `python -B scripts/research/test-gb-cc75.py`: 11 tests passed, rejecting duplicate IDs, out-of-range passes, nonfinite scores, forged strict flags, altered means/summary/missingness, changed hashes, and manifest traversal.
- `python -B scripts/research/verify-gb-cc75.py`: checked hashes of 12 selected files, 65 QIDs, 167 pairs, primary difference +19.714733855657578, strict difference +8.982035928143711 points, and 81 strict passes. There are 13 negative primary QIDs.
- `python -B scripts/research/verify-research-docs.py`: syntactic checks for bilingual order, numeric tables, metric tokens, and local links. Semantic claim parity requires separate manual rereading.
- Import checks: selected published JSON/CSV values retained, line endings normalized, source/imported hashes separated. Raw answers, private queues, credentials, and personal paths excluded.
- Inspected `git diff --check` and staged content, excluding source/version changes.

Historical bootstrap intervals and sign-flip p-values are retained, not independently regenerated here. New model calls, Grok generation, and regrading count is zero. Aggregate checks establish neither gold, grading accuracy, causal effects, nor alpha.2 performance.

### Auxiliary tools and limits

Used RepoSeiri `1.1.0+codex.20260823031336` with research profile and repository scope. Summary/linter/governance are document/route observations, not quality guarantees or publication acceptance. Machine overclaim risks, Unknown, and policy holds are not performance defects or mutation authority. Final summary/linter/summary source-session digests are compared before Git operations.

Assurance-Compiler doctor returned `ok=true`. No Rust unit was changed or audited here; Assurance does not validate statistics, prose, repository safety, or merge approval in this task.

Epistesys alpha.2 repository, TL, and TLDG doctors responded. The DGCL facade receiving the original Japanese user seed over stdin instead ran without timely completion and with growing resources; only this task's invocation was stopped. It is not recorded as success, completion, or host observation, and no other line is used as fallback. This is one invocation diagnostic, not an experiment identifying general performance or root cause. README research effects do not follow from auxiliary-tool outcomes.

### Manual claim review and integration conditions

Both language halves were reread to separate current implementation observations, predecessor grading observations, design intent, and future hypotheses. Means are not task accuracy/complete compliance; 56 agreements are not independent gold; hashes are not external experimental replication; predecessor results do not transfer to alpha.2. Slogans, implementation descriptions, clone/source-only boundaries remain; only the current workspace count was corrected to 13 crates.

GitHub Actions checks public aggregates and documents read-only; it is neither whole-Rust CI nor research-effect evaluation. Confirm remote success separately in the GitHub run. This record does not preclaim merge/activation receipts, version promotion, reinstall, or host restart.
