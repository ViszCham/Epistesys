# Epistesys 6.3.2-alpha.1 implementation status

## 日本語

この作業ツリーは、Labyrinth-Codex v6.3.1の選択source snapshotをEpistesys（エピステシス）として独立起動できる形へ移植した初回alphaです。receipt検証、TL/TLDG、256×8 world materialization、RPA、candidate-only Media、Host replay v2を実装面として継承しています。内部の`lc631-*` crate・command・schema IDは、移植による挙動差を抑えるため互換識別子として保持します。製品identity、manifest、Cargo version、launcher、hook表示はEpistesysへ変更します。

EPI-05のsource build、feature-reduced/default test、strict clippy、locked release buildが成功しました。生成したrelease binaryは検査時点でclone pathをdebug metadataへ含むため、packaged binaryとして採用せず、target下の検証artifactに留めます。元のGit履歴、旧environment、個人state、cache、credentials、receipt root、replay ledgerはありません。

標語：**シードは仕様ではない。**／**シード以前からコミットまで、すべての射影に証跡を。**

## English

This worktree is the first Epistesys (エピステシス) alpha, migrating the selected Labyrinth-Codex v6.3.1 source snapshot for independent startup. It inherits executable receipt verification, TL/TLDG, 256×8 world materialization, RPA, candidate-only Media, and Host replay v2 surfaces. Internal `lc631-*` crate, command, and schema IDs remain compatibility identifiers to reduce migration behavior drift. Product identity, manifest, Cargo version, launcher, and hook display are changed to Epistesys.

EPI-05 source build, feature-reduced/default tests, strict clippy, and locked release build passed. The generated release binary retained the clone path in debug metadata during inspection, so it remains a validation artifact under target and is not adopted as a packaged binary. There is no original Git history, old environment, personal state, cache, credential, receipt root, or replay ledger.

Slogans: **Seed is not spec.** / **From pre-seed to commit, every projection leaves a witness.**
