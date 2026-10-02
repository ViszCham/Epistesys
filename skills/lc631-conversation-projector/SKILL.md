---
name: lc631-conversation-projector
description: Default Epistesys 6.3.2-alpha.2 facade for conversation, explanation, planning, writing, and design unless the user explicitly selects another reasoning line. Coding uses the coding facade.
---

# Epistesys Conversation Projector

## 日本語

明示的な別line指定がなければ、全sessionの非coding turnはこのalpha.2 facadeを既定として使用します。coding facadeと二重起動しません。返答前にplugin rootから`scripts/run-epistesys.ps1 lc631-dgcl-run --prompt-stdin`へ元user seedをUTF-8 stdinで渡し、version `6.3.2-alpha.2`とProgram IR/TL/未解決状態を確認します。controlled grammar外の通常会話はContext/Unsupported/Unresolvedを保ち、命令のない挨拶等へ不要な実行obligationを発行しません。source候補のHoldと、通常説明の禁止を混同しません。故障時は診断を示し、黙って旧engineへfallbackしません。これはbounded DGCL source経路であり、旧67-stage経路とのparityや全host強制を主張しません。

通常promptにもlc631-tldg-doctorを実行し、自然言語とprogram断片を共通syntax substrateへ投影します。これはEpistesys 6.3.2-alpha.2が継承するv6.3.1経路です。geometry proposalはadvisoryで、Program IR/outputが未bindingならClarify/hold residualを保持します。

`scripts/run-lc631.ps1 lc631-tl-doctor --prompt <seed>`と`lc631-world-doctor --prompt <seed>`を実行します。

- seedは暫定入力であり仕様ではありません。
- ProjectionDefectGraph v3 stateをcanonicalとし、model scoreはadvisoryです。
- world surfaceは256 distinct temporary posit×8 projectionを2,048個のmaterialized evaluationとして保持します。
- 低defect、test成功、GPU実行はtruth、proof、permission、host bindingではありません。
- conversation promptから外部media取得を実行しません。
- Epistesysの経路を自動的に別lineと二重起動せず、ユーザーが指定したlineだけを使います。

---

## English

Unless explicitly overridden, use this alpha.2 facade by default for non-coding turns in every session. Do not double-run the coding facade. Before responding, pass the original user seed as UTF-8 stdin to `scripts/run-epistesys.ps1 lc631-dgcl-run --prompt-stdin` from the plugin root; check version `6.3.2-alpha.2` and Program IR/TL/unresolved states. Ordinary conversation outside controlled grammar retains Context/Unsupported/Unresolved; do not issue unnecessary execution obligations for instruction-free greetings. Do not confuse a source-candidate Hold with a ban on ordinary explanation. Show failures without silently falling back to the old engine. This is a bounded DGCL source path, not parity with the old 67-stage route or host-wide enforcement.

For ordinary prompts, also run lc631-tldg-doctor and project natural-language and program fragments into the shared syntax substrate. This is the inherited v6.3.1 route under Epistesys 6.3.2-alpha.2. Geometry proposals remain advisory, and an unbound Program IR/output keeps its Clarify/hold residual.

Run `scripts/run-lc631.ps1 lc631-tl-doctor --prompt <seed>` and `lc631-world-doctor --prompt <seed>`.

- The seed is provisional, not a specification.
- ProjectionDefectGraph v3 state is canonical; model scores are advisory.
- The world surface materializes 256 distinct temporary posits by eight projections as exactly 2,048 evaluations.
- Low defect, test success, or GPU execution is not truth, proof, permission, or host binding.
- Do not invoke external media acquisition from a conversation prompt.
- Do not automatically double-run Epistesys with another line; use only the line selected by the user.
