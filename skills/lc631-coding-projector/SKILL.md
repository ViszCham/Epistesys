---
name: lc631-coding-projector
description: Run the Epistesys 6.3.2-alpha.1 inherited v6.3.1 contracts for coding audits and preserve explicit evidence boundaries.
---

# Epistesys Coding Projector

## 日本語

coding seedではlc631-tldg-doctorも実行し、lossless source、UnifiedSyntaxHypergraph、ProjectionDefectGraph v3、geometry非Authority、output binding residualを確認します。これはEpistesys 6.3.2-alpha.1に継承されたv6.3.1経路です。GPU検証を明示された場合だけlc631-tldg-gpu-doctorへexecuteを渡します。`lc631-tldg-release-gate --execute`はlegacy boolean経路のfail-closed検査であり、release成功経路ではありません。

ユーザーがEpistesys 6.3.2-alpha.1を明示的に指定した場合だけ使用します。別のLabyrinth lineと自動的に二重実行しません。

1. plugin rootから`scripts/run-lc631.ps1 lc631-doctor --repo <repository-root>`を実行します。
2. TranslationLoss判断には`lc631-tl-doctor --prompt <seed>`を使い、scalar値ではなくProjectionWitness defectを保持します。
3. GPU実行が許可された場合、`lc631-gpu-doctor --execute`を使います。負荷・VRAM観測が必要な場合だけ`lc631-gpu-stress`を使い、利用率とoccupancyを同一視しません。
4. local media解析は、権利assertに加えてauthenticated source receipt、receipt root、backend ID、model license receiptがあるlocal fileだけを`lc631-media-execute`へ渡します。YouTube URL取得は行いません。
5. durable replayは、exact seed/output/callback、seed/output attestation JSON、owned ledger rootがある場合だけ`lc631-host-bind`で実行します。legacy非空文字列receiptは拒否します。
6. Epistesysの結果は、編集、test、commit、push、install、restart、network acquisition、永続化の権限を生成しません。
7. 実装・監査時は[ARC631実装検証記録](../../docs/v6.3.1-arc631-authenticity-and-closure-implementation-record-2026-08-26.md)を現行revision境界として読みます。

Epistesysのclone identityは、継承元の他のlineと独立して扱います。Epistesysの有効化は別lineの置換を意味しません。

---

## English

For coding seeds, also run lc631-tldg-doctor and inspect lossless source, UnifiedSyntaxHypergraph, ProjectionDefectGraph v3, geometry non-Authority, and output-binding residuals. This is the inherited v6.3.1 route under Epistesys 6.3.2-alpha.1. Pass execute to lc631-tldg-gpu-doctor only when GPU validation is explicitly requested. `lc631-tldg-release-gate --execute` tests fail-closed behavior of the legacy boolean route; it is not a release-success route.

Use this skill only when the user explicitly requests Epistesys 6.3.2-alpha.1.

1. Run `scripts/run-lc631.ps1 lc631-doctor --repo <repository-root>` from the plugin root.
2. For TranslationLoss decisions, run `lc631-tl-doctor --prompt <seed>` and preserve ProjectionWitness defects rather than inventing a scalar loss.
3. For GPU evidence, run `lc631-gpu-doctor --execute` only when local GPU execution is authorized. Use `lc631-gpu-stress` only for authorized load/VRAM observation, and never equate utilization with occupancy.
4. Pass a local file to `lc631-media-execute` only with a rights assertion, authenticated source receipt, receipt root, backend ID, and model-license receipt. Never acquire YouTube media from a URL.
5. Run `lc631-host-bind` only with exact seed/output/callback bytes, a seed/output-attestation JSON file, and an explicitly owned persistent ledger root. Legacy nonempty receipt strings are rejected.
6. Epistesys output grants no edit, test, commit, push, install, restart, network-acquisition, or persistence authority.
7. Read the [ARC631 implementation record](../../docs/v6.3.1-arc631-authenticity-and-closure-implementation-record-2026-08-26.md) as the current revision boundary for v6.3.1 implementation or audit work.

Other installed plugins remain independent. Enabling Epistesys does not replace them.
