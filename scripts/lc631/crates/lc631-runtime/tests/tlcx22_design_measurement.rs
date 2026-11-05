mod tlcx22_design_support;
use lc631_analysis::creativity_measurement::{
    ComponentState, IndependenceStatus, InputBasis, SemanticValidation,
};
use lc631_core::measurement_numeric::RationalInterval;
use lc631_runtime::design_measurement::*;
use lc631_substrate::resource::Cancellation;
use tlcx22_design_support::*;
#[test]
fn tlcx22_design_actual_program_gold_and_feasible_wrong_priority_replay() {
    let f = Fixture::gold();
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let suite = register(binding(), &[f.workload(b"case")], limits(), &view).unwrap();
    assert_eq!(
        suite.baseline_observation(0).unwrap().raw.ticks,
        ModelTicks(5)
    );
    assert_eq!(
        suite.baseline_observation(0).unwrap().score,
        RationalInterval::exact_ratio(1, 3).unwrap()
    );
    let order = improved();
    let frozen = bind_candidate(
        &suite,
        candidate(1),
        &[PriorityProgram {
            case: b"case",
            order: &order,
        }],
        &view,
    )
    .unwrap();
    let owner = execute(&frozen, &view).unwrap();
    assert_eq!(
        owner.candidate_observation(0).unwrap().raw.ticks,
        ModelTicks(4)
    );
    assert_eq!(
        owner.historical_report().state,
        ComponentState::Measured(RationalInterval::exact_ratio(1, 3).unwrap())
    );
    assert!(matches!(
        replay_reported(
            &suite,
            b"case",
            &f.baseline,
            owner.candidate_schedule(0).unwrap(),
            &view
        ),
        Err(Error::ReplayMismatch)
    ));
    assert_eq!(owner.historical_report().basis, InputBasis::Given);
    assert_eq!(
        owner.historical_report().validation,
        SemanticValidation::Unassessed
    );
    assert_eq!(
        owner.historical_report().independence,
        IndependenceStatus::SharedDependency
    );
}
#[test]
fn tlcx22_design_raw_accepted_range_precedes_saturation_and_boundaries() {
    for (m, upper, accepted, flag, score, valid) in [
        (1, 4, 20, Saturation::SaturatedHigh, 0, true),
        (6, 9, 20, Saturation::SaturatedLow, 1, true),
        (5, 8, 20, Saturation::None, 1, true),
        (2, 5, 20, Saturation::None, 0, true),
        (1, 4, 4, Saturation::SaturatedHigh, 0, false),
    ] {
        let mut f = Fixture::gold();
        f.m = q(m, 1);
        f.upper = q(upper, 1);
        f.accepted_high = q(accepted, 1);
        let (_br, _run, res) = run(Cancellation::default());
        let view = res.lease();
        let result = register(binding(), &[f.workload(b"case")], limits(), &view);
        if !valid {
            assert!(matches!(result, Err(Error::OutsideAcceptedRange)));
            continue;
        }
        let suite = result.unwrap();
        let observed = suite.baseline_observation(0).unwrap();
        assert_eq!(observed.raw.ticks, ModelTicks(5));
        assert_eq!(observed.saturation, flag);
        assert_eq!(
            observed.score,
            RationalInterval::exact_ratio(score, 1).unwrap()
        );
    }
}
#[test]
fn tlcx22_design_empty_invalid_program_and_baseline_budget_never_fabricate_score() {
    let f = Fixture::gold();
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    assert!(matches!(
        register(binding(), &[], limits(), &view),
        Err(Error::EmptySuite)
    ));
    let mut zero = limits();
    zero.max_events_per_run = 0;
    assert!(matches!(
        register(binding(), &[f.workload(b"case")], zero, &view),
        Err(Error::PrerequisitePending)
    ));
    let suite = register(binding(), &[f.workload(b"case")], limits(), &view).unwrap();
    for order in [
        vec![JobId(11), JobId(22)],
        vec![JobId(11), JobId(22), JobId(22)],
        vec![JobId(11), JobId(22), JobId(99)],
    ] {
        assert!(matches!(
            bind_candidate(
                &suite,
                candidate(1),
                &[PriorityProgram {
                    case: b"case",
                    order: &order
                }],
                &view
            ),
            Err(Error::InvalidProgram)
        ));
    }
}

#[test]
fn tlcx22_design_existing_k2_uses_actual_weighted_positive_and_negative_gains() {
    let a = Fixture::gold();
    let mut b = Fixture::gold();
    b.baseline = improved().to_vec();
    b.weight = q(3, 1);
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let suite = register(
        binding(),
        &[a.workload(b"a"), b.workload(b"b")],
        limits(),
        &view,
    )
    .unwrap();
    let good = improved();
    let worse = [JobId(22), JobId(33), JobId(11)];
    let frozen = bind_candidate(
        &suite,
        candidate(1),
        &[
            PriorityProgram {
                case: b"a",
                order: &good,
            },
            PriorityProgram {
                case: b"b",
                order: &worse,
            },
        ],
        &view,
    )
    .unwrap();
    let owner = execute(&frozen, &view).unwrap();
    // Independent hand arithmetic: (1*(+1/3)+3*(-1/3))/(1+3)=-1/6.
    assert_eq!(
        owner.historical_report().state,
        ComponentState::Measured(RationalInterval::exact_ratio(-1, 6).unwrap())
    );
}

#[test]
fn tlcx22_design_scoped_k2_nodes_distinguish_phases_and_preserve_opaque_relabel() {
    let f = Fixture::gold();
    let base_node;
    {
        let (_br, _run, res) = run(Cancellation::default());
        let view = res.lease();
        let suite = register(binding(), &[f.workload(b"case")], limits(), &view).unwrap();
        base_node = suite.baseline_score_node(0).unwrap();
        let a = candidate(1);
        let fa = bind_candidate(
            &suite,
            a,
            &[PriorityProgram {
                case: b"case",
                order: &f.baseline,
            }],
            &view,
        )
        .unwrap();
        let oa = execute(&fa, &view).unwrap();
        assert_eq!(
            oa.historical_report().state,
            ComponentState::Measured(RationalInterval::exact_ratio(0, 1).unwrap())
        );
        assert_ne!(oa.candidate_score_node(0), Some(base_node));
        let mut relabel = a;
        relabel.id = lc631_core::CandidateId(9999);
        let fb = bind_candidate(
            &suite,
            relabel,
            &[PriorityProgram {
                case: b"case",
                order: &f.baseline,
            }],
            &view,
        )
        .unwrap();
        let ob = execute(&fb, &view).unwrap();
        assert_eq!(oa.candidate_score_node(0), ob.candidate_score_node(0));
        assert_eq!(oa.historical_report(), ob.historical_report());
        assert_eq!(suite.baseline_score_node(0), Some(base_node));
    }
    // Each actual baseline execution uses the changed header and its matching lease.
    for cut in 0..3 {
        let mut b = binding();
        if cut == 0 {
            b.oracle = d(201);
        }
        if cut == 1 {
            b.selection.scope =
                lc631_core::context::ContextScope::declared(d(202), d(2), d(3), d(4));
        }
        if cut == 2 {
            b.selection.source =
                lc631_core::context::SourceRevision::of("different scheduling source");
            b.context = lc631_core::context::ContextRevision::original(b.selection.source);
        }
        let (_br, _run, res) = run_binding(b, Cancellation::default());
        let view = res.lease();
        let suite = register(b, &[f.workload(b"case")], limits(), &view).unwrap();
        assert_ne!(suite.baseline_score_node(0), Some(base_node));
    }
}
