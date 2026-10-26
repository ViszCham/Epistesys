# EPR priority remediation

## 日本語

対象は `6.3.2-alpha.2` の既知境界欠陥です。次の候補identityは `6.3.2-alpha.3` ですが、必須gate成功前には昇格しません。研究効果、実装修正、host activationは別の状態です。

| slice | 対象 | 状態 |
| --- | --- | --- |
| EPR-00 | 基線、個別欠陥、先行CI | 進行中 |
| EPR-01 | 日本語offset、進行、予算 | 未完了 |
| EPR-02/03 | 鍵非露出、source-bound authority | 未完了 |
| EPR-04/05 | Media権限、拒否時不変性 | 未完了 |
| EPR-06 | CSRとCPU/GPU算術境界 | 未完了 |
| EPR-07/08 | 共通実行監督、実接続の観測 | 未完了 |
| EPR-09/10/11 | 副作用、file修復、保存・再開 | 未完了 |
| EPR-12/13/14 | Media wire、graph identity、配布 | 未完了 |
| EPR-15/16 | 弱化・失効・断線、OS/feature gate | 未完了 |
| EPR-17/18/19 | 敵対監査、修復、再監査、closure | 未完了 |

各sliceはnegative/fail-first、最小実装、targeted validation、diff critique、個別消込の順です。既存差分を保持し、常時Hold、未使用helper、成功flag、必須jobのskipでは完了にしません。E12のうちTLDG backend/profile registryは既に事前存在検査をしており、その点を欠陥として再導入しません。

未知弱点ゼロ、一般意味精度、形式証明、host全体の強制を主張しません。研究評価と未観測host条件を、修正済み実装の証拠から自動昇格させません。

## English

The scope is known boundary defects in `6.3.2-alpha.2`. The next candidate identity is `6.3.2-alpha.3`, promoted only after required gates pass. Research effects, implementation remediation, and host activation are separate states.

| Slice | Scope | Status |
| --- | --- | --- |
| EPR-00 | Baseline, individual findings, early CI | In progress |
| EPR-01 | Japanese offsets, progress, budgets | Incomplete |
| EPR-02/03 | Secret redaction, source-bound authority | Incomplete |
| EPR-04/05 | Media authority, rejection invariance | Incomplete |
| EPR-06 | CSR and CPU/GPU arithmetic bounds | Incomplete |
| EPR-07/08 | Common supervision, actual connection observations | Incomplete |
| EPR-09/10/11 | Effects, file repair, persistence/resume | Incomplete |
| EPR-12/13/14 | Media wire, graph identity, distribution | Incomplete |
| EPR-15/16 | Weakening/invalidation/cuts, OS/feature gates | Incomplete |
| EPR-17/18/19 | Adversarial audit, repair, reaudit, closure | Incomplete |

Each slice follows negative fail-first testing, minimal implementation, targeted validation, diff critique, and per-finding discharge. Preserve existing changes. All-Hold behavior, unused helpers, success flags, and skipped mandatory jobs do not establish completion. The TLDG backend/profile registries cited under E12 already check for existing entries; do not reintroduce that mistaken finding.

No zero-unknown-weakness, general semantic accuracy, formal proof, or absolute host-enforcement claim is made. Implementation evidence never automatically promotes pending research/host observations.
