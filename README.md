# Epistesys

## 日本語

Epistesys（エピステシス）は、コードの変更を元の要求と検証結果に結び付ける**開発支援ツール**です。自分の開発で「依頼した条件は残っているか」「追加した機能は実際に呼ばれるか」「何を確認して完了としたのか」を、後からコードとテストに戻って確かめるために作っています。

たとえば「既存の出力形式を変えずに機能を追加してほしい」という依頼では、ビルドが通ること、新機能が呼び出されること、従来の出力形式が保たれることは別々の確認です。Epistesysはモデルの重みを変更せず、その外側で元の要求、許可された操作、対象となるコードの版、検証結果、未確認の項目を対応づけます。具体的な使い方は[コーディング支援](#coding-assistance-ja)、現時点で何が動き、何が未検証かは以下の表で確認できます。

[使い方](docs/execution-guide.md) · [EpiMapLossの式](docs/epimaploss-epirecompscore-formulas.md#map-ja) · [EpiRecompScoreの式](docs/epimaploss-epirecompscore-formulas.md#score-ja) · [現在の検証範囲](docs/epistesys-6.9.0-beta.1-source-snapshot-2026-11-05.md)

### 現在の位置付け

| 項目 | 現在の状態 |
| --- | --- |
| バージョン | **Beta 6.9 — `6.9.0-beta.1`**（開発途中のソース保存版） |
| 実装の由来 | Labyrinth-Codex v6.3.1の選択source snapshotに、DGCLの構文解析・検証・修復経路を追加 |
| ソース検証 | 今回の状態は[Beta 6.9保存記録](docs/epistesys-6.9.0-beta.1-source-snapshot-2026-11-05.md)をご覧ください。全体監査とrelease gateは未完了です。[alpha.3検証記録](validation/epr-release-verification-2026-10-27.md)と[alpha.2の最終検証](validation/epia2-alpha2-release-verification-2026-10-02.md)は履歴であり、この版の合格結果には読み替えません |
| 配布 | source-only。モデルweight、秘密鍵、個人設定、未監査の配布binaryを同梱しない |
| ホスト接続 | adapter・receipt・replayの実装を含む。実ホストのcallback／delivery観測は`PendingHostObservation` |
| 開発上の効果 | Epistesys自体が作業の失敗をどれだけ減らすかは未検証です。独立評価に使う正解データは`PendingNoCorpus`です |

ソース経路の局所検証は、任意指示の意味的正確性、ハルシネーション抑制、長期対話の性能を実証するものではありません。Beta 6.9は、現在の実装を一度保存するための中間スナップショットです。完成・配布準備・host activationを示す版ではありません。Alpha.3は[独立した保守ブランチ](https://github.com/ViszCham/Epistesys/tree/main-alpha.3)に残し、全接続の監査と修復は後続のBeta-7.0に向けて続けます。

### TLCXで追加中の実装

現在のソースには、文脈の再構成、複数の有限論理による検査、変換ごとの四記録、再編候補の評価、バイナリ状態・journal、資源所有権、Coding・Media・hostへの接続作業があります。ソースと試験コードがある一方、まだ呼び出し経路につながっていない機能や未確定の試験もあります。[測定契約](docs/tlcx-tl-screative-measurement-contract-v1-2026-10-29.md)に適用条件、[実装ロードマップ](docs/epistesys-tlcx-contextual-multilogic-loss-creativity-roadmap-2026-10-28.md)に依存順と完了条件を記載しています。

[EpiMapLoss：写像保存損失](docs/epimaploss-epirecompscore-formulas.md#map-ja)は、表現を移したときに保存すべき性質がどれだけ失われたかを扱います。[EpiRecompScore：構造再編評価値](docs/epimaploss-epirecompscore-formulas.md#score-ja)は、候補が新しく組み替えたものと、その利得・損失・コストを比較します。

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

各記号、下位式、[全40成分の計算法](docs/epimaploss-epirecompscore-formulas.md#catalog-ja)、[区間演算](docs/epimaploss-epirecompscore-formulas.md#kernels-ja)、[選択・探索](docs/epimaploss-epirecompscore-formulas.md#policy-ja)は数式リファレンスをご覧ください。未測定・比較不能を0とはせず、高得点を実行許可や完了判定には使いません。


### 制御対象とアーキテクチャ

主要なデータフローは、**入力の保存 → 構文・要求の抽出 → 要求中間表現 → 射影の不整合記録 → 検証計画 → 証拠の再検証 → 出力候補の確定**です。実行権限の検証と実ホストへの送信・受領確認は、別の境界として扱います。

| 機構 | 現行の実装範囲 |
| --- | --- |
| 構文・要求解析 | CommonMark文書領域、Rust CST、pin済み日英Stanza dependency worker、予算付きEarley controlled instruction grammar |
| 要求の追跡 | 否定・条件・例外・時間・scope・参照・依存を、source revisionと要求別Program IRに結び付ける |
| 射影の検査 | TranslationLoss v3の`ProjectionDefectGraph`で、対応・欠落・矛盾・比較不能を保持する。幾何的提案は補助情報 |
| 権限・証拠 | principal、scope、revision、nonce、期限、parent、payload digestを持つreceiptを検証する |
| Coding検証・修復 | compiler check、runtime test、登録されたproduction APIの直接呼出し、acceptance testを別観測として扱い、限定ファイル修復と新snapshotの再検証を接続する |
| 仮説探索・GPU | 256のdistinct world × 8 projection＝2,048 evaluation rows。数値計算経路とCPU最終検証を分離する |
| Program Analysis・Media | Rust／Python／AssemblyのRPA型、媒体のsource・backend・model・license・method・budgetに束縛した候補と観測 |
| 出力・再開 | exact output digest、HostOutputReceipt v2、Stop hook、sink検査、checkpoint、append-only journal／replay |

**DGCL**はDeepGrammarからCodingの要求別完了判定までを接続する経路です。未知の条件の真偽や未対応文法を推測で補わず、`Unsupported`／`Unresolved`／`Ambiguous`として保持します。外部モデルを使わない構成と、設定済みStanza構成を区別します。

alpha.2は、実ファイル修復、非上書きbackup、最大4回を上限とするSDK repair coordinator、writer lock、ledgerと別管理の署名head、状態更新を伴う離散・連続表現の蒸留、明示的consumer view v1/v2を追加しています。alpha.2時点のRust workspaceは13 crateで、現在のTLCX開発sourceは16 crateです。修復・再開の事前条件と検証範囲は[DGCL実行契約](docs/dgcl-operating-profile-and-closure.md)、型・境界・継承機構の詳細は[システムアーキテクチャ](docs/system-architecture.md)に整理しています。

alpha.3はEPR監査に基づき、既存経路の次の境界を強化します。

- **解析・権限**：日本語解析の進行・byte span・共有作業予算と、元receiptに束縛したsource／scope／期限の検査。
- **実行・回復**：登録direct driverの接続証拠、共通process監督、handle基準のfile修復、private Media snapshot。
- **数値・配布**：数値／graphの入場検査、固定inventory配布、Rust変更を捕捉するWindows／Linux CI。

[変更一覧](docs/epistesys-6.3.2-alpha.3-release-notes-2026-10-27.md)と[境界契約](docs/epr-boundary-contracts.md)は、この版で追加された実装の範囲を示します。開発での効果や、実ホストでの有効化を示す記録ではありません。

<a id="coding-assistance-ja"></a>

### コーディング支援：要求追跡・検証・修復

コーディング支援は、Epistesysの主要な適用先の一つです。対応するprofileでは、要求、実装対象、呼び出し経路、検証結果を対応づけ、完了判定の根拠を追跡する制御経路を実装しています。対象とする問題には、要求の一部欠落、追加した機能の未接続、検証不足、変更前の証拠の再利用があります。

Epistesysを明示的に選択した場合、Coding Projectorが設計・実装・テスト・レビューの依頼を解析経路へ渡します。原文に結び付く要求と未解決事項をProgram IRに保持し、要求ごとの検証計画へ接続します。要求と実装対象の対応、コンパイラ検査、指定した実行時テスト、登録されたproduction APIの直接呼出し、受入試験を別々の検証材料として扱います。例えば、コンパイラ検査が成功しても、計画で要求した呼び出しの証拠がなければ、その接続のgapは残ります。このruntime観測は登録driverとpinしたassertionに限定し、API内部からexecutorまでの任意の呼出し追跡とは区別します。

修復経路は、呼び出し側が提供した候補を、許可された既存Rustファイルへ適用し、変更前の内容を保存して、変更後snapshotと新しい証拠から要求を再評価します。再開時にも現在のsource・権限・証拠を確認します。完了判定の範囲は、抽出・登録された要求と検証計画です。要求抽出の意味精度、一般的なCoding性能、実hostへの出力・配信は別の評価・観測対象です。[利用手順](docs/execution-guide.md)、[実行契約](docs/dgcl-operating-profile-and-closure.md)、[既知の制約](docs/known-limitations.md)で条件を確認できます。

### 検証状態を読むための区別

- **候補と検証済み状態**：`candidate`は生成・解析された候補です。`Validated`等の状態には、対象revisionに対応する所定の証拠が必要です。
- **証拠と権限**：テスト成功、低いloss、hash一致、schema適合は、操作の許可を生成しません。
- **出力確定と配信**：standalone candidateの確定、hostへの送信、sinkの受領、durable replayは別々に観測します。
- **個別の検証と一般的な効果**：build／test／CLIの成功は、調べた条件での結果です。一般性能、形式証明、完全な文法理解、host全体での強制を示すものではありません。

本書の「出力確定」はアプリケーション内のoutput commitmentを指し、Git commitや事実の真理性とは異なります。用語の操作的定義と検証項目は[アーキテクチャの用語表](docs/system-architecture.md#terminology-ja)、未観測範囲は[既知の制約](docs/known-limitations.md)を参照してください。

### 開発で確かめたいこと

| 開発上の関心 | 評価する失敗・指標 |
| --- | --- |
| 裏付けのない生成の確定 | unsupported claim、誤ったcommit、検出漏れ、回答coverage |
| 複雑指示の制約保持 | 条件別適合、全条件達成、例外・依存・意図の保持、回帰 |
| 長期対話の指示整合性 | 初期指示の脱落、状態drift、古い権限・証拠の誤使用 |
| 失敗の可観測性 | 不整合検出、適切な保留、過剰な保留、risk–coverage |
| 信頼性を維持できる運用範囲 | 入力長、条件数、依存深度、状態量、対話長、総計算予算ごとの信頼性 |

制約・権限・状態・証拠を明示的に残すことは**設計目標**です。それが実際に失敗を減らすか、どの程度複雑な依頼や長い対話まで役立つかは、別途確かめる必要があります。[設計仮説と評価項目](docs/research-hypotheses.md)には、同じモデル・課題・総予算で比較する方法を整理しています。[前向き評価案](docs/research/epistesys-prospective-evaluation-plan.md)は未実行・未登録の計画です。

### 前身システムの探索的評価：GB-CC75

2026-08-18の前身Labyrinth-Codexに関する、ComplexConstraints由来の75問を用いた後ろ向き・探索的な対応付き評価です。

| 集計単位 | Direct Grok | Grok＋Labyrinth | 差 |
| --- | ---: | ---: | ---: |
| 主要：65 QIDの等重み平均ルーブリック適合度 | 74.38% | 94.10% | +19.71ポイント |
| 記述：167対応ペアの全基準達成率 | 39.52%（66/167） | 48.50%（81/167） | +8.98ポイント |

主要差の記録されたQID bootstrap 95%区間は+12.20〜+27.83ポイントで、差は丸め前に計算しています。主要65 QIDでは32勝・20同点・13敗でした。選択は75→67→65 QID、225→171→167ペアです。平均適合度は完全遵守や正答率ではなく、区間は取得選択やJudgeの系統誤差を含みません。

これは**前身の限定観測**です。EpistesysのAlpha各版やBeta 6.9の改善、公式ComplexConstraintsスコア、独立した人手の正解データ、TLの因果効果、ハルシネーション削減、長期保持を実証したものではありません。経路ラベル等を非提示とするJudge設計ですが、Labyrinthを含む評価系の独立性は未確立です。既知の75問を新しいholdoutとして扱いません。

[前身の評価報告](docs/research/gb-cc75-study.md)・[方法と由来](docs/research/gb-cc75-methods-and-provenance.md)・[統計仕様](docs/research/gb-cc75-statistical-analysis.md)・[方法論監査](docs/research/gb-cc75-adversarial-audit.md)・[再計算可能な集計](benchmarks/gb-cc75/2026-08-18/README.md)を提供しています。

### 実行例

次は診断用の例です。解析結果から実行権限は発行されません。副作用を伴うverify／finalizeには明示的な`--execute`と、登録tool・外部receipt等の条件が必要です。

以下のlauncher例は対応する同梱binaryがある場合の入口です。現TLCX開発sourceでは、binary欠落時に暗黙buildを行いません。source-only checkoutからの明示開発build、共用targetの指定、turn-localキャッシュの寿命は[実行ガイド](docs/execution-guide.md#不要キャッシュを作らない起動とturn境界)で確認してください。これらの開発差分は、配布・installed plugin反映を意味しません。

```powershell
./scripts/run-epistesys.ps1 lc631-tl-doctor --prompt "Please test the package. Do not publish."
./scripts/run-epistesys.ps1 lc631-world-doctor --prompt "Please test the package. Do not publish."
./scripts/run-epistesys.ps1 lc631-doctor --repo .
```

launcherは自身の配置からrunnerを解決します。別cwdからの起動を含む手順は[実行ガイド](docs/execution-guide.md)、Epistesysを明示選択した場合のCodexのUTF-8 stdin／facadeは[ルーティング契約](docs/codex-default-routing.md)を参照してください。インストールと利用の選択は別の操作です。

### 文書の読み順

| 読者の関心 | 入口 |
| --- | --- |
| コーディング支援の利用場面と判定条件 | [概要](#coding-assistance-ja) → [実行ガイド](docs/execution-guide.md) → [DGCL実行契約](docs/dgcl-operating-profile-and-closure.md) |
| システム境界・データフロー・用語 | [システムアーキテクチャ](docs/system-architecture.md) |
| 実行条件・修復・再開 | [実行ガイド](docs/execution-guide.md) → [DGCL実行契約](docs/dgcl-operating-profile-and-closure.md) |
| wire形式・互換性 | [DGCL wire互換性](docs/dgcl-wire-compatibility.md) → [schema契約](schemas/closure-and-receipt-contract.md) |
| 指標の数式・適用条件 | [EpiMapLoss](docs/epimaploss-epirecompscore-formulas.md#map-ja) → [EpiRecompScore F1・F2](docs/epimaploss-epirecompscore-formulas.md#score-ja) → [全40成分](docs/epimaploss-epirecompscore-formulas.md#catalog-ja) |
| 設計の評価・前身の測定結果 | [設計仮説と評価項目](docs/research-hypotheses.md) → [GB-CC75の評価報告](docs/research/gb-cc75-study.md) |
| 現行の検証・残存条件 | [Beta 6.9保存記録](docs/epistesys-6.9.0-beta.1-source-snapshot-2026-11-05.md) → [既知の制約](docs/known-limitations.md) |
| sourceの由来・除外・履歴 | [移行契約](docs/migration-contract.md) → [文書索引](docs/README.md) |

初期cloneは独立したGit履歴で保存され、個人状態、credentials、cache、receipt root、replay ledgerを継承していません。`lc631-*`は挙動差を抑える互換識別子です。詳細な実装記録と継承元の設計・監査文書は[文書索引](docs/README.md)で現行仕様から区別しています。

---

## English

Epistesys is a **development tool** that connects a code change to its original requirements and validation results. I’m building it so I can return to the code and tests in my own projects and answer three questions: Did the requested constraints survive? Is the new behavior actually called? What was verified before the change was called complete?

For example, a request to “add a feature without changing the existing output format” needs three distinct checks: the build passes, the new feature is invoked, and the old format is preserved. Without changing model weights, Epistesys relates the original requirements, permitted operations, code revision, validation results, and outstanding checks. See [Coding assistance](#coding-assistance-en) for the workflow and the table below for what currently works and what remains unverified.

[How to use it](docs/execution-guide.md) · [EpiMapLoss equations](docs/epimaploss-epirecompscore-formulas.md#map-en) · [EpiRecompScore equations](docs/epimaploss-epirecompscore-formulas.md#score-en) · [Current verification scope](docs/epistesys-6.9.0-beta.1-source-snapshot-2026-11-05.md)

### Current status

| Item | Current state |
| --- | --- |
| Version | **Beta 6.9 — `6.9.0-beta.1`** (unfinished source checkpoint) |
| Implementation provenance | Selected Labyrinth-Codex v6.3.1 source snapshot with added DGCL parsing, validation, and repair paths |
| Source verification | See the [Beta 6.9 snapshot record](docs/epistesys-6.9.0-beta.1-source-snapshot-2026-11-05.md) for this checkpoint. Whole-system audit and release gates are incomplete. The [alpha.3 record](validation/epr-release-verification-2026-10-27.md) and [alpha.2 final verification](validation/epia2-alpha2-release-verification-2026-10-02.md) are historical, not passing results for this version |
| Distribution | Source-only; excludes model weights, private keys, personal settings, and unaudited distribution binaries |
| Host integration | Includes adapter/receipt/replay implementations; real-host callback/delivery observations remain `PendingHostObservation` |
| Effect on development work | How much Epistesys itself reduces development failures is unverified. The independent reference corpus remains `PendingNoCorpus` |

Local source-path verification does not establish arbitrary-instruction semantic accuracy, hallucination containment, or long-session performance. Beta 6.9 is an intermediate checkpoint of the current implementation, not completion, distribution readiness, or host activation. Alpha.3 remains on its [independent maintenance branch](https://github.com/ViszCham/Epistesys/tree/main-alpha.3), while end-to-end audit and repairs continue toward Beta-7.0.

### Implementation being added through TLCX

The current source contains context reconstruction, checks across several finite logics, four records per transformation, evaluation of recomposed candidates, binary state and journals, resource ownership, and Coding/Media/host integration work. There is source and test code, but some features still lack a caller and some tests are unresolved. The [measurement contract](docs/tlcx-tl-screative-measurement-contract-v1-2026-10-29.md) defines applicability; the [implementation roadmap](docs/epistesys-tlcx-contextual-multilogic-loss-creativity-roadmap-2026-10-28.md) gives dependency order and completion criteria.

[EpiMapLoss: mapping preservation loss](docs/epimaploss-epirecompscore-formulas.md#map-en) tracks what a change of representation fails to preserve. [EpiRecompScore: structural recomposition score](docs/epimaploss-epirecompscore-formulas.md#score-en) evaluates what a candidate reorganizes, together with its gains, losses, and costs.

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

See the formula reference for notation, subformulas, [all 40 component calculations](docs/epimaploss-epirecompscore-formulas.md#catalog-en), [interval arithmetic](docs/epimaploss-epirecompscore-formulas.md#kernels-en), and [selection and search](docs/epimaploss-epirecompscore-formulas.md#policy-en). Unmeasured or incomparable values are not zero, and high scores do not authorize execution or establish completion.


### Control scope and architecture

The principal dataflow is **source preservation → syntax/requirement extraction → requirement IR → projection-defect recording → validation planning → evidence revalidation → candidate finalization**. Execution-authority checks and real-host send/receipt observations are separate boundaries.

| Mechanism | Current implementation scope |
| --- | --- |
| Syntax and requirement parsing | CommonMark regions, Rust CST, pinned Japanese/English Stanza dependency workers, budgeted Earley controlled instruction grammar |
| Requirement traceability | Bind negation, conditions, exceptions, time, scope, references, and dependencies to source revision and per-requirement Program IR |
| Projection checks | TranslationLoss v3 `ProjectionDefectGraph` retains correspondence, omissions, contradictions, and incomparability; geometric proposals are advisory |
| Authority and evidence | Validate receipts containing principal, scope, revision, nonce, expiry, parent, and payload digest |
| Coding validation and repair | Separate compiler checks, runtime tests, registered direct production-API invocations, and acceptance observations; connect bounded file repair and revalidation of changed snapshots |
| Hypothesis exploration and GPU | 256 distinct worlds × 8 projections = 2,048 evaluation rows; separate numeric computation from CPU final validation |
| Program Analysis and Media | Rust/Python/Assembly RPA types; candidates/observations bound to source, backend, model, license, method, and budget |
| Output and resume | Exact output digests, HostOutputReceipt v2, Stop hook, sink checks, checkpoints, append-only journals/replay |

**DGCL** connects DeepGrammar to per-requirement Coding completion decisions. Unknown predicate truth and unsupported grammar remain `Unsupported`/`Unresolved`/`Ambiguous`, not guessed. No-external-model and configured-Stanza profiles are distinct.

Alpha.2 adds real file repair, non-overwriting backups, an SDK repair coordinator capped at four attempts, writer locks, signed heads managed separately from ledgers, state-changing discrete/continuous distillation, and explicit consumer views v1/v2. The alpha.2 Rust workspace contained 13 crates; current TLCX development source contains 16. See the [DGCL execution contract](docs/dgcl-operating-profile-and-closure.md) for repair/resume prerequisites and scope; the [system architecture](docs/system-architecture.md) details types, boundaries, and inherited mechanisms.

Following the EPR audit, alpha.3 strengthens these boundaries in existing paths:

- **Parsing and authority:** Japanese-parser progress, byte spans and shared work budgets; source/scope/expiry checks bound to original receipts.
- **Execution and recovery:** registered direct-driver connection evidence, shared process supervision, handle-based file repair, and private Media snapshots.
- **Numerics and distribution:** numeric/graph admission, fixed-inventory packaging, and Windows/Linux CI that captures Rust changes.

The [change notes](docs/epistesys-6.3.2-alpha.3-release-notes-2026-10-27.md) and [boundary contracts](docs/epr-boundary-contracts.md) describe the implementation added in that version. They do not establish its effect on development work or activation on a real host.

<a id="coding-assistance-en"></a>

### Coding assistance: requirement traceability, validation, and repair

Coding assistance is one of Epistesys's principal application areas. Within supported profiles, it implements a control path relating requirements, implementation targets, invocation paths, and validation results, making the basis for completion decisions traceable. The failure model includes omitted requirements, disconnected functionality, missing validation, and reuse of evidence collected before a change.

When Epistesys is explicitly selected, the Coding Projector routes design, implementation, testing, and review requests into the analysis path. Source-linked requirements and unresolved items are retained in Program IR and connected to per-requirement validation plans. Requirement-to-implementation bindings, compiler checks, specified runtime tests, registered direct production-API invocations, and acceptance tests are treated as distinct validation materials. For example, a successful compiler check still leaves a connection gap open when evidence for the invocation required by the plan is missing. This runtime observation is limited to a registered driver and pinned assertion, not arbitrary internal API-to-executor tracing.

The repair path applies a caller-provided candidate to an authorized existing Rust file, preserves its previous contents, and reevaluates requirements using the changed snapshot and fresh evidence. Resume also checks current source, authority, and evidence. Completion decisions cover the extracted and registered requirements and validation plans. Semantic accuracy of requirement extraction, general coding performance, and real-host output and delivery require separate evaluation or observation. See the [execution guide](docs/execution-guide.md), [execution contract](docs/dgcl-operating-profile-and-closure.md), and [known limitations](docs/known-limitations.md) for conditions.

### Interpreting verification states

- **Candidate versus validated state:** a `candidate` is a generated or parsed proposal. States such as `Validated` require the specified evidence for the relevant revision.
- **Evidence versus authority:** test success, low loss, matching hashes, and schema validity do not grant operation permissions.
- **Finalization versus delivery:** standalone candidate finalization, host send, sink receipt, and durable replay are separately observed.
- **Individual checks versus general effectiveness:** build/test/CLI success reports a result under the conditions tested. It does not establish general performance, formal proof, complete grammar understanding, or host-wide enforcement.

“Output commitment” here means application-level finalization, not a Git commit or truth of a claim. See the [operational terminology](docs/system-architecture.md#terminology-en) and [known limitations](docs/known-limitations.md).

### What we want to check in development

| Development concern | Failures/metrics to evaluate |
| --- | --- |
| Commitment of unsupported generation | Unsupported claims, erroneous commitments, missed detections, response coverage |
| Constraint preservation | Per-criterion and all-criteria adherence, exception/dependency/intent retention, regressions |
| Long-horizon instruction integrity | Initial-instruction omission, state drift, misuse of stale authority/evidence |
| Failure observability | Defect detection, appropriate and excessive holds, risk–coverage |
| Reliable operating range | Reliability by input length, condition count, dependency depth, state volume, session length, and total compute |

Explicitly retaining constraints, authority, state, and evidence is a **design objective**. Whether this reduces failures in practice, and how useful it remains as requests get more complex or sessions get longer, requires separate evaluation. The [design hypotheses and evaluation criteria](docs/research-hypotheses.md) describe how to compare runs using the same model, tasks, and total budget. The [prospective evaluation plan](docs/research/epistesys-prospective-evaluation-plan.md) has not been run or preregistered.

### Exploratory predecessor evaluation: GB-CC75

A retrospective exploratory paired evaluation of predecessor Labyrinth-Codex, dated 2026-08-18, using 75 tasks adapted from ComplexConstraints.

| Aggregation unit | Direct Grok | Grok + Labyrinth | Difference |
| --- | ---: | ---: | ---: |
| Primary: equal-weight mean rubric adherence over 65 QIDs | 74.38% | 94.10% | +19.71 points |
| Descriptive: all-criteria pass over 167 paired observations | 39.52% (66/167) | 48.50% (81/167) | +8.98 points |

The recorded QID-bootstrap 95% interval is +12.20 to +27.83 points; differences use unrounded values. The primary 65 QIDs show 32 wins, 20 ties, and 13 losses. Selection is 75→67→65 QIDs and 225→171→167 pairs. Mean adherence is neither complete compliance nor task accuracy; the interval excludes acquisition selection and systematic Judge error.

These are **bounded predecessor observations**. They do not establish improvements in the Epistesys alphas or Beta 6.9, official ComplexConstraints scores, independent human gold, causal TL effects, hallucination reduction, or long-session retention. The Judge design withholds route labels and other identifiers, but independence of the Labyrinth-based evaluator is unestablished. Known tasks are not a new holdout.

See the [predecessor evaluation report](docs/research/gb-cc75-study.md), [methods/provenance](docs/research/gb-cc75-methods-and-provenance.md), [statistical specification](docs/research/gb-cc75-statistical-analysis.md), [methodological audit](docs/research/gb-cc75-adversarial-audit.md), and [recomputable aggregates](benchmarks/gb-cc75/2026-08-18/README.md).

### Execution examples

These are diagnostics; parsing does not issue execution authority. Effectful verify/finalize commands require explicit `--execute` and conditions including registered tools and external receipts.

The launcher examples below require a matching packaged binary. Current TLCX development source never builds implicitly when that binary is missing. See the [execution guide](docs/execution-guide.md#launch-prevention-and-turn-local-cache-boundaries) for explicit source-checkout development builds, shared targets and turn-local cache lifetimes. These development changes do not establish distribution or installed-plugin pickup.

```powershell
./scripts/run-epistesys.ps1 lc631-tl-doctor --prompt "Please test the package. Do not publish."
./scripts/run-epistesys.ps1 lc631-world-doctor --prompt "Please test the package. Do not publish."
./scripts/run-epistesys.ps1 lc631-doctor --repo .
```

The launcher resolves its runner relative to its installation. See the [execution guide](docs/execution-guide.md) for startup including other working directories, and the [routing contract](docs/codex-default-routing.md) for UTF-8 stdin/facades when Epistesys is explicitly selected in Codex. Installation and selection for use are separate operations.

### Documentation routes

| Reader interest | Entry point |
| --- | --- |
| Coding use cases and decision conditions | [Overview](#coding-assistance-en) → [Execution guide](docs/execution-guide.md) → [DGCL execution contract](docs/dgcl-operating-profile-and-closure.md) |
| System boundaries, dataflow, terminology | [System architecture](docs/system-architecture.md) |
| Execution, repair, resume | [Execution guide](docs/execution-guide.md) → [DGCL execution contract](docs/dgcl-operating-profile-and-closure.md) |
| Wire formats and compatibility | [DGCL wire compatibility](docs/dgcl-wire-compatibility.md) → [schema contract](schemas/closure-and-receipt-contract.md) |
| Measurement equations and applicability | [EpiMapLoss](docs/epimaploss-epirecompscore-formulas.md#map-en) → [EpiRecompScore F1/F2](docs/epimaploss-epirecompscore-formulas.md#score-en) → [all 40 components](docs/epimaploss-epirecompscore-formulas.md#catalog-en) |
| Design evaluation and predecessor results | [Design hypotheses and evaluation criteria](docs/research-hypotheses.md) → [GB-CC75 evaluation report](docs/research/gb-cc75-study.md) |
| Current verification and residual conditions | [Beta 6.9 snapshot record](docs/epistesys-6.9.0-beta.1-source-snapshot-2026-11-05.md) → [Known limitations](docs/known-limitations.md) |
| Source provenance, exclusions, history | [Migration contract](docs/migration-contract.md) → [Documentation index](docs/README.md) |

The initial clone uses independent Git history and excludes personal state, credentials, caches, receipt roots, and replay ledgers. `lc631-*` names are compatibility identifiers retained to limit behavior changes. The [documentation index](docs/README.md) separates detailed implementation records and inherited design/audit history from the current specification.
