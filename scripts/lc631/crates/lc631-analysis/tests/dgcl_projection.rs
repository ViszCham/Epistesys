#![forbid(unsafe_code)]

use lc631_analysis::{
    build_dgcl_pipeline, verify_dgcl_projection, DgclBackendSet, DgclInstructionParseState,
    InstructionParseState, TranslationLossKind,
};
use lc631_tl::{PreservationState, ProjectionGateDecision, ProjectionStage};

fn report(source: &str) -> lc631_analysis::DgclPipelineReport {
    build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap()
}

#[test]
fn one_source_bound_artifact_projects_grammar_contract_to_program_ir_and_tl_loss_events() {
    let source = "Please test the package if the build passes.";
    let report = report(source);
    assert_eq!(report.program_ir.source_revision, report.source_revision);
    assert_eq!(
        report.translation_projection.source_revision,
        report.source_revision
    );
    assert_eq!(report.program_ir.requirements.len(), 1);

    let requirement = &report.program_ir.requirements[0];
    assert_eq!(requirement.parse_state, DgclInstructionParseState::Parsed);
    assert_eq!(requirement.root_source_revision, report.source_revision);
    assert_eq!(
        source.get(requirement.source_span.clone()),
        Some("Please test the package if the build passes.")
    );
    assert_eq!(requirement.alternatives.len(), 1);
    assert_eq!(
        requirement.selected_alternative_id.as_deref(),
        Some(requirement.alternatives[0].alternative_id.as_str())
    );
    assert_eq!(
        requirement.alternatives[0].root_source_span.start,
        requirement.source_span.start + requirement.alternatives[0].ast.source_span.start
    );
    assert!(!requirement.semantic_truth_claim);
    assert!(!requirement.authority_grant);
    assert!(!report.program_ir.authority_created);
    assert!(!report.translation_projection.authority_created);
    assert!(!report.translation_projection.output_commit_allowed);
    assert!(!report.translation_projection.scalar_aggregate_used);

    let stages = report
        .translation_projection
        .loss_events
        .iter()
        .filter(|event| event.requirement_id == requirement.requirement_id)
        .map(|event| (event.stage, event.state, event.kind))
        .collect::<Vec<_>>();
    assert_eq!(stages.len(), 4);
    assert!(stages
        .iter()
        .all(|(_, state, _)| *state == PreservationState::Unresolved));
    let requirement_events = report
        .translation_projection
        .loss_events
        .iter()
        .filter(|event| event.requirement_id == requirement.requirement_id)
        .collect::<Vec<_>>();
    assert!(requirement_events.iter().all(|event| {
        event.stage_event_digest.starts_with("sha256:") && event.stage_event_digest.len() == 71
    }));
    assert_eq!(
        requirement_events[0].output_digest,
        requirement_events[1].input_digest
    );
    let contract_claim = report
        .translation_projection
        .envelope
        .target_claims
        .iter()
        .find(|claim| claim.artifact == "deep-grammar-contract")
        .unwrap();
    let program_claim = report
        .translation_projection
        .envelope
        .target_claims
        .iter()
        .find(|claim| claim.artifact == "program-ir-requirement")
        .unwrap();
    assert_eq!(
        requirement_events[0].output_digest.as_deref(),
        Some(contract_claim.content_digest.as_str())
    );
    assert_eq!(
        requirement_events[1].output_digest,
        requirement_events[2].input_digest
    );
    assert_eq!(
        requirement_events[1].output_digest.as_deref(),
        Some(program_claim.content_digest.as_str())
    );
    assert!(requirement_events[2].output_digest.is_none());
    assert!(requirement_events[3].input_digest.is_none());
    assert!(requirement_events[3].output_digest.is_none());
    assert!(stages.iter().any(|(stage, _, kind)| {
        *stage == ProjectionStage::ProgramToCandidate
            && *kind == TranslationLossKind::DownstreamCandidateAbsent
    }));
    assert!(stages.iter().any(|(stage, _, kind)| {
        *stage == ProjectionStage::CandidateToOutput
            && *kind == TranslationLossKind::FinalOutputUnbound
    }));
    assert_eq!(
        report.translation_projection.envelope.gate,
        ProjectionGateDecision::Clarify
    );
    assert!(verify_dgcl_projection(
        source,
        &report.dg1,
        &report.language_regions,
        &report.instruction_parses,
        &report.program_ir,
        &report.translation_projection,
    )
    .is_ok());
}

#[test]
fn ambiguous_and_unsupported_surface_states_survive_projection_without_selection_or_false_commit() {
    let ambiguous_source = "Please testしてください";
    let ambiguous = report(ambiguous_source);
    assert_eq!(ambiguous.instruction_parses.len(), 1);
    assert_eq!(
        ambiguous.instruction_parses[0].state,
        DgclInstructionParseState::Ambiguous
    );
    assert_eq!(ambiguous.program_ir.requirements.len(), 1);
    assert!(ambiguous.program_ir.requirements[0]
        .selected_alternative_id
        .is_none());
    assert!(ambiguous.program_ir.requirements[0].alternatives.len() >= 2);
    assert!(!ambiguous.program_ir.authority_created);

    let no_requirement = report("This is context only.");
    assert!(no_requirement.program_ir.requirements.is_empty());
    assert_eq!(
        no_requirement.translation_projection.gate,
        ProjectionGateDecision::Clarify
    );
    assert_eq!(
        no_requirement.translation_projection.envelope.gate,
        ProjectionGateDecision::Clarify
    );
    assert!(!no_requirement.translation_projection.output_commit_allowed);
    assert!(no_requirement
        .translation_projection
        .loss_events
        .iter()
        .any(|event| event.kind == TranslationLossKind::UnsupportedInputRetained));
}

#[test]
fn empty_source_has_an_explicit_noncommit_translation_gate() {
    let no_requirement = report("");
    assert!(no_requirement.program_ir.requirements.is_empty());
    assert_eq!(
        no_requirement.translation_projection.gate,
        ProjectionGateDecision::Clarify
    );
    assert!(
        no_requirement
            .translation_projection
            .empty_requirement_set_held
    );
    assert_eq!(
        no_requirement.translation_projection.envelope.gate,
        ProjectionGateDecision::CandidateCommit
    );
    assert!(!no_requirement.translation_projection.output_commit_allowed);
}

#[test]
fn program_ir_tampering_or_missing_operative_parse_is_rejected() {
    let source = "Please edit the file.";
    let mut tampered = report(source);
    tampered.program_ir.requirements[0].alternatives[0]
        .ast
        .actions[0]
        .polarity = lc631_analysis::InstructionPolarity::Forbidden;
    assert_eq!(
        verify_dgcl_projection(
            source,
            &tampered.dg1,
            &tampered.language_regions,
            &tampered.instruction_parses,
            &tampered.program_ir,
            &tampered.translation_projection,
        ),
        Err(lc631_analysis::DgclProjectionError::ProjectionDefect(
            lc631_analysis::ProjectionDefectDimension::Polarity
        ))
    );

    let mut missing_parse = report(source);
    missing_parse.instruction_parses.clear();
    assert_eq!(
        lc631_analysis::build_dgcl_projection(
            source,
            &missing_parse.dg1,
            &missing_parse.language_regions,
            &missing_parse.instruction_parses,
        )
        .unwrap_err(),
        lc631_analysis::DgclProjectionError::ParseStateMismatch
    );

    let report = report(source);
    assert_eq!(
        lc631_analysis::build_dgcl_projection(
            "Please edit another file.",
            &report.dg1,
            &report.language_regions,
            &report.instruction_parses,
        )
        .unwrap_err(),
        lc631_analysis::DgclProjectionError::SourceRevisionMismatch
    );
}

#[test]
fn dropped_conditions_scopes_requirements_and_source_anchors_report_their_own_defect_dimension() {
    let condition_source = "Please publish if tests pass.";
    let mut missing_condition = report(condition_source);
    missing_condition.program_ir.requirements[0].alternatives[0]
        .ast
        .conditions
        .clear();
    assert_eq!(
        verify_dgcl_projection(
            condition_source,
            &missing_condition.dg1,
            &missing_condition.language_regions,
            &missing_condition.instruction_parses,
            &missing_condition.program_ir,
            &missing_condition.translation_projection,
        ),
        Err(lc631_analysis::DgclProjectionError::ProjectionDefect(
            lc631_analysis::ProjectionDefectDimension::Condition
        ))
    );

    let mut missing_action = report(condition_source);
    missing_action.program_ir.requirements[0].alternatives[0]
        .ast
        .actions
        .clear();
    assert_eq!(
        verify_dgcl_projection(
            condition_source,
            &missing_action.dg1,
            &missing_action.language_regions,
            &missing_action.instruction_parses,
            &missing_action.program_ir,
            &missing_action.translation_projection,
        ),
        Err(lc631_analysis::DgclProjectionError::ProjectionDefect(
            lc631_analysis::ProjectionDefectDimension::SemanticStructure
        ))
    );

    let mut altered_loss_graph = report(condition_source);
    altered_loss_graph.translation_projection.loss_events[0].state = PreservationState::Exact;
    assert_eq!(
        verify_dgcl_projection(
            condition_source,
            &altered_loss_graph.dg1,
            &altered_loss_graph.language_regions,
            &altered_loss_graph.instruction_parses,
            &altered_loss_graph.program_ir,
            &altered_loss_graph.translation_projection,
        ),
        Err(lc631_analysis::DgclProjectionError::ProjectionDefect(
            lc631_analysis::ProjectionDefectDimension::TranslationGraph
        ))
    );

    let source = "Please edit the file.";
    let mut missing_scope = report(source);
    missing_scope.program_ir.requirements[0].alternatives[0]
        .ast
        .scopes
        .push(lc631_analysis::InstructionScopeCandidate {
            kind: lc631_analysis::InstructionScopeKind::Resource,
            value: "repo/file".into(),
            source_span: 0..1,
            action_indices: 0..1,
            state: lc631_analysis::InstructionScopeState::Candidate,
        });
    assert_eq!(
        verify_dgcl_projection(
            source,
            &missing_scope.dg1,
            &missing_scope.language_regions,
            &missing_scope.instruction_parses,
            &missing_scope.program_ir,
            &missing_scope.translation_projection,
        ),
        Err(lc631_analysis::DgclProjectionError::ProjectionDefect(
            lc631_analysis::ProjectionDefectDimension::ScopeDependency
        ))
    );

    let mut missing_requirement = report(source);
    missing_requirement.program_ir.requirements.clear();
    assert_eq!(
        verify_dgcl_projection(
            source,
            &missing_requirement.dg1,
            &missing_requirement.language_regions,
            &missing_requirement.instruction_parses,
            &missing_requirement.program_ir,
            &missing_requirement.translation_projection,
        ),
        Err(lc631_analysis::DgclProjectionError::ProjectionDefect(
            lc631_analysis::ProjectionDefectDimension::Requirement
        ))
    );

    let mut wrong_anchor = report(source);
    wrong_anchor.program_ir.requirements[0].alternatives[0]
        .root_source_span
        .start += 1;
    assert_eq!(
        verify_dgcl_projection(
            source,
            &wrong_anchor.dg1,
            &wrong_anchor.language_regions,
            &wrong_anchor.instruction_parses,
            &wrong_anchor.program_ir,
            &wrong_anchor.translation_projection,
        ),
        Err(lc631_analysis::DgclProjectionError::ProjectionDefect(
            lc631_analysis::ProjectionDefectDimension::SourceAnchor
        ))
    );
}

#[test]
fn old_parser_candidate_without_controlled_grammar_remains_a_blocking_residual() {
    let source = "Please delete files.";
    let mut report = report(source);
    report.instruction_parses[0].report = None;
    report.instruction_parses[0].state = DgclInstructionParseState::BudgetRejected;
    report.instruction_parses[0].error =
        Some(lc631_analysis::InstructionGrammarError::TokenBudgetExceeded);
    let (program_ir, translation) = lc631_analysis::build_dgcl_projection(
        source,
        &report.dg1,
        &report.language_regions,
        &report.instruction_parses,
    )
    .unwrap();
    assert!(program_ir.requirements.is_empty());
    assert!(program_ir
        .residuals
        .iter()
        .any(|residual| residual.blocking_requirement));
    assert_eq!(translation.envelope.gate, ProjectionGateDecision::Clarify);
    assert!(!translation.output_commit_allowed);

    // The legacy DG1 IDs are references only; they never supply a missing AST.
    assert_ne!(
        report.instruction_parses[0].state,
        DgclInstructionParseState::Parsed
    );
    assert_ne!(
        report.instruction_parses[0]
            .report
            .as_ref()
            .map(|parse| parse.state),
        Some(InstructionParseState::Parsed)
    );
}
