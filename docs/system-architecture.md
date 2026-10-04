# Epistesys system architecture and operational terminology

## 日本語

### 対象と境界

対象はEpistesys `6.3.2-alpha.2`のsource実装です。本書は、LLMが生成する候補と、要求・操作権限・検証証拠・出力状態を管理する制御系を分離して記述します。モデルweightの変更、任意自然言語の完全理解、全host操作の強制制御は実装範囲に含めません。

現在の参照点は[alpha.2 release notes](epistesys-6.3.2-alpha.2-release-notes-2026-10-02.md)と[最終検証](../validation/epia2-alpha2-release-verification-2026-10-02.md)です。実行条件の詳細は[DGCL実行契約](dgcl-operating-profile-and-closure.md)、wireの条件は[互換性文書](dgcl-wire-compatibility.md)に分離しています。

<a id="terminology-ja"></a>

### 用語の操作的定義

| 用語 | 本実装での意味 | 混同しない対象 |
| --- | --- | --- |
| 入力／source | revision・digest・UTF-8 byte spanを持つ原文。互換APIではseedと呼ぶ場合がある | 確定した仕様、検証済み事実、操作許可 |
| 要求／requirement・obligation | 原文に結び付いた実現・検証対象。条件・否定・scope等を別構造として保持 | 解析器が推測した真偽、実行済み状態 |
| Program IR | source-boundな要求・関係・未解決事項の中間表現 | 任意プログラムの意味的等価性証明 |
| 射影／projection | 表現間の変換と、変換元・変換先・規則・証拠の対応 | 単なる言い換え、意味保存の保証 |
| TranslationLoss（TL） | ProjectionDefectGraph等による対応・欠落・矛盾・比較不能の記録 | 校正済みの普遍的な意味距離、真理値 |
| Membrane | 表現・状態間の検証境界を指す実装上の名称 | 物理的な膜や、それ自体による正しさの保証 |
| candidate | 生成・解析・構築された未確定候補 | Validated、Completed、Delivered |
| evidence | 対象revision・観測方法・stageに束縛した検証材料 | permission、独立gold、形式証明 |
| receipt | 発行主体・class・subject・scope・nonce・期限・parent・payloadを検査する証跡 | JSON内の成功flag、非空の署名らしい文字列 |
| authority | trusted caller／hostに由来する操作別・scope別の権限 | 推論の許可、解析された肯定表現、低loss |
| output commitment | 所定の条件でexact candidate bytesを確定するアプリケーション状態 | Git commit、sink受領、内容の真理性 |
| world／evaluation | 前提・仮説と射影に対する予算内の評価単位 | 独立したLLM agent、現実の別宇宙 |

型名・command・schema IDは互換性のため英語名を保持します。状態を表す語は[wire契約](../schemas/closure-and-receipt-contract.md)と各validatorの定義に従い、日常語の「正しい」「安全」「完成」と同一視しません。

### データフローと検証境界

| 段階 | 主な入力・出力 | 保存・検査する情報 |
| --- | --- | --- |
| 原文の保存 | source → 文書領域・token/region lattice | revision、digest、byte span、quote/code等の役割 |
| 構文・指示解析 | 領域・CST・dependency → controlled Instruction AST | 否定、条件、例外、時間、scope、参照、依存、解析残差 |
| 要求の射影 | AST → 要求別Program IR | 原文への対応、要求ID、構造関係、Unsupported/Unresolved/Ambiguous |
| 不整合の追跡 | source/IR → TL・ProjectionDefectGraph | 対応・欠落・矛盾・比較不能、verifier capability、証拠不足 |
| 検証計画 | 要求 → completion plan・evidence task・gap ledger | 対象snapshot、必要な証拠kind、実行条件、未解決gap |
| 操作と観測 | registered tool・外部receipt → compiler/runtime/connection/acceptance結果 | 操作scope、tool provenance、実行別の観測、失効条件 |
| 候補の確定 | current receipts・lifecycle → exact candidate | 要求別の再検証、gap、candidate digest、construction seal |
| ホストへの配信 | candidate → send/sink/replay観測 | stage別receipt、exact output bytes、送信試行と受領の区別 |

上表は依存関係を示します。診断CLIが全ての副作用段階を実行するわけではありません。権限と観測の入口は外部caller／host契約を必要とし、source signature、schema適合、tool hashだけでは操作許可や観測tokenを構築できません。

### 現行の構文・要求解析

DeepGrammarは本プロジェクトの解析module名です。現行は次の構成を持ちます。

- pin済みCommonMark event parserによる文書領域と、Rust tree-sitter CST。
- 日英Stanza dependency workerを利用する構成と、外部モデルなしの構成の区別。worker／model／runtimeのpin不足は診断付き失敗として扱う。
- `controlled-ja-en-instruction-earley.v2`による予算付き指示文法。否定・条件右辺・scope・参照・依存等を保持し、混在booleanの優先順位や未確定predicate truthを選択しない。
- 同一source identityからProgram IR、TL、completion plan、candidateまでの追跡。公開reportの成功flagは検証済みconstructorの代わりにならない。

source roundtripは原文byteの保存を表します。意味理解の正確性、一般文法の網羅性、独立goldへの一致率は別の評価対象です。

### Coding検証・修復・再開

Codingでは、compiler check、library runtime test、production API接続呼出し、acceptance testを独立した観測として扱います。登録tool、外部Authority receipt、独立intent bindingを必要とします。

CLIのpackage finalizerは条件なしRequired要求の限定profileを扱います。repairは既存Rust fileとrepository/fileのscopeに束縛され、修復前bytesを非上書きbackupへ保存します。変更後snapshotと新しいreceiptで要求を再検証します。SDK repair coordinatorの予算は最大4回で、NoProgress、cycle、stale/expired evidence、未確認作用を区別します。

再開は「保存された完了flagの復元」ではありません。writer lock、append-only journal、別管理の署名headを用い、現在のsource・権限・intent・Cargo観測を再評価します。`MayHaveApplied`は未確認作用であり、成功やrollback済みを意味しません。

### 仮説探索・幾何・GPU

継承した仮説探索は256 distinct worldを8 projectionへ展開し、2,048 evaluation rowsとしてmaterializeします。8 logical lane、bounded sparse relation、device-side reduction、CPU final validation、fault quarantineの実装を含みます。これは2,048回のLLM呼出しを表しません。

離散・連続表現間の蒸留は、同じInstruction ASTの構造をbounded geometryへ送り、離散edge membershipを検査したmaterializationをProgram IRへ戻します。content-state更新がepoch progressになり、IR/TL/plan/candidateのidentityへ結び付きます。幾何的な近さ、探索score、構造membershipからpredicate truthやGrantを生成しません。

GPUの数値経路、CPUとのparity、readback、device timing、overlap、occupancy、device-loss callbackは個別の観測対象です。実装の存在や短いGPU試験を、全hardwareの飽和・常時高速化・研究効果へ読み替えません。

### Program AnalysisとMedia

RPA-00〜39はRust／Python／Assemblyの解析をtyped stageとして整理し、EvidenceStateとClosureState、tool provenanceを分離します。特定toolの実観測と、解析型・adapterの存在は区別します。

Mediaはauthenticated local source、backend、model、license、method、budgetに束縛した計画・candidate・observationを扱い、TranslationEnvelopeへ接続します。YouTube URLや任意JSONだけではValidatedになりません。外部parser、vision、ASR等のbackendやモデルは、個別の条件と観測を必要とします。

### 出力・ホスト・互換性

出力候補の確定、host送信、sink受領、durable replayを分離します。HostOutputReceipt v2、Stop hook、exact output digest、append-only replayのsource経路を持ちますが、ローカルのkey署名は実Codex ingressや人間の独立性を証明しません。

public JSON fieldは削除・renameせず、新しいfield/schemaはadditiveです。明示consumer view v1/v2を分け、strict限定v1は未知fieldとtrueのcompletion/send/commitを拒否します。v2のreported stateは表示であり、証拠や権限の発行ではありません。全ての旧consumerの互換性を保証しません。

### 由来・配布・評価の区別

当初のprivate cloneは独立Git履歴で保存され、選択source snapshotから必要なRust workspace、tests、fixtures、schemas、launchers、hooks、skillsを採用しました。元の履歴、個人state、credentials、cache、receipt root、replay ledger、非開示の上流固有情報は除外しました。現在のworkspaceは13 crateです。

source-only配布とローカル生成binaryの存在は矛盾しません。build成功と、配布binaryに含まれるabsolute build path等の監査を区別しています。[移行契約](migration-contract.md)、[採用manifest](adoption-manifest.md)、[clone検証](clone-verification.md)が由来を記録します。

宣言profileの構築・実行・断線・修復・schema検査は局所的な実装証拠です。独立gold、一般意味精度、ハルシネーション抑制、長期session、性能、実host callback／即時revocation、OS sandbox、opaque build scripts／proc macros、全build world、Assurance未対応は[残存条件](known-limitations.md)として扱います。

## English

### Scope and boundaries

This document covers Epistesys `6.3.2-alpha.2` source implementation. It separates LLM-generated candidates from control of requirements, operation authority, validation evidence, and output states. Model-weight modification, complete arbitrary-language understanding, and enforcement over every host operation are outside scope.

Current references are the [alpha.2 release notes](epistesys-6.3.2-alpha.2-release-notes-2026-10-02.md) and [final verification](../validation/epia2-alpha2-release-verification-2026-10-02.md). The [DGCL execution contract](dgcl-operating-profile-and-closure.md) defines operating conditions; the [compatibility document](dgcl-wire-compatibility.md) defines wire conditions.

<a id="terminology-en"></a>

### Operational terminology

| Term | Meaning in this implementation | Not equivalent to |
| --- | --- | --- |
| Input/source | Original text with revision, digest, and UTF-8 byte spans; some compatibility APIs call it seed | A finalized specification, validated fact, or permission |
| Requirement/obligation | Source-bound realization/validation target with conditions, negation, and scope represented separately | Guessed predicate truth or an executed action |
| Program IR | Source-bound intermediate representation of requirements, relations, and residuals | Proof of arbitrary-program semantic equivalence |
| Projection | Representation transformation with source/target/rule/evidence correspondence | Paraphrase alone or guaranteed meaning preservation |
| TranslationLoss (TL) | Correspondence/omission/contradiction/incomparability records, including ProjectionDefectGraph | A universally calibrated semantic distance or truth value |
| Membrane | Implementation name for a validation boundary between representations/states | A physical membrane or correctness guarantee |
| Candidate | Generated, parsed, or constructed proposal | Validated, Completed, Delivered |
| Evidence | Validation material bound to subject revision, method, and stage | Permission, independent gold, formal proof |
| Receipt | Witness with checked issuer/class/subject/scope/nonce/expiry/parent/payload | A JSON success flag or nonempty signature-like string |
| Authority | Operation/scope-specific permissions originating in a trusted caller/host | Permission to reason, parsed affirmative language, low loss |
| Output commitment | Application state finalizing exact candidate bytes under specified conditions | Git commit, sink receipt, or truth |
| World/evaluation | Budgeted evaluation of premise/hypothesis assignments and projections | An independent LLM agent or physical universe |

English type/command/schema identifiers remain for compatibility. State terms follow the [wire contract](../schemas/closure-and-receipt-contract.md) and validators, not ordinary-language meanings of correct, safe, or complete.

### Dataflow and validation boundaries

| Stage | Principal input/output | Preserved/checked information |
| --- | --- | --- |
| Source preservation | Source → regions/token-region lattice | Revision, digest, byte spans, quote/code roles |
| Syntax/instruction parsing | Regions/CST/dependencies → controlled Instruction AST | Negation, conditions, exceptions, time, scope, references, dependencies, residuals |
| Requirement projection | AST → per-requirement Program IR | Source correspondence, requirement IDs, structural relations, Unsupported/Unresolved/Ambiguous |
| Defect tracking | Source/IR → TL/ProjectionDefectGraph | Correspondence, omission, contradiction, incomparability, verifier capability, evidence gaps |
| Validation planning | Requirements → completion plan/evidence tasks/gap ledger | Target snapshot, evidence kinds, operating conditions, unresolved gaps |
| Operations/observations | Registered tools/external receipts → compiler/runtime/connection/acceptance results | Operation scope, tool provenance, distinct observations, invalidation conditions |
| Candidate finalization | Current receipts/lifecycle → exact candidate | Per-requirement revalidation, gaps, candidate digest, construction seal |
| Host delivery | Candidate → send/sink/replay observations | Stage receipts, exact output bytes, send attempts versus receipt |

The table specifies dependencies, not a claim that diagnostics execute all effectful stages. Authority/observation ingress requires external caller/host contracts; source signatures, schema validity, and tool hashes alone cannot construct permissions or observation tokens.

### Current syntax and requirement parsing

DeepGrammar is this project's parsing-module name. It currently includes:

- Pinned CommonMark event parsing and Rust tree-sitter CST.
- Distinct configured Japanese/English Stanza-worker and no-external-model profiles. Missing worker/model/runtime pins produce diagnostic failures.
- Budgeted `controlled-ja-en-instruction-earley.v2`; preserve negation, condition RHS, scope, references, and dependencies without selecting ambiguous boolean precedence or unknown predicate truth.
- Same-source identity through Program IR/TL/completion plan/candidate. Public-report success flags cannot replace verified constructors.

Exact source roundtrip preserves bytes, not semantic accuracy, general-grammar coverage, or agreement with independent gold.

### Coding validation, repair, and resume

Compiler checks, library runtime tests, production API connection invocations, and acceptance tests are distinct observations. Registered tools, external Authority receipts, and independent intent binding are required.

Package finalization uses a bounded profile of unconditional Required requirements. Repair is scoped to existing Rust files and repository/file identities; preserve preimage bytes in non-overwriting backups. Revalidate changed snapshots with fresh receipts. The SDK repair coordinator permits at most four attempts and separates NoProgress, cycles, stale/expired evidence, and unconfirmed effects.

Resume is not restoration of a saved completion flag. Writer locks, append-only journals, and separately managed signed heads support revalidation of current source/authority/intent/Cargo evidence. `MayHaveApplied` denotes unconfirmed effects, neither success nor completed rollback.

### Hypothesis exploration, geometry, and GPU

Inherited exploration materializes 256 distinct worlds through eight projections as 2,048 evaluation rows, with eight logical lanes, bounded sparse relations, device-side reduction, CPU final validation, and fault quarantine. This does not mean 2,048 LLM calls.

Discrete/continuous distillation projects the same Instruction AST structure into bounded geometry and returns materializations checked against discrete edge membership to Program IR. Changed content state records epoch progress and binds IR/TL/plan/candidate identity. Geometric proximity, search scores, and membership do not mint predicate truth or Grants.

Numeric GPU paths, CPU parity, readback, device timing, overlap, occupancy, and device-loss callbacks are separate observations. Implementation presence or short GPU tests are not hardware-saturation, universal-speedup, or research-effect evidence.

### Program Analysis and Media

RPA-00..39 represents Rust/Python/Assembly analysis as typed stages, separating EvidenceState, ClosureState, and tool provenance. Distinguish actual tool observations from the existence of types/adapters.

Media handles plans/candidates/observations bound to authenticated local source, backend, model, license, method, and budget, connected to TranslationEnvelope. A YouTube URL or arbitrary JSON cannot become Validated. External parsers/vision/ASR backends and models require individual conditions and observations.

### Output, hosts, and compatibility

Separate candidate finalization, host send, sink receipt, and durable replay. Source paths include HostOutputReceipt v2, Stop hooks, exact output digests, and append-only replay; local signatures do not prove real Codex ingress or human independence.

Public JSON fields are not removed/renamed; added fields/schemas are additive. Explicit consumer views v1/v2 distinguish strict limited v1, which rejects unknown fields and true completion/send/commit flags, from v2 reported-state display. Display does not issue evidence/permissions; compatibility with every legacy consumer is unproven.

### Provenance, distribution, and evaluation

The initial private clone established independent Git history and adopted required Rust workspace/tests/fixtures/schemas/launchers/hooks/skills from a selected source snapshot. Original history, personal state, credentials, caches, receipt roots, replay ledgers, and non-disclosable upstream material were excluded. The current workspace contains 13 crates.

Source-only distribution is compatible with locally generated binaries: build success and distribution audits of absolute build paths are separate. See [migration contract](migration-contract.md), [adoption manifest](adoption-manifest.md), and [clone verification](clone-verification.md).

Declared-profile construction/execution/cut/repair/schema checks provide bounded implementation evidence. Independent gold, general semantic accuracy, hallucination containment, long sessions, performance, real-host callbacks/immediate revocation, OS sandboxing, opaque build scripts/proc macros, complete build worlds, and unsupported Assurance scope remain [residual conditions](known-limitations.md).
