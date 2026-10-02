# DGCL operating profile and closure / 対応範囲と閉鎖

## 日本語

### 実装範囲

DGCLは、lossless source・文書領域・Rust CST・pin済み日英dependency worker・予算付きcontrolled instruction grammarを、同じsource identityから要求別Program IR、TranslationLoss、completion planへ接続する。pipelineの構築sealはlocalな改変検出であり、転送可能な署名や意味正確性の証明ではない。public JSONの成功fieldを後から書き換えても、検証済みcandidateの入口にはならない。

`lc631-dgcl-run`はno-external-modelsとconfigured-Stanzaを区別する。worker/model/runtimeのpinが不足したconfigured呼出しは診断付き失敗になる。配布物はmodel本体を含まない。自然言語一般の意味理解、完全な任意文法、指示の正解率は未実証であり、controlled grammar外はUnsupported/Unresolved/Ambiguousとして保持する。

### Codingの実行経路

- `lc631-dgcl-package-finalize`：外部Authority receiptと登録tool、独立intent bindingを確認し、別々のCargo check・library runtime test・production API→executor接続呼出し・明示acceptance testを観測する。1〜8個の条件なしRequired要求に限定する。条件のtruthを推測して解除しない。
- `lc631-dgcl-package-repair`：同じsource/pipelineを維持し、rootと対象fileに束縛されたEdit receipt、予定された修復後snapshotへの独立binding、Test receiptと全validation planを先に検証する。明示的な`--ledger-root`と別管理の`--head-root`を必要とし、修復前にrequest/root-bound journalと署名headを保存する。既存Rust fileだけを限定更新し、変更前bytesを`.dgcl-repair-backups/`へ非上書き保存する。新snapshotと新receiptから全要求を再評価する。
- `lc631-dgcl-package-resume`：同じrepair bundleとledger/headを検証し、書込みを再実行せずcurrent sourceを再検証する。旧Completed checkpointも新しいTest receipt・独立binding・Cargo観測を省略できない。crash後の現在targetが予定candidateと一致しなければHold/診断になる。joint rollbackやhead期限切れを回避する自動repairは行わない。CLIに指定するjournal rootはcallerが事前に用意する保管契約であり、host全体のpersist権限を生成しない。
- SDKのrepair coordinator：最大4回、wall/delta予算、NoProgress、cycle、expired/stale evidence、未確認作用を分離する。`MayHaveApplied`は成功でもrollback済みでもない。旧validation reportだけではCompletedにしない。
- final candidate：全receiptをcurrent lifecycleに対して再検証してから、同じ要求・gap ledgerをexact bytesへ描画する。`contract_ledger_rendered`はledger実現の観測であり、任意回答文の意味正確性ではない。standalone completion、host send、sink deliveryは別状態である。

Authority入力は、configured trust rootに対して検証する外部発行の`Authority` receiptである。seed/source signature、`--execute`、low loss、tool hash、slot存在だけからGrantを新規発行しない。tool registrationも別に必要で、public process/report objectは観測tokenへdeserializeできない。scopeとsnapshotはrepositoryの識別子を保持する。キーのprovenance・保護・現在のauthority/revocation contextは、trusted caller/hostの契約であり、ローカル署名が実Codex ingressや人間の独立性を証明するわけではない。

### 再開と互換性

checkpoint v1の既存wireを保持し、v2でaction開始・action観測・candidate準備・送信試行・sink観測を分離する。writer lockで同時writerを排除し、ledger外の署名headを保存する。`--head-root`付きresumeはheadを読込・検証し、必ずfresh revalidationを要求する。head未提供はUnanchoredであり、完了・authority・deliveryを復元しない。別管理headとの不一致は検出できるが、ledgerとhead双方の共同rollback、同一user/administratorによる変更、directory fsyncの全OS保証は主張しない。

schema追加はadditive。新しいconsumerはverified constructor/receipt policyを使用し、旧consumerは未知enumを拒否または限定viewとして扱う。schema validはevidence validではない。defaultのportable testはモデル不足の失敗経路を検査し、実日英worker positiveは`EPISTESYS_TEST_CONFIGURED_NLP=1`を指定した別観測として記録する。

明示consumer negotiationは`epistesys-dgcl-consumer-view.v1/v2`を分ける。v1限定viewのcompletion/send/commit boolはfalseに保持し、strict decoderは未知fieldとtrueを拒否する。v2の`reported_implementation_complete_candidate`は観測reportの描画であり、証拠発行やpermissionではない。旧consumer一般の互換性は主張しない。

離散↔連続接続は同じcontrolled Instruction ASTの構造をbounded geometryへ送り、離散edge membership検査を通ったmaterializationをProgram IRへ戻す。content-state変更だけがepoch progressとなり、materializationはProgram IR digest→TL→completion plan→candidate sealに含まれる。条件のtruth、実行順序の達成、Grantは構造membershipから生じない。未検証・比較不能・曖昧性を保持し、自由な意味関係の発見精度は未実証である。

### 完了と未実証の区別

Source functional closureは、declared profileの実装・実行・接続・断線・修復・schemaを確認する。独立gold、一般意味精度、hallucination containment、長期session、performance、real host callback/即時revocation、OS filesystem/network sandbox、opaque build scripts/proc macros、外部依存の完全なbuild world、Assurance未対応は別の残存riskである。Cargo/test pass、ローカルkey signature、source versionだけでこれらを実証済みにしない。研究評価はPendingNoCorpus、host実観測はPendingHostObservationを維持する。

## English

### Implementation scope

DGCL connects lossless source, document regions, Rust CST, pinned Japanese/English dependency workers, and budgeted controlled instruction grammar from one source identity to per-requirement Program IR, TranslationLoss, and completion plans. The pipeline construction seal detects local mutation; it is neither a transferable signature nor proof of semantic accuracy. Editing public JSON success fields does not enter the verified-candidate path.

`lc631-dgcl-run` separates no-external-models and configured-Stanza profiles. Configured invocations missing worker/model/runtime pins fail with diagnostics. Distribution excludes model weights. General natural-language understanding, complete arbitrary grammar, and instruction accuracy remain unproven; inputs outside controlled grammar retain Unsupported/Unresolved/Ambiguous states.

### Coding execution path

- `lc631-dgcl-package-finalize`: check external authority receipts, registered tools, and independent intent binding; observe separate Cargo check, library runtime test, production API→executor connection invocation, and explicit acceptance-test runs. Limited to 1–8 unconditional Required requirements. Condition truth is not guessed or discharged.
- `lc631-dgcl-package-repair`: retain the same source/pipeline and preverify an Edit receipt bound to the root/file, independent binding to the projected post-repair snapshot, Test receipts, and every validation plan. Require explicit `--ledger-root` and a separately managed `--head-root`; persist a request/root-bound journal and signed head before repair. Update only an existing Rust file and preserve preimage bytes without overwriting backups under `.dgcl-repair-backups/`. Reevaluate all requirements from the new snapshot and new receipts.
- `lc631-dgcl-package-resume`: verify the same repair bundle and ledger/head, then revalidate current source without repeating a write. Even an old Completed checkpoint cannot omit new Test receipts, independent binding, and Cargo observations. A current target that differs from the planned candidate after a crash produces Hold/diagnostics. No automatic repair bypasses joint rollback or expired heads. Caller-provisioned journal roots are an explicit storage contract, not newly generated host-wide persistence authority.
- SDK repair coordinator: at most four attempts; separate wall/delta budgets, NoProgress, cycles, expired/stale evidence, and unconfirmed effects. `MayHaveApplied` means neither success nor rollback. Legacy validation reports alone cannot produce Completed.
- Final candidate: reverify every receipt against the current lifecycle before rendering the same requirement/gap ledger into exact bytes. `contract_ledger_rendered` observes ledger realization, not arbitrary-answer semantic correctness. Standalone completion, host send, and sink delivery remain separate states.

Authority input consists of externally issued `Authority` receipts checked against configured trust roots. Seed/source signatures, `--execute`, low loss, tool hashes, and slot presence do not mint Grants. Tool registration is also required; public process/report objects cannot deserialize into observation tokens. Scopes and snapshots preserve repository identity. Key provenance/custody and current authority/revocation context remain trusted-caller/host contracts; local signatures do not prove real Codex ingress or human independence.

### Resume and compatibility

Preserve existing checkpoint-v1 wire data; v2 separates action start/observation, candidate preparation, send attempts, and sink observation. Writer locks exclude concurrent writers; signed heads persist outside the ledger. Resume with `--head-root` loads/verifies that head and always requires fresh revalidation. Missing heads remain Unanchored and restore no completion, authority, or delivery. Mismatches against separately managed heads are detectable; joint ledger/head rollback, same-user/administrator modification, and universal OS directory-fsync guarantees are not claimed.

Schema additions are additive. New consumers use verified constructors/receipt policies; old consumers reject unknown enums or use limited views. Schema validity is not evidence validity. Portable default tests inspect missing-model failure paths; actual Japanese/English worker positives are recorded separately with `EPISTESYS_TEST_CONFIGURED_NLP=1`.

Explicit consumer negotiation separates `epistesys-dgcl-consumer-view.v1/v2`. The limited v1 view retains false completion/send/commit booleans; its strict decoder rejects unknown fields and true values. V2's `reported_implementation_complete_candidate` renders an observation report, not evidence issuance or permission. Compatibility with every legacy consumer is not claimed.

The discrete-continuous connection sends structure from the same controlled Instruction AST to bounded geometry, then returns materializations checked by discrete edge membership to Program IR. Only changed content state records epoch progress; materializations enter Program IR digest→TL→completion plan→candidate seal. Predicate truth, accomplished execution order, and Grants do not arise from structural membership. Unverified/incomparable/ambiguous states remain, and accuracy in discovering arbitrary semantic relations is unproven.

### Closure versus unproven outcomes

Source functional closure checks implementation, execution, connections, cuts, repairs, and schemas for the declared profile. Independent gold, general semantic accuracy, hallucination containment, long sessions, performance, real host callbacks/immediate revocation, OS filesystem/network sandboxing, opaque build scripts/proc macros, complete external-dependency build worlds, and unsupported Assurance scope remain separate residual risks. Cargo/test passes, local key signatures, and source versions do not establish them. Research evaluation remains PendingNoCorpus; real host observation remains PendingHostObservation.
