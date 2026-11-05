//! Actual-producer regression: pending comparison must retain both feasible
//! native witnesses, rather than silently replacing the incumbent with one row.
//! This does not ask missing TL/map-back to pass or manufacture creative gains.
#[path = "tlcx40_canonical_support/mod.rs"]
#[allow(dead_code)] // Shared actual fixtures also contain other integration cases.
mod support;
use lc631_runtime::{canonical_solve::*, explicit_formal::FormalLowering};
use support::*;

const MAXIMIZE: &str = "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (<= 0 x))\n(assert (<= x 10))\n(maximize x)\n(check-sat)\n";

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
#[ignore = "actual pinned Z3; fail-first incumbent/selected alternative retention"]
fn tlcx40_actual_improved_witness_keeps_feasible_incumbent_when_comparison_is_pending() {
    let f = Fixture::new(MAXIMIZE);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("actual formal objective")
    };
    let request = CanonicalRequest::formal(
        &problem,
        f.context.state(),
        GoalPolicy::DeclaredSourceObjective {
            revision: f.context.state().scope().goal(),
        },
    )
    .unwrap()
    .with_objective_step(OneStepBudget::checked(3, 15_000).unwrap())
    .unwrap();
    let registry = dual_registry();
    let cwd = std::env::current_dir().unwrap();
    let output = solve_with_services(request, services(&f, &view, Some(&registry), &cwd)).unwrap();
    let initial = output.native_model().expect("real initial checked model");
    let step = output.objective_step().expect("explicit actual step");
    let goal = problem.goal().unwrap();
    let initial_value = i64::try_from(initial.model().integer(goal.symbol()).unwrap()).unwrap();
    assert_eq!(initial_value, 0, "pinned fixture observation changed");
    assert_eq!(step.state(), ObjectiveStepState::ImprovedModelRechecked);
    let selected = step.selected_value().unwrap();
    assert!(selected > initial_value && selected <= goal.upper());
    assert_eq!(step.improved_stored_results(), 1);
    assert_eq!(initial.context(), problem.context());
    assert!(std::ptr::eq(initial.model().query(), problem.query()));
    assert!(initial.observation().stdout().starts_with(b"sat"));
    assert!(step.selected_stdout().starts_with(b"sat"));
    assert!(output.project_current(&f.broker, &view).is_ok());
    let selection = output.selection_summary().unwrap();
    assert_eq!(
        selection.recommendation, None,
        "missing required TL stays pending"
    );
    assert!(output
        .historical_text()
        .contains("NoScalar/MissingEvaluator"));
    // This is the regression boundary. Both different actual witnesses exist,
    // but total TL/map-back have not established certified dominance. The
    // incumbent must remain an alternative in the same consumed decision.
    assert_eq!(
        selection.candidate_count, 2,
        "actual feasible incumbent vanished from candidate set"
    );
    assert_eq!(
        output
            .historical_text()
            .matches("Alternative candidate:")
            .count(),
        2
    );
}

#[test]
#[ignore = "actual pinned Z3; no second witness may be fabricated when its producer is cut"]
fn tlcx40_cut_improvement_producer_does_not_pad_the_candidate_set() {
    let f = Fixture::new(MAXIMIZE);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("actual formal objective")
    };
    let base = CanonicalRequest::formal(
        &problem,
        f.context.state(),
        GoalPolicy::DeclaredSourceObjective {
            revision: f.context.state().scope().goal(),
        },
    )
    .unwrap();
    let registry = native_registry(); // Explicit model WireV2, no status capability.
    let cwd = std::env::current_dir().unwrap();
    for enabled in [false, true] {
        let request = if enabled {
            base.with_objective_step(OneStepBudget::checked(3, 15_000).unwrap())
                .unwrap()
        } else {
            base
        };
        let output =
            solve_with_services(request, services(&f, &view, Some(&registry), &cwd)).unwrap();
        assert!(output.native_model().is_some());
        if enabled {
            assert_eq!(
                output.objective_step().unwrap().state(),
                ObjectiveStepState::UnavailableStatusCapability
            );
            assert_eq!(
                output.objective_step().unwrap().improved_stored_results(),
                0
            );
        } else {
            assert!(output.objective_step().is_none());
        }
        assert_eq!(output.selection_summary().unwrap().candidate_count, 1);
        assert_eq!(output.selection_summary().unwrap().recommendation, None);
    }
}
