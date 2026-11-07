# 会話とコーディングの支援 / Conversation and coding assistance

[日本語](#日本語) · [English](#english)

## 日本語

説明を聞きたいとき、考えを整理したいとき、仕様を相談したいときには **Conversation Projector** を選びます。実装・テスト・レビューの依頼には **Coding Projector** を選びます。

### 相談を、確認できる要件へ

> 今の出力形式は維持したい。新機能は設定を有効にしたときだけ使いたい。設定の名前はまだ決めていない。

この相談で決まっているのは「出力形式を維持する」「有効なときだけ使う」の二つです。設定名は次に相談する事項として残せます。実装へ進む場合は、合意した条件を次の依頼にも書き添えます。

この例は相談の進め方を示しており、そのままの文を解析器が受理したという記録ではありません。解析器は、対応する日英の指示文法で読み取れた条件を原文の該当箇所に結び付けます。条件や例外を読み取れなかった場合や、複数の解釈が残った場合は、依頼文と照合して補えます。要件を抽出できなかったことを、要件を満たしたとは扱いません。

別セッションや別ホストへ、会話全体が自動共有されるわけではありません。相談から実装へ移る際も、引き継ぐ条件を明示します。

### 依頼に添えること

- 実現したい動作と、変えてはいけない動作。
- 既に決めたことと、仮定・未決定のこと。
- 対象のファイルや版、使える検査、実行してほしくない操作。

<a id="checks-ja"></a>

### 「できた」の根拠を分ける

リポジトリの検証用パッケージには、2個の整数を足す公開関数 `checked_add` があります。同じ関数に対して、次のテストはそれぞれ別の問いに答えます。

| 確かめたいこと | テストに書かれている確認 |
| --- | --- |
| 関数は境界値を正しく扱うか | [ライブラリ内の単体テスト](../fixtures/dgcl/producer-package/src/lib.rs)では `2 + 2` が `Some(4)`、`u32::MAX + 1` が `None` になることを確認する。 |
| 利用側から公開APIを呼べるか | [接続テスト](../fixtures/dgcl/producer-package/tests/connection.rs)ではテスト側から `dgcl_producer_fixture::checked_add(3, 7)` を呼び、戻り値が `Some(10)` であることを確認する。 |

Epistesysの検証計画では、コンパイル、実行時テスト、登録された検証用プログラムからの公開API呼び出し、受入条件を要求ごとに扱います。ビルドの成功だけが記録されている段階では、公開APIの呼び出しは未確認です。接続テストの結果が示す範囲は、登録した呼び出しと、そこで調べた戻り値や条件までです。[検証の種類と実行条件](dgcl-operating-profile-and-closure.md#日本語)

<a id="languages-ja"></a>

### 言語ごとの対応範囲

プログラム解析の対象は **Rust・Python・Assembly** です。登録ツールでビルドとテストを実行する経路はRust/Cargo向けで、ファイル修復は許可された既存のRustファイルが対象です。[解析対象の言語定義](../scripts/lc631/crates/lc631-analysis/src/model.rs) · [検証・修復の範囲](known-limitations.md#日本語)

C/C++はDeepGrammarの設計書で、コンパイラのフロントエンドとTree-sitterを使う候補に挙がっています。Beta-7.1ではC/C++専用の解析・ビルド・修復は未対応です。[設計書のバックエンド候補](v6.3.1-deepgrammar-parser-engine-research-and-design-2026-08-25.md#14-backend配置)

### 設計案を同じ条件で比べる

設計を相談するときは、まず各案で必ず守る条件を揃えます。たとえば「既存の出力形式を維持する」が必須なら、その条件を固定して、案ごとに得られる動作と変更コストを比較します。

有限モデルと測定入力を用意できる場合は、[EpiMapLossとEpiRecompScore](epimaploss-epirecompscore-formulas.md#日本語)を比較に使えます。たとえば `design_measurement` が扱うのは、スケジューリングの優先順位を変えたときの結果です。通常の会話から任意のプロジェクトを自動採点する機能ではありません。未測定・比較不能を0で埋めず、定義と値域が明確で観測だけが不足する場合は、測定契約に従って区間を保持できます。

### 変更後と再開時に、確認を更新する

修復では、呼び出し側が渡した変更候補を許可されたファイルに適用し、変更後のソースで検証し直します。再開時に役立つのは、**どの版を変更したか、その版で何を検査したか、どの確認が残ったか**の記録です。進展なし、同じ状態の繰り返し、予算切れ、変更が適用されたか不明な失敗は、それぞれ別の結果として記録します。

証拠は対象の版と有効期間に結び付いています。checkpointがあっても、再開時の権限・対象の版・証拠の有効性を再確認します。以前の成功だけで、変更後の検証を済ませたことにはしません。

通常の `lc631-dgcl-run` で得られるのは診断と検証計画です。実際の検証や修復には登録ツールと権限が必要です。標準バイナリの `lc631-dgcl-coding-repair` と `lc631-dgcl-coding-resume` は、外部の実行先が未接続のため `Hold`（保留）を返します。SDKやパッケージ操作を使う手順は[実行ガイド](execution-guide.md#日本語)にあります。

### 実装を追う入口

| 確認したい機構 | 実装 |
| --- | --- |
| 原文と要求の対応 | [instruction grammar](../scripts/lc631/crates/lc631-analysis/src/instruction_grammar.rs)、[DGCL pipeline](../scripts/lc631/crates/lc631-analysis/src/dgcl_pipeline.rs) |
| 検証計画と未確認項目 | [completion plan](../scripts/lc631/crates/lc631-analysis/src/completion_plan.rs)、[implementation closure](../scripts/lc631/crates/lc631-analysis/src/dgcl_implementation_closure.rs) |
| 登録された呼出しの観測 | [production connection](../scripts/lc631/crates/lc631-analysis/src/production_connection.rs) |
| 証拠の更新・修復 | [evidence lifecycle](../scripts/lc631/crates/lc631-analysis/src/evidence_lifecycle.rs)、[repair loop](../scripts/lc631/crates/lc631-analysis/src/repair_loop.rs) |
| 有限モデルの比較 | [design measurement](../scripts/lc631/crates/lc631-runtime/src/design_measurement.rs)、[joint selection](../scripts/lc631/crates/lc631-runtime/src/joint_selection.rs) |

現在の試験範囲は[Beta-7.1の記録](epistesys-7.1.0-beta.1-release-notes-2026-11-06.md#日本語)、未対応・未観測の範囲は[既知の制約](known-limitations.md#日本語)で確認できます。

## English

Choose **Conversation Projector** to ask for an explanation, think through a question or discuss a specification. Choose **Coding Projector** for implementation, tests or review.

### From discussion to checkable requirements

> Keep the existing output format. Enable the new feature only when its setting is on. We haven't chosen a name for the setting yet.

Two decisions have been made: preserve the output format and require the setting to be on. The setting name can remain an open question. If you move on to implementation, include the agreed conditions in the next request.

This illustrates a discussion, not an observation that the parser accepted the example verbatim. The parser links conditions it can read under its supported Japanese and English instruction grammar to their place in the original request. Refer to that text to fill in a missed exception or resolve an ambiguous condition. Failure to extract a requirement is not evidence that it was satisfied.

Complete conversations are not automatically shared across sessions or hosts. Explicitly carry forward the relevant constraints when moving from discussion to implementation.

### What to include in a request

- Desired behavior and behavior that must remain unchanged.
- Decisions already made, assumptions and open questions.
- Target files and revisions, available checks and actions not to perform.

<a id="checks-en"></a>

### Separate the evidence for completion

The repository's verification fixture exposes a function called `checked_add` that adds two integers. Two tests ask different questions about that function.

| What do you want to know? | What the test checks |
| --- | --- |
| Does the function handle boundary values? | The [library unit test](../fixtures/dgcl/producer-package/src/lib.rs) checks that `2 + 2` returns `Some(4)` and `u32::MAX + 1` returns `None`. |
| Can a caller use the public API? | The [connection test](../fixtures/dgcl/producer-package/tests/connection.rs) calls `dgcl_producer_fixture::checked_add(3, 7)` from outside the library and checks for `Some(10)`. |

For each requirement, the validation plan distinguishes compilation, runtime tests, public API calls from a registered driver, and acceptance conditions. When only a successful build has been recorded, the API call is still unverified. The connection test provides evidence for the registered call and its specific assertions. [Validation types and operating conditions](dgcl-operating-profile-and-closure.md#english)

<a id="languages-en"></a>

### Language coverage

The program analysis module handles **Rust, Python and Assembly**. The registered build and test path targets Rust/Cargo. File repair applies to authorized, existing Rust files. [Analysis language definitions](../scripts/lc631/crates/lc631-analysis/src/model.rs) · [Validation and repair scope](known-limitations.md#english)

The DeepGrammar design lists a compiler frontend and Tree-sitter as candidate backends for C/C++. Dedicated C/C++ analysis, build and repair are outside the Beta-7.1 implementation. [Backend candidates in the design](v6.3.1-deepgrammar-parser-engine-research-and-design-2026-08-25.md#14-backend-placement)

### Compare designs under shared conditions

Begin with the constraints every proposal must satisfy. For example, if the existing output format must be preserved, keep it fixed while comparing the behavior each proposal adds and the cost of making the change.

With a finite model and measurement inputs, you can use [EpiMapLoss and EpiRecompScore](epimaploss-epirecompscore-formulas.md#english) in that comparison. For example, `design_measurement` evaluates what happens when scheduling priorities change. It does not automatically score arbitrary projects from ordinary conversation. Do not fill unmeasured or incomparable items with zero; when the definition and range are clear but observations are missing, the measurement contract may allow an interval.

### Refresh checks after changes and on resume

Repair applies a caller-supplied candidate to an authorized file and validates the changed source. For a handoff, record **which revision changed, what was checked against it and which checks remain open**. The repair process records lack of progress, repeated states, exhausted budgets and failures with uncertain effects separately.

Evidence is bound to a revision and validity period. A checkpoint does not remove the need to recheck authority, the current revision and evidence validity when resuming. Earlier success does not establish validation of changed source.

Ordinary `lc631-dgcl-run` returns diagnostics and a validation plan. Running checks or applying repairs requires registered tools and authority. In the standard binary, `lc631-dgcl-coding-repair` and `lc631-dgcl-coding-resume` return `Hold` because the external provider is unconnected. The [execution guide](execution-guide.md#english) covers the SDK and package operations.

### Implementation entry points

| Mechanism | Source |
| --- | --- |
| Original text and requirements | [Instruction grammar](../scripts/lc631/crates/lc631-analysis/src/instruction_grammar.rs), [DGCL pipeline](../scripts/lc631/crates/lc631-analysis/src/dgcl_pipeline.rs) |
| Plans and unresolved obligations | [Completion plan](../scripts/lc631/crates/lc631-analysis/src/completion_plan.rs), [implementation closure](../scripts/lc631/crates/lc631-analysis/src/dgcl_implementation_closure.rs) |
| Registered invocation observations | [Production connection](../scripts/lc631/crates/lc631-analysis/src/production_connection.rs) |
| Evidence refresh and repair | [Evidence lifecycle](../scripts/lc631/crates/lc631-analysis/src/evidence_lifecycle.rs), [repair loop](../scripts/lc631/crates/lc631-analysis/src/repair_loop.rs) |
| Finite-model comparison | [Design measurement](../scripts/lc631/crates/lc631-runtime/src/design_measurement.rs), [joint selection](../scripts/lc631/crates/lc631-runtime/src/joint_selection.rs) |

See the [Beta-7.1 record](epistesys-7.1.0-beta.1-release-notes-2026-11-06.md#english) for current test scope, and [known limitations](known-limitations.md#english) for unsupported and unobserved areas.
