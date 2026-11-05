//! The actual producer's reason must survive selection and exact rendering.
#[path = "tlcx40_canonical_support/mod.rs"]
#[allow(dead_code)] // Reuse only the existing actual source/native fixture boundary.
mod support;
use lc631_core::context::{ContextScope, SourceRevision};
use lc631_runtime::{canonical_solve::*, explicit_formal::FormalLowering};
use support::*;

const NUMERIC_AST: &str =
    "(set-logic QF_LIA)\n(declare-const p Bool)\n(assert (and p (= 1 1)))\n(check-sat)\n";
const UNIQUE: &str = "(set-logic QF_UF)\n(declare-const p Bool)\n(assert p)\n(check-sat)\n";

fn check(text: &str, expected: BooleanEvaluationState, rendered_axis: &str) {
    // The evaluation and scope are fixed before context creation and parsing.
    let descriptor = DeclaredEvaluation::finite_boolean(
        SourceRevision::of(text),
        ContextScope::declared(d(1), d(2), d(3), d(4)),
        GoalPolicy::NoGoal,
    )
    .unwrap();
    let f = Fixture::with_scope(text, descriptor.scope());
    let view = f.reservation.lease();
    let FormalLowering::Checked(problem) = lower(&f, &view) else {
        panic!("actual source must lower, not bypass the native producer")
    };
    let request = CanonicalRequest::formal(&problem, f.context.state(), GoalPolicy::NoGoal)
        .unwrap()
        .with_evaluation(descriptor)
        .unwrap();
    let registry = native_registry();
    let cwd = std::env::current_dir().unwrap();
    let output = solve_with_services(request, services(&f, &view, Some(&registry), &cwd)).unwrap();
    let native = output
        .native_model()
        .expect("actual rechecked native model");
    assert!(native.observation().stdout().starts_with(b"sat"));
    assert!(std::ptr::eq(native.model().query(), problem.query()));
    let measured = output.boolean_measurement().unwrap();
    assert_eq!(measured.state(), expected);
    if expected == BooleanEvaluationState::Measured {
        assert_eq!(
            measured.semantic(),
            Some(&lc631_core::measurement_numeric::RationalInterval::exact_ratio(0, 1).unwrap())
        );
    } else {
        assert!(measured.semantic().is_none());
        assert!(measured.soundness().is_none());
        assert!(measured.completeness().is_none());
        assert!(measured.probes().is_empty());
    }
    let selection = output.selection_summary().unwrap();
    assert_eq!(selection.candidate_count, 1);
    assert_eq!(selection.recommendation, None);
    let rendered = output.historical_text();
    assert!(rendered.contains(rendered_axis), "{rendered}");
    assert!(rendered.contains("total TL is NoScalar/MissingEvaluator"));
    assert!(output.project_current(&f.broker, &view).is_ok());
    f.cancellation.cancel();
    assert!(output.project_current(&f.broker, &view).is_err());
    assert_eq!(output.historical_text(), rendered);
}

#[test]
#[ignore = "actual pinned Z3; negative-first unsupported integer reason preservation"]
fn tlcx40_actual_integer_semantic_axis_remains_unsupported() {
    check(
        SAT,
        BooleanEvaluationState::UnsupportedInteger,
        "reported axis SemanticLoss: Unavailable/Unsupported",
    );
}

#[test]
#[ignore = "actual pinned Z3; negative-first unsupported numeric AST reason preservation"]
fn tlcx40_actual_numeric_ast_semantic_axis_remains_unsupported() {
    check(
        NUMERIC_AST,
        BooleanEvaluationState::UnsupportedFormal,
        "reported axis SemanticLoss: Unavailable/Unsupported",
    );
}

#[test]
#[ignore = "actual pinned Z3; supported Boolean control retains measured zero in scope"]
fn tlcx40_actual_supported_boolean_semantic_axis_retains_exact_measurement() {
    check(
        UNIQUE,
        BooleanEvaluationState::Measured,
        "reported axis SemanticLoss: ExactArithmetic [0,0]",
    );
}
