# EPIA2追加閉鎖記録 / Additional closure record

## 日本語

2026-10-02 JST。基点HEAD `60cc71db2ba47517bbd209894033f9b55b3708f8`、branch `dgcl-completion`。既存tracked/untracked差分を保持したworking treeを対象とする。履歴的prepromotion Holdは過去の観測であり、修復後snapshotの判定と混同しない。

最終追記：6 sliceは順次完了し、[alpha.2 identityの再検証](epia2-alpha2-release-verification-2026-10-02.md)まで成功した。以下の段階記述は作業順と観測を保存する。parallel constraintの参照元混同もfail-firstで再現・修正し、最終distillation moduleのAssurance inputは`c562f8c59cc972dae7f50dd04f2d637a31aa277ea63030027b9a3b01e7b91ca9`、`needs_evidence`/diagnostics 0。以前のdigestは履歴であり最終sourceへ転用しない。

### 依存順sliceと観測

1. 実修復→checkpoint→再開：通常CLIでjournal欠落をfail-first再現。request/root-bound署名headを修復前に保存し、CLIを実際に修復後・検証前に中断する試験を追加。再開は再書込みせずfresh Test/intent/Cargo証拠を要求する。通常・中断・再開positiveを観測した。preflight後の時計基点と外部receipt時刻のずれも修正し、trusted adapter clockをcoordinatorに渡す。checkpointは旧completion/authority/deliveryを復元しない。
2. 文法・断線：11 family×3正例、99 origin境界反例に加え、33個の文法/意味欠落反例を追加。条件右辺・明示scope引数の欠落をUnresolvedに修正。mixed boolean precedenceはalternativeへ保持しselectedを生成しない。16 contract-cut roundtripに加え、実call削除、物理checkpoint巻戻し、exact callback/candidate差替えのcut→restore試験を通過。condition復旧はtruthを捏造しない。これらはdeveloper contractであり独立goldではない。
3. 蒸留・consumer：同Instruction ASTからstructural artifact→bounded geometry→discrete edge membership→materialization→Program IR/TL/plan/candidate identityへ接続。missing APIをfail-firstで確認後、接続/改変拒否試験を通過。v1限定viewはcompletion/send/commitをfalseに保持しstrict decoderで未知field/true/v2を拒否。v2はreported stateのみ表示しpermissionを発行しない。package finalizerから両viewを返す。
4. schema・配布・監査・文書：追加fieldとresume/consumer/distillation schemaをadditiveに記述。実CLIから別temp directoryへbundle/出力をcaptureし、legacyを含むschema/roundtripとpermission mutationを検査。source-only packageはallowlist、copy後hash、private情報/cache/model/key除外を検査する。bytecodeがallowlistへ入るnegativeを観測し、既存fileを削除せずignore/exclusionで修正。fresh build/testと最終監査は後続の現在snapshot記録で確定する。
5. A2-G00〜13：現在source固定後の全gateとinventoryを照合する。過去のpassや件数のみでrequired gateを閉じない。
6. alpha.2：5成功前はalpha.1を維持する。昇格後identityで全必須gateを再検証するまでSourceReleaseReadyと記録しない。

### 補助監査と境界

Assurance doctorは作業系列で一度実行済み。今回の完全Rust moduleは各16KiB以下で、`dgcl_consumer.rs` input `8aeae2c37f48518b52aa35c666280f983ddaa29358834c4c7d50e57e5cb56453`、`dgcl_distillation.rs` input `4db7125394686721a45db27671f60d72380413bb839924ef3ea33761627060d5`、`dgcl_journal.rs` input `eb3fe405b9207f00ad368aa677ac97c4abbb0f80b6b994205f79f974572a4255`は全て`needs_evidence`/diagnostics 0。別の[修復監査](epia2-repair-blocker-audit-2026-10-02.md)の`blocked_with_diagnostics`も保持する。診断0は証明、runtime safety、repo全体の安全性、merge approvalではない。

Rust method worldはsafeなtyped APIと既存のbounded supervisor。新GPU/async/unsafeは導入せず既存platform capsuleを維持。所有hunk以外の破壊、reset/checkout/clean/既存file削除は行わない。gold、一般意味精度、長期session、実host callback/pickup、即時revocation、OS sandbox、joint rollback、外部build world、Assurance未対応は残存する。署名はtrust-root-bound payloadの観測であり実人間/実Codex ingressの証明ではない。

## English

2026-10-02 JST. Base HEAD `60cc71db2ba47517bbd209894033f9b55b3708f8`, branch `dgcl-completion`. Scope is the working tree with existing tracked/untracked changes preserved. Historical prepromotion Hold is a prior observation, not the repaired snapshot's decision.

Final addition: all six slices completed in order, including successful [alpha.2 identity revalidation](epia2-alpha2-release-verification-2026-10-02.md). Stage descriptions below preserve execution order and observations. Parallel-constraint provenance confusion was also reproduced fail-first and fixed. Final distillation-module Assurance input: `c562f8c59cc972dae7f50dd04f2d637a31aa277ea63030027b9a3b01e7b91ca9`, `needs_evidence`/zero diagnostics. Earlier digests remain historical and are not reused for final source.

### Dependency-ordered slices and observations

1. Actual repair→checkpoint→resume: reproduced missing journals with a fail-first CLI test. Persist request/root-bound signed heads before repair and test actual CLI interruption after writing but before validation. Resume requires fresh Test/intent/Cargo evidence without rewriting. Normal/interrupted/resumed positives were observed. Fixed preflight clock-origin drift relative to external receipts and pass a trusted adapter clock to the coordinator. Checkpoints restore no prior completion/authority/delivery.
2. Grammar/disconnections: supplement 11 families×3 positives and 99 origin-boundary negatives with 33 malformed/semantic-omission negatives. Missing condition RHS and explicit scope operands become Unresolved. Mixed boolean precedence remains an alternative with no selected AST. Alongside 16 contract-cut roundtrips, real call deletion, physical checkpoint rollback, and exact callback/candidate substitution cut→restore tests passed. Restoring a condition does not fabricate truth. These are developer contracts, not independent gold.
3. Distillation/consumers: connect the same Instruction AST through structural artifact→bounded geometry→discrete edge membership→materialization→Program IR/TL/plan/candidate identity. After fail-first missing-API observations, connection/tamper-rejection tests passed. Limited v1 views retain false completion/send/commit; their strict decoder rejects unknown fields, true values, and v2. V2 displays reported state without issuing permission. Package finalization returns both views.
4. Schemas/distribution/audit/docs: add field constraints and resume/consumer/distillation schemas additively. Capture real CLI bundles/outputs into a separate temporary directory; check legacy schemas/roundtrips and permission mutations. Source-only packages check allowlists, post-copy hashes, and private/cache/model/key exclusions. A bytecode-in-allowlist negative was observed and fixed with ignore/exclusion without deleting existing files. Fresh builds/tests and final audit are established by subsequent current-snapshot records.
5. A2-G00..13: reconcile all gates and inventory after freezing current source. Neither old passes nor test counts alone close mandatory gates.
6. Alpha.2: retain alpha.1 before slice 5 passes. Do not record SourceReleaseReady until every mandatory gate is revalidated under the promoted identity.

### Supplementary audit and boundaries

Assurance doctor was executed once in this work sequence. Complete Rust modules were each at most 16 KiB: `dgcl_consumer.rs` input `8aeae2c37f48518b52aa35c666280f983ddaa29358834c4c7d50e57e5cb56453`, `dgcl_distillation.rs` input `4db7125394686721a45db27671f60d72380413bb839924ef3ea33761627060d5`, and `dgcl_journal.rs` input `eb3fe405b9207f00ad368aa677ac97c4abbb0f80b6b994205f79f974572a4255` all returned `needs_evidence`/zero diagnostics. Preserve `blocked_with_diagnostics` from the separate [repair audit](epia2-repair-blocker-audit-2026-10-02.md). Zero diagnostics is not proof, runtime safety, repository-wide safety, or merge approval.

Selected Rust method world: safe typed APIs and the existing bounded supervisor. No new GPU/async/unsafe machinery; retain the existing platform capsule. No destructive unrelated hunks, reset/checkout/clean, or existing-file deletion. Gold, general semantic accuracy, long sessions, real host callbacks/pickup, immediate revocation, OS sandboxing, joint rollback, external build worlds, and unsupported Assurance scope remain residuals. Signatures observe trust-root-bound payloads, not real human/real Codex ingress provenance.
