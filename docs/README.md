# Epistesys documentation

## 日本語

この索引は、現行の仕様、実行条件、検証証跡、評価設計、歴史的な設計記録を区別します。対象は`6.3.2-alpha.2`です。日本語説明の後に、同内容の英語説明を置きます。

### 読者別の参照経路

| 目的 | 読む順序 | 確認できる内容 |
| --- | --- | --- |
| システム工学・情報科学の観点で構造を把握する | [README](../README.md) → [アーキテクチャ](system-architecture.md) | 対象とする失敗、用語、データフロー、信頼境界 |
| sourceを実行・検査する | [実行ガイド](execution-guide.md) → [DGCL実行契約](dgcl-operating-profile-and-closure.md) | 診断、権限・tool条件、修復、再開、適用範囲 |
| interfaceを統合する | [wire互換性](dgcl-wire-compatibility.md) → [schema契約](../schemas/closure-and-receipt-contract.md) | field・stage・consumer view、失効と互換性 |
| 効果を評価する | [研究仮説](research-hypotheses.md) → [前向き評価案](research/epistesys-prospective-evaluation-plan.md) | 仮説、予算統制、holdout、Judge、欠測 |
| 現行状態を監査する | [最終検証](../validation/epia2-alpha2-release-verification-2026-10-02.md) → [既知の制約](known-limitations.md) | 局所gateの結果、研究・配布・hostの未観測範囲 |

### 文書の役割

| 種別 | 現行の参照点 | 読み方 |
| --- | --- | --- |
| 仕様・実行契約 | [アーキテクチャ](system-architecture.md)、[DGCL実行契約](dgcl-operating-profile-and-closure.md)、[wire互換性](dgcl-wire-compatibility.md) | sourceの型・状態・適用条件を説明する |
| release記録 | [alpha.2 release notes](epistesys-6.3.2-alpha.2-release-notes-2026-10-02.md)、[最終検証](../validation/epia2-alpha2-release-verification-2026-10-02.md) | 特定revision・環境で観測した結果を示す |
| 評価設計 | [研究仮説](research-hypotheses.md)、[独立gold収集手順](dgcl-gold-collection-protocol.md)、[前向き評価案](research/epistesys-prospective-evaluation-plan.md) | 未実行の計画と検証済み結果を区別する |
| 由来・除外 | [移行契約](migration-contract.md)、[採用manifest](adoption-manifest.md)、[clone検証](clone-verification.md) | sourceの継承範囲、互換名、非開示情報の除外を示す |
| 補助監査 | [Assurance境界](assurance-compiler-status.md)、[RepoSeiri記録](reposeiri-audit.md) | 監査範囲・未対応・保留を保持する。一般安全性の証明ではない |

現alphaのsource検証、配布検証、実host activation、研究効果は別状態です。candidate／Validated、証拠／権限、出力確定／配信を同一視しません。共通の定義は[用語表](system-architecture.md#terminology-ja)に集約しています。

### GB-CC75：前身の探索的な経験的観測

主要65 QIDの平均適合度は74.38%→94.10%（+19.71ポイント）、167対応ペアの全基準達成率は39.52%→48.50%でした。前身の独自Judge下の結果であり、alpha.2の改善、完全遵守、ハルシネーション抑制、長期保持へ移転しません。

| 文書 | 主な内容 |
| --- | --- |
| [研究本文](research/gb-cc75-study.md) | 要旨、研究問い、方法、結果、妥当性への脅威、考察 |
| [方法・由来](research/gb-cc75-methods-and-provenance.md) | 75→67→65 QID、225→171→167ペア、Judge設計と未確定設定 |
| [統計仕様](research/gb-cc75-statistical-analysis.md) | estimand、反復依存、歴史的区間・検定、限定欠測bound |
| [方法論監査](research/gb-cc75-adversarial-audit.md) | A01〜A12、文書上の対処、未解決の実験課題 |
| [失敗分析](research/gb-cc75-failure-analysis.md) | strict未達、負の差、事後taxonomy、未同梱subgroup |
| [公開集計](../benchmarks/gb-cc75/2026-08-18/README.md) | 選択12ファイル、hash、点推定の再計算 |

### 履歴資料

下記は現在の仕様を読む前提ではありません。必要な実装の由来・過去の判断を調べる際に参照してください。過去のHeld、完了flag、旧identity、検証結果を、現在の状態へ自動的に読み替えません。

<details>
<summary>Epistesysのclone・DGCL・alpha.2の実装履歴</summary>

- [Clone baseline](clone-baseline.md)、[初回実装状態](implementation-status.md)、[初回release](initial-clone-release-record.md)
- [初回commit候補](initial-commit-candidate.md)、[初回preflight](initial-commit-preflight.md)
- [DGCL実装状態：2026-09-28](dgcl-implementation-status-2026-09-28.md)、[alpha.2詳細roadmap](epistesys-6.3.2-alpha.2-dgcl-closure-roadmap-2026-10-01.md)
- [昇格前functional gate](../validation/epia2-prepromotion-functional-pass-2026-10-02.md)、[修復監査](../validation/epia2-repair-blocker-audit-2026-10-02.md)
- [Codex既定ルーティング](codex-default-routing.md)：現在のfacade契約。host強制とは区別する

</details>

<details>
<summary>継承したv6.3.1設計・監査記録</summary>

- [Parser設計記録](v6.3.1-deepgrammar-parser-engine-research-and-design-2026-08-25.md)、[TLDG実装記録](v6.3.1-tldg-implementation-record-2026-08-25.md)
- [ARC631検証記録](v6.3.1-arc631-authenticity-and-closure-implementation-record-2026-08-26.md)
- [Receipt真正性の監査計画](v6.3.1-postdoctoral-receipt-authenticity-adversarial-audit-roadmap-2026-08-26.md)
- [Multimodal／RPA計画](v6.3.1-ultimate-adversarial-audit-and-multimodal-adapter-closure-roadmap-2026-08-25.md)
- [監査と実装roadmap](v6.3.1-adversarial-audit-and-implementation-roadmap-2026-08-25.md)、[V631実装記録](v6.3.1-v631-implementation-record-2026-08-25.md)
- [Host Stop hookの過去観測](v6.3.1-fresh-host-stop-hook-observation-2026-08-26.md)、[commit/review分離計画](v631-commit-and-review-split-plan.md)

</details>

### 文書上の解釈規則

入力を確定した要求とみなさず、解析・解釈・検証の結果を段階ごとに記録します。local test、source hash、schema適合、GPU実行、補助監査は、それぞれの観測範囲を超えた真理、一般性能、権限、配信完了を示しません。未観測事項は、その確認条件とともに[既知の制約](known-limitations.md)へ記録します。

---

## English

This index separates current specifications, operating conditions, verification records, evaluation designs, and historical design notes for `6.3.2-alpha.2`. Equivalent English prose follows Japanese prose.

### Reader-oriented routes

| Purpose | Reading order | Information |
| --- | --- | --- |
| Understand the structure from systems engineering/computer science | [README](../README.md) → [Architecture](system-architecture.md) | Failure model, terminology, dataflow, trust boundaries |
| Execute/inspect source | [Execution guide](execution-guide.md) → [DGCL contract](dgcl-operating-profile-and-closure.md) | Diagnostics, authority/tool conditions, repair, resume, scope |
| Integrate interfaces | [Wire compatibility](dgcl-wire-compatibility.md) → [schema contract](../schemas/closure-and-receipt-contract.md) | Fields/stages/consumer views, invalidation, compatibility |
| Evaluate effects | [Research hypotheses](research-hypotheses.md) → [Prospective plan](research/epistesys-prospective-evaluation-plan.md) | Hypotheses, budgets, holdouts, Judges, missingness |
| Audit current status | [Final verification](../validation/epia2-alpha2-release-verification-2026-10-02.md) → [Known limitations](known-limitations.md) | Local gates and unobserved research/distribution/host scope |

### Document roles

| Category | Current reference | Interpretation |
| --- | --- | --- |
| Specification/operating contract | [Architecture](system-architecture.md), [DGCL contract](dgcl-operating-profile-and-closure.md), [wire compatibility](dgcl-wire-compatibility.md) | Source types/states/preconditions |
| Release record | [Alpha.2 notes](epistesys-6.3.2-alpha.2-release-notes-2026-10-02.md), [final verification](../validation/epia2-alpha2-release-verification-2026-10-02.md) | Observations for a particular revision/environment |
| Evaluation design | [Hypotheses](research-hypotheses.md), [independent-gold protocol](dgcl-gold-collection-protocol.md), [prospective plan](research/epistesys-prospective-evaluation-plan.md) | Unexecuted plans versus verified results |
| Provenance/exclusions | [Migration contract](migration-contract.md), [adoption manifest](adoption-manifest.md), [clone verification](clone-verification.md) | Inheritance, compatibility names, excluded non-disclosable material |
| Advisory audit | [Assurance boundary](assurance-compiler-status.md), [RepoSeiri record](reposeiri-audit.md) | Scope, unsupported cases, and holds; not general-safety proof |

Source verification, distribution verification, real host activation, and research effects are separate states. Candidate/Validated, evidence/authority, and finalization/delivery differ. Definitions are centralized in the [terminology table](system-architecture.md#terminology-en).

### GB-CC75: exploratory predecessor observations

Primary mean adherence over 65 QIDs changed from 74.38% to 94.10% (+19.71 points); all-criteria pass over 167 pairs changed from 39.52% to 48.50%. Results use the predecessor's custom Judge and do not transfer to alpha.2 improvement, complete compliance, hallucination containment, or long retention.

| Document | Content |
| --- | --- |
| [Study](research/gb-cc75-study.md) | Abstract, questions, methods, results, validity threats, discussion |
| [Methods/provenance](research/gb-cc75-methods-and-provenance.md) | 75→67→65 QIDs, 225→171→167 pairs, Judge design, unresolved settings |
| [Statistics](research/gb-cc75-statistical-analysis.md) | Estimands, repetition dependence, historical intervals/tests, limited missingness bounds |
| [Methodological audit](research/gb-cc75-adversarial-audit.md) | A01–A12, documentation corrections, unresolved experiments |
| [Failure analysis](research/gb-cc75-failure-analysis.md) | Strict failures, negative differences, post hoc taxonomy, excluded subgroups |
| [Public aggregates](../benchmarks/gb-cc75/2026-08-18/README.md) | 12 selected files, hashes, point-estimate recomputation |

### Historical records

The following are not prerequisites for the current specification. Consult them for implementation provenance or earlier decisions; do not automatically reinterpret historical Held/completion flags/identities/results as current states.

<details>
<summary>Epistesys clone, DGCL, and alpha.2 implementation history</summary>

- [Clone baseline](clone-baseline.md), [initial implementation](implementation-status.md), [initial release](initial-clone-release-record.md)
- [Initial commit candidate](initial-commit-candidate.md), [initial preflight](initial-commit-preflight.md)
- [DGCL status: 2026-09-28](dgcl-implementation-status-2026-09-28.md), [detailed alpha.2 roadmap](epistesys-6.3.2-alpha.2-dgcl-closure-roadmap-2026-10-01.md)
- [Prepromotion functional gates](../validation/epia2-prepromotion-functional-pass-2026-10-02.md), [repair audit](../validation/epia2-repair-blocker-audit-2026-10-02.md)
- [Default Codex routing](codex-default-routing.md): current facade contract, distinct from host-wide enforcement

</details>

<details>
<summary>Inherited v6.3.1 design/audit history</summary>

- [Parser design](v6.3.1-deepgrammar-parser-engine-research-and-design-2026-08-25.md), [TLDG implementation](v6.3.1-tldg-implementation-record-2026-08-25.md)
- [ARC631 verification](v6.3.1-arc631-authenticity-and-closure-implementation-record-2026-08-26.md)
- [Receipt-authenticity audit plan](v6.3.1-postdoctoral-receipt-authenticity-adversarial-audit-roadmap-2026-08-26.md)
- [Multimodal/RPA plan](v6.3.1-ultimate-adversarial-audit-and-multimodal-adapter-closure-roadmap-2026-08-25.md)
- [Audit/implementation roadmap](v6.3.1-adversarial-audit-and-implementation-roadmap-2026-08-25.md), [V631 implementation](v6.3.1-v631-implementation-record-2026-08-25.md)
- [Historical host Stop-hook observation](v6.3.1-fresh-host-stop-hook-observation-2026-08-26.md), [commit/review separation plan](v631-commit-and-review-split-plan.md)

</details>

### Interpretation policy

Input is not treated as a finalized requirement; interpretation/parsing/verification are recorded by stage. Local tests, source hashes, schema validity, GPU execution, and advisory audits establish neither truth/general performance nor authority/delivery beyond observed scope. Retain unobserved conditions and required checks in [known limitations](known-limitations.md).
