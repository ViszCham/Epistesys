use lc631_core::{context::*, AuthorityRevision};
use lc631_runtime::crosslogic_loss::*;
use lc631_solvers::{
    ConsequenceMode, ExecutionStatus, LogicalResult, ObjectiveResult, Quantification,
    Satisfiability, SolverRun, Support, TraceDomain,
};
use lc631_substrate::Digest32;

fn pin(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}

fn context() -> (ContextConsumer, ComparisonBinding) {
    let scope = ContextScope::declared(pin(1), pin(2), pin(3), pin(4));
    let original = OriginalContext::checked(
        SourceRevision::of("crosslogic fixture"),
        scope,
        AuthorityRevision(1),
        vec![],
        ContextLimits {
            max_contexts: 8,
            max_premises: 16,
            max_edits: 8,
            max_statement_bytes: 1024,
        },
    )
    .unwrap();
    let binding = ComparisonBinding {
        context: original.state().revision(),
        scope,
        root_query: pin(5),
        translated_query: pin(6),
        domain: pin(7),
        premises: pin(8),
        projection: pin(9),
    };
    (ContextConsumer::new(original).unwrap(), binding)
}

fn completed(result: LogicalResult) -> SolverRun {
    SolverRun::reported(ExecutionStatus::Completed, Some(result)).unwrap()
}

fn pair(binding: ComparisonBinding, a: SolverRun, b: SolverRun) -> BridgeDiagnostic {
    analyze_pair(
        ReportedObservation::reported(binding, a, &[]).unwrap(),
        ReportedObservation::reported(binding, b, &[]).unwrap(),
    )
}

fn entailment(support: Support, models: Quantification) -> SolverRun {
    completed(LogicalResult::Entailment { support, models })
}

#[test]
fn tlcx20_sat_is_not_entailment_and_request_reaches_live_context_consumer() {
    let (contexts, binding) = context();
    let diagnostic = pair(
        binding,
        completed(LogicalResult::Satisfiability(Satisfiability::Sat)),
        entailment(Support::Supported, Quantification::ForAll),
    );
    assert_eq!(
        diagnostic.correspondence(),
        Correspondence::NeedsReprojection(ReprojectionCause::OutcomeKindMismatch)
    );
    let request = diagnostic.reprojection().unwrap();
    let pending = request
        .inspect_current(&contexts, binding, binding)
        .unwrap();
    assert_eq!(pending.request(), request);
    assert_eq!(pending.state(), "NeedsEvidence/ReprojectorNotConnected");
    let mut stale = binding;
    stale.translated_query = pin(22);
    assert_eq!(
        request.inspect_current(&contexts, stale, binding),
        Err(PreparationError::QueryBindingMismatch)
    );
}

#[test]
fn tlcx20_brave_cautious_and_exists_forall_never_collapse() {
    let (_, binding) = context();
    let diagnostic = pair(
        binding,
        completed(LogicalResult::Inference {
            support: Support::Supported,
            mode: ConsequenceMode::Brave,
        }),
        completed(LogicalResult::Inference {
            support: Support::Supported,
            mode: ConsequenceMode::Cautious,
        }),
    );
    assert_eq!(
        diagnostic.reprojection().unwrap().cause(),
        ReprojectionCause::ConsequenceModeMismatch
    );
    assert_eq!(
        pair(
            binding,
            entailment(Support::Supported, Quantification::Exists),
            entailment(Support::Supported, Quantification::ForAll),
        )
        .reprojection()
        .unwrap()
        .cause(),
        ReprojectionCause::QuantificationMismatch
    );
    assert_eq!(
        pair(
            binding,
            completed(LogicalResult::ModelCheck {
                support: Support::Supported,
                traces: TraceDomain::Finite,
            }),
            completed(LogicalResult::ModelCheck {
                support: Support::Supported,
                traces: TraceDomain::Infinite,
            }),
        )
        .reprojection()
        .unwrap()
        .cause(),
        ReprojectionCause::TraceDomainMismatch
    );
}

#[test]
fn tlcx20_shared_bad_parser_agreement_never_becomes_independent_evidence() {
    let (_, binding) = context();
    let deps = [pin(11), pin(12)];
    let run = completed(LogicalResult::Satisfiability(Satisfiability::Unsat));
    let a = ReportedObservation::reported(binding, run, &deps).unwrap();
    let diagnostic = analyze_pair(a, a);
    assert_eq!(
        diagnostic.dependencies(),
        DependencyStatus::ReportedSharedDependency
    );
    assert_eq!(
        diagnostic.correspondence(),
        Correspondence::SameReportedOutcome
    );
    assert_eq!(diagnostic.reported_runs(), (run, run));
    assert_eq!(
        diagnostic.reprojection(),
        Err(PreparationError::MissingDiagnostic)
    );
    let disjoint = [pin(13)];
    let b = ReportedObservation::reported(binding, run, &disjoint).unwrap();
    assert_eq!(
        analyze_pair(a, b).dependencies(),
        DependencyStatus::Unassessed
    );
}

#[test]
fn tlcx20_both_neither_timeout_and_unknown_preserve_distinct_reasons() {
    let (_, binding) = context();
    for (support, cause) in [
        (Support::Both, ReprojectionCause::ConflictingSupport),
        (Support::Neither, ReprojectionCause::UnknownOutcome),
    ] {
        let run = entailment(support, Quantification::ForAll);
        let diagnostic = pair(binding, run, run);
        assert_eq!(diagnostic.reprojection().unwrap().cause(), cause);
        assert_eq!(diagnostic.reported_runs(), (run, run));
    }
    for state in [
        ExecutionStatus::Timeout,
        ExecutionStatus::ResourceExhausted,
        ExecutionStatus::Unsupported,
        ExecutionStatus::Failed,
    ] {
        let run = SolverRun::reported(state, None).unwrap();
        assert_eq!(
            pair(binding, run, run).reprojection().unwrap().cause(),
            ReprojectionCause::IncompleteExecution
        );
    }
    for result in [
        LogicalResult::Satisfiability(Satisfiability::Unknown),
        LogicalResult::Optimization(ObjectiveResult::Unknown),
    ] {
        let run = completed(result);
        assert_eq!(
            pair(binding, run, run).reprojection().unwrap().cause(),
            ReprojectionCause::UnknownOutcome
        );
    }
}

#[test]
fn tlcx20_context_query_domain_premise_and_semantics_swaps_are_diagnostic() {
    let (_, binding) = context();
    let run = completed(LogicalResult::Satisfiability(Satisfiability::Sat));
    let mut cases = [(binding, ReprojectionCause::ContextMismatch); 6];
    cases[0].0.context = binding.context.reconstructed([21; 32]);
    cases[1] = (
        ComparisonBinding {
            scope: ContextScope::declared(pin(21), pin(2), pin(3), pin(4)),
            ..binding
        },
        ReprojectionCause::ScopeMismatch,
    );
    cases[2] = (
        ComparisonBinding {
            root_query: pin(21),
            ..binding
        },
        ReprojectionCause::QueryMismatch,
    );
    cases[3] = (
        ComparisonBinding {
            domain: pin(21),
            ..binding
        },
        ReprojectionCause::DomainMismatch,
    );
    cases[4] = (
        ComparisonBinding {
            premises: pin(21),
            ..binding
        },
        ReprojectionCause::PremiseMismatch,
    );
    cases[5] = (
        ComparisonBinding {
            scope: ContextScope::declared(pin(1), pin(2), pin(3), pin(21)),
            ..binding
        },
        ReprojectionCause::SemanticsMismatch,
    );
    for (changed, expected) in cases {
        let diagnostic = analyze_pair(
            ReportedObservation::reported(binding, run, &[]).unwrap(),
            ReportedObservation::reported(changed, run, &[]).unwrap(),
        );
        assert_eq!(diagnostic.reprojection().unwrap().cause(), expected);
    }
}

#[test]
fn tlcx20_consumer_rejects_unknown_context_and_stale_scope() {
    let (contexts, binding) = context();
    let a = completed(LogicalResult::Satisfiability(Satisfiability::Sat));
    let b = completed(LogicalResult::Satisfiability(Satisfiability::Unsat));
    let mut unknown = binding;
    unknown.context = binding.context.reconstructed([22; 32]);
    let request = pair(unknown, a, b).reprojection().unwrap();
    assert_eq!(
        request.inspect_current(&contexts, unknown, unknown),
        Err(PreparationError::UnknownContext)
    );
    let stale = ComparisonBinding {
        scope: ContextScope::declared(pin(21), pin(2), pin(3), pin(4)),
        ..binding
    };
    let request = pair(stale, a, b).reprojection().unwrap();
    assert_eq!(
        request.inspect_current(&contexts, stale, stale),
        Err(PreparationError::StaleScope)
    );
}

#[test]
fn tlcx20_dependency_budget_and_duplicates_reject_before_pair_analysis() {
    let (_, binding) = context();
    let run = completed(LogicalResult::Satisfiability(Satisfiability::Sat));
    assert_eq!(
        ReportedObservation::reported(binding, run, &[pin(1); 17]).unwrap_err(),
        PreparationError::TooManyDependencies
    );
    assert_eq!(
        ReportedObservation::reported(binding, run, &[pin(1), pin(1)]).unwrap_err(),
        PreparationError::DuplicateDependency
    );
    let dependencies: Vec<_> = (0..16).map(pin).collect();
    assert!(ReportedObservation::reported(binding, run, &dependencies).is_ok());
}

#[test]
fn tlcx20_disagreement_changes_request_while_identical_reports_do_not_issue_one() {
    let (_, binding) = context();
    let a = entailment(Support::Supported, Quantification::ForAll);
    let b = entailment(Support::Refuted, Quantification::ForAll);
    assert_eq!(
        pair(binding, a, a).correspondence(),
        Correspondence::SameReportedOutcome
    );
    assert_eq!(
        pair(binding, a, b).reprojection().unwrap().cause(),
        ReprojectionCause::ReportedDisagreement
    );
    assert_eq!(
        pair(binding, b, a).reprojection().unwrap().cause(),
        ReprojectionCause::ReportedDisagreement
    );
}
