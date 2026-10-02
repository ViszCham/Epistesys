# EPIA2-29 / A2-G14 — alpha.2 release verification

## 日本語

2026-10-02 JST。**Epistesys 6.3.2-alpha.2 SourceReleaseReady**。source完了範囲は`ImplementationClosedForDeclaredProfile`、profileは`dgcl-rust-cli-ja-en-alpha2-v1`。Gold/一般意味精度/性能/長期session/実hostの実証を意味しない。既存差分を保持し、commit/push/merge/install/restartはこの記録のsource readinessから自動的に許可されない。

### 順序とidentity

[A2-G00〜13の昇格前Pass](epia2-prepromotion-functional-pass-2026-10-02.md)をalpha.1のまま記録した後、Cargo/Cargo.lockの13 package、plugin manifest、README/docs/AGENTS/skill identityをalpha.2へ整合した。既存schema名・command名は互換識別子として保持。source/release/fresh-package executableは`6.3.2-alpha.2`、grammarは`controlled-ja-en-instruction-earley.v2`と観測した。3 skillはUTF-8 modeで公式quick validatorを通過し、invocation policyを拡張していない。

[alpha.2 source inventory](epia2-alpha2-source-inventory-2026-10-02.json)：160 members、digest `sha256:fa94dc67156ac2cd155b56dd50cd6ea63bfd49798dceed25bd7b2f1e0de6e3b5`。scopeはRust/Cargo/tests/workers/fixtures/schemas/runtime pins/plugin identityであり、prose/audit book-keeping/build/cacheを除く。基点HEADは`60cc71db2ba47517bbd209894033f9b55b3708f8`、branchは`dgcl-completion`。初期40 memberは存在を維持し、reset/checkout/clean/既存file削除は行わなかった。

### alpha.2再検証

`scripts/validate-alpha2-gates.ps1 -FreshPackage`をalpha.2で実行し、以下は全てexit 0。

| command | output SHA-256 |
| --- | --- |
| cargo fmt --all --check | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| cargo check --workspace --locked | `ecfa4248eef4eba08d6a014289ec0d206eb8bf86bb2c37c29d9bd3faa45956cb` |
| cargo test --workspace --no-default-features --locked | `ea11359a298814aa1876712b2dc3693ef9dba7b6def147e73ebaabac22444e76` |
| cargo test --workspace --locked | `f12d3e61346e8a4804bf630f7822a1d67d2c4ce5029ca5e849b54ddbca151cda` |
| cargo clippy --workspace --all-targets --locked -- -D warnings | `b53f1a3776d2472d16b1188f27937396758836ab361836ce94c9422b1587fa84` |
| cargo build --release --locked --offline | `08cd2c1cfcb62392766032e0ddfe0aa87d9ddbb731e98ef506c705fb5516184b` |
| configured JA/EN Stanza targeted test | `7d7b66e95fa13efdd45d6699bd1983f703e1151710ab0d77c82ffc987669faa5` |

169-member fresh source-only packageのdigestは`sha256:3258bb1202911ae7d4df5e9c39e99c9ae1f8af713a2a9b026a49fe54e135a544`。別directory/space pathでoffline locked release buildと3つのproduction CLI試験（finalize、実repair、実中断/resume）が通過。そのrelease test binaryを別途再実行してalpha.2出力をtempへcaptureし、legacyも含む14 surface/roundtrip、5 permission mutation拒否をschema検査した。fresh binary自身のversionもalpha.2だった。既存toolchain/registry cacheを使う検証で、clean-machine provisioning、配布binaryのprivacy適合、host installではない。

### 閉鎖と残存の分離

実修復→署名checkpoint/head→fresh再開、grammar/16断線、同ASTの蒸留→IR/TL/plan/candidate、consumer v1/v2、schema、source package、A2-G00〜13、alpha.2 identityの順次sliceを完了した。詳細な失敗再現・修正・差分と補助監査は[閉鎖記録](epia2-closure-slices-2026-10-02.md)と昇格前Passを参照する。誤scope、source変更、失効、unknown condition、fake report、missing/replayed headを完成扱いにしない。

ResearchEvaluation=`PendingNoCorpus`、HostObservation=`PendingHostObservation`、remote CI=`NotRun`。[ResidualRiskLedger](epia2-alpha2-residual-risk-ledger.v1.json)に15項目を保持する。owner assignment未確定も明示する。Assuranceのneeds_evidence/blocked_with_diagnosticsと未監査範囲を残し、形式証明、runtime safety、repository-wide safety、merge approvalへ昇格しない。実装完了と「全弱点ゼロ」、source readinessとhost activationを同一視しない。

最終RepoSeiri 1.1のsummary→routes→linter→summaryは同session `sha256:de8a140dc884359375c048c60a770312862fd5d5a2f536b7da9a60e0c49efbee`、document diagnostics 0、linter 0 findings/29 files、14 route assessments。この追記以前の文書snapshotを対象とする。一般構成の6 findingsはsource完成証明ではなく、方針判断を勝手に実装しない。Labyrinth v6.3.0の実diff critique/recursive_updateは、集約した代表10 file diffへcross-cutting/split pressureを返した。これはcode正確性の失敗証明ではない。実装は6依存slice・個別targeted試験・小さなsibling moduleへ分離済みであり、今後のGit reviewも境界別に行う。集約diffを低riskと呼ばず、partial/verification boundaryを保持する。

## English

2026-10-02 JST. **Epistesys 6.3.2-alpha.2 SourceReleaseReady**. Source completion scope: `ImplementationClosedForDeclaredProfile`, profile `dgcl-rust-cli-ja-en-alpha2-v1`. This does not establish gold/general semantic accuracy/performance/long sessions/real hosts. Existing changes were preserved; this source-readiness record never automatically authorizes commit/push/merge/install/restart.

### Order and identity

After recording [prepromotion A2-G00..13 Pass](epia2-prepromotion-functional-pass-2026-10-02.md) while still alpha.1, align all 13 Cargo/Cargo.lock packages, plugin manifest, README/docs/AGENTS/skill identities to alpha.2. Existing schema/command names remain compatibility identifiers. Source/release/fresh-package executables observed `6.3.2-alpha.2`, grammar `controlled-ja-en-instruction-earley.v2`. Three skills passed the official quick validator in UTF-8 mode without expanded invocation policy.

[Alpha.2 inventory](epia2-alpha2-source-inventory-2026-10-02.json): 160 members, digest `sha256:fa94dc67156ac2cd155b56dd50cd6ea63bfd49798dceed25bd7b2f1e0de6e3b5`. Scope: Rust/Cargo/tests/workers/fixtures/schemas/runtime pins/plugin identity; excludes prose/audit bookkeeping/build/cache. Base HEAD `60cc71db2ba47517bbd209894033f9b55b3708f8`, branch `dgcl-completion`. All 40 initial members remain, with no reset/checkout/clean/existing-file deletion.

### Alpha.2 revalidation

Execute `scripts/validate-alpha2-gates.ps1 -FreshPackage` under alpha.2. Every command exited zero.

| command | output SHA-256 |
| --- | --- |
| cargo fmt --all --check | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| cargo check --workspace --locked | `ecfa4248eef4eba08d6a014289ec0d206eb8bf86bb2c37c29d9bd3faa45956cb` |
| cargo test --workspace --no-default-features --locked | `ea11359a298814aa1876712b2dc3693ef9dba7b6def147e73ebaabac22444e76` |
| cargo test --workspace --locked | `f12d3e61346e8a4804bf630f7822a1d67d2c4ce5029ca5e849b54ddbca151cda` |
| cargo clippy --workspace --all-targets --locked -- -D warnings | `b53f1a3776d2472d16b1188f27937396758836ab361836ce94c9422b1587fa84` |
| cargo build --release --locked --offline | `08cd2c1cfcb62392766032e0ddfe0aa87d9ddbb731e98ef506c705fb5516184b` |
| configured JA/EN Stanza targeted test | `7d7b66e95fa13efdd45d6699bd1983f703e1151710ab0d77c82ffc987669faa5` |

Fresh 169-member source-only package digest: `sha256:3258bb1202911ae7d4df5e9c39e99c9ae1f8af713a2a9b026a49fe54e135a544`. Offline locked release build and three production CLI tests (finalization, actual repair, actual interruption/resume) passed from another directory/space path. Rerun that release test binary to capture alpha.2 outputs into temp; schema checks covered 14 legacy/actual surfaces and roundtrips plus five permission-mutation rejections. The fresh executable itself reported alpha.2. Existing toolchain/registry cache was used; this is not clean-machine provisioning, distribution-binary privacy approval, or host installation.

### Closure versus residuals

Completed dependency-ordered slices: actual repair→signed checkpoint/head→fresh resume; grammar/16 cuts; same-AST distillation→IR/TL/plan/candidate; consumer v1/v2; schemas; source packages; A2-G00..13; alpha.2 identity. See the [closure record](epia2-closure-slices-2026-10-02.md) and prepromotion Pass for failure observations, repairs, diffs, and supplementary audits. Wrong scopes, source changes, stale evidence, unknown conditions, fake reports, and missing/replayed heads never count as completion.

ResearchEvaluation=`PendingNoCorpus`, HostObservation=`PendingHostObservation`, remote CI=`NotRun`. Preserve 15 entries in the [ResidualRiskLedger](epia2-alpha2-residual-risk-ledger.v1.json), including pending owner assignment. Retain Assurance needs_evidence/blocked_with_diagnostics and unreviewed scope; never promote them into formal proof, runtime safety, repository-wide safety, or merge approval. Implementation completion is not zero weaknesses; source readiness is not host activation.

Final RepoSeiri 1.1 summary→routes→linter→summary shared session `sha256:de8a140dc884359375c048c60a770312862fd5d5a2f536b7da9a60e0c49efbee`, zero document diagnostics, zero linter findings/29 files, and 14 route assessments, on the document snapshot before this addition. Six generic organization findings are not source-completion proof, and policy decisions were not invented. Labyrinth v6.3.0 actual-diff critique/recursive_update flagged cross-cutting/split pressure on an aggregated representative ten-file diff. This is not proof of incorrect code. Implementation was already separated into six dependency slices, targeted checks, and small sibling modules; future Git review should retain those boundaries. Do not label the aggregate diff low-risk; preserve partial/verification boundaries.
