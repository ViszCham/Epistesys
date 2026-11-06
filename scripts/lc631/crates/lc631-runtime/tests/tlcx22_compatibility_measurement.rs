//! 日本語: 固定CR-ACOMP契約の負例・接続試験を実装bodyより先に追加する。
//! English: Negative/connection tests precede the implementation body.
//! Planned API is intentionally absent. Initial E0432 is NOT semantic RED.
mod tlcx22_compatibility_support;
use lc631_analysis::creativity_measurement::{
    ComponentMetric, ComponentState, IndependenceStatus, InputBasis, RationalInterval,
    SemanticValidation,
};
use lc631_runtime::compatibility_measurement::*;
use lc631_solvers::typed_query::{IntSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::resource::{ResourceError, ResourceKind};
use tlcx22_compatibility_support::*;

fn full() -> [Dispatch; 4] {
    [0, 1, 2, 3].map(|case| Dispatch {
        case,
        through: Stage::Use,
    })
}
fn cut_use() -> [Dispatch; 4] {
    [
        Dispatch {
            case: 0,
            through: Stage::Use,
        },
        Dispatch {
            case: 1,
            through: Stage::Use,
        },
        Dispatch {
            case: 2,
            through: Stage::Use,
        },
        Dispatch {
            case: 3,
            through: Stage::Roundtrip,
        },
    ]
}
fn bounds(state: &ComponentState) -> &RationalInterval {
    match state {
        ComponentState::Measured(value) => value,
        other => panic!("expected measured finite bounds, got {other:?}"),
    }
}

#[test]
fn tlcx22_acomp_consistent_wrong_permutation_is_typed_roundtrip_but_half_use_success() {
    fixture(1024 * 1024, |fx, cases, view, _cancel| {
        let input = fx.input(cases);
        let plan = Plan::given(input, limits(64), view).unwrap();
        let program = plan
            .permutation(&swapped(), &swapped_inverse(), view)
            .unwrap();
        let owner = measure(&plan, &program, &full(), view).unwrap();
        let report = owner.report_current(&plan, &program, view).unwrap();
        assert_eq!(report.metric, ComponentMetric::Compatibility);
        assert_eq!(
            bounds(&report.state),
            &RationalInterval::exact_ratio(1, 2).unwrap()
        );
        assert_eq!(report.basis, InputBasis::Given);
        assert_eq!(report.validation, SemanticValidation::Unassessed);
        assert_eq!(report.independence, IndependenceStatus::SharedDependency);
        let trials = owner.trials_current(&plan, &program, view).unwrap();
        assert_eq!(trials.len(), 4);
        // Literal hand oracle; production never generates its own expected table.
        let expected = [
            (false, false, Check::Pass),
            (false, true, Check::Fail),
            (true, false, Check::Fail),
            (true, true, Check::Pass),
        ];
        for (trial, (source, target, used)) in trials.iter().zip(expected) {
            assert_eq!(trial.type_check(), Check::Pass);
            assert_eq!(trial.roundtrip_check(), Check::Pass);
            assert_eq!(trial.source_value(), Some(source));
            assert_eq!(trial.target_value(), Some(target));
            assert_eq!(trial.use_check(), used);
        }
        // Optional interface failures never silently become candidate ineligibility.
        assert_eq!(
            owner.hard_current(&plan, &program, view).unwrap(),
            HardState::Pass
        );
        assert_eq!(
            next_use_current(&plan, &program, &owner, view).unwrap(),
            NextUse::RejectedCompatibility
        );
        let use_result = use_current(&plan, &program, &owner, 0, view).unwrap();
        assert_eq!(
            use_result.outcome_current(view).unwrap(),
            UseOutcome::RejectedCompatibility
        );
        assert_eq!(use_result.actual_use_count(), 0);
        assert!(matches!(
            use_result.models_current(view),
            Err(Error::NoExecutedUse)
        ));
    });
}

#[test]
fn tlcx22_acomp_correct_bridge_false_false_and_fresh_actual_current_use() {
    fixture(1024 * 1024, |fx, cases, view, _cancel| {
        let input = fx.input(cases);
        let plan = Plan::given(input, limits(26), view).unwrap();
        let program = plan.permutation(&map(), &inverse(), view).unwrap();
        let owner = measure(&plan, &program, &full(), view).unwrap();
        let report = owner.report_current(&plan, &program, view).unwrap();
        assert_eq!(
            bounds(&report.state),
            &RationalInterval::exact_ratio(1, 1).unwrap()
        );
        assert_eq!(
            owner.hard_current(&plan, &program, view).unwrap(),
            HardState::Pass
        );
        assert_eq!(plan.work_used_current(view).unwrap(), 24);
        let lineage = owner.lineage_current(&plan, &program, view).unwrap();
        assert_eq!(lineage.source_context, input.source_context);
        assert_eq!(lineage.target_context, input.target_context);
        assert_eq!(lineage.root_goal_revision, input.scope.goal());
        assert_eq!(lineage.oracle, input.pins.oracle);
        assert_eq!(lineage.baseline, input.pins.baseline);
        assert_ne!(lineage.forward_program, lineage.inverse_program);
        assert_eq!(
            next_use_current(&plan, &program, &owner, view).unwrap(),
            NextUse::Ready
        );
        let fresh = use_current(&plan, &program, &owner, 0, view).unwrap();
        assert_eq!(
            fresh.outcome_current(view).unwrap(),
            UseOutcome::Executed {
                case: 0,
                source_value: false,
                target_value: false,
            }
        );
        assert_eq!(fresh.actual_use_count(), 2);
        let (source, target) = fresh.models_current(view).unwrap();
        assert!(std::ptr::eq(source.query(), &fx.source_theory));
        assert!(std::ptr::eq(target.query(), &fx.target_theory));
        assert_eq!(source.boolean(P), Some(false));
        assert_eq!(target.boolean(U), Some(false));
        assert_eq!(target.boolean(RP), Some(true));
        assert_eq!(plan.work_used_current(view).unwrap(), 26);
        assert!(matches!(
            use_current(&plan, &program, &owner, 0, view),
            Err(Error::WorkBudget)
        ));
        assert_eq!(plan.work_used_current(view).unwrap(), 26);
    });
    fixture(1024 * 1024, |fx, cases, view, _cancel| {
        let plan = Plan::given(fx.input(cases), limits(24), view).unwrap();
        let program = plan.permutation(&map(), &inverse(), view).unwrap();
        let owner = measure(&plan, &program, &full(), view).unwrap();
        assert_eq!(
            next_use_current(&plan, &program, &owner, view).unwrap(),
            NextUse::Ready
        );
        // Readiness alone cannot pay for or fabricate a fresh consumer invocation.
        assert!(matches!(
            use_current(&plan, &program, &owner, 0, view),
            Err(Error::WorkBudget)
        ));
        assert_eq!(plan.work_used_current(view).unwrap(), 24);
    });
}

#[test]
fn tlcx22_acomp_bad_inverse_executes_and_is_measured_instead_of_preaccepted() {
    fixture(1024 * 1024, |fx, cases, view, _cancel| {
        let plan = Plan::given(fx.input(cases), limits(64), view).unwrap();
        // Both programs are individually bijective; actual R(F(x)) fails on 01/10.
        let program = plan.permutation(&map(), &swapped_inverse(), view).unwrap();
        let owner = measure(&plan, &program, &full(), view).unwrap();
        let trials = owner.trials_current(&plan, &program, view).unwrap();
        assert_eq!(
            trials
                .iter()
                .map(Trial::roundtrip_check)
                .collect::<Vec<_>>(),
            vec![Check::Pass, Check::Fail, Check::Fail, Check::Pass]
        );
        assert_eq!(
            bounds(&owner.report_current(&plan, &program, view).unwrap().state),
            &RationalInterval::exact_ratio(1, 2).unwrap()
        );
        assert_eq!(
            owner.hard_current(&plan, &program, view).unwrap(),
            HardState::Pass
        );
    });
}

#[test]
fn tlcx22_acomp_use_cut_keeps_denominator_and_followup_consumes_same_budget() {
    fixture(1024 * 1024, |fx, cases, view, _cancel| {
        let plan = Plan::given(fx.input(cases), limits(24), view).unwrap();
        let program = plan.permutation(&map(), &inverse(), view).unwrap();
        let owner = measure(&plan, &program, &cut_use(), view).unwrap();
        assert_eq!(
            bounds(&owner.report_current(&plan, &program, view).unwrap().state),
            &RationalInterval::from_ratio(3, 4, 1, 1).unwrap()
        );
        assert_eq!(plan.work_used_current(view).unwrap(), 22);
        assert_eq!(
            next_use_current(&plan, &program, &owner, view).unwrap(),
            NextUse::NeedsProbe { case: 3 }
        );
        let fresh = use_current(&plan, &program, &owner, 3, view).unwrap();
        assert_eq!(fresh.actual_use_count(), 2);
        assert_eq!(fresh.models_current(view).unwrap().1.boolean(U), Some(true));
        assert_eq!(
            fresh.outcome_current(view).unwrap(),
            UseOutcome::Executed {
                case: 3,
                source_value: true,
                target_value: true,
            }
        );
        assert_eq!(
            bounds(&fresh.report_current(view).unwrap().state),
            &RationalInterval::exact_ratio(1, 1).unwrap()
        );
        assert_eq!(fresh.readiness_current(view).unwrap(), NextUse::Ready);
        assert_eq!(plan.work_used_current(view).unwrap(), 24);
        // Original observations are immutable; followup is a new private owner.
        assert_eq!(
            bounds(&owner.report_current(&plan, &program, view).unwrap().state),
            &RationalInterval::from_ratio(3, 4, 1, 1).unwrap()
        );
        assert!(matches!(
            use_current(&plan, &program, &owner, 3, view),
            Err(Error::WorkBudget)
        ));
    });
}

#[test]
fn tlcx22_acomp_funded_missing_frame_requests_probe_without_work_or_fresh_model() {
    fixture(1024 * 1024, |fx, cases, view, _cancel| {
        let plan = Plan::given(fx.input(cases), limits(26), view).unwrap();
        let program = plan.permutation(&map(), &inverse(), view).unwrap();
        let owner = measure(&plan, &program, &[], view).unwrap();
        assert_eq!(
            bounds(&owner.report_current(&plan, &program, view).unwrap().state),
            &RationalInterval::from_ratio(0, 1, 1, 1).unwrap()
        );
        assert_eq!(
            owner.hard_current(&plan, &program, view).unwrap(),
            HardState::NeedsEvidence
        );
        assert_eq!(
            next_use_current(&plan, &program, &owner, view).unwrap(),
            NextUse::NeedsProbe { case: 0 }
        );
        let pending = use_current(&plan, &program, &owner, 0, view).unwrap();
        assert_eq!(
            pending.outcome_current(view).unwrap(),
            UseOutcome::NeedsProbe { case: 0 }
        );
        assert_eq!(pending.actual_use_count(), 0);
        assert!(matches!(
            pending.models_current(view),
            Err(Error::NoExecutedUse)
        ));
        assert_eq!(plan.work_used_current(view).unwrap(), 0);
    });
}

#[test]
fn tlcx22_acomp_hard_output_failure_stays_separate_from_optional_use_failure() {
    fixture(1024 * 1024, |fx, cases, view, _cancel| {
        let plan = Plan::given(fx.input(cases), limits(64), view).unwrap();
        let program = plan
            .permutation(&hard_broken(), &hard_broken_inverse(), view)
            .unwrap();
        let owner = measure(&plan, &program, &full(), view).unwrap();
        assert_eq!(
            owner.hard_current(&plan, &program, view).unwrap(),
            HardState::Fail
        );
        assert!(matches!(
            use_current(&plan, &program, &owner, 0, view),
            Err(Error::HardInvariantFailed)
        ));
    });
}

#[test]
fn tlcx22_acomp_stale_plan_foreign_program_and_foreign_lease_do_not_reactivate_owner() {
    fixture(1024 * 1024, |fx, cases, view, _cancel| {
        let input = fx.input(cases);
        let plan = Plan::given(input, limits(64), view).unwrap();
        let program = plan.permutation(&map(), &inverse(), view).unwrap();
        let owner = measure(&plan, &program, &full(), view).unwrap();
        let other_program = plan.permutation(&map(), &inverse(), view).unwrap();
        assert!(matches!(
            owner.report_current(&plan, &other_program, view),
            Err(Error::BindingMismatch)
        ));
        let mut changed = input;
        changed.pins.oracle = d(99);
        let stale_plan = Plan::given(changed, limits(64), view).unwrap();
        assert!(matches!(
            owner.report_current(&stale_plan, &program, view),
            Err(Error::BindingMismatch)
        ));
        fixture(
            1024 * 1024,
            |_other_fx, _other_cases, other_view, _cancel| {
                assert!(matches!(
                    owner.report_current(&plan, &program, other_view),
                    Err(Error::ForeignReservation)
                ));
                assert!(matches!(
                    use_current(&plan, &program, &owner, 0, other_view),
                    Err(Error::ForeignReservation)
                ));
            },
        );
    });
}

#[test]
fn tlcx22_acomp_cancelled_current_reads_and_fresh_use_fail_without_changing_history() {
    fixture(1024 * 1024, |fx, cases, view, cancel| {
        let plan = Plan::given(fx.input(cases), limits(64), view).unwrap();
        let program = plan.permutation(&map(), &inverse(), view).unwrap();
        let owner = measure(&plan, &program, &full(), view).unwrap();
        let fresh = use_current(&plan, &program, &owner, 0, view).unwrap();
        cancel.cancel();
        assert!(matches!(
            owner.report_current(&plan, &program, view),
            Err(Error::Resource(ResourceError::Cancelled))
        ));
        assert!(matches!(
            fresh.models_current(view),
            Err(Error::Resource(ResourceError::Cancelled))
        ));
        assert!(matches!(
            use_current(&plan, &program, &owner, 0, view),
            Err(Error::Resource(ResourceError::Cancelled))
        ));
        assert_eq!(
            bounds(&owner.historical_report().state),
            &RationalInterval::exact_ratio(1, 1).unwrap()
        );
    });
}

#[test]
fn tlcx22_acomp_invalid_input_unsupported_and_work_unknown_are_distinct() {
    fixture(1024 * 1024, |fx, cases, view, _cancel| {
        let input = fx.input(cases);
        let empty = [];
        let mut missing = input;
        missing.cases = &empty;
        assert!(matches!(
            Plan::given(missing, limits(64), view),
            Err(Error::InvalidInput)
        ));
        let mut repeated = fx.cases();
        repeated[3] = repeated[0];
        let mut altered = input;
        altered.cases = &repeated;
        assert!(matches!(
            Plan::given(altered, limits(64), view),
            Err(Error::InvalidInput)
        ));
        for weight in [0, 2] {
            let mut weighted = fx.cases();
            weighted[0].weight = weight;
            let mut altered = input;
            altered.cases = &weighted;
            assert!(matches!(
                Plan::given(altered, limits(64), view),
                Err(Error::InvalidInput)
            ));
        }
        let mut duplicate = fx.values[0];
        duplicate[2] = duplicate[1];
        let mut malformed_cases = fx.cases();
        malformed_cases[0] = ProbeCase {
            input: &duplicate,
            weight: 1,
        };
        let mut malformed = input;
        malformed.cases = &malformed_cases;
        assert!(matches!(
            Plan::given(malformed, limits(64), view),
            Err(Error::InvalidInput)
        ));
        let integer = SmtStatusQuery::checked(
            vec![R, P, Q],
            vec![IntSymbolId::new(0)],
            fx.source_theory.assertions().to_vec(),
            SmtEmitLimits::default(),
        )
        .unwrap();
        let mut unsupported = input;
        unsupported.source_theory = &integer;
        assert!(matches!(
            Plan::given(unsupported, limits(64), view),
            Err(Error::Unsupported(Unsupported::Integer))
        ));
        let plan = Plan::given(input, limits(1), view).unwrap();
        let program = plan.permutation(&map(), &inverse(), view).unwrap();
        let duplicate = [Dispatch {
            case: 0,
            through: Stage::Use,
        }; 2];
        assert!(matches!(
            measure(&plan, &program, &duplicate, view),
            Err(Error::InvalidInput)
        ));
        assert_eq!(plan.work_used_current(view).unwrap(), 0);
        let owner = measure(&plan, &program, &full(), view).unwrap();
        assert_eq!(
            bounds(&owner.report_current(&plan, &program, view).unwrap().state),
            &RationalInterval::from_ratio(0, 1, 1, 1).unwrap()
        );
        assert!(owner
            .trials_current(&plan, &program, view)
            .unwrap()
            .iter()
            .any(|trial| trial.gap() == Some(Gap::WorkBudget)));
        assert!(matches!(
            use_current(&plan, &program, &owner, 0, view),
            Err(Error::WorkBudget)
        ));
    });
}

#[test]
fn tlcx22_acomp_output_and_program_bounds_are_checked_before_success() {
    fixture(1, |fx, cases, view, _cancel| {
        let plan = Plan::given(fx.input(cases), limits(64), view).unwrap();
        let program = plan.permutation(&map(), &inverse(), view).unwrap();
        assert!(matches!(
            measure(&plan, &program, &full(), view),
            Err(Error::Resource(ResourceError::Exhausted(
                ResourceKind::Output
            )))
        ));
    });
    fixture(1024 * 1024, |fx, cases, view, _cancel| {
        let plan = Plan::given(fx.input(cases), limits(64), view).unwrap();
        let mut broken = map();
        broken[2].target = U;
        assert!(matches!(
            plan.permutation(&broken, &inverse(), view),
            Err(Error::InvalidInput)
        ));
        let too_small = Limits {
            max_program_steps: 2,
            ..limits(64)
        };
        let narrow = Plan::given(fx.input(cases), too_small, view).unwrap();
        assert!(matches!(
            narrow.permutation(&map(), &inverse(), view),
            Err(Error::ProgramBudget)
        ));
    });
}
