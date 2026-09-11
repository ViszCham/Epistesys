# LC631 ProjectionWitness Contract

## 日本語

V3 ProjectionDefectGraphは`SourceLedgerId`、obligation固有anchor、`ProjectionEdgeId`、revision/digest付き`TargetClaimId`を保持します。one-to-one、one-to-many、many-to-oneを許しますが、別ledgerのanchorをobligationへ付け替えられません。IntroducedWithoutSourceはobligationを偽装せずtarget-only edgeとして保持します。Exactにはobligation kind・stage別のauthenticated VerifierCapability、matching revision、non-model evidence digestが必要です。

Canonical TranslationLossは数値ではなく、source obligationとtarget realizationのstage付き対応です。MustはSeedToContract、ContractToProgram、ProgramToCandidate、CandidateToOutputの全stage、またはauthenticated NotApplicable receiptが揃うまでCandidateCommitになりません。domain payloadはtyped detailとして残り、scalarへ自動圧縮されません。

## English

ProjectionDefectGraph v3 retains `SourceLedgerId`, obligation-specific anchors, `ProjectionEdgeId`, and revision/digest-bound `TargetClaimId`. It admits one-to-one, one-to-many, and many-to-one correspondence but rejects swapping an anchor from another ledger onto an obligation. IntroducedWithoutSource remains a target-only edge and cannot impersonate an obligation. Exact requires an authenticated VerifierCapability scoped to obligation kind and stage, a matching revision, and a non-model evidence digest.

Canonical TranslationLoss is a stage-aware mapping between a source obligation and target realization, not a scalar. A Must cannot reach CandidateCommit until SeedToContract, ContractToProgram, ProgramToCandidate, and CandidateToOutput all have correspondences or authenticated NotApplicable receipts. Domain payloads retain typed detail and are never automatically compressed into one scalar.
