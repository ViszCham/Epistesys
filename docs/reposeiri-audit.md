# Epistesys RepoSeiri audit / RepoSeiri監査

## 日本語

### EPR候補の観測：最終sourceではない

2026-10-27 JSTに、RepoSeiri `1.1.0+codex.20260823031336`のRust coreをbundle launcherから実行し、`common` profile・**subtree scope**で固定source-only exportを検査した。export元commitは`cfe29f89403f44ed1cd55552cf652c7c11cb3425`、summaryのsource-session digestは`sha256:4cd84e782284363ae1ec1b98b34f7950c9bc3cd827f5d5ae913a28611524a9d3`である。exportは親repositoryを継承する作業用subtreeのため、履歴のrepository scopeとは同一scopeとして比較しない。

`summary`と`linter`を同じ固定exportへ実行し、書込みは行っていない。これは`6.3.2-alpha.2`から次の候補を点検する中間snapshotであり、alpha.3の最終source・release・公開準備を認証するものではない。後続修正やこの文書の更新へ、以下の数値をそのまま引き継がない。

| 観測 | 固定候補の結果 |
| --- | --- |
| Entries / document events / document diagnostics | 389 / 8,064 / 0 |
| Documents | 64 selected / 64 candidates、918,038 bytes |
| Markdown / conflict coverage | `Complete` / `Complete` |
| Limit-exceeded observations / scopes | 0 / 0 |
| Repository observations | 52 present、25 absent、1 unknown、0 conflict |
| Root README grammar nodes | 136 |
| Profile fit | 70/100、top profile `cli`。品質・性能の点数ではない |
| Summary | underclaim opportunities 1、overclaim risks 8、findings 4 |
| Claim drafts / baseline unknown | drafts 0、baseline unknown 8 |
| Patch operations / holds | 2 / 7、`writes_files=false` |
| Wording linter | 40 files、3 generated surfaces、2 findings、10 suppressed boundary exceptions |

linterの2件は`docs/system-architecture.md`の固定export上の137行・139行で、対応表の「Not equivalent to」列にある`guaranteed meaning preservation`と`correctness guarantee`を検出したものだった。文脈上はいずれも保証を否定する境界説明であり、手動確認では肯定的な保証claimではないと判断した。ただし、machine resultは2 findingsのまま保持し、0件へ書き換えたり、他のriskやholdを一括解除したりしない。

`underclaim`、`overclaim`、`baseline unknown`、finding、holdは、それぞれ異なるreview状態である。8件のriskを8件の実装欠陥と読み替えず、1件のunderclaimを性能実証と読み替えない。patch提案を実行せず、未定義のpolicyやlicense、support promiseを補ってholdを消さない。

### 最終snapshotの確認は別途必要

最終source export・同じscope/profileのqueryとlinter・対応するsource-session digestの固定は**この候補観測の後続作業**である。この文書はその成功を先取りしない。sourceやscope、digestが変われば新しい観測として記録し、古い候補のcoverage、finding数、claim draft状態を最終版の結果として表示しない。commit/push/merge、release、package、host activationはRepoSeiriの出力だけから許可・完了にならない。

### EPI-05の履歴：6.3.2-alpha.1

以下は初回clone時の履歴であり、現在のsourceへの再監査結果ではない。RepoSeiri `1.1.0+codex.20260823031336`の`common` profileで、当時のEpistesys cloneをrepository scopeとして監査した。queryとlinterはそれぞれ同一内容のsource snapshotへ実行し、writes filesはfalseだった。

#### 当時の観測

- Entries: 183、document events: 3,107、document diagnostics: 0。
- Documents: 29 selected / 29 candidates、selected bytes: 288,128。
- Markdown coverage: Complete。conflict coverage: Complete。
- Repository observations: 30 present、43 absent、1 unknown、0 conflict。
- Root README grammar nodes: 22。profile fit score: 40/100（top profile `ml`）。これは文書品質や機能品質ではなく、RepoSeiriのprofile適合観測である。
- Wording linter: 21 files、3 generated surfaces、0 findings。
- Patches: writes files false。1 edit-existing preview、6 skeleton review items、5 manual decisions、11 held items。patch suggestionは適用していない。

#### 履歴の解釈境界

`absent`、`unknown`、hold、profile fit score、claim draft stateは、機能の欠落、正しさ、公開準備、security、license、support promiseを自動的に意味しない。当時は新規private repositoryの入口、support、intake、contributing、security、release、lifecycle、governance、license、automation、ownership targetが未定義だったため、RepoSeiriのpatch holdを保持した。これは当時の状態であり、現在の各targetが未定義であると再判定した記録ではない。方針や文書を発明してholdを消さない。

日英の入口はREADME、docs/README、migration contract、adoption manifest、clone verification、known limitations、Assurance boundary、implementation statusへ接続されている。継承元の長文文書は歴史記録として扱い、Epistesysの新機能や外部保証へ昇格させない。

## English

### EPR candidate observation: not the final source

On 2026-10-27 JST, RepoSeiri `1.1.0+codex.20260823031336` ran its Rust core through the bundle launcher over a fixed source-only export using the `common` profile and **subtree scope**. The export source commit was `cfe29f89403f44ed1cd55552cf652c7c11cb3425`; the summary source-session digest was `sha256:4cd84e782284363ae1ec1b98b34f7950c9bc3cd827f5d5ae913a28611524a9d3`. This working export inherits a parent repository, so its subtree scope is not silently compared as the same scope as the historical repository-scope audit.

The `summary` and `linter` queries ran over the same fixed export without writes. This is an intermediate snapshot examining the next candidate from `6.3.2-alpha.2`, not certification of final alpha.3 source, release, or publication readiness. Do not transfer the following numbers unchanged to later fixes or updates to this document.

| Observation | Fixed-candidate result |
| --- | --- |
| Entries / document events / document diagnostics | 389 / 8,064 / 0 |
| Documents | 64 selected / 64 candidates; 918,038 bytes |
| Markdown / conflict coverage | `Complete` / `Complete` |
| Limit-exceeded observations / scopes | 0 / 0 |
| Repository observations | 52 present, 25 absent, 1 unknown, 0 conflict |
| Root README grammar nodes | 136 |
| Profile fit | 70/100, top profile `cli`; not a quality or performance score |
| Summary | 1 underclaim opportunity, 8 overclaim risks, 4 findings |
| Claim drafts / baseline unknown | 0 drafts, 8 baseline unknown |
| Patch operations / holds | 2 / 7; `writes_files=false` |
| Wording linter | 40 files, 3 generated surfaces, 2 findings, 10 suppressed boundary exceptions |

The two linter findings matched `guaranteed meaning preservation` and `correctness guarantee` in the “Not equivalent to” table column at lines 137 and 139 of `docs/system-architecture.md` in the fixed export. Both are negated claim-boundary descriptions; manual contextual review did not interpret them as affirmative guarantees. The machine result remains two findings, however: it was not rewritten as zero, nor used to dismiss other risks or holds wholesale.

`underclaim`, `overclaim`, `baseline unknown`, findings, and holds are distinct review states. Eight risks do not become eight implementation defects, and one underclaim opportunity does not establish performance. No patch suggestion was executed, and undefined policy, licensing, or support promises were not invented to remove holds.

### A separate final-snapshot check remains necessary

A final source export, queries and linter using the same scope/profile, and the corresponding pinned source-session digest are **follow-up work after this candidate observation**. This document does not preempt their success. Changed source, scope, or digest requires a new observation; do not display old candidate coverage, finding counts, or claim-draft states as final-version results. RepoSeiri output alone does not authorize or complete commit/push/merge, release, packaging, or host activation.

### EPI-05 history: 6.3.2-alpha.1

The following is initial-clone history, not a re-audit of current source. RepoSeiri `1.1.0+codex.20260823031336` audited the Epistesys clone at that time at repository scope with the `common` profile. The query and linter ran over the same content snapshot; `writes files` was false.

#### Historical observations

- Entries: 183; document events: 3,107; document diagnostics: 0.
- Documents: 29 selected / 29 candidates; selected bytes: 288,128.
- Markdown coverage: Complete. Conflict coverage: Complete.
- Repository observations: 30 present, 43 absent, 1 unknown, 0 conflict.
- Root README grammar nodes: 22. Profile fit score: 40/100 (top profile `ml`). This is a RepoSeiri profile-fit observation, not document or feature quality.
- Wording linter: 21 files, 3 generated surfaces, 0 findings.
- Patches: writes files false. One edit-existing preview, six skeleton review items, five manual decisions, and eleven held items; no patch suggestion was applied.

#### Historical interpretation boundary

`absent`, `unknown`, holds, profile fit, and claim-draft state do not automatically mean missing functionality, correctness, publication readiness, security, licensing, or support promises. At that time, RepoSeiri holds were retained because new-private-repository targets for entry, support, intake, contributing, security, release, lifecycle, governance, license, automation, and ownership were undefined. This describes the historical state; it is not a renewed judgment that each current target is undefined. Do not invent policy or documents to remove the holds.

The bilingual entry points connect README, docs/README, migration contract, adoption manifest, clone verification, known limitations, Assurance boundary, and implementation status. Long inherited documents remain historical records and do not promote into new Epistesys features or external guarantees.
