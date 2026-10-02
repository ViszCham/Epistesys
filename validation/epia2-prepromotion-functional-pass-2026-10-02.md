# EPIA2-28 functional source gate / 機能source gate

## 日本語

2026-10-02 JST。`PrePromotionFunctionalPass`。判定時versionは`6.3.2-alpha.1`。宣言profileは`dgcl-rust-cli-ja-en-alpha2-v1`、Windows x86_64、trusted callerが明示した登録済みCargoと検証済みintent、controlled grammar、source-only standaloneを対象とする。任意repositoryの安全性、任意指示の意味正確性、実Codex ingress/deliveryは範囲外であり、root/key/authority contextの信頼を仮定する。

基点HEAD `60cc71db2ba47517bbd209894033f9b55b3708f8`、branch `dgcl-completion`。[source inventory](epia2-prepromotion-source-inventory-2026-10-02.json)の160 member digestは`sha256:ddb8a99c3adb44ad61a4dea86d1ec9dc1ea6970c60c4c07f6970786598eec6b9`。このscopeはRust/Cargo、tests、worker、fixtures、schemas、runtime pins、plugin identityを含み、build/cache/log/prose/監査bookkeepingを除く。最終command系列の後に全memberの不変を確認した。初期差分40 memberは全て存在し、既存内容をreset/checkout/clean/削除で失わせていない。

### A2-G00〜13対応

| Gate | 判定と実証範囲 |
| --- | --- |
| G00 | Pass：初期baseline、現inventory、Cargo.lock/runtime pins、差分保持。履歴と現snapshotを分離。 |
| G01 | Pass：source/span/identity/budget、CommonMark/Rust CST、UTF-8とproperty試験。 |
| G02 | Pass：11 family×3正例、99 origin反例、33 malformed/semantic欠落反例、曖昧性・交差試験。開発契約でありgoldではない。 |
| G03 | Pass：外部Authority/registered toolのexact scope/root/parent/principal/expiry、Deny/quote/standalone/replay/失効のnegative。実host全体や即時revocation feedは未観測。 |
| G04 | Pass：pinned configured Stanza JA/ENの実実行、invalid framing、UD graph、timeout/pipe/Unavailable試験。OS network sandboxではない。 |
| G05 | Pass：同artifactのgrammar→Program IR→TL→plan→closure→exact candidate、sealと断線検出。任意回答文の意味証明ではない。 |
| G06 | Pass：independent intent mapping、別Cargo check/runtime/connection invocation/acceptance実観測、kind/run/issuer/lease binding、public report偽装/zero tests拒否。対象外の任意call graphを観測したとはしない。 |
| G07 | Pass：16 contract-cut roundtripと個別projection/gap試験、A10実call-edge cut、A15物理rollback、A16exact callback/candidate cut→restore。条件structure復旧もtruthはUnknownのまま。contract fixtureを実host証拠へ昇格しない。 |
| G08 | Pass：current snapshot/lease失効、writer/head/crash/resume、実ファイル2故障修復、最大4回/NoProgress/cycle、production CLI実中断→fresh再検証。未知作用やjoint rollback耐性は別risk。 |
| G09 | Pass：実content state/次epochの変更、同ASTのstructural materializationをIR/TL/planへ接続、pairの別制約provenance混同を修正、未検証/比較不能保持。距離はtruth/permissionにならない。 |
| G10 | Pass：standalone verified-ledger positiveと不足時Hold。host-required profileは未観測時Heldを維持し、standaloneへ黙って降格しない。 |
| G11 | Pass：旧5fixtureとadditive schema/roundtrip、明示v1/v2 negotiation、strict限定v1 decoderの未知field/true拒否。全旧consumerの互換性は未保証。 |
| G12 | Pass：下記7command、実日英worker、fresh source-only release build/CLI 3試験、配布allowlist/hash/private/cache/model/key除外。既存toolchain/registry cacheを使用し、clean machine provisioningではない。 |
| G13 | Pass：対応profileの再現した穴をforward fix、Assurance/RepoSeiri補助監査、日英scope/互換性/残存riskを整合。未監査を安全性へ昇格しない。 |

### 実行証跡

`scripts/validate-alpha2-gates.ps1 -FreshPackage`のfmt/check/no-default/default/clippy/release/configured-ja-enは全てexit 0。no-default output digest `bb16f8463a0106d0fd32dcb07be84d1e678d6039669a89f523de0439d74f2982`、default `69e69db816308a70e00530e49324725d22184f852a8a9e985553f2e93bffc17e`、clippy `a8685d00de01f45e60788193511047390c254dbf213212f8100a3d6872f81357`、release `fee7074c6d12d1fe797d41112247a2b422ec89b691944557116687e53342861e`、configured JA/EN `0b8f5168ad703b0fc6290c5a12b96a0a69bc3cf77edc09cfefc8cafeb4460a29`。raw logとJSON command recordはlocal temp保存であり、個人pathを公開repoへコピーしない。

fresh packageは169 files、member digest `sha256:1a1661eff9a1bbdcbe219e31302118110e4113607012f17585e6826c47e6ad1b`。offline locked release build成功、別cwd/space pathのproduction finalize、repair、実中断/resume 3試験成功。schema validator（pin済みjsonschema 4.25.1）はlegacyと実CLI captureを含む25 surface/roundtripと10 permission mutation拒否を観測した。schema passと正常completion試験は別証拠であり、Hold captureを正常completionと数えない。

RepoSeiri 1.1 research/repositoryのsummary→routes→linter→summaryは同source session `sha256:d3be866ac4018879da4005c3da7b7f29123b126428a766ce9a1f02b651f84475`。document diagnostics 0、linter 0 findings/28 files、route assessments 14。6一般構成findings/11patch holds等はmaintainer判断として保持し、license/security/supportの方針を捏造しない。この判定文書追加以前のquery観測であり、source inventory scopeとは別。

Assurance最終`dgcl_distillation.rs` input `c562f8c59cc972dae7f50dd04f2d637a31aa277ea63030027b9a3b01e7b91ca9`、complete `preflight_dgcl_cargo_plan` input `d787158c25a4cc7ef6f99ee9713834544b8e44304215cbc887d8744149516457`、complete `dgcl_validator_revision` input `ca3a48528bf9711e10700c2e3f72542335054daa2d1344c134fb7c3affdc97ca`は各≤16KiB、`needs_evidence`/diagnostics 0。旧`blocked_with_diagnostics`も保持。compiler truth、形式証明、runtime safety、repository-wide safety、merge approvalではない。

次はEPIA2-29のmetadata昇格とalpha.2 identity再検証。現時点の研究評価は`PendingNoCorpus`、実hostは`PendingHostObservation`、remote CI未実行。既存DGCLの履歴的Heldやv6.3.1 global promotion gateを成功へ書き換えない。

## English

2026-10-02 JST. `PrePromotionFunctionalPass`, with version still `6.3.2-alpha.1` at assessment. Declared profile: `dgcl-rust-cli-ja-en-alpha2-v1`, Windows x86_64, explicitly caller-authorized registered Cargo and checked intent, controlled grammar, source-only standalone. Arbitrary-repository safety, arbitrary-instruction semantic accuracy, and real Codex ingress/delivery are outside scope; root/key/authority-context trust is assumed.

Base HEAD `60cc71db2ba47517bbd209894033f9b55b3708f8`, branch `dgcl-completion`. The 160-member [source inventory](epia2-prepromotion-source-inventory-2026-10-02.json) has digest `sha256:ddb8a99c3adb44ad61a4dea86d1ec9dc1ea6970c60c4c07f6970786598eec6b9`. Scope includes Rust/Cargo, tests, workers, fixtures, schemas, runtime pins, and plugin identity; excludes build/cache/log/prose/audit bookkeeping. All members were unchanged after the final command series. All 40 initial-difference members remain; no reset/checkout/clean/deletion discarded existing content.

### A2-G00..13 mapping

| Gate | Decision and observed scope |
| --- | --- |
| G00 | Pass: initial baseline, current inventory, Cargo.lock/runtime pins, preserved changes; separate history from current snapshot. |
| G01 | Pass: source/span/identity/budgets, CommonMark/Rust CST, UTF-8 and property tests. |
| G02 | Pass: 11 families×3 positives, 99 origin negatives, 33 malformed/semantic-omission negatives, ambiguity/cross-feature tests; developer contracts, not gold. |
| G03 | Pass: external Authority/registered-tool exact scope/root/parent/principal/expiry; Deny/quote/standalone/replay/invalidation negatives. Real host-wide enforcement and immediate revocation feeds are unobserved. |
| G04 | Pass: actual pinned configured Stanza JA/EN, invalid framing, UD graphs, timeout/pipe/Unavailable tests; not OS network sandboxing. |
| G05 | Pass: same-artifact grammar→Program IR→TL→plan→closure→exact candidate, seals/cut detection; not proof of arbitrary-answer meaning. |
| G06 | Pass: independent intent mapping; separate actual Cargo check/runtime/connection-invocation/acceptance observations; kind/run/issuer/lease binding; reject public-report forgery and zero tests. No arbitrary out-of-profile call-graph observation claim. |
| G07 | Pass: 16 contract-cut roundtrips and individual projection/gap tests; A10 real call-edge cut, A15 physical rollback, A16 exact callback/candidate cut→restore. Restored condition structure retains Unknown truth. Contract fixtures are not real-host evidence. |
| G08 | Pass: current snapshot/lease invalidation, writer/head/crash/resume, two actual file faults repaired, four-attempt/NoProgress/cycle tests, actual production CLI interruption→fresh revalidation. Unknown effects and joint rollback resistance remain separate risks. |
| G09 | Pass: real content-state/next-epoch changes, same-AST structural materialization connected to IR/TL/plan, repaired parallel-pair constraint provenance, retained unverified/incomparable states. Distance creates no truth/permission. |
| G10 | Pass: standalone verified-ledger positives and insufficient-evidence Hold. Required-host profiles remain Held when unobserved, with no silent standalone downgrade. |
| G11 | Pass: five old fixtures and additive schemas/roundtrips, explicit v1/v2 negotiation, strict limited-v1 unknown-field/true rejection; no guarantee for every legacy consumer. |
| G12 | Pass: seven commands below, real JA/EN workers, fresh source-only release build/three CLI tests, allowlist/hash/private/cache/model/key exclusions; existing toolchain/registry cache, not clean-machine provisioning. |
| G13 | Pass: forward fixes for reproduced in-profile gaps, supplementary Assurance/RepoSeiri, aligned JA/EN scope/compatibility/residuals; no safety promotion of unreviewed scope. |

### Execution witnesses

All fmt/check/no-default/default/clippy/release/configured-ja-en commands in `scripts/validate-alpha2-gates.ps1 -FreshPackage` exited zero. Output digests: no-default `bb16f8463a0106d0fd32dcb07be84d1e678d6039669a89f523de0439d74f2982`; default `69e69db816308a70e00530e49324725d22184f852a8a9e985553f2e93bffc17e`; clippy `a8685d00de01f45e60788193511047390c254dbf213212f8100a3d6872f81357`; release `fee7074c6d12d1fe797d41112247a2b422ec89b691944557116687e53342861e`; configured JA/EN `0b8f5168ad703b0fc6290c5a12b96a0a69bc3cf77edc09cfefc8cafeb4460a29`. Raw logs and JSON command records remain in local temp storage; personal paths are not copied to the public repository.

Fresh package: 169 files, member digest `sha256:1a1661eff9a1bbdcbe219e31302118110e4113607012f17585e6826c47e6ad1b`. Offline locked release build and three production finalization/repair/actual-interruption-resume tests from another cwd/space path passed. The pinned jsonschema 4.25.1 validator observed 25 legacy/actual-CLI surfaces and roundtrips, plus ten permission-mutation rejections. Schema passes and successful-completion tests are separate evidence; Hold captures are not counted as normal completion.

RepoSeiri 1.1 research/repository summary→routes→linter→summary shared session `sha256:d3be866ac4018879da4005c3da7b7f29123b126428a766ce9a1f02b651f84475`: zero document diagnostics, zero linter findings/28 files, 14 route assessments. Six generic organization findings/11 patch holds etc. remain maintainer decisions; no invented license/security/support policy. Queries preceded this decision-document addition and are separate from the source-inventory scope.

Final Assurance inputs: complete `dgcl_distillation.rs` `c562f8c59cc972dae7f50dd04f2d637a31aa277ea63030027b9a3b01e7b91ca9`; complete `preflight_dgcl_cargo_plan` `d787158c25a4cc7ef6f99ee9713834544b8e44304215cbc887d8744149516457`; complete `dgcl_validator_revision` `ca3a48528bf9711e10700c2e3f72542335054daa2d1344c134fb7c3affdc97ca`. Each ≤16 KiB, `needs_evidence`/zero diagnostics. Retain earlier `blocked_with_diagnostics`. These are not compiler truth, formal proof, runtime safety, repository-wide safety, or merge approval.

Next: EPIA2-29 metadata promotion and alpha.2 identity revalidation. Research remains `PendingNoCorpus`; real hosts remain `PendingHostObservation`; remote CI has not run. Historical DGCL Held and the v6.3.1 global promotion gate are not rewritten as success.
