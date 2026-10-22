# Codex明示選択時のルーティング / Codex routing after explicit selection

## 日本語

この文書は、利用者がEpistesysを明示選択し、host側で利用を有効にした場合の手順である。非codingには`epistesys:lc631-conversation-projector`、coding/review/testには`epistesys:lc631-coding-projector`を一つだけ使用する。version指定がなければ対象は6.3.2-alpha.2とする。選択範囲は利用者が指定したtask/sessionに従い、repositoryの閲覧・編集やEpistesysへの言及だけでは利用を選択しない。古いskill内の「既定」表現より、利用者の明示指示を優先する。

既存Labyrinth lineも明示指定用に保持し、通常の二重実行や故障時の無断fallbackは行わない。要求されたlineが利用不能なら、その状態を報告する。これはhost設定・利用者指示で選択するルーティングであり、host interceptionではない。ファイル名は既存linkとの互換性のため維持している。

alpha.2の主経路は`lc631-dgcl-run`で、同sourceのDeepGrammar→Program IR→TL→plan→candidateを保持する。外部modelなしとconfigured Stanzaを区別し、unknown/Unsupported/未検証をtruthや権限へ昇格しない。通常会話のsource-candidate Holdを返答禁止と同一視せず、明示的に許可された作業への権限判断は元user instructionから別に行う。危険作用、実装完了、host送信はそれぞれの検証・権限を必要とする。

PowerShellでstdinを使う場合、scriptへの直接pipelineではなく、stdinを受けるnative `pwsh -File` processへ渡す。

```powershell
'Please preserve source evidence.' | pwsh -NoProfile -File ./scripts/run-epistesys.ps1 lc631-dgcl-run --prompt-stdin
```

automationでは`ProcessStartInfo.StandardInputEncoding`をUTF-8にし、元seedをstdinへ書き、EOFと全体deadlineを明示する。seedをprocess argumentやGit文書へ記録しない。返却version `6.3.2-alpha.2`を確認する。旧v6の67-stage full-compute parityは、このDGCL routeから主張しない。

source release、plugin登録/cache install、enabled設定、host pickup、再起動後の実観測を分離する。manifestやdirectoryだけではhost activationの証拠にならない。pluginの追加はhost公式marketplace/install経路を使い、cache/model/keyをsource repositoryへ持ち込まない。個人の既定選択をこの公開repositoryから他人の設定へ自動適用しない。

## English

This document describes use after the user explicitly selects Epistesys and enables it on the host. Select exactly one facade: `epistesys:lc631-conversation-projector` for non-coding or `epistesys:lc631-coding-projector` for coding/review/tests. Use 6.3.2-alpha.2 when no version is specified. Respect the task/session scope stated by the user; browsing/editing the repository or mentioning Epistesys alone does not select its use. Explicit user instructions take precedence over older default-use wording in skills.

Retain existing Labyrinth lines for explicit selection, without routine double execution or silent failure fallback. Report an unavailable requested line. This routing is selected through host settings/user instructions, not host interception. The existing filename is retained for link compatibility.

Alpha.2's primary route is `lc631-dgcl-run`, retaining same-source DeepGrammar→Program IR→TL→plan→candidate. Distinguish no-external-model and configured Stanza profiles; unknown/Unsupported/unverified states never become truth or authority. A conversational source-candidate Hold is not a ban on responding; authorization for explicitly requested work is separately classified from the original user instruction. Dangerous effects, implementation completion, and host sending require their own validation/authority.

For PowerShell stdin, pipe to a native `pwsh -File` process that receives stdin, not directly to the script's PowerShell parameter binding.

```powershell
'Please preserve source evidence.' | pwsh -NoProfile -File ./scripts/run-epistesys.ps1 lc631-dgcl-run --prompt-stdin
```

Automation sets `ProcessStartInfo.StandardInputEncoding` to UTF-8, writes the original seed through stdin, and specifies EOF and an overall deadline. Do not record seeds in process arguments or Git documents. Verify returned version `6.3.2-alpha.2`. This DGCL route does not establish old-v6 67-stage full-compute parity.

Separate source release, plugin registration/cache installation, enabled settings, host pickup, and post-restart observation. Manifests/directories alone do not establish host activation. Use official marketplace/install paths; never copy caches/models/keys into the source repository. This public repository does not automatically apply a personal default choice to other people's settings.
