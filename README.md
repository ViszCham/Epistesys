# Epistesys

[日本語](#日本語) · [English](#english)

## 日本語

Epistesys（エピステシス）は、AIとの対話で整理した条件を、実装・検証・修復につなぐ支援ツールです。原文の条件、対応するコード、どの版で何を確かめたかを追いやすくします。相談では、守りたい条件と、まだ決まっていない点を分けて整理します。

**Beta-7.1**（`7.1.0-beta.1`） · [具体的な使い方](docs/coding-and-conversation-assistance.md#日本語) · [起動手順](docs/execution-guide.md#日本語) · [文書索引](docs/README.md#日本語)

<a id="coding-assistance-ja"></a>
<a id="conversation-assistance-ja"></a>

### 会話とコーディングの支援

| やりたいこと | Epistesysで確認すること |
| --- | --- |
| 仕様を相談する | Conversation Projectorで、依頼文の条件・例外・解釈が分かれる箇所を確認する。相談で決まった条件は、実装を頼むときにも明記する。 |
| 機能を追加してレビューする | Coding Projectorで、どのコードが要求に対応するか、ビルドが通るか、登録されたテストから公開APIを呼べるか、受入条件を確かめたかを追う。 |
| 修正後に作業を再開する | ソースの版と検証結果を照合し、変更後にやり直す検査を確認する。 |

要求の構造化や検証結果の更新など、**開発手順の一部をRustの型と状態遷移で実装**しています。解析できなかった条件や検証結果が足りない項目は、未確認として残ります。[公開APIの検証例](docs/coding-and-conversation-assistance.md#checks-ja) · [対応する言語と操作](docs/coding-and-conversation-assistance.md#languages-ja)

### 実行例

対応するWindows用実行ファイルを同梱したパッケージでは、ルートでPowerShellを開き、次の診断を実行できます。ソースから始める場合は、[開発用ビルドの手順](docs/execution-guide.md#不要キャッシュを作らない起動とturn境界)を参照してください。

```powershell
./scripts/run-epistesys.ps1 lc631-dgcl-run --prompt "Please test the package. Do not publish."
```

結果には、原文に対応する要求の候補、未解決事項、検証計画が含まれます。この例の「テストして」「公開しないで」は解析する依頼文です。実際にテストを実行する条件や修復・再開の手順は[実行ガイド](docs/execution-guide.md#日本語)、Codexでの選択と起動は[プラグイン接続契約](docs/plugin-host-integration.md#日本語)にあります。

<a id="制御対象とアーキテクチャ"></a>

### 主な機構

| 機構 | 役割 |
| --- | --- |
| Conversation／Coding Projector | 説明・設計相談と、実装・テスト・レビューの入口を分ける。 |
| DGCL・Program IR | 原文から読み取れた条件を要求の中間表現へ結び付け、要求別の検証計画を作る。 |
| EpiMapLoss・EpiRecompScore | 条件を固定した有限モデルで、損失・利得・コストを比較する。 |
| 証拠の更新・修復・再開 | 対象の版、実行した検査、残る確認を追い、変更後に再評価する。 |

型・データフロー・実行条件は[アーキテクチャ](docs/system-architecture.md#日本語)と[DGCL実行契約](docs/dgcl-operating-profile-and-closure.md#日本語)で確認できます。

### TLCXの実装と測定

[EpiMapLoss（写像保存損失）](docs/epimaploss-epirecompscore-formulas.md#map-ja)は、表現を変えたときに保存すべき性質がどれだけ失われたかを扱います。[EpiRecompScore（構造再編評価値）](docs/epimaploss-epirecompscore-formulas.md#score-ja)は、候補が何を組み替えたかと、その変更による利得・損失・コストを比較します。

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

各項の定義と計算例は[全40成分](docs/epimaploss-epirecompscore-formulas.md#catalog-ja)、[区間演算](docs/epimaploss-epirecompscore-formulas.md#kernels-ja)、[候補の選択と探索](docs/epimaploss-epirecompscore-formulas.md#policy-ja)にあります。未測定・比較不能を0で埋めません。測る量と値域が定義され、観測だけが不足する場合は、契約に従って区間として扱えます。得点だけで実行許可や完了を決めません。適用条件は[測定契約](docs/tlcx-tl-screative-measurement-contract-v1-2026-10-29.md)をご覧ください。

<a id="検証状態を読むための区別"></a>

### 検証状態の読み方

- **候補と検証結果**：候補は確認前の提案です。検証済みとするには、対象の版と条件に対応する証拠が必要です。
- **証拠と権限**：テスト成功や得点、hash一致は、操作の許可を発行しません。
- **出力確定と配信**：出力候補の確定、hostへの送信、受領は別の確認です。ここでの出力確定はGit commitとは異なります。

詳しい定義は[用語表](docs/system-architecture.md#terminology-ja)にあります。

<a id="開発で確かめたいこと"></a>

### 現在の位置付け

Beta-7.1の[検証記録](docs/epistesys-7.1.0-beta.1-release-notes-2026-11-06.md)には、WindowsとLinuxで確認した範囲、使用した機能設定、修復した失敗が記されています。実ホストでの受領確認は `PendingHostObservation`、独立した評価用データは `PendingNoCorpus` です。開発上の失敗をどれだけ減らせるかは未評価です。[既知の制約](docs/known-limitations.md) · [alpha.3保守ブランチ](https://github.com/ViszCham/Epistesys/tree/main-alpha.3)

### 前身システムの探索的評価：GB-CC75

前身のLabyrinth-Codexでは、ComplexConstraints由来の課題で、65問を等しく重み付けした平均適合度が74.38%から94.10%（+19.71ポイント）、167組の比較で全基準を満たした割合が39.52%から48.50%（+8.98ポイント）でした。差は丸め前の値から算出しています。

これは前身システムの後ろ向き・探索的な集計であり、現在のEpistesysの性能や、写像損失の因果的な効果を示すものではありません。[評価方法と限界](docs/research/gb-cc75-study.md) · [再計算用の集計](benchmarks/gb-cc75/2026-08-18/README.md)

### 文書の読み順

- **使う**　[会話・実装の具体例](docs/coding-and-conversation-assistance.md#日本語) → [実行ガイド](docs/execution-guide.md#日本語)
- **仕組みを調べる**　[アーキテクチャと用語](docs/system-architecture.md#日本語) → [検証・修復の実行条件](docs/dgcl-operating-profile-and-closure.md#日本語)
- **検証結果・履歴をたどる**　[文書索引](docs/README.md#日本語) → [移行契約](docs/migration-contract.md#日本語)

---

## English

Epistesys connects constraints clarified in AI conversations to implementation, validation and repair. It helps you trace the original conditions, corresponding code, and what was checked against which revision. In a discussion, it separates constraints to preserve from open questions.

**Beta-7.1** (`7.1.0-beta.1`) · [Examples](docs/coding-and-conversation-assistance.md#english) · [Run Epistesys](docs/execution-guide.md#english) · [Documentation index](docs/README.md#english)

<a id="coding-assistance-en"></a>
<a id="conversation-assistance-en"></a>

### Conversation and coding assistance

| What you want to do | What you can check |
| --- | --- |
| Discuss a specification | Use Conversation Projector to identify conditions, exceptions and ambiguities in the request. Include agreed constraints when you ask for an implementation. |
| Add and review a feature | Use Coding Projector to track which code addresses the requirement, whether it builds, whether a registered test calls the public API, and which acceptance conditions were checked. |
| Resume after a change | Match validation results to the source revision and identify which checks to repeat. |

It implements **parts of the development procedure as Rust types and state transitions**, including requirement structure and validation-evidence refresh. Unparsed conditions and items with missing evidence remain open. [Public API example](docs/coding-and-conversation-assistance.md#checks-en) · [Language and operation coverage](docs/coding-and-conversation-assistance.md#languages-en)

### Execution examples

In a package that includes the matching Windows binary, open PowerShell at the package root and run the diagnostic below. For a source checkout, follow the [development build instructions](docs/execution-guide.md#launch-prevention-and-turn-local-cache-boundaries).

```powershell
./scripts/run-epistesys.ps1 lc631-dgcl-run --prompt "Please test the package. Do not publish."
```

The result includes candidate requirements tied to the original text, unresolved items and a validation plan. The request to test the package and the instruction to keep it unpublished are inputs to this diagnostic. See the [execution guide](docs/execution-guide.md#english) for running tests, applying a repair or resuming work, and the [plugin integration contract](docs/plugin-host-integration.md#english) for selecting and invoking Epistesys in Codex.

<a id="control-scope-and-architecture"></a>

### Main mechanisms

| Mechanism | Role |
| --- | --- |
| Conversation / Coding Projector | Separate explanation and design discussions from implementation, tests and review. |
| DGCL and Program IR | Link conditions read from the original text to requirement IR and create per-requirement validation plans. |
| EpiMapLoss and EpiRecompScore | Compare losses, gains and costs within finite models under fixed conditions. |
| Evidence refresh, repair and resume | Track the relevant revision, checks performed and remaining checks, then reevaluate after changes. |

See the [architecture](docs/system-architecture.md#english) and [DGCL operating contract](docs/dgcl-operating-profile-and-closure.md#english) for types, dataflow and execution conditions.

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

The reference covers [all 40 components](docs/epimaploss-epirecompscore-formulas.md#catalog-en), [interval arithmetic](docs/epimaploss-epirecompscore-formulas.md#kernels-en) and [candidate selection and search](docs/epimaploss-epirecompscore-formulas.md#policy-en). Do not fill unmeasured or incomparable items with zero. When the quantity and range are defined but observations are missing, the contract may allow an interval. A score alone neither authorizes execution nor establishes completion. See the [measurement contract](docs/tlcx-tl-screative-measurement-contract-v1-2026-10-29.md) for applicability.

<a id="interpreting-verification-states"></a>

### Reading verification states

- **Candidates and validation results:** a candidate is a proposal awaiting checks. Validation requires evidence for the relevant revision and conditions.
- **Evidence and authority:** passing tests, scores and matching hashes do not issue operation permissions.
- **Finalization and delivery:** finalizing a candidate, sending it to a host and observing receipt are separate checks. Output finalization here is not a Git commit.

See the [glossary](docs/system-architecture.md#terminology-en) for detailed definitions.

<a id="what-we-want-to-check-in-development"></a>

### Current status

The [Beta-7.1 verification record](docs/epistesys-7.1.0-beta.1-release-notes-2026-11-06.md) lists the Windows and Linux checks, feature configurations and observed failures that were repaired. Real-host receipt remains `PendingHostObservation`. The independent evaluation corpus remains `PendingNoCorpus`. The effect on failures in everyday development has yet to be measured. [Known limitations](docs/known-limitations.md) · [alpha.3 maintenance branch](https://github.com/ViszCham/Epistesys/tree/main-alpha.3)

### Exploratory predecessor evaluation: GB-CC75

For the predecessor, Labyrinth-Codex, mean adherence on tasks derived from ComplexConstraints rose from 74.38% to 94.10% (+19.71 points) with 65 questions weighted equally. The share meeting every criterion went from 39.52% to 48.50% (+8.98 points) over 167 paired observations. Differences use unrounded values.

This is a retrospective exploratory summary for the predecessor, not evidence of current Epistesys performance or a causal effect of mapping loss. [Methods and limitations](docs/research/gb-cc75-study.md) · [Recomputable aggregates](benchmarks/gb-cc75/2026-08-18/README.md)

### Documentation routes

- **Use the tool** · [Conversation and coding examples](docs/coding-and-conversation-assistance.md#english) → [Execution guide](docs/execution-guide.md#english)
- **Inspect the implementation** · [Architecture and terminology](docs/system-architecture.md#english) → [Validation and repair conditions](docs/dgcl-operating-profile-and-closure.md#english)
- **Find verification records and history** · [Documentation index](docs/README.md#english) → [Migration contract](docs/migration-contract.md#english)
