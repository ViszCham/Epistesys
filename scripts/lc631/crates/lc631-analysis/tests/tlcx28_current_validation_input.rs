use lc631_analysis::{
    build_dgcl_pipeline,
    context_lowering::{lower_dgcl_context, LoweredContext},
    logical_lowering::{lower_dgcl_logical, LogicProfile, LogicalLoweringBudget},
    solver_coding_bridge::{
        dgcl_input::current::*, FiniteSupport, ReportCheck, ReportedRelation, SourceConditionState,
    },
    CodingEvidenceKind, DgclBackendSet, DgclCargoValidationPlan, DgclPipelineReport,
};
use lc631_core::{context::*, AuthorityRevision};

fn pipeline(source: &str) -> DgclPipelineReport {
    build_dgcl_pipeline(source, DgclBackendSet { english: None, japanese: None }).unwrap()
}

fn context(report: &DgclPipelineReport, scope: &str, authority: u64) -> LoweredContext {
    let pin = SourceRevision::of(scope).digest();
    lower_dgcl_context(
        report,
        ContextScope::declared(pin, pin, pin, pin),
        AuthorityRevision(authority),
        ContextLimits { max_contexts: 4, max_premises: 1024, max_edits: 4, max_statement_bytes: 262_144 },
    ).unwrap()
}

fn plan(report: &DgclPipelineReport, requirement: usize, kind: CodingEvidenceKind) -> DgclCargoValidationPlan {
    let task = report.completion_plan.requirements[requirement].tasks.iter()
        .find(|task| task.evidence_kind == kind).unwrap();
    DgclCargoValidationPlan {
        schema_version: "epistesys-dgcl-cargo-validation-plan.v1".into(),
        source_revision: report.source_revision.clone(),
        completion_plan_digest: report.completion_plan.plan_digest.clone(),
        requirement_id: task.requirement_id.clone(), task_id: task.task_id.clone(), kind,
        cargo_path: "unobserved-cargo".into(), cargo_digest: "unobserved-cargo-digest".into(),
        manifest_path: "Cargo.toml".into(), manifest_digest: "unobserved-manifest".into(),
        repository_root_digest: "unobserved-root".into(),
        acceptance_target: None, acceptance_test: None, connection: None,
        timeout_ms: 1000, tool_registration: None,
    }
}

#[test]
fn tlcx28_current_input_rejects_same_source_stale_scope_or_authority_context() {
    let report = pipeline("Please test the package.");
    let old = context(&report, "old scope", 7);
    for current in [context(&report, "new scope", 7), context(&report, "old scope", 8)] {
        let logical = lower_dgcl_logical(&report, &old, TaskKind::Coding,
            LogicProfile::ClassicalPropositionalV1, LogicalLoweringBudget::default()).unwrap();
        assert!(matches!(CurrentCodingInput::from_lowered(&report, &current, logical),
            Err(CurrentCodingError::ContextMismatch)));
    }
}

#[test]
fn tlcx28_current_validation_rejects_each_detached_plan_identity() {
    let report = pipeline("Please test the package.");
    let current = context(&report, "scope", 7);
    let input = CurrentCodingInput::lower(&report, &current, LogicalLoweringBudget::default()).unwrap();
    let requirement = input.bind_requirement(0).unwrap();
    let original = plan(&report, 0, CodingEvidenceKind::StaticValidation);
    let mut variants = vec![original.clone(); 6];
    variants[0].source_revision.push_str("changed");
    variants[1].completion_plan_digest.push_str("changed");
    variants[2].requirement_id.push_str("changed");
    variants[3].task_id.push_str("changed");
    variants[4].kind = CodingEvidenceKind::RuntimeValidation;
    variants[5].schema_version.push_str("changed");
    for detached in variants {
        assert!(matches!(requirement.bind_validation_plan(&detached),
            Err(CurrentCodingError::ValidationPlanMismatch)));
    }
}

#[test]
fn tlcx28_current_validation_uses_original_tasks_without_minting_binding_evidence() {
    let report = pipeline("Please test the package.");
    let current = context(&report, "scope", 7);
    let input = CurrentCodingInput::lower(&report, &current, LogicalLoweringBudget::default()).unwrap();
    let requirement = input.bind_requirement(0).unwrap();
    for kind in [CodingEvidenceKind::StaticValidation, CodingEvidenceKind::RuntimeValidation,
        CodingEvidenceKind::ProductionConnection, CodingEvidenceKind::AcceptanceTest] {
        let plan = plan(&report, 0, kind);
        let bound = requirement.bind_validation_plan(&plan).unwrap();
        assert!(std::ptr::eq(bound.plan(), &plan));
        assert!(report.completion_plan.requirements[0].tasks.iter()
            .any(|task| std::ptr::eq(task, bound.task())));
        assert_eq!(bound.task().evidence_kind, kind);
        assert_eq!(bound.context_revision(), current.original().state().revision());
    }
    let binding = plan(&report, 0, CodingEvidenceKind::ImplementationBinding);
    assert!(matches!(requirement.bind_validation_plan(&binding),
        Err(CurrentCodingError::ImplementationBindingIsNotCargo)));
    assert!(!report.completion_plan.coding_closure_allowed);
}

#[test]
fn tlcx28_current_condition_consumes_actual_finite_support_and_cut_without_source_promotion() {
    let report = pipeline("Please test the package if ready.");
    let current = context(&report, "scope", 7);
    let input = CurrentCodingInput::lower(&report, &current, LogicalLoweringBudget::default()).unwrap();
    let requirement = input.bind_requirement(0).unwrap();
    let index = requirement.requirement().condition_indices()[0];
    let query = requirement.condition_query(index, ReportedRelation::UniversalSupport).unwrap();
    let premises = [(query.predicate(), true)];
    let observed = query.observe(&premises, 100_000).unwrap();
    let review = requirement.review_condition(&observed).unwrap();
    assert_eq!(review.model_support, FiniteSupport::Supported);
    assert_eq!(review.report_check, ReportCheck::Agrees);
    assert_eq!(review.source_state, SourceConditionState::NeedsEvidence);
    let cut = query.observe(&[], 100_000).unwrap();
    assert_eq!(requirement.review_condition(&cut).unwrap().model_support, FiniteSupport::Undetermined);
    assert_eq!(cut.observation().enumerated_assignments(), 2);
    let plan = plan(&report, 0, CodingEvidenceKind::StaticValidation);
    assert!(matches!(requirement.bind_validation_plan(&plan), Err(CurrentCodingError::NotReady(_))));
}

#[test]
fn tlcx28_current_condition_rejects_distinct_live_owner_with_identical_source_and_context() {
    let report = pipeline("Please test the package if ready.");
    let current = context(&report, "scope", 7);
    let first = CurrentCodingInput::lower(&report, &current, LogicalLoweringBudget::default()).unwrap();
    let second = CurrentCodingInput::lower(&report, &current, LogicalLoweringBudget::default()).unwrap();
    let first_requirement = first.bind_requirement(0).unwrap();
    let second_requirement = second.bind_requirement(0).unwrap();
    let query = second_requirement.condition_query(0, ReportedRelation::Sat).unwrap();
    let observation = query.observe(&[], 100_000).unwrap();
    assert!(matches!(first_requirement.review_condition(&observation), Err(CurrentCodingError::ForeignConditionOwner)));
}

#[test]
fn tlcx28_current_validation_never_routes_non_coding_or_alternative_tasks_into_cargo() {
    let report = pipeline("Please build or test.");
    let current = context(&report, "scope", 7);
    let math = lower_dgcl_logical(&report, &current, TaskKind::Mathematics,
        LogicProfile::ClassicalPropositionalV1, LogicalLoweringBudget::default()).unwrap();
    assert!(CurrentCodingInput::from_lowered(&report, &current, math).is_err());
    let input = CurrentCodingInput::lower(&report, &current, LogicalLoweringBudget::default()).unwrap();
    for index in 0..report.completion_plan.requirements.len() {
        let requirement = input.bind_requirement(index).unwrap();
        let plan = plan(&report, index, CodingEvidenceKind::StaticValidation);
        assert!(matches!(requirement.bind_validation_plan(&plan), Err(CurrentCodingError::NotReady(_))));
    }
}

#[test]
fn tlcx28_current_validation_rejects_a_different_requirement_with_the_same_kind() {
    // Distinct complete commands in separate CommonMark paragraphs: repeating
    // the same command in one region can correctly canonicalize to one task.
    let first_source = "Please test the package.";
    let second_source = "Please test the parser.";
    let source = format!("{first_source}\n\n{second_source}");
    let report = pipeline(&source);
    assert_eq!(report.instruction_parses.len(), 2, "fixture needs two parsed regions");
    assert_eq!(report.program_ir.requirements.len(), 2, "fixture needs two actual IR requirements");
    assert_eq!(report.completion_plan.requirements.len(), 2);
    let first_ir = &report.program_ir.requirements[0];
    let second_ir = &report.program_ir.requirements[1];
    assert_ne!(first_ir.region_id, second_ir.region_id);
    assert_ne!(first_ir.requirement_id, second_ir.requirement_id);
    assert!(first_ir.selected_alternative_id.is_some());
    assert!(second_ir.selected_alternative_id.is_some());
    assert!(first_ir.source_span.end <= first_source.len());
    assert!(second_ir.source_span.start >= first_source.len() + 2);
    assert!(source[first_ir.source_span.clone()].contains("package"));
    assert!(source[second_ir.source_span.clone()].contains("parser"));
    for planned in &report.completion_plan.requirements {
        let original = report.program_ir.requirements.iter()
            .find(|ir| ir.requirement_id == planned.parent_program_requirement_id).unwrap();
        assert_eq!(planned.region_id, original.region_id);
        assert!(planned.source_span.start >= original.source_span.start);
        assert!(planned.source_span.end <= original.source_span.end);
        assert!(planned.ready_for_evidence, "fixture must retain actual ready lowering");
    }
    let current = context(&report, "scope", 7);
    let input = CurrentCodingInput::lower(&report, &current, LogicalLoweringBudget::default()).unwrap();
    let requirement = input.bind_requirement(0).unwrap();
    let plan = plan(&report, 1, CodingEvidenceKind::StaticValidation);
    let expected_id = &requirement.requirement().requirement().requirement_id;
    let readiness = requirement.requirement().readiness();
    let ready = lc631_analysis::solver_coding_bridge::dgcl_input::CodingInputReadiness::ReadyForEvidence;
    assert_eq!(readiness, ready, "fixture must reach the plan identity checks");
    let other = input.bind_requirement(1).unwrap();
    assert_eq!(other.requirement().readiness(), ready);
    assert!(other.bind_validation_plan(&plan).is_ok(), "foreign plan must be valid for its own requirement");
    assert_ne!(expected_id, &plan.requirement_id, "fixture must contain distinct requirements");
    match requirement.bind_validation_plan(&plan) {
        Err(CurrentCodingError::ValidationPlanMismatch) => {}
        Err(actual) => panic!(
            "expected ValidationPlanMismatch, got {actual:?}; readiness={readiness:?}; expected_requirement={expected_id}; other_requirement={}; other_task={}",
            plan.requirement_id, plan.task_id
        ),
        Ok(_) => panic!(
            "unexpectedly accepted foreign requirement; readiness={readiness:?}; expected_requirement={expected_id}; other_requirement={}; other_task={}",
            plan.requirement_id, plan.task_id
        ),
    }
}

#[test]
fn tlcx28_current_validation_preserves_needs_interpretation_before_plan_matching() {
    use lc631_analysis::solver_coding_bridge::dgcl_input::CodingInputReadiness;
    let report = pipeline("Please build and test.");
    assert_eq!(report.completion_plan.requirements.len(), 2);
    let current = context(&report, "scope", 7);
    let input = CurrentCodingInput::lower(&report, &current, LogicalLoweringBudget::default()).unwrap();
    let requirement = input.bind_requirement(0).unwrap();
    let plan = plan(&report, 1, CodingEvidenceKind::StaticValidation);
    assert_eq!(requirement.requirement().readiness(), CodingInputReadiness::NeedsInterpretation);
    assert!(matches!(requirement.bind_validation_plan(&plan),
        Err(CurrentCodingError::NotReady(CodingInputReadiness::NeedsInterpretation))));
}

#[test]
fn tlcx28_current_condition_keeps_inconsistent_sat_and_unavailable_results_distinct() {
    let report = pipeline("Please test the package if ready.");
    let current = context(&report, "scope", 7);
    let input = CurrentCodingInput::lower(&report, &current, LogicalLoweringBudget::default()).unwrap();
    let requirement = input.bind_requirement(0).unwrap();
    let sat = requirement.condition_query(0, ReportedRelation::Sat).unwrap();
    let premises = [(sat.predicate(), true), (sat.predicate(), false)];
    let observed = sat.observe(&premises, 100_000).unwrap();
    let review = requirement.review_condition(&observed).unwrap();
    assert_eq!(review.model_support, FiniteSupport::ContextInconsistent);
    assert_eq!(review.report_check, ReportCheck::WrongResultKind);
    assert_eq!(review.source_state, SourceConditionState::NeedsEvidence);
    let unavailable = requirement.condition_query(0, ReportedRelation::Unavailable).unwrap();
    let observed = unavailable.observe(&[], 100_000).unwrap();
    assert_eq!(requirement.review_condition(&observed).unwrap().report_check, ReportCheck::Unavailable);
    assert!(sat.observe(&[], 0).is_err());
}

#[test]
fn tlcx28_current_validation_consumes_real_preflight_and_detects_changed_manifest() {
    use sha2::{Digest, Sha256};
    fn digest(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut text = String::with_capacity(71);
        text.push_str("sha256:");
        for byte in Sha256::digest(bytes).iter().copied() {
            text.push(char::from(HEX[usize::from(byte >> 4)]));
            text.push(char::from(HEX[usize::from(byte & 15)]));
        }
        text
    }
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().join(format!("tlcx28-plan-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let manifest = root.join("Cargo.toml");
    let tool = root.join("non-executed-tool.fixture");
    let manifest_bytes = b"[package]\nname='preflight-fixture'\nversion='0.0.0'\n";
    let tool_bytes = b"Not executed: metadata/fingerprint preflight fixture only";
    std::fs::write(&manifest, manifest_bytes).unwrap();
    std::fs::write(&tool, tool_bytes).unwrap();
    let report = pipeline("Please test the package.");
    let current = context(&report, "scope", 7);
    let input = CurrentCodingInput::lower(&report, &current, LogicalLoweringBudget::default()).unwrap();
    let requirement = input.bind_requirement(0).unwrap();
    let mut plan = plan(&report, 0, CodingEvidenceKind::StaticValidation);
    plan.repository_root_digest = lc631_analysis::dgcl_repository_root_digest(&root).unwrap();
    plan.cargo_path = tool.to_str().unwrap().into();
    plan.cargo_digest = digest(tool_bytes);
    plan.manifest_digest = digest(manifest_bytes);
    let bound = requirement.bind_validation_plan(&plan).unwrap();
    assert_eq!(bound.preflight(&root), Ok(()));
    std::fs::write(&manifest, b"changed input").unwrap();
    assert_eq!(bound.preflight(&root), Err(lc631_analysis::DgclValidationProducerError::StaleInput));
    // Only the two exact files created in this unique directory are reclaimed.
    std::fs::remove_file(&manifest).unwrap();
    std::fs::remove_file(&tool).unwrap();
    std::fs::remove_dir(&root).unwrap();
}
