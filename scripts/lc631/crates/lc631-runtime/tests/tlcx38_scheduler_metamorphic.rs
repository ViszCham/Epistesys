//! Pure scheduler/geometry proposals are not solver executions or evidence.
use lc631_analysis::creativity_measurement::{set_distance, Feature, MeasurementError};
use lc631_core::measurement_numeric::{min_enclosure, BigInt, Rational, RationalInterval};
use lc631_world::curved_exploration::{self as geometry, BallPolicy, GeometryUnavailable};
use lc631_world::exploration_schedule::{priority::*, *};

fn q(n: i64, d: i64) -> Rational {
    Rational::new(BigInt::from(n), BigInt::from(d))
}
fn interval(n: i64, d: i64) -> RationalInterval {
    RationalInterval::exact_ratio(n, d).unwrap()
}
fn point(n: i64, d: i64) -> UnitInterval {
    UnitInterval::checked(q(n, d), q(n, d)).unwrap()
}
fn binding(byte: u8) -> ScheduleBinding {
    ScheduleBinding {
        comparison_scope: [byte; 32],
        selection_policy: [byte + 1; 32],
        budget_series: [byte + 2; 32],
        novelty_policy: SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1,
    }
}
fn proposal<'a>(
    binding: ScheduleBinding,
    key: &'a [u8],
    priority: &'a PriorityInput,
) -> SchedulingProposal<'a> {
    SchedulingProposal {
        binding,
        canonical_key: key,
        priority,
        estimated_query_slots: 0,
        demand: [1, 0, 1],
    }
}
fn ticket(step: ScheduleStep) -> SchedulingTicket {
    match step {
        ScheduleStep::Proposed(ticket) => ticket,
        ScheduleStep::Stopped(why) => panic!("{why:?}"),
    }
}

#[test]
fn tlcx38_scheduler_permutation_and_opaque_binding_relabel_preserve_content_order() {
    let missing = PriorityInput::Missing(MissingPriorityReason::Unknown);
    let mut observed = Vec::new();
    for (tag, order) in [(1, [b"c", b"a", b"b"]), (9, [b"b", b"c", b"a"])] {
        let pins = binding(tag);
        let batch: Vec<_> = order
            .iter()
            .map(|key| proposal(pins, key.as_slice(), &missing))
            .collect();
        let mut scheduler = ExplorationScheduler::new(pins, 3, 3, [1, 0, 1], 100).unwrap();
        let mut keys = Vec::new();
        for tick in 1..=3 {
            let t = ticket(
                scheduler
                    .propose(&batch, [tick as u8; 32], tick, false)
                    .unwrap(),
            );
            assert_eq!(t.reason, SelectionReason::UndefinedInputRoundRobin);
            keys.push(t.canonical_key().to_vec());
            scheduler.finish(&t, true).unwrap();
        }
        assert!(matches!(
            scheduler.propose(&batch, [4; 32], 4, false).unwrap(),
            ScheduleStep::Stopped(StopReason::BudgetExhausted)
        ));
        observed.push(keys);
    }
    assert_eq!(
        observed[0],
        vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]
    );
    assert_eq!(observed[0], observed[1]);
}

#[test]
fn tlcx38_unknown_cleanup_and_foreign_ticket_cuts_block_then_restore_local_dispatch() {
    let pins = binding(1);
    let priority = PriorityInput::Missing(MissingPriorityReason::NeedsEvidence);
    let batch = [proposal(pins, b"only", &priority)];
    let mut scheduler = ExplorationScheduler::new(pins, 4, 4, [1, 0, 1], 100).unwrap();
    let mut foreign = ExplorationScheduler::new(pins, 4, 4, [1, 0, 1], 100).unwrap();
    let t = ticket(scheduler.propose(&batch, [1; 32], 1, false).unwrap());
    assert_eq!(
        foreign.finish(&t, true),
        Err(ScheduleError::BindingMismatch)
    );
    assert_eq!(
        scheduler.finish(&t, false),
        Err(ScheduleError::CleanupUnobserved)
    );
    assert!(matches!(
        scheduler.propose(&batch, [2; 32], 2, false).unwrap(),
        ScheduleStep::Stopped(StopReason::NoAdmissibleWork)
    ));
    scheduler.finish(&t, true).unwrap();
    let restored = ticket(scheduler.propose(&batch, [3; 32], 3, false).unwrap());
    assert_eq!(restored.canonical_key(), b"only");
    assert_eq!(scheduler.finish(&t, true), Err(ScheduleError::StaleTicket));
    scheduler.finish(&restored, true).unwrap();
}

#[test]
fn tlcx38_duplicate_conflict_cancel_and_clock_regression_never_increase_budget() {
    let pins = binding(1);
    let low = PriorityInput::Discrete {
        uncertainty: point(0, 1),
        novelty: point(0, 1),
    };
    let high = PriorityInput::Discrete {
        uncertainty: point(1, 1),
        novelty: point(0, 1),
    };
    let conflict = [
        proposal(pins, b"same", &low),
        proposal(pins, b"same", &high),
    ];
    let mut scheduler = ExplorationScheduler::new(pins, 4, 4, [1, 0, 1], 100).unwrap();
    assert!(matches!(
        scheduler.propose(&conflict, [1; 32], 1, false),
        Err(ScheduleError::ConflictingDuplicate)
    ));
    assert_eq!(scheduler.fairness().slots_consumed, 0);
    let valid = [proposal(pins, b"same", &low), proposal(pins, b"same", &low)];
    let t = ticket(scheduler.propose(&valid, [2; 32], 2, false).unwrap());
    assert_eq!(scheduler.fairness().slots_consumed, 1);
    scheduler.finish(&t, true).unwrap();
    assert!(matches!(
        scheduler.propose(&valid, [3; 32], 3, true).unwrap(),
        ScheduleStep::Stopped(StopReason::Cancelled)
    ));
    assert!(matches!(
        scheduler.propose(&valid, [4; 32], 4, false).unwrap(),
        ScheduleStep::Stopped(StopReason::Cancelled)
    ));
    assert!(matches!(
        scheduler.propose(&valid, [1; 32], 1, false),
        Err(ScheduleError::ClockRegression)
    ));
    assert_eq!(scheduler.fairness().slots_consumed, 1);
}

#[test]
fn tlcx38_g35_actual_k3_policies_reverse_the_second_dispatch_without_changing_default() {
    let one = q(1, 1);
    let universe = [
        Feature {
            canonical_payload: b"a",
            weight: &one,
        },
        Feature {
            canonical_payload: b"b",
            weight: &one,
        },
    ];
    let a: &[&[u8]] = &[b"a"];
    let b: &[&[u8]] = &[b"b"];
    let union: &[&[u8]] = &[b"a", b"b"];
    let union_a = set_distance(&universe, a, union).unwrap();
    let union_b = set_distance(&universe, union, union).unwrap();
    let near_a = min_enclosure(&[
        set_distance(&universe, a, a).unwrap(),
        set_distance(&universe, a, b).unwrap(),
    ])
    .unwrap();
    let near_b = min_enclosure(&[
        set_distance(&universe, union, a).unwrap(),
        set_distance(&universe, union, b).unwrap(),
    ])
    .unwrap();
    assert_eq!(
        (&union_a, &union_b, &near_a, &near_b),
        (
            &interval(1, 2),
            &interval(0, 1),
            &interval(0, 1),
            &interval(1, 2)
        )
    );
    for (policy, na, nb, expected) in [
        (
            SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1,
            union_a,
            union_b,
            b"a",
        ),
        (
            SchedulerNoveltyPolicyId::NearestVerifiedCandidateV1,
            near_a,
            near_b,
            b"b",
        ),
    ] {
        let pins = ScheduleBinding {
            novelty_policy: policy,
            ..binding(1)
        };
        let pa = PriorityInput::Discrete {
            uncertainty: point(0, 1),
            novelty: UnitInterval::checked(na.lower().clone(), na.upper().clone()).unwrap(),
        };
        let pb = PriorityInput::Discrete {
            uncertainty: point(0, 1),
            novelty: UnitInterval::checked(nb.lower().clone(), nb.upper().clone()).unwrap(),
        };
        let batch = [proposal(pins, b"a", &pa), proposal(pins, b"b", &pb)];
        let mut scheduler = ExplorationScheduler::new(pins, 4, 4, [1, 0, 1], 100).unwrap();
        let first = ticket(scheduler.propose(&batch, [1; 32], 1, false).unwrap());
        scheduler.finish(&first, true).unwrap();
        let second = ticket(scheduler.propose(&batch, [2; 32], 2, false).unwrap());
        assert_eq!(second.canonical_key(), expected);
        scheduler.finish(&second, true).unwrap();
    }
    assert!(matches!(
        set_distance(&universe, &[b"new"], a),
        Err(MeasurementError::UniverseMismatch)
    ));
    assert_eq!(
        geometry::SchedulerNoveltyPolicyId::default(),
        geometry::SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1
    );
}

#[test]
fn tlcx38_g34_g32_coordinate_permutation_keeps_nonnegative_square_and_poincare_t() {
    let crossing = RationalInterval::from_ratio(-1, 10, 1, 10).unwrap();
    assert_eq!(
        crossing.square().unwrap(),
        RationalInterval::from_ratio(0, 1, 1, 100).unwrap()
    );
    let policy = BallPolicy::new(q(1, 1), q(1, 4), q(1, 1), 4).unwrap();
    let x = [interval(1, 2), interval(0, 1)];
    let y = [interval(0, 1), interval(1, 2)];
    let a = geometry::prepare_poincare(&policy, &x, &y).unwrap();
    let b = geometry::prepare_poincare(
        &policy,
        &[x[1].clone(), x[0].clone()],
        &[y[1].clone(), y[0].clone()],
    )
    .unwrap();
    // Hand rational t=(1/2)/((3/4)*(3/4))=8/9; this is not a distance/embedding proof.
    assert_eq!(a.t_enclosure(), &interval(8, 9));
    assert_eq!(a.t_enclosure(), b.t_enclosure());
    assert_eq!(
        geometry::scheduling_readiness(None, 1),
        GeometryUnavailable::EmbeddingPolicyNotRegistered
    );
    assert_eq!(
        geometry::scheduling_readiness(None, 0),
        GeometryUnavailable::EmptyVerifiedFrontier
    );
}
