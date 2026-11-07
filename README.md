# Epistesys

[日本語](#日本語) · [English](#english)

## 日本語

Epistesys（エピステシス）は、AIとの対話とコーディングを支援するツールです。相談では「何を守りたいか」「何がまだ決まっていないか」を原文に沿って整理します。コードを変更するときは、元の要求と検証結果を結び付けて「どの版で、何を確かめたか」を追えます。

**Beta-7.1**（`7.1.0-beta.1`） · [具体的な使い方](docs/coding-and-conversation-assistance.md#日本語) · [起動手順](docs/execution-guide.md#日本語) · [文書索引](docs/README.md#日本語)

<a id="coding-assistance-ja"></a>
<a id="conversation-assistance-ja"></a>

### 会話とコーディングの支援

| やりたいこと | Epistesysで確認すること |
| --- | --- |
| 仕様を相談する | Conversation Projectorで、依頼文の条件・例外・解釈が分かれる箇所を確認する。相談で決まった条件は、実装を頼むときにも明記する。 |
| 機能を追加してレビューする | Coding Projectorで、どのコードが要求に対応するか、ビルドが通るか、登録されたテストから公開APIを呼べるか、受入条件を確かめたかを追う。 |
| 修正後に作業を再開する | ソースの版と検証結果を照合し、変更後にやり直す検査を確認する。 |

要求を構造化し、確認項目を作り、変更後に証拠を更新する手順をRustの型と状態遷移で実装しています。解析できなかった条件や検証結果が足りない項目は、未確認として残ります。[公開APIの検証例](docs/coding-and-conversation-assistance.md#checks-ja) · [対応する言語と操作](docs/coding-and-conversation-assistance.md#languages-ja)

### 実行例

対応するWindows用実行ファイルを同梱したパッケージでは、ルートでPowerShellを開き、次の診断を実行できます。ソースから始める場合は、[開発用ビルドの手順](docs/execution-guide.md#不要キャッシュを作らない起動とturn境界)を参照してください。

```powershell
./scripts/run-epistesys.ps1 lc631-dgcl-run --prompt "Please test the package. Do not publish."
```

結果には、原文に対応する要求の候補、未解決事項、検証計画が含まれます。この例の「テストして」「公開しないで」は解析する依頼文です。実際にテストを実行する条件や修復・再開の手順は[実行ガイド](docs/execution-guide.md#日本語)、Codexでの選択と起動は[プラグイン接続契約](docs/plugin-host-integration.md#日本語)にあります。

### TLCXの実装と測定

[EpiMapLoss（写像保存損失）](docs/epimaploss-epirecompscore-formulas.md#map-ja)は、表現を変えたときに保存すべき性質がどれだけ失われたかを扱います。[EpiRecompScore（構造再編評価値）](docs/epimaploss-epirecompscore-formulas.md#score-ja)は、候補が何を組み替え、その変更で何を得て、何を失ったかを比較します。

同じ条件で比べる候補を $`z`$ とし、適用する軸 $`\mathcal A`$ と正の重み $`\rho_a`$ を先に固定します。

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

各項の定義と計算例は[全40成分](docs/epimaploss-epirecompscore-formulas.md#catalog-ja)、[区間演算](docs/epimaploss-epirecompscore-formulas.md#kernels-ja)、[候補の選択と探索](docs/epimaploss-epirecompscore-formulas.md#policy-ja)にあります。未測定の項目には数値を割り当てません。操作の許可と完了判定には、それぞれ権限と検証結果を確認します。数値を使える条件は[測定契約](docs/tlcx-tl-screative-measurement-contract-v1-2026-10-29.md)に定めています。

### 現在の位置付け

Beta-7.1の[検証記録](docs/epistesys-7.1.0-beta.1-release-notes-2026-11-06.md)には、WindowsとLinuxで確認した範囲、使用した機能設定、修復した失敗が記されています。実ホストでの受領確認は `PendingHostObservation`、独立した評価用データは `PendingNoCorpus` です。開発上の失敗をどれだけ減らせるかは未評価です。[既知の制約](docs/known-limitations.md) · [alpha.3保守ブランチ](https://github.com/ViszCham/Epistesys/tree/main-alpha.3)

### 前身システムの探索的評価：GB-CC75

前身のLabyrinth-Codexでは、ComplexConstraints由来の課題で、65問を等しく重み付けした平均適合度が74.38%から94.10%（+19.71ポイント）、167組の比較で全基準を満たした割合が39.52%から48.50%（+8.98ポイント）でした。これは当時の課題と評価器に限った結果です。[評価方法と限界](docs/research/gb-cc75-study.md) · [再計算用の集計](benchmarks/gb-cc75/2026-08-18/README.md)

### 文書の読み順

- **使う**　[会話・実装の具体例](docs/coding-and-conversation-assistance.md#日本語) → [実行ガイド](docs/execution-guide.md#日本語)
- **仕組みを調べる**　[アーキテクチャと用語](docs/system-architecture.md#日本語) → [検証・修復の実行条件](docs/dgcl-operating-profile-and-closure.md#日本語)
- **検証結果・履歴をたどる**　[文書索引](docs/README.md#日本語) → [移行契約](docs/migration-contract.md#日本語)

---

## English

Epistesys assists with AI conversations and coding. In a discussion, it helps you refer back to the original wording to identify constraints and open questions. When changing code, it links requirements to validation results so you can see what was checked and against which revision.

**Beta-7.1** (`7.1.0-beta.1`) · [Examples](docs/coding-and-conversation-assistance.md#english) · [Run Epistesys](docs/execution-guide.md#english) · [Documentation index](docs/README.md#english)

<a id="coding-assistance-en"></a>
<a id="conversation-assistance-en"></a>

### Conversation and coding assistance

| What you want to do | What you can check |
| --- | --- |
| Discuss a specification | Use Conversation Projector to identify conditions, exceptions and ambiguities in the request. Include agreed constraints when you ask for an implementation. |
| Add and review a feature | Use Coding Projector to track which code addresses the requirement, whether it builds, whether a registered test calls the public API, and which acceptance conditions were checked. |
| Resume after a change | Match validation results to the source revision and identify which checks to repeat. |

Rust types and state transitions implement the steps for representing requirements, planning checks and refreshing evidence after a change. Unparsed conditions and items with missing evidence remain open. [Public API example](docs/coding-and-conversation-assistance.md#checks-en) · [Language and operation coverage](docs/coding-and-conversation-assistance.md#languages-en)

### Execution examples

In a package that includes the matching Windows binary, open PowerShell at the package root and run the diagnostic below. For a source checkout, follow the [development build instructions](docs/execution-guide.md#launch-prevention-and-turn-local-cache-boundaries).

```powershell
./scripts/run-epistesys.ps1 lc631-dgcl-run --prompt "Please test the package. Do not publish."
```

The result includes candidate requirements tied to the original text, unresolved items and a validation plan. The request to test the package and the instruction to keep it unpublished are inputs to this diagnostic. See the [execution guide](docs/execution-guide.md#english) for running tests, applying a repair or resuming work, and the [plugin integration contract](docs/plugin-host-integration.md#english) for selecting and invoking Epistesys in Codex.

### TLCX implementation and measures

[EpiMapLoss (mapping preservation loss)](docs/epimaploss-epirecompscore-formulas.md#map-en) measures how much of a required property is lost when its representation changes. [EpiRecompScore (structural recomposition score)](docs/epimaploss-epirecompscore-formulas.md#score-en) compares what a candidate reorganizes and what that change gains, loses and costs.

For candidates $`z`$ compared under the same conditions, fix the applicable axes $`\mathcal A`$ and positive weights $`\rho_a`$ first.

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

The reference covers [all 40 components](docs/epimaploss-epirecompscore-formulas.md#catalog-en), [interval arithmetic](docs/epimaploss-epirecompscore-formulas.md#kernels-en) and [candidate selection and search](docs/epimaploss-epirecompscore-formulas.md#policy-en). Unmeasured items have no numeric value. Operations require explicit authority, and completion requires validation evidence. The [measurement contract](docs/tlcx-tl-screative-measurement-contract-v1-2026-10-29.md) defines when each value applies.

### Current status

The [Beta-7.1 verification record](docs/epistesys-7.1.0-beta.1-release-notes-2026-11-06.md) lists the Windows and Linux checks, feature configurations and observed failures that were repaired. Real-host receipt remains `PendingHostObservation`. The independent evaluation corpus remains `PendingNoCorpus`. The effect on failures in everyday development has yet to be measured. [Known limitations](docs/known-limitations.md) · [alpha.3 maintenance branch](https://github.com/ViszCham/Epistesys/tree/main-alpha.3)

### Exploratory predecessor evaluation: GB-CC75

For the predecessor, Labyrinth-Codex, mean adherence on tasks derived from ComplexConstraints rose from 74.38% to 94.10% (+19.71 points) with 65 questions weighted equally. The share meeting every criterion went from 39.52% to 48.50% (+8.98 points) over 167 paired observations. These results apply to the tasks and evaluator used at the time. [Methods and limitations](docs/research/gb-cc75-study.md) · [Recomputable aggregates](benchmarks/gb-cc75/2026-08-18/README.md)

### Documentation routes

- **Use the tool** · [Conversation and coding examples](docs/coding-and-conversation-assistance.md#english) → [Execution guide](docs/execution-guide.md#english)
- **Inspect the implementation** · [Architecture and terminology](docs/system-architecture.md#english) → [Validation and repair conditions](docs/dgcl-operating-profile-and-closure.md#english)
- **Find verification records and history** · [Documentation index](docs/README.md#english) → [Migration contract](docs/migration-contract.md#english)
