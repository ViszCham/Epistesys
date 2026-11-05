//! Real canonical ownership without a fabricated TL tuple or default coefficients.
#[path = "tlcx40_canonical_support/mod.rs"]
#[allow(dead_code)] // Shared native-positive helpers are outside this missing-input target.
mod support;
use lc631_runtime::{
    canonical_original_pending as original, canonical_solve::*, explicit_formal::FormalLowering,
};
use support::*;

#[test]
fn tlcx22_actual_canonical_missing_backend_preserves_original_score_gaps() {
    let fixture = Fixture::new(SAT);
    let view = fixture.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&fixture, &view) else {
        panic!("typed fixture")
    };
    let request =
        CanonicalRequest::formal(&problem, fixture.context.state(), GoalPolicy::NoGoal).unwrap();
    let cwd = std::env::current_dir().unwrap();
    let decision = solve_with_services(request, services(&fixture, &view, None, &cwd)).unwrap();
    let pending = original::inspect_current(&decision, &fixture.broker, &view).unwrap();
    assert_eq!(pending.native_kind(), NativeEvidenceKind::Unavailable);
    assert!(pending.profile_missing() && pending.total_tl_owner_missing());
    assert!(pending.scalar().is_none() && pending.recommendation().is_none());
    assert!(!pending.formula_evaluated());
    assert!(pending.english_line().contains("NoScalar"));
    let foreign = Fixture::new(SAT);
    assert!(
        original::inspect_current(&decision, &foreign.broker, &foreign.reservation.lease())
            .is_err()
    );
    fixture.cancellation.cancel();
    assert!(original::inspect_current(&decision, &fixture.broker, &view).is_err());
}

#[test]
fn tlcx22_actual_clarify_is_not_a_measured_original_score() {
    let fixture = Fixture::new("arbitrary prose is not a finite tuple");
    let view = fixture.reservation.lease();
    let error = lc631_runtime::explicit_formal::lower_explicit_formal(
        &fixture.source,
        fixture.context.state().revision(),
        fixture.context.state().scope(),
        &view,
    )
    .err()
    .unwrap();
    let request = CanonicalRequest::clarify(&error, fixture.context.state()).unwrap();
    let cwd = std::env::current_dir().unwrap();
    let decision = solve_with_services(request, services(&fixture, &view, None, &cwd)).unwrap();
    let pending = original::inspect_current(&decision, &fixture.broker, &view).unwrap();
    assert_eq!(pending.native_kind(), NativeEvidenceKind::NoExecutableInput);
    assert!(pending.profile_missing() && pending.total_tl_owner_missing());
    assert!(pending.scalar().is_none());
    assert!(pending.recommendation().is_none());
}

#[test]
#[ignore = "opt-in actual native witness; requires the registered Z3 binary"]
fn tlcx22_actual_native_model_does_not_manufacture_original_profile_or_total_tl() {
    let fixture = Fixture::new(SAT);
    let view = fixture.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&fixture, &view) else {
        panic!("typed fixture")
    };
    let request = CanonicalRequest::formal(
        &problem,
        fixture.context.state(),
        GoalPolicy::SatisfyingModel {
            revision: fixture.context.state().scope().goal(),
        },
    )
    .unwrap();
    let registry = native_registry();
    let cwd = std::env::current_dir().unwrap();
    let decision =
        solve_with_services(request, services(&fixture, &view, Some(&registry), &cwd)).unwrap();
    assert!(decision.native_model().is_some());
    let pending = original::inspect_current(&decision, &fixture.broker, &view).unwrap();
    assert_eq!(pending.native_kind(), NativeEvidenceKind::ModelRechecked);
    assert_eq!(pending.canonical_candidate_count(), 1);
    assert!(pending.boolean_measurement_state().is_none());
    assert!(pending.profile_missing() && pending.total_tl_owner_missing());
    assert!(pending.scalar().is_none() && pending.recommendation().is_none());
    assert!(!pending.formula_evaluated());
    fixture.cancellation.cancel();
    assert!(original::inspect_current(&decision, &fixture.broker, &view).is_err());
}
