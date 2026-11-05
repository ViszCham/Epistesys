//! Negative vectors authored before the new owned numeric consumer.
#[path = "tlcx23_formal_support/mod.rs"]
mod support;
use lc631_core::measurement_numeric::RationalInterval as Interval;
use lc631_core::CandidateId;
use lc631_runtime::formal_measurement::{self as m, owned};
use lc631_runtime::joint_selection::formal::{
    reduce_formal_owned, REDUCTION_RETAINED_OUTPUT, REDUCTION_RETAINED_RAM, REDUCTION_SCRATCH_RAM,
};
use lc631_runtime::joint_selection::*;
use lc631_runtime::semantic_accelerator::selection::*;
use lc631_substrate::resource::{Cancellation, ResourceError};
use support::*;

fn epoch() -> EpochStamp {
    EpochStamp {
        epoch: 1,
        generation: 2,
        fence: 3,
    }
}
fn reborrow<'v>(row: &JointCandidate<'v>, axes: &'v [AxisValue<'v>]) -> JointCandidate<'v> {
    let total_loss = match &row.total_loss {
        AxisValue::Exact(v) => AxisValue::Exact(v),
        AxisValue::Bounded(v) => AxisValue::Bounded(v),
        AxisValue::Estimated(v) => AxisValue::Estimated(v),
        AxisValue::Unavailable(reason) => AxisValue::Unavailable(*reason),
    };
    JointCandidate {
        id: row.id,
        context: row.context,
        pins: row.pins,
        hard_gate: row.hard_gate,
        map_back: row.map_back,
        axes,
        total_loss,
        canonical_key: row.canonical_key,
        measurement_binding: row.measurement_binding,
    }
}

#[test]
fn tlcx33_actual_owned_measurement_consumer_cut_and_restore() {
    cases(|c| {
        // Preserve simultaneous original/cut/restored owners. The shared setup
        // reserves only 1MiB Output: five owned losses + three reduced axes +
        // three final selections exceed that. No production quota is changed.
        let retained = owned::requirements(c.plan).unwrap();
        let selection_retained = 256 * 1024;
        let output =
            5 * retained.retained_output + 3 * REDUCTION_RETAINED_OUTPUT + 3 * selection_retained;
        // Conservative aggregate: all retained values/batches/reports plus
        // every distinct scratch profile, even though scratch is sequential.
        let ram = 5 * retained.retained_ram
            + 3 * REDUCTION_RETAINED_RAM
            + 6 * selection_retained
            + retained.scratch_ram
            + REDUCTION_SCRATCH_RAM
            + 2 * 1024 * 1024;
        assert!(ram + output <= 128 * 1024 * 1024);
        let _broker = broker();
        let _run = run(
            &_broker,
            c.plan.scope().source.digest(),
            Cancellation::default(),
        );
        let reservation = reserve(&_run, ram, output);
        let view = reservation.lease();
        let gs = owned::measure_owned(
            c.plan,
            c.good,
            m::Direction::Soundness,
            c.good_obs,
            work(),
            &view,
        )
        .unwrap();
        let gc = owned::measure_owned(
            c.plan,
            c.good,
            m::Direction::Completeness,
            c.good_obs,
            work(),
            &view,
        )
        .unwrap();
        let bs = owned::measure_owned(
            c.plan,
            c.bad,
            m::Direction::Soundness,
            c.bad_obs,
            work(),
            &view,
        )
        .unwrap();
        let bc = owned::measure_owned(
            c.plan,
            c.bad,
            m::Direction::Completeness,
            c.bad_obs,
            work(),
            &view,
        )
        .unwrap();
        let one = Interval::exact_ratio(1, 1).unwrap();
        let zero = Interval::exact_ratio(0, 1).unwrap();
        let reduction = policy(pins(c.plan), one.lower(), one.lower(), &view);
        let pins = reduction.pins();
        let good = reduce_formal_owned(&reduction, &gs, &gc, c.plan, c.good, &view).unwrap();
        let bad = reduce_formal_owned(&reduction, &bs, &bc, c.plan, c.bad, &view).unwrap();
        let (semantic, gv) = good
            .project_current(&reduction, pins, c.plan, c.good, &view)
            .unwrap();
        let (_, bv) = bad
            .project_current(&reduction, pins, c.plan, c.bad, &view)
            .unwrap();
        let ga = [AxisValue::Exact(&one), gv];
        let ba = [AxisValue::Exact(&one), bv];
        let axes = [AxisKind::Goal(d(40)), semantic];
        let selection = SelectionPolicy {
            pins,
            axes: &axes,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 2,
            max_canonical_bytes: 4096,
        };
        let rows = [
            JointCandidate {
                id: CandidateId(4),
                context: c.good.context(),
                pins,
                hard_gate: ReportedGate::Pass,
                map_back: ReportedGate::Pass,
                axes: &ga,
                total_loss: AxisValue::Exact(&zero),
                canonical_key: b"good",
                measurement_binding: d(4),
            },
            JointCandidate {
                id: CandidateId(5),
                context: c.bad.context(),
                pins,
                hard_gate: ReportedGate::Pass,
                map_back: ReportedGate::Pass,
                axes: &ba,
                total_loss: AxisValue::Exact(&zero),
                canonical_key: b"bad",
                measurement_binding: d(5),
            },
        ];
        let batch = prepare_numeric_batch(
            &selection,
            &rows,
            SearchCoverage::EvaluatedSet,
            epoch(),
            &view,
            8,
        )
        .unwrap();
        // Full-vector mismatch fails before any selection publication.
        let wrong = FrontierCandidate {
            token: batch.token(),
            stamp: epoch(),
            retained: &[true, true],
        };
        assert!(matches!(
            finish_selection(&batch, Some(wrong), &view),
            Err(ConsumerError::VectorMismatch { row: 1 })
        ));
        let expected = independent_cpu_frontier(&batch).unwrap();
        assert_eq!(expected, [true, false]);
        let good_proposal = FrontierCandidate {
            token: batch.token(),
            stamp: epoch(),
            retained: &expected,
        };
        let final_report = finish_selection(&batch, Some(good_proposal), &view).unwrap();
        let report = final_report.project_current(&view).unwrap();
        assert_eq!(report.frontier, [0]);
        assert_eq!(report.recommendation, Some(0));
        assert_eq!(report.alternatives, [1]);
        assert_eq!(final_report.gpu_state(), GpuState::Unavailable);
        // Cut the real completeness observation, retaining fixed probe/denominator.
        let absent = owned::measure_owned(
            c.plan,
            c.bad,
            m::Direction::Completeness,
            &[],
            work(),
            &view,
        )
        .unwrap();
        let cut_axis = reduce_formal_owned(&reduction, &bs, &absent, c.plan, c.bad, &view).unwrap();
        let (_, cv) = cut_axis
            .project_current(&reduction, pins, c.plan, c.bad, &view)
            .unwrap();
        let ca = [AxisValue::Exact(&one), cv];
        let cut_rows = [reborrow(&rows[0], &ga), reborrow(&rows[1], &ca)];
        let cut = prepare_numeric_batch(
            &selection,
            &cut_rows,
            SearchCoverage::EvaluatedSet,
            epoch(),
            &view,
            8,
        )
        .unwrap();
        assert_ne!(cut.token(), batch.token());
        let cut_report = finish_selection(&cut, None, &view).unwrap();
        assert_eq!(cut_report.project_current(&view).unwrap().frontier, [0, 1]);
        let restored = [reborrow(&rows[0], &ga), reborrow(&rows[1], &ba)];
        let restore = prepare_numeric_batch(
            &selection,
            &restored,
            SearchCoverage::EvaluatedSet,
            epoch(),
            &view,
            8,
        )
        .unwrap();
        assert_eq!(
            finish_selection(&restore, None, &view)
                .unwrap()
                .project_current(&view)
                .unwrap()
                .frontier,
            [0]
        );
    });
}

#[test]
fn tlcx33_numeric_consumer_stale_partial_unknown_and_cancel_cuts() {
    cases(|c| {
        let cancel = Cancellation::default();
        let (_broker, _run, reservation) = setup(&c, cancel.clone());
        let view = reservation.lease();
        let pins = pins(c.plan);
        let one = Interval::exact_ratio(1, 1).unwrap();
        let axes = [AxisKind::Goal(d(40))];
        let policy = SelectionPolicy {
            pins,
            axes: &axes,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 1,
            max_canonical_bytes: 4096,
        };
        let av = [AxisValue::Exact(&one)];
        let rows = [JointCandidate {
            id: CandidateId(1),
            context: c.good.context(),
            pins,
            hard_gate: ReportedGate::Pass,
            map_back: ReportedGate::Pass,
            axes: &av,
            total_loss: AxisValue::Exact(&one),
            canonical_key: b"candidate",
            measurement_binding: d(1),
        }];
        assert!(matches!(
            prepare_numeric_batch(
                &policy,
                &rows,
                SearchCoverage::EvaluatedSet,
                epoch(),
                &view,
                0
            ),
            Err(ConsumerError::WorkLimit)
        ));
        let batch = prepare_numeric_batch(
            &policy,
            &rows,
            SearchCoverage::EvaluatedSet,
            epoch(),
            &view,
            1,
        )
        .unwrap();
        let mut stale = epoch();
        stale.fence += 1;
        for (token, stamp) in [(d(99), epoch()), (batch.token(), stale)] {
            assert!(matches!(
                finish_selection(
                    &batch,
                    Some(FrontierCandidate {
                        token,
                        stamp,
                        retained: &[true]
                    }),
                    &view
                ),
                Err(ConsumerError::StaleCandidate)
            ));
        }
        assert!(matches!(
            finish_selection(
                &batch,
                Some(FrontierCandidate {
                    token: batch.token(),
                    stamp: epoch(),
                    retained: &[]
                }),
                &view
            ),
            Err(ConsumerError::PartialVector)
        ));
        let missing = [AxisValue::Unavailable(MissingAxis::Unknown)];
        let pending = [reborrow(&rows[0], &missing)];
        assert!(matches!(
            prepare_numeric_batch(
                &policy,
                &pending,
                SearchCoverage::EvaluatedSet,
                epoch(),
                &view,
                1
            ),
            Err(ConsumerError::MissingNumeric {
                row: 0,
                axis: 0,
                reason: MissingAxis::Unknown
            })
        ));
        let result = finish_selection(&batch, None, &view).unwrap();
        cancel.cancel();
        assert!(matches!(
            result.project_current(&view),
            Err(ConsumerError::Resource(ResourceError::Cancelled))
        ));
        assert!(matches!(
            finish_selection(&batch, None, &view),
            Err(ConsumerError::Resource(ResourceError::Cancelled))
        ));
    });
}
