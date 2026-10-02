# DGCL implementation status — 2026-09-28

## 日本語

### 状態と基線

これはDGCL-00〜31の実装記録です。完了宣言・release-ready宣言ではありません。対象はEpistesys 6.3.2-alpha.1、作業開始revision `60cc71db2ba47517bbd209894033f9b55b3708f8`。versionは変更していません。作業前の`cargo test --workspace --locked`は成功しました。

DGCL-00〜03で次をpinし、`scripts/lc631/Cargo.lock`へ固定しました。

- CommonMark event/byte offsets: `pulldown-cmark 0.13.4`。
- Rust CST: `tree-sitter 0.27.0`と`tree-sitter-rust 0.24.2`。
- DG1入力上限: source 262,144 bytes、region 100,000、統合region/CST node 100,000、requirement candidate 1,024。超過はerrorとし、入力を黙って切り詰めません。
- これらは初期resource contractであって、性能測定や任意Markdown/Rust完全対応のclaimではありません。Tree-sitterの処理時間を強制終了するOS隔離はまだ接続していません。

### 実装済みの経路

- `lc631-tldg::analyze_dg1`はCommonMark block/inline領域、heading/paragraph/list/quote/link/table、code、HTML opaque領域をbyte span付きで記録します。日英判定はUnicode文字種の分類hintだけです。
- Rust fenced blockと、明確なRust itemで始まるraw sourceをtree-sitter Rust CSTへlowerします。親子node、UTF-8境界、literal/comment構造、error nodeを追跡します。認識した`sql!`はopaque embedded regionとして残し、SQL parserがあるとはclaimしません。
- 限定的な日英instruction-marker ruleは、個別要求候補、Must/Should候補、polarity、条件/例外marker、引用/未対応residualを出します。これは形態素解析・依存構文解析ではなく、完全な文法解析や自然言語意味解析ではありません。quoteやcontent textはhost-delivered authorityにならず、候補の`authority_grant`は常にfalseです。
- 同じDG1結果はUnifiedSyntaxHypergraph、SemanticViewのsource-anchored構造payload、`RequirementProgramIr` candidate、TL `ProjectionDefectGraphV3`へ渡ります。TLの各stageは未検証状態のまま保持し、空の要求集合を`Clarify`へ閉じます。
- `lc631-analysis::build_coding_closure`は要求ごとにimplementation、production connection、static validation、runtime validation、acceptance testの5 gapを個別に要求します。receiptはsource revision、requirement ID、kind、target、target digest、scopeに束縛します。変更source、重複、tamper、不正receiptはgapを閉じません。証拠完備状態であってもauthorityとoutput commitはfalseです。
- builtin adapter receiptはrelation/payload内容のcanonical JSON digestへ束縛されます。重複・拒否receiptは旧検証済みbackend状態を消し、診断を保持します。Geometry distillationは候補edge endpointの存在だけではacceptせず、matching verified edgeがない場合は`NeedsEvidence`となり、canonical edge authorityは発行しません。同じ状態を再処理して進展しない場合は`NoProgress`で止まります。
- Production CLIに`lc631-dg1-doctor`と`lc631-coding-closure`を追加しました。後者は証拠なしならgapsを開いたまま返します。READMEの起動例はこの経路も示します。
- generated robustness test 20,000件（10,000 bilingual/source-span casesと10,000 Rust raw-string/CST cases）を実行しました。これは構造・保持の回帰であり、意味精度・独立gold・外部言語backendの評価ではありません。

### 再開後の実装・検証追記（下表の旧判定を更新）

- Python 3.12の隔離環境にStanza 1.14.0と日英モデルを導入し、モデル全file・worker script・Python実行体をdigestで固定しました。モデルは無視対象のローカルcacheに置き、repoや配布物には含めません。workerはoffline設定、長さ付きJSON framing、入力/出力上限、実行時間上限、前後hash照合を持ちます。これは依存構文解析の実行観測であり、意味解釈の正しさやモデル再配布権の証拠ではありません。
- `lc631-dgcl-language`と`lc631-dgcl-parse`をproduction CLIへ追加しました。英語・日本語の単独文および混在文書でStanza依存木、UTF-8 byte offset、DG1 requirement candidateのsource対応を観測しました。実出力はJSON Schema Draft 2020-12で検証しました。
- 限定ruleの否定・条件・例外・複文・引用・未解決参照の8個のnegative regression、source-local接続経路の正例と6個の切断mutation、gold evaluatorの分母・family leakage試験、resumeのpartial/unknown-delivery試験を通しました。静的なsymbol/route照合と実binary/JSON出力観測は全call graphやhost deliveryの証明ではありません。
- `lc631-dgcl-evaluate`は言語別holdout数、gold obligation数、exact tuple、precision/recall、document exact、coverage、critical error、family leakageを評価し、不足を保留します。ユーザー確認どおり独立作成・裁定済みgoldは存在しません。現時点の評価結果を推定・捏造しません。
- `lc631-dgcl-finalize`はcheckpointを実行前に`UnknownDelivery`として記録し、実接続を観測した場合だけ`ConfirmedSuccess`を追記します。同じactionの自動再実行とresumeによる権限/完了復元を拒否します。ローカルの5 gap receiptが揃っても最終結果は`hold_host_output_unbound`であり、host最終出力は未観測です。
- Fail-first testで`--receipt-root`欠落時に実行後checkpointが残る欠陥を再現し、必須引数・receipt環境のpreflightをcheckpoint前へ移しました。実binaryを起動する`lc631-dgcl-verify`/`finalize`は明示的な`--execute`なしでは失敗します。
- 下表は初回パス時点の判定を含みます。上記追記を優先して読み、DGCL-04/11/13〜15/20/22〜26は「未実装」ではなく「限定実装・未閉鎖」と解釈します。DGCL-10/27/30/31は依然未達です。

### 32 slice判定

| Slice | 判定 | 今回の記録 |
|---|---|---|
| DGCL-00 | 完了 | clean baseline、workspace test、toolchainとcrate pinを記録 |
| DGCL-01 | 部分完了 | UTF-8 source/spanとbyte/node/count budgetを追加。worker hard-timeoutは未実装 |
| DGCL-02 | 部分完了 | CommonMark領域・opaque embedded regionを追加。全拡張/全dialectは対象外 |
| DGCL-03 | 部分完了 | pinned Rust CSTをproduction pathへ接続。compiler AST/HIR/type resolutionではない |
| DGCL-04 | 部分完了 | 隔離Python 3.12 + Stanza 1.14.0の実日英morphology/dependencyをローカル観測。モデル再配布権と独立評価は未確認 |
| DGCL-05 | 部分完了 | source-anchored structural payloadあり。MRS/UCCA/AMR/UD等の実backend payloadはUnavailable |
| DGCL-06 | 部分完了 | bounded marker rulesあり。完全なinstruction grammar/GLR解析ではない |
| DGCL-07 | 部分完了 | 各対応ruleからcandidateを出す。gold recall/precisionは未測定 |
| DGCL-08 | 部分完了 | polarity conflict・conditional/exception hint・residualを保持。任意複文/照応の正しさは未検証 |
| DGCL-09 | 部分完了 | quote/context candidateはauthority grantにならない。host principal bindingとの全入口試験は未完了 |
| DGCL-10 | 未達 | 独立作成・裁定済みgoldは未整備 |
| DGCL-11 | 部分完了 | content-bound digest・source revision・receipt failure invalidationに加え、実Stanza workerの前後hash・model manifest・source/offset bindingを観測。独立意味証拠は未取得 |
| DGCL-12 | 部分完了 | DG1→RequirementProgramIr candidate→TL辺を接続。stageはNeedsEvidence/Unresolvedであり実コード実装の証明ではない |
| DGCL-13 | 部分完了 | per-requirement closureとsource-local target/file hash・実binary照合を追加。全targetの独立照合ではない |
| DGCL-14 | 部分完了 | symbol/route文字列と実CLI出力をsource-boundで照合。完全なrepository call graphは未解析 |
| DGCL-15 | 部分完了 | production CLI実binaryの実行とJSON出力を観測。host最終出力callbackは未接続 |
| DGCL-16 | 部分完了 | strict evidence evaluatorと署名receipt経路あり。actual product acceptance suiteとは別 |
| DGCL-17 | 部分完了 | gap個別判定・duplicate/stale/tamper拒否あり。全実装が閉じたとは言えない |
| DGCL-18 | 部分完了 | exact source revision mismatchで拒否。TOCTOU/rebuild lifecycle全体は未実装 |
| DGCL-19 | 部分完了 | unchanged stateは`NoProgress`として明示。自動修復actionを実行するclosed loopは未実装 |
| DGCL-20 | 部分完了 | append-only local checkpoint/resumeでunknown delivery・重複再実行を抑制。host durable replayではない |
| DGCL-21 | 部分完了 | fake epoch progressを防止。新しい実状態を受けて更新するdistillationは未実装 |
| DGCL-22 | 部分完了 | 追加negative regression 8件、接続正例と切断mutation 6件、既存tamper等を検証。全断線種は未網羅 |
| DGCL-23 | 部分完了 | DG1/closureに加え、実日英worker/pipeline/評価/接続/再開CLIをproduction binaryへ接続。host経路は未閉鎖 |
| DGCL-24 | 未達 | finalizerは実行できるが`hold_host_output_unbound`を返す。exact final host callbackは未接続 |
| DGCL-25 | 部分完了 | additive schemaをDraft 2020-12として検査し、実混在文書出力をvalid化。全legacy consumer matrixは未走査 |
| DGCL-26 | 部分完了 | source/region/node/requirement、worker入出力・実行時間に上限。OS process tree隔離は未接続 |
| DGCL-27 | 未達 | 200 independent holdout docs/language、1,000 gold obligations/language、risk-coverageは未評価 |
| DGCL-28 | 部分完了 | Rust CST→DG1→TL→closure CLIと日英worker→DG1候補→接続観測をsource-localで実行。単一の全段chain/host outputは未閉鎖 |
| DGCL-29 | 部分完了 | doctorと16KiB以下の限定compile/renderを実施。`needs_evidence`/`blocked_with_diagnostics`を保持。repository-wide reviewではない |
| DGCL-30 | 未達 | 必須gateが未達。completion判定は`Held` |
| DGCL-31 | 未達 | 配布候補・最終closure・handoff claimなし。release-readyではない |

### 検証・残存リスク

作業前baselineに加え、DG1/closure/adapter/distillation/CLIのtargeted test、`cargo fmt --all --check`、`cargo check --workspace --locked`、`cargo test --workspace --no-default-features --locked`、`cargo test --workspace --locked`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo build --release --locked`、release binary DG1/closure CLI smokeを実行し成功しました。新schemaはDraft 2020-12として検査し、混在文書の実CLI出力をvalidatorに通しました。20,000件のgenerated testを含む今回範囲のテスト結果は、上記の限定profileに対する観測です。DGCL-31の受入条件・独立評価は満たしていません。

#### Assurance-Compiler補助監査

- Doctor: `ok=true`、semantic digest `2d102a78fdecd3d325bce8c4ef0921fb390e86b7071b7634c1f7ade8823313a`。Doctorはfacade healthのみです。
- 対象はworking tree source revision（base HEAD `60cc71db2ba47517bbd209894033f9b55b3708f8`からの未commit差分）。完全moduleは16KiB以下の`parser.rs` 14,953 bytes、`semantics.rs` 7,487 bytes、`kernel.rs` 4,137 bytes、`distillation.rs` 15,163 bytes。追加で`analyze_dg1` 10,960 bytes、`extract_requirements` 3,461 bytes、`instruction_markers` 3,399 bytes、`build_coding_closure` 9,046 bytes、BackendRegistry impl 6,616 bytes、`build_output_closure` 8,839 bytes、`build_syntax` 5,674 bytesの完全な関数/impl単位をin-bandで渡しました。
- それらのstatusは、parser/semantics/kernel、DG1/requirements/rules/closure/output/syntax単位が`needs_evidence`、diagnostics 0。`distillation.rs`全体は`blocked_with_diagnostics`（5 diagnostics）。BackendRegistry implは`blocked_with_diagnostics`（2 diagnostics）。
- distillationのrender結果も`blocked_with_diagnostics`。返却されたstable IDsとunknown/missing evidenceを保持しています。選択外の大きい`dg1.rs`（36,128 bytes）、`adapters.rs`（21,320 bytes）、`closure.rs`（23,583 bytes）、`integration.rs`（28,500 bytes）その他は未監査です。16KiB超のfileを分割扱いで監査済みに見せていません。
- これはstatic bounded review surfaceです。analyzer truth、全repository coverage、独立 corroboration、runtime safety、意味正しさ、proof、approval、authorityへ昇格しません。
- 再開後に完全module 5件（各16KiB以下）を追加監査しました。`dgcl_gold.rs`、`language_worker.rs`、`dgcl_pipeline.rs`、`closure_connection.rs`は`needs_evidence`、`closure_resume.rs`は`blocked_with_diagnostics`（1件）です。詳細は[DGCLローカル検証](../validation/dgcl-local-gates-2026-09-28.md)に記録します。

未閉鎖の主要点は、モデル由来データの再配布権の確認、独立gold、full instruction grammarと任意複文/照応/例外scope、全per-requirement targetの完全call graphおよび実行経路の追跡、host output/finalizer、process tree隔離、remote CI、独立host試験です。ローカルcheckpointはdurable host replayではありません。よってDGCL-00〜31全完了、DeepGrammar完全構文解析、実装完全閉鎖、一般的な意味理解、formal proof、host-level enforcementを主張しません。

## English

### Status and baseline

This is an implementation record for DGCL-00 through DGCL-31, not a completion or release-ready declaration. The target is Epistesys 6.3.2-alpha.1, starting revision `60cc71db2ba47517bbd209894033f9b55b3708f8`. The version was not changed. The pre-change `cargo test --workspace --locked` baseline passed.

DGCL-00 through DGCL-03 pin the following dependencies in `scripts/lc631/Cargo.lock`:

- CommonMark events and byte offsets: `pulldown-cmark 0.13.4`.
- Rust CST: `tree-sitter 0.27.0` and `tree-sitter-rust 0.24.2`.
- DG1 input limits: source 262,144 bytes, 100,000 regions, 100,000 combined region/CST nodes, and 1,024 requirement candidates. Exceeding a limit returns an error rather than silently truncating input.
- These are initial resource contracts, not performance measurements or claims of complete support for arbitrary Markdown/Rust. OS isolation that can terminate Tree-sitter by wall-clock timeout is not connected yet.

### Implemented paths

- `lc631-tldg::analyze_dg1` records CommonMark block/inline regions, headings/paragraphs/lists/quotes/links/tables, code, and opaque HTML regions with byte spans. Its Japanese/English classification is a Unicode-script hint; the separate DGCL worker described below performs real morphology and dependency parsing.
- Rust fenced blocks and raw source beginning with a clear Rust item are lowered to tree-sitter Rust CST. Parent/child nodes, UTF-8 boundaries, literal/comment structure, and error nodes are tracked. Recognized `sql!` macros remain opaque embedded regions; no SQL parser is claimed.
- A limited Japanese/English instruction-marker rule emits individual requirement candidates, Must/Should candidates, polarity, condition/exception markers, and quote/unsupported residuals. This is not morphological/dependency parsing, complete grammar parsing, or natural-language semantic parsing. Quoted/content text is not host-delivered authority; candidate `authority_grant` is always false.
- The same DG1 result flows into UnifiedSyntaxHypergraph, a source-anchored structural payload for SemanticView, a `RequirementProgramIr` candidate, and TL `ProjectionDefectGraphV3`. TL stages stay unverified; an empty requirement set is held at `Clarify`.
- `lc631-analysis::build_coding_closure` requires five individual gaps per requirement: implementation, production connection, static validation, runtime validation, and acceptance test. Receipts bind source revision, requirement ID, kind, target, target digest, and scope. Changed source, duplicates, tampering, or invalid receipts do not close gaps. Even an evidence-complete state keeps authority and output commitment false.
- Builtin adapter receipts bind to canonical JSON digests of relation/payload content. Duplicate/rejected receipts clear previous validated backend state and retain diagnostics. Geometry distillation no longer accepts an edge only because its endpoints exist; without a matching verified edge it returns `NeedsEvidence` and never issues canonical-edge authority. Reprocessing an unchanged state stops with `NoProgress`.
- Production CLI adds `lc631-dg1-doctor` and `lc631-coding-closure`. The latter returns open gaps when no evidence is provided. README startup examples show these paths.
- Ran 20,000 generated robustness cases (10,000 bilingual/source-span cases and 10,000 Rust raw-string/CST cases). These are structural/preservation regressions, not semantic accuracy, independent gold, or external language-backend evaluations.

### Resumed implementation and validation addendum (updates earlier table entries)

- An isolated Python 3.12 environment contains Stanza 1.14.0 and Japanese/English models. Digests pin every model file, worker script, and Python executable. Models remain in ignored local cache and are not in the repository or distribution. The worker uses offline mode, length-prefixed JSON framing, input/output limits, a runtime deadline, and pre/post hash checks. This observes dependency parsing, not semantic correctness or model redistribution rights.
- Production CLI commands `lc631-dgcl-language` and `lc631-dgcl-parse` were added. English and Japanese individual sentences and a mixed document produced observed Stanza dependency trees, UTF-8 byte offsets, and source-linked DG1 requirement candidates. Actual output was validated against JSON Schema Draft 2020-12.
- Eight negative regressions cover limited-rule negation, condition, exception, compound instruction, quotation, and unresolved reference handling. A source-local connection positive case and six disconnection mutations, gold denominator/family-leakage tests, and partial/unknown-delivery resume tests passed. Static symbol/route checks and actual-binary/JSON-output observation do not establish a complete call graph or host delivery.
- `lc631-dgcl-evaluate` checks language-specific holdout/gold-obligation counts, exact tuples, precision/recall, document exactness, coverage, critical errors, and family leakage; insufficient evidence stays held. As confirmed by the user, no independently authored and adjudicated gold corpus exists. We do not infer or fabricate evaluation results.
- `lc631-dgcl-finalize` records `UnknownDelivery` before execution and appends `ConfirmedSuccess` only after observing the exact connection. It rejects automatic retry of the same action and restoration of authority/completion from resume. Even when five local evidence-gap receipts are present, the final result is `hold_host_output_unbound`; final host output was not observed.
- A fail-first test reproduced a checkpoint left behind when `--receipt-root` was missing. Required arguments and the receipt environment now pass preflight before checkpointing. `lc631-dgcl-verify` and `finalize`, which launch a real binary, fail without explicit `--execute`.
- The table below includes judgments from the initial implementation pass. Read this addendum as the current update: DGCL-04/11/13–15/20/22–26 are now limited implementations with open closure, not wholly absent. DGCL-10/27/30/31 remain unmet.

### Status of the 32 slices

| Slice | Status | Record for this run |
|---|---|---|
| DGCL-00 | Complete | Clean baseline, workspace test, toolchain and crate pins recorded |
| DGCL-01 | Partial | UTF-8 source/span and byte/node/count budgets added; worker hard-timeout not implemented |
| DGCL-02 | Partial | CommonMark regions and opaque embedded region added; not every extension/dialect is supported |
| DGCL-03 | Partial | Pinned Rust CST connected to production path; not compiler AST/HIR/type resolution |
| DGCL-04 | Partial | Real Japanese/English morphology/dependency observed locally with isolated Python 3.12 and Stanza 1.14.0. Model redistribution rights and independent evaluation remain unverified |
| DGCL-05 | Partial | Source-anchored structural payload exists. Real MRS/UCCA/AMR/UD backend payloads remain Unavailable |
| DGCL-06 | Partial | Bounded marker rules exist; this is not complete instruction grammar/GLR parsing |
| DGCL-07 | Partial | Supported rules emit candidate requirements; gold precision/recall not measured |
| DGCL-08 | Partial | Polarity conflict, conditional/exception hints, and residuals are retained. Arbitrary compound-sentence/anaphora correctness is unverified |
| DGCL-09 | Partial | Quote/context candidates do not grant authority. Full entrypoint tests with host-principal binding are incomplete |
| DGCL-10 | Not met | No independently authored/adjudicated gold set |
| DGCL-11 | Partial | Content-bound digest, source revision, and failure invalidation plus real Stanza worker pre/post hashes, model manifest, and source/offset binding observed. Independent semantic evidence is missing |
| DGCL-12 | Partial | DG1→RequirementProgramIr candidate→TL edges connected. Stages remain NeedsEvidence/Unresolved and do not prove implementation |
| DGCL-13 | Partial | Per-requirement closure plus source-local target/file hashes and actual-binary checks added. Not an independent check of every target |
| DGCL-14 | Partial | Symbol/route strings and actual CLI output checked against source. Not a complete repository call graph |
| DGCL-15 | Partial | Actual production CLI binary execution and JSON output observed. Final host-output callback remains unconnected |
| DGCL-16 | Partial | Strict evidence evaluator and signed-receipt path exist; this is separate from actual product acceptance suites |
| DGCL-17 | Partial | Individual gap evaluation and duplicate/stale/tamper rejection exist. This does not mean every implementation is closed |
| DGCL-18 | Partial | Exact source revision mismatch is rejected. Full TOCTOU/rebuild lifecycle is not implemented |
| DGCL-19 | Partial | Unchanged state is explicit as `NoProgress`. No closed loop executes automatic repair actions |
| DGCL-20 | Partial | Append-only local checkpoints/resume preserve unknown delivery and suppress duplicate execution. This is not durable host replay |
| DGCL-21 | Partial | Fake epoch progress is blocked. Distillation that receives and updates new real state is not implemented |
| DGCL-22 | Partial | Eight added negative regressions, a connection positive case, six disconnection mutations, and existing tamper tests passed. Not all disconnection classes are covered |
| DGCL-23 | Partial | DG1/closure and real Japanese/English worker/pipeline/evaluation/connection/resume CLIs enter the production binary. Host route remains open |
| DGCL-24 | Not met | Finalizer runs but returns `hold_host_output_unbound`. Exact final host callback is not connected |
| DGCL-25 | Partial | Additive schemas checked as Draft 2020-12 and real mixed-document output validated. Full legacy-consumer matrix not scanned |
| DGCL-26 | Partial | Source/region/node/requirement plus worker input/output/runtime limits exist. OS process-tree isolation is not connected |
| DGCL-27 | Not met | 200 independent holdout documents/language, 1,000 gold obligations/language, and risk-coverage not evaluated |
| DGCL-28 | Partial | Rust CST→DG1→TL→closure CLI and Japanese/English worker→DG1 candidates→connection observation run source-locally. One full-stage chain and host output remain open |
| DGCL-29 | Partial | Doctor and bounded compile/render for units ≤16 KiB completed. `needs_evidence`/`blocked_with_diagnostics` are retained. This is not repository-wide review |
| DGCL-30 | Not met | Required gates remain unmet. Completion decision is `Held` |
| DGCL-31 | Not met | No distribution candidate, final closure, or handoff claim. Not release-ready |

### Validation and residual risk

In addition to the pre-change baseline, targeted DG1/closure/adapter/distillation/CLI tests, `cargo fmt --all --check`, `cargo check --workspace --locked`, `cargo test --workspace --no-default-features --locked`, `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build --release --locked`, and release-binary DG1/closure CLI smoke checks passed. New schemas were checked as Draft 2020-12, and actual mixed-document CLI output passed validation. Test results for this run, including 20,000 generated cases, are observations for the limited profiles above. DGCL-31 acceptance conditions and independent evaluation are not met.

#### Assurance-Compiler advisory review

- Doctor: `ok=true`, semantic digest `2d102a78fdecd3d325bce8c4ef0921fb390e86b7071b7634c1f7ade8823313a`. Doctor reports facade health only.
- Scope was the working-tree source revision (uncommitted changes from base HEAD `60cc71db2ba47517bbd209894033f9b55b3708f8`). Complete modules under 16 KiB were `parser.rs` 14,953 bytes, `semantics.rs` 7,487 bytes, `kernel.rs` 4,137 bytes, and `distillation.rs` 15,163 bytes. Complete in-band function/impl units were also supplied: `analyze_dg1` 10,960 bytes, `extract_requirements` 3,461 bytes, `instruction_markers` 3,399 bytes, `build_coding_closure` 9,046 bytes, BackendRegistry impl 6,616 bytes, `build_output_closure` 8,839 bytes, and `build_syntax` 5,674 bytes.
- Status was `needs_evidence`, 0 diagnostics for parser/semantics/kernel and DG1/requirements/rules/closure/output/syntax units. Full `distillation.rs` returned `blocked_with_diagnostics` (5 diagnostics). BackendRegistry impl returned `blocked_with_diagnostics` (2 diagnostics).
- Distillation render also returned `blocked_with_diagnostics`. Stable IDs and unknown/missing evidence were retained. Larger unreviewed files include `dg1.rs` (36,128 bytes), `adapters.rs` (21,320 bytes), `closure.rs` (23,583 bytes), `integration.rs` (28,500 bytes), and others. Files over 16 KiB were not presented as reviewed through partial fragments.
- These are bounded static review surfaces, not analyzer truth, whole-repository coverage, independent corroboration, runtime safety, semantic correctness, proof, approval, or authority.
- Five additional complete modules (each below 16 KiB) were reviewed after resumption. `dgcl_gold.rs`, `language_worker.rs`, `dgcl_pipeline.rs`, and `closure_connection.rs` returned `needs_evidence`; `closure_resume.rs` returned `blocked_with_diagnostics` (one diagnostic). Details are in the [DGCL local validation record](../validation/dgcl-local-gates-2026-09-28.md).

Major open items are model-derived data redistribution rights, independent gold, complete instruction grammar and arbitrary compound/anaphora/exception-scope behavior, complete call-graph and execution-path tracing for every per-requirement target, host output/finalizer, process-tree isolation, remote CI, and independent host tests. Local checkpoints are not durable host replay. Therefore this work does not claim all DGCL-00..31 complete, complete DeepGrammar parsing, complete implementation closure, general semantic understanding, formal proof, or host-level enforcement.
