# Coding文書改訂の敵対監査 / Coding documentation adversarial review

## 日本語

### 対象と変更範囲

基準commitは`693ff9404830abdfea6bcb10b8b798329c0482b6`、source identityは`6.3.2-alpha.2`です。READMEと現行DocsのCoding説明を、対応するsource・実行契約・既存release記録と照合しました。変更はMarkdown文書だけです。source、schema、fixture、version、runtime behaviorは変更していません。本書は内部の文書・限定source監査であり、独立査読や一般性能の評価ではありません。

### 敵対所見と反映

| ID | 読み手が誤認しうる点 | 文書上の修正 |
| --- | --- | --- |
| D01 | Coding Projectorの呼出しだけで検証・修復が実行される | facade、解析候補、入力契約付きpackage操作の役割を明示 |
| D02 | 登録要求の完了が、元の依頼の全要求を抽出した証拠になる | 要求抽出の精度と、抽出・登録済み要求に対する判定を区別 |
| D03 | repairが自律的にコードを生成する、または全言語へ適用できる | 呼出側replacement、既存Rust file、外部権限、snapshotと再検証を明記 |
| D04 | coordinator上限の4回がCLIによる4候補の自動連続修復を意味する | SDK上限と、CLIが渡す単一request・`attempt=1`を明記 |
| D05 | connection観測が全呼出し経路や実サービスの配備を検証する | 登録run・target・APIからexecutorへの指定経路に限定 |
| D06 | ledger出力が任意コード・最終説明文の意味的正しさを検証する | 要求・gap ledgerの再検査、候補bytes、host送信・受領を区別 |
| D07 | repository閲覧やEpistesysへの言及が常時使用を選択する | 明示選択・host有効化後の利用方法へ変更。旧skill表現より利用者指示を優先 |
| D08 | 継承元sourceへの相対linkが現在のcloneにも存在する | 履歴資料の10 linkを当時の参照文字列へ変更し、現行Docsへの参照を追加 |

各修正は日本語を前半、同じ主張範囲の英語を後半に反映しました。研究目的、実装機構、限定された観測、将来評価を保持し、GB-CC75の数値や研究上の境界は変更していません。

### Sourceとの照合

- [Package repair CLI](../scripts/lc631/crates/lc631-cli/src/dgcl_repair.rs)：`replacement_utf8`、`attempt=1`、単一requestのcoordinator入力を確認。
- [File repair driver](../scripts/lc631/crates/lc631-analysis/src/file_repair.rs)：既存Rust fileへの適用、backup、変更後snapshotと再検証を確認。
- [Repair coordinator](../scripts/lc631/crates/lc631-analysis/src/repair_loop.rs)：最大4回の上限と予算・NoProgress等の状態を確認。
- [Completion plan](../scripts/lc631/crates/lc631-analysis/src/completion_plan.rs)：要求別の5 evidence kindとproducerの区別を確認。
- [Final output](../scripts/lc631/crates/lc631-analysis/src/dgcl_final_output.rs)：検証済みconstructorによるledger描画と、通常candidateの`NotObserved`、未観測host stageを確認。

### 補助監査と検証

Assurance-Compiler doctorは`ok=true`でした。上記`dgcl_final_output.rs`全体12,932 bytesをsource textとして渡しました。ファイルSHA-256は`d71d40bba41444cfe7f1af8625b51a78e2c8ceda361c344bcff8c1cf20f350d5`、返却されたAssurance input digestは`6ca1b1a2e495b640c6e00e00c45900bd07ed61544b17321e928c8c2860d7d880`です。結果は`needs_evidence`、diagnostics 0、evidence `missing_or_unknown`。入力digestとファイルhashは別の識別子として記録します。依存crate、Cargo world、runtime、host全体はこの補助監査の範囲外であり、証明・安全性・merge approvalへ昇格しません。

RepoSeiri 1.1を`docs` profile・`docs` subtreeへ使用しました。改訂前は37文書すべてを選択し、source-session digestは`sha256:e7352434f5ba46f070d1d5399ac9951911b88f53568e7d319e07b20355dc1461`、document diagnosticsは0でした。改訂後のsummary・documents・linter・summaryは`sha256:bda053c570da96f7923a38399e78a0f94f749accd5ac94ec529f87e3201beafb`で一致し、37文書を省略なく選択、diagnostics 0でした。このdigestのscopeはDocs subtreeであり、root READMEと本監査記録の本文を含みません。wording linterは1 fileと3 generated surfacesを対象にfindings 0であり、全37文書の意味監査とは異なります。Docs scopeにはsource capabilityの観測がありません。Unknown、履歴versionとの差、数値をversion候補と誤認する抽出結果を、機能上の矛盾の証明にしません。

検証項目は既存の`python -B scripts/research/verify-research-docs.py`、変更した追加文書への同validator、Docs全体のローカルlink存在・repository境界検査、`git diff --check`、変更差分の日英意味同等性とsource根拠の再読です。文書検査で検出した数値表記差を修正し、履歴linkの欠落・repository外参照を解消しました。source変更がないため、Cargo検証や新しいCoding実験は行っていません。

既存validatorは17文書・304 link、追加適用は3文書・78 linkの言語順・数値・参照検査を通過しました。Docs、root README、本書を合わせた39 file・476 linkのローカル参照先検査では、欠落・repository外参照は0件でした。新README anchorの存在も確認しました。これらの構造検査と、日英の意味同等性の再読は別の確認として扱います。

### 残る評価範囲

独立gold、要求抽出recall、oracle妥当性、一般Coding性能、長期session、実host callback/deliveryは引き続き別の評価・観測を要します。`PendingNoCorpus`と`PendingHostObservation`を保持します。宣言profileの機構を説明する文書改訂であり、研究効果の達成や全弱点の解消は主張しません。[DGCL実行契約](../docs/dgcl-operating-profile-and-closure.md)と[既知の制約](../docs/known-limitations.md)を参照してください。

## English

### Scope and changes

Baseline commit: `693ff9404830abdfea6bcb10b8b798329c0482b6`; source identity: `6.3.2-alpha.2`. README/current Coding documentation was compared with the corresponding source, execution contract, and existing release records. Changes affect Markdown only; source, schemas, fixtures, version, and runtime behavior are unchanged. This is an internal documentation/bounded-source review, not independent peer review or general-performance evaluation.

### Adversarial findings and treatment

| ID | Potential reader misunderstanding | Documentation correction |
| --- | --- | --- |
| D01 | Invoking the Coding Projector executes validation/repair by itself | Explain facade, analysis candidates, and package operations requiring input contracts |
| D02 | Completion of registered requirements proves exhaustive extraction from the request | Separate extraction accuracy from decisions over extracted/registered requirements |
| D03 | Repair autonomously generates code or applies to every language | Specify caller replacements, existing Rust files, external authority, snapshots, and revalidation |
| D04 | The ceiling of 4 attempts establishes automatic consecutive repair of 4 CLI candidates | Specify the SDK ceiling and the CLI's single request with `attempt=1` |
| D05 | Connection observations validate all invocation paths or service deployment | Bound evidence to registered runs, targets, and specified API-to-executor paths |
| D06 | Ledger output validates arbitrary code/final-prose semantics | Distinguish requirement/gap revalidation, candidate bytes, and host send/receipt |
| D07 | Repository browsing or mentioning Epistesys selects continual use | Describe use after explicit selection/host enablement; user instructions override old skill wording |
| D08 | Relative links to predecessor source resolve in the current clone | Convert 10 historical links to provenance text and add current-document references |

Corrections place Japanese prose before English prose with the same claim scope. Research goals, mechanisms, bounded observations, and future evaluation remain distinct; GB-CC75 numbers and research boundaries are unchanged.

### Source correspondence

- [Package repair CLI](../scripts/lc631/crates/lc631-cli/src/dgcl_repair.rs): inspect `replacement_utf8`, `attempt=1`, and single-request coordinator input.
- [File repair driver](../scripts/lc631/crates/lc631-analysis/src/file_repair.rs): inspect existing-Rust-file application, backups, changed snapshots, and revalidation.
- [Repair coordinator](../scripts/lc631/crates/lc631-analysis/src/repair_loop.rs): inspect the maximum of 4 attempts, budgets, and NoProgress/other states.
- [Completion plan](../scripts/lc631/crates/lc631-analysis/src/completion_plan.rs): inspect the 5 per-requirement evidence kinds and distinct producers.
- [Final output](../scripts/lc631/crates/lc631-analysis/src/dgcl_final_output.rs): inspect checked-constructor ledger rendering, ordinary candidates' `NotObserved` state, and unobserved host stages.

### Advisory audit and verification

Assurance-Compiler doctor reported `ok=true`. The complete 12,932-byte `dgcl_final_output.rs` unit was supplied as source text. File SHA-256: `d71d40bba41444cfe7f1af8625b51a78e2c8ceda361c344bcff8c1cf20f350d5`; returned Assurance input digest: `6ca1b1a2e495b640c6e00e00c45900bd07ed61544b17321e928c8c2860d7d880`. Result: `needs_evidence`, zero diagnostics, evidence `missing_or_unknown`. Input digest and file hash are recorded as distinct identifiers. Dependency crates, Cargo worlds, runtime, and whole-host behavior are outside this advisory review; no promotion to proof, safety, or merge approval.

RepoSeiri 1.1 was used with profile `docs` and scope `docs` subtree. Before revision, all 37 documents were selected, source-session digest was `sha256:e7352434f5ba46f070d1d5399ac9951911b88f53568e7d319e07b20355dc1461`, and document diagnostics were zero. After revision, summary/documents/linter/summary matched at `sha256:bda053c570da96f7923a38399e78a0f94f749accd5ac94ec529f87e3201beafb`, selecting all 37 documents without omissions and reporting zero diagnostics. This digest covers the Docs subtree, excluding the root README and this review record's contents. Wording lint covered 1 file and 3 generated surfaces with zero findings, distinct from semantic review of all 37 documents. Docs scope has no source-capability observations. Unknowns, historical-version differences, and extraction of a numeric score as a version candidate do not prove functional contradictions.

Verification covers existing `python -B scripts/research/verify-research-docs.py`, the same validator on additional changed documents, all-Docs local-link existence/repository-boundary checks, `git diff --check`, and rereading changed prose for Japanese/English semantic parity and source support. The numeric-notation mismatch found by document checks was corrected, and missing/escaping historical links were resolved. No Cargo checks or new Coding experiments were run because source did not change.

The existing validator passed language-order, numeric, and reference checks for 17 documents/304 links; additional application passed for 3 documents/78 links. Local-target checks over Docs, the root README, and this record covered 39 files/476 links with zero missing or repository-escaping references. New README anchors were also present. These structural checks and rereading for semantic parity are separate observations.

### Remaining evaluation scope

Independent gold, requirement-extraction recall, oracle validity, general coding performance, long sessions, and real-host callbacks/delivery still require separate evaluation or observation. Retain `PendingNoCorpus` and `PendingHostObservation`. This revision explains declared-profile mechanisms without claiming research-goal completion or elimination of every weakness. See the [DGCL execution contract](../docs/dgcl-operating-profile-and-closure.md) and [known limitations](../docs/known-limitations.md).
