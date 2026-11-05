//! Exhaustive actual enum tags; field additions require explicit revision review.

pub(super) fn planstatus(value: &crate::CompletionPlanStatus) -> &'static str {
    match value {
        crate::CompletionPlanStatus::ReadyForEvidence => "ready_for_evidence",
        crate::CompletionPlanStatus::ConditionEvidencePending => "condition_evidence_pending",
        crate::CompletionPlanStatus::NeedsClarification => "needs_clarification",
        crate::CompletionPlanStatus::EmptyRequirementSet => "empty_requirement_set",
    }
}

pub(super) fn evidencekind(value: &crate::CodingEvidenceKind) -> &'static str {
    match value {
        crate::CodingEvidenceKind::ImplementationBinding => "implementation_binding",
        crate::CodingEvidenceKind::ProductionConnection => "production_connection",
        crate::CodingEvidenceKind::StaticValidation => "static_validation",
        crate::CodingEvidenceKind::RuntimeValidation => "runtime_validation",
        crate::CodingEvidenceKind::AcceptanceTest => "acceptance_test",
    }
}

pub(super) fn applicability(value: &crate::TaskApplicability) -> &'static str {
    match value {
        crate::TaskApplicability::Required => "required",
        crate::TaskApplicability::Conditional => "conditional",
        crate::TaskApplicability::NotApplicable => "not_applicable",
        crate::TaskApplicability::Unresolved => "unresolved",
    }
}

pub(super) fn notapplicable(value: &crate::NotApplicableBasis) -> &'static str {
    match value {
        crate::NotApplicableBasis::ForbiddenActionUsesAbsenceCheck => {
            "forbidden_action_uses_absence_check"
        }
        crate::NotApplicableBasis::PermissionDoesNotRequestExecution => {
            "permission_does_not_request_execution"
        }
    }
}

pub(super) fn producer(value: &crate::CodingEvidenceProducer) -> &'static str {
    match value {
        crate::CodingEvidenceProducer::StructuralObservation => "structural_observation",
        crate::CodingEvidenceProducer::DeclaredConnectionObserver => "declared_connection_observer",
        crate::CodingEvidenceProducer::CompilerObserved => "compiler_observed",
        crate::CodingEvidenceProducer::RuntimeObserved => "runtime_observed",
        crate::CodingEvidenceProducer::AcceptanceOracleObserved => "acceptance_oracle_observed",
    }
}

pub(super) fn blockerkind(value: &crate::CompletionBlockerKind) -> &'static str {
    match value {
        crate::CompletionBlockerKind::EmptyRequirementSet => "empty_requirement_set",
        crate::CompletionBlockerKind::LegacyCandidateNeedsGrammar => {
            "legacy_candidate_needs_grammar"
        }
        crate::CompletionBlockerKind::AmbiguousInstructionParse => "ambiguous_instruction_parse",
        crate::CompletionBlockerKind::UnresolvedInstructionParse => "unresolved_instruction_parse",
        crate::CompletionBlockerKind::UnsupportedInstructionParse => {
            "unsupported_instruction_parse"
        }
        crate::CompletionBlockerKind::BudgetRejectedInstructionParse => {
            "budget_rejected_instruction_parse"
        }
        crate::CompletionBlockerKind::NoActionAlternative => "no_action_alternative",
        crate::CompletionBlockerKind::UnknownPolarityOrModality => "unknown_polarity_or_modality",
        crate::CompletionBlockerKind::ConditionEvidenceUnknown => "condition_evidence_unknown",
        crate::CompletionBlockerKind::ConditionScopeUnresolved => "condition_scope_unresolved",
    }
}

pub(super) fn implementationstatus(value: &crate::DgclImplementationStatus) -> &'static str {
    match value {
        crate::DgclImplementationStatus::ImplementationClosed => "implementation_closed",
        crate::DgclImplementationStatus::NoAction => "no_action",
        crate::DgclImplementationStatus::Hold => "hold",
        crate::DgclImplementationStatus::Clarify => "clarify",
        crate::DgclImplementationStatus::Conflict => "conflict",
    }
}

pub(super) fn gapstate(value: &crate::ImplementationGapState) -> &'static str {
    match value {
        crate::ImplementationGapState::VerifiedReceipt => "verified_receipt",
        crate::ImplementationGapState::NotApplicable => "not_applicable",
        crate::ImplementationGapState::Unavailable => "unavailable",
        crate::ImplementationGapState::Rejected => "rejected",
        crate::ImplementationGapState::Stale => "stale",
        crate::ImplementationGapState::ConditionalPending => "conditional_pending",
        crate::ImplementationGapState::Unresolved => "unresolved",
    }
}
