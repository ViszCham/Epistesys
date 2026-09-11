# LC631 Closure and Receipt Contract

## 日本語

`Declared -> Typed -> Reachable -> Executed -> Consumed -> Contained -> HostBound`は相互に異なる状態です。後段へ進むには、同一component、principal、subject revision、scope、nonce、payload digestに結び付く認証済みwitnessが必要です。schemaの存在、test成功、低いloss、backend設定、public enum／boolだけでは昇格しません。wire receiptはdeserialize後に常にUntrustedへ戻り、exact policyとreplay guardを通過したRust typestateだけがstrong gateへ入ります。

### Accelerator receipt

`NumericBackend`はsealedで、`AcceleratorBroker`はdeclared backend kindとraw observation kindの一致を検査します。GPU backendはnumeric observationだけを返し、Authority、semantic truth、OutputCommitを生成しません。外部Evidenceへ使う場合は別のauthenticated accelerator receiptが必要です。

ARC631経路は256 world×8 projectionを2,048個のrevision-bound evaluationへ実体化します。8 laneはProjection、Dominance/Conflict、Jacobi、QUBO、World Distance、Conflict Count、Incomparability Count、Paretoです。relationは全pairではなく各rowのbounded sparse planを使います。device側chunk reductionと最大16 finalistのindex／value／cutoff／tieにより、実GPU doctorのhost readbackは98値でした。CPU/GPU parityはcontext、semantic binding、row identity、mask、summary、finalist、cutoffを比較し、summary digest単体では決めません。

`device_timing`、`host_inflight_overlap_observed`、`shader_occupancy`、`device_loss_callback_registered`は別fieldです。WGPU callbackは実deviceへ登録され、loss/error signalはarena quarantineとfresh CPU fallback要求へ流れます。`nvidia-smi`のGPU利用率はSM activity sampleであり、shader occupancy counterではありません。

### Media receipt

`lc631-media-backend.v1`は、authenticated source snapshot、Python/script/FFmpeg hash、model-cache manifest、license、method ID、config、resource budget、process outputを束縛します。rights boolやexisting pathだけではMediaSourceAuthorityになりません。任意script JSONが列挙したfamilyは`ExecutedCandidate`に留まり、独立method receiptなしでは`ValidatedObservation`になりません。Haar face regionはidentityではなく、PCM二クラスタはdiarizationではなく、co-presenceはsemantic alignmentではなく、ASR textは誤り得るcandidateです。cache miss時downloadは行いません。

### Host and replay receipt

`lc631-host-contract.v2`はartifact、turn、candidate digest、exact output bytes digest、callback payload、receipt rootを結びます。candidate digestはoutput textから再計算します。plugin既定`Stop` hookはPLUGIN_DATAにlocal receipt rootを生成・保持し、`lc631-host-replay.v2` sinkはattestationを再検証してからappend-only JSONL hash chainへ同期書込みします。ledgerにはoutput本文を保存せずdigestだけを保存します。v1 chainは読取り互換ですがv2 strong gateへは入りません。

2026-08-25のv1 host probeは`exact_binding=true`等を観測した履歴です。ARC631 v2のfresh host pickup、remote client配送、user authority、semantic truthはその履歴から昇格しません。非managed hookはinstall／変更後に公式`/hooks` reviewとtrustを必要とします。

### Paired regression

固定64 seed corpusはv6.3.0 full DecisionEnvelopeとv6.3.1 witness routeへ同じseedを渡し、version、full-compute、semantic-parity flag、stage floor、mode、host authority boundary、witness chain、fixed-threshold不使用を検査します。これは固定corpus上の構造的回帰検査であり、普遍的semantic equivalence、回答品質、真理を証明しません。

---

## English

`Declared -> Typed -> Reachable -> Executed -> Consumed -> Contained -> HostBound` are distinct states. Advancement requires an authenticated witness bound to the same component, principal, subject revision, scope, nonce, and payload digest. Schema presence, passing tests, low loss, backend configuration, or a public enum/boolean cannot promote a state. A wire receipt always returns to Untrusted after deserialization; only a Rust typestate produced by exact-policy verification and a replay guard enters a strong gate.

### Accelerator receipt

`NumericBackend` is sealed, and `AcceleratorBroker` checks that the declared backend kind matches the raw observation kind. A GPU backend returns numeric observations only and cannot create Authority, semantic truth, or OutputCommit. External Evidence use requires a separate authenticated accelerator receipt.

The ARC631 route materializes 256 worlds by eight projections as 2,048 revision-bound evaluations. Its eight lanes cover Projection, Dominance/Conflict, Jacobi, QUBO, World Distance, Conflict Count, Incomparability Count, and Pareto. Relations use a bounded sparse plan per row instead of every pair. Device-side chunk reduction plus at most 16 finalist indices, values, cutoff, and tie data produced 98 host-readback values in the real-GPU doctor. CPU/GPU parity compares context, semantic binding, row identity, masks, summaries, finalists, and cutoff; a summary digest alone is insufficient.

`device_timing`, `host_inflight_overlap_observed`, `shader_occupancy`, and `device_loss_callback_registered` are separate fields. The WGPU callback is registered on the real device, and a loss/error signal flows to arena quarantine plus a fresh-CPU-fallback requirement. `nvidia-smi` GPU utilization is an SM-activity sample, not a shader occupancy counter.

### Media receipt

`lc631-media-backend.v1` binds an authenticated source snapshot, Python/script/FFmpeg hashes, model-cache manifest, license, method IDs, configuration, resource budgets, and process output. A rights boolean and existing path do not create MediaSourceAuthority. Families listed by arbitrary script JSON remain `ExecutedCandidate`; they cannot become `ValidatedObservation` without independent method receipts. A Haar face region is not identity, two-cluster PCM analysis is not diarization, co-presence is not semantic alignment, and ASR text is a fallible candidate. Cache misses never trigger downloads.

### Host and replay receipt

`lc631-host-contract.v2` binds the artifact, turn, candidate digest, exact output-byte digest, callback payload, and receipt root. The candidate digest is recomputed from the output text. The default `Stop` hook generates and retains a local receipt root under PLUGIN_DATA, and the `lc631-host-replay.v2` sink re-verifies the attestation before synchronously appending to the JSONL hash chain. The ledger stores output digests, not output prose. A v1 chain remains readable but cannot enter the v2 strong gate.

The 2026-08-25 v1 host probe is historical evidence that observed `exact_binding=true` and related fields. Fresh ARC631 v2 host pickup, remote-client delivery, user authority, and semantic truth do not inherit from that history. A non-managed hook requires official `/hooks` review and trust after installation or hook changes.

### Paired regression

The fixed 64-seed corpus sends the same seeds through the v6.3.0 full DecisionEnvelope and the v6.3.1 witness route, checking version, full compute, the semantic-parity flag, stage floor, mode, host authority boundary, witness chain, and absence of a fixed threshold. This is a structural regression check over a fixed corpus, not proof of universal semantic equivalence, answer quality, or truth.
