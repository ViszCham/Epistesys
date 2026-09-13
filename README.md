# Epistesys

## 日本語

> **シードは仕様ではない。** \
> **シード以前からコミットまで、すべての射影に証跡を。** \
> Epistesysは、解釈・権限・証拠・計算・検証・出力コミットを、ひとつの境界付き制御経路として扱います。

### Epistesysとは

Epistesys（エピステシス）は、AIの推論・実行を外部から境界付きで制御する、長期的な**AI control / reliability research project**です。解釈（interpretation）、権限（authority）、証拠（evidence）、状態（state）、計算（computation）、検証（validation）、出力コミット（output commitment）を一つのbounded control pathとして扱う、実験的なAI制御アーキテクチャを研究しています。

生成が誤り得ることを前提に、unsupported candidate、missing evidence、projection defect、未解決obligationを、回答の確定やactionへ進む前に観測・制御できるようにする設計です。未検証・矛盾・証拠不足の状態が検証済みの事実として通過するfailureを、検出・抑制・封じ込めできるかを調べます。生成そのものの無誤謬性や「hallucination-free AI」を主張するものではありません。

長期のinteractionでもinstruction、state、evidence lineageを保持・再検証する制御系が研究対象です。bounded control、明示的なevidence/state表現、defect tracking、validation gates、replay/provenance、long-horizon state integrityを通して、信頼性を保てる運用範囲をどこまで広げられるかを研究します。これらの効果と、そのための機構の実装・実行確認は別に評価します。

### 研究目標

- **Hallucination containment（裏付けのない生成の封じ込め）**：unsupportedなcandidateや推測が、十分なevidenceなしにvalidated / committed stateへ昇格するfailureを減らせるか研究します。誤った生成の検出・封じ込めを目指す設計であり、hallucinationの防止効果は未検証です。
- **Constraint preservation（制約の保持）**：長い・複雑・多段・入れ子・相互依存・競合するinstructionで、重要なconstraint、exception、dependency、intentの脱落・融解・誤解釈を抑え、保持・再検証できるか研究します。
- **Long-horizon instruction integrity（長期interactionでの指示整合性）**：sessionが長くなったとき、初期instruction、state、authority、未解決obligation、decision history、evidence provenanceの忘却・drift・脱落を抑えられるか研究します。記憶容量だけでなく、指示の有効範囲、状態の更新、証拠の由来、過去の判断を再検証できることを扱います。
- **Failure observability（失敗の可観測性）**：不明確さ、missing evidence、projection defect、contradiction、未解決dependencyを、commit前に`Clarify` / `Hold`等の明示状態として露出できる設計を研究します。証拠不足のまま回答・actionへ進むfailureと、不必要に保留するfailureの両方を評価します。
- **Reliable complexity frontier（信頼性を維持できる複雑性の範囲）**：instruction complexity、dependency depth、保持すべきstate量、session lengthが増えても、定義したreliability targetを維持できる範囲を拡張できるか研究します。複雑性やsession長に伴うreliability collapseを測定し、課題族・総計算予算・目標信頼度を定めた**reliable operating envelope**の変化として評価します。

長期目標は、AIが定義された信頼性を維持したまま扱える指示複雑性・状態量・session長の範囲を押し広げることです。現在のalphaはそのためのbaseline implementationであり、プロジェクトの最終到達点ではありません。

### 研究状態

**上記は研究目標です。設計機構はその達成手段であり、期待される信頼性改善は未検証の研究仮説です。Epistesysとしてのbenchmark検証と独立評価は未実施です（benchmark pending / independent evaluation pending）。**

研究目標は、定義した信頼性を維持できる範囲を明らかにし、その拡張可能性を検証することです。設計目標は、制約・権限・状態・証拠を明示的に保持し、defectを観測可能にし、commit前に再検証できる経路を作ることです。「これらの機構により、同じモデル・課題・総予算で誤ったcommitや制約脱落が減る」という予想が研究仮説です。機構と予想効果の対応を設計仮説として記録し、[研究仮説と評価方針](docs/research-hypotheses.md)で区別します。

現在の`6.3.2-alpha.1`は、Epistesys固有のreasoning improvementを実証した版ではありません。hallucination reduction、complex-instruction reliability、long-session retention、reliable complexity frontierの拡張は今後検証する研究仮説です。現在確認しているのは主にimplementation surface、state/evidence boundary、およびCLI・schema・fixture・testの実行・検査です。build/test/CLI結果から一般性能や、長期session・複雑指示における改善を推論しません。

今後はhallucination、複雑なinstructionの制約保持、long-session retention、risk–coverage、誤ったcommitと過剰な保留、reliability frontierを測定する予定です。入力長、条件数、依存深度、状態保持量、session長、総予算を明示し、独立した評価で研究仮説を検証します。general performance、SOTA、formal proof、hallucination prevention、long-session robustnessは保証しません。

### 現在のalpha

Epistesys（エピステシス）`6.3.2-alpha.1`は、Labyrinth-Codex v6.3.1の選択source snapshot由来のbaseline implementationです。当初private cloneとして新規Git履歴で独立して保存され、元の履歴、個人状態、credentials、cache、receipt root、replay ledgerを引き継いでいません。内部の`lc631-*` crate・command・schema IDは、挙動差を抑える互換識別子として当面保持します。

**Capability:** receipt検証、TL/TLDG、256×8 world materialization、RPA-00〜39、candidate-only Media、Host replay v2を、Epistesys identityの下で実行できます。

**Outcome:** seedからcandidate・validation・outputまでの状態と証拠境界を、CLI・schema・fixture・Rust testとして追跡できます。

**First action:** `./scripts/run-epistesys.ps1 lc631-tl-doctor --prompt "mixed seed"`を実行します。

**First result:** TL doctorはsource roundtrip、parse defect、Clarify/Hold境界をJSONで返します。world budgetとpromotion predicateは、それぞれworld doctorとpromotion gateで観測します。

**Evidence:** Cargo build/test/clippy、主要CLI、RepoSeiri、Assurance-Compilerの記録を個別に保持します。

**Constraint:** alphaはsource-onlyで、外部receipt、fresh host pickup、外部backend、remote CI、occupancy、一般性能を自動保証しません。

### 継承した実装面

- **Authenticated receipt**：principal、receipt class、subject revision、scope、nonce、payload digest、期限、parentを検証し、wire入力を`Untrusted`へ戻してからopaque verified stateだけを強いgateへ通します。
- **Unified DeepGrammar / TranslationLoss v3**：自然言語とprogramをlossless source、token/region lattice、UnifiedSyntaxHypergraph、constraint graph、semantic viewへ投影し、ProjectionDefectGraphで対応・欠落・矛盾・比較不能を保持します。geometry proposalはcanonical truthやauthorityを生成しません。
- **措定世界とGPU数値島**：256個のdistinct worldを8 projectionへ通し、2,048 evaluation rowsとして扱います。8 logical lane、bounded sparse relation、device-side reduction、CPU final validation、fault quarantineを持ちます。
- **Program Analysis / Media**：Rust・Python・AssemblyをRPA-00〜39のtyped stageとして扱い、EvidenceStateとClosureStateを分離します。Mediaはauthenticated local source、backend、model、license、method、budgetへ束縛し、candidateを自動的にValidatedへ昇格させません。
- **Host / replay**：exact output bytes digest、HostOutputReceipt v2、Stop hook、sink再検証、append-only replayを接続します。legacy fieldは互換表示であり、強い証拠ではありません。

### 実装済みで使える能力

v6.3.1から継承した次の実装面を、Epistesysのidentityと独立launcherの下で実行・検査できます。ここでの能力は実装経路の存在と動作を指し、上記の研究目標に対する効果を実証するものではありません。

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
- [Initial clone release](docs/initial-clone-release-record.md)：root commitとmain bootstrapの記録

`docs/`内のv6.3.1文書は継承元の履歴的な設計・監査記録です。Epistesys独自の機能追加を示すものではなく、現在のidentityと状態は本README、clone verification、known limitationsを基準にします。

### 初回alphaの状態

現在のalphaはsource-onlyです。移植元のWindows/macOS packaged binaryは絶対build path監査のため除外しました。EPI-05のEpistesys sourceからのrelease buildは成功していますが、生成binaryにもbuild pathが残るため、配布物には採用していません。継承したUnavailable、Hold、Clarify、fallback、未観測範囲は未解決のまま記録します。local testの成功はcorrectnessやgeneral performanceを意味せず、形式証明、host-level enforcement、外部backendの完成も意味しません。candidateとValidatedは異なる状態であり、general-performance claimは引き続き検証待ちです。

Epistesys-7は、初回cloneの挙動比較と情報分離が完了した後に、別版・別変更として設計します。

## English

> **Seed is not spec.** \
> **From pre-seed to commit, every projection leaves a witness.** \
> Epistesys governs interpretation, authority, evidence, computation, validation, and output commitment as one bounded control path.

### What is Epistesys?

Epistesys (Japanese: エピステシス) is a long-term **AI control / reliability research project** studying bounded external control of AI reasoning and execution. It researches an experimental AI control architecture that treats interpretation, authority, evidence, state, computation, validation, and output commitment as one bounded control path.

Its design assumes generation can be wrong and aims to make unsupported candidates, missing evidence, projection defects, and unresolved obligations observable and controllable before an answer is committed or an action proceeds. It investigates whether this path can detect, reduce, and contain failures in which unverified, contradictory, or insufficiently supported states pass as validated facts. It does not assume infallible generation or claim “hallucination-free AI.”

The research also concerns control systems designed to preserve and revalidate instruction, state, and evidence lineage during long-horizon interaction. Through bounded control, explicit evidence/state representation, defect tracking, validation gates, replay/provenance, and long-horizon state integrity, the project investigates how far reliable operation can extend. These effects are evaluated separately from implementation and execution of the mechanisms intended to support them.

### Research goals

- **Hallucination containment:** investigate whether failures in which unsupported candidates or guesses advance to validated / committed states without sufficient evidence can be reduced. The architecture is designed to detect and contain unsupported generation; hallucination-prevention effects remain untested.
- **Constraint preservation:** investigate whether important constraints, exceptions, dependencies, and intent can be preserved and revalidated across long, complex, multi-stage, nested, interdependent, or competing instructions, reducing omission, erosion, and misinterpretation.
- **Long-horizon instruction integrity:** investigate whether forgetting, drift, and loss of initial instructions, state, authority, unresolved obligations, decision history, and evidence provenance can be reduced as sessions grow. This concerns instruction scope, state updates, evidence lineage, and revalidation of earlier decisions as well as memory capacity.
- **Failure observability:** research designs that expose ambiguity, missing evidence, projection defects, contradictions, and unresolved dependencies as explicit states such as `Clarify` / `Hold` before commitment. Evaluate both proceeding to an answer/action with insufficient evidence and unnecessarily withholding a response.
- **Reliable complexity frontier:** investigate whether the range that meets a defined reliability target can expand as instruction complexity, dependency depth, required state retention, and session length increase. Measure reliability collapse under increasing complexity or session length, evaluating changes in the **reliable operating envelope** for specified task families, total compute budgets, and target reliability.

The long-term goal is to expand the range of instruction complexity, state volume, and session length that AI can handle while maintaining defined reliability. The current alpha is a baseline implementation, not the endpoint of the project.

### Research status

**The goals above are research goals. Design mechanisms are means toward those goals; expected reliability improvements are untested research hypotheses. Epistesys-specific benchmarking and independent evaluation remain pending.**

The research goal is to characterize the range that maintains defined reliability and test whether it can expand. Design objectives are to retain constraints, authority, state, and evidence explicitly, expose defects, and enable revalidation before commitment. The prediction that these mechanisms reduce erroneous commitments or dropped constraints under the same model, tasks, and total budget is a research hypothesis. Mechanism-to-effect expectations are recorded as design hypotheses, distinguished in the [research hypotheses and evaluation plan](docs/research-hypotheses.md).

The current `6.3.2-alpha.1` does not demonstrate Epistesys-specific reasoning improvement. Hallucination reduction, complex-instruction reliability, long-session retention, and expansion of the reliable complexity frontier are hypotheses for future evaluation. Current observations primarily concern implementation surfaces, state/evidence boundaries, and execution/inspection of CLI, schema, fixture, and test surfaces. Build/test/CLI results do not establish general performance or improvements on long sessions or complex instructions.

Planned measurements cover hallucination, constraint preservation under complex instructions, long-session retention, risk–coverage, erroneous commitment and excessive withholding, and the reliability frontier. Input length, condition count, dependency depth, retained state, session length, and total budget will be made explicit, with independent evaluation of the hypotheses. General performance, SOTA, formal proof, hallucination prevention, and long-session robustness are not guaranteed.

### Current alpha

Epistesys (Japanese: エピステシス) `6.3.2-alpha.1` is a baseline implementation derived from a selected Labyrinth-Codex v6.3.1 source snapshot. It was initially stored independently as a private clone with a new Git history and does not inherit original history, personal state, credentials, caches, receipt roots, or replay ledgers. Internal `lc631-*` crate, command, and schema IDs remain compatibility identifiers for now to limit behavioral drift.

**Capability:** Epistesys executes receipt verification, TL/TLDG, 256×8 world materialization, RPA-00 through RPA-39, candidate-only Media, and Host replay v2 under its own identity.

**Outcome:** It tracks states and evidence boundaries from seed through candidate, validation, and output using CLI, schema, fixture, and Rust-test surfaces.

**First action:** run `./scripts/run-epistesys.ps1 lc631-tl-doctor --prompt "mixed seed"`.

**First result:** the TL doctor returns source roundtrip, parse defects, and Clarify/Hold boundaries as JSON. Observe the world budget and promotion predicates through the world doctor and promotion gate respectively.

**Evidence:** keep Cargo build/test/clippy, major CLI, RepoSeiri, and Assurance-Compiler records separately.

**Constraint:** the alpha is source-only and does not automatically guarantee external receipts, fresh host pickup, external backends, remote CI, occupancy, or general performance.

### Inherited implementation surface

- **Authenticated receipts:** verify principal, receipt class, subject revision, scope, nonce, payload digest, expiry, and parent; return wire inputs to `Untrusted` and admit only opaque verified states to strong gates.
- **Unified DeepGrammar / TranslationLoss v3:** project natural language and programs through lossless source, token/region lattices, UnifiedSyntaxHypergraph, constraint graph, and semantic views; retain correspondence, omissions, contradictions, and incomparability in ProjectionDefectGraph. Geometry proposals create no canonical truth or authority.
- **Posited worlds and GPU numeric islands:** pass 256 distinct worlds through eight projections as 2,048 evaluation rows. The inherited surface includes eight logical lanes, bounded sparse relations, device-side reduction, CPU final validation, and fault quarantine.
- **Program Analysis / Media:** represent Rust, Python, and Assembly through RPA-00 through RPA-39 typed stages and keep EvidenceState separate from ClosureState. Media binds an authenticated local source to backend, model, license, method, and budget; candidates do not auto-promote to Validated.
- **Host / replay:** connect exact output-byte digests, HostOutputReceipt v2, the Stop hook, sink re-verification, and append-only replay. Legacy fields remain compatibility views and are not strong evidence.

### Implemented and usable capabilities

The following implementation surfaces inherited from v6.3.1 can be executed and inspected under the Epistesys identity and independent launcher. Capabilities here refer to the presence and operation of implementation paths; they do not demonstrate effects on the research goals above.

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
- [Initial clone release](docs/initial-clone-release-record.md): root commit and main-bootstrap record

The v6.3.1 documents under `docs/` are inherited historical design and audit records. They do not describe new Epistesys features; the current identity and state are defined by this README, clone verification, and known limitations.

### Initial alpha state

The current alpha is source-only. The source Windows/macOS packaged binaries were excluded by the absolute-build-path audit. The EPI-05 release build from Epistesys source succeeded, but the resulting binary also retained build paths and is not included in distribution. Inherited Unavailable, Hold, Clarify, fallback, and unobserved ranges remain recorded. Local test success does not establish correctness or general performance, formal proof, host-level enforcement, or completed external backends. Candidate and Validated are distinct states; general-performance claims remain pending evaluation.

Epistesys-7 will be designed as a separate version and change set after initial clone behavior and information separation are complete.
