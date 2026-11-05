//! Native rejection, postprocess failure and cancellation ownership boundaries.
use super::support::*;
use lc631_runtime::{canonical_solve::*, explicit_formal::FormalLowering};
use lc631_solvers::{protocol::sexpr::SExprLimits, smt::native_model::NativeModelError};
use lc631_substrate::resource::{ResourceAmount, ResourceKind, ResourceRequest};

#[test]
#[ignore = "opt-in actual native failure retains its sole payload"]
fn tlcx40_canonical_native_model_rejection_retains_owned_bytes() {
    let f = Fixture::new(SAT);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("formal fixture")
    };
    let request =
        CanonicalRequest::formal(&problem, f.context.state(), GoalPolicy::NoGoal).unwrap();
    let registry = native_registry();
    let cwd = std::env::current_dir().unwrap();
    let mut configured = services(&f, &view, Some(&registry), &cwd);
    configured.parser = SExprLimits::checked(65536, 4, 4).unwrap();
    let result = solve_with_services(request, configured).unwrap();
    assert_eq!(result.status(), CanonicalStatus::NativeFailure);
    assert_eq!(result.stored_results(), 1);
    let Some(NativeModelError::Rejected { observation, .. }) = result.native_error() else {
        panic!("actual parser rejection retained")
    };
    assert!(observation.stdout().starts_with(b"sat"));
    assert!(observation.payload().is_some());
    assert!(result.native_model().is_none());
    assert!(result.goal_observation().is_none());
}

#[test]
#[ignore = "opt-in native postprocess failure must retain actual evidence"]
fn tlcx40_canonical_native_render_failure_keeps_evidence_and_store() {
    let f = Fixture::new(SAT);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("formal fixture")
    };
    let request =
        CanonicalRequest::formal(&problem, f.context.state(), GoalPolicy::NoGoal).unwrap();
    let registry = native_registry();
    let cwd = std::env::current_dir().unwrap();
    let mut configured = services(&f, &view, Some(&registry), &cwd);
    configured.budget = DecisionBudget::checked(1).unwrap();
    match solve_with_services(request, configured) {
        Err(CanonicalError::Postprocess(failure)) => {
            assert!(failure.check_retained_resources().is_ok());
            let evidence = failure.evidence();
            assert_eq!(evidence.kind(), NativeEvidenceKind::ModelRechecked);
            assert_eq!(evidence.stored_results(), 1);
            assert!(evidence
                .native_model()
                .unwrap()
                .observation()
                .stdout()
                .starts_with(b"sat"));
            let amount = view.amount(ResourceKind::Output).unwrap();
            let request = || {
                ResourceRequest::new(
                    ResourceKind::Output,
                    ResourceAmount::checked(amount).unwrap(),
                )
            };
            assert!(view.claim_resources(&[request()]).is_err());
            drop(failure);
            let _released = view
                .claim_resources(&[request()])
                .expect("private failure drops native backing before claim refund");
        }
        other => panic!(
            "post-native render failure must retain evidence, got error: {:?}",
            other.err()
        ),
    };
}

#[test]
#[ignore = "opt-in native current projection cannot be restored by historical text"]
fn tlcx40_canonical_native_cancelled_output_remains_historical() {
    let f = Fixture::new(SAT);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("formal fixture")
    };
    let request =
        CanonicalRequest::formal(&problem, f.context.state(), GoalPolicy::NoGoal).unwrap();
    let registry = native_registry();
    let cwd = std::env::current_dir().unwrap();
    let result = solve_with_services(request, services(&f, &view, Some(&registry), &cwd)).unwrap();
    assert_eq!(result.status(), CanonicalStatus::ConditionalModel);
    assert!(result.goal_observation().is_none());
    f.cancellation.cancel();
    assert!(result.project_current(&f.broker, &view).is_err());
    assert!(!result.historical_text().is_empty());
    assert!(result.native_model().is_some());
}
