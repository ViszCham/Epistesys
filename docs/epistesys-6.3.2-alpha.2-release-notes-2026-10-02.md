# Epistesys 6.3.2-alpha.2 — Release notes

## 日本語

2026-10-02 JST。alpha.2はv6.3.1由来のbaselineにDGCLの実装と接続を追加した実験的source releaseです。昇格前のA2-G00〜13が通過してからidentityを更新し、alpha.2 identityでも全command gate、実日英worker、fresh source packageからのrelease build/production CLIを再検証しました。判定は`SourceReleaseReady`、完成範囲は`ImplementationClosedForDeclaredProfile`です。実Codex host activation、独立gold、一般性能の実証とは別です。

### 実際に増えたもの

- `controlled-ja-en-instruction-earley.v2`：条件右辺・明示scope引数の欠落をUnresolvedへ保持し、mixed boolean precedenceを勝手に選択しない。11 grammar familyの正例・意味/不正文法反例とorigin境界試験を追加。
- 同artifactのgrammar→Program IR→TL→completion→candidate：構築seal、要求別の5 evidence kind、public report成功flagの遮断、全receipt/current lifecycleの再検査、exact ledger realization。
- 登録Cargoの別々のcheck/runtime/connection invocation/acceptance観測、外部Authorityと独立intent binding。validation planは修復前に検査し、未対応backend optionを黙って無視しない。
- 実ファイル修復、非上書きbackup、更新後snapshot、最大4回/NoProgress/cycle、署名head付きjournal。`lc631-dgcl-package-resume`は中断後も再書込みせずfresh権限・intent・Cargo証拠を要求する。checkpointだけで旧completionを復元しない。
- 実stateを更新する離散↔連続蒸留と下流identity接続。materializationはexact source relation evidenceを保持し、同pairの別制約へ取り違えない。AST digest改変を拒否し、距離や構造からtruth/Grantを作らない。
- 明示consumer view v1/v2。strict限定v1は未知field、v2、trueのcompletion/send/commitを拒否し、v2はreported stateを描画するだけ。public JSON fieldは削除・renameせず、新field/schemaをadditiveにした。
- package versionとparser/projection/closure implementationへ束縛したvalidator revision、evidence失効、schema/roundtrip/permission mutation、16断線、実call/head/callback cut→restore試験。

### 検証と読み方

[最終検証記録](../validation/epia2-alpha2-release-verification-2026-10-02.md)、[昇格前gate](../validation/epia2-prepromotion-functional-pass-2026-10-02.md)、[対応範囲](dgcl-operating-profile-and-closure.md)、[wire互換性](dgcl-wire-compatibility.md)を参照してください。source-only packageはmodel weights、keys、cache、個人host path、配布未監査binaryを含みません。fresh buildは既存toolchain/registry cacheを使用し、未構築PCの自動provisionやplugin installの証拠ではありません。

ResearchEvaluationは`PendingNoCorpus`、実hostは`PendingHostObservation`です。任意自然言語の完全理解、hallucination防止、長期session性能、OS sandbox、即時host revocation、任意build world、形式証明、「全弱点ゼロ」は主張しません。継承元の履歴的Held/global promotionと、このstandalone source profileの成功を混同しません。Epistesys-7は追加していません。

## English

2026-10-02 JST. Alpha.2 is an experimental source release adding DGCL implementation and connectivity to the v6.3.1-derived baseline. Identity changed only after prepromotion A2-G00..13 passed; all command gates, actual JA/EN workers, and fresh-source-package release build/production CLI were revalidated under alpha.2. Decision: `SourceReleaseReady`; completion scope: `ImplementationClosedForDeclaredProfile`. This is separate from real Codex activation, independent gold, and general-performance demonstration.

### Actual additions

- `controlled-ja-en-instruction-earley.v2`: retain missing condition RHS/explicit scope operands as Unresolved; do not select mixed boolean precedence arbitrarily. Add positives, malformed/semantic negatives, and origin-boundary tests for 11 grammar families.
- Same-artifact grammar→Program IR→TL→completion→candidate: construction seals, five per-requirement evidence kinds, rejection of public-report success flags, rechecking every receipt/current lifecycle, and exact ledger realization.
- Separate registered-Cargo check/runtime/connection-invocation/acceptance observations, external Authority, and independent intent binding. Preflight validation plans before repair; never silently ignore unsupported backend options.
- Actual file repair, non-overwriting backups, changed snapshots, four-attempt/NoProgress/cycle handling, and signed-head journals. `lc631-dgcl-package-resume` requires fresh authority/intent/Cargo evidence after interruption without rewriting. Checkpoints alone never restore old completion.
- Discrete-continuous distillation that changes real state and binds downstream identities. Materialization retains exact source-relation evidence rather than substituting another constraint sharing a pair. Reject AST-digest tampering; distance/structure creates no truth/Grant.
- Explicit consumer views v1/v2. Strict limited v1 rejects unknown fields, v2, and true completion/send/commit booleans; v2 only renders reported state. No public JSON fields were removed/renamed; new fields/schemas are additive.
- Validator revisions bound to package version and parser/projection/closure implementations, evidence invalidation, schema/roundtrip/permission mutations, 16 cuts, and real call/head/callback cut→restore tests.

### Verification and interpretation

See the [final verification](../validation/epia2-alpha2-release-verification-2026-10-02.md), [prepromotion gates](../validation/epia2-prepromotion-functional-pass-2026-10-02.md), [operating profile](dgcl-operating-profile-and-closure.md), and [wire compatibility](dgcl-wire-compatibility.md). Source-only packages exclude weights, keys, caches, personal host paths, and unaudited distribution binaries. Fresh builds use an existing toolchain/registry cache and do not establish clean-machine provisioning or plugin installation.

ResearchEvaluation remains `PendingNoCorpus`; real hosts remain `PendingHostObservation`. No claim of complete arbitrary-language understanding, hallucination prevention, long-session performance, OS sandboxing, immediate host revocation, arbitrary build-world completeness, formal proof, or zero weaknesses. Keep inherited historical Held/global promotion separate from this standalone source profile's success. Epistesys-7 was not added.
