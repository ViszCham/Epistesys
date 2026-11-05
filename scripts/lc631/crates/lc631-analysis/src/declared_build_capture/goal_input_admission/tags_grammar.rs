//! Exhaustive actual enum tags; field additions require explicit revision review.

pub(super) fn grammarerror(value: &crate::InstructionGrammarError) -> &'static str {
    match value {
        crate::InstructionGrammarError::InvalidBudget => "invalid_budget",
        crate::InstructionGrammarError::SourceBudgetExceeded => "source_budget_exceeded",
        crate::InstructionGrammarError::TokenBudgetExceeded => "token_budget_exceeded",
        crate::InstructionGrammarError::ChartItemBudgetExceeded => "chart_item_budget_exceeded",
        crate::InstructionGrammarError::ForestBudgetExceeded => "forest_budget_exceeded",
        crate::InstructionGrammarError::DerivationDepthExceeded => "derivation_depth_exceeded",
        crate::InstructionGrammarError::PostprocessBudgetExceeded => "postprocess_budget_exceeded",
        crate::InstructionGrammarError::SourceSpanInvalid => "source_span_invalid",
    }
}

pub(super) fn parsestate(value: &crate::InstructionParseState) -> &'static str {
    match value {
        crate::InstructionParseState::Parsed => "parsed",
        crate::InstructionParseState::Ambiguous => "ambiguous",
        crate::InstructionParseState::Unresolved => "unresolved",
        crate::InstructionParseState::Unsupported => "unsupported",
    }
}

pub(super) fn instructionlanguage(value: &crate::InstructionLanguage) -> &'static str {
    match value {
        crate::InstructionLanguage::English => "english",
        crate::InstructionLanguage::Japanese => "japanese",
    }
}

pub(super) fn marker(value: &crate::InstructionMarker) -> &'static str {
    match value {
        crate::InstructionMarker::Please => "please",
        crate::InstructionMarker::Must => "must",
        crate::InstructionMarker::MustNot => "must_not",
        crate::InstructionMarker::Should => "should",
        crate::InstructionMarker::ShouldNot => "should_not",
        crate::InstructionMarker::May => "may",
        crate::InstructionMarker::DoNot => "do_not",
        crate::InstructionMarker::JapaneseRequest => "japanese_request",
        crate::InstructionMarker::JapaneseForbidden => "japanese_forbidden",
        crate::InstructionMarker::JapanesePermission => "japanese_permission",
        crate::InstructionMarker::JapaneseMust => "japanese_must",
    }
}

pub(super) fn coordinationstate(value: &crate::InstructionCoordinationState) -> &'static str {
    match value {
        crate::InstructionCoordinationState::Resolved => "resolved",
        crate::InstructionCoordinationState::Unresolved => "unresolved",
    }
}

pub(super) fn sharedargument(value: &crate::SharedArgumentState) -> &'static str {
    match value {
        crate::SharedArgumentState::NotApplicable => "not_applicable",
        crate::SharedArgumentState::UnresolvedCandidate => "unresolved_candidate",
    }
}

pub(super) fn modality(value: &crate::InstructionModality) -> &'static str {
    match value {
        crate::InstructionModality::Requested => "requested",
        crate::InstructionModality::Required => "required",
        crate::InstructionModality::Recommended => "recommended",
        crate::InstructionModality::Permitted => "permitted",
        crate::InstructionModality::Unknown => "unknown",
    }
}

pub(super) fn polarity(value: &crate::InstructionPolarity) -> &'static str {
    match value {
        crate::InstructionPolarity::Positive => "positive",
        crate::InstructionPolarity::Forbidden => "forbidden",
        crate::InstructionPolarity::Unknown => "unknown",
        crate::InstructionPolarity::Conflict => "conflict",
    }
}

pub(super) fn conjunction(value: &crate::InstructionConjunction) -> &'static str {
    match value {
        crate::InstructionConjunction::And => "and",
        crate::InstructionConjunction::Or => "or",
        crate::InstructionConjunction::Xor => "xor",
        crate::InstructionConjunction::But => "but",
        crate::InstructionConjunction::Then => "then",
        crate::InstructionConjunction::JapaneseTe => "japanese_te",
        crate::InstructionConjunction::JapaneseComma => "japanese_comma",
    }
}

pub(super) fn polarityscope(value: &crate::InstructionPolarityScopeState) -> &'static str {
    match value {
        crate::InstructionPolarityScopeState::LocalAction => "local_action",
        crate::InstructionPolarityScopeState::UnresolvedGroup => "unresolved_group",
    }
}

pub(super) fn conditionrelation(value: &crate::ConditionRelation) -> &'static str {
    match value {
        crate::ConditionRelation::Sufficient => "sufficient",
        crate::ConditionRelation::Necessary => "necessary",
        crate::ConditionRelation::Exception => "exception",
        crate::ConditionRelation::Temporal => "temporal",
    }
}

pub(super) fn temporalrelation(value: &crate::TemporalRelation) -> &'static str {
    match value {
        crate::TemporalRelation::Before => "before",
        crate::TemporalRelation::After => "after",
        crate::TemporalRelation::Until => "until",
        crate::TemporalRelation::When => "when",
    }
}

pub(super) fn conditionscope(value: &crate::ConditionScopeState) -> &'static str {
    match value {
        crate::ConditionScopeState::Resolved => "resolved",
        crate::ConditionScopeState::Unresolved => "unresolved",
    }
}

pub(super) fn conditionevidence(value: &crate::ConditionEvidenceState) -> &'static str {
    match value {
        crate::ConditionEvidenceState::Supported => "supported",
        crate::ConditionEvidenceState::Refuted => "refuted",
        crate::ConditionEvidenceState::Unknown => "unknown",
        crate::ConditionEvidenceState::Conflict => "conflict",
    }
}

pub(super) fn scopekind(value: &crate::InstructionScopeKind) -> &'static str {
    match value {
        crate::InstructionScopeKind::Resource => "resource",
        crate::InstructionScopeKind::Branch => "branch",
        crate::InstructionScopeKind::Environment => "environment",
        crate::InstructionScopeKind::Time => "time",
    }
}

pub(super) fn scopestate(value: &crate::InstructionScopeState) -> &'static str {
    match value {
        crate::InstructionScopeState::Candidate => "candidate",
        crate::InstructionScopeState::Ambiguous => "ambiguous",
        crate::InstructionScopeState::Unresolved => "unresolved",
    }
}

pub(super) fn referencestate(value: &crate::InstructionReferenceState) -> &'static str {
    match value {
        crate::InstructionReferenceState::Candidate => "candidate",
        crate::InstructionReferenceState::Ambiguous => "ambiguous",
        crate::InstructionReferenceState::Unresolved => "unresolved",
    }
}

pub(super) fn constraintkind(value: &crate::InstructionConstraintKind) -> &'static str {
    match value {
        crate::InstructionConstraintKind::Requires => "requires",
        crate::InstructionConstraintKind::Before => "before",
        crate::InstructionConstraintKind::After => "after",
        crate::InstructionConstraintKind::Until => "until",
        crate::InstructionConstraintKind::When => "when",
        crate::InstructionConstraintKind::Conditional => "conditional",
        crate::InstructionConstraintKind::Exception => "exception",
        crate::InstructionConstraintKind::Conflicts => "conflicts",
    }
}

pub(super) fn edgestate(value: &crate::InstructionEdgeState) -> &'static str {
    match value {
        crate::InstructionEdgeState::Candidate => "candidate",
        crate::InstructionEdgeState::Unresolved => "unresolved",
        crate::InstructionEdgeState::Conflict => "conflict",
    }
}
