# Epistesys

## 日本語

Epistesys（エピステシス）は、AIシステムの**要求管理、実行権限、証拠の由来、状態更新、出力検証**を一つの処理系として扱う、実験的なAI制御・信頼性研究プロジェクトです。生成モデルを改変するのではなく、その外部に構造化された制御状態と検証手続きを配置します。

対象とする問題は、裏付けのない生成の確定、複雑な指示に含まれる制約の脱落、長期対話での状態・権限・証拠の失効、未解決事項を残したままの実行です。研究上の評価対象は、所定の信頼性を維持できる**指示複雑性・依存深度・状態量・対話長の運用範囲**です。以下では、実装された機構、その局所検証、今後検証する効果を区別します。

### 現在の位置付け

| 項目 | 現在の状態 |
| --- | --- |
| バージョン | `6.3.2-alpha.2` |
| 実装の由来 | Labyrinth-Codex v6.3.1の選択source snapshotに、DGCLの構文解析・検証・修復経路を追加 |
| ソース検証 | 宣言された実行条件に対する`SourceReleaseReady`。条件と記録は[最終検証](validation/epia2-alpha2-release-verification-2026-10-02.md)に記載 |
| 配布 | source-only。モデルweight、秘密鍵、個人設定、未監査の配布binaryを同梱しない |
| ホスト接続 | adapter・receipt・replayの実装を含む。実ホストのcallback／delivery観測は`PendingHostObservation` |
| 研究評価 | Epistesys固有の効果と独立評価は未実施。独立gold corpusは`PendingNoCorpus` |

ソース経路の局所検証は、任意指示の意味的正確性、ハルシネーション抑制、長期対話の性能を実証するものではありません。現在のalphaは長期研究の基盤実装であり、研究目標の達成版ではありません。Epistesys-7はこの版の範囲外です。

### 制御対象とアーキテクチャ

主要なデータフローは、**入力の保存 → 構文・要求の抽出 → 要求中間表現 → 射影の不整合記録 → 検証計画 → 証拠の再検証 → 出力候補の確定**です。実行権限の検証と実ホストへの送信・受領確認は、別の境界として扱います。

| 機構 | 現行の実装範囲 |
| --- | --- |
| 構文・要求解析 | CommonMark文書領域、Rust CST、pin済み日英Stanza dependency worker、予算付きEarley controlled instruction grammar |
| 要求の追跡 | 否定・条件・例外・時間・scope・参照・依存を、source revisionと要求別Program IRに結び付ける |
| 射影の検査 | TranslationLoss v3の`ProjectionDefectGraph`で、対応・欠落・矛盾・比較不能を保持する。幾何的提案は補助情報 |
| 権限・証拠 | principal、scope、revision、nonce、期限、parent、payload digestを持つreceiptを検証する |
| Coding検証・修復 | compiler check、runtime test、production接続、acceptance testを別観測として扱い、限定ファイル修復と新snapshotの再検証を接続する |
| 仮説探索・GPU | 256のdistinct world × 8 projection＝2,048 evaluation rows。数値計算経路とCPU最終検証を分離する |
| Program Analysis・Media | Rust／Python／AssemblyのRPA型、媒体のsource・backend・model・license・method・budgetに束縛した候補と観測 |
| 出力・再開 | exact output digest、HostOutputReceipt v2、Stop hook、sink検査、checkpoint、append-only journal／replay |

**DGCL**はDeepGrammarからCodingの要求別完了判定までを接続する経路です。未知の条件の真偽や未対応文法を推測で補わず、`Unsupported`／`Unresolved`／`Ambiguous`として保持します。外部モデルを使わない構成と、設定済みStanza構成を区別します。

alpha.2は、実ファイル修復、非上書きbackup、最大4回のrepair coordinator、writer lock、ledgerと別管理の署名head、状態更新を伴う離散・連続表現の蒸留、明示的consumer view v1/v2を追加しています。Rust workspaceは13 crateです。修復・再開の事前条件と検証範囲は[DGCL実行契約](docs/dgcl-operating-profile-and-closure.md)、型・境界・継承機構の詳細は[システムアーキテクチャ](docs/system-architecture.md)に整理しています。

### 検証状態を読むための区別

- **候補と検証済み状態**：`candidate`は生成・解析された候補です。`Validated`等の状態には、対象revisionに対応する所定の証拠が必要です。
- **証拠と権限**：テスト成功、低いloss、hash一致、schema適合は、操作の許可を生成しません。
- **出力確定と配信**：standalone candidateの確定、hostへの送信、sinkの受領、durable replayは別々に観測します。
- **実装検証と研究効果**：build／test／CLIの成功から、一般性能、形式証明、完全な文法理解、host全体の強制を推論しません。

本書の「出力確定」はアプリケーション内のoutput commitmentを指し、Git commitや事実の真理性とは異なります。用語の操作的定義と検証項目は[アーキテクチャの用語表](docs/system-architecture.md#terminology-ja)、未観測範囲は[既知の制約](docs/known-limitations.md)を参照してください。

### 研究課題と評価方針

| 研究課題 | 評価する失敗・指標 |
| --- | --- |
| 裏付けのない生成の確定 | unsupported claim、誤ったcommit、検出漏れ、回答coverage |
| 複雑指示の制約保持 | 条件別適合、全条件達成、例外・依存・意図の保持、回帰 |
| 長期対話の指示整合性 | 初期指示の脱落、状態drift、古い権限・証拠の誤使用 |
| 失敗の可観測性 | 不整合検出、適切な保留、過剰な保留、risk–coverage |
| 信頼性を維持できる運用範囲 | 入力長、条件数、依存深度、状態量、対話長、総計算予算ごとの信頼性 |

これらは**研究目標**です。制約・権限・状態・証拠の明示保持は**設計目標**であり、その機構が同じモデル・課題・総予算で失敗を減らすかは**検証対象の仮説**です。モデル、system instruction、tool、出力上限、retry、controller costを含む比較条件を記録します。[研究仮説と評価方針](docs/research-hypotheses.md)に測定項目、[前向き評価案](docs/research/epistesys-prospective-evaluation-plan.md)に未実行・未登録の計画を記載しています。

### 前身システムの探索的評価：GB-CC75

2026-08-18の前身Labyrinth-Codexに関する、ComplexConstraints由来の75問を用いた後ろ向き・探索的な対応付き評価です。

| 集計単位 | Direct Grok | Grok＋Labyrinth | 差 |
| --- | ---: | ---: | ---: |
| 主要：65 QIDの等重み平均ルーブリック適合度 | 74.38% | 94.10% | +19.71ポイント |
| 記述：167対応ペアの全基準達成率 | 39.52%（66/167） | 48.50%（81/167） | +8.98ポイント |

主要差の記録されたQID bootstrap 95%区間は+12.20〜+27.83ポイントで、差は丸め前に計算しています。主要65 QIDでは32勝・20同点・13敗でした。選択は75→67→65 QID、225→171→167ペアです。平均適合度は完全遵守や正答率ではなく、区間は取得選択やJudgeの系統誤差を含みません。

これは**前身の限定観測**で、Epistesys alpha.2の改善、公式ComplexConstraintsスコア、独立human gold、TLの因果効果、ハルシネーション削減、長期保持の実証ではありません。経路ラベル等を非提示とするJudge設計ですが、Labyrinthを含む評価系の独立性は未確立です。既知の75問を新しいholdoutとして扱いません。

[研究本文](docs/research/gb-cc75-study.md)・[方法と由来](docs/research/gb-cc75-methods-and-provenance.md)・[統計仕様](docs/research/gb-cc75-statistical-analysis.md)・[方法論監査](docs/research/gb-cc75-adversarial-audit.md)・[再計算可能な集計](benchmarks/gb-cc75/2026-08-18/README.md)を提供しています。

### 実行例

次は診断用の例です。解析結果から実行権限は発行されません。副作用を伴うverify／finalizeには明示的な`--execute`と、登録tool・外部receipt等の条件が必要です。

```powershell
./scripts/run-epistesys.ps1 lc631-tl-doctor --prompt "Please test the package. Do not publish."
./scripts/run-epistesys.ps1 lc631-world-doctor --prompt "Please test the package. Do not publish."
./scripts/run-epistesys.ps1 lc631-doctor --repo .
```

launcherは自身の配置からrunnerを解決します。別cwdからの起動を含む手順は[実行ガイド](docs/execution-guide.md)、CodexのUTF-8 stdin／既定facadeは[ルーティング契約](docs/codex-default-routing.md)を参照してください。既定設定はhost全体への強制とは異なります。

### 文書の読み順

| 読者の関心 | 入口 |
| --- | --- |
| システム境界・データフロー・用語 | [システムアーキテクチャ](docs/system-architecture.md) |
| 実行条件・修復・再開 | [実行ガイド](docs/execution-guide.md) → [DGCL実行契約](docs/dgcl-operating-profile-and-closure.md) |
| wire形式・互換性 | [DGCL wire互換性](docs/dgcl-wire-compatibility.md) → [schema契約](schemas/closure-and-receipt-contract.md) |
| 評価設計・経験的結果 | [研究仮説](docs/research-hypotheses.md) → [GB-CC75研究本文](docs/research/gb-cc75-study.md) |
| 現行の検証・残存条件 | [alpha.2最終検証](validation/epia2-alpha2-release-verification-2026-10-02.md) → [既知の制約](docs/known-limitations.md) |
| sourceの由来・除外・履歴 | [移行契約](docs/migration-contract.md) → [文書索引](docs/README.md) |

初期cloneは独立したGit履歴で保存され、個人状態、credentials、cache、receipt root、replay ledgerを継承していません。`lc631-*`は挙動差を抑える互換識別子です。詳細な実装記録と継承元の設計・監査文書は[文書索引](docs/README.md)で現行仕様から区別しています。

---

## English

Epistesys is an experimental **AI control and reliability research project** that treats requirements, execution authority, evidence provenance, state updates, and output validation as one processing system. It places structured control state and verification procedures outside a generative model rather than modifying that model.

The failure model includes commitment of unsupported generation, omitted constraints in complex instructions, expired or drifting state/authority/evidence during long interactions, and execution with unresolved obligations. Its research target is the **operating range of instruction complexity, dependency depth, retained state, and session length at a defined reliability level**. Implemented mechanisms, local verification, and effects awaiting evaluation are distinguished below.

### Current status

| Item | Current state |
| --- | --- |
| Version | `6.3.2-alpha.2` |
| Implementation provenance | Selected Labyrinth-Codex v6.3.1 source snapshot with added DGCL parsing, validation, and repair paths |
| Source verification | `SourceReleaseReady` for declared operating conditions; conditions and records are in the [final verification](validation/epia2-alpha2-release-verification-2026-10-02.md) |
| Distribution | Source-only; excludes model weights, private keys, personal settings, and unaudited distribution binaries |
| Host integration | Includes adapter/receipt/replay implementations; real-host callback/delivery observations remain `PendingHostObservation` |
| Research evaluation | Epistesys-specific effects and independent evaluation are pending; independent gold corpus remains `PendingNoCorpus` |

Local source-path verification does not establish arbitrary-instruction semantic accuracy, hallucination containment, or long-session performance. The alpha is a baseline for long-term research, not completion of the research objectives. Epistesys-7 is outside this version.

### Control scope and architecture

The principal dataflow is **source preservation → syntax/requirement extraction → requirement IR → projection-defect recording → validation planning → evidence revalidation → candidate finalization**. Execution-authority checks and real-host send/receipt observations are separate boundaries.

| Mechanism | Current implementation scope |
| --- | --- |
| Syntax and requirement parsing | CommonMark regions, Rust CST, pinned Japanese/English Stanza dependency workers, budgeted Earley controlled instruction grammar |
| Requirement traceability | Bind negation, conditions, exceptions, time, scope, references, and dependencies to source revision and per-requirement Program IR |
| Projection checks | TranslationLoss v3 `ProjectionDefectGraph` retains correspondence, omissions, contradictions, and incomparability; geometric proposals are advisory |
| Authority and evidence | Validate receipts containing principal, scope, revision, nonce, expiry, parent, and payload digest |
| Coding validation and repair | Separate compiler checks, runtime tests, production connections, and acceptance observations; connect bounded file repair and revalidation of changed snapshots |
| Hypothesis exploration and GPU | 256 distinct worlds × 8 projections = 2,048 evaluation rows; separate numeric computation from CPU final validation |
| Program Analysis and Media | Rust/Python/Assembly RPA types; candidates/observations bound to source, backend, model, license, method, and budget |
| Output and resume | Exact output digests, HostOutputReceipt v2, Stop hook, sink checks, checkpoints, append-only journals/replay |

**DGCL** connects DeepGrammar to per-requirement Coding completion decisions. Unknown predicate truth and unsupported grammar remain `Unsupported`/`Unresolved`/`Ambiguous`, not guessed. No-external-model and configured-Stanza profiles are distinct.

Alpha.2 adds real file repair, non-overwriting backups, a four-attempt repair coordinator, writer locks, signed heads managed separately from ledgers, state-changing discrete/continuous distillation, and explicit consumer views v1/v2. The Rust workspace contains 13 crates. See the [DGCL execution contract](docs/dgcl-operating-profile-and-closure.md) for repair/resume prerequisites and scope; the [system architecture](docs/system-architecture.md) details types, boundaries, and inherited mechanisms.

### Interpreting verification states

- **Candidate versus validated state:** a `candidate` is a generated or parsed proposal. States such as `Validated` require the specified evidence for the relevant revision.
- **Evidence versus authority:** test success, low loss, matching hashes, and schema validity do not grant operation permissions.
- **Finalization versus delivery:** standalone candidate finalization, host send, sink receipt, and durable replay are separately observed.
- **Implementation verification versus research effects:** build/test/CLI success does not establish general performance, formal proof, complete grammar understanding, or host-wide enforcement.

“Output commitment” here means application-level finalization, not a Git commit or truth of a claim. See the [operational terminology](docs/system-architecture.md#terminology-en) and [known limitations](docs/known-limitations.md).

### Research questions and evaluation policy

| Research question | Failures/metrics to evaluate |
| --- | --- |
| Commitment of unsupported generation | Unsupported claims, erroneous commitments, missed detections, response coverage |
| Constraint preservation | Per-criterion and all-criteria adherence, exception/dependency/intent retention, regressions |
| Long-horizon instruction integrity | Initial-instruction omission, state drift, misuse of stale authority/evidence |
| Failure observability | Defect detection, appropriate and excessive holds, risk–coverage |
| Reliable operating range | Reliability by input length, condition count, dependency depth, state volume, session length, and total compute |

These are **research goals**. Explicit retention of constraints, authority, state, and evidence is a **design objective**; whether those mechanisms reduce failures under the same model, tasks, and total budget is a **testable hypothesis**. Comparisons record model, system instructions, tools, output limits, retries, and controller cost. The [research hypotheses](docs/research-hypotheses.md) define measurements; the [prospective evaluation plan](docs/research/epistesys-prospective-evaluation-plan.md) remains unexecuted and unregistered.

### Exploratory predecessor evaluation: GB-CC75

A retrospective exploratory paired evaluation of predecessor Labyrinth-Codex, dated 2026-08-18, using 75 tasks adapted from ComplexConstraints.

| Aggregation unit | Direct Grok | Grok + Labyrinth | Difference |
| --- | ---: | ---: | ---: |
| Primary: equal-weight mean rubric adherence over 65 QIDs | 74.38% | 94.10% | +19.71 points |
| Descriptive: all-criteria pass over 167 paired observations | 39.52% (66/167) | 48.50% (81/167) | +8.98 points |

The recorded QID-bootstrap 95% interval is +12.20 to +27.83 points; differences use unrounded values. The primary 65 QIDs show 32 wins, 20 ties, and 13 losses. Selection is 75→67→65 QIDs and 225→171→167 pairs. Mean adherence is neither complete compliance nor task accuracy; the interval excludes acquisition selection and systematic Judge error.

These are **bounded predecessor observations**, not evidence of alpha.2 improvement, official ComplexConstraints scores, independent human gold, causal TL effects, hallucination reduction, or long retention. The Judge design withholds route labels and other identifiers, but independence of the Labyrinth-based evaluator is unestablished. Known tasks are not a new holdout.

See the [study](docs/research/gb-cc75-study.md), [methods/provenance](docs/research/gb-cc75-methods-and-provenance.md), [statistical specification](docs/research/gb-cc75-statistical-analysis.md), [methodological audit](docs/research/gb-cc75-adversarial-audit.md), and [recomputable aggregates](benchmarks/gb-cc75/2026-08-18/README.md).

### Execution examples

These are diagnostics; parsing does not issue execution authority. Effectful verify/finalize commands require explicit `--execute` and conditions including registered tools and external receipts.

```powershell
./scripts/run-epistesys.ps1 lc631-tl-doctor --prompt "Please test the package. Do not publish."
./scripts/run-epistesys.ps1 lc631-world-doctor --prompt "Please test the package. Do not publish."
./scripts/run-epistesys.ps1 lc631-doctor --repo .
```

The launcher resolves its runner relative to its installation. See the [execution guide](docs/execution-guide.md) for startup including other working directories, and the [routing contract](docs/codex-default-routing.md) for UTF-8 stdin/default Codex facades. Configured defaults are not host-wide enforcement.

### Documentation routes

| Reader interest | Entry point |
| --- | --- |
| System boundaries, dataflow, terminology | [System architecture](docs/system-architecture.md) |
| Execution, repair, resume | [Execution guide](docs/execution-guide.md) → [DGCL execution contract](docs/dgcl-operating-profile-and-closure.md) |
| Wire formats and compatibility | [DGCL wire compatibility](docs/dgcl-wire-compatibility.md) → [schema contract](schemas/closure-and-receipt-contract.md) |
| Evaluation design and observations | [Research hypotheses](docs/research-hypotheses.md) → [GB-CC75 study](docs/research/gb-cc75-study.md) |
| Current verification and residual conditions | [Alpha.2 final verification](validation/epia2-alpha2-release-verification-2026-10-02.md) → [Known limitations](docs/known-limitations.md) |
| Source provenance, exclusions, history | [Migration contract](docs/migration-contract.md) → [Documentation index](docs/README.md) |

The initial clone uses independent Git history and excludes personal state, credentials, caches, receipt roots, and replay ledgers. `lc631-*` names are compatibility identifiers retained to limit behavior changes. The [documentation index](docs/README.md) separates detailed implementation records and inherited design/audit history from the current specification.
