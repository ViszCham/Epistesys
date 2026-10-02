# Epistesys 6.3.2-alpha.2 — DGCL実装閉鎖ロードマップ / DGCL implementation closure roadmap

## 日本語

### 0. 固定状態・目的・変更境界

作成日：2026-10-01 JST。対象：Epistesys、現source version `6.3.2-alpha.1`、目標source version `6.3.2-alpha.2`。監査基点HEAD：`60cc71db2ba47517bbd209894033f9b55b3708f8`、作業branch：`dgcl-completion`。監査対象はHEADと既存の未commit・未追跡DGCL差分を合わせたworking treeである。HEADだけを実装snapshotと呼ばない。

この文書は実装前の固定設計であり、slice完了記録ではない。今回保存したのは設計文書だけであり、version・source behaviorは変更していない。実装開始時に全対象fileのcontent digest・toolchain・依存・既存差分を再取得する。2026-09-28の[実装状況](dgcl-implementation-status-2026-09-28.md)と[ローカル検証](../validation/dgcl-local-gates-2026-09-28.md)は履歴的な観測として保持する。

目標は、DeepGrammarのsource/文書領域/構文/指示構造を強化し、要求をProgram IR、TranslationLoss、Coding Completion、検証、最終候補まで一貫して接続することである。対応範囲内の実装経路を正常系でも完走させ、断線・欠落・競合・失効を個別に検出して修復・再開する。独立goldによる意味精度、hallucination containment、長期session、一般性能の実証は後続評価へ置く。実装の構造・実行・接続を確認する試験はalpha.2でも省略しない。

既存source/schema/fixture/docsの差分を保持する。reset、checkoutによる上書き、clean、cache削除、plugin更新、host再起動をこの文書から自動実行しない。実装sliceで必要な変更は後続の実装指示に基づく。commit/push/merge/installはrelease readinessと別の操作である。

### 1. 敵対的監査：現在の穴と設計への反映

判定ラベル：`SourceObserved`はsourceで確認した挙動・構造、`RiskInferred`はその構造から導いた故障可能性、`ReproductionPending`は専用negative testで再現する必要があるもの。P0/P1は本計画内の優先度であり、外部の脆弱性評価ではない。今回、sourceを実行して新しい攻撃ケースを再現したとは主張しない。

| ID / 優先度 | 観測と敵対的指摘 | 修正先 |
| --- | --- | --- |
| F01 / P0 | `issue_connection_evidence`は公開reportの`Observed`やdigestを受けて5種類のreceiptを発行する。callerがreportを構築でき、同じ観測がstatic/runtime/acceptanceの別証拠に増幅される余地がある。`SourceObserved`、API bypass再現は`ReproductionPending`。[source](../scripts/lc631/crates/lc631-analysis/src/closure_connection.rs) | 14–17 |
| F02 / P0 | implementation判定は同名identifier、route判定は同じstring literalの存在である。定義・実呼出し・dataflowと同一視できず、未使用関数や診断文字列でも候補になりうる。`SourceObserved`。[source](../scripts/lc631/crates/lc631-analysis/src/closure_connection.rs) | 15–16 |
| F03 / P0 | `--execute`は明示呼出しの記録だが、plan内の実行体・引数・影響先を束縛するpermitではない。digestが一致する悪意ある実行体を許容しうる。`RiskInferred`。 | 03, 11, 16, 18 |
| F04 / P1 | DG1の抽出・複文分割・scopeは文字列marker中心である。条件全体がbool/enumへ縮み、staging/production以外のscopeや複数actionへの条件配分が落ちる。`SourceObserved`。[source](../scripts/lc631/crates/lc631-tldg/src/dg1.rs) | 07–10 |
| F05 / P1 | Stanza結果の統合は否定語・`advcl`/`mark`の有無中心で、dependency treeから指示の作用域を構成していない。UDの`advcl`は条件専用ではなく、basic `conj`だけでは共有要素や全入れ子を確定できない。`SourceObserved`＋一次資料に基づく制約。[pipeline](../scripts/lc631/crates/lc631-analysis/src/dgcl_pipeline.rs)、[UD advcl](https://universaldependencies.org/u/dep/advcl.html)、[UD conj](https://universaldependencies.org/u/dep/conj.html) | 06–10 |
| F06 / P1 | dependency decoderはID/head範囲を検査するが、self-loop、cycle、root規約、全tokenの被覆、MWTとwordの区別まで閉じていない。`SourceObserved`。[source](../scripts/lc631/crates/lc631-analysis/src/language_worker.rs) | 04–06 |
| F07 / P1 | worker結果とDG1→Program IR→TL→closureは単一のimmutable artifact経路ではない。接続検査もsourceを再parseするため、profile/version差で要求identityが変わりうる。`SourceObserved`。 | 02, 12–13, 26 |
| F08 / P1 | checkpointはhash chainを持つが、再計算・末尾削除・同時writer・途中書込への耐性とfresh head認証が不十分。`ConfirmedSuccess`はcommand観測でありhost deliveryではない。`SourceObserved`＋`RiskInferred`。[source](../scripts/lc631/crates/lc631-analysis/src/closure_resume.rs) | 18–20, 22 |
| F09 / P1 | child終了後のstdin/stdout/stderr thread joinに別deadlineがない。pipeを保持するdescendantによって呼出し全体が停止する可能性がある。`RiskInferred`、再現は未実施。offline環境変数もOS network隔離ではない。[source](../scripts/lc631/crates/lc631-analysis/src/process.rs) | 03–04 |
| F10 / P1 | mutual distillationの各epochは同じkernelをcloneする。`NoProgress`は誠実だが、実際の新evidence・候補・制約stateを次epochへ適用する処理が必要。`SourceObserved`。[source](../scripts/lc631/crates/lc631-tldg/src/distillation.rs) | 20–21 |
| F11 / P1 | finalizerとv1 schemaはhost flagsをfalseに固定している。単純にtrueへ反転しても、受入検査やscope分離にはならない。`SourceObserved`。[CLI](../scripts/lc631/crates/lc631-cli/src/lib.rs)、[schema](../schemas/epistesys-dgcl-surfaces.v1.schema.json) | 22–23 |
| F12 / P1 | goldの`non_hold_coverage`は予測集合が非空かで計算される。実際のcommit/hold決定や要求単位coverageとは異なる。`independent_review_verified`も常にfalseで、独立性の受入経路がない。`SourceObserved`。[source](../scripts/lc631/crates/lc631-analysis/src/dgcl_gold.rs) | 01, 25 |
| F13 / P1 | Python実行体・script・model hashは、全import・native library・processor設定・追加model依存の固定ではない。license URLが非空でも権利確認にはならない。`RiskInferred`。 | 04, 26, 28 |
| F14 / P1 | 必須条件をgoldとhost全体へ一括依存させると、source alphaの実装進捗と未実証研究が混同される。反対に全保留を成功扱いすると正常系が完成しない。`SourceObserved`＋release policy上の問題。 | 01, 17, 24, 28–29 |

友好的な再検討では、lossless source、CST、candidate/authority分離、gapごとのreceipt、`NoProgress`、unknown delivery、offline model利用を維持する。弱い構造を捨てて全面rewriteするのではなく、typed producer・構造grammar・単一artifact・実行観測を既存経路へ接続する。

Assurance-Compiler doctorは`ok=true`。16KiB以下の完全moduleをin-bandで補助監査し、`closure_connection.rs`と`dgcl_pipeline.rs`は`needs_evidence`（diagnostics 0）、`closure_resume.rs`は`blocked_with_diagnostics`（1）だった。input digestは順に`133c8b49bd6b491140eae6535457c9b3eccf7b8ceb90add11ccd30849e3f1003`、`0edb578bdfa7ec4b43dce298fb68d908b333fb8ac03b456b4c9f5d23e8f40db3`、`dd021572ce9425d56d30c7ba31d15778061da6209aed47a9f0cc7608d8e9ef99`。これは限定static補助結果であり、意味精度やrepo全体の結論を与えない。

Grok-Labyrinth-Adapterのoffline statusは`blocked`、理由は`lc631_shadow_contract_failed`（既存LC631 TLDG advisory境界の不整合）。Grok推論callは行っていない。このinstalled dependency状態をEpistesys sourceの実装済み証拠へ転用せず、外部adapter利用の残存条件として記録する。

### 2. alpha.2の完成定義と後続実証

次の3軸を別fieldとして保持する。既存DGCL-30/31の履歴的`Held`を、alpha.2の新profileが通っただけで書き換えない。

1. **ImplementationClosure**：宣言したfeature集合のproducer→consumer→validator→candidate outputが実装され、正常系が完走し、故障時に個別の欠落を報告する。alpha.2の必須条件。
2. **ResearchEvaluation**：独立gold、意味精度、risk–coverage、一般性能、長期sessionの測定。現状態は`PendingNoCorpus`等であり、alpha.2のsource releaseを妨げない。元のgold閾値は後続評価targetとして維持する。
3. **HostObservation**：precommit candidate、post-send callback、sink delivery、durable replayを個別に保持する。standalone CLIの完成とhost全体の利用観測を分ける。host deliveryを要求したrunでは証拠欠落をhard holdし、standaloneへ黙って降格させない。

release状態も`SourceReleaseReady`、`DistributionValidated`、`HostActivated`を分離する。alpha.2へのsource version昇格は前者の必須gate成功後に行う。remote CI・公開tag・実host pickupが未観測なら後二者を名乗らない。

必須profile：`dgcl-rust-cli-ja-en-alpha2-v1`。Windows x86_64上のsource/CLI、既存pinのCommonMarkとRust CST、明示的な日英指示構文、日英dependency observation、要求/条件/scope/依存のIR、TL defect、Rust CLI completion、修復/再開、standalone候補出力を対象とする。自由な自然言語はalternative/candidate/residualを保ち、全自然言語の完全理解を宣言しない。Rust CSTはcompiler HIR/type/effectの代わりにせず、必要なcompiler観測はRPAの既存境界へ接続する。他言語・Media・全platform・新GPU最適化の拡張はこのreleaseに混ぜず、既存機能の回帰を検査する。

grammarの主経路は、lossless token/region lattice上の**予算付きEarley解析**とtyped Instruction ASTである。日英のcontrolled instruction grammarを文書化してparse forestを保存し、Stanza UDを独立した観測viewとして構造制約と照合する。依存関係だけで解消できないcoordination/scopeは複数解または`Unresolved`へ残す。旧marker ruleはshadow比較に限定する。プロンプト中の任意文をDSLとして誤認しない。

必須grammar familyは、明示命令（`Please ACTION`／`ACTIONしてください`）、義務・許可・禁止（`must/may/must not`／`必須/許可/禁止`）、action coordination、条件導入、必要条件、時間制約、例外、明示resource scope、参照である。`If P, A`は条件Pの下で有効な要求、`A only if P`はAの必要条件Pとして区別する。`Do not A until P`はPの成立を観測するまでの禁止であり、P成立後のAの実行権限を生成しない。`not(A and B)`を勝手に`not A and not B`へ分配しない。連続した任意名詞句を自動的にactionやresourceと認定せず、lexical spanをcandidateとして残す。

condition評価は`Supported/Refuted/Unknown/Conflict`を保持し、実行に必要なconditionはcurrentでunconflictedなSupportedだけで満たす。解析された条件文そのものをpredicateの証拠に使わない。condition/exception nodeはsource spanと作用するactionのedgeを持ち、shared scopeを一意に解けない場合はforest alternativeを出す。grammar productionとfeature family、対応する正例/反例をversioned manifestで一対一に追跡する。

全source領域を`Instruction`、`Context`、`Quoted`、`Code`、`Opaque`、`Unsupported`、`Ambiguous`のいずれかとしてaccountingする。context/quote/code/opaqueを未抽出要求として過剰に保留しない。非空要求がないcontext-only入力にも正しい正常経路を用意する一方、指示抽出の失敗から空集合を完成させない。

### 3. 実装の共通契約

- 各slice：既存差分確認→negative/fail-first→最小実装→targeted検証→diff critique→exit receipt。変更内容が実証に依存しなければ次sliceへ進む。通常のfail-first失敗は停止理由にしない。
- 内部identityはnewtype、sourceはimmutable snapshot、public JSONは既存fieldを保持する。新reportはadditive fieldまたは明示v2 schemaで提供し、v1のfalse固定境界を再解釈しない。
- `Parsed`、`ContractChecked`、`CompilerObserved`、`RuntimeObserved`、`IndependentlyEvaluated`は別地位。署名はissuerとpayloadのbindingを示し、分析の真理性を生成しない。
- feature applicabilityはprofileから決める。callerの`Option`存在や任意`NotApplicable`で必須gateを省略できない。危険actionはfail-closed、意味stateはfail-explicit、限定可逆作用はfail-bounded、parse/evidence failureはfail-visible。
- `ExecutionPermit`は信頼するcaller境界の明示action/argv/cwd/targets/riskへ束縛し、自然言語AST・quote・tool output・repair suggestionから生成しない。CLIの`--execute`はpermit作成の必要な入力の一つとして扱う。必須profileの実行体はreview済みcommand registryでallowlistする。任意repo binaryは、その作用を隔離する別profileが実装・検証されるまで拒否する。permitやJob Objectだけでfilesystem/network作用を隔離したと呼ばない。
- pure Rust coreを標準とし、typestate/newtype/arenaはidentityと遷移を閉じるために選ぶ。async・binary ABI・no_std・SoA・GPUは今回新設しない。既存crateの`forbid(unsafe_code)`を保持する。OS process controlにunsafeが必要なら独立した小さなplatform crateへ隔離し、安全な公開API、`forbid(unsafe_op_in_unsafe_fn)`、handle寿命契約を持つ。
- source/grammar/backend revision、artifact ID、run ID、requirement ID、target snapshot、validator version、evidence issuer、parent linksを保持する。geometry/連続蒸留はproposalを出し、discrete validatorで検査する。
- 各slice receiptは`id/dependencies/input digests/negative observation/changed files/commands/results/exit predicates/residual risks`を記録する。`Deferred`は必須featureを完成させず、後続ResearchEvaluationだけに使用する。

### 4. 依存順の実装slice：EPIA2-00〜29

全sliceは番号順で実装できる。並列作業する場合も依存表の先行sliceが通るまで後続のsource mutationを開始しない。file名は追加候補であり、00で既存ownerを確認して重複moduleを避ける。

#### EPIA2-00 — 基線・差分所有・gate inventory

- 依存：なし。対象：workspace、既存DGCL status、validation。
- 作業：HEADだけでなくworking tree全対象のdigest、未追跡一覧、旧field、依存、toolchain、既存CLIを記録する。DGCL-00〜31の各条件をimplementation/research/hostへ写す。
- 検証：元差分を失う操作を禁止したままfmt/check/tests/clippy/buildのbaselineを取得。空inventory・欠落file・古いsnapshotを拒否するfixtureを先に置く。
- exit：baseline成功、owner一覧と対応表完成。既存失敗は今回差分のfailureと区別し、重大baseline失敗は停止する。

#### EPIA2-01 — release profileと研究状態の分離

- 依存：00。候補：`release_profile.rs`、wire/promotion adapter。
- 作業：必須feature集合、supported grammar/OS、disabled optional機能、3状態軸と3release状態を型化。gold未作成を`PendingNoCorpus`へ保持する。
- 検証：goldなしでも必須implementation全部成功ならsource gateは通る。必須未接続、全hold、空feature集合、caller指定のN/Aでは通らない。
- exit：既存DGCL gateは変更せず、alpha.2専用判定とclaim scopeが追加される。

#### EPIA2-02 — immutable sourceと単一artifact identity

- 依存：01。候補：core newtype、`dgcl_artifact.rs`、DG1 adapter。
- 作業：`SourceRevision/GrammarRevision/ArtifactId/RequirementId/RunId/TargetRevision`、parent chain、parse snapshotを導入。IDは単なる抽出順に依存させない。evidence用途のcanonical serializationを固定する。
- 検証：UTF-8中間、source変更、同文別profile、異なるartifactからのID混入、duplicate IDを拒否。同snapshotのencode/decodeはidentityを保持する。
- exit：後続stageがraw sourceを再parseせず、同じartifactを借用できる。

#### EPIA2-03 — process supervisorと全I/O deadline

- 依存：02。対象：`process.rs`、platform capsule。
- 作業：独立platform crateまたはreview済みsafe dependencyでWindows Job Objectへ開始前にprocessを所属させ、spawn raceを閉じる。process/pipe/writerの全体deadline、output/memory/process上限、cancel、cwdとenvironmentを明示する。Job Objectをnetwork sandboxと呼ばない。
- 検証：descendantがpipeを保持、stdin不読、output flood、child早期終了、cancel/timeoutのnegative helperを別processで実行し、親呼出しが上限内に戻る。
- exit：timeoutで残存processとpending readerを検出・終了できる。隔離不能platformでは対象実行を拒否し、無限join fallbackを置かない。

#### EPIA2-04 — 日英backend runtimeと再現可能な設定

- 依存：03。対象：`language_worker.rs`、Python worker、model/runtime manifest。
- 作業：Python・package/wheel/native依存・processors/package/model closureをmanifest化。明示モデルpathとoffline設定を使い、isolated import環境を検査する。download/provisionは別操作にする。利用条件はfileごとに記録しweightsを配布しない。
- 検証：違うpackage、追加import、欠落model、manifest偽装、bad framing、tool不在、deadline、空応答を検査。real JA/EN worker観測をconfigured-runtime profileで記録する。
- exit：configured runtime正常系と未導入時の明示Unavailable経路が動く。CIのsnapshot fixtureを実モデル実行証拠と混同しない。

#### EPIA2-05 — 文書領域・mixed language・lossless accounting

- 依存：02, 04。対象：DG1 regions/source map。
- 作業：Markdown list/heading/quote/code/HTML/tableとhost originを保持。混在文をlanguage span latticeで扱い、synthetic batchの境界を越える依存を拒否する。
- 検証：inline quote、code内命令、HTML命令、日英同paragraph、CRLF、combining character、raw string、sourceとbatchで異なるoffsetを検査。
- exit：全byteが原sourceに戻り、各領域の役割と未対応理由がaccountingされる。領域roleだけからauthorityを生成しない。

#### EPIA2-06 — UD観測の構造validator

- 依存：05。候補：`dependency_graph.rs`。
- 作業：token/word/MWT/empty nodeを分離し、sentence-local IDs、root規約、head存在、self-loop/cycle、span順序/被覆、features、relation subtypeを検査する。許可する非木viewは別profileへ明示する。
- 検証：cyclic/headless/multi-root/重複/逆順/region越境/未被覆をrejectまたはprofile規約のexplicit residualへ出す。
- exit：構造検査済みUD viewがtyped graphとしてgrammarへ渡る。syntactic relationを論理条件と同一視しない。

#### EPIA2-07 — 予算付きinstruction grammar / parse forest

- 依存：06。候補：`instruction_grammar.rs`、`instruction_ast.rs`、versioned grammar table。
- 作業：日英imperative、deontic modality、negation、coordination、condition、exception、referenceをtyped ASTへ解析するEarley coreを実装。active items、forest nodes、depth、token数を制限する。自由文とcontrolled grammarのprofileを区別する。
- 検証：曖昧入力を最初の解に固定しない、grammar外を黙って受理しない、budgetで切り捨て成功しない。明示grammar全productionの到達・正例・反例を検査する。
- exit：normal pathがAST/forestを実体化し、旧marker抽出はshadow差分へ降ろす。geometry scoreはparse妥当性を決めない。

#### EPIA2-08 — action別coordination・polarity・modality

- 依存：07。対象：grammar lowering。
- 作業：共有subject/object、actionごとのMust/Should/MayとForbidden、negation scope、and/or/xorを保存する。共有が曖昧なら複数candidateを残す。
- 検証：`テストを実行し、公開しない`、`Do not delete or publish`、`You may test but must not publish`、二重否定、quoted negationを最小対で検査する。
- exit：一文に複数要求を保持し、Forbiddenの検証は「禁止作用が発生しない」testへ接続する。禁止文を実行要求へ変えない。

#### EPIA2-09 — condition・時間・例外の式

- 依存：08。候補：`condition_ir.rs`。
- 作業：条件を`All/Any/Not/Predicate`、必要条件/十分条件、before/after/untilの時相関係、exception precedenceとして保持する。condition truthは外部のcurrent evidenceが必要で、文字列存在では成立しない。
- 検証：only-ifとif、unless、例外の例外、nested条件、未観測predicate、時間順序の逆転でunconditional化しない。
- exit：各actionからcondition/exception ASTへのsource-bound linkがあり、unknown/conflictを評価器とTLへ渡せる。

#### EPIA2-10 — scope・参照・依存と未抽出accounting

- 依存：09。候補：`requirement_graph.rs`。
- 作業：resource/action/branch/environment/time scopeを型化し、参照候補、requires/before/conflicts edgeを保持。cycleは理由と関与要求を示す。要求候補を検出しながら完全には解析できない領域をresidual ledgerへ置く。
- 検証：stagingとproductionのscope移動、`それ`の複数参照、指示の欠落、同文duplicate、循環、context-only正常入力、無関係説明で過剰holdしない。
- exit：全operative候補にASTまたはresidualがあり、個別要求の作用域が明示される。

#### EPIA2-11 — caller/host authority境界

- 依存：10。対象：core/host authority、execution adapter。
- 作業：principal、user span、action scope、revision、期限、revocationをpermitへ束縛する。parsed modalityと実行権限を分離。standalone CLI callerとhost-attested callerを別originとして扱う。registry外binaryと未隔離の任意actionは、planにhashがあっても実行対象にしない。
- 検証：assistant/tool/repo/quote/code/完了報告からのGrant、scope拡張、Deny消失、古いpermit、同名principal偽装、repair経由の権限増幅を拒否する。
- exit：安全なdeclared commandの正常permitがあり、permitなし・scope外の実行が閉じる。研究評価結果はpermitを変えない。

#### EPIA2-12 — DeepGrammar→Program IR→TLの単一経路

- 依存：11。対象：`integration.rs`、`dgcl_pipeline.rs`、TL adapters。
- 作業：同artifactから要求・条件・source anchors・parse alternatives・UD viewをProgram IRへlowerし、各projectionの保全/欠落/追加/比較不能をTL graphへ記録する。legacy raw-source再parseを新正常経路から除く。
- 検証：要求/否定/条件/scope/anchorを一つずつ落とすmutationが対応defectになる。同shape別payloadや別sourceをdigest countで同一視しない。
- exit：一つのCLI artifactで各stageのparent/input/output digestを追跡できる。構造保存と意味実証の状態は別のまま。

#### EPIA2-13 — obligationとcompletion plan

- 依存：12。対象：closure contract、target manifest。
- 作業：要求ごとにimplementation、connection、static check、runtime check、acceptanceの適用条件とtargetを列挙する。Forbidden/非実行要求には適切なabsence/static検証を割り当てる。
- 検証：必須gap一つ欠落、任意N/A、関係ないtest、unknown requirement、duplicate回答、condition不成立でも完成するケースを拒否する。
- exit：全要求の完成に必要な証拠の種類・producer・受入条件が先に固定される。

#### EPIA2-14 — evidence producerとreceipt発行の非増幅

- 依存：13。対象：`closure_connection.rs`、receipt kernel adapters。
- 作業：公開reportからreceiptを発行する経路を閉じる。検査済み観測tokenのconstructorをprivateにし、evidence kindごとにproducer・policy・exact payloadを検査する。一つの観測から別kindを自動生成しない。
- 検証：caller作成Observed report、署名済みwrong-kind、wrong-run、同issuerの自己整合だけ、scope/digest差、replay、期限切れをrejectする。
- exit：構造検査はStructuralObservation、compiler実行はCompilerObserved、run traceはRuntimeObservedとして発行され、個別gapへ対応する。

#### EPIA2-15 — static connection graph

- 依存：14。候補：`connection_graph.rs`、RPA adapter。
- 作業：CSTの定義・call expression・module/routeのbindingを解析し、必要なcross-module/type resolutionは明示compiler/RPA入力へ送る。opaque macro/dynamic dispatchはunknownを残す。全repository graph完成は要求せずdeclared target pathを閉じる。
- 検証：未使用関数、同名local、dead route文字列、間違ったcallee、macro opaque、別feature/targetでroute不在を検出する。
- exit：declared producer→consumerのstatic pathと未解決edgeを実体化し、identifier存在だけではgapを閉じない。

#### EPIA2-16 — runtime traceと受入oracle

- 依存：15。候補：`connection_runtime.rs`、CLI trace adapter。
- 作業：run ID、request/requirement ID、実行体/入力/環境、producer/consumerの実到達、出力digestをtraceへ束縛する。開発仕様から作るexpected oracleをcandidateの自己生成結果から分離し、artifact contractを照合する。
- 検証：誤route、stale executable、trace欠落/偽造/重複、wrong input、別runの正しいJSON、oracle改変、scope外writeを検査する。
- exit：許可したdeclared正常経路の到達と受入結果を観測し、未到達pathは閉じない。trace発行体のtrust boundaryを記録する。

#### EPIA2-17 — 個別gap closureと正常系finalization

- 依存：16。対象：closure/finalization。
- 作業：全要求ごとの適用gapを検査し、完了/未完了/矛盾/失効を集約する。対応profile正常入力の`ImplementationClosed`と不足時の`Hold/Clarify`を両方実装する。
- 検証：一つの失敗が別要求の証拠を消さない、全holdでrelease成功しない、empty指示失敗がvacuous closureにならない、context-onlyは正常として扱う。
- exit：source-local正常runが実装完了candidateを返す。output actionやhost deliveryの許可は別判定へ渡す。

#### EPIA2-18 — source/build/evidence lifecycle

- 依存：17。対象：snapshot manager、execution preflight。
- 作業：immutable build入力・target snapshot・leaseを使い、source/build/profile/validator/authority変更時に関与evidenceを失効する。前後hash照合は改変検出であり実行前隔離の代替にしない。
- 検証：検査後のfile差替え、reparse point、build feature変更、モデル変更、source一箇所変更、permit revocation中の実行を検査する。
- exit：stale evidenceが再利用されず、隔離できない危険作用は実行前に拒否する。host管理者攻撃への絶対耐性は名乗らない。

#### EPIA2-19 — crash-consistent checkpointとresume

- 依存：18。対象：`closure_resume.rs`、receipt/replay adapters。
- 作業：typed event、authenticity binding、write transaction、flush、exclusive writer、head/freshness、partial-write回復を実装する。action実行、candidate送信、sink deliveryのstateを分離する。
- 検証：各書込段階crash、two writers、末尾削除、digest再計算、権限失効、unknown delivery、replayed old headを検査する。信頼headがなければrollback確認はUnknownに残す。
- exit：resumeは検査済みstateを再構成し再検証へ進めるが、古い権限や未確認deliveryを復元しない。

#### EPIA2-20 — 最大4回の実修復ループ

- 依存：19。候補：`repair_loop.rs`。
- 作業：gap→typed RepairRequest→authorized adapter→新snapshot→対象再検証→全closureのstate machineを実装する。最大4回、wall/resource budget、oscillation/NoProgressを保持し、修復候補だけではstateを更新しない。
- 検証：実際の欠落route/条件projectionを直す正例、空diff、同じdigest、権限外edit、cycle、4回超、途中crash、検証失敗を検査する。
- exit：少なくとも2種の故障で閉じたrepair→revalidate経路を観測する。commit/push/mergeはrepair action集合に含めない。

#### EPIA2-21 — 実stateを更新する離散↔連続蒸留

- 依存：20。対象：`distillation.rs`、kernel/integration。
- 作業：geometry proposalを離散constraint validatorへ渡し、受理された新evidence/constraint graphを次epoch inputへ適用する。directional loss、NotComparable、conflictは個別保持し、distanceだけでtruthやauthorityへ昇格しない。
- 検証：実graph変更の正例、同state NoProgress、fake epoch、未検証edge、geometry順位とconstraint矛盾、CPU/GPU既存pathの回帰を検査する。
- exit：入力と出力のcontent stateが変わったepochだけprogressを記録する。新GPU性能・占有率claimをrelease条件に混ぜない。

#### EPIA2-22 — standalone outputとhost adapter

- 依存：21。対象：CLI finalizer、host binding/replay。
- 作業：standalone candidateのexact bytes/schema/requirement realizationをcommit前に検査する。host adapterはprecommit/postsend/delivery/replayの別APIとtrusted ingressを持ち、fileやstdin自己申告をhost観測へ自動昇格しない。
- 検証：standalone正常finalization、候補差替え、任意callback JSON、別turn、旧receipt、replay、unsupported hostを検査。host要求runはcallback欠落でholdする。
- exit：standalone経路は閉じ、host正常/異常adapterはcontract fixtureで検査可能。実host未観測は`PendingHostObservation`として残す。

#### EPIA2-23 — additive schemaとlegacy consumer

- 依存：22。対象：wire/schema、旧snapshot。
- 作業：v1 fieldと意味を保持し、必要な成功stateをv2 reportへ追加する。新内部型を明示wire adapterで変換し、未知enum/fieldのhandlingをconsumerごとに決める。
- 検証：旧fixtureのfield/type/意味、旧consumerでの新report拒否/限定view、strict解析、source/condition AST roundtrip、互換bool非増幅を検査する。
- exit：legacyは新成功を勝手にhost permissionへ読み替えず、新consumerは定義した正常stateを利用できる。

#### EPIA2-24 — 敵対matrixと過剰保留の検査

- 依存：23。候補：`tests/epia2_*`、明示dev fixture。
- 作業：下記16断線とgrammar全feature familyを検査する。各familyに少なくとも3正例・3反例、boundary/cross-feature caseを追加。fixtureは仕様作成のdev契約であり独立goldと呼ばない。
- 検証：元正例成功→一箇所切断で対象gapだけ開く→修復で同source要求が戻る。unsupported/ambiguous入力は説明付きhold、対応正常入力はalways-holdにならない。
- exit：16断線が全て検出され、全宣言featureにpositive pathがある。試験件数だけで意味性能を主張しない。

#### EPIA2-25 — 後続実証を受け入れる評価API

- 依存：24。対象：`dgcl_gold.rs`、research status/provenance。
- 作業：実decisionに基づくcoverage、要求単位recall、誤受理/誤保留、risk–coverageの分母を定義する。旧非空予測metricは名前を保持し説明を追加。trusted annotator registry/署名/裁定digestの受入経路を型化する。
- 検証：goldなしPending、欠落分母、negative-only文書、family/content leakage、偽ID/署名、独立裁定receiptをcontract検査する。test identityは実human独立性の証拠ではない。
- exit：実gold投入時に測定・provenance検査できるAPIが完成する。日英200文書/1,000要求、既存精度target、一般性能評価は未実施のまま明示する。

#### EPIA2-26 — production CLI・packageからのend-to-end

- 依存：25。対象：CLI/router、launcher/package manifest。
- 作業：一つのsupported commandでsource→region→grammar/UD→requirements→Program IR→TL→closure→standalone outputへ到達する。別cwd/space path/offline起動、configured-modelとno-model profileを検査する。
- 検証：launcherだけ存在、古いbinary、debug/release差、packageに必要source欠落、モデル暗黙download、未知command、backend未導入を検査。configured-model正常runは実workerを用いる。
- exit：source-only packageからfresh buildし正常系を再現できる。モデル/receipt/cache/私有pathをpackageへ含めない。

#### EPIA2-27 — 敵対的再監査・Assurance・RepoSeiri

- 依存：26。対象：最終diff、docs/readme、残存risk。
- 作業：F01〜F14を再監査し、各所のtrust/authority/evidence/semantic boundaryを追う。Assuranceは完全単位≤16KiB、RepoSeiri 1.1は同snapshotでsummary/routes/linterを確認する。
- 検証：安全性を回数・signature・hash・低lossだけで主張する文言、日英claim差、未接続を実装済みとする記述、risk ledger未登録を検出する。
- exit：対応profileのactionable P0/P1はforward fixして対象gateを再実行。Assuranceの未知/未対応は別記録し、repository-wide proofに変えない。

#### EPIA2-28 — 昇格前の全functional/source gate

- 依存：27。対象：release report、verification record。
- 作業：次節のA2-G00〜13を現snapshotで評価する。全必須featureのImplementationClosureを揃え、ResearchEvaluation/HostObservationのpendingを正確に記録する。
- 検証：fmt/check/no-default/default/clippy/release、targeted/schema/property/mutation、実JA/EN configured worker、fresh-package standalone end-to-endを実行。remote CIは観測した場合だけ別receiptにする。
- exit：versionはalpha.1のまま`PrePromotionFunctionalPass`。gold未作成はこのsource gateを妨げないが、必須経路失敗は妨げる。

#### EPIA2-29 — alpha.2 metadata昇格・再検証・引渡し

- 依存：28。対象：Cargo/lock、core identity、manifest/launcher、compatibility/schema docs、README/docs/release/verification record、AGENTSのidentity参照。
- 作業：owner hunkのみを`6.3.2-alpha.2`へ一括整合し、A2-G14として全必須gateを再実行する。既存schema versionはpackage versionと連動して無理由に変更しない。
- 検証：source/package/executable identityの不一致、古いfixture、fresh build/release経路、日英同内容、私有情報混入を検査する。
- exit：成功時のみ`6.3.2-alpha.2 SourceReleaseReady`。失敗はforward fix、必要なら所有metadata hunkだけapply_patchで戻す。戻せなければunverified candidateと記録する。commit/push/merge/installの実行は後続の明示指示と各operation gateで行う。

### 5. release gate・断線matrix・停止条件

| Gate | 必須条件 |
| --- | --- |
| A2-G00 | baseline、所有差分、content snapshot、依存inventoryが存在し整合 |
| A2-G01 | lossless source、region/role、identity、UTF-8、budgetが全対応入力で検査可能 |
| A2-G02 | grammar全宣言featureの正例・反例・ambiguity・非対応を検査し、正常系が通る |
| A2-G03 | authorityはtrusted caller originへ束縛され、全projectionでscope/risk/Grantを増幅しない |
| A2-G04 | worker/configured model正常系、invalid framing、timeout、descendant/pipe、明示Unavailableを検査 |
| A2-G05 | 同artifactのrequirements→Program IR→TL→closure→candidateを一貫して追跡し、断線を検出 |
| A2-G06 | evidence kindに合う検査済みproducer、source/run/target/issuer binding、偽report/replay拒否 |
| A2-G07 | 16断線が対応gapを開き、修復後に同要求の正常経路が戻る |
| A2-G08 | evidence失効、checkpoint crash/concurrency、resume、最大4回repair、NoProgressが検査可能 |
| A2-G09 | discrete↔continuousで実state更新を観測し、未検証/NotComparable/conflictを保持 |
| A2-G10 | standalone実candidateが検査済み正常結果を返す。host要求runは別gateを満たすかhold |
| A2-G11 | v1互換、v2 negotiation、旧consumer bool境界、schema roundtripを検査 |
| A2-G12 | 現snapshotのRust全gate、configured JA/ENとfresh-package正常起動、配布privacy/依存記録 |
| A2-G13 | sourceで確認した対応profileのP0/P1を修正し、日英docs・残存risk・監査receiptを整合 |
| A2-G14 | alpha.2 identityでA2-G00〜13を再評価し、source release readyを実receiptで記録 |

必須16断線：source anchor欠落、grammar/profile差替え、quoted origin昇格、polarity消失、condition消失、scope拡張、requirement欠落、Program IR→TL辺欠落、implementation binding誤り、static call edge切断、runtime producer/consumer未到達、受入oracle差替え、evidence kind/issuer偽装、stale build/model/source、checkpoint欠落/重複/巻戻し、candidate/host callback digest差替え。各断線は該当input identity・期待gap・観測gap・復旧結果を記録する。

実装時の基本commandは`cargo fmt --all --check`、`cargo check --workspace --locked`、`cargo test --workspace --no-default-features --locked`、`cargo test --workspace --locked`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo build --release --locked`。slice内は必要なtargeted/property/schema/mutation/process helperを追加する。unsafe/platform capsuleではその境界に適した検査を追加する。

重大baseline失敗、既存差分との実質的衝突、public互換性破壊、source/span破壊、実行authority不明、必須source gate検証不能で停止する。gold不在、独立研究未実証、disabled optional host未観測はsource releaseの停止理由にしない。要求されたhost deliveryなどmandatory capabilityをdisabled扱いに変えて通さない。内部的に整合するだけのテストを意味精度・一般信頼性へ昇格しない。

### 6. Rust interface断片と実装上の禁止事項

以下は将来実装のinterface設計断片であり、現sourceへ追加済みではない。実装時は新moduleとしてcompileし、既存型へのadapterとnegative testを付ける。未定義型を残して「コード完成」と記録しない。

```rust
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct RequirementId(u64);

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ObservationState {
    Missing,
    Candidate,
    ContractChecked,
    Refuted,
    Conflict,
}

pub enum ConditionExpr {
    Predicate(PredicateId),
    All(Vec<ConditionId>),
    Any(Vec<ConditionId>),
    Not(ConditionId),
    Temporal(TemporalRelation),
}

pub struct RequirementNode {
    pub id: RequirementId,
    pub source: SourceAnchor,
    pub action: ActionExpr,
    pub modality: Modality,
    pub polarity: Polarity,
    pub conditions: Vec<ConditionId>,
    pub exceptions: Vec<ConditionId>,
    pub scope: ScopeExpr,
    pub alternatives: Vec<AlternativeId>,
}
```

`PredicateId/ConditionId`は同artifactのarenaに属するnewtypeとして定義する。conditionのboolへの早期圧縮、scopeの任意文字列推測、Forestの最初の候補採用を禁止する。

```rust
// Constructors and raw observations stay inside the producer module.
pub struct CheckedRuntimeObservation {
    run: RunId,
    artifact: ArtifactId,
    binding: ExecutionBinding,
    trace: CheckedTrace,
}

impl RuntimeEvidenceIssuer {
    pub fn issue(
        &self,
        observation: &CheckedRuntimeObservation,
        requirement: RequirementId,
        policy: &RuntimeEvidencePolicy,
    ) -> Result<RuntimeEvidenceReceipt, EvidenceError> {
        policy.check_exact_binding(observation, requirement)?;
        self.sign_runtime_payload(observation, requirement)
    }
}
```

公開JSON reportやcaller指定boolから`CheckedRuntimeObservation`をdeserializeしない。`RuntimeEvidenceReceipt`からstatic/acceptance receiptを自動作成しない。self-attested host eventもlocal callback candidateとして保持する。

```rust
pub enum ResearchStatus {
    PendingNoCorpus,
    PendingIndependentReview,
    Evaluated(ResearchEvaluationReceipt),
}

pub enum OutputScope {
    StandaloneCandidate,
    HostPreCommit,
    HostPostSend,
    SinkDelivery,
}

pub fn alpha_source_gate(
    profile: &CheckedReleaseProfile,
    results: &FunctionalGateLedger,
) -> Result<SourceReleaseReady, Vec<GateFailure>> {
    let failures = results.check_all_required(profile);
    if failures.is_empty() {
        SourceReleaseReady::checked(profile, results)
            .map_err(|failure| vec![failure])
    } else {
        Err(failures)
    }
}
```

`CheckedReleaseProfile`は非空feature集合とscopeを検査し、`SourceReleaseReady::checked`はrequired gateのcompleteness/freshness/identityを再確認する。ResearchStatusは別fieldへ含めるがこの関数から評価済みに変えない。SourceReleaseReadyはMutationAuthorityやoutput action permitではない。

### 7. 後続実証と残存risk

後続ResearchEvaluationは、独立goldの収集・裁定・identity確認、言語ごと200 holdout文書/1,000要求、precision 99%/recall 98%/文書exact・coverage 95%/critical error 0という元target、family分離、誤保留/risk–coverage、複雑条件・長期sessionのpaired評価を保持する。[gold収集手順](dgcl-gold-collection-protocol.md)を使用する。閾値は現実装の達成値ではない。

ResidualRiskLedgerの必須項目：自由自然言語・未対応grammar、model誤解析、未実証意味精度、human provenance、multilingual参照、動的dispatch/opaque macro、host ingress/delivery、local key/OS管理者trust、rollback freshness、依存の再配布権、remote CI、Assurance未対応、既存GPU hardware観測。各項目にowner・影響scope・現在状態・次の必要証拠を置く。

alpha.2の説明は「日英/RustのDGCL構造・要求・証拠・検証・修復・候補出力を、宣言profileで実行できる実験的実装」。完成claimは`ImplementationClosedForDeclaredProfile`へ束縛する。「hallucinationを防止した」「任意指示を完全理解する」「host全体を強制制御する」「全弱点ゼロ」は使用しない。READMEは実際に追加した機能を明示し、研究効果が後続評価であることを同じ強さで日英に書く。

### 8. 一次資料による設計制約

- [Stanza pipeline/processors](https://stanfordnlp.github.io/stanza/pipeline.html)：tokenとword/MWT、processor依存、設定を分けるために参照。実行観測を命令意味の検証へ自動昇格しない。
- [UD coordination](https://universaldependencies.org/u/dep/conj.html)、[UD adverbial clause](https://universaldependencies.org/u/dep/advcl.html)：coordination共有/入れ子の表現限界と、advclの複数用途を踏まえ、独立したscope/condition ASTを設計する。
- [Microsoft Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)：関連processをまとめて管理する境界として参照。network隔離や管理者攻撃耐性は別条件。
- [Semantic Versioning](https://semver.org/)：`6.3.2-alpha.2`はalpha.1から進めるpre-release identity。package versionと研究効果の達成を同一視しない。

## English

### 0. Fixed status, purpose, and change boundary

Created: 2026-10-01 JST. Target: Epistesys, current source version `6.3.2-alpha.1`, intended source version `6.3.2-alpha.2`. Audit base HEAD: `60cc71db2ba47517bbd209894033f9b55b3708f8`; working branch: `dgcl-completion`. The audited snapshot combines HEAD and the existing uncommitted/untracked DGCL working-tree changes. HEAD alone is not the implementation snapshot.

This is a fixed design before implementation, not a slice-completion record. Only this design document was saved; version and source behavior were not changed. At implementation start, refresh content digests for all target files, toolchain, dependencies, and existing changes. Retain the 2026-09-28 [implementation status](dgcl-implementation-status-2026-09-28.md) and [local validation](../validation/dgcl-local-gates-2026-09-28.md) as historical observations.

The goal is to strengthen DeepGrammar source, document regions, syntax, and instruction structure, then connect requirements consistently through Program IR, TranslationLoss, Coding Completion, validation, and the final candidate. Supported implementation paths must also complete successfully; disconnections, omissions, conflicts, and invalidation must be individually detected, repaired, and resumed. Independent-gold semantic accuracy, hallucination containment, long sessions, and general-performance evaluation remain later work. Tests of implementation structure, execution, and connectivity remain mandatory for alpha.2.

Preserve existing source/schema/fixture/docs changes. This document does not automatically execute reset, checkout overwrites, clean, cache deletion, plugin updates, or host restart. Implementation changes follow a subsequent implementation instruction. Commit/push/merge/install are operations separate from release readiness.

### 1. Adversarial audit: current gaps and design response

Labels: `SourceObserved` identifies behavior or structure inspected in source; `RiskInferred` identifies a possible failure inferred from that structure; `ReproductionPending` requires a dedicated negative test. P0/P1 are priorities within this plan, not an external vulnerability rating. No newly executed attack reproduction is claimed in this design audit.

| ID / priority | Observation and adversarial criticism | Repair slices |
| --- | --- | --- |
| F01 / P0 | `issue_connection_evidence` accepts a public report's `Observed` state and digests and issues five receipt kinds. A caller can construct the report, and one observation can be amplified into separate static/runtime/acceptance evidence. `SourceObserved`; reproducing an API bypass is `ReproductionPending`. [Source](../scripts/lc631/crates/lc631-analysis/src/closure_connection.rs) | 14–17 |
| F02 / P0 | Implementation detection finds a same-named identifier; route detection finds a string literal. These do not establish a definition, executed call, or dataflow; an unused function or diagnostic string can become a candidate. `SourceObserved`. [Source](../scripts/lc631/crates/lc631-analysis/src/closure_connection.rs) | 15–16 |
| F03 / P0 | `--execute` records an explicit invocation but is not a permit binding the executable, arguments, and affected targets in the plan. A malicious executable with a matching supplied digest may be accepted. `RiskInferred`. | 03, 11, 16, 18 |
| F04 / P1 | DG1 extraction, compound splitting, and scope rely on string markers. Conditions collapse into a bool/enum; scopes beyond staging/production and distribution of conditions across actions can be lost. `SourceObserved`. [Source](../scripts/lc631/crates/lc631-tldg/src/dg1.rs) | 07–10 |
| F05 / P1 | Stanza integration largely detects negation words and `advcl`/`mark`, rather than deriving instruction scope from dependency structure. UD `advcl` is not exclusively conditional, and basic `conj` does not resolve all sharing/nesting. `SourceObserved` with primary-source constraints. [Pipeline](../scripts/lc631/crates/lc631-analysis/src/dgcl_pipeline.rs), [UD advcl](https://universaldependencies.org/u/dep/advcl.html), [UD conj](https://universaldependencies.org/u/dep/conj.html) | 06–10 |
| F06 / P1 | The dependency decoder checks ID/head ranges, but does not fully close self-loops, cycles, root conventions, token coverage, or distinctions between MWTs and words. `SourceObserved`. [Source](../scripts/lc631/crates/lc631-analysis/src/language_worker.rs) | 04–06 |
| F07 / P1 | Worker results and DG1→Program IR→TL→closure do not share one immutable artifact path. Connection checks reparse source, permitting requirement identity to change with profile/version drift. `SourceObserved`. | 02, 12–13, 26 |
| F08 / P1 | Checkpoints have a hash chain but insufficient protection against recomputation, tail removal, concurrent writers, partial writes, and unauthenticated freshness. `ConfirmedSuccess` observes a command, not host delivery. `SourceObserved` and `RiskInferred`. [Source](../scripts/lc631/crates/lc631-analysis/src/closure_resume.rs) | 18–20, 22 |
| F09 / P1 | stdin/stdout/stderr thread joins after child exit lack a separate deadline. Descendants retaining pipe handles may prevent the whole call from returning. `RiskInferred`, not reproduced here. Offline environment hints also do not enforce OS network isolation. [Source](../scripts/lc631/crates/lc631-analysis/src/process.rs) | 03–04 |
| F10 / P1 | Each mutual-distillation epoch clones the same kernel. `NoProgress` is honest, but new evidence, candidates, and constraint state must actually be applied to the next epoch. `SourceObserved`. [Source](../scripts/lc631/crates/lc631-tldg/src/distillation.rs) | 20–21 |
| F11 / P1 | The finalizer and v1 schema fix host flags to false. Flipping them to true does not create admission checks or scope separation. `SourceObserved`. [CLI](../scripts/lc631/crates/lc631-cli/src/lib.rs), [schema](../schemas/epistesys-dgcl-surfaces.v1.schema.json) | 22–23 |
| F12 / P1 | Gold `non_hold_coverage` counts nonempty predictions, which differs from actual commit/hold decisions and requirement coverage. `independent_review_verified` is always false and has no independent-provenance admission path. `SourceObserved`. [Source](../scripts/lc631/crates/lc631-analysis/src/dgcl_gold.rs) | 01, 25 |
| F13 / P1 | Python executable, script, and model hashes do not pin all imports, native libraries, processor settings, or additional model dependencies. A nonempty license URL does not verify usage rights. `RiskInferred`. | 04, 26, 28 |
| F14 / P1 | Making every requirement depend on gold and the entire host conflates source-alpha progress with untested research. Conversely, counting universal hold as success leaves the positive path incomplete. `SourceObserved` and a release-policy issue. | 01, 17, 24, 28–29 |

The constructive review retains lossless source, CST, candidate/authority separation, per-gap receipts, `NoProgress`, unknown delivery, and offline model use. Connect typed producers, structural grammar, one artifact, and execution observations to existing paths instead of a wholesale rewrite.

Assurance-Compiler doctor returned `ok=true`. Complete modules below 16 KiB were reviewed in-band: `closure_connection.rs` and `dgcl_pipeline.rs` returned `needs_evidence` with zero diagnostics; `closure_resume.rs` returned `blocked_with_diagnostics` with one diagnostic. Their input digests respectively were `133c8b49bd6b491140eae6535457c9b3eccf7b8ceb90add11ccd30849e3f1003`, `0edb578bdfa7ec4b43dce298fb68d908b333fb8ac03b456b4c9f5d23e8f40db3`, and `dd021572ce9425d56d30c7ba31d15778061da6209aed47a9f0cc7608d8e9ef99`. These bounded static advisory results establish neither semantic accuracy nor a repository-wide conclusion.

Grok-Labyrinth-Adapter offline status was `blocked`, with `lc631_shadow_contract_failed` caused by an existing LC631 TLDG advisory-boundary mismatch. No Grok inference call was made. Do not transfer this installed-dependency state into evidence for Epistesys source; retain it as a condition for external-adapter use.

### 2. alpha.2 completion and later empirical evaluation

Keep these three axes in separate fields. Passing the new alpha.2 profile does not rewrite historical DGCL-30/31 `Held` records.

1. **ImplementationClosure:** declared features have connected producer→consumer→validator→candidate output, successful positive paths, and individual failure reporting. Mandatory for alpha.2.
2. **ResearchEvaluation:** independent gold, semantic accuracy, risk–coverage, general performance, and long-session measurement. States such as `PendingNoCorpus` do not block the alpha.2 source release. Retain original gold thresholds as later evaluation targets.
3. **HostObservation:** track precommit candidate, post-send callback, sink delivery, and durable replay separately. Distinguish standalone CLI completion from observed host-wide use. Runs requiring host delivery hard-hold on missing evidence and never silently downgrade to standalone.

Also distinguish `SourceReleaseReady`, `DistributionValidated`, and `HostActivated`. Promote the alpha.2 source version after mandatory gates for the first state pass. Do not claim the latter states while remote CI, a public tag, or real host pickup remain unobserved.

Required profile: `dgcl-rust-cli-ja-en-alpha2-v1`. It covers Windows x86_64 source/CLI, existing CommonMark and Rust CST pins, explicit Japanese/English instruction syntax, Japanese/English dependency observation, requirement/condition/scope/dependency IR, TL defects, Rust CLI completion, repair/resume, and standalone candidate output. Preserve alternatives/candidates/residuals for free-form language; do not claim complete understanding of all natural language. Rust CST does not replace compiler HIR/types/effects; connect required compiler observations through existing RPA boundaries. Do not mix new language, Media, all-platform, or GPU-optimization expansion into this release; regress existing capabilities.

The primary grammar path uses a **budgeted Earley parser** over a lossless token/region lattice and typed Instruction AST. Document a controlled Japanese/English instruction grammar, preserve a parse forest, and compare Stanza UD as an independent observation view against structural constraints. Preserve multiple candidates or `Unresolved` where dependencies cannot resolve coordination/scope. Keep old marker rules for shadow comparisons. Do not misclassify arbitrary prompt prose as DSL.

Required grammar families are explicit commands (`Please ACTION` / `ACTIONしてください`), obligations/permissions/prohibitions (`must/may/must not` / `必須/許可/禁止`), action coordination, condition introduction, necessary conditions, temporal restrictions, exceptions, explicit resource scope, and references. Distinguish `If P, A`, a requirement active under P, from `A only if P`, which makes P necessary for A. `Do not A until P` prohibits A until P is observed; observing P creates no execution authority for A. Never distribute `not(A and B)` into `not A and not B` automatically. Preserve arbitrary noun-phrase spans as candidates rather than automatically treating them as actions or resources.

Condition evaluation retains `Supported/Refuted/Unknown/Conflict`; execution-required conditions are satisfied only by current, unconflicted Supported evidence. The parsed condition itself is not evidence for its predicate. Condition/exception nodes carry source spans and edges to affected actions; unresolved shared scope yields forest alternatives. A versioned manifest links grammar productions and feature families to their positive/negative cases.

Account for every source region as `Instruction`, `Context`, `Quoted`, `Code`, `Opaque`, `Unsupported`, or `Ambiguous`. Do not over-hold context/quotes/code/opaque regions as missing instructions. Provide a valid positive path for context-only inputs with no requirements while preventing failed instruction extraction from closing an empty set.

### 3. Shared implementation contract

- Each slice follows existing-diff inspection→negative/fail-first→minimal implementation→targeted validation→diff critique→exit receipt. Advance if work does not depend on empirical evaluation. Expected fail-first failure is not a stop condition.
- Use newtypes for internal identity, immutable source snapshots, and preserved public JSON fields. Add fields or explicit v2 reports; do not reinterpret v1's fixed-false boundaries.
- `Parsed`, `ContractChecked`, `CompilerObserved`, `RuntimeObserved`, and `IndependentlyEvaluated` have separate status. A signature binds issuer and payload; it does not create analyzer truth.
- Derive feature applicability from the profile. Caller-supplied `Option` presence or arbitrary `NotApplicable` cannot skip mandatory gates. Dangerous actions fail closed; meaning states fail explicitly; bounded reversible effects fail within bounds; parse/evidence failures remain visible.
- Bind `ExecutionPermit` to explicit action/argv/cwd/targets/risk from a trusted caller boundary. Do not derive it from natural-language AST, quotes, tool output, or repair suggestions. CLI `--execute` is one necessary input to permit construction. Allowlist executables in the required profile through a reviewed command registry. Reject arbitrary repository binaries until a separate profile isolating their effects is implemented and tested. Permits and Job Objects alone do not establish filesystem/network isolation.
- Default to pure Rust core; select typestate/newtypes/arenas to close identity and transitions. Do not add async, binary ABI, no_std, SoA, or GPU systems in this release. Preserve existing crates' `forbid(unsafe_code)`. If OS process control needs unsafe, isolate it in a small separate platform crate with a safe API, `forbid(unsafe_op_in_unsafe_fn)`, and explicit handle-lifetime contracts.
- Preserve source/grammar/backend revisions, artifact/run/requirement IDs, target snapshot, validator version, issuer, and parent links. Geometry/continuous distillation proposes candidates checked by discrete validators.
- Each slice receipt records `id/dependencies/input digests/negative observation/changed files/commands/results/exit predicates/residual risks`. `Deferred` cannot complete a required feature; use it only for later ResearchEvaluation.

### 4. Dependency-ordered implementation: EPIA2-00 through EPIA2-29

All slices can be implemented in numeric order. Even with parallel work, do not mutate downstream source until its listed dependencies pass. File names are proposed additions; inventory existing owners in slice 00 to avoid duplicate modules.

#### EPIA2-00 — Baseline, change ownership, and gate inventory

- Depends on: none. Scope: workspace, existing DGCL status, validation.
- Work: record target working-tree digests rather than HEAD alone, untracked files, old fields, dependencies, toolchain, and CLI contracts. Map every DGCL-00..31 condition into implementation/research/host categories.
- Validate: obtain fmt/check/tests/clippy/build baseline while preserving changes. First add rejection fixtures for empty inventory, missing files, and stale snapshots.
- Exit: baseline passes and ownership/mapping are complete. Distinguish pre-existing failures from new ones; stop on a major baseline failure.

#### EPIA2-01 — Release profiles and research state

- Depends on: 00. Proposed scope: `release_profile.rs`, wire/promotion adapter.
- Work: type the required feature set, supported grammar/OS, disabled optional features, three status axes, and three release states. Keep missing gold as `PendingNoCorpus`.
- Validate: all mandatory implementation checks allow the source gate to pass without gold. Missing mandatory connections, universal hold, an empty feature set, and caller-chosen N/A must fail.
- Exit: add an alpha.2-specific decision and claim scope without changing existing DGCL gates.

#### EPIA2-02 — Immutable source and one artifact identity

- Depends on: 01. Proposed scope: core newtypes, `dgcl_artifact.rs`, DG1 adapter.
- Work: add `SourceRevision/GrammarRevision/ArtifactId/RequirementId/RunId/TargetRevision`, parent chains, and parse snapshots. IDs must not depend solely on extraction order. Pin canonical serialization for evidence.
- Validate: reject mid-UTF-8 spans, source changes, same text under another profile, IDs from another artifact, and duplicates. Same-snapshot encoding/decoding retains identity.
- Exit: downstream stages borrow the same artifact without reparsing raw source.

#### EPIA2-03 — Process supervisor and complete I/O deadline

- Depends on: 02. Scope: `process.rs`, platform capsule.
- Work: use a separate platform crate or reviewed safe dependency to attach Windows processes to a Job Object before execution and close the spawn race. Define whole-call deadlines for process/pipes/writer, output/memory/process budgets, cancellation, cwd, and environment. Do not describe a Job Object as a network sandbox.
- Validate: separate helper processes retain descendant pipe handles, ignore stdin, flood output, exit early, and exercise cancellation/timeouts. The parent call must return within bounds.
- Exit: timeout detects and terminates remaining processes/readers. Unsupported isolation rejects the requested execution rather than introducing an unbounded join fallback.

#### EPIA2-04 — Japanese/English backend runtime and reproducible configuration

- Depends on: 03. Scope: `language_worker.rs`, Python worker, model/runtime manifests.
- Work: manifest Python, package/wheel/native dependencies, processors/packages, and the model dependency closure. Use explicit model paths and offline settings; inspect isolated imports. Download/provisioning is a separate action. Record per-file usage terms and distribute no weights.
- Validate: alternate package, extra imports, missing model, false manifest, malformed framing, absent tools, deadline, and empty response. Record real JA/EN worker observations under the configured-runtime profile.
- Exit: configured-runtime positive paths and explicit Unavailable paths work. CI snapshots are not real-model execution evidence.

#### EPIA2-05 — Document regions, mixed language, and lossless accounting

- Depends on: 02, 04. Scope: DG1 regions/source maps.
- Work: retain Markdown lists/headings/quotes/code/HTML/tables and host origin. Use language-span lattices for mixed text and reject dependency edges across synthetic batch boundaries.
- Validate: inline quotes, commands inside code/HTML, JA/EN within one paragraph, CRLF, combining characters, raw strings, and source/batch offset differences.
- Exit: every byte maps back to the original source and each region's role/unsupported reason is accounted for. Region roles create no authority.

#### EPIA2-06 — Structural validator for UD observations

- Depends on: 05. Proposed scope: `dependency_graph.rs`.
- Work: separate tokens, words, MWTs, and empty nodes; check sentence-local IDs, root rules, head existence, self-loops/cycles, span ordering/coverage, features, and relation subtypes. Explicitly profile any allowed non-tree view.
- Validate: cyclic/headless/multi-root/duplicate/reversed/cross-region/uncovered cases are rejected or receive explicit residuals defined by the profile.
- Exit: structurally checked UD enters grammar as a typed graph. Syntactic relations are not equated with logical conditions.

#### EPIA2-07 — Budgeted instruction grammar and parse forest

- Depends on: 06. Proposed scope: `instruction_grammar.rs`, `instruction_ast.rs`, versioned grammar table.
- Work: implement an Earley core that parses JA/EN imperatives, deontic modalities, negation, coordination, conditions, exceptions, and references into typed AST. Limit active items, forest nodes, depth, and tokens. Separate free-form and controlled-grammar profiles.
- Validate: do not select the first ambiguous result, silently accept out-of-grammar input, or truncate a budget overflow into success. Test reachability, positives, and negatives for every declared production.
- Exit: the positive path materializes AST/forest; old marker extraction becomes shadow comparison. Geometry scores never decide parse validity.

#### EPIA2-08 — Per-action coordination, polarity, and modality

- Depends on: 07. Scope: grammar lowering.
- Work: preserve shared subjects/objects, per-action Must/Should/May/Forbidden, negation scope, and and/or/xor. Retain alternatives when sharing is ambiguous.
- Validate minimal pairs: `テストを実行し、公開しない`, `Do not delete or publish`, `You may test but must not publish`, double negation, and quoted negation.
- Exit: multiple requirements survive one sentence. Connect Forbidden to tests that prohibited effects do not occur, without converting it into an execution request.

#### EPIA2-09 — Conditions, time, and exception expressions

- Depends on: 08. Proposed scope: `condition_ir.rs`.
- Work: retain `All/Any/Not/Predicate`, necessary/sufficient conditions, before/after/until temporal relations, and exception precedence. Current external evidence is needed to establish a condition; text presence is insufficient.
- Validate: only-if versus if, unless, exceptions to exceptions, nested conditions, unobserved predicates, and reversed temporal order never become unconditional.
- Exit: source-bound links connect each action to condition/exception AST; unknown/conflict states reach evaluators and TL.

#### EPIA2-10 — Scope, references, dependencies, and extraction accounting

- Depends on: 09. Proposed scope: `requirement_graph.rs`.
- Work: type resource/action/branch/environment/time scopes; preserve reference candidates and requires/before/conflicts edges. Explain cycles with involved requirements. Record candidate instruction regions that cannot be fully parsed in a residual ledger.
- Validate: staging/production scope drift, multiple referents for `それ`, dropped instructions, duplicates, cycles, valid context-only input, and false holds on unrelated explanation.
- Exit: every operative candidate has AST or residual; each requirement's scope is explicit.

#### EPIA2-11 — Caller/host authority boundary

- Depends on: 10. Scope: core/host authority, execution adapter.
- Work: bind principal, user span, action scope, revision, expiry, and revocation to permits. Separate parsed modality from execution authority. Distinguish standalone CLI and host-attested caller origins. Do not execute unregistered binaries or unisolated arbitrary actions even when a plan supplies their hash.
- Validate: reject grants from assistant/tool/repo/quote/code/completion text, scope expansion, lost denies, stale permits, forged same-name principals, and authority amplification through repair.
- Exit: safe declared commands have a valid positive permit path; missing/out-of-scope permits block execution. Research results do not change permits.

#### EPIA2-12 — One DeepGrammar→Program IR→TL path

- Depends on: 11. Scope: `integration.rs`, `dgcl_pipeline.rs`, TL adapters.
- Work: lower requirements, conditions, anchors, alternatives, and UD views from the same artifact into Program IR; record preservation, omission, introduction, and incomparability for each projection in TL. Remove legacy raw-source reparsing from the new positive path.
- Validate: individually dropping requirement/negation/condition/scope/anchor produces the corresponding defect. Equal shape with different payload/source must not share a count-based identity.
- Exit: one CLI artifact exposes parent/input/output digests for every stage. Structural preservation remains separate from semantic evaluation.

#### EPIA2-13 — Obligations and completion plan

- Depends on: 12. Scope: closure contract, target manifest.
- Work: enumerate applicability and targets of implementation, connection, static, runtime, and acceptance checks per requirement. Assign suitable absence/static checks to Forbidden and non-execution requirements.
- Validate: reject a missing mandatory gap, arbitrary N/A, unrelated tests, unknown requirements, duplicate answers, and completion with an unmet condition.
- Exit: required evidence kinds, producers, and acceptance predicates are fixed before completion.

#### EPIA2-14 — Evidence producers and non-amplifying receipt issuance

- Depends on: 13. Scope: `closure_connection.rs`, receipt-kernel adapters.
- Work: close receipt issuance from public reports. Keep checked observation-token constructors private; verify producer, policy, and exact payload per evidence kind. Never synthesize another kind from the same observation automatically.
- Validate: reject caller-constructed Observed reports, signed wrong-kind/wrong-run inputs, same-issuer self-consistency alone, differing scope/digests, replay, and expiry.
- Exit: issue StructuralObservation for structural checks, CompilerObserved for compiler execution, and RuntimeObserved for run traces, each mapped to its own gap.

#### EPIA2-15 — Static connection graph

- Depends on: 14. Proposed scope: `connection_graph.rs`, RPA adapter.
- Work: inspect CST definitions, call expressions, module/route bindings; use explicit compiler/RPA inputs for required cross-module/type resolution. Preserve unknowns for opaque macros and dynamic dispatch. Close declared target paths without requiring a complete repository graph.
- Validate: unused functions, same-named locals, dead route strings, wrong callees, opaque macros, and absent routes under another feature/target.
- Exit: materialize declared producer→consumer static paths and unresolved edges. Identifier presence alone closes no gap.

#### EPIA2-16 — Runtime trace and acceptance oracle

- Depends on: 15. Proposed scope: `connection_runtime.rs`, CLI trace adapter.
- Work: bind traces to run/request/requirement IDs, executable/input/environment, actual producer/consumer traversal, and output digests. Separate expected oracles authored from development specifications from a candidate's own generated result; check artifact contracts.
- Validate: wrong routes, stale binaries, missing/forged/duplicate traces, wrong input, correct JSON from another run, oracle modification, and writes beyond scope.
- Exit: observe traversal and acceptance for permitted declared positive paths; untraversed paths remain open. Record the trace producer's trust boundary.

#### EPIA2-17 — Individual-gap closure and positive finalization

- Depends on: 16. Scope: closure/finalization.
- Work: inspect every applicable gap per requirement and aggregate complete/incomplete/conflict/stale states. Implement both `ImplementationClosed` for supported positives and `Hold/Clarify` for missing inputs.
- Validate: one failure does not erase another requirement's evidence; universal hold is not a release pass; failed extraction does not create vacuous closure; context-only inputs remain valid.
- Exit: a source-local positive run produces an implementation-complete candidate. Output actions and host delivery remain separate decisions.

#### EPIA2-18 — Source/build/evidence lifecycle

- Depends on: 17. Scope: snapshot manager, execution preflight.
- Work: use immutable build inputs, target snapshots, and leases; invalidate affected evidence on source/build/profile/validator/authority changes. Pre/post hashes detect modification but do not replace pre-execution isolation.
- Validate: files replaced after checking, reparse points, changed build features/models/source, and permit revocation during execution.
- Exit: stale evidence is not reused; dangerous effects lacking isolation are rejected before execution. Do not claim absolute resistance to a host administrator.

#### EPIA2-19 — Crash-consistent checkpoints and resume

- Depends on: 18. Scope: `closure_resume.rs`, receipt/replay adapters.
- Work: implement typed events, authenticity binding, write transactions, flush, exclusive writer, head/freshness, and partial-write recovery. Separate action execution, candidate submission, and sink delivery.
- Validate: crashes at each write phase, two writers, tail removal, recomputed digests, revoked authority, unknown delivery, and replayed old heads. Without a trusted head, rollback verification remains Unknown.
- Exit: resume reconstructs checked state and proceeds to revalidation without restoring old authority or unconfirmed delivery.

#### EPIA2-20 — Actual repair loop, at most four attempts

- Depends on: 19. Proposed scope: `repair_loop.rs`.
- Work: implement gap→typed RepairRequest→authorized adapter→new snapshot→targeted revalidation→full closure. Preserve a four-attempt cap, wall/resource budgets, oscillation, and NoProgress. Suggestions alone never update state.
- Validate: positive repairs of missing routes and condition projections, empty diffs, unchanged digests, unauthorized edits, cycles, a fifth attempt, crashes, and failed validation.
- Exit: observe closed repair→revalidate paths for at least two fault kinds. Exclude commit/push/merge from repair actions.

#### EPIA2-21 — Discrete↔continuous distillation that updates real state

- Depends on: 20. Scope: `distillation.rs`, kernel/integration.
- Work: pass geometry proposals through discrete constraint validation and apply accepted new evidence/constraint graphs as next-epoch input. Preserve directional loss, NotComparable, and conflict separately; distance creates no truth or authority.
- Validate: actual graph-change positives, unchanged-state NoProgress, false epochs, unverified edges, geometry ranking contradicting constraints, and regression of existing CPU/GPU paths.
- Exit: record progress only for epochs with changed content state. Do not add new GPU performance or occupancy claims to release conditions.

#### EPIA2-22 — Standalone output and host adapter

- Depends on: 21. Scope: CLI finalizer, host binding/replay.
- Work: check exact standalone candidate bytes, schema, and requirement realization before commitment. Give the host adapter separate precommit/postsend/delivery/replay APIs and trusted ingress. Files or self-reported stdin do not automatically become host observations.
- Validate: standalone positive finalization, substituted candidates, arbitrary callback JSON, another turn, stale receipts, replay, and unsupported hosts. A host-required run holds on missing callback.
- Exit: standalone paths close; host positive/negative adapter contracts can be tested with fixtures. Real host observation remains `PendingHostObservation` if absent.

#### EPIA2-23 — Additive schema and legacy consumers

- Depends on: 22. Scope: wire/schema, old snapshots.
- Work: preserve v1 fields and meaning; add required success states through v2 reports. Convert new internal types through explicit wire adapters and define handling of unknown enums/fields per consumer.
- Validate: old fixture field/type/meaning, old consumers rejecting or limiting new reports, strict parsing, source/condition AST roundtrips, and non-amplifying compatibility booleans.
- Exit: legacy consumers never infer host permission from new success; new consumers use the declared positive states.

#### EPIA2-24 — Adversarial matrix and false-hold checks

- Depends on: 23. Proposed scope: `tests/epia2_*`, explicit development fixtures.
- Work: test the 16 disconnections below and every grammar feature family. Add at least three positives and three negatives per family plus boundary/cross-feature cases. Fixtures are authored development contracts, not independent gold.
- Validate: initial positive success→one disconnection opens its gap→repair restores the same source requirement. Unsupported/ambiguous inputs hold with explanations; supported positives do not always hold.
- Exit: detect all 16 disconnections and provide positives for every declared feature. Test counts alone establish no semantic performance.

#### EPIA2-25 — Evaluation API for later empirical evidence

- Depends on: 24. Scope: `dgcl_gold.rs`, research status/provenance.
- Work: define coverage from actual decisions, per-requirement recall, false acceptance/holds, and risk–coverage denominators. Preserve and explain the old nonempty-prediction metric. Type admission of a trusted annotator registry, signatures, and adjudication digests.
- Validate: no-gold Pending, omitted denominators, negative-only documents, family/content leakage, forged IDs/signatures, and independent-adjudication receipt contracts. Test identities do not prove real human independence.
- Exit: an API can measure and inspect provenance when real gold arrives. Explicitly retain pending 200 documents/1,000 obligations per language, existing accuracy targets, and general-performance evaluation.

#### EPIA2-26 — End-to-end through production CLI and package

- Depends on: 25. Scope: CLI/router, launcher/package manifest.
- Work: one supported command reaches source→regions→grammar/UD→requirements→Program IR→TL→closure→standalone output. Check another cwd, paths with spaces, offline startup, configured-model and no-model profiles.
- Validate: launcher-only wiring, old binaries, debug/release differences, missing source in packages, implicit model download, unknown commands, and absent backends. Configured-model positive runs use the actual worker.
- Exit: a source-only package builds fresh and reproduces positives. Exclude models, receipts, caches, and private paths from packages.

#### EPIA2-27 — Adversarial reaudit, Assurance, and RepoSeiri

- Depends on: 26. Scope: final diff, README/docs, residual risks.
- Work: reaudit F01..F14, tracing trust/authority/evidence/semantic boundaries. Assurance uses complete units ≤16 KiB; RepoSeiri 1.1 checks summary/routes/linter on the same snapshot.
- Validate: claims of safety from counts/signatures/hashes/low loss alone, mismatched JA/EN claims, disconnected work described as implemented, and missing risk-ledger entries.
- Exit: forward-fix actionable P0/P1 in the supported profile and rerun affected gates. Record Assurance unknowns/unsupported scope separately without converting them into repository-wide proof.

#### EPIA2-28 — All functional/source gates before promotion

- Depends on: 27. Scope: release report, verification record.
- Work: evaluate A2-G00..13 below on the current snapshot. Complete ImplementationClosure for every mandatory feature and accurately retain pending ResearchEvaluation/HostObservation.
- Validate: fmt/check/no-default/default/clippy/release, targeted/schema/property/mutation tests, actual configured JA/EN workers, and fresh-package standalone end-to-end. Add remote CI as a separate receipt only if observed.
- Exit: `PrePromotionFunctionalPass` while version remains alpha.1. Missing gold does not block this source gate; mandatory-path failure does.

#### EPIA2-29 — alpha.2 metadata promotion, revalidation, and handoff

- Depends on: 28. Scope: Cargo/lock, core identity, manifest/launcher, compatibility/schema docs, README/docs/release/verification records, AGENTS identity references.
- Work: consistently update owned hunks to `6.3.2-alpha.2`, then rerun all mandatory gates as A2-G14. Do not change existing schema versions solely because package version changes.
- Validate: source/package/executable identity mismatch, stale fixtures, fresh builds/releases, equivalent JA/EN content, and private-data inclusion.
- Exit: only success yields `6.3.2-alpha.2 SourceReleaseReady`. Prefer forward fixes after failure; if needed restore only owned metadata hunks with apply_patch. If restoration is unsafe, record an unverified candidate. Execute commit/push/merge/install only under subsequent explicit instructions and operation-specific gates.

### 5. Release gates, disconnection matrix, and stop conditions

| Gate | Required condition |
| --- | --- |
| A2-G00 | Consistent baseline, change ownership, content snapshot, and dependency inventory |
| A2-G01 | Testable lossless source, regions/roles, identity, UTF-8, and budgets for supported inputs |
| A2-G02 | Positives, negatives, ambiguity, and unsupported cases for every declared grammar feature; positives succeed |
| A2-G03 | Authority binds trusted caller origins; projections never amplify scope/risk/grants |
| A2-G04 | Configured worker/model positives, invalid framing, timeout, descendants/pipes, and explicit Unavailable are tested |
| A2-G05 | Same-artifact requirements→Program IR→TL→closure→candidate are traceable and disconnections detected |
| A2-G06 | Checked producers appropriate to evidence kind; source/run/target/issuer binding; false-report/replay rejection |
| A2-G07 | All 16 disconnections open the right gap and repair restores the same requirement |
| A2-G08 | Testable invalidation, checkpoint crashes/concurrency, resume, four-attempt repair, and NoProgress |
| A2-G09 | Actual discrete↔continuous state updates; unverified/NotComparable/conflict states preserved |
| A2-G10 | Real standalone candidates return checked positives; host-required runs satisfy their own gate or hold |
| A2-G11 | v1 compatibility, v2 negotiation, old-consumer boolean boundaries, and schema roundtrips tested |
| A2-G12 | Current-snapshot Rust gates, configured JA/EN and fresh-package positives, privacy/dependency records |
| A2-G13 | Repair supported-profile P0/P1 found in source; align JA/EN docs, residual risks, and audit receipts |
| A2-G14 | Reevaluate A2-G00..13 under alpha.2 identity and record source readiness with actual receipts |

Required 16 disconnections: missing source anchor; substituted grammar/profile; promoted quoted origin; lost polarity; lost condition; expanded scope; missing requirement; missing Program IR→TL edge; wrong implementation binding; broken static call edge; untraversed runtime producer/consumer; substituted acceptance oracle; forged evidence kind/issuer; stale build/model/source; missing/duplicate/rolled-back checkpoint; substituted candidate/host-callback digest. Record input identity, expected gap, observed gap, and recovery for each.

Basic implementation commands are `cargo fmt --all --check`, `cargo check --workspace --locked`, `cargo test --workspace --no-default-features --locked`, `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo build --release --locked`. Add appropriate targeted/property/schema/mutation/process helpers within slices. Test unsafe/platform capsules according to their boundaries.

Stop for a major baseline failure, substantive conflict with existing changes, broken public compatibility, corrupted source/spans, unresolved execution authority, or an untestable mandatory source gate. Missing gold, untested independent research, and unobserved disabled optional hosts do not block source release. Never disable a requested mandatory capability such as host delivery merely to pass. Internally consistent tests do not establish semantic accuracy or general reliability.

### 6. Rust interface fragments and implementation prohibitions

These are future interface designs, not additions already made to source. Compile their implementation as modules, with adapters and negative tests. Do not leave undefined types and record code completion.

```rust
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct RequirementId(u64);

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ObservationState {
    Missing,
    Candidate,
    ContractChecked,
    Refuted,
    Conflict,
}

pub enum ConditionExpr {
    Predicate(PredicateId),
    All(Vec<ConditionId>),
    Any(Vec<ConditionId>),
    Not(ConditionId),
    Temporal(TemporalRelation),
}

pub struct RequirementNode {
    pub id: RequirementId,
    pub source: SourceAnchor,
    pub action: ActionExpr,
    pub modality: Modality,
    pub polarity: Polarity,
    pub conditions: Vec<ConditionId>,
    pub exceptions: Vec<ConditionId>,
    pub scope: ScopeExpr,
    pub alternatives: Vec<AlternativeId>,
}
```

Define `PredicateId/ConditionId` as newtypes belonging to the same artifact arena. Prohibit premature condition-to-bool collapse, arbitrary string-based scope guesses, and selection of the first forest candidate.

```rust
// Constructors and raw observations stay inside the producer module.
pub struct CheckedRuntimeObservation {
    run: RunId,
    artifact: ArtifactId,
    binding: ExecutionBinding,
    trace: CheckedTrace,
}

impl RuntimeEvidenceIssuer {
    pub fn issue(
        &self,
        observation: &CheckedRuntimeObservation,
        requirement: RequirementId,
        policy: &RuntimeEvidencePolicy,
    ) -> Result<RuntimeEvidenceReceipt, EvidenceError> {
        policy.check_exact_binding(observation, requirement)?;
        self.sign_runtime_payload(observation, requirement)
    }
}
```

Never deserialize `CheckedRuntimeObservation` from public JSON reports or caller booleans. Do not derive static/acceptance receipts automatically from runtime receipts. Preserve self-attested host events as local callback candidates.

```rust
pub enum ResearchStatus {
    PendingNoCorpus,
    PendingIndependentReview,
    Evaluated(ResearchEvaluationReceipt),
}

pub enum OutputScope {
    StandaloneCandidate,
    HostPreCommit,
    HostPostSend,
    SinkDelivery,
}

pub fn alpha_source_gate(
    profile: &CheckedReleaseProfile,
    results: &FunctionalGateLedger,
) -> Result<SourceReleaseReady, Vec<GateFailure>> {
    let failures = results.check_all_required(profile);
    if failures.is_empty() {
        SourceReleaseReady::checked(profile, results)
            .map_err(|failure| vec![failure])
    } else {
        Err(failures)
    }
}
```

`CheckedReleaseProfile` validates a nonempty feature set and scope. `SourceReleaseReady::checked` rechecks completeness, freshness, and identity of required gates. Include ResearchStatus separately without turning it into an evaluated state here. SourceReleaseReady is neither MutationAuthority nor an output-action permit.

### 7. Later empirical evaluation and residual risks

Later ResearchEvaluation retains independent-gold collection, adjudication, and identity checks; 200 holdout documents/1,000 requirements per language; original targets of precision 99%, recall 98%, document exactness/coverage 95%, and zero critical errors; family separation; false-hold/risk–coverage; and paired evaluation of complex conditions and long sessions. Use the [gold collection protocol](dgcl-gold-collection-protocol.md). These thresholds are not achievements of the current implementation.

Required ResidualRiskLedger entries: free-form language/unsupported grammar, model misparsing, untested semantic accuracy, human provenance, multilingual reference, dynamic dispatch/opaque macros, host ingress/delivery, local-key/OS-administrator trust, rollback freshness, dependency redistribution rights, remote CI, Assurance unsupported scope, and existing GPU hardware observations. Give each an owner, affected scope, current status, and next required evidence.

Describe alpha.2 as an experimental implementation that executes Japanese/English/Rust DGCL structure, requirements, evidence, validation, repair, and candidate output within its declared profile. Bind completion claims to `ImplementationClosedForDeclaredProfile`. Do not claim prevention of hallucinations, complete understanding of arbitrary instructions, host-wide enforced control, or zero weaknesses. README must state actual additions and later research evaluation with matching strength in Japanese and English.

### 8. Design constraints from primary sources

- [Stanza pipeline/processors](https://stanfordnlp.github.io/stanza/pipeline.html): informs separation of tokens, words/MWTs, processor dependencies, and configuration. Execution observation does not automatically validate instruction meaning.
- [UD coordination](https://universaldependencies.org/u/dep/conj.html) and [UD adverbial clauses](https://universaldependencies.org/u/dep/advcl.html): account for coordination sharing/nesting limits and multiple uses of advcl when designing separate scope/condition ASTs.
- [Microsoft Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects): informs management of associated processes as a unit. Network isolation and administrator resistance require separate conditions.
- [Semantic Versioning](https://semver.org/): `6.3.2-alpha.2` advances the pre-release identity from alpha.1. Package version does not establish research effects.
