# Epistesys 6.3.2-alpha.1 implementation status

## 日本語

これは初回alpha.1の履歴記録です。現在のalpha.2の仕様・検証状態は[アーキテクチャ](system-architecture.md)と[最終検証](../validation/epia2-alpha2-release-verification-2026-10-02.md)を参照してください。

この作業ツリーは、Labyrinth-Codex v6.3.1の選択source snapshotをEpistesys（エピステシス）として独立起動できる形へ移植した初回alphaです。receipt検証、TL/TLDG、256×8 world materialization、RPA、candidate-only Media、Host replay v2を実装面として継承しています。内部の`lc631-*` crate・command・schema IDは、移植による挙動差を抑えるため互換識別子として保持します。製品identity、manifest、Cargo version、launcher、hook表示はEpistesysへ変更します。

EPI-05のsource build、feature-reduced/default test、strict clippy、locked release buildが成功しました。生成したrelease binaryは検査時点でclone pathをdebug metadataへ含むため、packaged binaryとして採用せず、target下の検証artifactに留めます。元のGit履歴、旧environment、個人state、cache、credentials、receipt root、replay ledgerはありません。

設計上は原文、要求、権限、証拠、出力状態を分離し、source revisionと検証条件を保持します。記録された機能と、その研究効果の実証は区別します。

## English

This is an initial-alpha.1 historical record. See the [architecture](system-architecture.md) and [final verification](../validation/epia2-alpha2-release-verification-2026-10-02.md) for current alpha.2 specifications and status.

This worktree is the first Epistesys (エピステシス) alpha, migrating the selected Labyrinth-Codex v6.3.1 source snapshot for independent startup. It inherits executable receipt verification, TL/TLDG, 256×8 world materialization, RPA, candidate-only Media, and Host replay v2 surfaces. Internal `lc631-*` crate, command, and schema IDs remain compatibility identifiers to reduce migration behavior drift. Product identity, manifest, Cargo version, launcher, and hook display are changed to Epistesys.

EPI-05 source build, feature-reduced/default tests, strict clippy, and locked release build passed. The generated release binary retained the clone path in debug metadata during inspection, so it remains a validation artifact under target and is not adopted as a packaged binary. There is no original Git history, old environment, personal state, cache, credential, receipt root, or replay ledger.

The design separates source text, requirements, authority, evidence, and output states while preserving source revisions and validation conditions. Recorded functionality and demonstrated research effects remain distinct.
