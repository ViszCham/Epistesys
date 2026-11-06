//! Actual capability producer -> current-owner join -> pinned native Z3.
//! Ignored cases are unobserved until central explicitly runs the native profile.
#[path = "tlcx40_canonical_support/mod.rs"]
#[allow(dead_code)]
mod support;

use lc631_core::{
    context::{ContextLimits, ContextScope, ContextState, OriginalContext, SourceRevision},
    measurement_numeric::Rational,
    AuthorityRevision,
};
use lc631_runtime::{
    capability_measurement::{self as capability, Capability, Plan, ProbeSpec, Registry},
    resource::*,
    scheduled_solver::{
        CapabilityFreshness, CapabilityNativeWork, DispatchStep, NativeDemand, NativeScheduler,
        NativeWorkSpec, ReservationSettlement, TicketSettlement, SCHEDULER_RAM_BYTES,
    },
};
use lc631_solvers::{
    protocol::sexpr::SExprLimits,
    runner::{ProcessCleanupState, QueryDigest},
    typed_query::{BoolExpr, IntExpr, IntSymbolId, SmtEmitLimits, SmtStatusQuery},
};
use lc631_world::exploration_schedule::{
    priority::UnitInterval, ScheduleBinding, SchedulerNoveltyPolicyId,
};
use std::time::Duration;

fn context() -> ContextState {
    let d = Digest32::from_bytes([1; 32]);
    OriginalContext::checked(
        SourceRevision::of("nov4 current capability native dispatch"),
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
    .unwrap()
    .state()
    .clone()
}
fn binding() -> ScheduleBinding {
    ScheduleBinding {
        comparison_scope: [1; 32],
        selection_policy: [2; 32],
        budget_series: [3; 32],
        novelty_policy: SchedulerNoveltyPolicyId::DistanceToFrontierUnionV1,
    }
}
fn probes() -> [ProbeSpec; 2] {
    [Capability::Both, Capability::NoExplosion].map(|capability| ProbeSpec {
        capability,
        weight: Rational::from_integer(1.into()),
    })
}
fn zero() -> UnitInterval {
    UnitInterval::checked(
        Rational::from_integer(0.into()),
        Rational::from_integer(0.into()),
    )
    .unwrap()
}
fn query(n: i64) -> SmtStatusQuery {
    SmtStatusQuery::checked(
        vec![],
        vec![IntSymbolId::new(0)],
        vec![BoolExpr::Equal(
            IntExpr::Variable(IntSymbolId::new(0)),
            IntExpr::Constant(n),
        )],
        SmtEmitLimits::checked(4096, 0, 1, 8, 32, 8).unwrap(),
    )
    .unwrap()
}
fn spec<'a>(
    key: &'a [u8],
    query: &'a SmtStatusQuery,
    context: &'a ContextState,
    priority: &'a lc631_world::exploration_schedule::priority::PriorityInput,
) -> NativeWorkSpec<'a> {
    NativeWorkSpec {
        binding: binding(),
        canonical_key: key,
        priority,
        estimated_query_slots: 1,
        query,
        context,
        budget: support::execution(),
        parser: SExprLimits::checked(65536, 4096, 64).unwrap(),
        demand: NativeDemand::checked([1, 0, 1], 128 * 1024 * 1024, 2 * 1024 * 1024, 65536)
            .unwrap(),
    }
}
fn request(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}
fn run(c: &ContextState, cancel: Cancellation) -> (ResourceBroker, RunResources) {
    let limits = ResourceLimits::uniform(256 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            25,
            c.revision().source().digest(),
            limits,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            cancel,
        )
        .unwrap();
    (broker, run)
}
fn metadata(run: &RunResources) -> Reservation {
    run.reserve(&[
        request(ResourceKind::Ram, SCHEDULER_RAM_BYTES + 16 * 1024 * 1024),
        request(ResourceKind::Output, 8 * 1024 * 1024),
    ])
    .unwrap()
}

fn experiment(cut_b_probe: bool) -> (i128, QueryDigest) {
    let c = context();
    let (broker, mut run) = run(&c, Cancellation::default());
    let mut metadata = metadata(&run);
    let second = {
        let view = metadata.lease();
        let a_registry = Registry::capture(
            c.revision(),
            [1; 32],
            &[Capability::Both, Capability::NoExplosion],
            &view,
        )
        .unwrap();
        let b_registry = Registry::capture(
            c.revision(),
            [1; 32],
            if cut_b_probe {
                &[Capability::Both]
            } else {
                &[Capability::Both, Capability::NoExplosion]
            },
            &view,
        )
        .unwrap();
        let probes = probes();
        let a_plan = Plan::capture(&a_registry, &probes, 100, &view).unwrap();
        let b_plan = Plan::capture(&b_registry, &probes, 100, &view).unwrap();
        // These are current-checking APIs, not context/plan labels supplied by us.
        assert_eq!(a_registry.context_current(&view).unwrap(), c.revision());
        assert!(a_plan
            .same_profile_current(&a_registry, &b_plan, &b_registry, &view)
            .unwrap());
        let a = capability::evaluate(&a_plan, &view).unwrap();
        let b = capability::evaluate(&b_plan, &view).unwrap();
        assert_eq!(
            a.current(&a_plan, &a_registry, &view)
                .unwrap()
                .actual_evaluation_count(),
            2
        );
        assert_eq!(
            b.current(&b_plan, &b_registry, &view)
                .unwrap()
                .actual_evaluation_count(),
            if cut_b_probe { 1 } else { 2 }
        );
        let ap = a
            .priority_current(&a_plan, &a_registry, &view, zero())
            .unwrap();
        let bp = b
            .priority_current(&b_plan, &b_registry, &view, zero())
            .unwrap();
        let aq = query(3);
        let bq = query(7);
        let works = [
            CapabilityNativeWork::checked(
                spec(
                    b"a",
                    &aq,
                    &c,
                    ap.current(&a_plan, &a_registry, &view).unwrap(),
                ),
                &ap,
                &a_plan,
                &a_registry,
                &view,
            )
            .unwrap(),
            CapabilityNativeWork::checked(
                spec(
                    b"b",
                    &bq,
                    &c,
                    bp.current(&b_plan, &b_registry, &view).unwrap(),
                ),
                &bp,
                &b_plan,
                &b_registry,
                &view,
            )
            .unwrap(),
        ];
        let mut scheduler =
            NativeScheduler::with_resources(&broker, &view, binding(), 4, 2, 100).unwrap();
        let native = support::native_registry();
        let cwd = std::env::current_dir().unwrap();
        let mut second = None;
        for tick in 1..=2 {
            let observed = scheduler
                .dispatch_capability_next(
                    &run,
                    &works,
                    [0; 32],
                    tick,
                    false,
                    &native,
                    &cwd,
                    |result| match result {
                        Ok(model) => {
                            assert_eq!(model.context(), c.revision());
                            model.model().integer(IntSymbolId::new(0)).unwrap()
                        }
                        Err(_) => panic!("actual pinned native model required"),
                    },
                )
                .unwrap();
            assert_eq!(
                observed.freshness_at_return(),
                CapabilityFreshness::CurrentAtReturn
            );
            let DispatchStep::Observed(receipt) = observed.historical() else {
                panic!("actual native report required")
            };
            assert_eq!(
                receipt.reservation_settlement(),
                &ReservationSettlement::Released
            );
            assert_eq!(receipt.ticket_settlement(), &TicketSettlement::Settled);
            let report = receipt.report().unwrap();
            assert_eq!(
                report.cleanup_state(),
                ProcessCleanupState::RunnerReportedVerified
            );
            assert!(!report.authority_created());
            assert!(!report.proof_checked());
            if tick == 1 {
                assert_eq!(receipt.selected_key(), b"a");
            } else {
                assert_eq!(
                    receipt.selected_key(),
                    if cut_b_probe { &b"b"[..] } else { &b"a"[..] }
                );
                second = Some((*receipt.value(), report.query()));
            }
            assert_eq!(scheduler.slots_consumed(), tick);
            assert_eq!(broker.used(ResourceKind::Child).unwrap(), 0);
        }
        second.unwrap()
    };
    metadata.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
    second
}

#[test]
#[ignore = "explicit pinned Z3; central must observe this positive and probe-cut pair"]
fn tlcx25_nov4_actual_capability_probe_cut_changes_native_query_and_model() {
    let full = experiment(false);
    let cut = experiment(true);
    assert_eq!((full.0, cut.0), (3, 7));
    assert_ne!(full.1, cut.1);
}

#[test]
#[ignore = "explicit pinned Z3 registry; current owner cuts must run before dispatch"]
fn tlcx25_nov4_detached_priority_foreign_plan_and_cancel_reject_before_slot() {
    let c = context();
    let cancel = Cancellation::default();
    let (broker, run) = run(&c, cancel.clone());
    let metadata = metadata(&run);
    let view = metadata.lease();
    let registry = Registry::capture(c.revision(), [1; 32], &[Capability::Both], &view).unwrap();
    let probes = probes();
    let plan = Plan::capture(&registry, &probes, 100, &view).unwrap();
    let foreign = Plan::capture(&registry, &probes, 100, &view).unwrap();
    let assessment = capability::evaluate(&plan, &view).unwrap();
    let priority = assessment
        .priority_current(&plan, &registry, &view, zero())
        .unwrap();
    let q = query(3);
    let current = priority.current(&plan, &registry, &view).unwrap();
    let detached = current.clone();
    assert!(CapabilityNativeWork::checked(
        spec(b"a", &q, &c, &detached),
        &priority,
        &plan,
        &registry,
        &view
    )
    .is_err());
    assert!(CapabilityNativeWork::checked(
        spec(b"a", &q, &c, current),
        &priority,
        &foreign,
        &registry,
        &view
    )
    .is_err());
    let works = [CapabilityNativeWork::checked(
        spec(b"a", &q, &c, current),
        &priority,
        &plan,
        &registry,
        &view,
    )
    .unwrap()];
    let mut scheduler =
        NativeScheduler::with_resources(&broker, &view, binding(), 4, 2, 100).unwrap();
    let native = support::native_registry();
    cancel.cancel();
    assert!(scheduler
        .dispatch_capability_next(
            &run,
            &works,
            [0; 32],
            1,
            false,
            &native,
            &std::env::current_dir().unwrap(),
            |_| ()
        )
        .is_err());
    assert_eq!(scheduler.slots_consumed(), 0);
    assert_eq!(broker.used(ResourceKind::Child).unwrap(), 0);
}

#[test]
#[ignore = "explicit native-owner profile; same source/run labels do not prove owner/context"]
fn tlcx25_nov4_foreign_reservation_and_same_source_context_are_rejected() {
    let c = context();
    let (_broker, run) = run(&c, Cancellation::default());
    let metadata = metadata(&run);
    let view = metadata.lease();
    let registry = Registry::capture(c.revision(), [1; 32], &[Capability::Both], &view).unwrap();
    let probes = probes();
    let plan = Plan::capture(&registry, &probes, 100, &view).unwrap();
    let assessment = capability::evaluate(&plan, &view).unwrap();
    let priority = assessment
        .priority_current(&plan, &registry, &view, zero())
        .unwrap();
    let q = query(3);
    let current = priority.current(&plan, &registry, &view).unwrap();
    let (_foreign_broker, foreign_run) = self::run(&c, Cancellation::default());
    let foreign_metadata = self::metadata(&foreign_run);
    let foreign_view = foreign_metadata.lease();
    assert_eq!(view.run_id(), foreign_view.run_id());
    assert!(CapabilityNativeWork::checked(
        spec(b"a", &q, &c, current),
        &priority,
        &plan,
        &registry,
        &foreign_view
    )
    .is_err());
    let d = Digest32::from_bytes([1; 32]);
    let foreign_context = OriginalContext::checked(
        c.revision().source(),
        ContextScope::declared(d, Digest32::from_bytes([9; 32]), d, d),
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
    assert_ne!(c.revision(), foreign_context.state().revision());
    assert!(CapabilityNativeWork::checked(
        spec(b"a", &q, foreign_context.state(), current),
        &priority,
        &plan,
        &registry,
        &view
    )
    .is_err());
}

#[test]
#[ignore = "explicit pinned Z3; cancellation after actual I/O preserves its report"]
fn tlcx25_nov4_post_io_stale_keeps_actual_report_value_and_settlement() {
    let c = context();
    let cancel = Cancellation::default();
    let (broker, run) = run(&c, cancel.clone());
    let metadata = metadata(&run);
    let view = metadata.lease();
    let registry = Registry::capture(c.revision(), [1; 32], &[Capability::Both], &view).unwrap();
    let probes = probes();
    let plan = Plan::capture(&registry, &probes, 100, &view).unwrap();
    let assessment = capability::evaluate(&plan, &view).unwrap();
    let priority = assessment
        .priority_current(&plan, &registry, &view, zero())
        .unwrap();
    let q = query(3);
    let works = [CapabilityNativeWork::checked(
        spec(
            b"a",
            &q,
            &c,
            priority.current(&plan, &registry, &view).unwrap(),
        ),
        &priority,
        &plan,
        &registry,
        &view,
    )
    .unwrap()];
    let mut scheduler =
        NativeScheduler::with_resources(&broker, &view, binding(), 4, 2, 100).unwrap();
    let observed = scheduler
        .dispatch_capability_next(
            &run,
            &works,
            [0; 32],
            1,
            false,
            &support::native_registry(),
            &std::env::current_dir().unwrap(),
            |result| {
                let value = match result {
                    Ok(model) => model.model().integer(IntSymbolId::new(0)).unwrap(),
                    Err(_) => panic!("native model must precede cancellation"),
                };
                cancel.cancel();
                value
            },
        )
        .unwrap();
    assert_eq!(
        observed.freshness_at_return(),
        CapabilityFreshness::ObservedStale
    );
    let DispatchStep::Observed(receipt) = observed.historical() else {
        panic!("actual dispatch cannot disappear")
    };
    assert_eq!(*receipt.value(), 3);
    assert_eq!(receipt.selected_key(), b"a");
    let report = receipt
        .report()
        .expect("retain actual pre-cancel native report");
    assert_eq!(
        report.cleanup_state(),
        ProcessCleanupState::RunnerReportedVerified
    );
    assert!(!report.authority_created());
    assert_eq!(
        receipt.reservation_settlement(),
        &ReservationSettlement::Released
    );
    assert_eq!(receipt.ticket_settlement(), &TicketSettlement::Settled);
    assert_eq!(scheduler.slots_consumed(), 1);
    assert_eq!(broker.used(ResourceKind::Child).unwrap(), 0);
    assert_eq!(
        broker.used(ResourceKind::Ram).unwrap(),
        SCHEDULER_RAM_BYTES + 16 * 1024 * 1024,
    );
    assert_eq!(broker.used(ResourceKind::Output).unwrap(), 8 * 1024 * 1024);
    assert!(priority.current(&plan, &registry, &view).is_err());
}
