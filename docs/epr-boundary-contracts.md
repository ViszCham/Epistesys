# EPR boundary contracts

## 日本語

この文書はEPR修正で扱う契約を定義します。実装の存在、対象profileの実観測、一般能力の実証、host activationを別々に扱います。

### 要求・証拠・作用

- 相対byte offsetと原文offsetを内部型で分離し、引用領域・UTF-8境界・後処理の出力と作業予算を検査します。予算不足は明示的な失敗であり、要求不存在やNoActionへ変換しません。
- verified authority eventは原receiptのsource subjectとparentを保持します。host seedの表示用fieldを書き換えても、保存した認証済み原文・span・scope・artifact・turnを置換できません。署名済み文章を正規化せず、Markdownの文書領域判定だけを権限適格性の検査に使います。
- 現在のreceiptの期限は半開区間 `[issued, expires)` です。履歴headの認証と、現在の操作の許可は別物です。期限切れの真正な履歴を確認できても、fresh Test権限、独立binding、全検証を省略できません。
- CLIエラーの`effect_state`はsource作用と永続化を分けます。既知の作用後にjournal・出力が失敗しても、未作用とは報告しません。分類できない古いerrorはUnknownであり、rollbackの証拠ではありません。

### 接続の観測範囲

`DgclCargoValidationPlan.connection`はadditiveな登録計画です。旧fieldは保持しますが、connection specのないlibrary test成功をProductionConnectionへ昇格しません。

登録driverのsource/digest、単一の直接API呼出し、pinした標準assertion、実Cargo target metadata、exact named test、現在の登録receiptを照合します。`std`を別dependencyやlibraryで置換する入力は、このprofileでは拒否します。実driverとoracleの対応を持たない成功summaryは接続証拠ではありません。

これは、協調的な登録Cargo/build profileの限定した直接接続観測です。任意の推移的到達性、悪意あるbuild scripts/proc macros/global constructors、ambient Cargo configurationを含むhermetic認証、仕様全体の意味的正しさは証明しません。source-only snapshotの入力閉包を越える再利用は正当化しません。[計画schema](../schemas/epistesys-dgcl-connection-spec.v1.schema.json)は構造検査であり、証拠の真正性を発行しません。

repository rootのscope identityは、存在するdirectoryのcanonical pathへ統一します。Windowsのcase／短縮名が同じdirectoryを指す場合は同じidentityとし、計画・snapshot・observerの表記差による偽の失効を避けます。存在しないrootとUTF-8で表せないrootは拒否し、lossy変換で異なるpathを統合しません。この正規化はpathの入場許可ではなく、nofollow／reparse／containment検査を省略しません。旧lexical digestと新digestが異なる場合はfreshなscope binding／receiptが必要で、旧Grantを翻訳して拡大しません。

### Media・資源・数値

Mediaの表示DTOと認証済み入力bindingを分離し、使用直前と完了時に元receipt・現在時刻・issuerを検査します。backendは原文pathの再読ではなく、同じbyteを確認したprivate snapshotを読みます。snapshotとdecoderが別々にtemp上限を使い切らないよう、同じ予算から差し引きます。

人物boxと顔region、音響clusterと話者diarizationは別kindです。観測kindはbackendが報告したfamily/methodと対応しなければ拒否します。対応していてもmethodの意味精度を独立検証したことにはならず、family validationは別です。UTF-8のlabel打切りと未観測coverageを残します。モデルcacheのmanifest v3はUTF-8 path順と構造化したentry encodingを共有し、旧v2のdigestを黙って再解釈しません。

共通supervisorは所有process/pipeをdeadline内で監督します。Windows Job内のcommitted memory制約、Unix workerのper-process address-space制約、実RSS、OS全体の隔離は異なるものです。network/filesystem sandboxや同一owner/administratorへの絶対強制を主張しません。private fileの権限・ACL・identity、拒否時の登録不変性を検査します。

数値入力はCSR・四値mask・中間算術・作業予算をdevice投入前に検査します。実GPU比較で観測するsummary/mask/finalist/decision parityと、全vector一致・occupancy・speedupは別です。2,048 world evaluationは2,048回のLLM推論ではありません。

### 互換性と残存条件

既存JSON fieldは削除せず、connection spec、token binding、historical head状態、作用・memory metadataを追加します。未知enumを拒否する旧consumerがあるため、全旧decoderの透過互換は保証しません。既存の制限付きviewと現在のverified constructorを用い、古いreportから新しいpermissionを復元しません。

配布helperが作るものは、固定commit／承認inventoryから選んだ**CLIのsource検証用package**です。skillsやhooksをすべて含むplugin install archiveではありません。source packageのbuild／実CLI成功、repositoryのplugin配置、実install／host pickupは別の確認対象です。

独立gold、一般意味精度、実hostのpickup/配信、外部revocationと鍵保管、全build world、OS-wide isolationは別の未観測・未実証条件です。研究仮説の実証や形式証明を、これらの修正から自動的に主張しません。

## English

This document defines the contracts covered by EPR remediation. Implementation presence, observed declared profiles, general capability evidence, and host activation remain separate.

### Requirements, evidence, and effects

- Separate relative and source byte offsets internally; check quoting, UTF-8 boundaries, and postprocessing output/work budgets. Exhaustion is explicit, not absence of requirements or NoAction.
- Verified authority events retain receipt subjects/parents. Editing displayed host-seed fields cannot replace authenticated bytes/spans/scopes/artifacts/turns. Signed text is never normalized; Markdown regions determine source eligibility only.
- Current receipt validity uses half-open `[issued, expires)`. Historical-head authentication is not current action authority. Authentic expired history cannot omit fresh Test authority, independent bindings, or validation.
- CLI `effect_state` separates source and persistence effects. Journal/output failure after an observed effect never means no effect. Unclassified legacy errors remain Unknown, not rollback evidence.

### Connection coverage

`DgclCargoValidationPlan.connection` is additive planned registration. Existing fields remain, but library-test success without a connection spec cannot become ProductionConnection.

Check pinned driver source/digest, one direct API invocation, standard assertion, observed Cargo targets, exact named runtime test, and current registration receipts. This profile rejects replacement of `std` by a dependency/library. Generic success summaries without driver/oracle correspondence are not connection evidence.

This is bounded direct-driver observation under a cooperative registered Cargo/build profile, not arbitrary transitive reachability, malicious-build-script/proc-macro/global-constructor resistance, hermetic ambient configuration, or complete semantic correctness. It does not justify reuse outside the source-only input closure. The [plan schema](../schemas/epistesys-dgcl-connection-spec.v1.schema.json) checks structure and does not issue authentic evidence.

Repository-root scope identity uses the canonical path of an existing directory. Windows case/short-name aliases for that directory share one identity, preventing spurious invalidation between plan/snapshot/observer spellings. Missing or non-UTF-8 roots are rejected rather than merged through lossy conversion. Identity normalization does not authorize paths or bypass nofollow/reparse/containment checks. When an old lexical digest differs, require fresh scope bindings/receipts; never translate an old Grant into wider authority.

### Media, resources, and numerics

Separate Media display DTOs from authenticated bindings and recheck original receipts/current time/issuer at use and completion. Backends read private byte-authenticated snapshots, not mutable original paths. Snapshot bytes and decoder output share one temporary budget.

Person boxes versus face regions, and acoustic clusters versus diarization, are distinct. Observation kinds must match reported executed families/methods; matching does not independently validate method accuracy. Preserve label truncation and missing coverage. Model-manifest v3 shares UTF-8 path ordering and structured entries; do not reinterpret v2 digests silently.

The shared supervisor owns process/pipe deadlines. Windows Job committed memory, Unix worker per-process address-space limits, actual RSS, and OS-wide isolation differ. No network/filesystem sandbox or absolute same-owner/administrator enforcement is claimed. Check private-file permissions/ACLs/identities and transactional registration rejection.

Validate CSR, four-state masks, intermediate arithmetic, and work budgets before device submission. Observed summary/mask/finalist/decision parity differs from full-vector equality, occupancy, and speedup. 2,048 world evaluations are not 2,048 LLM inferences.

### Compatibility and residual conditions

Retain existing JSON fields while adding connection specs, token bindings, historical-head states, and effect/memory metadata. Some legacy consumers reject unknown enums, so transparent compatibility with every old decoder is unproven. Use limited views/current verified constructors; old reports do not restore permissions.

The packaging helper produces a **CLI-source verification package** selected from a fixed commit/approved inventory, not a complete plugin-install archive containing every skill/hook. Source-package builds/real CLI tests, repository plugin layout, actual installation, and host pickup require separate checks.

Independent gold, general semantic accuracy, real-host pickup/delivery, external revocation/key custody, complete build worlds, and OS-wide isolation remain separate unknowns. These fixes do not establish research hypotheses or formal proof.
