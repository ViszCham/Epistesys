//! Test the declared logic and the operand sorts at the public lowering boundary.
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::{
    explicit_formal::{
        lower_explicit_formal, DeclaredLogic, FormalError, FormalErrorKind, FormalLowering,
        ResidualReason,
    },
    resource::{
        Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceKind,
        ResourceLimits, ResourceRequest, ResourceReservationView,
    },
};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, IntExpr, IntSymbolId};
use lc631_substrate::source::SourceStore;
use std::{sync::Arc, time::Duration};

fn with_lowering(
    text: &str,
    inspect: impl FnOnce(&ResourceReservationView<'_>, Result<FormalLowering<'_>, FormalError>),
) {
    let source = SourceStore::new(Arc::from(text), 16 * 1024).unwrap();
    let context = ContextRevision::original(SourceRevision::of(text));
    let pin = Digest32::from_bytes([7; 32]);
    let scope = ContextScope::declared(pin, pin, pin, pin);
    let ram = 4 * 1024 * 1024;
    let limits = ResourceLimits::uniform(ram).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            1,
            source.id().digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(ram).unwrap(),
        )])
        .unwrap();
    {
        let view = reservation.lease();
        inspect(&view, lower_explicit_formal(&source, context, scope, &view));
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

fn expect_held(logic: DeclaredLogic, assertion: &str, detail: &str) {
    let logic_name = match logic {
        DeclaredLogic::Boolean => "QF_UF",
        DeclaredLogic::QfLia => "QF_LIA",
    };
    let command = format!("(assert {assertion})");
    // The comment contains the same command. Its occurrence must never own the residual.
    let prefix =
        format!("; {command}\n(set-logic {logic_name})\n(declare-const p Bool)\n(assert p)\n");
    let tail = "\n(check-sat)\n";
    let text = format!("{prefix}{command}{tail}");
    with_lowering(&text, |view, result| {
        let report = match result {
            Ok(FormalLowering::Unsupported(report)) => report,
            Ok(FormalLowering::Checked(_)) => {
                panic!("{logic_name} produced an executable query for {assertion}")
            }
            Err(error) => panic!(
                "{logic_name} returned {:?} for {assertion}, expected a residual",
                error.kind()
            ),
        };
        report.check_current(view).unwrap();
        assert_eq!(report.declared_logic(), Some(logic));
        assert_eq!(report.source().text(), text);
        let [offense, remainder] = report.residuals() else {
            panic!("expected exactly the held assertion and its unprocessed tail")
        };
        match logic {
            DeclaredLogic::Boolean => assert!(matches!(
                offense.reason(),
                ResidualReason::UnsupportedExpression | ResidualReason::UnsupportedLogic
            )),
            DeclaredLogic::QfLia => {
                assert_eq!(offense.reason(), ResidualReason::UnsupportedExpression)
            }
        }
        assert_eq!(offense.span().start(), prefix.len());
        assert_eq!(report.source().slice(offense.span()).unwrap(), command);
        assert_eq!(
            report.source().slice(offense.detail_span()).unwrap(),
            detail
        );
        assert!(offense.detail_span().start() >= offense.span().start());
        assert!(offense.detail_span().end() <= offense.span().end());
        assert_eq!(remainder.reason(), ResidualReason::UnprocessedRemainder);
        assert_eq!(remainder.span().start(), offense.span().end());
        assert_eq!(remainder.detail_span(), remainder.span());
        assert_eq!(report.source().slice(remainder.span()).unwrap(), tail);
    });
}

#[test]
fn qf_uf_holds_integer_comparisons_instead_of_exposing_a_query() {
    for comparison in ["(< 0 1)", "(<= 1 1)", "(> 1 0)", "(>= (- 2) (+ 0 1))", "(= 0 1)"] {
        expect_held(DeclaredLogic::Boolean, comparison, comparison);
    }
}

#[test]
fn qf_uf_holds_the_nested_integer_comparison_at_its_own_span() {
    expect_held(
        DeclaredLogic::Boolean,
        "(or p (not (<= (+ 0 1) 2)))",
        "(<= (+ 0 1) 2)",
    );
}

#[test]
fn qf_lia_boolean_equality_reports_the_whole_unsupported_expression() {
    for equality in [
        "(= true false)",
        "(= true p)",
        "(= p false)",
        "(= p p)",
        "(= (not p) true)",
        "(= false (and p true))",
        "(= (or p false) (not p))",
        "(= (< 0 1) true)",
    ] {
        expect_held(DeclaredLogic::QfLia, equality, equality);
    }
    expect_held(
        DeclaredLogic::QfLia,
        "(and p (= true false))",
        "(= true false)",
    );
}

#[test]
fn qf_lia_invalid_operands_keep_their_typed_errors() {
    let prefix = "(set-logic QF_LIA)(declare-const p Bool)(declare-const x Int)";
    for (predicate, expected, exact_detail) in [
        (
            "(= true missing)",
            FormalErrorKind::UndeclaredSymbol,
            Some("missing"),
        ),
        (
            "(= missing p)",
            FormalErrorKind::UndeclaredSymbol,
            Some("missing"),
        ),
        (
            "(= p (and p missing))",
            FormalErrorKind::UndeclaredSymbol,
            Some("missing"),
        ),
        ("(= (not) false)", FormalErrorKind::MalformedCommand, Some("(not)")),
        ("(= true (and p))", FormalErrorKind::MalformedCommand, Some("(and p)")),
        ("(= p x)", FormalErrorKind::SortMismatch, None),
        ("(= x p)", FormalErrorKind::SortMismatch, None),
        (
            "(= true)",
            FormalErrorKind::MalformedCommand,
            Some("(= true)"),
        ),
    ] {
        let command = format!("(assert {predicate})");
        let text = format!("{prefix}{command}(check-sat)");
        with_lowering(&text, |_, result| {
            let error = match result {
                Err(error) => error,
                Ok(FormalLowering::Unsupported(_)) => {
                    panic!("{predicate} masked {expected:?} as unsupported")
                }
                Ok(FormalLowering::Checked(_)) => {
                    panic!("{predicate} produced an executable query")
                }
            };
            assert_eq!(error.kind(), &expected, "{predicate}");
            assert_eq!(error.source().text(), text);
            let predicate_start = prefix.len() + "(assert ".len();
            assert!(error.span().start() >= predicate_start);
            assert!(error.span().start() < error.span().end());
            assert!(error.span().end() <= predicate_start + predicate.len());
            if let Some(detail) = exact_detail {
                assert_eq!(error.source().slice(error.span()).unwrap(), detail);
            }
        });
    }
}

#[test]
fn supported_boolean_and_integer_assertions_keep_their_typed_query() {
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let x = IntExpr::Variable(IntSymbolId::new(0));
    let sum = IntExpr::Add(vec![IntExpr::Constant(1), IntExpr::Constant(2)]);
    for (source_logic, logic, declaration, predicate, expected) in [
        (
            "QF_UF",
            DeclaredLogic::Boolean,
            "(declare-const p Bool)",
            "(or p (not false))",
            BoolExpr::Or(vec![p, BoolExpr::Not(Box::new(BoolExpr::Constant(false)))]),
        ),
        (
            "QF_LIA",
            DeclaredLogic::QfLia,
            "(declare-const x Int)",
            "(= x (+ 1 2))",
            BoolExpr::Equal(x, sum),
        ),
        (
            "QF_LIA",
            DeclaredLogic::QfLia,
            "(declare-const p Bool)",
            "(and p (< 0 1))",
            BoolExpr::And(vec![
                BoolExpr::Variable(BoolSymbolId::new(0)),
                BoolExpr::LessThan(IntExpr::Constant(0), IntExpr::Constant(1)),
            ]),
        ),
    ] {
        let command = format!("(assert {predicate})");
        let text = format!("(set-logic {source_logic}){declaration}{command}(check-sat)");
        with_lowering(&text, |view, result| {
            let Ok(FormalLowering::Checked(problem)) = result else {
                panic!("expected checked {source_logic} query for {predicate}")
            };
            problem.check_current(view).unwrap();
            assert_eq!(problem.declared_logic(), logic);
            assert_eq!(problem.query().assertions(), &[expected]);
            let [span] = problem.assertion_spans() else {
                panic!("expected one assertion")
            };
            assert_eq!(problem.source().slice(*span).unwrap(), command);
        });
    }
}
