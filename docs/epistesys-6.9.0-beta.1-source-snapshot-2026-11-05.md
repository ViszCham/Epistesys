# Epistesys Beta 6.9 — ソース保存記録 / Source snapshot

## 日本語

記録日：2026年11月5日（JST）。表示名は **Epistesys Beta 6.9**、内部バージョンは `6.9.0-beta.1` です。

### この版の目的

Beta 6.9は、開発途中のTLCX実装を後から参照できるように残したソースのスナップショットです。ここでは、保存した内容、実行した検証、未確認の接続を区別します。全機能の完成、release gateの合格、配布準備、host activationを示す版ではありません。Alpha.3は[保守ブランチ `main-alpha.3`](https://github.com/ViszCham/Epistesys/tree/main-alpha.3)に残しています。

### 保存するもの

この保存版は、作者の開発で必要になった機構のソースと試験を残すものです。16-crate workspaceのsource・test・fixture・schema・launcher・CI定義には、型付きsource/span、文脈再構成、有限論理solver、四記録と指標の算術、バイナリ状態・journal・資源所有権、world／蒸留、Coding／Media／hostの接続作業が含まれます。

sourceや試験が存在することと、本番経路が最後まで接続・検証されたことは区別します。未登録module、test-first API、未閉鎖consumerも含まれます。

### この版が参照する測定式

[EpiMapLoss：写像保存損失](epimaploss-epirecompscore-formulas.md#map-ja)は、表現を移したときに保存すべき性質がどれだけ失われたかを扱います。[EpiRecompScore：構造再編評価値](epimaploss-epirecompscore-formulas.md#score-ja)は、候補が新しく組み替えたものと、その利得・損失・コストを比較します。

**主要式：写像保存損失、再編評価F1・F2**

右辺の各成分は同じ比較範囲の候補 $`z`$ に対する値です。適用軸 $`\mathcal A`$ と正の重み $`\rho_a`$ は比較前に固定します。

```math
\begin{aligned}
L=\operatorname{EpiMapLoss}(z)
 &=\frac{\sum_{a\in\mathcal A}\rho_a L_a(z)}{\sum_{a\in\mathcal A}\rho_a},
\\ \operatorname{EpiRecompScore}_{F1}(z)
 &=\alpha D_{\rm ctx}+\beta D_{\rm sem}-\lambda L
   +\gamma A_{\rm compat}+\delta C_{\rm match}-\eta N_{\rm known},
\\ \operatorname{EpiRecompScore}_{F2}(z)
 &=w_1T_{\rm nc}+w_2G_{\rm struct}+w_3R_{\rm gain}+w_4U_{\rm gain}
\\ &\quad+w_5Q_{\rm parity}-w_6L-w_7R_{\rm counter}-w_8B_{\rm mem}.
\end{aligned}
```

各記号、下位式、[全40成分の計算法](epimaploss-epirecompscore-formulas.md#catalog-ja)、[区間演算](epimaploss-epirecompscore-formulas.md#kernels-ja)、[選択・探索](epimaploss-epirecompscore-formulas.md#policy-ja)は数式リファレンスをご覧ください。未測定・比較不能を0とはせず、高得点を実行許可や完了判定には使いません。


ここでの公開名は、保存した数式を読みやすく示すためのものです。固定契約の記号・式ID、以下の検証結果、インストール済みプラグインの状態は変わりません。

### 未完了の主な範囲

- 全コードと仕様の全文照合、敵対的監査、その結果に基づく修復。
- 実行時刻、権限失効、部分実行結果の保持と、Coding／checkpoint／再開の全接続。
- 全測定成分、共同選択、文脈再構成、world／蒸留のproduction接続と独立した確認。
- 現snapshotの全feature／OS／backend試験、package検証、必要な実host観測。
- 独立した正解データを使った、開発上の効果の比較評価。未提供・未観測を成功や形式証明へ置き換えません。

### 今回の確認

| 確認 | この保存作業での状態 |
| --- | --- |
| Cargo／plugin identity | `6.9.0-beta.1`へ整合しています |
| Windows `cargo fmt --all --check` | OS error 206（コマンド長制限）で実行できませんでした。書式合格ではありません |
| WSLの同じformat確認 | 選択されたstable toolchainに`cargo-fmt`がなく、実行できませんでした |
| `cargo check --workspace --locked --offline -j 1` | 成功しました（16 crate、5分59秒）。実行試験やrelease readinessの証明ではありません |
| 同じ条件の `cargo check --workspace --all-targets` | 失敗しました。test-first試験の `goal_clock_domain` importが未解決です（E0432） |
| 全試験／strict Clippy／release／GPU／host | この保存作業での合格結果はありません |
| 公開集計・文書・設計revision | 保存時には公開集計の12 hash、11集計試験、17日英文書／342リンク、11設計revision試験が成功しました。開発上の効果を新たに測ったものではありません |
| Assurance-Compiler | 完全な小Rust単位`repair_verified_application.rs`への補助確認は`needs_evidence`、診断0件でした。未監査範囲と証拠不足は残ります |
| RepoSeiri | 公開対象だけの独立コピーでbounded監査を行い、個別結果と保留をPRに記録します。元の作業領域への初回走査は上限に達したため、包括的な監査結果として使いません |

Alpha.3の過去の合格結果やコード量100%は、この版の合格結果へ読み替えません。今回はplugin再インストール、host再起動、既存のinstalled packageの変更を行いません。

### 読み順と公開範囲

概要は[README](../README.md)、数式・適用条件は[測定契約](tlcx-tl-screative-measurement-contract-v1-2026-10-29.md)、依存順と完成条件は[ロードマップ](epistesys-tlcx-contextual-multilogic-loss-creativity-roadmap-2026-10-28.md)をご覧ください。既存資料は記載されたrevisionと適用範囲を保ち、最終完成は後続のBeta-7.0へ向けて進めます。

cache、モデルweight、鍵、実receipt root、replay/resume状態、内部作業台帳・引継ぎメモ・未適用patch、compiler生成の診断補助ファイルは公開対象に含めません。削除せず元の作業環境に保持します。公開に必要な設計契約とrevision chainは、歴史的な証跡と現在の完成判定を区別して保存します。

## English

Recorded on November 5, 2026 (JST). The display name is **Epistesys Beta 6.9**, with internal version `6.9.0-beta.1`.

### Purpose

Beta 6.9 preserves the in-progress TLCX source as a point of reference. This record distinguishes what was saved, which checks ran, and which connections remain unverified. It does not establish complete functionality, passing release gates, distribution readiness, or host activation. Alpha.3 remains on its [`main-alpha.3` maintenance branch](https://github.com/ViszCham/Epistesys/tree/main-alpha.3).

### What is preserved

This checkpoint preserves source and tests for mechanisms the author needed in development. The 16-crate workspace, fixtures, schemas, launchers, and CI definitions include typed source/spans, context reconstruction, finite-logic solvers, four records and measurement arithmetic, binary state/journals/resource ownership, worlds/distillation, and Coding/Media/host integration work.

Source or test presence is not end-to-end production connection and validation. Unregistered modules, test-first APIs, and unfinished consumers are included.

### Measurement equations used by this version

[EpiMapLoss: mapping preservation loss](epimaploss-epirecompscore-formulas.md#map-en) tracks what a change of representation fails to preserve. [EpiRecompScore: structural recomposition score](epimaploss-epirecompscore-formulas.md#score-en) evaluates what a candidate reorganizes, together with its gains, losses, and costs.

**Main equations: mapping preservation loss and recomposition profiles F1 and F2**

Components on the right refer to the same candidate $`z`$ and comparison scope. Fix the active axes $`\mathcal A`$ and positive weights $`\rho_a`$ before comparing.

```math
\begin{aligned}
L=\operatorname{EpiMapLoss}(z)
 &=\frac{\sum_{a\in\mathcal A}\rho_a L_a(z)}{\sum_{a\in\mathcal A}\rho_a},
\\ \operatorname{EpiRecompScore}_{F1}(z)
 &=\alpha D_{\rm ctx}+\beta D_{\rm sem}-\lambda L
   +\gamma A_{\rm compat}+\delta C_{\rm match}-\eta N_{\rm known},
\\ \operatorname{EpiRecompScore}_{F2}(z)
 &=w_1T_{\rm nc}+w_2G_{\rm struct}+w_3R_{\rm gain}+w_4U_{\rm gain}
\\ &\quad+w_5Q_{\rm parity}-w_6L-w_7R_{\rm counter}-w_8B_{\rm mem}.
\end{aligned}
```

See the formula reference for notation, subformulas, [all 40 component calculations](epimaploss-epirecompscore-formulas.md#catalog-en), [interval arithmetic](epimaploss-epirecompscore-formulas.md#kernels-en), and [selection and search](epimaploss-epirecompscore-formulas.md#policy-en). Unmeasured or incomparable values are not zero, and high scores do not authorize execution or establish completion.


The public names here make the saved formulas easier to find. They do not change the fixed contract's symbols or formula IDs, the verification results below, or the installed plugin.

### Main remaining work

- Full code/specification reading, conformance checks, adversarial review, and resulting repairs.
- Complete connections for execution time, authority revocation, retained partial effects, Coding, checkpoints, and resume.
- Production integration and independent checks of all measurement components, joint selection, context reconstruction, worlds, and distillation.
- All required feature/OS/backend tests, package validation, and real-host observations for the current snapshot.
- Comparative evaluation of effects on development work using independent reference data. Missing or unobserved states do not become success or formal proof.

### Checks for this snapshot

| Check | State in this preservation task |
| --- | --- |
| Cargo/plugin identity | Aligned to `6.9.0-beta.1` |
| Windows `cargo fmt --all --check` | Could not run because of OS error 206 (command-length limit); not a formatting pass |
| The same format check in WSL | Could not run because the selected stable toolchain lacks `cargo-fmt` |
| `cargo check --workspace --locked --offline -j 1` | Passed for 16 crates in 5m59s; not execution-test or release-readiness evidence |
| `cargo check --workspace --all-targets` with the same settings | Failed: unresolved `goal_clock_domain` imports in a test-first target (E0432) |
| Full tests/strict Clippy/release/GPU/host | No passing result from this preservation task |
| Public aggregates, documentation, and design revisions | At snapshot time, 12 public-aggregate hashes, 11 aggregate tests, 17 bilingual documents/342 links, and 11 design-revision tests passed. These were not new measurements of development outcomes |
| Assurance-Compiler | Bounded review of the complete small Rust unit `repair_verified_application.rs` returned `needs_evidence` with 0 diagnostics. Evidence gaps and unreviewed scope remain |
| RepoSeiri | A bounded audit of an independent copy containing only the publication scope records individual results and holds in the PR. The initial scan of the original working area reached its traversal limit and is not used as a comprehensive audit |

Historical Alpha.3 passes and 100% written quantity are not passing results for this version. This task does not reinstall plugins, restart the host, or change the existing installed package.

### Reading order and public scope

Start with the [README](../README.md), then the [measurement contract](tlcx-tl-screative-measurement-contract-v1-2026-10-29.md) for formulas/applicability and the [roadmap](epistesys-tlcx-contextual-multilogic-loss-creativity-roadmap-2026-10-28.md) for dependencies/completion criteria. Existing material retains its stated revision and scope; final completion continues toward Beta-7.0.

Caches, model weights, keys, actual receipt roots, replay/resume state, internal working ledgers/handoff notes/unapplied patches, and compiler-generated diagnostic sidecars are excluded from publication. They are preserved locally, not deleted. Design contracts and revision chains needed for public inspection are retained while separating historical evidence from current completion.

