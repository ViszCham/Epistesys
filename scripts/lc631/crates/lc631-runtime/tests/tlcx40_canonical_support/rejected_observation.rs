//! Actual third-model parser rejection retains evidence without selecting it.
use super::support::*;
use lc631_runtime::{canonical_solve::*, explicit_formal::FormalLowering};
use lc631_solvers::{
    protocol::{
        sexpr::{self, SExprError, SExprLimits},
        SolverProtocol,
    },
    registry::{SolverRegistration, SolverRegistry},
    smt::native_model::{NativeModelError, NativeModelRejection},
};

const NEGATED_OBJECTIVE: &str = "(set-logic QF_LIA)\n(declare-const x Int)\n(declare-const y Int)\n(assert (<= 0 x))\n(assert (<= x 10))\n(assert (= y (- x)))\n(maximize x)\n(check-sat)\n";

fn registry() -> SolverRegistry {
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
#[ignore = "opt-in actual initial 13-node payload accepted; third 15-node payload rejected"]
fn tlcx40_objective_step_native_third_syntax_rejection_keeps_actual_observation() {
    let f = Fixture::new(NEGATED_OBJECTIVE);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("actual objective")
    };
    let request = CanonicalRequest::formal(
        &problem,
        f.context.state(),
        GoalPolicy::DeclaredSourceObjective { revision: d(2) },
    )
    .unwrap()
    .with_objective_step(OneStepBudget::checked(3, 15_000).unwrap())
    .unwrap();
    let registry = registry();
    let cwd = std::env::current_dir().unwrap();
    let mut services = services(&f, &view, Some(&registry), &cwd);
    services.parser = SExprLimits::checked(65536, 13, 64).unwrap();
    let output = solve_with_services(request, services).unwrap();
    let initial = output
        .native_model()
        .expect("initial parser must actually pass");
    assert_eq!(
        initial.model().integer(problem.goal().unwrap().symbol()),
        Some(0)
    );
    let wide = SExprLimits::checked(65536, 64, 64).unwrap();
    assert_eq!(
        sexpr::parse(initial.observation().payload().unwrap(), wide)
            .unwrap()
            .node_count(),
        13,
        "pinned initial payload shape changed; do not weaken parser cut"
    );
    let step = output.objective_step().unwrap();
    assert_eq!(step.state(), ObjectiveStepState::ImprovedModelRejected);
    assert_eq!(step.initial_value(), Some(0));
    assert_eq!(step.selected_value(), Some(0));
    assert_eq!(step.attempts_requested(), 3);
    assert!(step.probe_report().is_some());
    let Some(NativeModelError::Rejected {
        reason,
        observation,
    }) = step.native_error()
    else {
        panic!("actual third model must be rejected with its owned observation")
    };
    assert_eq!(
        reason,
        &NativeModelRejection::Syntax(SExprError::NodeLimitExceeded)
    );
    assert_eq!(
        sexpr::parse(observation.payload().unwrap(), wide)
            .unwrap()
            .node_count(),
        15,
        "pinned strict model negative-value payload shape changed"
    );
    assert_eq!(step.third_model_report(), Some(observation.report()));
    assert_eq!(step.third_model_stdout(), Some(observation.stdout()));
    assert!(step.third_observation_binding().is_some());
    assert_eq!(
        step.third_binding_at_render(),
        step.third_observation_binding()
    );
    assert_eq!(step.improved_stored_results(), 1);
    assert_eq!(step.selected_report(), Some(initial.observation().report()));
    assert_eq!(step.selected_stdout(), initial.observation().stdout());
    assert!(!step.wall_cutoff_is_atomic());
    assert_eq!(output.selection_summary().unwrap().recommendation, None);
    assert!(output.project_current(&f.broker, &view).is_ok());
    f.cancellation.cancel();
    assert!(output.project_current(&f.broker, &view).is_err());
    assert_eq!(step.third_model_stdout(), Some(observation.stdout()));
}

#[test]
#[ignore = "opt-in actual successful third model exposes the same retained binding surface"]
fn tlcx40_objective_step_native_third_success_has_actual_report_stdout_and_store() {
    let f = Fixture::new(NEGATED_OBJECTIVE);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("actual objective")
    };
    let request = CanonicalRequest::formal(
        &problem,
        f.context.state(),
        GoalPolicy::DeclaredSourceObjective { revision: d(2) },
    )
    .unwrap()
    .with_objective_step(OneStepBudget::checked(3, 15_000).unwrap())
    .unwrap();
    let registry = registry();
    let cwd = std::env::current_dir().unwrap();
    let output = solve_with_services(request, services(&f, &view, Some(&registry), &cwd)).unwrap();
    assert_eq!(
        output
            .native_model()
            .unwrap()
            .model()
            .integer(problem.goal().unwrap().symbol()),
        Some(0)
    );
    let step = output.objective_step().unwrap();
    assert_eq!(step.state(), ObjectiveStepState::ImprovedModelRechecked);
    assert!(step.selected_value().unwrap() > 0);
    assert_eq!(step.third_model_report(), step.selected_report());
    assert_eq!(step.third_model_stdout(), Some(step.selected_stdout()));
    assert!(step.third_observation_binding().is_some());
    assert_eq!(
        step.third_binding_at_render(),
        step.third_observation_binding()
    );
    assert_eq!(step.improved_stored_results(), 1);
    assert!(!step.wall_cutoff_is_atomic());
    assert_eq!(output.selection_summary().unwrap().recommendation, None);
    assert!(output.project_current(&f.broker, &view).is_ok());
}
