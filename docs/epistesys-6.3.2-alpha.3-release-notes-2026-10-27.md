# Epistesys 6.3.2-alpha.3 — release candidate notes

## 日本語

状態：**候補。昇格gate確認中**。この文書の作成だけではversion更新やsource release readinessを意味しません。元の実装はLabyrinth-Codex v6.3.1由来で、今回の変更は既存経路の境界・接続・回復の修正です。Epistesys-7やモデルweightの更新ではありません。

### 実装上の変更

- `controlled-ja-en-instruction-earley.v3`：相対／原文byte offsetを分離し、日本語反復参照・3節以上の列挙を修正。action、scope、参照、条件式、constraint graphへ共有の後処理予算を通し、超過をProgram IR／completionまで明示保持します。
- Authority：元receiptのsource/parent/issuerを保持。host seedの公開field変更、期限切れ、別source/scope/span、code／quote由来の権限発行、別seedのstage混合を拒否します。現在のreceipt期間は`[issued, expires)`です。秘密鍵のDebugはredactします。
- Coding：genericなlibrary test成功をProductionConnectionと扱う経路を閉鎖。登録driver・pinしたoracle・実Cargo metadata・exact named runtime testに接続し、`std`のaliasによるassertion差替えも拒否します。範囲は協調的な登録build profileの直接API観測に限定します。
- 実行・修復：Windows Job／Unix process groupの共通supervisor、有界pipe、所有資源のcleanup状態、anchored filesystem、private backup、作用を保持するCLI errorへ接続します。期限切れheadは履歴としてのみ認証し、新しい権限・独立binding・実検証後に同じheadを明示renewalします。
- Media：元入力のmutable pathではなくbyte-authenticated private snapshotを処理し、snapshotとdecoderでtemp予算を共有します。frame/PCM/outputをstreaming制限し、UTF-8上限、人物／顔kind、報告methodと観測kind、manifest v3を一致させます。実decoder／OpenCV／CPU ASR候補は独立精度検証とは区別します。
- 数値・graph：CSR、四値mask、中間算術、QUBO対角項、empty-edge GPU処理を修正。token ordinalとsyntax nodeを同一視せず、明示的なsource/span付きbindingを下流でも再検査します。
- 配布・CI：固定commitまたは明示承認inventoryのみをpackage化し、未追跡key/env等を黙って含めません。Rust-only変更を捕捉するWindows/Linux CIと、実際に失敗する負例を追加しました。pin対象のLF byteをcheckoutで変えないよう固定します。
- 開発・テストprofileではSHA-256 backendだけを最適化し、実行fileの暗号学的再検査のコストを抑えます。検査の省略、digestのcache化、期限延長は行いません。標準ベクトルと変更inputの負例を確認し、実修復・接続6件を直列で再観測しました。一般latency保証や性能benchmarkではありません。

### 互換性・主張の境界

既存JSON fieldは保持し、新しいconnection spec、token binding、memory／effect metadata等はadditiveです。ただしstrictな旧consumerは新enum／renewal headを拒否する場合があります。古いcontext-free authority reportは診断用であり、現在のpermissionとして扱いません。新しいMAC bindingにはfreshな材料が必要です。

GPUのnative completionとsummary/mask/finalist/decision parityは、全vector等値、occupancy、speedupの証明ではありません。CPU ASRの非空出力は文字起こし精度やlicense監査の代わりではありません。source、package、plugin install、host pickup、研究評価を分離します。独立gold・一般性能・実host強制・未知欠陥ゼロ・形式証明は未成立です。

最終的なgate状態は[修正台帳](../validation/epr-remediation-ledger.v1.json)と[境界契約](epr-boundary-contracts.md)を参照してください。候補が未検証の間は、成功を推論しないでください。

## English

Status: **candidate; promotion gates in progress**. Creating these notes does not change the version or establish source release readiness. The inherited implementation derives from Labyrinth-Codex v6.3.1; this update fixes boundaries, connections, and recovery in existing paths. It is not Epistesys-7 or model-weight training.

### Implementation changes

- `controlled-ja-en-instruction-earley.v3`: separate relative/source byte offsets and fix repeated Japanese references/multi-clause coordination. Share postprocessing budgets across actions, scopes, references, condition expressions, and constraints, retaining exhaustion through Program IR/completion.
- Authority: retain receipt source/parent/issuer; reject mutable host seeds, expiry, wrong source/scope/span, code/quote Grants, and cross-seed stages. Current validity is `[issued, expires)`; key Debug is redacted.
- Coding: generic library success no longer becomes ProductionConnection. Bind registered drivers, pinned oracles, observed Cargo metadata, and exact runtime tests; reject `std` alias assertion replacement. Coverage remains direct API observation under a cooperative registered build profile.
- Execution/repair: connect common Windows Job/Unix process-group supervision, bounded pipes, cleanup observations, anchored filesystems, private backups, and effect-preserving CLI errors. Expired heads authenticate history only; renew the same head explicitly after fresh authority, independent binding, and actual validation.
- Media: process byte-authenticated private snapshots instead of mutable original paths; share temp quotas between snapshots/decoders. Stream-bound frames/PCM/output and align UTF-8 limits, person/face kinds, reported methods, and manifest v3. Actual decoder/OpenCV/CPU-ASR candidates are separate from accuracy evaluation.
- Numerics/graphs: correct CSR/masks/intermediate arithmetic/diagonal QUBO/empty GPU edges. Replace token-ordinal aliasing with explicit source/span bindings checked downstream.
- Distribution/CI: package fixed commits or approved inventories only, never silently include untracked key/env files. Add Windows/Linux Rust-change CI and an actual failing negative control; stabilize physical LF bytes for pinned inputs.
- Dev/test profiles optimize only the SHA-256 backend to reduce repeated cryptographic executable-check cost. Do not skip checks, cache digests, or extend deadlines. Standard vectors/input-mutation negatives and six serial real repair/connection cases were observed; this is neither a general latency guarantee nor a performance benchmark.

### Compatibility and claim boundaries

Retain existing JSON fields; connection specs, token bindings, and memory/effect metadata are additive. Strict legacy consumers may reject new enums/renewal heads. Old context-free authority reports are diagnostic, not current permission. New authenticated bindings require fresh material.

GPU native completion and summary/mask/finalist/decision parity do not prove full-vector equality, occupancy, or speedup. Nonempty CPU ASR output is not accuracy or license review. Separate source, packages, plugin install, host pickup, and research evaluation. Independent gold, general performance, absolute host enforcement, zero unknown defects, and formal proof are not established.

See the [remediation ledger](../validation/epr-remediation-ledger.v1.json) and [boundary contracts](epr-boundary-contracts.md) for final gate state. Do not infer success while the candidate remains unverified.
