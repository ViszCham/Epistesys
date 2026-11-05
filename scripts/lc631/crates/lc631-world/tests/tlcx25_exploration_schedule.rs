//! Preparatory contract tests; execution is delegated to parent integration.
use exploration_schedule::priority::*;
use exploration_schedule::*;
use lc631_core::measurement_numeric::{BigInt, Rational as BigRational};
use lc631_world::exploration_schedule;

fn q(n: i64, d: i64) -> BigRational {
    BigRational::new(BigInt::from(n), BigInt::from(d))
}
fn point(n: i64, d: i64) -> UnitInterval {
    UnitInterval::checked(q(n, d), q(n, d)).unwrap()
}
fn binding() -> ScheduleBinding {
    ScheduleBinding {
        comparison_scope: [1; 32],
        selection_policy: [2; 32],
        budget_series: [3; 32],
        novelty_policy: SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1,
    }
}
fn scheduler(slots: u64, attempts: u32) -> ExplorationScheduler {
    ExplorationScheduler::new(binding(), slots, attempts, [2, 10, 2], 100).unwrap()
}
fn input(u: i64, n: i64) -> PriorityInput {
    PriorityInput::Discrete {
        uncertainty: point(u, 1),
        novelty: point(n, 1),
    }
}
fn work<'a>(key: &'a [u8], priority: &'a PriorityInput, cost: u64) -> SchedulingProposal<'a> {
    SchedulingProposal {
        binding: binding(),
        canonical_key: key,
        priority,
        estimated_query_slots: cost,
        demand: [1, 0, 1],
    }
}
fn ticket(step: ScheduleStep) -> SchedulingTicket {
    match step {
        ScheduleStep::Proposed(ticket) => ticket,
        ScheduleStep::Stopped(reason) => panic!("unexpected stop: {reason:?}"),
    }
}

#[test]
fn tlcx25_priority_hand_golden_and_interval_bounds() {
    let value = PriorityInput::Discrete {
        uncertainty: UnitInterval::checked(q(0, 1), q(1, 1)).unwrap(),
        novelty: UnitInterval::checked(q(1, 2), q(1, 1)).unwrap(),
    }
    .priority(4, 8)
    .unwrap()
    .unwrap();
    // Hand arithmetic: [1*3/2/(3/2), 2*2/(3/2)] = [1,8/3].
    assert_eq!(value.lower, q(1, 1));
    assert_eq!(value.upper, q(8, 3));
    assert_eq!(input(0, 0).priority(9, 8).unwrap().unwrap().lower, q(1, 2));
    assert!(UnitInterval::checked(q(-1, 10), q(1, 1)).is_err());
    assert!(UnitInterval::checked(q(1, 1), q(1, 2)).is_err());
    assert!(input(0, 0).priority(0, 0).is_err());
}

#[test]
fn tlcx25_priority_lower_bound_not_midpoint() {
    let uncertain = PriorityInput::Discrete {
        uncertainty: UnitInterval::checked(q(0, 1), q(1, 1)).unwrap(),
        novelty: point(0, 1),
    };
    let certain = PriorityInput::Discrete {
        uncertainty: point(1, 4),
        novelty: point(0, 1),
    };
    let batch = [work(b"a", &uncertain, 0), work(b"b", &certain, 0)];
    let mut scheduler = scheduler(8, 8);
    let first = ticket(scheduler.propose(&batch, [4; 32], 1, false).unwrap());
    scheduler.finish(&first, true).unwrap();
    let next = ticket(scheduler.propose(&batch, [4; 32], 2, false).unwrap());
    assert_eq!(next.canonical_key(), b"b");
    assert_eq!(next.reason, SelectionReason::PriorityLowerBound);
}

#[test]
fn tlcx25_cost_and_uncertainty_causally_change_next_query_proposal() {
    let low = input(0, 0);
    let high = input(1, 0);
    for (u, cost, expected) in [(&high, 0, b"b".as_slice()), (&low, 8, b"a".as_slice())] {
        let batch = [work(b"a", &low, 0), work(b"b", u, cost)];
        let mut scheduler = scheduler(8, 8);
        let first = ticket(scheduler.propose(&batch, [4; 32], 1, false).unwrap());
        scheduler.finish(&first, true).unwrap();
        let next = ticket(scheduler.propose(&batch, [4; 32], 2, false).unwrap());
        assert_eq!(next.canonical_key(), expected);
    }
}

#[test]
fn tlcx25_g35_novelty_policy_binding_and_rank_reversal() {
    for (policy, na, nb, expected) in [
        (
            SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1,
            q(1, 2),
            q(0, 1),
            b"a".as_slice(),
        ),
        (
            SchedulerNoveltyPolicyId::NearestVerifiedCandidateV1,
            q(0, 1),
            q(1, 2),
            b"b".as_slice(),
        ),
    ] {
        let a = PriorityInput::Discrete {
            uncertainty: point(0, 1),
            novelty: UnitInterval::checked(na.clone(), na).unwrap(),
        };
        let b = PriorityInput::Discrete {
            uncertainty: point(0, 1),
            novelty: UnitInterval::checked(nb.clone(), nb).unwrap(),
        };
        let mut bound = binding();
        bound.novelty_policy = policy;
        let mut scheduler = ExplorationScheduler::new(bound, 8, 8, [2, 10, 2], 100).unwrap();
        let mut batch = [work(b"a", &a, 0), work(b"b", &b, 0)];
        for proposal in &mut batch {
            proposal.binding = bound;
        }
        let first = ticket(scheduler.propose(&batch, [4; 32], 1, false).unwrap());
        scheduler.finish(&first, true).unwrap();
        assert_eq!(
            ticket(scheduler.propose(&batch, [4; 32], 2, false).unwrap()).canonical_key(),
            expected
        );
        batch[0].binding.novelty_policy =
            if policy == SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1 {
                SchedulerNoveltyPolicyId::NearestVerifiedCandidateV1
            } else {
                SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1
            };
        assert!(matches!(
            scheduler.propose(&batch, [4; 32], 3, false),
            Err(ScheduleError::BindingMismatch)
        ));
    }
}

#[test]
fn tlcx25_cursor_continues_after_deleted_key_across_frontier_revision() {
    let missing = PriorityInput::Missing(MissingPriorityReason::Unsupported);
    let mut scheduler = scheduler(8, 8);
    let first_batch = [work(b"b", &missing, 0), work(b"a", &missing, 0)];
    let first = ticket(scheduler.propose(&first_batch, [1; 32], 1, false).unwrap());
    assert_eq!(first.canonical_key(), b"a");
    assert_eq!(first.fairness_anchor, FairnessAnchor::Unanchored);
    scheduler.finish(&first, true).unwrap();
    let second_batch = [work(b"c", &missing, 0), work(b"b", &missing, 0)];
    let second = ticket(scheduler.propose(&second_batch, [2; 32], 2, false).unwrap());
    assert_eq!(second.canonical_key(), b"b");
    assert_eq!(second.reason, SelectionReason::UndefinedInputRoundRobin);
    assert_eq!(scheduler.fairness().frontier_revision, [2; 32]);
    assert_eq!(scheduler.fairness().anchor, FairnessAnchor::LocalCursorOnly);
}

#[test]
fn tlcx25_cheap_branch_does_not_consume_reserved_exploration_slot() {
    let low = input(0, 0);
    let high = input(1, 1);
    let batch = [work(b"b", &low, 8), work(b"a", &high, 0)];
    let mut scheduler = scheduler(9, 9);
    for slot in 0..9 {
        let next = ticket(scheduler.propose(&batch, [1; 32], slot + 1, false).unwrap());
        if slot == 8 {
            assert_eq!(next.reason, SelectionReason::ReservedExploration);
            assert_eq!(next.canonical_key(), b"b");
        }
        scheduler.finish(&next, true).unwrap();
    }
    assert!(matches!(
        scheduler.propose(&batch, [1; 32], 10, false).unwrap(),
        ScheduleStep::Stopped(StopReason::BudgetExhausted)
    ));
}

#[test]
fn tlcx25_unknown_retry_is_bounded_and_cleanup_unknown_retains_demand() {
    let missing = PriorityInput::Missing(MissingPriorityReason::Unknown);
    let batch = [work(b"a", &missing, 0)];
    let mut scheduler = scheduler(8, 1);
    let first = ticket(scheduler.propose(&batch, [1; 32], 1, false).unwrap());
    assert!(matches!(
        scheduler.finish(&first, false),
        Err(ScheduleError::CleanupUnobserved)
    ));
    assert!(matches!(
        scheduler.propose(&batch, [1; 32], 2, false).unwrap(),
        ScheduleStep::Stopped(StopReason::NoAdmissibleWork)
    ));
    scheduler.finish(&first, true).unwrap();
    assert!(matches!(
        scheduler.finish(&first, true),
        Err(ScheduleError::StaleTicket)
    ));
    assert!(matches!(
        scheduler.propose(&batch, [1; 32], 3, false).unwrap(),
        ScheduleStep::Stopped(StopReason::NoAdmissibleWork)
    ));
}

#[test]
fn tlcx25_local_capacity_cancel_deadline_and_clock_fail_closed() {
    let value = input(1, 1);
    let mut batch = [work(b"a", &value, 0), work(b"b", &value, 0)];
    for work in &mut batch {
        work.demand = [2, 10, 2];
    }
    let mut scheduler = scheduler(8, 8);
    let first = ticket(scheduler.propose(&batch, [1; 32], 1, false).unwrap());
    assert!(matches!(
        scheduler.propose(&batch, [1; 32], 2, false).unwrap(),
        ScheduleStep::Stopped(StopReason::NoAdmissibleWork)
    ));
    scheduler.finish(&first, true).unwrap();
    assert!(matches!(
        scheduler.propose(&batch, [1; 32], 3, true).unwrap(),
        ScheduleStep::Stopped(StopReason::Cancelled)
    ));
    assert!(matches!(
        scheduler.propose(&batch, [1; 32], 2, false),
        Err(ScheduleError::ClockRegression)
    ));
    assert!(matches!(
        scheduler.propose(&batch, [1; 32], 100, false).unwrap(),
        ScheduleStep::Stopped(StopReason::Cancelled)
    ));
    let mut fresh = ExplorationScheduler::new(binding(), 8, 8, [2, 10, 2], 100).unwrap();
    assert!(matches!(
        fresh.propose(&batch, [1; 32], 100, false).unwrap(),
        ScheduleStep::Stopped(StopReason::Deadline)
    ));
}

#[test]
fn tlcx25_duplicate_groups_and_input_permutation_do_not_inflate_attempts() {
    let a = input(0, 0);
    let b = input(1, 1);
    let batch = [work(b"b", &b, 0), work(b"a", &a, 0), work(b"a", &a, 0)];
    let mut scheduler = scheduler(8, 1);
    let first = ticket(scheduler.propose(&batch, [1; 32], 1, false).unwrap());
    assert_eq!(first.canonical_key(), b"a");
    scheduler.finish(&first, true).unwrap();
    assert_eq!(
        ticket(scheduler.propose(&batch, [1; 32], 2, false).unwrap()).canonical_key(),
        b"b"
    );
    let mut fresh = ExplorationScheduler::new(binding(), 8, 8, [2, 10, 2], 100).unwrap();
    let conflict = [work(b"a", &a, 0), work(b"a", &b, 0)];
    assert!(matches!(
        fresh.propose(&conflict, [1; 32], 1, false),
        Err(ScheduleError::ConflictingDuplicate)
    ));
    assert_eq!(fresh.fairness().slots_consumed, 0);
}

#[test]
fn tlcx25_same_binding_foreign_instance_ticket_cannot_release_demand() {
    let value = input(0, 0);
    let batch = [work(b"a", &value, 0)];
    let mut first = scheduler(8, 8);
    let mut second = scheduler(8, 8);
    let a = ticket(first.propose(&batch, [1; 32], 1, false).unwrap());
    let b = ticket(second.propose(&batch, [1; 32], 1, false).unwrap());
    assert!(matches!(
        second.finish(&a, true),
        Err(ScheduleError::BindingMismatch)
    ));
    second.finish(&b, true).unwrap();
}
