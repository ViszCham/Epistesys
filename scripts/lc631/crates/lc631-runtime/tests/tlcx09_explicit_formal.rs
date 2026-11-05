use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::{
    explicit_formal::{
        lower_explicit_formal, FormalErrorKind, FormalLowering, RecommendationState,
    },
    resource::*,
};
use lc631_substrate::source::SourceStore;
use std::{sync::Arc, time::Duration};

fn fixture(
    text: &str,
    body: impl FnOnce(&SourceStore, ContextRevision, ContextScope, &ResourceReservationView<'_>),
) {
    let source = SourceStore::new(Arc::from(text), 32768).unwrap();
    let context = ContextRevision::original(SourceRevision::of(text));
    let d = Digest32::from_bytes([7; 32]);
    let scope = ContextScope::declared(d, d, d, d);
    let limits = ResourceLimits::uniform(8 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            1,
            context.source().digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(8 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    {
        let view = reservation.lease();
        body(&source, context, scope, &view);
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx09_explicit_formal_source_is_lossless_and_status_emission_is_independent_golden() {
    let text="; exact input\n(set-logic QF_LIA)\n(declare-const x Int)\n(assert (<= 0 x))\n(assert (< x 4))\n(check-sat)\n";
    fixture(text, |source, context, scope, view| {
        let FormalLowering::Checked(problem) =
            lower_explicit_formal(source, context, scope, view).unwrap()
        else {
            panic!("formal fragment must lower")
        };
        assert_eq!(problem.source().text(), text);
        assert_eq!(problem.context(), context);
        assert_eq!(problem.scope(), scope);
        assert_eq!(
            problem.recommendation_state(),
            RecommendationState::NoRecommendation
        );
        assert!(problem.goal().is_none());
        assert_eq!(source.slice(problem.symbols()[0].span()).unwrap(), "x");
        let emitted = problem
            .query()
            .emit(lc631_solvers::runner::QuerySourceIdentity::from_bytes(
                *context.source().digest().as_bytes(),
            ))
            .unwrap();
        assert_eq!(emitted.payload(),b"(set-logic QF_LIA)\n(declare-const i0 Int)\n(assert (<= 0 i0))\n(assert (< i0 4))\n(check-sat)\n");
    });
}

#[test]
fn tlcx09_explicit_goal_is_single_integer_and_finite_not_inferred() {
    fixture("(set-logic QF_LIA)(declare-const x Int)(assert (<= (- 3) x))(assert (<= x 8))(minimize x)(check-sat)",|source,context,scope,view|{
        let FormalLowering::Checked(problem)=lower_explicit_formal(source,context,scope,view).unwrap() else {panic!("bounded goal expected")};
        let goal=problem.goal().unwrap();assert_eq!((goal.lower(),goal.upper()),(-3,8));
        assert_eq!(goal.direction(),lc631_solvers::smt::ObjectiveDirection::Minimize);
        assert_eq!(problem.recommendation_state(),RecommendationState::ExplicitFiniteGoal);
    });
}

#[test]
fn tlcx09_unsupported_commands_and_nested_features_never_produce_executable_query() {
    for text in [
        "(set-logic QF_LIA)(assert (forall ((x Int)) (> x 0)))(check-sat)",
        "(set-logic QF_LIA)(declare-const x Int)(assert (= (* x x) 4))(check-sat)",
        "(set-logic QF_LIA)(assert (let ((p true)) p))(check-sat)",
        "(set-logic QF_LIA)(set-option :produce-proofs true)(check-sat)",
        "(set-logic QF_LIA)(declare-const x Int)(maximize x)(check-sat)",
    ] {
        fixture(text, |source, context, scope, view| {
            let FormalLowering::Unsupported(report) =
                lower_explicit_formal(source, context, scope, view).unwrap()
            else {
                panic!("unsupported must not execute")
            };
            assert_eq!(report.source().text(), text);
            assert!(!report.residuals().is_empty());
            assert!(source
                .slice(report.residuals()[0].span())
                .unwrap()
                .starts_with('('));
        });
    }
}

#[test]
fn tlcx09_duplicates_undeclared_sort_mismatch_and_command_injection_are_rejected() {
    for text in [
        "(set-logic QF_LIA)(declare-const x Int)(declare-const x Int)(check-sat)",
        "(set-logic QF_LIA)(assert p)(check-sat)",
        "(set-logic QF_LIA)(declare-const x Int)(assert x)(check-sat)",
    ] {
        fixture(text, |source, context, scope, view| {
            assert!(lower_explicit_formal(source, context, scope, view).is_err());
        });
    }
    fixture(
        "(set-logic QF_LIA)(check-sat)(assert false)",
        |source, context, scope, view| {
            assert!(matches!(
                lower_explicit_formal(source, context, scope, view),
                Ok(FormalLowering::Unsupported(_))
            ));
        },
    );
}

#[test]
fn tlcx09_source_limit_context_swap_and_malformed_source_are_not_success() {
    let oversized = " ".repeat(16385);
    fixture(&oversized, |source, context, scope, view| {
        assert!(matches!(
            lower_explicit_formal(source, context, scope, view)
                .err()
                .unwrap()
                .kind(),
            FormalErrorKind::SourceTooLarge
        ));
    });
    fixture(
        "(set-logic QF_LIA)(check-sat)",
        |source, _context, scope, view| {
            assert!(matches!(
                lower_explicit_formal(
                    source,
                    ContextRevision::original(SourceRevision::of("foreign")),
                    scope,
                    view
                )
                .err()
                .unwrap()
                .kind(),
                FormalErrorKind::SourceMismatch
            ));
        },
    );
    fixture("(set-logic QF_LIA", |source, context, scope, view| {
        assert!(lower_explicit_formal(source, context, scope, view).is_err());
    });
}

#[test]
fn tlcx09_boolean_fragment_has_opaque_ids_and_explicit_node_depth_limits() {
    fixture(
        "(set-logic QF_UF)(declare-fun p () Bool)(assert (or p (not p)))(check-sat)",
        |source, context, scope, view| {
            let FormalLowering::Checked(problem) =
                lower_explicit_formal(source, context, scope, view).unwrap()
            else {
                panic!("Boolean source should lower")
            };
            assert_eq!(
                problem.declared_logic(),
                lc631_runtime::explicit_formal::DeclaredLogic::Boolean
            );
            let emitted = problem
                .query()
                .emit(lc631_solvers::runner::QuerySourceIdentity::from_bytes(
                    *context.source().digest().as_bytes(),
                ))
                .unwrap();
            assert_eq!(emitted.payload(),b"(set-logic QF_LIA)\n(declare-const b0 Bool)\n(assert (or b0 (not b0)))\n(check-sat)\n");
        },
    );
    let wide = format!(
        "(set-logic QF_UF)(declare-const p Bool)(assert (and {}))(check-sat)",
        "p ".repeat(4100)
    );
    fixture(&wide, |source, context, scope, view| {
        assert!(matches!(
            lower_explicit_formal(source, context, scope, view)
                .err()
                .unwrap()
                .kind(),
            FormalErrorKind::Syntax(lc631_solvers::protocol::sexpr::SExprError::NodeLimitExceeded)
        ));
    });
    let deep = format!(
        "(set-logic QF_UF)(assert {}true{})(check-sat)",
        "(not ".repeat(65),
        ")".repeat(65)
    );
    fixture(&deep, |source, context, scope, view| {
        assert!(matches!(
            lower_explicit_formal(source, context, scope, view)
                .err()
                .unwrap()
                .kind(),
            FormalErrorKind::Syntax(lc631_solvers::protocol::sexpr::SExprError::DepthLimitExceeded)
        ));
    });
}

#[test]
fn tlcx09_degenerate_boolean_and_integer_operators_are_not_repaired() {
    for expression in [
        "(and)",
        "(and true)",
        "(or)",
        "(or false)",
        "(= (+) 0)",
        "(= (+ 1) 1)",
    ] {
        let text = format!("(set-logic QF_LIA)(assert {expression})(check-sat)");
        fixture(&text, |source, context, scope, view| {
            assert!(matches!(
                lower_explicit_formal(source, context, scope, view)
                    .err()
                    .unwrap()
                    .kind(),
                FormalErrorKind::MalformedCommand
            ));
        });
    }
}

#[test]
fn tlcx09_held_residual_rejects_foreign_and_cancelled_current_consumption() {
    fn owner(
        source: Digest32,
        cancel: Cancellation,
    ) -> (ResourceBroker, RunResources, Reservation) {
        let limits = ResourceLimits::uniform(8 * 1024 * 1024).unwrap();
        let broker = ResourceBroker::new(limits).unwrap();
        let run = broker
            .begin_run(
                1,
                source,
                limits,
                Deadline::after(Duration::from_secs(30)).unwrap(),
                cancel,
            )
            .unwrap();
        let reservation = run
            .reserve(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(8 * 1024 * 1024).unwrap(),
            )])
            .unwrap();
        (broker, run, reservation)
    }
    let text = "(set-logic QF_LIA)(assert (forall ((x Int)) (> x 0)))(check-sat)";
    let source = SourceStore::new(Arc::from(text), 32768).unwrap();
    let context = ContextRevision::original(SourceRevision::of(text));
    let d = Digest32::from_bytes([7; 32]);
    let scope = ContextScope::declared(d, d, d, d);
    let cancel = Cancellation::default();
    let (_broker, _run, mut reservation) = owner(context.source().digest(), cancel.clone());
    let (_foreign_broker, _foreign_run, mut foreign) =
        owner(context.source().digest(), Cancellation::default());
    {
        let view = reservation.lease();
        let foreign_view = foreign.lease();
        let FormalLowering::Unsupported(report) =
            lower_explicit_formal(&source, context, scope, &view).unwrap()
        else {
            panic!("unsupported residual expected")
        };
        assert_eq!(report.check_current(&view), Ok(()));
        assert_eq!(
            report.check_current(&foreign_view),
            Err(ResourceError::ForeignOwner)
        );
        cancel.cancel();
        assert_eq!(report.check_current(&view), Err(ResourceError::Cancelled));
        // Historical source/residual inspection is not active consumption.
        assert_eq!(report.source().text(), text);
        assert!(!report.residuals().is_empty());
    }
    reservation.release_confirmed().unwrap();
    foreign.release_confirmed().unwrap();
}
