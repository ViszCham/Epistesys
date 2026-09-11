---
name: lc631-conversation-projector
description: Inspect ordinary prompts through the Epistesys 6.3.2-alpha.1 inherited witness and world-budget surfaces when the user explicitly asks for Epistesys.
---

# Epistesys Conversation Projector

## 日本語

通常promptにもlc631-tldg-doctorを実行し、自然言語とprogram断片を共通syntax substrateへ投影します。これはEpistesys 6.3.2-alpha.1が継承するv6.3.1経路です。geometry proposalはadvisoryで、Program IR/outputが未bindingならClarify/hold residualを保持します。

`scripts/run-lc631.ps1 lc631-tl-doctor --prompt <seed>`と`lc631-world-doctor --prompt <seed>`を実行します。

- seedは暫定入力であり仕様ではありません。
- ProjectionDefectGraph v3 stateをcanonicalとし、model scoreはadvisoryです。
- world surfaceは256 distinct temporary posit×8 projectionを2,048個のmaterialized evaluationとして保持します。
- 低defect、test成功、GPU実行はtruth、proof、permission、host bindingではありません。
- conversation promptから外部media取得を実行しません。
- Epistesysの経路を自動的に別lineと二重起動せず、ユーザーが指定したlineだけを使います。

---

## English

For ordinary prompts, also run lc631-tldg-doctor and project natural-language and program fragments into the shared syntax substrate. This is the inherited v6.3.1 route under Epistesys 6.3.2-alpha.1. Geometry proposals remain advisory, and an unbound Program IR/output keeps its Clarify/hold residual.

Run `scripts/run-lc631.ps1 lc631-tl-doctor --prompt <seed>` and `lc631-world-doctor --prompt <seed>`.

- The seed is provisional, not a specification.
- ProjectionDefectGraph v3 state is canonical; model scores are advisory.
- The world surface materializes 256 distinct temporary posits by eight projections as exactly 2,048 evaluations.
- Low defect, test success, or GPU execution is not truth, proof, permission, or host binding.
- Do not invoke external media acquisition from a conversation prompt.
- Do not automatically double-run Epistesys with another line; use only the line selected by the user.
