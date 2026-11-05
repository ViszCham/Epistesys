//! Independent model-space oracle cases; native valuation is never caller input.
use super::support::*;
use lc631_core::{
    context::{ContextScope, SourceRevision},
    measurement_numeric::RationalInterval as Interval,
};
use lc631_runtime::{
    canonical_solve::*,
    explicit_formal::{FormalLowering, FormalSymbol},
    facet_ledger::Facet,
};
use lc631_substrate::resource::{ResourceAmount, ResourceError, ResourceKind, ResourceRequest};

const UNIQUE: &str = "(set-logic QF_UF)\n(declare-const p Bool)\n(assert p)\n(check-sat)\n";
const TOP: &str =
    "(set-logic QF_UF)\n(declare-const p Bool)\n(assert (or p (not p)))\n(check-sat)\n";
const OR: &str = "(set-logic QF_UF)\n(declare-const p Bool)\n(declare-const q Bool)\n(assert (or p q))\n(check-sat)\n";

fn registered(text: &str) -> (Fixture, DeclaredEvaluation) {
    let base = ContextScope::declared(d(1), d(2), d(3), d(4));
    let descriptor =
        DeclaredEvaluation::finite_boolean(SourceRevision::of(text), base, GoalPolicy::NoGoal)
            .unwrap();
    (Fixture::with_scope(text, descriptor.scope()), descriptor)
}
fn exact(n: i64, denominator: i64) -> Interval {
    Interval::exact_ratio(n, denominator).unwrap()
}

#[test]
fn tlcx40_boolean_profile_must_match_source_scope_and_goal_before_execution() {
    let (f, descriptor) = registered(UNIQUE);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("Boolean fixture")
    };
    let request =
        CanonicalRequest::formal(&problem, f.context.state(), GoalPolicy::NoGoal).unwrap();
    assert!(request.with_evaluation(descriptor).is_ok());
    let base = ContextScope::declared(d(1), d(2), d(3), d(4));
    let wrong_source =
        DeclaredEvaluation::finite_boolean(SourceRevision::of(TOP), base, GoalPolicy::NoGoal)
            .unwrap();
    assert!(matches!(
        request.with_evaluation(wrong_source),
        Err(AdmissionError::ContextMismatch)
    ));
    let wrong_scope = DeclaredEvaluation::finite_boolean(
        SourceRevision::of(UNIQUE),
        ContextScope::declared(d(99), d(2), d(3), d(4)),
        GoalPolicy::NoGoal,
    )
    .unwrap();
    assert!(matches!(
        request.with_evaluation(wrong_scope),
        Err(AdmissionError::ContextMismatch)
    ));
    let with_goal = CanonicalRequest::formal(
        &problem,
        f.context.state(),
        GoalPolicy::SatisfyingModel { revision: d(2) },
    )
    .unwrap();
    assert!(matches!(
        with_goal.with_evaluation(descriptor),
        Err(AdmissionError::GoalConflict)
    ));
}

#[test]
fn tlcx40_boolean_missing_native_model_is_not_measured_zero_and_cancel_stays_current() {
    let (f, descriptor) = registered(UNIQUE);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("Boolean fixture")
    };
    let request = CanonicalRequest::formal(&problem, f.context.state(), GoalPolicy::NoGoal)
        .unwrap()
        .with_evaluation(descriptor)
        .unwrap();
    let cwd = std::env::current_dir().unwrap();
    let output = solve_with_services(request, services(&f, &view, None, &cwd)).unwrap();
    let receipt = output
        .boolean_measurement()
        .expect("explicit profile preserves missing-native state");
    assert_eq!(receipt.state(), BooleanEvaluationState::MissingNativeModel);
    assert!(receipt.soundness().is_none());
    assert!(receipt.completeness().is_none());
    assert!(receipt.semantic().is_none());
    assert!(receipt.probes().is_empty());
    assert!(receipt.target_query().is_none());
    f.cancellation.cancel();
    assert!(output.project_current(&f.broker, &view).is_err());
    assert_eq!(receipt.state(), BooleanEvaluationState::MissingNativeModel);
}

#[test]
#[ignore = "opt-in actual native unique/tautological Boolean model-space goldens"]
fn tlcx40_boolean_native_unique_and_top_have_independent_exact_expectations() {
    let registry = native_registry();
    let cwd = std::env::current_dir().unwrap();
    for (text, completeness, semantic, expected_defects) in [
        (UNIQUE, exact(0, 1), exact(0, 1), 0),
        (TOP, exact(1, 2), exact(1, 4), 1),
    ] {
        let (f, descriptor) = registered(text);
        let view = f.reservation.lease();
        let FormalLowering::Checked(problem) = lower(&f, &view) else {
            panic!("Boolean fixture")
        };
        let request = CanonicalRequest::formal(&problem, f.context.state(), GoalPolicy::NoGoal)
            .unwrap()
            .with_evaluation(descriptor)
            .unwrap();
        let output =
            solve_with_services(request, services(&f, &view, Some(&registry), &cwd)).unwrap();
        assert_eq!(output.status(), CanonicalStatus::ConditionalModel);
        // Cut only the explicitly requested evaluation, preserving the actual
        // parsed source/context and native model producer. Scope alone cannot
        // synthesize a measured receipt or a numeric zero.
        let no_profile =
            CanonicalRequest::formal(&problem, f.context.state(), GoalPolicy::NoGoal).unwrap();
        let unmeasured =
            solve_with_services(no_profile, services(&f, &view, Some(&registry), &cwd)).unwrap();
        assert!(unmeasured.native_model().is_some());
        assert!(unmeasured.boolean_measurement().is_none());
        let measured = output.boolean_measurement().unwrap();
        assert_eq!(measured.state(), BooleanEvaluationState::Measured);
        assert_eq!(measured.soundness(), Some(&exact(0, 1)));
        assert_eq!(measured.completeness(), Some(&completeness));
        assert_eq!(measured.semantic(), Some(&semantic));
        assert_eq!(measured.probes().len(), 2);
        assert!(measured.probes().iter().all(|p| p.weight_numerator == 1
            && p.weight_denominator == 1
            && p.assignments == Some(2)));
        let ledger = measured.facets().unwrap();
        assert_eq!(
            ledger
                .entries()
                .iter()
                .filter(|e| matches!(e.facet, Facet::Defect(_)))
                .count(),
            expected_defects
        );
        assert!(ledger
            .entries()
            .iter()
            .any(|e| matches!(e.facet, Facet::Uncertainty(_))));
        let bytes = measured.target_bytes();
        assert_eq!(
            measured.target_revision(),
            Some(SourceRevision::of(std::str::from_utf8(bytes).unwrap()))
        );
        assert_eq!(
            measured.target_query().unwrap().boolean_symbols(),
            problem.query().boolean_symbols()
        );
        assert_eq!(measured.source_context(), f.context.state().revision());
        assert_eq!(output.selection_summary().unwrap().recommendation, None);
        let rendered = output.historical_text();
        let facets = rendered
            .lines()
            .find(|line| line.starts_with("D: "))
            .expect("actual renderer emits the four reported facet references");
        let defect = facets
            .strip_prefix("D: ")
            .unwrap()
            .split_once("; Delta: ")
            .unwrap()
            .0;
        if expected_defects == 0 {
            assert_eq!(defect, "Unknown/NeedsEvidence");
        } else {
            assert_eq!(defect.len(), 64);
            assert!(defect.bytes().all(|b| b.is_ascii_hexdigit()));
        }
        assert!(output.project_current(&f.broker, &view).is_ok());
        f.cancellation.cancel();
        assert!(output.project_current(&f.broker, &view).is_err());
        assert_eq!(measured.semantic(), Some(&semantic));
        assert_eq!(output.historical_text(), rendered);
    }
}

#[test]
#[ignore = "opt-in actual native OR witness with independent valuation-conditional oracle"]
fn tlcx40_boolean_native_or_uses_actual_witness_not_assumed_solver_choice() {
    let (f, descriptor) = registered(OR);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("Boolean fixture")
    };
    let request = CanonicalRequest::formal(&problem, f.context.state(), GoalPolicy::NoGoal)
        .unwrap()
        .with_evaluation(descriptor)
        .unwrap();
    let registry = native_registry();
    let cwd = std::env::current_dir().unwrap();
    let output = solve_with_services(request, services(&f, &view, Some(&registry), &cwd)).unwrap();
    let model = output.native_model().unwrap().model();
    let values: Vec<_> = problem
        .symbols()
        .iter()
        .map(|binding| match binding.symbol() {
            FormalSymbol::Boolean(id) => model.boolean(id).unwrap(),
            _ => panic!("Boolean profile"),
        })
        .collect();
    assert_eq!(values.len(), 2);
    assert!(values[0] || values[1]);
    // Independent hand enumeration: Γ models are 01,10,11. For witness11 all
    // four literal events lose alternatives; for witness01/10 three do.
    let failures = if values[0] && values[1] { 4 } else { 3 };
    let measured = output.boolean_measurement().unwrap();
    assert_eq!(measured.soundness(), Some(&exact(0, 1)));
    assert_eq!(measured.completeness(), Some(&exact(failures, 4)));
    assert_eq!(measured.semantic(), Some(&exact(failures, 8)));
    assert_eq!(measured.probes().len(), 4);
    assert!(measured.probes().iter().all(|p| p.assignments == Some(4)));
    assert_eq!(output.selection_summary().unwrap().recommendation, None);
}

#[test]
#[ignore = "opt-in actual native out-of-profile cases must return no scalar"]
fn tlcx40_boolean_native_unsupported_integer_empty_large_and_expression_profiles_are_not_zero() {
    let large = format!(
        "(set-logic QF_UF)\n{}(assert (and {}))\n(check-sat)\n",
        (0..9)
            .map(|n| format!("(declare-const b{n} Bool)\n"))
            .collect::<String>(),
        (0..9)
            .map(|n| format!("b{n}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let empty = "(set-logic QF_LIA)\n(assert true)\n(check-sat)\n";
    let numeric =
        "(set-logic QF_LIA)\n(declare-const p Bool)\n(assert (and p (= 1 1)))\n(check-sat)\n";
    let registry = native_registry();
    let cwd = std::env::current_dir().unwrap();
    for (text, state) in [
        (SAT, BooleanEvaluationState::UnsupportedInteger),
        (empty, BooleanEvaluationState::EmptyAtomSet),
        (large.as_str(), BooleanEvaluationState::AtomLimit),
        (numeric, BooleanEvaluationState::UnsupportedFormal),
    ] {
        let (f, descriptor) = registered(text);
        let view = f.reservation.lease();
        let FormalLowering::Checked(problem) = lower(&f, &view) else {
            panic!("valid AST, outside finite Boolean profile")
        };
        let request = CanonicalRequest::formal(&problem, f.context.state(), GoalPolicy::NoGoal)
            .unwrap()
            .with_evaluation(descriptor)
            .unwrap();
        let output =
            solve_with_services(request, services(&f, &view, Some(&registry), &cwd)).unwrap();
        assert!(output.native_model().is_some());
        let receipt = output.boolean_measurement().unwrap();
        assert_eq!(receipt.state(), state);
        assert!(receipt.soundness().is_none());
        assert!(receipt.completeness().is_none());
        assert!(receipt.semantic().is_none());
        assert!(receipt.probes().is_empty());
        assert_eq!(output.selection_summary().unwrap().recommendation, None);
    }
}

#[test]
#[ignore = "opt-in actual native result must survive evaluation receipt output-quota failure"]
fn tlcx40_boolean_native_receipt_quota_failure_keeps_actual_model_and_claim() {
    let (f, descriptor) = registered(UNIQUE);
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("Boolean fixture")
    };
    let request = CanonicalRequest::formal(&problem, f.context.state(), GoalPolicy::NoGoal)
        .unwrap()
        .with_evaluation(descriptor)
        .unwrap();
    // Actual native peak: decision64 + retained stdout64 + adapter80 +
    // supervisor80 = 288KiB. Leave320 so the child completes. After its
    // transient buffers drop, retained128 leaves192 < receipt's256KiB claim.
    let amount = view.amount(ResourceKind::Output).unwrap() - 320 * 1024;
    let _hog = view
        .claim_resources(&[ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(amount).unwrap(),
        )])
        .unwrap();
    let registry = native_registry();
    let cwd = std::env::current_dir().unwrap();
    match solve_with_services(request, services(&f, &view, Some(&registry), &cwd)) {
        Err(CanonicalError::Postprocess(failure)) => {
            assert!(matches!(
                failure.reason(),
                PostprocessError::Evaluation(EvaluationError::Resource(ResourceError::Exhausted(
                    ResourceKind::Output
                )))
            ));
            assert_eq!(
                failure.evidence().kind(),
                NativeEvidenceKind::ModelRechecked
            );
            assert_eq!(failure.evidence().stored_results(), 1);
            assert!(failure
                .evidence()
                .native_model()
                .unwrap()
                .observation()
                .stdout()
                .starts_with(b"sat"));
            assert!(failure.check_retained_resources().is_ok());
        }
        other => panic!(
            "expected post-native receipt quota failure, got {:?}",
            other.err()
        ),
    };
}
