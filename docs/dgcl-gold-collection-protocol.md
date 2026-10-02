# DGCL独立gold収集手順 / Independent DGCL gold collection protocol

## 日本語

これはDGCL-10/27を再開するための収集契約であり、goldデータや精度結果ではありません。2026-09-28時点で独立作成・裁定済みgold corpusは存在しません。自動生成ラベル、DG1出力、LLMの自己採点をgoldとして扱いません。

1. 再配布・評価利用の権利を確認できる日英sourceを集め、source文字列とそのSHA-256、出典、利用条件を記録します。同一familyや同一内容はdevとholdoutをまたがせません。
2. 各sourceを少なくとも2名が独立に、モデル予測を見ずにannotationします。要求ごとに正確なUTF-8 byte span、polarity、condition kind、scopeを記録します。引用、例外、未解決照応、相互依存、競合は「なかったこと」にせず別途記録します。
3. 相違は第三者が裁定し、annotator ID、裁定者ID、裁定理由、revisionを保存します。文字列IDだけでは本人性を認証できないため、独立性・監査可能性は外部の署名または管理台帳で確認します。
4. family単位でsplitを固定し、holdoutは開発中のrule修正に使いません。最低受入規模は言語ごとにholdout 200文書・gold obligation 1,000件です。公開可能なcorpusのみをrepoに追加し、非公開資料・個人情報・モデルweightは含めません。
5. `epistesys-dgcl-gold.v1`に変換し、`schemas/epistesys-dgcl-surfaces.v1.schema.json`で構造検査した後、`lc631-dgcl-evaluate --gold-file <path>`を実行します。欠落予測はrecallとcoverageの分母に残し、critical false acceptance / missed obligationを個別に数えます。内部統計が閾値を満たしても、現評価器の`independent_review_verified=false`を独立監査済みに読み替えません。

Decision-basedの後続測定には、文書ごとのsource revision、decision（ImplementationClosed/Hold/Clarify/Conflict/NoAction）、候補要求、候補digestを`DgclDecisionObservation`として与えます。`evaluate_gold_decisions`はcommitされた候補だけのprecision/recall、decision coverage、selective risk、exact candidateが保留されたover-hold rateを別々に数えます。legacy `non_hold_coverage`はparser candidateが非空の文書率であり、実際のdecision coverageではありません。Decision observations自体は現時点で未認証として扱われます。

署名付きprovenance用の`GoldProvenanceBundle`はcaseごとに2人以上の登録annotator署名と、別keyのadjudicator署名を要求します。`validate_gold_provenance`はcase/source/annotation digest、signer registry fingerprint、署名のscopeと期限を検証しますが、実在する人間の本人性・思考の独立性・利益相反の不存在までは証明しません。そのため`human_independence_verified=false`のままです。現時点で実gold corpus/decision setはまだありません。

残る受入条件は、独立したannotation provenanceと裁定の確認、holdout leakage監査、偽保留/risk–coverage評価、host出力証跡です。この手順だけではDGCL-30/31、version昇格、配布、commit/push/mergeを許可しません。

## English

This is a collection contract for resuming DGCL-10/27, not gold data or an accuracy result. No independently authored and adjudicated gold corpus exists as of 2026-09-28. Auto-generated labels, DG1 output, and an LLM's self-grading are not gold.

1. Gather Japanese and English sources whose redistribution and evaluation rights can be checked, and record each source string, its SHA-256, origin, and usage terms. Do not split the same family or content across dev and holdout.
2. At least two annotators independently label each source without seeing model predictions. Record an exact UTF-8 byte span, polarity, condition kind, and scope for every requirement. Preserve quotations, exceptions, unresolved references, dependencies, and conflicts as separate records rather than silently dropping them.
3. A third party adjudicates disagreements, retaining annotator IDs, adjudicator ID, rationale, and revision. String IDs alone do not authenticate people; verify independence and auditability through external signatures or a managed ledger.
4. Freeze a family-level split and do not use holdout to tune rules. The minimum acceptance size is 200 holdout documents and 1,000 gold obligations per language. Add only publishable corpora to the repository; exclude private material, personal data, and model weights.
5. Convert to `epistesys-dgcl-gold.v1`, validate structure against `schemas/epistesys-dgcl-surfaces.v1.schema.json`, then run `lc631-dgcl-evaluate --gold-file <path>`. Missing predictions remain in recall and coverage denominators; count critical false acceptance and missed obligations separately. Meeting internal metric thresholds does not transform the evaluator's `independent_review_verified=false` into an independent audit.

For later decision-based evaluation, provide one `DgclDecisionObservation` per document with source revision, decision (`ImplementationClosed`/`Hold`/`Clarify`/`Conflict`/`NoAction`), candidate requirements, and candidate digest. `evaluate_gold_decisions` reports precision/recall over committed candidates, decision coverage, selective risk, and the over-hold rate for exact candidates that were held as separate metrics. Legacy `non_hold_coverage` means the share of documents with nonempty parser candidates, not actual decision coverage. Decision observations remain unauthenticated in the current API.

The signed-provenance `GoldProvenanceBundle` requires at least two registered annotator signatures per case and a distinct-key adjudicator signature. `validate_gold_provenance` checks case/source/annotation digests, signer-registry fingerprints, receipt scope, and expiry. It does not prove real-person identity, independent thought, or absence of conflicts of interest; `human_independence_verified` therefore stays false. No real gold corpus or decision set is present yet.

Remaining acceptance conditions are authenticated independent annotation and adjudication provenance, holdout-leakage audit, false-hold/risk–coverage evaluation, and host-output witnesses. This protocol alone authorizes neither DGCL-30/31, version promotion, distribution, nor commit/push/merge.
