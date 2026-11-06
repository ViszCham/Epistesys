---
name: lc631-conversation-projector
description: Use Epistesys 7.1.0-beta.1 for conversation, explanation, planning, writing and design when selected; coding uses the coding facade.
---

# Epistesys Conversation Projector

## 日本語

このpluginが選択されている非coding turnで使用します。coding facadeや別lineと二重起動しません。[plugin接続契約](../../docs/plugin-host-integration.md)に従い、元のuser seedを `scripts/run-epistesys.ps1 lc631-dgcl-run --prompt-stdin` へUTF-8 stdinで一度渡します。

返却version `7.1.0-beta.1`、元sourceの保存、Program IR・EpiMapLoss・plan・candidateの対応と未解決状態を確認します。同一turn内の同じ入力・binary・snapshotでは診断を再利用できます。構文や写像の詳細が必要な場合だけTLDG／TL doctorを使い、通常会話で複数のworld／GPU診断を一律実行しません。

controlled grammar外の会話はContext・Unsupported・Unresolvedを保持します。挨拶等から実行要求を作らず、候補のHoldを返答禁止と同一視しません。幾何的提案やmodel scoreは補助情報です。診断はtruth・proof・操作権限・host bindingを作りません。外部取得や永続化を会話から自動実行せず、故障時に旧engineへfallbackしません。

## English

Use this facade for non-coding turns when the plugin is selected; do not double-run the coding facade or another line. Follow the [plugin integration contract](../../docs/plugin-host-integration.md) and pass the original user seed once as UTF-8 stdin to `scripts/run-epistesys.ps1 lc631-dgcl-run --prompt-stdin`.

Check version `7.1.0-beta.1`, original-source preservation, Program IR/EpiMapLoss/plan/candidate bindings and unresolved states. Reuse the diagnostic within the turn only for unchanged input, binary and snapshot. Use TLDG/TL doctors when detailed syntax or mapping diagnostics are needed, not multiple world/GPU diagnostics for every conversation.

Conversation outside controlled grammar retains Context, Unsupported and Unresolved states. Do not invent execution obligations from greetings or treat a candidate Hold as a ban on responding. Geometry proposals and model scores are advisory. Diagnostics create no truth, proof, permission or host binding. Do not automatically acquire external data or persist state from conversation, and do not fall back to an old engine on failure.
