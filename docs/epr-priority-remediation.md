# EPR priority remediation

## 日本語

対象は `6.3.2-alpha.2` の既知境界欠陥です。昇格前functional gateの成功後、source identityを `6.3.2-alpha.3` へ更新し、alpha.3 identityでも宣言standalone profileのfunctional gateを通過しました。研究効果、実装修正、host activationは別の状態です。文書監査を含む最終source readinessは[検証記録](../validation/epr-release-verification-2026-10-27.md)で別に判定します。

| slice | 対象 | 状態 |
| --- | --- | --- |
| EPR-00 | 基線、個別欠陥、先行CI | 基線固定、実Rust-only負例は両OSで予定通り失敗。昇格前・alpha.3 identityのremote positive成功 |
| EPR-01 | 日本語offset、進行、予算 | 対象試験成功。共有work budgetをProgram IR／completionまで接続 |
| EPR-02/03 | 鍵非露出、source-bound authority | 対象試験成功。host原receipt、期限、scope、stage lineageも再検査 |
| EPR-04/05 | Media権限、拒否時不変性 | 対象試験成功。private snapshotと現在の権限を実backend経路で確認 |
| EPR-06 | CSRとCPU/GPU算術境界 | alpha.3 identityでCPUと実RTX 3060の限定parity試験成功 |
| EPR-07/08 | 共通実行監督、実接続の観測 | 所有process監督と登録driver観測、昇格前・alpha.3 identityのWindows／Linux gate成功 |
| EPR-09/10/11 | 副作用、file修復、保存・再開 | 実write後障害、修復、期限切れ履歴、fresh再開の対象試験成功 |
| EPR-12/13/14 | Media wire、graph identity、配布 | 対象試験とalpha.3固定commitのfresh CLI-source package・実CLI gate成功 |
| EPR-15/16 | 弱化・失効・断線、OS/feature gate | negative／positive対照と昇格前・alpha.3 identityのWindows／Linux gate成功 |
| EPR-17/18/19 | 敵対監査、修復、再監査、closure | チーム内監査A／Bと追加反例の修復を記録。宣言standalone gate成功。最終readinessは検証記録へ分離 |

各sliceはnegative/fail-first、最小実装、targeted validation、diff critique、個別消込の順です。既存差分を保持し、常時Hold、未使用helper、成功flag、必須jobのskipでは完了にしません。E12のうちTLDG backend/profile registryは既に事前存在検査をしており、その点を欠陥として再導入しません。

未知弱点ゼロ、一般意味精度、形式証明、host全体の強制を主張しません。研究評価と未観測host条件を、修正済み実装の証拠から自動昇格させません。

## English

The scope is known boundary defects in `6.3.2-alpha.2`. Source identity was updated to `6.3.2-alpha.3` after pre-promotion functional gates passed; declared standalone functional gates also passed under alpha.3 identity. Research effects, implementation remediation, and host activation remain separate. Final source readiness, including documentation audit, is assessed separately in the [verification record](../validation/epr-release-verification-2026-10-27.md).

| Slice | Scope | Status |
| --- | --- | --- |
| EPR-00 | Baseline, individual findings, early CI | Baseline pinned; real Rust-only negative failed as intended on both OSes. Pre-promotion and alpha.3-identity remote positives passed |
| EPR-01 | Japanese offsets, progress, budgets | Targeted tests passed; shared work budget connected through Program IR/completion |
| EPR-02/03 | Secret redaction, source-bound authority | Targeted tests passed; original host receipt, expiry, scope, and stage lineage rechecked |
| EPR-04/05 | Media authority, rejection invariance | Targeted tests passed; private snapshots/current authority observed through the actual backend |
| EPR-06 | CSR and CPU/GPU arithmetic bounds | CPU and bounded actual RTX 3060 parity tests passed under alpha.3 identity |
| EPR-07/08 | Common supervision, actual connection observations | Owned-process/registered-driver and pre-promotion/alpha.3-identity Windows/Linux gates passed |
| EPR-09/10/11 | Effects, file repair, persistence/resume | Real post-write failure, repair, expired history, and fresh-resume tests passed |
| EPR-12/13/14 | Media wire, graph identity, distribution | Targeted tests and alpha.3 fixed-commit fresh CLI-source package/real CLI gates passed |
| EPR-15/16 | Weakening/invalidation/cuts, OS/feature gates | Negative/positive controls and pre-promotion/alpha.3-identity Windows/Linux gates passed |
| EPR-17/18/19 | Adversarial audit, repair, reaudit, closure | Team audits A/B and additional counterexample fixes recorded; declared standalone gates passed. Final readiness remains a separate verification-record judgment |

Each slice follows negative fail-first testing, minimal implementation, targeted validation, diff critique, and per-finding discharge. Preserve existing changes. All-Hold behavior, unused helpers, success flags, and skipped mandatory jobs do not establish completion. The TLDG backend/profile registries cited under E12 already check for existing entries; do not reintroduce that mistaken finding.

No zero-unknown-weakness, general semantic accuracy, formal proof, or absolute host-enforcement claim is made. Implementation evidence never automatically promotes pending research/host observations.
