//! Independent canonical-services tests. Native positives require pinned Z3.
#[path = "tlcx40_canonical_support/boolean_measurement.rs"]
mod boolean_measurement;
#[path = "tlcx40_canonical_support/lifecycle.rs"]
mod lifecycle;
#[path = "tlcx40_canonical_support/objective_step.rs"]
mod objective_step;
#[path = "tlcx40_canonical_support/rejected_observation.rs"]
mod rejected_observation;
#[path = "tlcx40_canonical_support/mod.rs"]
mod support;
use lc631_runtime::{
    canonical_solve::*,
    explicit_formal::{lower_explicit_formal, FormalLowering},
    facet_ledger::{Facet, GapReason},
};
use lc631_solvers::typed_query::IntSymbolId;
use lc631_substrate::resource::{ResourceAmount, ResourceKind, ResourceRequest};
use support::*;

#[test]
fn tlcx40_canonical_missing_backend_has_no_model_goal_or_stored_result() {
    let f = Fixture::new(SAT);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("supported formal fixture")
    };
    let request = CanonicalRequest::formal(
        &problem,
        f.context.state(),
        GoalPolicy::SatisfyingModel { revision: d(2) },
    )
    .unwrap();
    let cwd = std::env::current_dir().unwrap();
    let result = solve_with_services(request, services(&f, &view, None, &cwd)).unwrap();
    assert_eq!(result.status(), CanonicalStatus::Unavailable);
    assert_eq!(
        result.backend_unavailable(),
        Some(&BackendUnavailable::MissingRegistry)
    );
    assert!(result.native_model().is_none());
    assert!(result.goal_observation().is_none());
    assert_eq!(result.stored_results(), 0);
    assert!(result.selection_summary().is_none());
    assert_eq!(
        result.no_recommendation_reason(),
        NoRecommendationReason::MissingBackend
    );
    assert!(!result.historical_text().is_empty());
}

#[test]
fn tlcx40_canonical_unsupported_and_actual_parser_error_clarify_without_dispatch() {
    let f = Fixture::new(UNSUPPORTED);
    let view = f.reservation.lease();
    let FormalLowering::Unsupported(report) = lower(&f, &view) else {
        panic!("unsupported command must remain a residual")
    };
    let request = CanonicalRequest::unsupported(&report, f.context.state()).unwrap();
    let cwd = std::env::current_dir().unwrap();
    let result = solve_with_services(request, services(&f, &view, None, &cwd)).unwrap();
    assert_eq!(result.status(), CanonicalStatus::Clarify);
    assert_eq!(result.stored_results(), 0);
    assert!(result.native_model().is_none());
    assert!(result.goal_observation().is_none());
    let prose = Fixture::new("Explain why the sky is blue.");
    let prose_view = prose.reservation.lease();
    let error = lower_explicit_formal(
        &prose.source,
        prose.context.state().revision(),
        prose.context.state().scope(),
        &prose_view,
    )
    .err()
    .expect("arbitrary prose is not executable SMT");
    let request = CanonicalRequest::clarify(&error, prose.context.state()).unwrap();
    let result = solve_with_services(request, services(&prose, &prose_view, None, &cwd)).unwrap();
    assert_eq!(result.status(), CanonicalStatus::Clarify);
    assert_eq!(result.stored_results(), 0);
}

#[test]
fn tlcx40_canonical_context_and_explicit_goal_conflicts_fail_before_backend() {
    let f = Fixture::new(SAT);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("formal fixture")
    };
    let other = Fixture::new(OBJECTIVE);
    assert!(matches!(
        CanonicalRequest::formal(&problem, other.context.state(), GoalPolicy::NoGoal),
        Err(AdmissionError::ContextMismatch)
    ));
    assert!(matches!(
        CanonicalRequest::formal(
            &problem,
            f.context.state(),
            GoalPolicy::SatisfyingModel { revision: d(99) }
        ),
        Err(AdmissionError::GoalRevision)
    ));
    let objective_view = other.reservation.lease();
    let FormalLowering::Checked(objective) = lower(&other, &objective_view) else {
        panic!("finite source objective")
    };
    assert!(objective.goal().is_some());
    assert!(matches!(
        CanonicalRequest::formal(
            &objective,
            other.context.state(),
            GoalPolicy::SatisfyingModel { revision: d(2) }
        ),
        Err(AdmissionError::GoalConflict)
    ));
    assert!(CanonicalRequest::formal(
        &objective,
        other.context.state(),
        GoalPolicy::DeclaredSourceObjective { revision: d(2) }
    )
    .is_ok());
}

#[test]
fn tlcx40_canonical_foreign_broker_cancel_and_quota_are_preflight_failures() {
    let f = Fixture::new(SAT);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("formal fixture")
    };
    let request =
        CanonicalRequest::formal(&problem, f.context.state(), GoalPolicy::NoGoal).unwrap();
    let cwd = std::env::current_dir().unwrap();
    let foreign = Fixture::new(SAT);
    let mut relative = services(&f, &view, None, &cwd);
    relative.working_directory = std::path::Path::new(".");
    assert!(matches!(
        solve_with_services(request, relative),
        Err(CanonicalError::Admission(
            AdmissionError::RelativeWorkingDirectory
        ))
    ));
    let mut wrong = services(&f, &view, None, &cwd);
    wrong.broker = &foreign.broker;
    assert!(matches!(
        solve_with_services(request, wrong),
        Err(CanonicalError::Admission(AdmissionError::Resource(_)))
    ));
    let hog = view.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(127 * 1024 * 1024 - 1).unwrap(),
    )]);
    // The parser owns 2MiB, so a request for nearly the entire reservation cannot
    // succeed. No model/backend is used to make this accounting assertion.
    assert!(hog.is_err());
    let remaining_hog = view
        .claim_resources(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(126 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    assert!(matches!(
        solve_with_services(request, services(&f, &view, None, &cwd)),
        Err(CanonicalError::Admission(AdmissionError::Resource(_)))
    ));
    drop(remaining_hog);
    let output_hog = view
        .claim_resources(&[ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(view.amount(ResourceKind::Output).unwrap()).unwrap(),
        )])
        .unwrap();
    assert!(matches!(
        solve_with_services(request, services(&f, &view, None, &cwd)),
        Err(CanonicalError::Admission(AdmissionError::Resource(_)))
    ));
    drop(output_hog);
    f.cancellation.cancel();
    assert!(matches!(
        solve_with_services(request, services(&f, &view, None, &cwd)),
        Err(CanonicalError::Admission(AdmissionError::Resource(_)))
    ));
}

#[test]
fn tlcx40_canonical_budget_and_required_gpu_do_not_silently_succeed() {
    assert!(DecisionBudget::checked(0).is_err());
    assert!(DecisionBudget::checked(65537).is_err());
    let f = Fixture::new(SAT);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("formal fixture")
    };
    let request =
        CanonicalRequest::formal(&problem, f.context.state(), GoalPolicy::NoGoal).unwrap();
    let cwd = std::env::current_dir().unwrap();
    let mut required = services(&f, &view, None, &cwd);
    required.optional_gpu = GpuPolicy::Require;
    let result = solve_with_services(request, required).unwrap();
    assert_eq!(
        result.backend_unavailable(),
        Some(&BackendUnavailable::GpuRequired)
    );
    assert_ne!(result.status(), CanonicalStatus::ConditionalModel);
    assert!(result.goal_observation().is_none());
    assert_eq!(result.stored_results(), 0);
}

#[test]
#[ignore = "opt-in actual pinned native Z3 canonical services model"]
fn tlcx40_canonical_native_actual_model_and_missing_result_cut() {
    let f = Fixture::new(SAT);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("formal fixture")
    };
    let request = CanonicalRequest::formal(
        &problem,
        f.context.state(),
        GoalPolicy::SatisfyingModel { revision: d(2) },
    )
    .unwrap();
    let registry = native_registry();
    let cwd = std::env::current_dir().unwrap();
    let result = solve_with_services(request, services(&f, &view, Some(&registry), &cwd)).unwrap();
    assert_eq!(result.status(), CanonicalStatus::ConditionalModel);
    assert_eq!(result.stored_results(), 1);
    let model = result
        .native_model()
        .expect("actual native checked model retained");
    assert!(std::ptr::eq(model.model().query(), problem.query()));
    assert_eq!(model.context(), f.context.state().revision());
    assert_eq!(model.model().integer(IntSymbolId::new(0)), Some(-3));
    assert_eq!(
        model.model().assertions_checked(),
        problem.query().assertions().len()
    );
    assert!(model.observation().invocation().is_some());
    assert!(model.observation().stdout().starts_with(b"sat"));
    assert!(result.goal_observation().is_some());
    let summary = result
        .selection_summary()
        .expect("actual selector was consumed");
    assert_eq!(summary.candidate_count, 1);
    assert_eq!(summary.recommendation, None);
    assert_eq!(
        result.no_recommendation_reason(),
        NoRecommendationReason::MissingTranslationEvaluator
    );
    assert_eq!(
        result.goal_observation().unwrap(),
        &lc631_core::measurement_numeric::RationalInterval::exact_ratio(1, 1).unwrap()
    );
    assert!(!result.facets().entries().is_empty());
    assert!(result.facets().entries().iter().any(|entry| matches!(entry.facet, Facet::Uncertainty(gap) if gap.reason == GapReason::Unverified)));
    assert!(result
        .facets()
        .entries()
        .iter()
        .all(|entry| matches!(entry.facet, Facet::Uncertainty(_))));
    assert!(result.project_current(&f.broker, &view).is_ok());
    assert!(result.matches_exact(f.source.text(), result.historical_text().as_bytes()));
    assert!(!result.matches_exact(f.source.text(), b"changed output"));
    // Cut the actual native-result producer, retaining identical parsed source,
    // context, goal policy and budgets. No injected report substitutes for it.
    let cut = solve_with_services(request, services(&f, &view, None, &cwd)).unwrap();
    assert_eq!(cut.status(), CanonicalStatus::Unavailable);
    assert!(cut.native_model().is_none());
    assert!(cut.goal_observation().is_none());
    assert_eq!(cut.stored_results(), 0);
    let output_quota = view.amount(ResourceKind::Output).unwrap();
    let whole_output = || {
        ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(output_quota).unwrap(),
        )
    };
    assert!(
        view.claim_resources(&[whole_output()]).is_err(),
        "native/output backing must retain its claims"
    );
    drop(cut);
    drop(result);
    let _released_output = view
        .claim_resources(&[whole_output()])
        .expect("all canonical/native output backing dropped before refund");
}

#[test]
#[ignore = "opt-in actual native source objective must remain explicitly deferred"]
fn tlcx40_canonical_native_source_objective_is_not_replaced_by_satisfy_goal() {
    let f = Fixture::new(OBJECTIVE);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("finite source objective")
    };
    let goal = problem.goal().expect("actual parsed source objective");
    assert_eq!(
        goal.direction(),
        lc631_solvers::smt::ObjectiveDirection::Minimize
    );
    assert_eq!((goal.lower(), goal.upper()), (0, 10));
    let request = CanonicalRequest::formal(
        &problem,
        f.context.state(),
        GoalPolicy::DeclaredSourceObjective { revision: d(2) },
    )
    .unwrap();
    let registry = native_registry();
    let cwd = std::env::current_dir().unwrap();
    let result = solve_with_services(request, services(&f, &view, Some(&registry), &cwd)).unwrap();
    assert_eq!(result.status(), CanonicalStatus::ConditionalModel);
    assert_eq!(result.stored_results(), 1);
    let raw = result
        .native_model()
        .unwrap()
        .model()
        .integer(goal.symbol())
        .unwrap();
    assert!((0..=10).contains(&raw));
    assert!(result.goal_observation().is_none());
    assert_eq!(
        result.no_recommendation_reason(),
        NoRecommendationReason::DeferredSourceObjective
    );
    assert_eq!(result.selection_summary().unwrap().recommendation, None);
}

#[test]
#[ignore = "opt-in two actual native observations must not alias across canonical contexts"]
fn tlcx40_canonical_native_same_source_distinct_contexts_have_distinct_measurement_bindings() {
    let f = Fixture::new(SAT);
    let view = f.reservation.lease();
    let FormalLowering::Checked(first) = lower(&f, &view) else {
        panic!("formal fixture")
    };
    let other = lc631_core::context::OriginalContext::checked(
        lc631_core::context::SourceRevision::of(SAT),
        f.context.state().scope(),
        lc631_core::AuthorityRevision(2),
        f.context.state().premises().to_vec(),
        f.context.limits(),
    )
    .unwrap();
    assert_eq!(
        other.state().revision().source(),
        f.context.state().revision().source()
    );
    assert_ne!(other.state().revision(), f.context.state().revision());
    let FormalLowering::Checked(second) = lower_explicit_formal(
        &f.source,
        other.state().revision(),
        other.state().scope(),
        &view,
    )
    .unwrap() else {
        panic!("same source with distinct Given structural context")
    };
    assert!(matches!(
        CanonicalRequest::formal(&first, other.state(), GoalPolicy::NoGoal),
        Err(AdmissionError::ContextMismatch)
    ));
    let left = CanonicalRequest::formal(&first, f.context.state(), GoalPolicy::NoGoal).unwrap();
    let right = CanonicalRequest::formal(&second, other.state(), GoalPolicy::NoGoal).unwrap();
    let registry = native_registry();
    let cwd = std::env::current_dir().unwrap();
    let left = solve_with_services(left, services(&f, &view, Some(&registry), &cwd)).unwrap();
    let right = solve_with_services(right, services(&f, &view, Some(&registry), &cwd)).unwrap();
    assert_eq!(left.status(), CanonicalStatus::ConditionalModel);
    assert_eq!(right.status(), CanonicalStatus::ConditionalModel);
    assert_eq!(
        left.native_model().unwrap().observation().stdout(),
        right.native_model().unwrap().observation().stdout()
    );
    let a = &left.facets().transforms()[0];
    let b = &right.facets().transforms()[0];
    assert_ne!(a.source_context, b.source_context);
    assert_ne!(
        a.id, b.id,
        "identical native bytes cannot erase the actual context binding"
    );
}
