# CI修復と限定監査 / CI repair and bounded audit

## 日本語

対象はBeta-7.0のCI断線と、実際に表面化した障害です。中央セッションだけで修復し、全コードの再読や一般的な安全性の証明は行っていません。

| 所見 | 修復と確認方法 |
| --- | --- |
| Windowsの整形commandが引数長上限を超える | Cargo metadataの全targetを短い引数単位へ分割。editionとout-of-line moduleを保持。空対象・対象漏れ・先行失敗の隠蔽に対する負例を追加 |
| Linuxで整形差分が残る | pin済みrustfmtによる機械整形。assertion、受入閾値、ignored指定は整形のために変更しない |
| SDKのframe実行入口がprivateなre-exportで途切れる | resource→process→crateの公開経路を接続し、外部consumerからのconstructor・拒否・binding変更の契約試験を追加。descriptorはGrantではない |
| capture・observerの保持情報をconsumerが参照できない | planの予定入力、eventのchild、witnessのstdout／stderrに借用getterを追加。実行・権限・鮮度の証明とは区別 |
| Linux／Windowsのfixture名衝突 | 排他的createでのみ所有権を取得し、AlreadyExistsだけを上限付きで別名へ再試行。既存directoryは再利用・削除しない。固定nonceの衝突回帰を追加 |
| hookだけの変更がRust CI対象外 | triggerへhook・manifest・bridge scriptを追加し、release binaryを使う既存Windows byte転送試験を接続 |

厳格clippyの`-D warnings`は維持しています。失敗時ownerのinline返却・native history保持・独立capability引数は、未予算のheap割当や公開API変更を避けるため、該当箇所だけに理由付き`expect`を置いています。これらは機能の正確性の証明ではありません。共有test helperの未使用項目はtest-onlyの限定allowにまとめ、重複属性は除去しました。production全体やCIへの一括allowは追加していません。

監査でClippyの機械提案も照合し、可変chunkへの提案は`iter_mut`を保つよう修正しました。元のreject条件・期限・resource上限・正常系と負例を維持します。fixtureの衝突を避けるために試験の並列性を無効化していません。CIはStrict clippyを実行試験より前へ移し、試験は`--no-fail-fast`で独立した失敗を一度に回収します。失敗の終了コードは引き続き非零です。

ローカルでは全workspace／全targetのstrict clippyと、公開入口・動的時計の対象試験を確認しました。最終のOS別結果は[PR #11](https://github.com/ViszCham/Epistesys/pull/11)のCIに記録します。実host callback・外部backend・独立gold・一般性能は本監査の範囲外です。既存の作業差分、鍵、model、再開状態と証跡は保持します。

## English

Scope is Beta-7.0 CI disconnections and failures actually surfaced during verification. Repairs use only the central session, without another complete-code reading or a general safety proof.

| Finding | Repair and verification |
| --- | --- |
| Windows format command exceeds argv limits | Batch every Cargo metadata target into bounded arguments while preserving editions and out-of-line modules. Add negative controls for empty selection, omissions and hidden earlier failures |
| Linux formatting drift | Mechanical formatting with pinned rustfmt, without changing assertions, acceptance thresholds or ignored states for formatting |
| SDK frame-dispatch export chain ends privately | Connect resource→process→crate exports and test constructor rejection/binding changes from an external consumer. Descriptors are not Grants |
| Retained capture/observer data inaccessible to consumers | Add borrowed accessors for planned input, observed children and retained stdout/stderr. These are not execution, authority or freshness proofs |
| Fixture-name collisions on Linux/Windows | Acquire ownership through exclusive directory creation; bounded retry only on AlreadyExists. Never reuse or remove existing directories. Add fixed-nonce collision regressions |
| Hook-only changes miss Rust CI | Trigger on hooks, manifests and bridge scripts; connect the existing Windows byte-transport test using the release binary |

Strict clippy retains `-D warnings`. Original failure owners, native history and independent capability arguments use narrowly justified `expect` attributes rather than unbudgeted heap allocations or public API changes. These attributes do not prove correctness. Shared test-helper unused surfaces use test-only local allows, with duplicate attributes removed. No production-wide or CI-wide allow was added.

The audit also checked Clippy's mechanical suggestions: mutable chunk iteration retains `iter_mut`. Original rejection conditions, expiry, resource limits, positives and negatives remain. Test parallelism is not disabled to hide fixture collisions. Strict clippy now precedes runtime tests, and `--no-fail-fast` collects independent failures while retaining nonzero failure exits.

Local checks cover strict clippy over the full workspace/all targets and affected public-entry/dynamic-clock tests. Final OS-specific outcomes are recorded by CI on [PR #11](https://github.com/ViszCham/Epistesys/pull/11). Real-host callbacks, external backends, independent gold and general performance are outside this audit. Existing differences, keys, models, resume state and evidence are preserved.
