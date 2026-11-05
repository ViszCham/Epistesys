# Epistesys Alpha.3 maintenance guide

## 日本語

### この保守ラインについて

`main-alpha.3`は、Epistesys `6.3.2-alpha.3`の参照点を、後続の開発と分けて保持するブランチです。既存の`main`を置き換えるものではなく、リポジトリの既定ブランチや公開範囲も変更しません。将来の更新頻度、サポート期間、新しいreleaseの提供を約束するものではありません。

保存元は、既存の[alpha.3 release PR #5](https://github.com/ViszCham/Epistesys/pull/5)に含まれるcommit [`1fca5165eca21011bcefd6518d3b3c9b93e17d58`](https://github.com/ViszCham/Epistesys/commit/1fca5165eca21011bcefd6518d3b3c9b93e17d58)です。保守作業開始時に、このcommitのtreeが同PRのmerge commit `c4ffe5cc06b92f16b0801bca51c474934113c1d0`のtreeと一致し、plugin manifestとRust workspaceのversionがともに`6.3.2-alpha.3`であることを確認しました。

今回の整理はREADME、文書索引、この案内に限定しています。ソース実装、schema、fixture、依存関係、launcher、version、過去の検証記録は変更しません。後続のTLCXやBeta-7.0の実装・作業資料・未完了の主張は取り込みません。個人設定、credentials、cache、receipt root、replay ledger、host絶対pathも持ち込みません。

### おすすめの読み順

1. [README](../README.md)：研究対象、実装機構、研究目標と観測結果の区別。
2. [Alpha.3 release notes](epistesys-6.3.2-alpha.3-release-notes-2026-10-27.md)と[検証記録](../validation/epr-release-verification-2026-10-27.md)：この版の変更と、当時のrevision・環境に対応するgate。
3. [実行ガイド](execution-guide.md)と[DGCL実行契約](dgcl-operating-profile-and-closure.md)：診断、実行権限、登録tool、修復・再開の条件。
4. [既知の制約](known-limitations.md)と[移行契約](migration-contract.md)：未観測範囲、sourceの由来、除外条件。

さらに詳しい資料は[文書索引](README.md)にあります。[Workflow定義](../.github/workflows/)と[ignore設定](../.gitignore)も参照できますが、設定の存在だけで特定commitのCI成功や秘密情報の不存在が確認されたことにはなりません。

### 検証結果の読み方

この保守作業では文書・参照先・versionの整合を確認します。過去のfunctional gateを今回の再実行結果へ読み替えず、Rust実装、GPU、Media、実hostの再検証と区別します。文書監査の候補、警告、保留は、それぞれの観測範囲のまま扱います。未定義のlicense、security、support方針を監査結果から新たに作成するものではありません。

Alpha.3はsource-onlyの実験版です。ソース経路の検証、packageの検証、plugin install、host pickup、研究評価は別状態です。`PendingNoCorpus`と`PendingHostObservation`を保持し、一般性能、安全性、意味精度、形式証明、host全体の強制が実証されたとは述べません。候補や低いlossは証拠・権限にならず、`Unavailable`、`Unsupported`、`Unknown`、`NeedsEvidence`、`Clarify`、`Hold`、未観測を成功へ置き換えません。

---

## English

### About this maintenance line

`main-alpha.3` preserves a reference point for Epistesys `6.3.2-alpha.3` separately from subsequent development. It does not replace `main` or change the repository's default branch or visibility. It makes no promise about update frequency, support duration, or future releases.

The baseline is commit [`1fca5165eca21011bcefd6518d3b3c9b93e17d58`](https://github.com/ViszCham/Epistesys/commit/1fca5165eca21011bcefd6518d3b3c9b93e17d58), included in the existing [alpha.3 release PR #5](https://github.com/ViszCham/Epistesys/pull/5). At the start of maintenance work, its tree was checked against that PR's merge commit, `c4ffe5cc06b92f16b0801bca51c474934113c1d0`, and found identical. The plugin manifest and Rust workspace both reported version `6.3.2-alpha.3`.

This update is limited to the README, documentation index, and this guide. It changes no source implementation, schema, fixture, dependency, launcher, version, or historical verification record. It imports no later TLCX or Beta-7.0 implementation, working material, or unfinished claims. Personal settings, credentials, caches, receipt roots, replay ledgers, and absolute host paths are also excluded.

### Suggested reading order

1. [README](../README.md): research scope, implementation mechanisms, and the distinction between research objectives and observations.
2. [Alpha.3 release notes](epistesys-6.3.2-alpha.3-release-notes-2026-10-27.md) and [verification record](../validation/epr-release-verification-2026-10-27.md): changes in this version and gates bound to their historical revisions and environments.
3. [Execution guide](execution-guide.md) and [DGCL execution contract](dgcl-operating-profile-and-closure.md): conditions for diagnostics, authority, registered tools, repair, and resume.
4. [Known limitations](known-limitations.md) and [migration contract](migration-contract.md): unobserved scope, source provenance, and exclusions.

The [documentation index](README.md) provides further references. [Workflow definitions](../.github/workflows/) and [ignore configuration](../.gitignore) are also available; configuration presence alone does not establish CI success for a particular commit or the absence of secrets.

### Interpreting verification results

This maintenance work checks documentation, references, and version consistency. It does not reinterpret historical functional gates as new runs, and it remains separate from revalidation of Rust implementations, GPU, Media, or real hosts. Documentation-audit candidates, warnings, and holds retain their observed scope. Audit results do not create undefined licensing, security, or support policies.

Alpha.3 is an experimental source-only version. Source-path verification, package verification, plugin installation, host pickup, and research evaluation are separate states. Retain `PendingNoCorpus` and `PendingHostObservation`; general performance, safety, semantic accuracy, formal proof, and host-wide enforcement are not established. Candidates and low loss are neither evidence nor authority. Do not turn `Unavailable`, `Unsupported`, `Unknown`, `NeedsEvidence`, `Clarify`, `Hold`, or unobserved states into success.
