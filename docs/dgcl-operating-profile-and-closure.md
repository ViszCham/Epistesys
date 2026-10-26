# DGCL execution contract and verification scope

## 日本語

### 適用範囲

DGCLは、同じsource identityから、文書領域・構文・controlled Instruction ASTを要求別Program IR、TranslationLoss、completion plan、candidateへ接続します。これは宣言profileの実行契約です。共通用語と全体のデータフローは[アーキテクチャ](system-architecture.md)、起動例は[実行ガイド](execution-guide.md)、wire形式は[互換性](dgcl-wire-compatibility.md)を参照してください。

| 項目 | 対応条件 |
| --- | --- |
| source | 原文byte、revision、digest、spanを保持する |
| 文書・構文 | CommonMark領域、Rust CST、pin済み日英dependency worker、予算付き指示文法 |
| NLP構成 | no-external-modelsとconfigured-Stanzaを区別。worker/model/runtime pin不足は失敗 |
| 文法外・未確定 | Unsupported／Unresolved／Ambiguousを保持し、truthを推測しない |
| finalizerの要求数 | 条件なしRequired要求の1〜8個に限定 |
| SDK修復予算 | 最大4回。wall／delta、NoProgress、cycleも制限 |
| package repair CLI | 呼出しごとに単一requestとreplacement。`attempt=1`を要求 |
| 配布 | source-only。model weightsや個人のreceipt/key環境を含めない |

pipeline construction sealはlocalな改変検出であり、転送可能な署名や意味正確性の証明ではありません。公開JSONの成功fieldを書き換えても、検証済みconstructorの入口にはなりません。

Coding Projectorは明示選択された依頼を解析経路へ渡します。`lc631-dgcl-run`の診断・候補と、以下のpackage操作を区別します。完了判定は抽出・登録された要求とvalidation planに限られ、元の依頼から要求を漏れなく抽出したことまでは検証しません。抽出精度と受入oracleの妥当性は独立した評価対象です。

### 実行権限と検証材料

Authorityはconfigured trust rootに対して検査する外部発行receiptです。実行時には次を独立に確認します。

- 操作別のAuthority receipt、対象repository/fileのscope、対象snapshot。
- tool registrationとtool provenance。
- 独立intent bindingと必要なvalidation plan。
- 現在の期限・失効・競合状態。

source signature、`--execute`、low loss、tool hash、slot存在はGrantを発行しません。public process/report objectは観測tokenへdeserializeできません。ローカルkey署名は実Codex ingressや人間の独立性を証明せず、key保護・issuer provenance・revocationはtrusted caller/host契約に依存します。

### Package finalization

`lc631-dgcl-package-finalize`は以下を行います。

1. 外部Authority、登録tool、独立intent、validation planを確認する。
2. Cargo check、library runtime test、登録direct driverによるpublic API呼出しとpin済みassertion、明示acceptance testを別々に観測する。
3. 同sourceの要求とgapを、現在のreceipt・lifecycleに対して再評価する。
4. 所定の条件を満たすexact candidateを構築する。

| evidence kind | 指定profileで必要な材料 |
| --- | --- |
| `ImplementationBinding` | 要求・実装target・source snapshotへ束縛した外部intent binding |
| `ProductionConnection` | 登録direct driverが指定public APIを呼び、pin済みassertionを実行した観測 |
| `StaticValidation` | 登録された現在のCargo check観測 |
| `RuntimeValidation` | 登録計画で指定したlibrary runtime test観測 |
| `AcceptanceTest` | 登録計画の明示acceptance testと、その受入条件の観測 |

kindは自動的に相互代替しません。EPR候補の`DgclConnectionSpec`はdriver・entrypoint・oracle・coverageを登録receiptへ束縛します。genericな`--lib`成功、推移的なexecutor到達、hermetic build、全経路や実サービスの配備検証を意味しません。別runへの分離はoracleの独立性の証明ではなく、受入試験の成功はそのoracleで検査した条件についての観測です。

条件のtruthを推測して要求を解除しません。`contract_ledger_rendered`はledger実現の観測であり、任意回答の意味的正確性ではありません。standalone completion、host send、sink deliveryは別状態です。

### Package repair

`lc631-dgcl-package-repair`は同じsource/pipelineを維持し、変更前に全事前条件を検査します。

- bundleは呼出側の`replacement_utf8`を受け取り、requestのcandidate digestと照合する。候補作成は呼出側LLM／toolの役割である。
- root/file-bound Edit receipt、修復後snapshotへの独立binding、Test receipt、全validation planを確認する。
- 明示的な`--ledger-root`と別管理の`--head-root`を要求する。
- 書込み前にrequest/root-bound journalと署名headを保存する。
- 既存Rust fileだけを限定更新し、変更前bytesを`.dgcl-repair-backups/`へ非上書き保存する。
- 変更後snapshotと新receiptから全要求を再評価する。

SDK coordinatorは最大4回の試行とwall/delta予算を扱い、NoProgress、cycle、expired/stale evidence、未確認作用を分離します。`MayHaveApplied`は成功やrollback済みを意味しません。旧validation reportだけではCompletedになりません。

現在のCLIは`attempt=1`のrequestを1件だけcoordinatorへ渡します。coordinatorの上限値だけを根拠に、CLIが4候補を自動生成・連続修復すると説明しません。[CLIのbundleと実行](../scripts/lc631/crates/lc631-cli/src/dgcl_repair.rs)、[SDK coordinator](../scripts/lc631/crates/lc631-analysis/src/repair_loop.rs)、[file driver](../scripts/lc631/crates/lc631-analysis/src/file_repair.rs)を参照してください。

### Package resume

`lc631-dgcl-package-resume`は同じrepair bundle、ledger、headを確認し、書込みを繰り返さず現在のsourceを再検証します。

- 旧Completed checkpointも、新しいTest receipt・独立binding・Cargo観測を省略できない。
- crash後のtargetが予定candidateと異なればHold/診断になる。
- joint rollbackを回避する自動repairや、期限切れの権限・完了状態の復元は行わない。EPR候補のhistorical head経路は、真正な期限切れ履歴の確認と現在の実行許可を分離する。
- journal rootはcallerが準備する保管契約であり、host全体のpersist権限を生成しない。

### Journal・head・配信の状態

checkpoint v1のwireを保持し、v2ではaction開始、action観測、candidate準備、送信試行、sink観測を分離します。writer lockで同時writerを排除し、署名headをledger外へ保存します。

`--head-root`付きresumeはheadを検証し、fresh revalidationを要求します。head未提供はUnanchoredであり、completion・authority・deliveryを復元しません。別管理headとの不一致を検出しますが、ledger/head双方の共同rollback、同一user/administratorによる改変、全OSでのdirectory fsync保証は範囲外です。

### 構造蒸留と下流接続

同じcontrolled Instruction ASTの構造をbounded geometryへ送り、離散edge membership検査後のmaterializationをProgram IRへ戻します。

- content-stateの変更だけをepoch progressとする。
- materializationをProgram IR digest→TL→completion plan→candidate sealへ含める。
- predicate truth、実行順序の達成、Grantを構造membershipから導出しない。
- 未検証・比較不能・曖昧性を保持する。自由な意味関係の発見精度は未実証。

### Consumerと検証範囲

schema追加はadditiveです。新consumerはverified constructor/receipt policyを使用し、旧consumerは未知enumを拒否または限定viewとして扱います。

- `epistesys-dgcl-consumer-view.v1`：completion/send/commit boolをfalseに保持。strict decoderは未知fieldとtrueを拒否する。
- v2の`reported_implementation_complete_candidate`：観測reportの描画であり、証拠・permission発行ではない。
- schema validはevidence validではなく、全旧consumerの互換性を保証しない。

portable default testはモデル不足時の失敗経路を検査します。実日英workerのpositiveは`EPISTESYS_TEST_CONFIGURED_NLP=1`による別観測です。source functional closureは、宣言profileの実装・実行・接続・断線・修復・schemaを検査する意味で用います。

独立gold、一般意味精度、ハルシネーション抑制、長期session、performance、実host callback/即時revocation、OS filesystem/network sandbox、opaque build scripts/proc macros、全外部依存build world、Assurance未対応は[残存条件](known-limitations.md)です。ResearchEvaluation=`PendingNoCorpus`、host実観測=`PendingHostObservation`を保持します。

### EPR候補の境界強化と残存条件

本節はEPR変更候補の実装契約であり、上記alpha.2の履歴的検証記録や全release gateの成功を更新するものではありません。

- `HostSeedEnvelope`は元の検証済みreceiptを内部に保持し、`VerifiedAuthorityEvent`と`ExecutionPermit`はsource・span・artifact/turn・scope・parent・issuer・現在時刻への対応を検査する。public fieldの変更から別sourceの権限を再発行しない。current receiptの有効区間は`[issued, expires)`であり、期限一致時と零長期間は無効。contextなしの`decide_verified_authority`はadvisoryで、現在の判断には`VerifiedAuthorityContext`を要する。
- Hostのplain-prose eligibilityはpin済みCommonMark offset eventでcode・quote・HTML・link等を除外し、解析予算を持つ。構文上の適格性はGrantでも完全な意図理解でもない。
- `DirectoryAnchor`／`AnchoredFile`はhandle基準で経路と対象identityを検査し、symlink／reparse、hardlink alias、root外、特殊名を拒否する。Windowsは祖先の置換を抑止するhandleと排他的file handle、Linuxはanchored `openat`と同一directory内のatomic replacementを使う。Linuxの更新profileは同一ownerかつ既存access ACLなしに限定し、新stage／private backupの継承ACLを除いてからmodeを復元する。他のUnix更新profileはUnsupported。Windowsの更新失敗は`MayHaveApplied`を保持し、全OSのatomicity・durabilityや特権主体に対するsandboxは主張しない。
- `load_checkpoint_head_historical`／`inspect_resume_with_historical_anchor`の`HistoricalHeadMatched`は元の署名・同一chain/source/scopeの履歴観測に限る。production resumeはfreshなTest権限、独立binding、現在の全validationを通した後に限り、同一request/root/headの明示renewalを保存する。source書込みを再実行せず、期限切れ権限・旧Completedを復元しない。strict current-head APIは引き続き期限切れを拒否する。
- Cargo snapshotは上限付きのRust/Cargo入力列挙であり、opaque script、非Rust include、外部依存、環境全体のhermetic closureではない。最大16,384訪問entry／4,096入力fileの上限超過や不足は成功へ変換しない。
- 共通process監督の`memory_limit_kind`は適用範囲を報告する。Windowsのconfigured Job-wide committed-memory制限と、他profileで未観測のmemory制限を区別する。Mediaのsource snapshot、streaming入力、temp quota、kind/family/methodの対応は、source-root全体の意味正確性やmethod-level semantic validationを保証しない。
- `CliFailure`では作用、検証、永続化、配信とlogical journal参照を分離する。error時に作用を`NotApplied`へ一律変換せず、未知や`MayHaveApplied`を保持する。

## English

### Operating scope

DGCL connects regions/syntax/controlled Instruction AST from one source identity to per-requirement Program IR, TranslationLoss, completion plans, and candidates. This is a declared-profile execution contract. See [architecture](system-architecture.md) for terms/dataflow, [execution guide](execution-guide.md) for startup, and [compatibility](dgcl-wire-compatibility.md) for wire formats.

| Item | Conditions |
| --- | --- |
| Source | Preserve bytes, revision, digest, spans |
| Documents/syntax | CommonMark regions, Rust CST, pinned JA/EN dependency workers, budgeted instruction grammar |
| NLP configuration | Distinguish no-external-models and configured Stanza; missing worker/model/runtime pins fail |
| Unsupported/unknown | Retain Unsupported/Unresolved/Ambiguous; do not infer truth |
| Finalizer requirement count | Limited to 1–8 unconditional Required requirements |
| SDK repair budget | At most 4 attempts plus wall/delta, NoProgress, cycle limits |
| Package repair CLI | One request/replacement per invocation; requires `attempt=1` |
| Distribution | Source-only, excluding weights and personal receipt/key environments |

Pipeline construction seals detect local tampering, not transferable signatures or semantic accuracy. Mutating public JSON success fields does not enter verified constructors.

The Coding Projector routes explicitly selected requests into analysis. Distinguish `lc631-dgcl-run` diagnostics/candidates from the package operations below. Completion decisions cover extracted/registered requirements and validation plans; they do not verify exhaustive requirement extraction from the original request. Extraction accuracy and acceptance-oracle validity require independent evaluation.

### Authority and validation material

Externally issued Authority receipts are checked against configured trust roots. Independently validate:

- Operation-specific receipts, repository/file scopes, target snapshots.
- Tool registration/provenance.
- Independent intent bindings and required validation plans.
- Current expiry/revocation/conflict state.

Source signatures, `--execute`, low loss, tool hashes, and slot presence do not mint Grants. Public process/report objects cannot deserialize observation tokens. Local signatures do not prove real Codex ingress or human independence; key custody, issuer provenance, and revocation depend on trusted callers/hosts.

### Package finalization

`lc631-dgcl-package-finalize`:

1. Validate external Authority, registered tools, independent intent, and validation plans.
2. Separately observe Cargo check, library runtime tests, registered direct-driver public-API calls with pinned assertions, and explicit acceptance tests.
3. Reevaluate same-source requirements/gaps against current receipts/lifecycles.
4. Construct an exact candidate only under specified conditions.

| Evidence kind | Material required in the specified profile |
| --- | --- |
| `ImplementationBinding` | External intent binding to the requirement, implementation target, and source snapshot |
| `ProductionConnection` | Observation that a registered direct driver calls the specified public API and runs its pinned assertion |
| `StaticValidation` | Registered current Cargo check observation |
| `RuntimeValidation` | Library runtime test observation specified in the registered plan |
| `AcceptanceTest` | Explicit acceptance test and observed acceptance conditions from the registered plan |

Kinds do not automatically substitute for one another. The EPR candidate's `DgclConnectionSpec` binds the driver, entrypoint, oracle, and coverage to a registration receipt. It does not establish generic `--lib` success as connection evidence, transitive executor reachability, hermetic builds, all paths, or deployment of a live service. Separate runs do not prove oracle independence; acceptance success observes the conditions checked by that oracle.

Do not discharge requirements by guessing condition truth. `contract_ledger_rendered` observes ledger realization, not arbitrary-answer semantic correctness. Standalone completion, host send, and sink delivery remain separate.

### Package repair

`lc631-dgcl-package-repair` retains the same source/pipeline and checks all prerequisites before modification.

- The bundle receives caller-supplied `replacement_utf8` and checks it against the request's candidate digest. Candidate creation belongs to the caller's LLM/tool.
- Validate root/file-bound Edit receipts, independent post-repair snapshot bindings, Test receipts, and all validation plans.
- Require explicit `--ledger-root` and separately managed `--head-root`.
- Persist request/root-bound journals and signed heads before writes.
- Update only existing Rust files; preserve preimage bytes without overwrite under `.dgcl-repair-backups/`.
- Reevaluate all requirements from changed snapshots/fresh receipts.

The SDK coordinator allows at most 4 attempts under wall/delta budgets; distinguish NoProgress, cycles, expired/stale evidence, and unconfirmed effects. `MayHaveApplied` means neither success nor completed rollback. Old validation reports alone do not yield Completed.

The current CLI supplies only one request with `attempt=1` to the coordinator. Its attempt ceiling alone does not establish automatic generation and consecutive repair of four CLI candidates. See the [CLI bundle/execution](../scripts/lc631/crates/lc631-cli/src/dgcl_repair.rs), [SDK coordinator](../scripts/lc631/crates/lc631-analysis/src/repair_loop.rs), and [file driver](../scripts/lc631/crates/lc631-analysis/src/file_repair.rs).

### Package resume

`lc631-dgcl-package-resume` checks the same repair bundle/ledger/head and revalidates current source without repeating writes.

- Old Completed checkpoints cannot omit new Test receipts, independent bindings, or Cargo observations.
- Targets differing from planned candidates after crashes produce Hold/diagnostics.
- No automatic repair bypasses joint rollback or restores expired authority/completion. The EPR candidate's historical-head route separates authentic expired history from current execution permission.
- Journal roots are caller-provisioned storage contracts, not host-wide persist permissions.

### Journals, heads, and delivery states

Preserve checkpoint-v1 wire data. V2 separates action start/observation, candidate preparation, send attempt, and sink observation. Writer locks exclude concurrent writers; signed heads are stored outside ledgers.

Resume with `--head-root` validates the head and requires fresh revalidation. Missing heads remain Unanchored and restore no completion/authority/delivery. Detect mismatches against separately managed heads; joint ledger/head rollback, same-user/administrator mutation, and universal OS directory-fsync guarantees are outside scope.

### Structural distillation and downstream binding

Project structure from the same controlled Instruction AST into bounded geometry; return materializations checked by discrete edge membership to Program IR.

- Only content-state changes record epoch progress.
- Include materialization in Program IR digest→TL→completion plan→candidate seal.
- Membership does not imply predicate truth, achieved execution order, or Grants.
- Retain unverified/incomparable/ambiguous states; arbitrary semantic-relation discovery is unproven.

### Consumers and verification scope

Schema additions are additive. New consumers use verified constructors/receipt policies; old consumers reject unknown enums or use limited views.

- `epistesys-dgcl-consumer-view.v1`: retain false completion/send/commit flags; strict decoding rejects unknown fields and true flags.
- V2 `reported_implementation_complete_candidate`: report rendering, not evidence/permission issuance.
- Schema validity is not evidence validity; compatibility with every legacy consumer is unproven.

Portable default tests cover missing-model failure paths. Actual JA/EN-worker positives with `EPISTESYS_TEST_CONFIGURED_NLP=1` are separate observations. Source functional closure means declared-profile implementation/execution/connections/cuts/repairs/schema checks.

Independent gold, general semantic accuracy, hallucination containment, long sessions, performance, real-host callbacks/immediate revocation, OS filesystem/network sandboxes, opaque build scripts/proc macros, complete external-dependency build worlds, and unsupported Assurance scope remain [residual conditions](known-limitations.md). Retain ResearchEvaluation=`PendingNoCorpus` and real-host observation=`PendingHostObservation`.

### EPR candidate boundary strengthening and residual conditions

This section specifies implementation contracts of the EPR candidate; it does not update the historical alpha.2 verification record above or assert that all release gates passed.

- `HostSeedEnvelope` retains the original verified receipt internally. `VerifiedAuthorityEvent` and `ExecutionPermit` check source, span, artifact/turn, scope, parent, issuer, and current-time bindings; public-field mutation cannot reissue authority for another source. Current receipts use `[issued, expires)`; exact expiry and zero-duration intervals are invalid. Context-free `decide_verified_authority` is advisory; current decisions require `VerifiedAuthorityContext`.
- Host plain-prose eligibility uses pinned CommonMark offset events to exclude code, quotes, HTML, links, and related regions under parsing budgets. Syntactic eligibility is neither a Grant nor complete intent interpretation.
- `DirectoryAnchor`/`AnchoredFile` check paths and object identities through handles, rejecting symlink/reparse traversal, hardlink aliases, root escape, and special names. Windows pins ancestors against replacement and uses an exclusive file handle; Linux uses anchored `openat` and same-directory atomic replacement. Linux updates require the same owner and no existing access ACL; inherited ACLs on new stages/private backups are removed before mode restoration. Other Unix update profiles are Unsupported. Windows write failures retain `MayHaveApplied`; universal atomicity/durability and confinement against privileged principals are not claimed.
- `HistoricalHeadMatched` from `load_checkpoint_head_historical`/`inspect_resume_with_historical_anchor` observes authentic original signatures and exact chain/source/scope history only. Production resume persists an explicit same-request/root/head renewal only after fresh Test authority, independent binding, and all current validation. It does not repeat source writes or restore expired permission/old Completed state. Strict current-head APIs continue rejecting expired heads.
- Cargo snapshots enumerate bounded Rust/Cargo inputs, not a hermetic closure over opaque scripts, non-Rust includes, external dependencies, or the complete environment. Exceeding 16,384 visited entries/4,096 input files, or missing inputs, does not become success.
- Shared process supervision reports the scope through `memory_limit_kind`, distinguishing configured Windows Job-wide committed-memory limits from unobserved memory limits in other profiles. Media source snapshots, streaming inputs, temporary quotas, and kind/family/method correspondence do not establish source-root-wide semantic correctness or method-level semantic validation.
- `CliFailure` separates effects, validation, persistence, delivery, and logical journal references. Errors do not uniformly become `NotApplied`; unknown states and `MayHaveApplied` remain visible.
