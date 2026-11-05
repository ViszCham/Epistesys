//! Exhaustive actual enum tags; field additions require explicit revision review.

pub(super) fn stopreason(value: &lc631_tldg::DistillationStopReason) -> &'static str {
    match value {
        lc631_tldg::DistillationStopReason::ResidualsCleared => "residuals_cleared",
        lc631_tldg::DistillationStopReason::NoProgress => "no_progress",
        lc631_tldg::DistillationStopReason::EpochBudget => "epoch_budget",
    }
}

pub(super) fn anchorstate(value: &lc631_tldg::AnchorState) -> &'static str {
    match value {
        lc631_tldg::AnchorState::Verified => "verified",
        lc631_tldg::AnchorState::Unknown => "unknown",
        lc631_tldg::AnchorState::Incomparable => "incomparable",
    }
}

pub(super) fn relationkind(value: &lc631_tldg::RelationKind) -> &'static str {
    match value {
        lc631_tldg::RelationKind::Contains => "contains",
        lc631_tldg::RelationKind::Sequence => "sequence",
        lc631_tldg::RelationKind::Scope => "scope",
        lc631_tldg::RelationKind::BindingCandidate => "binding_candidate",
        lc631_tldg::RelationKind::DependencyCandidate => "dependency_candidate",
        lc631_tldg::RelationKind::EmbeddedIn => "embedded_in",
    }
}

pub(super) fn proposalauthority(value: &lc631_tldg::ProposalAuthority) -> &'static str {
    match value {
        lc631_tldg::ProposalAuthority::AdvisoryOnly => "advisory_only",
    }
}

pub(super) fn validationstate(value: &lc631_tldg::ValidationState) -> &'static str {
    match value {
        lc631_tldg::ValidationState::Passed => "passed",
        lc631_tldg::ValidationState::Failed => "failed",
        lc631_tldg::ValidationState::NeedsEvidence => "needs_evidence",
        lc631_tldg::ValidationState::Incomparable => "incomparable",
    }
}

pub(super) fn gate(value: &lc631_tl::ProjectionGateDecision) -> &'static str {
    match value {
        lc631_tl::ProjectionGateDecision::CandidateCommit => "candidate_commit",
        lc631_tl::ProjectionGateDecision::Review => "review",
        lc631_tl::ProjectionGateDecision::Clarify => "clarify",
        lc631_tl::ProjectionGateDecision::HardHold => "hard_hold",
    }
}

pub(super) fn projectionstage(value: &lc631_tl::ProjectionStage) -> &'static str {
    match value {
        lc631_tl::ProjectionStage::SeedToContract => "seed_to_contract",
        lc631_tl::ProjectionStage::ContractToProgram => "contract_to_program",
        lc631_tl::ProjectionStage::ProgramToCandidate => "program_to_candidate",
        lc631_tl::ProjectionStage::CandidateToOutput => "candidate_to_output",
        lc631_tl::ProjectionStage::AudioToTranscript => "audio_to_transcript",
        lc631_tl::ProjectionStage::FrameToObservation => "frame_to_observation",
        lc631_tl::ProjectionStage::ObservationToEvent => "observation_to_event",
    }
}

pub(super) fn preservation(value: &lc631_tl::PreservationState) -> &'static str {
    match value {
        lc631_tl::PreservationState::Exact => "exact",
        lc631_tl::PreservationState::Refinement => "refinement",
        lc631_tl::PreservationState::Weakened => "weakened",
        lc631_tl::PreservationState::Strengthened => "strengthened",
        lc631_tl::PreservationState::Contradicted => "contradicted",
        lc631_tl::PreservationState::Dropped => "dropped",
        lc631_tl::PreservationState::IntroducedWithoutSource => "introduced_without_source",
        lc631_tl::PreservationState::Unresolved => "unresolved",
        lc631_tl::PreservationState::Incomparable => "incomparable",
    }
}

pub(super) fn verifier(value: &lc631_tl::VerifierKind) -> &'static str {
    match value {
        lc631_tl::VerifierKind::SourceSpan => "source_span",
        lc631_tl::VerifierKind::Schema => "schema",
        lc631_tl::VerifierKind::TypeChecker => "type_checker",
        lc631_tl::VerifierKind::Compiler => "compiler",
        lc631_tl::VerifierKind::Smt => "smt",
        lc631_tl::VerifierKind::Roundtrip => "roundtrip",
        lc631_tl::VerifierKind::RuntimeObservation => "runtime_observation",
        lc631_tl::VerifierKind::HumanReview => "human_review",
        lc631_tl::VerifierKind::ModelAdvisory => "model_advisory",
    }
}

pub(super) fn verificationstatus(value: &lc631_tl::VerificationStatus) -> &'static str {
    match value {
        lc631_tl::VerificationStatus::Passed => "passed",
        lc631_tl::VerificationStatus::Failed => "failed",
        lc631_tl::VerificationStatus::NeedsEvidence => "needs_evidence",
        lc631_tl::VerificationStatus::Unavailable => "unavailable",
        lc631_tl::VerificationStatus::AdvisoryOnly => "advisory_only",
    }
}

pub(super) fn losskind(value: &crate::TranslationLossKind) -> &'static str {
    match value {
        crate::TranslationLossKind::SemanticPreservationUnverified => {
            "semantic_preservation_unverified"
        }
        crate::TranslationLossKind::UnsupportedInputRetained => "unsupported_input_retained",
        crate::TranslationLossKind::DownstreamCandidateAbsent => "downstream_candidate_absent",
        crate::TranslationLossKind::FinalOutputUnbound => "final_output_unbound",
    }
}
