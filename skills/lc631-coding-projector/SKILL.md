---
name: lc631-coding-projector
description: Default Epistesys 6.3.2-alpha.2 facade for coding, debugging, tests, patches, and review unless the user explicitly selects another reasoning line.
---

# Epistesys Coding Projector

## 日本語

coding seedではlc631-tldg-doctorも実行し、lossless source、UnifiedSyntaxHypergraph、ProjectionDefectGraph v3、geometry非Authority、output binding residualを確認します。これはEpistesys 6.3.2-alpha.2に継承されたv6.3.1経路です。GPU検証を明示された場合だけlc631-tldg-gpu-doctorへexecuteを渡します。`lc631-tldg-release-gate --execute`はlegacy boolean経路のfail-closed検査であり、release成功経路ではありません。

明示的な別line指定がなければEpistesys 6.3.2-alpha.2を既定として使用します。別のLabyrinth lineと自動的に二重実行しません。

判断前・重要な差分後・最終回答前に、plugin rootから`scripts/run-epistesys.ps1 lc631-dgcl-run --prompt-stdin`へ元のuser seedをUTF-8 stdinで渡します。versionが`6.3.2-alpha.2`であること、同artifactのProgram IR/TL/plan/candidateと未解決状態を確認します。これはalpha.2 DGCLのbounded standalone経路であり、旧v6の67-stage DecisionEnvelopeとのsemantic parityや実hostのabsolute enforcementを名乗りません。外部modelなしのprofileをconfigured Stanza実観測へ昇格せず、故障時は診断を示して黙って別engineへfallbackしません。`implementation_complete_candidate=false`やhost未観測を、userが許可した作業への禁止と同一視しません。codeの完了判定は実検証とcurrent evidenceを必要とします。

1. plugin rootから`scripts/run-lc631.ps1 lc631-doctor --repo <repository-root>`を実行します。
2. TranslationLoss判断には`lc631-tl-doctor --prompt <seed>`を使い、scalar値ではなくProjectionWitness defectを保持します。
3. GPU実行が許可された場合、`lc631-gpu-doctor --execute`を使います。負荷・VRAM観測が必要な場合だけ`lc631-gpu-stress`を使い、利用率とoccupancyを同一視しません。
4. local media解析は、権利assertに加えてauthenticated source receipt、receipt root、backend ID、model license receiptがあるlocal fileだけを`lc631-media-execute`へ渡します。YouTube URL取得は行いません。
5. durable replayは、exact seed/output/callback、seed/output attestation JSON、owned ledger rootがある場合だけ`lc631-host-bind`で実行します。legacy非空文字列receiptは拒否します。
6. Epistesysの結果は、編集、test、commit、push、install、restart、network acquisition、永続化の権限を生成しません。
7. 継承元の[ARC631実装検証記録](../../docs/v6.3.1-arc631-authenticity-and-closure-implementation-record-2026-08-26.md)は履歴です。DGCL作業には[現対応範囲](../../docs/dgcl-operating-profile-and-closure.md)を読み、source closureとresearch/host pendingを分離します。修復の再開はcheckpointから完了を復元せず、fresh権限・snapshot・検証を要求します。

Epistesysのclone identityは、継承元の他のlineと独立して扱います。Epistesysの有効化は別lineの置換を意味しません。

---

## English

For coding seeds, also run lc631-tldg-doctor and inspect lossless source, UnifiedSyntaxHypergraph, ProjectionDefectGraph v3, geometry non-Authority, and output-binding residuals. This is the inherited v6.3.1 route under Epistesys 6.3.2-alpha.2. Pass execute to lc631-tldg-gpu-doctor only when GPU validation is explicitly requested. `lc631-tldg-release-gate --execute` tests fail-closed behavior of the legacy boolean route; it is not a release-success route.

Use Epistesys 6.3.2-alpha.2 by default unless the user explicitly selects another line. Do not automatically double-run another Labyrinth line.

Before decisions, after meaningful diffs, and before final responses, pass the original user seed as UTF-8 stdin to `scripts/run-epistesys.ps1 lc631-dgcl-run --prompt-stdin` from the plugin root. Check version `6.3.2-alpha.2`, same-artifact Program IR/TL/plan/candidate, and unresolved states. This is alpha.2's bounded standalone DGCL path, not semantic parity with the old v6 67-stage DecisionEnvelope or absolute real-host enforcement. Do not promote a no-external-model profile into observed configured Stanza; show failures and never silently fall back to another engine. A false implementation-complete candidate or unobserved host is not a denial of work explicitly authorized by the user. Actual code completion still requires validation and current evidence.

1. Run `scripts/run-lc631.ps1 lc631-doctor --repo <repository-root>` from the plugin root.
2. For TranslationLoss decisions, run `lc631-tl-doctor --prompt <seed>` and preserve ProjectionWitness defects rather than inventing a scalar loss.
3. For GPU evidence, run `lc631-gpu-doctor --execute` only when local GPU execution is authorized. Use `lc631-gpu-stress` only for authorized load/VRAM observation, and never equate utilization with occupancy.
4. Pass a local file to `lc631-media-execute` only with a rights assertion, authenticated source receipt, receipt root, backend ID, and model-license receipt. Never acquire YouTube media from a URL.
5. Run `lc631-host-bind` only with exact seed/output/callback bytes, a seed/output-attestation JSON file, and an explicitly owned persistent ledger root. Legacy nonempty receipt strings are rejected.
6. Epistesys output grants no edit, test, commit, push, install, restart, network-acquisition, or persistence authority.
7. The inherited [ARC631 implementation record](../../docs/v6.3.1-arc631-authenticity-and-closure-implementation-record-2026-08-26.md) is historical. For DGCL work read the [current operating scope](../../docs/dgcl-operating-profile-and-closure.md), separating source closure from pending research/hosts. Repair resume never restores completion from checkpoints; require fresh authority, snapshots, and validation.

Other installed plugins remain independent. Enabling Epistesys does not replace them.
