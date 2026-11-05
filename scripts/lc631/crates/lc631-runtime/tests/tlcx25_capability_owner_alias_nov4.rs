//! Negative-first admission: equal native numbers cannot alias actual owners.
#[path = "tlcx40_canonical_support/mod.rs"]
#[allow(dead_code)]
mod support;

use lc631_core::{
    context::{ContextLimits, ContextScope, OriginalContext, SourceRevision},
    measurement_numeric::Rational,
    AuthorityRevision,
};
use lc631_runtime::{
    capability_measurement::{self as capability, Capability, Plan, ProbeSpec, Registry},
    resource::*,
    scheduled_solver::{
        CapabilityNativeWork, DispatchError, NativeDemand, NativeScheduler, NativeWorkSpec,
        SCHEDULER_RAM_BYTES,
    },
};
use lc631_solvers::{
    protocol::sexpr::SExprLimits,
    typed_query::{BoolExpr, IntExpr, IntSymbolId, SmtEmitLimits, SmtStatusQuery},
};
use lc631_world::exploration_schedule::{
    priority::UnitInterval, ScheduleBinding, SchedulerNoveltyPolicyId,
};
use std::time::Duration;

#[test]
#[ignore = "explicit pinned native profile; observe owner-alias negative before its fix"]
fn tlcx25_nov4_same_key_equal_values_from_distinct_actual_owners_reject_before_slot() {
    owner_alias_case(true);
}

#[test]
#[ignore = "explicit pinned Z3; distinct owners with identical actual evidence share one ticket"]
fn tlcx25_nov4_same_key_equal_actual_evidence_from_distinct_owners_aliases_once() {
    owner_alias_case(false);
}

fn owner_alias_case(distinct_evidence: bool) {
    let d = Digest32::from_bytes([1; 32]);
    let context = OriginalContext::checked(
        SourceRevision::of("nov4 actual capability owner alias negative"),
        ContextScope::declared(d, d, d, d),
        AuthorityRevision(1),
        vec![],
        ContextLimits {
            max_contexts: 8,
            max_premises: 8,
            max_edits: 8,
            max_statement_bytes: 4096,
        },
    )
    .unwrap();
    let c = context.state();
    let limits = ResourceLimits::uniform(256 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            25,
            c.revision().source().digest(),
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let request = |kind, n| ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap());
    let mut metadata = run
        .reserve(&[
            request(ResourceKind::Ram, SCHEDULER_RAM_BYTES + 16 * 1024 * 1024),
            request(ResourceKind::Output, 8 * 1024 * 1024),
        ])
        .unwrap();
    {
        let view = metadata.lease();
        let required = [Capability::Both, Capability::NoExplosion, Capability::Derived];
        let first_registry = Registry::capture(c.revision(), [1; 32], &[Capability::Both], &view).unwrap();
        let second_registry = Registry::capture(
            c.revision(), [1; 32],
            if distinct_evidence { &[Capability::Derived] } else { &[Capability::Both] },
            &view,
        ).unwrap();
        let probes = required.map(|capability| ProbeSpec {
            capability,
            weight: Rational::from_integer(1.into()),
        });
        let first_plan = Plan::capture(&first_registry, &probes, 100, &view).unwrap();
        let second_plan = Plan::capture(&second_registry, &probes, 100, &view).unwrap();
        let first = capability::evaluate(&first_plan, &view).unwrap();
        let second = capability::evaluate(&second_plan, &view).unwrap();
        let first_current = first.current(&first_plan, &first_registry, &view).unwrap();
        let second_current = second.current(&second_plan, &second_registry, &view).unwrap();
        assert_eq!(first_current.actual_evaluation_count(), 1);
        assert_eq!(second_current.actual_evaluation_count(), 1);
        if distinct_evidence {
            assert_ne!(first_current.statuses(), second_current.statuses());
        } else {
            assert_eq!(first_current.statuses(), second_current.statuses());
        }
        assert_eq!(first_current.bounds(), second_current.bounds());
        let zero = || UnitInterval::checked(
            Rational::from_integer(0.into()), Rational::from_integer(0.into()),
        ).unwrap();
        let first_priority = first.priority_current(&first_plan, &first_registry, &view, zero()).unwrap();
        let second_priority = second.priority_current(&second_plan, &second_registry, &view, zero()).unwrap();
        let a = first_priority.current(&first_plan, &first_registry, &view).unwrap();
        let b = second_priority.current(&second_plan, &second_registry, &view).unwrap();
        assert_eq!(a, b);
        assert!(!std::ptr::eq(a, b));
        let query = SmtStatusQuery::checked(
            vec![], vec![IntSymbolId::new(0)],
            vec![BoolExpr::Equal(IntExpr::Variable(IntSymbolId::new(0)), IntExpr::Constant(3))],
            SmtEmitLimits::checked(4096, 0, 1, 8, 32, 8).unwrap(),
        ).unwrap();
        let binding = ScheduleBinding {
            comparison_scope: [1; 32], selection_policy: [2; 32], budget_series: [3; 32],
            novelty_policy: SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1,
        };
        let spec = |priority| NativeWorkSpec {
            binding, canonical_key: b"same-complete-work-key", priority,
            estimated_query_slots: 1, query: &query, context: c,
            budget: support::execution(),
            parser: SExprLimits::checked(65536, 4096, 64).unwrap(),
            demand: NativeDemand::checked([1, 0, 1], 128 * 1024 * 1024, 2 * 1024 * 1024, 65536).unwrap(),
        };
        let works = [
            CapabilityNativeWork::checked(spec(a), &first_priority, &first_plan, &first_registry, &view).unwrap(),
            CapabilityNativeWork::checked(spec(b), &second_priority, &second_plan, &second_registry, &view).unwrap(),
        ];
        let native = support::native_registry();
        let mut scheduler = NativeScheduler::with_resources(&broker, &view, binding, 4, 2, 100).unwrap();
        let mut native_calls = 0;
        let result = scheduler.dispatch_capability_next(
            &run, &works, [0; 32], 1, false, &native,
            &std::env::current_dir().unwrap(),
            |native| {
                native_calls += 1;
                match native {
                    Ok(model) => assert_eq!(model.model().integer(IntSymbolId::new(0)), Some(3)),
                    Err(_) => panic!("unexpected native error; this is not the expected admission RED"),
                }
            },
        );
        match &result {
            Err(error) => eprintln!("owner-alias result=Err({error:?}) native_calls={native_calls}"),
            Ok(observed) => match observed.historical() {
                lc631_runtime::scheduled_solver::DispatchStep::Stopped(reason) => {
                    eprintln!("owner-alias result=Ok(Stopped({reason:?})) native_calls={native_calls}");
                }
                lc631_runtime::scheduled_solver::DispatchStep::Unavailable(reason) => {
                    eprintln!("owner-alias result=Ok(Unavailable({reason:?})) native_calls={native_calls}");
                }
                lc631_runtime::scheduled_solver::DispatchStep::Observed(receipt) => {
                    eprintln!(
                        "owner-alias result=Ok(Observed) key={:?} report={} reservation={:?} ticket={:?} native_calls={native_calls}",
                        receipt.selected_key(),
                        receipt.report().is_some(),
                        receipt.reservation_settlement(),
                        receipt.ticket_settlement(),
                    );
                }
            },
        }
        if distinct_evidence {
            assert!(matches!(result, Err(DispatchError::ConflictingWork)));
            assert_eq!(native_calls, 0);
            assert_eq!(scheduler.slots_consumed(), 0);
        } else {
            let observed = result.expect("canonical actual evidence permits aliasing");
            assert_eq!(observed.freshness_at_return(), lc631_runtime::scheduled_solver::CapabilityFreshness::CurrentAtReturn);
            let lc631_runtime::scheduled_solver::DispatchStep::Observed(receipt) = observed.historical() else {
                panic!("true alias must execute its shared actual native query");
            };
            assert_eq!(receipt.selected_key(), b"same-complete-work-key");
            assert!(receipt.report().is_some());
            assert_eq!(receipt.reservation_settlement(), &lc631_runtime::scheduled_solver::ReservationSettlement::Released);
            assert_eq!(receipt.ticket_settlement(), &lc631_runtime::scheduled_solver::TicketSettlement::Settled);
            assert_eq!(native_calls, 1);
            assert_eq!(scheduler.slots_consumed(), 1);
        }
        assert_eq!(broker.used(ResourceKind::Child).unwrap(), 0);
    }
    metadata.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}
