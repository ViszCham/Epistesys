# Epistesys documentation

## 日本語

Epistesys 6.3.2-alpha.2の文書は、clone契約、採否、実行検証、証拠境界、既知の制約を入口から追跡できるように整理します。説明文は日本語を前半、対応する英語を後半に置きます。

### 研究目的と設計

研究目標は、定義した信頼性を維持できる範囲の把握と拡張可能性の検証です。設計目標は制約・権限・状態・証拠の明示保持、defectの可観測性、commit前の再検証です。その結果として信頼性が改善するかは未検証の研究仮説です。[研究仮説と評価方針](research-hypotheses.md)に五つの研究課題・設計手段・測定項目を整理しています。Epistesys固有のbenchmarkと独立評価は未実施です。前身の探索的なGB-CC75観測とは区別します。

### 前身の経験的観測と将来の評価

- [GB-CC75研究本文](research/gb-cc75-study.md)：要旨、研究問い、方法、結果、妥当性への脅威、考察
- [敵対的方法論監査](research/gb-cc75-adversarial-audit.md)：改訂前監査A01〜A12、文書上の対処と未解決の実験課題
- [方法と由来](research/gb-cc75-methods-and-provenance.md)：75→67→65 QID、225→171→167ペア、Judge設計と未確定設定
- [統計仕様](research/gb-cc75-statistical-analysis.md)：estimand、反復依存、歴史的区間・検定、限定欠測bound
- [失敗分析](research/gb-cc75-failure-analysis.md)：strict未達、負の差、事後taxonomyと未同梱subgroupの境界
- [公開集計bundle](../benchmarks/gb-cc75/2026-08-18/README.md)：選択12ファイル、hash、点推定の再計算
- [Epistesys前向き評価案](research/epistesys-prospective-evaluation-plan.md)：未実行・未登録のholdout／独立Judge／予算統制計画

主要65 QIDの平均74.38%→94.10%（差+19.71ポイント）は前身の独自Judge下の結果です。167ペアのstrictは39.52%→48.50%であり、平均は完全遵守ではありません。これをalpha.2の改善やhallucination／長期sessionの効果へ移転しません。

### 実装能力の入口

Epistesysはv6.3.1のsource cloneとして、receipt検証、TL/TLDG、256×8 world budget、RPA-00〜39、candidate-only Media、Host replay v2を実行可能な形で継承しています。build/test/clippy/CLIの検証記録は、単なる計画ではなくcloneで観測した実行結果です。外部receiptやhost pickupが未成立でも、これらのsource経路を未実装とは扱いません。

### 最初に読む文書

- [README](../README.md)：Epistesysの位置付けと起動方法
- [Migration contract](migration-contract.md)：何を継承し何を除外するか
- [Adoption manifest](adoption-manifest.md)：allowlistと非開示情報の検査方針
- [Clone verification](clone-verification.md)：build・test・clippy・CLIの観測
- [Assurance-Compiler status](assurance-compiler-status.md)：16KiB以下Rust単位の補助監査
- [Known limitations](known-limitations.md)：未観測、Unavailable、Hold、Clarify、fallback
- [DGCL implementation status](dgcl-implementation-status-2026-09-28.md)：DeepGrammar/Coding Closureのslice状態と未完了gate
- [DGCL operating profile](dgcl-operating-profile-and-closure.md)：現sourceのgrammar・Coding・修復/再開・蒸留・consumer契約と残存risk
- [DGCL wire compatibility](dgcl-wire-compatibility.md)：additive schema、明示v1/v2 negotiationと旧consumer境界
- [alpha.2 release notes](epistesys-6.3.2-alpha.2-release-notes-2026-10-02.md)：実際の追加機能とsource-only境界
- [alpha.2 final verification](../validation/epia2-alpha2-release-verification-2026-10-02.md)：A2-G00〜13とalpha.2 identity再検証、ResidualRisk
- [Codex default routing](codex-default-routing.md)：alpha.2 facadeの既定選択、stdin、host境界
- [DGCL独立gold収集手順](dgcl-gold-collection-protocol.md)：未作成の評価データを独立に収集・裁定する条件
- [RepoSeiri audit](reposeiri-audit.md)：repository scopeの構成・文言・hold記録
- [Initial clone release](initial-clone-release-record.md)：initial commitとmain bootstrap

### 継承元記録の扱い

同じdirectoryのv6.3.1設計・監査文書は、実装の由来と歴史的状態を保存するためのものです。そこにある旧line名、旧package identity、旧検証結果は、Epistesys 6.3.2-alpha.2の現在状態へ自動昇格しません。現在のクローンidentityと検証は、root README、clone verification、known limitationsを基準にします。

### 主張境界

local test、低loss、schemaの存在、GPU実行、source hash、Assurance statusは、それぞれの観測範囲を超えて正しさ、一般性能、形式証明、host activation、権限、release approvalを生成しません。未観測範囲を削除せず、次の検証条件と一緒に記録します。

## English

Epistesys 6.3.2-alpha.2 documentation is organized so that clone contract, disposition, execution validation, evidence boundaries, and known limitations can be followed from the entry point. Explanatory prose places Japanese first and equivalent English second.

### Research purpose and design

The research goal is to characterize reliable coverage and test its possible expansion. Design objectives are explicit retention of constraints, authority, state, and evidence, observable defects, and revalidation before commitment. Whether these improve reliability is an untested research hypothesis. The [research hypotheses and evaluation plan](research-hypotheses.md) maps five research questions to design mechanisms and measurements. Epistesys-specific benchmarking and independent evaluation are pending, separate from exploratory predecessor GB-CC75 observations.

### Predecessor observations and future evaluation

- [GB-CC75 study](research/gb-cc75-study.md): abstract, questions, methods, results, validity threats, discussion
- [Adversarial methodological audit](research/gb-cc75-adversarial-audit.md): pre-revision A01–A12, documentation corrections, unresolved experiments
- [Methods/provenance](research/gb-cc75-methods-and-provenance.md): 75→67→65 QIDs, 225→171→167 pairs, Judge design, unresolved settings
- [Statistical specification](research/gb-cc75-statistical-analysis.md): estimands, repeated dependence, historical intervals/tests, limited missingness bounds
- [Failure analysis](research/gb-cc75-failure-analysis.md): strict failures, negative differences, post hoc taxonomy and unpublished subgroup boundaries
- [Public aggregate bundle](../benchmarks/gb-cc75/2026-08-18/README.md): 12 selected files, hashes, point-estimate recomputation
- [Epistesys prospective plan](research/epistesys-prospective-evaluation-plan.md): unexecuted/unregistered holdout, independent-Judge, and budget-control proposal

Primary mean 74.38%→94.10% (+19.71 points) across 65 QIDs is a predecessor result under its custom Judge. Strict pass across 167 pairs is 39.52%→48.50%; means are not complete compliance. Do not transfer this to alpha.2 improvement or hallucination/long-session effects.

### Entry point for implemented capabilities

As a v6.3.1 source clone, Epistesys inherits executable surfaces for receipt verification, TL/TLDG, the 256×8 world budget, RPA-00 through RPA-39, candidate-only Media, and Host replay v2. Build, test, clippy, and CLI records are observed executions in the clone rather than plans. Missing external receipts or host pickup do not make these source paths nonexistent.

### Read first

- [README](../README.md): Epistesys position and startup
- [Migration contract](migration-contract.md): what is inherited and excluded
- [Adoption manifest](adoption-manifest.md): allowlist and non-disclosable-information policy
- [Clone verification](clone-verification.md): build, test, clippy, and CLI observations
- [Assurance-Compiler status](assurance-compiler-status.md): bounded review of Rust units no larger than 16 KiB
- [Known limitations](known-limitations.md): unobserved, Unavailable, Hold, Clarify, and fallback states
- [DGCL implementation status](dgcl-implementation-status-2026-09-28.md): DeepGrammar/Coding Closure slice state and incomplete gates
- [DGCL operating profile](dgcl-operating-profile-and-closure.md): current-source grammar, Coding, repair/resume, distillation, consumer contracts, and residual risks
- [DGCL wire compatibility](dgcl-wire-compatibility.md): additive schemas, explicit v1/v2 negotiation, and legacy-consumer boundaries
- [Alpha.2 release notes](epistesys-6.3.2-alpha.2-release-notes-2026-10-02.md): actual additions and source-only boundaries
- [Alpha.2 final verification](../validation/epia2-alpha2-release-verification-2026-10-02.md): A2-G00..13, alpha.2 identity revalidation, and residual risks
- [Codex default routing](codex-default-routing.md): alpha.2 facade defaults, stdin, and host boundaries
- [Independent DGCL gold collection protocol](dgcl-gold-collection-protocol.md): conditions for independent collection and adjudication of the missing evaluation corpus
- [RepoSeiri audit](reposeiri-audit.md): repository-scope structure, wording, and hold record
- [Initial clone release](initial-clone-release-record.md): initial commit and main bootstrap

### Treatment of inherited records

The v6.3.1 design and audit documents in this directory preserve implementation provenance and historical states. Their older line names, package identities, and validation results do not automatically promote into the current Epistesys 6.3.2-alpha.2 state. The root README, clone verification, and known limitations define the current clone identity and evidence.

### Claim boundary

Local tests, low loss, schema presence, GPU execution, source hashes, and Assurance status do not create correctness, general performance, formal proof, host activation, authority, or release approval beyond their observed scopes. Retain unobserved ranges with the conditions required for their next verification.
