# Beta-7.1の変更と確認範囲 / Beta-7.1 changes and verification scope

## 日本語

Epistesys Beta-7.1のidentityは`7.1.0-beta.1`です。同じ`epistesys` pluginの更新版としてBeta-7.0を置き換えます。別pluginは追加しません。EpiMapLoss／EpiRecompScoreの[数式と適用条件](epimaploss-epirecompscore-formulas.md)と互換command・wire IDは維持しています。

### 変更

- Windowsのrustfmt引数長上限とLinuxの整形差分を修復しました。Cargo metadataに宣言された全targetとmoduleを対象にし、空対象・失敗の隠蔽を拒否する制御を追加しています。
- privateなre-exportで途切れていたframe付きSDK実行入口を公開し、consumer向けの借用getterを補いました。descriptorや保持されたbytesから権限・完了を発行しません。
- Windows／Linuxで実際に観測した、同じ時計値によるfixture directory衝突を修復しました。排他的createに成功した場所だけを所有し、既存directoryは再利用・削除しません。
- hook・manifestだけを変更した場合もCIを起動し、既存のWindows byte転送試験をrelease binaryへ接続しました。
- 厳格clippyを実行試験の前に配置し、独立した失敗は`--no-fail-fast`で回収します。失敗の終了コード、既存の負例・期限・resource上限は維持します。

中央セッションで行った[限定監査と修復記録](../validation/ci-repair-audit-2026-11-06.md)、OS別の実際の結果は[PR #11](https://github.com/ViszCham/Epistesys/pull/11)をご覧ください。Beta-7.0の試験結果を新identityのCI成功へ付け替えません。

### 適用と残存条件

pluginは公式の更新経路で同じIDへ適用し、source commit・binary digest・installed version・有効化・実行を分けて確認します。全体既定の指示は新しいinstalled versionへ合わせます。変更したhookの信頼確認は、Codexの利用者側の状態であり、version更新だけでは代替しません。

このCIは設定したWindows／Linux・feature構成の観測です。実host配信、全外部backend、独立gold、一般的な効果・性能、Assurance unknownの解消を意味しません。元の[Beta-7.0記録](epistesys-7.0.0-beta.1-release-notes-2026-11-06.md)は履歴として保持します。

## English

Epistesys Beta-7.1 uses identity `7.1.0-beta.1`. It replaces Beta-7.0 under the same `epistesys` plugin ID, without adding another plugin. The EpiMapLoss/EpiRecompScore [equations and applicability contracts](epimaploss-epirecompscore-formulas.md), command IDs and wire compatibility are retained.

### Changes

- Repair Windows rustfmt argv overflow and Linux formatting drift, preserving every Cargo metadata target and module. Add controls against empty selection and hidden failures.
- Connect the frame-based SDK export chain and borrowed consumer accessors. Descriptors and retained bytes do not mint permission or completion.
- Repair observed Windows/Linux fixture-directory collisions under equal clock values. Acquire ownership only through exclusive creation; never reuse or remove existing directories.
- Trigger CI for hook/manifest-only changes and connect the existing Windows byte-transport test to the release binary.
- Run strict clippy before runtime tests and collect independent failures with `--no-fail-fast`, retaining nonzero failure exits, existing negatives, expiry and resource limits.

See the central session's [bounded audit and repair record](../validation/ci-repair-audit-2026-11-06.md) and actual OS-specific results in [PR #11](https://github.com/ViszCham/Epistesys/pull/11). Beta-7.0 observations are not relabeled as CI success for the new identity.

### Application and residuals

Apply the update through the official plugin path under the same ID. Separately verify source commits, binary digests, installed version, enablement and execution. Align global instructions with the new installed version. Hook trust is a user-side Codex state, not something a version bump establishes.

CI observes the declared Windows/Linux and feature configurations. It does not establish real-host delivery, all external backends, independent gold, general effectiveness/performance or resolution of Assurance unknowns. Preserve the [Beta-7.0 record](epistesys-7.0.0-beta.1-release-notes-2026-11-06.md) as history.
