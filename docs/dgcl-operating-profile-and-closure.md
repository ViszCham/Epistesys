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
| 配布 | source-only。model weightsや個人のreceipt/key環境を含めない |

pipeline construction sealはlocalな改変検出であり、転送可能な署名や意味正確性の証明ではありません。公開JSONの成功fieldを書き換えても、検証済みconstructorの入口にはなりません。

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
2. Cargo check、library runtime test、production API→executor接続呼出し、明示acceptance testを別々に観測する。
3. 同sourceの要求とgapを、現在のreceipt・lifecycleに対して再評価する。
4. 所定の条件を満たすexact candidateを構築する。

条件のtruthを推測して要求を解除しません。`contract_ledger_rendered`はledger実現の観測であり、任意回答の意味的正確性ではありません。standalone completion、host send、sink deliveryは別状態です。

### Package repair

`lc631-dgcl-package-repair`は同じsource/pipelineを維持し、変更前に全事前条件を検査します。

- root/file-bound Edit receipt、修復後snapshotへの独立binding、Test receipt、全validation planを確認する。
- 明示的な`--ledger-root`と別管理の`--head-root`を要求する。
- 書込み前にrequest/root-bound journalと署名headを保存する。
- 既存Rust fileだけを限定更新し、変更前bytesを`.dgcl-repair-backups/`へ非上書き保存する。
- 変更後snapshotと新receiptから全要求を再評価する。

SDK coordinatorは最大4回の試行とwall/delta予算を扱い、NoProgress、cycle、expired/stale evidence、未確認作用を分離します。`MayHaveApplied`は成功やrollback済みを意味しません。旧validation reportだけではCompletedになりません。

### Package resume

`lc631-dgcl-package-resume`は同じrepair bundle、ledger、headを確認し、書込みを繰り返さず現在のsourceを再検証します。

- 旧Completed checkpointも、新しいTest receipt・独立binding・Cargo観測を省略できない。
- crash後のtargetが予定candidateと異なればHold/診断になる。
- joint rollbackやhead期限切れを回避する自動repairは行わない。
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
| Distribution | Source-only, excluding weights and personal receipt/key environments |

Pipeline construction seals detect local tampering, not transferable signatures or semantic accuracy. Mutating public JSON success fields does not enter verified constructors.

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
2. Separately observe Cargo check, library runtime tests, production API→executor invocation, and explicit acceptance tests.
3. Reevaluate same-source requirements/gaps against current receipts/lifecycles.
4. Construct an exact candidate only under specified conditions.

Do not discharge requirements by guessing condition truth. `contract_ledger_rendered` observes ledger realization, not arbitrary-answer semantic correctness. Standalone completion, host send, and sink delivery remain separate.

### Package repair

`lc631-dgcl-package-repair` retains the same source/pipeline and checks all prerequisites before modification.

- Validate root/file-bound Edit receipts, independent post-repair snapshot bindings, Test receipts, and all validation plans.
- Require explicit `--ledger-root` and separately managed `--head-root`.
- Persist request/root-bound journals and signed heads before writes.
- Update only existing Rust files; preserve preimage bytes without overwrite under `.dgcl-repair-backups/`.
- Reevaluate all requirements from changed snapshots/fresh receipts.

The SDK coordinator allows at most 4 attempts under wall/delta budgets; distinguish NoProgress, cycles, expired/stale evidence, and unconfirmed effects. `MayHaveApplied` means neither success nor completed rollback. Old validation reports alone do not yield Completed.

### Package resume

`lc631-dgcl-package-resume` checks the same repair bundle/ledger/head and revalidates current source without repeating writes.

- Old Completed checkpoints cannot omit new Test receipts, independent bindings, or Cargo observations.
- Targets differing from planned candidates after crashes produce Hold/diagnostics.
- No automatic repair bypasses joint rollback or expired heads.
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
