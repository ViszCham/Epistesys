//! Negative-first contracts for one explicit strict-improvement step.
//! This module is unregistered until the production/test integration boundary.
use super::support::*;
use lc631_core::measurement_numeric::RationalInterval;
use lc631_runtime::{canonical_solve::*, explicit_formal::FormalLowering};
use lc631_solvers::{
    protocol::SolverProtocol,
    registry::{SolverRegistration, SolverRegistry},
};

const MAXIMIZE: &str = "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (<= 0 x))\n(assert (<= x 10))\n(maximize x)\n(check-sat)\n";
const ZERO_WIDTH: &str = "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (<= 0 x))\n(assert (<= x 0))\n(maximize x)\n(check-sat)\n";

fn dual_registry() -> SolverRegistry {
    let models = native_registry();
    let entry = &models.entries()[0];
    let status = SolverRegistration::checked(
        entry.id(),
        entry.profile(),
        SolverProtocol::SmtLibStatusV1,
        entry.executable(),
        entry.version(),
        *entry.binary_digest(),
        entry.fixed_arguments().to_vec(),
        entry.caps(),
    )
    .unwrap();
    SolverRegistry::checked(vec![entry.clone(), status]).unwrap()
}

#[test]
fn tlcx40_objective_step_budget_requires_three_attempts_and_bounded_wall_time() {
    for attempts in [0, 1, 2, 4, u8::MAX] {
        assert!(OneStepBudget::checked(attempts, 10_000).is_err());
    }
    for wall in [0, 60_001, u64::MAX] {
        assert!(OneStepBudget::checked(3, wall).is_err());
    }
    assert!(OneStepBudget::checked(3, 1).is_ok());
    assert!(OneStepBudget::checked(3, 60_000).is_ok());
}

#[test]
fn tlcx40_objective_step_requires_actual_source_goal_and_declared_objective_policy() {
    let budget = OneStepBudget::checked(3, 10_000).unwrap();
    let f = Fixture::new(SAT);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("actual source parser must accept fixture")
    };
    for goal in [
        GoalPolicy::NoGoal,
        GoalPolicy::SatisfyingModel { revision: d(2) },
    ] {
        let request = CanonicalRequest::formal(&problem, f.context.state(), goal).unwrap();
        assert!(request.with_objective_step(budget).is_err());
    }

    let f = Fixture::new(OBJECTIVE);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("actual source parser must preserve objective")
    };
    let no_policy =
        CanonicalRequest::formal(&problem, f.context.state(), GoalPolicy::NoGoal).unwrap();
    assert!(no_policy.with_objective_step(budget).is_err());
    let declared = CanonicalRequest::formal(
        &problem,
        f.context.state(),
        GoalPolicy::DeclaredSourceObjective { revision: d(2) },
    )
    .unwrap();
    assert!(declared.with_objective_step(budget).is_ok());
}

#[test]
fn tlcx40_objective_step_missing_registry_retains_explicit_missing_model() {
    let f = Fixture::new(MAXIMIZE);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("objective")
    };
    let request = CanonicalRequest::formal(
        &problem,
        f.context.state(),
        GoalPolicy::DeclaredSourceObjective { revision: d(2) },
    )
    .unwrap()
    .with_objective_step(OneStepBudget::checked(3, 15_000).unwrap())
    .unwrap();
    let cwd = std::env::current_dir().unwrap();
    let output = solve_with_services(request, services(&f, &view, None, &cwd)).unwrap();
    let step = output.objective_step().unwrap();
    assert_eq!(step.state(), ObjectiveStepState::MissingNativeModel);
    assert_eq!(
        step.normalization(),
        ObjectiveNormalization::MissingSelectedValue
    );
    assert_eq!(step.initial_value(), None);
    assert_eq!(step.selected_value(), None);
    assert!(step.goal_quality().is_none());
    assert!(step.probe_report().is_none());
    assert!(step.selected_report().is_none());
    assert!(step.selected_stdout().is_empty());
    assert!(output.native_model().is_none());
}

#[test]
#[ignore = "opt-in actual model must survive missing explicit status capability"]
fn tlcx40_objective_step_native_model_only_registry_is_not_improvement() {
    let f = Fixture::new(MAXIMIZE);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("objective")
    };
    let request = CanonicalRequest::formal(
        &problem,
        f.context.state(),
        GoalPolicy::DeclaredSourceObjective { revision: d(2) },
    )
    .unwrap()
    .with_objective_step(OneStepBudget::checked(3, 15_000).unwrap())
    .unwrap();
    let registry = native_registry();
    let cwd = std::env::current_dir().unwrap();
    let output = solve_with_services(request, services(&f, &view, Some(&registry), &cwd)).unwrap();
    let initial = i64::try_from(
        output
            .native_model()
            .unwrap()
            .model()
            .integer(problem.goal().unwrap().symbol())
            .unwrap(),
    )
    .unwrap();
    let step = output.objective_step().unwrap();
    assert_eq!(
        step.state(),
        ObjectiveStepState::UnavailableStatusCapability
    );
    assert_eq!(step.initial_value(), Some(initial));
    assert!(step.probe_report().is_none());
    assert!(step.selected_report().is_some());
    assert_eq!(step.attempts_requested(), 1);
    assert_eq!(
        step.selected_stdout(),
        output.native_model().unwrap().observation().stdout()
    );
    assert!(output.stored_results() >= 1);
    assert!(output.project_current(&f.broker, &view).is_ok());
}

#[test]
#[ignore = "opt-in pinned actual initial model and rechecked strict improvement"]
fn tlcx40_objective_step_native_maximize_consumes_actual_strict_probe() {
    let f = Fixture::new(MAXIMIZE);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("objective")
    };
    let request = CanonicalRequest::formal(
        &problem,
        f.context.state(),
        GoalPolicy::DeclaredSourceObjective { revision: d(2) },
    )
    .unwrap()
    .with_objective_step(OneStepBudget::checked(3, 15_000).unwrap())
    .unwrap();
    let registry = dual_registry();
    let cwd = std::env::current_dir().unwrap();
    let output = solve_with_services(request, services(&f, &view, Some(&registry), &cwd)).unwrap();
    let actual_initial = i64::try_from(
        output
            .native_model()
            .unwrap()
            .model()
            .integer(problem.goal().unwrap().symbol())
            .unwrap(),
    )
    .unwrap();
    // Explicit pinned-backend observation; this is not inferred from [0,10].
    assert_eq!(actual_initial, 0, "pinned initial model behavior changed");
    let step = output.objective_step().unwrap();
    assert_eq!(step.initial_value(), Some(actual_initial));
    assert_eq!(step.state(), ObjectiveStepState::ImprovedModelRechecked);
    assert_eq!(step.normalization(), ObjectiveNormalization::PositiveWidth);
    assert_eq!((step.lower(), step.upper()), (0, 10));
    assert_eq!(step.direction(), problem.goal().unwrap().direction());
    let selected = step.selected_value().unwrap();
    assert!(selected > actual_initial && selected <= 10);
    assert_eq!(
        step.goal_quality(),
        Some(&RationalInterval::exact_ratio(selected, 10).unwrap())
    );
    assert_eq!(step.gap_upper_bound(), Some((10 - selected) as u128));
    assert_eq!(step.attempts_requested(), 3);
    assert!(step.probe_report().is_some());
    assert!(step.selected_report().is_some());
    assert!(!step.probe_query_bytes().is_empty());
    assert!(!step.selected_query_bytes().is_empty());
    assert!(step.selected_stdout().starts_with(b"sat"));
    assert!(output.project_current(&f.broker, &view).is_ok());
    f.cancellation.cancel();
    assert!(output.project_current(&f.broker, &view).is_err());
    assert_eq!(step.selected_value(), Some(selected));
    assert_eq!(output.selection_summary().unwrap().recommendation, None);
}

#[test]
#[ignore = "opt-in reported no strict improvement with undefined zero-width quality"]
fn tlcx40_objective_step_native_boundary_unsat_is_not_scalar_or_global_proof() {
    let f = Fixture::new(ZERO_WIDTH);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("objective")
    };
    let request = CanonicalRequest::formal(
        &problem,
        f.context.state(),
        GoalPolicy::DeclaredSourceObjective { revision: d(2) },
    )
    .unwrap()
    .with_objective_step(OneStepBudget::checked(3, 15_000).unwrap())
    .unwrap();
    let registry = dual_registry();
    let cwd = std::env::current_dir().unwrap();
    let output = solve_with_services(request, services(&f, &view, Some(&registry), &cwd)).unwrap();
    let step = output.objective_step().unwrap();
    assert_eq!(
        step.state(),
        ObjectiveStepState::ReportedNoStrictImprovement
    );
    assert_eq!(
        step.normalization(),
        ObjectiveNormalization::UndefinedZeroWidth
    );
    assert_eq!(step.initial_value(), Some(0));
    assert_eq!(step.selected_value(), Some(0));
    assert_eq!((step.lower(), step.upper()), (0, 0));
    assert!(step.goal_quality().is_none());
    assert!(step.probe_report().is_some());
    // The selected report remains the INITIAL actual model; it does not imply
    // that a third native execution occurred after the reported UNSAT probe.
    assert!(step.selected_report().is_some());
    assert_eq!(step.attempts_requested(), 2);
    assert_eq!(
        step.selected_stdout(),
        output.native_model().unwrap().observation().stdout()
    );
    assert!(output.native_model().is_some());
    assert_eq!(output.selection_summary().unwrap().recommendation, None);
}
