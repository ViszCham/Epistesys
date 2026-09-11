# LC631 Unified Geometric DeepGrammar Contract

## 日本語

### Canonical owner

Canonical parseはlossless source anchor、packed discrete derivation、typed validator receiptが所有します。Mixed-curvature geometry、Finsler cost、Gromov-Wasserstein coupling、empirical riskはproposal/advisoryであり、parse truth、Evidence provenance、Authority、OutputCommitを生成しません。

### 共通基質

自然言語とprogramは同じUnifiedSyntaxHypergraph schemaを使用します。差異はNatural/Programというtop-level boolではなく、GrammarProfile featureとbackend-specific validatorで保持します。

### TranslationLoss

Canonical TranslationLossはProjectionDefectGraph v3です。source ledger、obligation anchor、stage、revision-bound target claimを保持し、Exact、Refinement、Weakened、Strengthened、Contradicted、Dropped、IntroducedWithoutSource、Unresolved、Incomparableを分離します。

Exactにはobligation kind別VerifierCapabilityとrevision一致が必要です。source span digest単独またはModelAdvisoryはsemantic Exactを検証できません。IntroducedWithoutSourceはinvalid recordとして捨てず、first-class defectとして保持します。

### Distillation

各epochの状態遷移はAnchoredDiscrete → ContinuousProposed → DiscreteDecoded → Validated → Backflowです。`max_epochs`は実反復回数を制御し、各output revisionとReprojectionRequestを記録します。grammar／thresholdを暗黙変更せず、geometry proposalはtyped decodeとdiscrete validator receiptなしにcanonical edgeへ昇格できません。

### Failure and release boundary

builtin backendもRevisionPinnedから始まり、source／backend／outputに束縛されたauthenticated execution receiptなしではValidatedになりません。legacy release boolは常にfail-closedです。外部parser、authenticated GPU parity、host output、remote CIが未観測ならtyped blockerを保持します。

---

## English

### Canonical owner

Canonical parsing is owned by lossless source anchors, packed discrete derivations, and typed validator receipts. Mixed-curvature geometry, Finsler costs, Gromov-Wasserstein coupling, and empirical risk are proposal/advisory surfaces and create no parse truth, Evidence provenance, Authority, or OutputCommit.

### Shared substrate

Natural language and programs use the same UnifiedSyntaxHypergraph schema. Differences remain in GrammarProfile features and backend-specific validators rather than a top-level Natural/Program boolean.

### TranslationLoss

Canonical TranslationLoss is ProjectionDefectGraph v3. It retains the source ledger, obligation anchor, stage, and revision-bound target claim while separating Exact, Refinement, Weakened, Strengthened, Contradicted, Dropped, IntroducedWithoutSource, Unresolved, and Incomparable.

Exact requires an obligation-specific VerifierCapability with a matching revision. A source-span digest alone or ModelAdvisory cannot verify semantic Exact. IntroducedWithoutSource is retained as a first-class defect rather than rejected as an invalid record.

### Distillation

Each epoch follows AnchoredDiscrete → ContinuousProposed → DiscreteDecoded → Validated → Backflow. `max_epochs` controls real iterations, and each output revision and ReprojectionRequest is recorded. Grammar and thresholds are not mutated implicitly, and geometry proposals cannot become canonical edges without typed decode and discrete validator receipts.

### Failure and release boundary

Builtin backends also start at RevisionPinned and cannot become Validated without an authenticated execution receipt bound to source, backend, and output. Legacy release booleans always fail closed. Unobserved external parsers, authenticated GPU parity, host output, or remote CI remain typed blockers.
