# Plugin接続と起動 / Plugin integration and invocation

## 日本語

Beta-7.1のplugin identityは`epistesys`、versionは`7.1.0-beta.1`です。`lc631-*`は既存command・wireの互換識別子です。個人の全session既定はhost側の設定と利用者指示で選び、この公開repositoryから他の利用者へ強制しません。

### 通常turnの最小経路

codingとconversationのfacadeを一つだけ使い、元入力をUTF-8 stdinで次の入口へ渡します。

```powershell
pwsh -NoLogo -NoProfile -NonInteractive -File ./scripts/run-epistesys.ps1 lc631-dgcl-run --prompt-stdin
```

automationは`ProcessStartInfo`でstdinのUTF-8 encodingを設定し、元byte相当の文字列を書いてEOFを閉じます。入力へBOMや改行を勝手に追加せず、stdout/stderrを並行して回収し、deadlineと実process終了コードを確認します。PowerShell scriptへの直接pipelineによるparameter bindingや、長いpromptのprocess引数化は避けます。

通常はDGCLを一度実行し、version、source roundtrip、Program IR・EpiMapLoss・plan・candidateのbindingと未解決状態を確認します。同一turn・同じ入力・同じbinary・同じ対象snapshotに限り、同じ診断を参照できます。source、tool、権限、時刻や期限に依存する検証結果は使い回しません。重要な差分後は影響先を再検証します。

TLDG／TL／world／repository doctorは、該当する詳細診断が必要なときだけ使います。通常turnで全doctorやGPU診断を一律に起動しません。診断の再利用はturn-localで、promptや候補の永続cacheは生成しません。

### LauncherとStop bridge

通常launcherは配置場所から同梱binaryを解決し、binaryがなければbuildを始めず失敗します。開発buildだけに明示opt-inと専用の絶対target pathを要求します。共有buildをturnごとに複製しません。

Stop hookは元のstdinをPowerShellで読み直したりJSONへ再変換したりせず、同梱native `lc631-host-stop-hook`へ渡します。Windows bridgeはNoLogo／NoProfile／NonInteractiveで起動します。hookの戻り値・byte binding・replay観測は、user権限や送信先の配信成功とは別です。`PLUGIN_DATA`の永続状態や鍵をsourceへ同梱しません。

### 登録・適用の区別

sourceのversion更新、Git merge、marketplace登録、cache install、enabled設定、既存chatでの読み込み、実callbackは別々に記録します。公式のmarketplace/install操作を使用し、古い版のcacheを手動で別版の内容へ上書きしません。別version directoryを使うことで、旧版と再開状態を保持できます。新規chatの読み込みと、既存chatのhot reload／再起動後の観測も区別します。

通常の診断成功は操作権限・一般性能・形式証明を作りません。現在の試験範囲と残存条件は[Beta-7.1記録](epistesys-7.1.0-beta.1-release-notes-2026-11-06.md)、修復・再開の条件は[DGCL実行契約](dgcl-operating-profile-and-closure.md)をご覧ください。

## English

The Beta-7.1 plugin identity is `epistesys`, version `7.1.0-beta.1`. Existing `lc631-*` command and wire identifiers remain compatible. A personal default across sessions is chosen through host settings and user instructions; this public repository does not impose it on other users.

### Minimal ordinary-turn route

Select one coding or conversation facade and pass the original input through UTF-8 stdin:

```powershell
pwsh -NoLogo -NoProfile -NonInteractive -File ./scripts/run-epistesys.ps1 lc631-dgcl-run --prompt-stdin
```

Automation sets UTF-8 stdin encoding through `ProcessStartInfo`, writes the original text, and closes EOF without adding a BOM or newline. Drain stdout/stderr concurrently and check deadlines and the actual process exit. Avoid direct PowerShell-script pipeline parameter binding and long prompts in process arguments.

Normally run DGCL once and check version, source roundtrip, Program IR/EpiMapLoss/plan/candidate bindings and unresolved states. Reference the same diagnostic only within a turn with unchanged input, binary and target snapshot. Do not reuse validations dependent on changed source, tools, authority, current time or expiry. Revalidate affected consumers after meaningful changes.

Use TLDG/TL/world/repository doctors only for the relevant detailed diagnosis, not every doctor or GPU diagnostic each turn. Reuse is turn-local and creates no persistent prompt or candidate cache.

### Launcher and Stop bridge

The normal launcher resolves its packaged binary relative to its installation and fails without starting a build if it is missing. Development builds alone require explicit opt-in and a dedicated absolute target path. Do not duplicate a shared build for each turn.

The Stop hook forwards original stdin to the packaged native `lc631-host-stop-hook` without reading or reserializing JSON in PowerShell. The Windows bridge starts with NoLogo/NoProfile/NonInteractive. Hook results, byte binding and replay observations are distinct from user authority or sink-delivery success. Never bundle `PLUGIN_DATA` state or keys into source.

### Registration and activation states

Record source versions, Git merges, marketplace registration, cache installation, enablement, existing-chat pickup and actual callbacks separately. Use official marketplace/install operations; never overwrite an old-version cache with another version. Separate version directories preserve the previous installation and resume state. Distinguish new-chat loading from existing-chat hot reload or post-restart observation.

A successful diagnostic creates no permission, general-performance result or formal proof. See the [Beta-7.1 record](epistesys-7.1.0-beta.1-release-notes-2026-11-06.md) for tests and residuals, and the [DGCL execution contract](dgcl-operating-profile-and-closure.md) for repair/resume conditions.
