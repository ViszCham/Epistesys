# 会話とコーディングの支援 / Conversation and coding assistance

## 日本語

Epistesysは、相談で整理した要件を、実装・検証・修復につなぐための開発支援ツールです。要求の扱い方、検証項目の分け方、証拠の更新、修復・再開といった**開発手順の一部をRustの型と状態遷移で実装**しています。モデルに代わってコードを書く仕組みではありません。

### どんな場面で使うか

| 場面 | 整理・確認したいこと |
| --- | --- |
| 仕様を相談する | 守る条件、仮定、例外、まだ決めていない点 |
| 既存機能を変更する | 新しい動作、呼び出し経路、従来の互換性 |
| レビューする | 確認済みの事実、推測、足りない検証 |
| 複数の案を比べる | 共通の必須条件、利点、損失、コスト、比較できない項目 |
| 中断した作業を再開する | 変更した版、実行した検査、その後に変わった条件 |

### 相談を、確認できる要件へ

Conversation Projectorは説明・設計相談を、Coding Projectorは実装・テスト・レビューを扱う入口です。共通の解析経路で原文との対応を保持し、引用やコード片などの領域を分けます。対応する指示文法の範囲で、否定・条件・例外・適用範囲・依存関係を要求へ結び付け、解釈が一つに決まらない箇所は未解決事項として残します。要件を抽出できなかったことを、要件を満たしたとは扱いません。

例えば「既存の出力形式は維持したい。新機能は設定を有効にしたときだけ使いたい」という相談なら、出力互換性と有効化条件を別の確認項目として扱います。これは相談の意図の例であり、この日本語がそのままパーサーに受理される保証ではありません。解析できなかった条件は明示して補う必要があります。

相談から実装へ移るときは、合意した条件を次の依頼にも含めます。会話の全内容が、別セッションや別ホストへ自動共有されるわけではありません。

### 「できた」の根拠を分ける

要求別の検証計画では、実装対象との対応、コンパイラ検査、実行時テスト、登録された公開APIの直接呼出し、受入条件を分けます。ビルドが通っても、計画に必要な呼出しや互換性の確認が欠けていれば、その項目は未確認です。

レビューでは、登録された検証結果から「どの版で、何を実行し、何が分かったか」を追えます。対象は検証用のプログラムと指定した検査条件の範囲です。リポジトリ内の全呼出し経路や、依頼から漏れなく要件を抽出できたことまで保証するものではありません。

### 変更後と再開時に、確認を更新する

証拠には対象の版や有効期間があり、変更前の成功をそのまま流用しません。修復経路は、呼出し側が渡した候補を許可された対象へ適用し、変更後のソースと証拠で再評価します。進展なし・循環・予算不足・失効・作用が不明な失敗を区別します。

引継ぎには、**変更内容・実行した検査・残る課題**をまとめると役立ちます。checkpointがあっても、再開時の権限や対象の版は再確認します。修復候補の生成や検証内容の妥当性まで、自動的に保証されるわけではありません。

### 設計案を同じ条件で比べる

設計相談では、必須条件を先に揃え、各案の利点・損失・コストを分けて考えます。利用者や呼出し側が有限モデルと測定入力を用意した場合は、EpiMapLoss、EpiRecompScoreや候補選択APIを比較材料に使えます。未測定・比較不能をゼロ扱いせず、理由とともに残します。

これらは、通常の会話から任意の設計を自動採点する機能ではありません。例えば`design_measurement`は有限のスケジューリングモデルを扱い、任意のRustプロジェクトの設計品質を判定するものではありません。巨大ファイルの防止や、常に最良のモジュール分割が得られるという説明はしません。

### まず何を渡せばよいか

相談や修正依頼には、次の内容を添えると確認項目を作りやすくなります。

- 実現したい動作と、変えてはいけない動作。
- 既に決めたことと、仮定・未決定のこと。
- 対象のファイルや版、使える検査、実行してほしくない操作。

通常の`lc631-dgcl-run`は解析候補と検証計画を返す診断です。外部ツールの実行やファイル修復まで一括で行いません。実際の修復・再開には、対応する権限、登録ツール、検証計画と、SDKまたはpackage操作への接続が必要です。現在の標準実行ファイルでは、`lc631-dgcl-coding-repair`／`lc631-dgcl-coding-resume`は外部provider未接続としてHold（保留）を返します。引数や環境変数だけで有効化するものではありません。[実行ガイド](execution-guide.md)と[DGCL実行契約](dgcl-operating-profile-and-closure.md)をご覧ください。

### 実装を追う入口

| 確認したい機構 | 実装 |
| --- | --- |
| 原文と要求の対応 | [instruction grammar](../scripts/lc631/crates/lc631-analysis/src/instruction_grammar.rs)、[DGCL pipeline](../scripts/lc631/crates/lc631-analysis/src/dgcl_pipeline.rs) |
| 検証計画と未確認項目 | [completion plan](../scripts/lc631/crates/lc631-analysis/src/completion_plan.rs)、[implementation closure](../scripts/lc631/crates/lc631-analysis/src/dgcl_implementation_closure.rs) |
| 登録された呼出しの観測 | [production connection](../scripts/lc631/crates/lc631-analysis/src/production_connection.rs) |
| 証拠の更新・修復 | [evidence lifecycle](../scripts/lc631/crates/lc631-analysis/src/evidence_lifecycle.rs)、[repair loop](../scripts/lc631/crates/lc631-analysis/src/repair_loop.rs) |
| 有限モデルの比較 | [design measurement](../scripts/lc631/crates/lc631-runtime/src/design_measurement.rs)、[joint selection](../scripts/lc631/crates/lc631-runtime/src/joint_selection.rs) |

実装と試験の状況は[Beta-7.1の記録](epistesys-7.1.0-beta.1-release-notes-2026-11-06.md)、数式は[測定式の一覧](epimaploss-epirecompscore-formulas.md)、未観測の範囲は[既知の制約](known-limitations.md)へ分けています。一般的な精度向上、手戻り削減率、長期会話の保持率を実証したという説明はしていません。

## English

Epistesys helps connect requirements discussed in conversation to implementation, validation and repair. It implements **parts of the development procedure as Rust types and state transitions**: requirement handling, distinct validation obligations, evidence refresh, repair and resume. It does not replace the model that writes code.

### When to use it

| Situation | Questions to clarify or check |
| --- | --- |
| Discuss a specification | Constraints, assumptions, exceptions and undecided points |
| Modify existing behavior | New behavior, invocation paths and compatibility |
| Review a change | Observed facts, inferences and missing validation |
| Compare alternatives | Shared hard requirements, benefits, losses, cost and incomparable items |
| Resume interrupted work | Changed revision, checks performed and conditions changed since then |

### From discussion to checkable requirements

Conversation Projector is the entry for explanation/design discussions; Coding Projector handles implementation, testing and review. Their common analysis path retains source correspondence and separates regions such as quotes and code. Supported controlled-grammar negation, conditions, exceptions, scopes and dependencies are linked to requirements; ambiguous interpretations remain unresolved. Failure to extract requirements is not treated as satisfying them.

For example, a discussion about preserving an output format while enabling a new feature only through a setting has separate compatibility and enablement checks. This illustrates the intent, not guaranteed parser acceptance of the Japanese example. Conditions that were not parsed must be made explicit and supplemented.

Carry agreed constraints into the implementation request. Complete conversations are not automatically shared across sessions or hosts.

### Separate the evidence for completion

Per-requirement plans distinguish implementation bindings, compiler checks, runtime tests, registered direct public-API calls and acceptance conditions. A successful build leaves an item open when a required invocation or compatibility check is missing.

Review can trace registered validation results to the revision, execution and observations they describe. The scope is the validation program and specified assertions, not every repository call path or exhaustive extraction of all intended requirements.

### Refresh checks after changes and on resume

Evidence is revision- and validity-bound; earlier success is not automatically reusable. The repair path applies a caller-supplied candidate to an authorized target and reevaluates it with a new snapshot and evidence. It separates no progress, cycles, budget exhaustion, expiry and failures with uncertain effects.

A useful handoff records **changes, checks performed and remaining work**. A checkpoint does not remove the need to recheck authority and current revisions. Candidate generation and the adequacy of validation are not automatically guaranteed.

### Compare designs under shared conditions

In design discussions, establish hard requirements first and separate benefits, loss and cost. With finite models and measurement inputs provided by the user or caller, EpiMapLoss, EpiRecompScore and selection APIs can supply comparison material. Unmeasured or incomparable items remain explicit rather than becoming zero.

These APIs do not automatically score arbitrary designs from ordinary conversation. For example, `design_measurement` uses a finite scheduling model, not an oracle for arbitrary Rust-project architecture. There is no guarantee of preventing large files or always producing the best module split.

### What to provide first

The following information helps identify checks for a discussion or change request:

- Desired behavior and behavior that must stay unchanged.
- Decisions already made, assumptions and undecided points.
- Target files/revisions, available checks and actions not to perform.

Ordinary `lc631-dgcl-run` returns analysis candidates and a validation plan; it does not automatically execute external tools or repair files. Actual repair/resume requires relevant authority, registered tools, validation plans and an SDK or package-operation connection. In the current standard binary, `lc631-dgcl-coding-repair` / `lc631-dgcl-coding-resume` returns Hold because the external provider is not connected; arguments or environment variables alone do not enable it. See the [execution guide](execution-guide.md) and [DGCL contract](dgcl-operating-profile-and-closure.md).

### Implementation entry points

| Mechanism | Source |
| --- | --- |
| Original text and requirements | [Instruction grammar](../scripts/lc631/crates/lc631-analysis/src/instruction_grammar.rs), [DGCL pipeline](../scripts/lc631/crates/lc631-analysis/src/dgcl_pipeline.rs) |
| Plans and unresolved obligations | [Completion plan](../scripts/lc631/crates/lc631-analysis/src/completion_plan.rs), [implementation closure](../scripts/lc631/crates/lc631-analysis/src/dgcl_implementation_closure.rs) |
| Registered invocation observations | [Production connection](../scripts/lc631/crates/lc631-analysis/src/production_connection.rs) |
| Evidence refresh and repair | [Evidence lifecycle](../scripts/lc631/crates/lc631-analysis/src/evidence_lifecycle.rs), [repair loop](../scripts/lc631/crates/lc631-analysis/src/repair_loop.rs) |
| Finite-model comparison | [Design measurement](../scripts/lc631/crates/lc631-runtime/src/design_measurement.rs), [joint selection](../scripts/lc631/crates/lc631-runtime/src/joint_selection.rs) |

Implementation/tests are recorded in the [Beta-7.1 notes](epistesys-7.1.0-beta.1-release-notes-2026-11-06.md), equations in the [formula reference](epimaploss-epirecompscore-formulas.md), and unobserved scope in [known limitations](known-limitations.md). No general accuracy improvement, rework-reduction rate or long-conversation retention rate is established here.
