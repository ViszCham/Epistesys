//! Hand expectations for the actual in-process four-valued capability profile.
use lc631_analysis::creativity_measurement::{Rational, RationalInterval};
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::capability_measurement::*;
use lc631_runtime::resource::*;
use lc631_world::exploration_schedule::{
    priority::{PriorityInput, UnitInterval},
    ExplorationScheduler, ScheduleBinding, ScheduleStep, SchedulerNoveltyPolicyId,
    SchedulingProposal,
};
use std::time::Duration;

fn fixture(body: impl FnOnce(ContextRevision, &ResourceReservationView<'_>, &Cancellation)) {
    let source = SourceRevision::of("explicit four-valued capability test");
    let limits = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let cancel = Cancellation::default();
    let mut run = broker
        .begin_run(
            742,
            source.digest(),
            limits,
            Deadline::after(Duration::from_secs(120)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let mut lease = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(24 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(8 * 1024 * 1024).unwrap(),
            ),
        ])
        .unwrap();
    {
        let view = lease.lease();
        body(ContextRevision::original(source), &view, &cancel);
    }
    lease.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}
fn requirements() -> Vec<ProbeSpec> {
    vec![
        ProbeSpec {
            capability: Capability::Both,
            weight: Rational::from_integer(1.into()),
        },
        ProbeSpec {
            capability: Capability::NoExplosion,
            weight: Rational::from_integer(3.into()),
        },
    ]
}
fn zero() -> UnitInterval {
    UnitInterval::checked(
        Rational::from_integer(0.into()),
        Rational::from_integer(0.into()),
    )
    .unwrap()
}

#[test]
fn tlcx_modal_registration_alone_is_unknown_and_actual_execution_fixes_denominator() {
    fixture(|context, view, _| {
        let registry = Registry::capture(
            context,
            [5; 32],
            &[Capability::Both, Capability::NoExplosion],
            view,
        )
        .unwrap();
        let plan = Plan::capture(&registry, &requirements(), 100, view).unwrap();
        assert_eq!(
            plan.unobserved_bounds(),
            Some(
                &RationalInterval::new(
                    Rational::from_integer(0.into()),
                    Rational::from_integer(1.into())
                )
                .unwrap()
            )
        );
        let result = evaluate(&plan, view).unwrap();
        let current = result.current(&plan, &registry, view).unwrap();
        assert_eq!(
            current.bounds(),
            Some(&RationalInterval::exact_ratio(1, 1).unwrap())
        );
        assert_eq!(
            current.statuses(),
            &[ProbeStatus::Matched, ProbeStatus::Matched]
        );
        assert_eq!(current.actual_evaluation_count(), 2);
    });
}
#[test]
fn tlcx_modal_missing_unregistered_timeout_and_unsupported_remain_distinct() {
    fixture(|context, view, _| {
        let registry = Registry::capture(context, [5; 32], &[Capability::Both], view).unwrap();
        let plan = Plan::capture(&registry, &requirements(), 100, view).unwrap();
        let result = evaluate(&plan, view).unwrap();
        let current = result.current(&plan, &registry, view).unwrap();
        assert_eq!(
            current.statuses(),
            &[ProbeStatus::Matched, ProbeStatus::Unregistered]
        );
        assert_eq!(
            current.bounds(),
            Some(
                &RationalInterval::new(
                    Rational::new(1.into(), 4.into()),
                    Rational::from_integer(1.into())
                )
                .unwrap()
            )
        );
        let exhausted = Plan::capture(&registry, &requirements(), 1, view).unwrap();
        let exhausted_result = evaluate(&exhausted, view).unwrap();
        assert_eq!(
            exhausted_result
                .current(&exhausted, &registry, view)
                .unwrap()
                .statuses()[0],
            ProbeStatus::Unknown
        );
        let unsupported = [ProbeSpec {
            capability: Capability::Unsupported,
            weight: Rational::from_integer(1.into()),
        }];
        let unsupported_plan = Plan::capture(&registry, &unsupported, 100, view).unwrap();
        let unsupported_result = evaluate(&unsupported_plan, view).unwrap();
        assert_eq!(
            unsupported_result
                .current(&unsupported_plan, &registry, view)
                .unwrap()
                .statuses(),
            &[ProbeStatus::Unsupported]
        );
        assert!(unsupported_result
            .current(&unsupported_plan, &registry, view)
            .unwrap()
            .bounds()
            .is_none());
    });
}
#[test]
fn tlcx_modal_current_projection_rejects_foreign_plan_and_cancellation() {
    fixture(|context, view, cancel| {
        let registry = Registry::capture(context, [5; 32], &[Capability::Both], view).unwrap();
        let plan = Plan::capture(&registry, &requirements(), 100, view).unwrap();
        let result = evaluate(&plan, view).unwrap();
        let foreign = Plan::capture(&registry, &requirements(), 100, view).unwrap();
        assert!(result.current(&foreign, &registry, view).is_err());
        cancel.cancel();
        assert!(result.current(&plan, &registry, view).is_err());
        assert!(result
            .priority_current(&plan, &registry, view, zero())
            .is_err());
    });
}

#[test]
fn tlcx_modal_derived_rule_is_executed_and_fixed_probe_is_not_a_label() {
    fixture(|context, view, _| {
        let registry = Registry::capture(context, [5; 32], &[Capability::Derived], view).unwrap();
        let plan = Plan::capture(
            &registry,
            &[ProbeSpec {
                capability: Capability::Derived,
                weight: Rational::from_integer(1.into()),
            }],
            100,
            view,
        )
        .unwrap();
        let result = evaluate(&plan, view).unwrap();
        let current = result.current(&plan, &registry, view).unwrap();
        assert_eq!(current.statuses(), &[ProbeStatus::Matched]);
        assert_eq!(
            current.bounds(),
            Some(&RationalInterval::exact_ratio(1, 1).unwrap())
        );
        assert_eq!(current.actual_evaluation_count(), 1);
    });
}

#[test]
fn tlcx_modal_same_numeric_run_in_foreign_broker_cannot_reuse_result() {
    let source = SourceRevision::of("explicit four-valued capability test");
    let limits = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    let broker_a = ResourceBroker::new(limits).unwrap();
    let broker_b = ResourceBroker::new(limits).unwrap();
    let mut run_a = broker_a
        .begin_run(
            743,
            source.digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut run_b = broker_b
        .begin_run(
            743,
            source.digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let requests = [
        ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(24 * 1024 * 1024).unwrap(),
        ),
        ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(8 * 1024 * 1024).unwrap(),
        ),
    ];
    let mut lease_a = run_a.reserve(&requests).unwrap();
    let mut lease_b = run_b.reserve(&requests).unwrap();
    {
        let a = lease_a.lease();
        let b = lease_b.lease();
        assert_eq!(a.run_id(), b.run_id());
        let registry = Registry::capture(
            ContextRevision::original(source),
            [5; 32],
            &[Capability::Both],
            &a,
        )
        .unwrap();
        let plan = Plan::capture(&registry, &requirements(), 100, &a).unwrap();
        let result = evaluate(&plan, &a).unwrap();
        assert!(result.current(&plan, &registry, &b).is_err());
        assert!(evaluate(&plan, &b).is_err());
    }
    lease_a.release_confirmed().unwrap();
    lease_b.release_confirmed().unwrap();
    run_a.close_confirmed().unwrap();
    run_b.close_confirmed().unwrap();
}
fn second_key(first: &PriorityInput, second: &PriorityInput) -> Vec<u8> {
    let binding = ScheduleBinding {
        comparison_scope: [5; 32],
        selection_policy: [6; 32],
        budget_series: [7; 32],
        novelty_policy: SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1,
    };
    let proposals = [
        SchedulingProposal {
            binding,
            canonical_key: b"a",
            priority: first,
            estimated_query_slots: 1,
            demand: [1, 0, 1],
        },
        SchedulingProposal {
            binding,
            canonical_key: b"z",
            priority: second,
            estimated_query_slots: 1,
            demand: [1, 0, 1],
        },
    ];
    let mut scheduler = ExplorationScheduler::new(binding, 16, 2, [1, 0, 1], 100).unwrap();
    let ScheduleStep::Proposed(ticket) = scheduler.propose(&proposals, [8; 32], 1, false).unwrap()
    else {
        panic!("first")
    };
    scheduler.finish(&ticket, true).unwrap();
    let ScheduleStep::Proposed(ticket) = scheduler.propose(&proposals, [8; 32], 2, false).unwrap()
    else {
        panic!("second")
    };
    ticket.canonical_key().to_vec()
}
#[test]
fn tlcx_modal_actual_scheduler_consumes_width_and_cut_changes_dispatch() {
    fixture(|context, view, _| {
        let registry = Registry::capture(
            context,
            [5; 32],
            &[Capability::Both, Capability::NoExplosion],
            view,
        )
        .unwrap();
        let full = Plan::capture(&registry, &requirements(), 100, view).unwrap();
        let full_result = evaluate(&full, view).unwrap();
        let missing = Registry::capture(context, [5; 32], &[Capability::Both], view).unwrap();
        let partial = Plan::capture(&missing, &requirements(), 100, view).unwrap();
        let partial_result = evaluate(&partial, view).unwrap();
        let full_priority = full_result
            .priority_current(&full, &registry, view, zero())
            .unwrap();
        let partial_priority = partial_result
            .priority_current(&partial, &missing, view, zero())
            .unwrap();
        let a = full_priority.current(&full, &registry, view).unwrap();
        let b = partial_priority.current(&partial, &missing, view).unwrap();
        assert_eq!(second_key(a, b), b"z");
        assert_eq!(second_key(b, a), b"a");
    });
}

#[test]
fn tlcx_modal_wrapper_preserves_ticket_and_refuses_cancelled_current_view() {
    fixture(|context, view, cancel| {
        let registry = Registry::capture(context, [5; 32], &[Capability::Both], view).unwrap();
        let plan = Plan::capture(&registry, &requirements(), 100, view).unwrap();
        let result = evaluate(&plan, view).unwrap();
        let priority = result
            .priority_current(&plan, &registry, view, zero())
            .unwrap();
        let binding = ScheduleBinding {
            comparison_scope: [5; 32],
            selection_policy: [6; 32],
            budget_series: [7; 32],
            novelty_policy: SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1,
        };
        let work = [CapabilityWork {
            priority: &priority,
            plan: &plan,
            registry: &registry,
            canonical_key: b"a",
            estimated_query_slots: 1,
            demand: [1, 0, 1],
        }];
        let mut scheduler = ExplorationScheduler::new(binding, 16, 2, [1, 0, 1], 100).unwrap();
        let observed = schedule_current(&mut scheduler, &work, [8; 32], 1, view).unwrap();
        assert_eq!(
            observed.freshness_at_return(),
            SchedulingFreshness::CurrentAtReturn
        );
        assert!(matches!(
            observed.current(view).unwrap(),
            ScheduleStep::Proposed(_)
        ));
        cancel.cancel();
        assert!(observed.current(view).is_err());
        assert!(matches!(observed.historical(), ScheduleStep::Proposed(_)));
        assert_eq!(scheduler.fairness().slots_consumed, 1);
    });
}

#[test]
fn tlcx_modal_wrapper_rejects_scope_or_cancel_before_scheduler_mutation() {
    fixture(|context, view, cancel| {
        let registry = Registry::capture(context, [5; 32], &[Capability::Both], view).unwrap();
        let plan = Plan::capture(&registry, &requirements(), 100, view).unwrap();
        let result = evaluate(&plan, view).unwrap();
        let priority = result
            .priority_current(&plan, &registry, view, zero())
            .unwrap();
        let binding = ScheduleBinding {
            comparison_scope: [9; 32],
            selection_policy: [6; 32],
            budget_series: [7; 32],
            novelty_policy: SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1,
        };
        let work = [CapabilityWork {
            priority: &priority,
            plan: &plan,
            registry: &registry,
            canonical_key: b"a",
            estimated_query_slots: 1,
            demand: [1, 0, 1],
        }];
        let mut scheduler = ExplorationScheduler::new(binding, 16, 2, [1, 0, 1], 100).unwrap();
        assert!(schedule_current(&mut scheduler, &work, [8; 32], 1, view).is_err());
        assert_eq!(scheduler.fairness().slots_consumed, 0);
        cancel.cancel();
        assert!(schedule_current(&mut scheduler, &work, [8; 32], 2, view).is_err());
        assert_eq!(scheduler.fairness().slots_consumed, 0);
    });
}

#[test]
fn tlcx_modal_wrapper_consumes_measured_width_to_change_second_proposal() {
    fixture(|context, view, _| {
        let full_reg = Registry::capture(
            context,
            [5; 32],
            &[Capability::Both, Capability::NoExplosion],
            view,
        )
        .unwrap();
        let full_plan = Plan::capture(&full_reg, &requirements(), 100, view).unwrap();
        let full = evaluate(&full_plan, view).unwrap();
        let full_priority = full
            .priority_current(&full_plan, &full_reg, view, zero())
            .unwrap();
        let partial_reg = Registry::capture(context, [5; 32], &[Capability::Both], view).unwrap();
        let partial_plan = Plan::capture(&partial_reg, &requirements(), 100, view).unwrap();
        let partial = evaluate(&partial_plan, view).unwrap();
        let partial_priority = partial
            .priority_current(&partial_plan, &partial_reg, view, zero())
            .unwrap();
        let binding = ScheduleBinding {
            comparison_scope: [5; 32],
            selection_policy: [6; 32],
            budget_series: [7; 32],
            novelty_policy: SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1,
        };
        let a = [
            CapabilityWork {
                priority: &full_priority,
                plan: &full_plan,
                registry: &full_reg,
                canonical_key: b"a",
                estimated_query_slots: 1,
                demand: [1, 0, 1],
            },
            CapabilityWork {
                priority: &partial_priority,
                plan: &partial_plan,
                registry: &partial_reg,
                canonical_key: b"z",
                estimated_query_slots: 1,
                demand: [1, 0, 1],
            },
        ];
        let b = [
            CapabilityWork {
                priority: &partial_priority,
                plan: &partial_plan,
                registry: &partial_reg,
                canonical_key: b"a",
                estimated_query_slots: 1,
                demand: [1, 0, 1],
            },
            CapabilityWork {
                priority: &full_priority,
                plan: &full_plan,
                registry: &full_reg,
                canonical_key: b"z",
                estimated_query_slots: 1,
                demand: [1, 0, 1],
            },
        ];
        let select = |work: &[CapabilityWork<'_, '_, '_, '_>]| {
            let mut scheduler = ExplorationScheduler::new(binding, 16, 2, [1, 0, 1], 100).unwrap();
            let first = schedule_current(&mut scheduler, work, [8; 32], 1, view).unwrap();
            let ScheduleStep::Proposed(ticket) = first.current(view).unwrap() else {
                panic!("first")
            };
            scheduler.finish(ticket, true).unwrap();
            drop(first);
            let second = schedule_current(&mut scheduler, work, [8; 32], 2, view).unwrap();
            let ScheduleStep::Proposed(ticket) = second.current(view).unwrap() else {
                panic!("second")
            };
            ticket.canonical_key().to_vec()
        };
        assert_eq!(select(&a), b"z");
        assert_eq!(select(&b), b"a");
    });
}

#[test]
fn tlcx_modal_wrapper_rejects_mixed_denominators_in_same_scope() {
    fixture(|context, view, _| {
        let registry = Registry::capture(context, [5; 32], &[Capability::Both], view).unwrap();
        let base = Plan::capture(&registry, &requirements(), 100, view).unwrap();
        let mut changed = requirements();
        changed[1].weight = Rational::from_integer(9.into());
        let different = Plan::capture(&registry, &changed, 100, view).unwrap();
        let base_result = evaluate(&base, view).unwrap();
        let other_result = evaluate(&different, view).unwrap();
        let base_priority = base_result
            .priority_current(&base, &registry, view, zero())
            .unwrap();
        let other_priority = other_result
            .priority_current(&different, &registry, view, zero())
            .unwrap();
        let binding = ScheduleBinding {
            comparison_scope: [5; 32],
            selection_policy: [6; 32],
            budget_series: [7; 32],
            novelty_policy: SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1,
        };
        let work = [
            CapabilityWork {
                priority: &base_priority,
                plan: &base,
                registry: &registry,
                canonical_key: b"a",
                estimated_query_slots: 1,
                demand: [1, 0, 1],
            },
            CapabilityWork {
                priority: &other_priority,
                plan: &different,
                registry: &registry,
                canonical_key: b"z",
                estimated_query_slots: 1,
                demand: [1, 0, 1],
            },
        ];
        let mut scheduler = ExplorationScheduler::new(binding, 16, 2, [1, 0, 1], 100).unwrap();
        assert!(schedule_current(&mut scheduler, &work, [8; 32], 1, view).is_err());
        assert_eq!(scheduler.fairness().slots_consumed, 0);
        let limited = Plan::capture(&registry, &requirements(), 1, view).unwrap();
        let limited_result = evaluate(&limited, view).unwrap();
        let limited_priority = limited_result
            .priority_current(&limited, &registry, view, zero())
            .unwrap();
        let mixed_work = [
            CapabilityWork {
                priority: &base_priority,
                plan: &base,
                registry: &registry,
                canonical_key: b"a",
                estimated_query_slots: 1,
                demand: [1, 0, 1],
            },
            CapabilityWork {
                priority: &limited_priority,
                plan: &limited,
                registry: &registry,
                canonical_key: b"z",
                estimated_query_slots: 1,
                demand: [1, 0, 1],
            },
        ];
        assert!(schedule_current(&mut scheduler, &mixed_work, [8; 32], 2, view).is_err());
        assert_eq!(scheduler.fairness().slots_consumed, 0);
    });
}

#[test]
fn tlcx_modal_owned_scheduler_rejects_cross_call_profile_drift_before_tick() {
    fixture(|context, view, _| {
        let full_reg = Registry::capture(
            context,
            [5; 32],
            &[Capability::Both, Capability::NoExplosion],
            view,
        )
        .unwrap();
        let partial_reg = Registry::capture(context, [5; 32], &[Capability::Both], view).unwrap();
        let baseline = Plan::capture(&full_reg, &requirements(), 100, view).unwrap();
        let partial = Plan::capture(&partial_reg, &requirements(), 100, view).unwrap();
        let mut altered = requirements();
        altered[1].weight = Rational::from_integer(9.into());
        let changed = Plan::capture(&partial_reg, &altered, 100, view).unwrap();
        let limited = Plan::capture(&partial_reg, &requirements(), 1, view).unwrap();
        let mut different = requirements();
        different[1].capability = Capability::Derived;
        let other = Plan::capture(&partial_reg, &different, 100, view).unwrap();
        let base_result = evaluate(&baseline, view).unwrap();
        let partial_result = evaluate(&partial, view).unwrap();
        let changed_result = evaluate(&changed, view).unwrap();
        let limited_result = evaluate(&limited, view).unwrap();
        let other_result = evaluate(&other, view).unwrap();
        let p_base = base_result
            .priority_current(&baseline, &full_reg, view, zero())
            .unwrap();
        let p_partial = partial_result
            .priority_current(&partial, &partial_reg, view, zero())
            .unwrap();
        let p_changed = changed_result
            .priority_current(&changed, &partial_reg, view, zero())
            .unwrap();
        let p_limited = limited_result
            .priority_current(&limited, &partial_reg, view, zero())
            .unwrap();
        let p_other = other_result
            .priority_current(&other, &partial_reg, view, zero())
            .unwrap();
        let binding = ScheduleBinding {
            comparison_scope: [5; 32],
            selection_policy: [6; 32],
            budget_series: [7; 32],
            novelty_policy: SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1,
        };
        let foreign = ScheduleBinding {
            comparison_scope: [9; 32],
            ..binding
        };
        assert!(CapabilityScheduler::new(foreign, 16, 2, 100, &baseline, &full_reg, view).is_err());
        assert!(CapabilityScheduler::new(binding, 0, 2, 100, &baseline, &full_reg, view).is_err());
        let mut owner =
            CapabilityScheduler::new(binding, 16, 2, 100, &baseline, &full_reg, view).unwrap();
        let first = [CapabilityWork {
            priority: &p_base,
            plan: &baseline,
            registry: &full_reg,
            canonical_key: b"a",
            estimated_query_slots: 1,
            demand: [1, 0, 1],
        }];
        let step = owner.propose(&first, [8; 32], 1, view).unwrap();
        assert!(matches!(
            step.current(view).unwrap(),
            ScheduleStep::Proposed(_)
        ));
        owner.finish_current(&step, view, true).unwrap();
        drop(step);
        let weight_cut = [CapabilityWork {
            priority: &p_changed,
            plan: &changed,
            registry: &partial_reg,
            canonical_key: b"z",
            estimated_query_slots: 1,
            demand: [1, 0, 1],
        }];
        assert!(owner.propose(&weight_cut, [8; 32], 50, view).is_err());
        let budget_cut = [CapabilityWork {
            priority: &p_limited,
            plan: &limited,
            registry: &partial_reg,
            canonical_key: b"z",
            estimated_query_slots: 1,
            demand: [1, 0, 1],
        }];
        assert!(owner.propose(&budget_cut, [8; 32], 60, view).is_err());
        let universe_cut = [CapabilityWork {
            priority: &p_other,
            plan: &other,
            registry: &partial_reg,
            canonical_key: b"z",
            estimated_query_slots: 1,
            demand: [1, 0, 1],
        }];
        assert!(owner.propose(&universe_cut, [8; 32], 70, view).is_err());
        assert_eq!(owner.slots_consumed(), 1);
        let valid = [CapabilityWork {
            priority: &p_partial,
            plan: &partial,
            registry: &partial_reg,
            canonical_key: b"z",
            estimated_query_slots: 1,
            demand: [1, 0, 1],
        }];
        // A successful lower timestamp proves the rejected cuts never advanced tick.
        let second = owner.propose(&valid, [8; 32], 2, view).unwrap();
        assert!(matches!(
            second.current(view).unwrap(),
            ScheduleStep::Proposed(_)
        ));
        assert_eq!(owner.slots_consumed(), 2);
    });
}
