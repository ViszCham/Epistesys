# Epistesys

## 日本語

> **シードは仕様ではない。** \
> **シード以前からコミットまで、すべての射影に証跡を。** \
> Epistesysは、解釈・権限・証拠・計算・検証・出力コミットを、ひとつの境界付き制御経路として扱います。

### 位置付け

Epistesys（エピステシス）`6.3.2-alpha.1`は、Labyrinth-Codex v6.3.1の選択source snapshotを基にしたprivate cloneです。初回版は新規Git履歴で独立して保存し、元の履歴、個人状態、credentials、cache、receipt root、replay ledgerを引き継ぎません。内部の`lc631-*` crate・command・schema IDは、挙動差を抑える互換識別子として当面保持します。

**Capability:** receipt検証、TL/TLDG、256×8 world materialization、RPA-00〜39、candidate-only Media、Host replay v2を、Epistesys identityの下で実行できます。

**Outcome:** seedからcandidate・validation・outputまでの状態と証拠境界を、CLI・schema・fixture・Rust testとして追跡できます。

**First action:** `./scripts/run-epistesys.ps1 lc631-tl-doctor --prompt "mixed seed"`を実行します。

**First result:** source roundtrip、parse defect、Clarify/Hold、world budget、promotion predicateをJSONで観測できます。

**Evidence:** Cargo build/test/clippy、主要CLI、RepoSeiri、Assurance-Compilerの記録を個別に保持します。

**Constraint:** alphaはsource-onlyで、外部receipt、fresh host pickup、外部backend、remote CI、occupancy、一般性能を自動保証しません。

### 継承した実装面

- **Authenticated receipt**：principal、receipt class、subject revision、scope、nonce、payload digest、期限、parentを検証し、wire入力を`Untrusted`へ戻してからopaque verified stateだけを強いgateへ通します。
- **Unified DeepGrammar / TranslationLoss v3**：自然言語とprogramをlossless source、token/region lattice、UnifiedSyntaxHypergraph、constraint graph、semantic viewへ投影し、ProjectionDefectGraphで対応・欠落・矛盾・比較不能を保持します。geometry proposalはcanonical truthやauthorityを生成しません。
- **措定世界とGPU数値島**：256個のdistinct worldを8 projectionへ通し、2,048 evaluation rowsとして扱います。8 logical lane、bounded sparse relation、device-side reduction、CPU final validation、fault quarantineを持ちます。
- **Program Analysis / Media**：Rust・Python・AssemblyをRPA-00〜39のtyped stageとして扱い、EvidenceStateとClosureStateを分離します。Mediaはauthenticated local source、backend、model、license、method、budgetへ束縛し、candidateを自動的にValidatedへ昇格させません。
- **Host / replay**：exact output bytes digest、HostOutputReceipt v2、Stop hook、sink再検証、append-only replayを接続します。legacy fieldは互換表示であり、強い証拠ではありません。

### 実装済みで使える能力

このalphaは、名前だけを置き換えた空の器ではありません。v6.3.1の次の実装面を、Epistesysのidentityと独立launcherの下で実行できます。

- Rust workspaceの12 crate、feature-reduced/default build、workspace test、strict clippy、locked release build。
- `lc631-tl-doctor`によるsource roundtrip、obligation、ProjectionDefect、Clarify/Hold境界の出力。
- `lc631-world-doctor`による256 distinct world、8 projection、2,048 materialized evaluationの厳密なbudget検査。
- `lc631-promotion-gate`による不足receiptのfail-closed判定、`promotion_allowed=false`と`automatic_promotion=false`の明示。
- RPA-00〜39のRust/Python/Assembly解析型、EvidenceStateとClosureStateの分離、tool provenanceの保持。
- authenticated local Media計画、候補型、Media observation、TranslationEnvelope接続。YouTube URLや任意JSONだけではValidatedになりません。
- exact output digest、HostOutputReceipt v2、Stop hook、sink再検証、append-only replayのsource経路。
- `run-epistesys.ps1`をclone外のcwdから呼び出せる独立起動。

これらはcloneで実行・検査できる機能です。外部receipt、実Host v2 pickup、外部parser/model、remote CI、shader occupancy、一般的な性能比較が未成立でも、上記の実装面そのものを未実装とは扱いません。

### 起動

```powershell
./scripts/run-epistesys.ps1 lc631-tl-doctor --prompt "mixed seed"
./scripts/run-epistesys.ps1 lc631-world-doctor --prompt "mixed seed"
./scripts/run-epistesys.ps1 lc631-doctor --repo .
```

`lc631-*`は継承した互換commandです。Epistesys用launcherは自身の配置からrunnerを解決し、旧リポジトリの絶対pathへfallbackしません。

### 文書入口

- [Clone contract](docs/migration-contract.md)：継承・除外・初回版の境界
- [Adoption manifest](docs/adoption-manifest.md)：採用119 filesと除外4項目
- [Clone verification](docs/clone-verification.md)：EPI-05の実行結果
- [Assurance boundary](docs/assurance-compiler-status.md)：bounded static reviewのstatus
- [Known limitations](docs/known-limitations.md)：alphaの未観測・保留・再検証条件
- [RepoSeiri audit](docs/reposeiri-audit.md)：構成・文言・保留状態の監査記録

`docs/`内のv6.3.1文書は継承元の履歴的な設計・監査記録です。Epistesys独自の機能追加を示すものではなく、現在のidentityと状態は本README、clone verification、known limitationsを基準にします。

### 初回alphaの状態

EPI-04時点ではsource-only cloneです。移植元のWindows/macOS packaged binaryは絶対build path監査のため除外し、EPI-05でEpistesys sourceから再ビルドします。継承したUnavailable、Hold、Clarify、fallback、未観測範囲は未解決のまま記録します。これは一般性能、形式証明、host-level enforcement、外部backendの完成を意味しません。

Epistesys-7は、初回cloneの挙動比較と情報分離が完了した後に、別版・別変更として設計します。

## English

> **Seed is not spec.** \
> **From pre-seed to commit, every projection leaves a witness.** \
> Epistesys governs interpretation, authority, evidence, computation, validation, and output commitment as one bounded control path.

### Position

Epistesys (Japanese: エピステシス) `6.3.2-alpha.1` is a private clone based on a selected Labyrinth-Codex v6.3.1 source snapshot. The initial version will be stored with a new Git history and will not inherit original history, personal state, credentials, caches, receipt roots, or replay ledgers. Internal `lc631-*` crate, command, and schema IDs remain compatibility identifiers for now to limit behavioral drift.

**Capability:** Epistesys executes receipt verification, TL/TLDG, 256×8 world materialization, RPA-00 through RPA-39, candidate-only Media, and Host replay v2 under its own identity.

**Outcome:** It tracks states and evidence boundaries from seed through candidate, validation, and output using CLI, schema, fixture, and Rust-test surfaces.

**First action:** run `./scripts/run-epistesys.ps1 lc631-tl-doctor --prompt "mixed seed"`.

**First result:** observe source roundtrip, parse defects, Clarify/Hold, the world budget, and promotion predicates as JSON.

**Evidence:** keep Cargo build/test/clippy, major CLI, RepoSeiri, and Assurance-Compiler records separately.

**Constraint:** the alpha is source-only and does not automatically guarantee external receipts, fresh host pickup, external backends, remote CI, occupancy, or general performance.

### Inherited implementation surface

- **Authenticated receipts:** verify principal, receipt class, subject revision, scope, nonce, payload digest, expiry, and parent; return wire inputs to `Untrusted` and admit only opaque verified states to strong gates.
- **Unified DeepGrammar / TranslationLoss v3:** project natural language and programs through lossless source, token/region lattices, UnifiedSyntaxHypergraph, constraint graph, and semantic views; retain correspondence, omissions, contradictions, and incomparability in ProjectionDefectGraph. Geometry proposals create no canonical truth or authority.
- **Posited worlds and GPU numeric islands:** pass 256 distinct worlds through eight projections as 2,048 evaluation rows. The inherited surface includes eight logical lanes, bounded sparse relations, device-side reduction, CPU final validation, and fault quarantine.
- **Program Analysis / Media:** represent Rust, Python, and Assembly through RPA-00 through RPA-39 typed stages and keep EvidenceState separate from ClosureState. Media binds an authenticated local source to backend, model, license, method, and budget; candidates do not auto-promote to Validated.
- **Host / replay:** connect exact output-byte digests, HostOutputReceipt v2, the Stop hook, sink re-verification, and append-only replay. Legacy fields remain compatibility views and are not strong evidence.

### Implemented and usable capabilities

This alpha is not an empty rename. The following v6.3.1 implementation surfaces execute under the Epistesys identity and independent launcher:

- A 12-crate Rust workspace with feature-reduced/default builds, workspace tests, strict clippy, and a locked release build.
- `lc631-tl-doctor` output for source roundtrip, obligations, ProjectionDefects, and Clarify/Hold boundaries.
- `lc631-world-doctor` exact budget checks for 256 distinct worlds, eight projections, and 2,048 materialized evaluations.
- `lc631-promotion-gate` fail-closed decisions when receipts are missing, explicitly returning `promotion_allowed=false` and `automatic_promotion=false`.
- RPA-00 through RPA-39 typed Rust/Python/Assembly analysis, separate EvidenceState and ClosureState, and retained tool provenance.
- Authenticated local Media plans, candidate types, Media observations, and TranslationEnvelope linkage. A YouTube URL or arbitrary JSON alone cannot become Validated.
- Source paths for exact output digests, HostOutputReceipt v2, the Stop hook, sink re-verification, and append-only replay.
- Independent startup through `run-epistesys.ps1` from a cwd outside the clone.

These capabilities execute and can be inspected in the clone. Missing external receipts, fresh Host v2 pickup, external parsers/models, remote CI, shader occupancy, and general performance comparisons do not make the implementation surfaces above nonexistent.

### Startup

```powershell
./scripts/run-epistesys.ps1 lc631-tl-doctor --prompt "mixed seed"
./scripts/run-epistesys.ps1 lc631-world-doctor --prompt "mixed seed"
./scripts/run-epistesys.ps1 lc631-doctor --repo .
```

`lc631-*` are inherited compatibility commands. The Epistesys launcher resolves its runner from its own installation and never falls back to an absolute path in the old repository.

### Documentation entry points

- [Clone contract](docs/migration-contract.md): inheritance, exclusions, and initial-version boundaries
- [Adoption manifest](docs/adoption-manifest.md): 119 adopted files and four exclusions
- [Clone verification](docs/clone-verification.md): EPI-05 execution results
- [Assurance boundary](docs/assurance-compiler-status.md): bounded static-review status
- [Known limitations](docs/known-limitations.md): alpha observations, holds, and revalidation conditions
- [RepoSeiri audit](docs/reposeiri-audit.md): repository-structure, wording, and hold observations

The v6.3.1 documents under `docs/` are inherited historical design and audit records. They do not describe new Epistesys features; the current identity and state are defined by this README, clone verification, and known limitations.

### Initial alpha state

At EPI-04 this is a source-only clone. The source Windows/macOS packaged binaries were excluded by the absolute-build-path audit and will be rebuilt from Epistesys source in EPI-05. Inherited Unavailable, Hold, Clarify, fallback, and unobserved ranges remain recorded. This does not establish general performance, formal proof, host-level enforcement, or completed external backends.

Epistesys-7 will be designed as a separate version and change set after initial clone behavior and information separation are complete.
