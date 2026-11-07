# Epistesys 文書索引 / Documentation index

[日本語](#日本語) · [English](#english)

## 日本語

初めて使う方は[会話とコーディングの支援](coding-and-conversation-assistance.md#日本語)から、手元で動かす方は[実行ガイド](execution-guide.md#日本語)から始めてください。現在の版は **Beta-7.1**（`7.1.0-beta.1`）です。

### 読者別の参照経路

| 知りたいこと | 読む文書 |
| --- | --- |
| 相談だけで使えるか。どの言語でどんな検証ができるか。 | [相談の例と検証の例](coding-and-conversation-assistance.md#日本語) · [言語ごとの対応範囲](coding-and-conversation-assistance.md#languages-ja) |
| 依頼を診断したい。変更後にテストをやり直したい。 | [実行ガイド](execution-guide.md#日本語) · [検証・修復・再開の実行条件](dgcl-operating-profile-and-closure.md#日本語) |
| Codexで使いたい。別のツールから結果を読みたい。 | [プラグインの接続](plugin-host-integration.md#日本語) · [呼び出す入口の選択](codex-default-routing.md#日本語) · [データ形式と互換性](dgcl-wire-compatibility.md) |
| 要求が検証計画になるまでの処理を追いたい。 | [アーキテクチャと用語](system-architecture.md#日本語) · [完了判定・実行記録のスキーマ](../schemas/closure-and-receipt-contract.md) |
| 写像保存損失や構造再編評価値を計算したい。 | [EpiMapLoss](epimaploss-epirecompscore-formulas.md#map-ja) · [EpiRecompScore F1・F2](epimaploss-epirecompscore-formulas.md#score-ja) · [全40成分と計算例](epimaploss-epirecompscore-formulas.md#catalog-ja) · [測定契約](tlcx-tl-screative-measurement-contract-v1-2026-10-29.md) |
| 現行版で何を確かめたか知りたい。 | [Beta-7.1の検証記録](epistesys-7.1.0-beta.1-release-notes-2026-11-06.md#日本語) · [既知の制約](known-limitations.md#日本語) |

### 文書の役割

- [TLCXロードマップ](epistesys-tlcx-contextual-multilogic-loss-creativity-roadmap-2026-10-28.md)は、実装の依存関係と各段階の完了条件をまとめたものです。[用語表](system-architecture.md#terminology-ja)では、解析候補、検証済みの結果、実行権限、出力の受領を区別しています。
- [設計の評価項目](research-hypotheses.md)、[評価用データの収集手順](dgcl-gold-collection-protocol.md)、[前向き評価案](research/epistesys-prospective-evaluation-plan.md)は、開発での効果を今後どう確かめるかを説明しています。
- 実装の由来は[移行契約](migration-contract.md)、[採用一覧](adoption-manifest.md)、[初期コピーの検証](clone-verification.md)を参照してください。監査ごとの対象範囲は[Assurance](assurance-compiler-status.md)、[RepoSeiri](reposeiri-audit.md)、[コーディング支援の文書監査](../validation/coding-documentation-adversarial-review.md)で確認できます。

### GB-CC75：前身システムのベンチマーク

Labyrinth-Codexを対象とした当時の集計・評価です。[評価報告](research/gb-cc75-study.md)に結果と限界があります。根拠をたどる際は[課題と採点方法](research/gb-cc75-methods-and-provenance.md)、[統計仕様](research/gb-cc75-statistical-analysis.md)、[方法論監査](research/gb-cc75-adversarial-audit.md)、[失敗分析](research/gb-cc75-failure-analysis.md)、[再計算用の集計](../benchmarks/gb-cc75/2026-08-18/README.md)を参照してください。

### 履歴資料

各記録は、その版の実装や検証について記述しています。[Beta 7.0](epistesys-7.0.0-beta.1-release-notes-2026-11-06.md) · [Beta 6.9](epistesys-6.9.0-beta.1-source-snapshot-2026-11-05.md) · [alpha.3の変更](epistesys-6.3.2-alpha.3-release-notes-2026-10-27.md) · [alpha.3の検証](../validation/epr-release-verification-2026-10-27.md)

<details>
<summary>Epistesysのclone・DGCL・alpha.2の実装履歴</summary>

- [Clone baseline](clone-baseline.md)、[初回実装状態](implementation-status.md)、[初回release](initial-clone-release-record.md)
- [初回commit候補](initial-commit-candidate.md)、[初回preflight](initial-commit-preflight.md)
- [DGCL実装状態：2026-09-28](dgcl-implementation-status-2026-09-28.md)、[alpha.2詳細roadmap](epistesys-6.3.2-alpha.2-dgcl-closure-roadmap-2026-10-01.md)
- [alpha.2 release notes](epistesys-6.3.2-alpha.2-release-notes-2026-10-02.md)、[alpha.2最終検証](../validation/epia2-alpha2-release-verification-2026-10-02.md)
- [昇格前functional gate](../validation/epia2-prepromotion-functional-pass-2026-10-02.md)、[修復監査](../validation/epia2-repair-blocker-audit-2026-10-02.md)

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

---

## English

For a first example, start with [conversation and coding assistance](coding-and-conversation-assistance.md#english). To run the tool, open the [execution guide](execution-guide.md#english). The current version is **Beta-7.1** (`7.1.0-beta.1`).

### Reader-oriented routes

| What would you like to know? | Where to read |
| --- | --- |
| Can I use it for a discussion? Which languages and checks does it support? | [Discussion and validation examples](coding-and-conversation-assistance.md#english) · [Language coverage](coding-and-conversation-assistance.md#languages-en) |
| How do I inspect a request or repeat checks after an edit? | [Execution guide](execution-guide.md#english) · [Validation, repair and resume conditions](dgcl-operating-profile-and-closure.md#english) |
| How do I use it in Codex or read its results from another tool? | [Plugin integration](plugin-host-integration.md#english) · [Entry point selection](codex-default-routing.md#english) · [Wire format and compatibility](dgcl-wire-compatibility.md) |
| How does a requirement become a validation plan? | [Architecture and terminology](system-architecture.md#english) · [Completion and receipt schema](../schemas/closure-and-receipt-contract.md) |
| How do I calculate mapping loss and recomposition scores? | [EpiMapLoss](epimaploss-epirecompscore-formulas.md#map-en) · [EpiRecompScore F1/F2](epimaploss-epirecompscore-formulas.md#score-en) · [All 40 components and examples](epimaploss-epirecompscore-formulas.md#catalog-en) · [Measurement contract](tlcx-tl-screative-measurement-contract-v1-2026-10-29.md) |
| What has been checked in this version? | [Beta-7.1 verification record](epistesys-7.1.0-beta.1-release-notes-2026-11-06.md#english) · [Known limitations](known-limitations.md#english) |

### Document roles

- The [TLCX roadmap](epistesys-tlcx-contextual-multilogic-loss-creativity-roadmap-2026-10-28.md) sets out dependencies and completion conditions for each stage. The [glossary](system-architecture.md#terminology-en) distinguishes analysis candidates, validated results, execution authority and output receipt.
- The [evaluation criteria](research-hypotheses.md), [reference-data collection protocol](dgcl-gold-collection-protocol.md) and [prospective evaluation plan](research/epistesys-prospective-evaluation-plan.md) describe how effects on development could be measured.
- For implementation provenance, see the [migration contract](migration-contract.md), [adoption manifest](adoption-manifest.md) and [initial clone verification](clone-verification.md). For the scope of specific audits, see [Assurance](assurance-compiler-status.md), [RepoSeiri](reposeiri-audit.md) and the [coding assistance documentation review](../validation/coding-documentation-adversarial-review.md).

### GB-CC75: predecessor benchmark

These records evaluate the predecessor, Labyrinth-Codex. The [study](research/gb-cc75-study.md) reports results and limitations. Supporting material covers [tasks and scoring](research/gb-cc75-methods-and-provenance.md), [statistics](research/gb-cc75-statistical-analysis.md), [methodological audit](research/gb-cc75-adversarial-audit.md), [failure analysis](research/gb-cc75-failure-analysis.md) and [recomputable aggregates](../benchmarks/gb-cc75/2026-08-18/README.md).

### Historical records

Each record describes implementation or verification for the version it names. [Beta 7.0](epistesys-7.0.0-beta.1-release-notes-2026-11-06.md) · [Beta 6.9](epistesys-6.9.0-beta.1-source-snapshot-2026-11-05.md) · [alpha.3 changes](epistesys-6.3.2-alpha.3-release-notes-2026-10-27.md) · [alpha.3 verification](../validation/epr-release-verification-2026-10-27.md)

<details>
<summary>Epistesys clone, DGCL, and alpha.2 implementation history</summary>

- [Clone baseline](clone-baseline.md), [initial implementation](implementation-status.md), [initial release](initial-clone-release-record.md)
- [Initial commit candidate](initial-commit-candidate.md), [initial preflight](initial-commit-preflight.md)
- [DGCL status: 2026-09-28](dgcl-implementation-status-2026-09-28.md), [detailed alpha.2 roadmap](epistesys-6.3.2-alpha.2-dgcl-closure-roadmap-2026-10-01.md)
- [Alpha.2 release notes](epistesys-6.3.2-alpha.2-release-notes-2026-10-02.md), [alpha.2 final verification](../validation/epia2-alpha2-release-verification-2026-10-02.md)
- [Prepromotion functional gates](../validation/epia2-prepromotion-functional-pass-2026-10-02.md), [repair audit](../validation/epia2-repair-blocker-audit-2026-10-02.md)

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
