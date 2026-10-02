# Codex default routing / Codex既定ルーティング

## 日本語

Epistesys 6.3.2-alpha.2を既定に設定したhostでは、明示的な別engine/version指定がない限り、非codingに`epistesys:lc631-conversation-projector`、coding/review/testに`epistesys:lc631-coding-projector`を一つだけ使用する。既存Labyrinth lineは比較・明示指定用に保持し、通常の二重実行や故障時の無断fallbackは行わない。この方針はuser instructionとplugin/skill設定であり、全sessionの実行を数学的に保証するhost interceptionではない。

alpha.2の主経路は`lc631-dgcl-run`で、同sourceのDeepGrammar→Program IR→TL→plan→candidateを保持する。外部modelなしとconfigured Stanzaを区別し、unknown/Unsupported/未検証をtruthや権限へ昇格しない。通常会話のsource-candidate Holdを返答禁止と同一視せず、明示的に許可された作業への権限判断は元user instructionから別に行う。危険作用、実装完了、host送信はそれぞれの検証・権限を必要とする。

PowerShellでstdinを使う場合、scriptへの直接pipelineではなく、stdinを受けるnative `pwsh -File` processへ渡す。

```powershell
'Please preserve source evidence.' | pwsh -NoProfile -File ./scripts/run-epistesys.ps1 lc631-dgcl-run --prompt-stdin
```

automationでは`ProcessStartInfo.StandardInputEncoding`をUTF-8にし、元seedをstdinへ書き、EOFと全体deadlineを明示する。seedをprocess argumentやGit文書へ記録しない。返却version `6.3.2-alpha.2`を確認する。旧v6の67-stage full-compute parityは、このDGCL routeから主張しない。

source release、plugin登録/cache install、enabled設定、host pickup、再起動後の実観測を分離する。manifestやdirectoryだけではhost activationの証拠にならない。pluginの追加はhost公式marketplace/install経路を使い、cache/model/keyをsource repositoryへ持ち込まない。個人の既定選択をこの公開repositoryから他人の設定へ自動適用しない。

## English

On hosts configured to use Epistesys 6.3.2-alpha.2 by default, select exactly one facade unless the user explicitly chooses another engine/version: `epistesys:lc631-conversation-projector` for non-coding, `epistesys:lc631-coding-projector` for coding/review/tests. Retain existing Labyrinth lines for comparisons/explicit selection, without routine double execution or silent failure fallback. This policy is implemented through user instructions and plugin/skill settings, not mathematically guaranteed host interception across sessions.

Alpha.2's primary route is `lc631-dgcl-run`, retaining same-source DeepGrammar→Program IR→TL→plan→candidate. Distinguish no-external-model and configured Stanza profiles; unknown/Unsupported/unverified states never become truth or authority. A conversational source-candidate Hold is not a ban on responding; authorization for explicitly requested work is separately classified from the original user instruction. Dangerous effects, implementation completion, and host sending require their own validation/authority.

For PowerShell stdin, pipe to a native `pwsh -File` process that receives stdin, not directly to the script's PowerShell parameter binding.

```powershell
'Please preserve source evidence.' | pwsh -NoProfile -File ./scripts/run-epistesys.ps1 lc631-dgcl-run --prompt-stdin
```

Automation sets `ProcessStartInfo.StandardInputEncoding` to UTF-8, writes the original seed through stdin, and specifies EOF and an overall deadline. Do not record seeds in process arguments or Git documents. Verify returned version `6.3.2-alpha.2`. This DGCL route does not establish old-v6 67-stage full-compute parity.

Separate source release, plugin registration/cache installation, enabled settings, host pickup, and post-restart observation. Manifests/directories alone do not establish host activation. Use official marketplace/install paths; never copy caches/models/keys into the source repository. This public repository does not automatically apply a personal default choice to other people's settings.
