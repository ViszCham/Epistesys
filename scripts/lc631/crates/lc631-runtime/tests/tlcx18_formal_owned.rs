use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::formal_boolean as f;
use lc631_runtime::formal_measurement::{owned::*, *};
use lc631_runtime::resource::{Reservation, ResourceBroker, ResourceLimits, RunResources};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceError, ResourceKind, ResourceRequest,
};
use lc631_substrate::Digest32;
use std::time::Duration;

fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn work() -> WorkBudget {
    WorkBudget {
        max_work: 100_000,
        max_payload_bytes: 1_000_000,
    }
}
fn request(kind: ResourceKind, bytes: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(bytes).unwrap())
}
fn begin_run(
    broker: &ResourceBroker,
    source: Digest32,
    cancellation: Cancellation,
) -> RunResources {
    broker
        .begin_run(
            1,
            source,
            ResourceLimits::uniform(64 * 1024 * 1024).unwrap(),
            Deadline::after(Duration::from_secs(60)).unwrap(),
            cancellation,
        )
        .unwrap()
}
fn broker() -> ResourceBroker {
    ResourceBroker::new(ResourceLimits::uniform(64 * 1024 * 1024).unwrap()).unwrap()
}
fn reserve(run: &RunResources, bounds: ResourceBounds) -> Reservation {
    run.reserve(&[
        request(ResourceKind::Ram, bounds.scratch_ram + bounds.retained_ram),
        request(ResourceKind::Output, bounds.retained_output),
    ])
    .unwrap()
}
fn fixture<T>(body: impl FnOnce(&ProbePlan<'_>, &Candidate<'_>) -> T) -> T {
    let q = SmtStatusQuery::checked(
        vec![BoolSymbolId::new(0)],
        vec![],
        vec![],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let source = SourceRevision::of("source");
    let target = SourceRevision::of("target");
    let mut r = f::Request {
        source: &q,
        target: &q,
        source_predicate: &p,
        target_predicate: &p,
        symbols: &map,
        unconstrained: f::UnconstrainedPolicy::ExplicitlyGiven,
        binding: f::Binding {
            source,
            target,
            source_context: ContextRevision::original(source),
            target_context: ContextRevision::original(target),
            probe_set: d(3),
            probe_map: d(0),
        },
    };
    r.binding.probe_map = f::probe_map_digest(&r, 10000).unwrap();
    let weight = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [FixedProbe {
        identity: b"a",
        weight: weight.lower(),
        template: &r,
    }];
    let scope = PlanScope {
        comparison: d(1),
        definition: d(2),
        probe_set: d(3),
        source,
        context: ContextRevision::original(source),
    };
    let plan = ProbePlan::checked(scope, &q, &probes, work()).unwrap();
    let candidate =
        Candidate::given(d(7), d(1), &q, target, ContextRevision::original(target)).unwrap();
    body(&plan, &candidate)
}

#[test]
fn tlcx18_owned_actual_lease_holds_output_and_releases_scratch_then_retained() {
    fixture(|plan, candidate| {
        let b = broker();
        let run = begin_run(&b, plan.scope().source.digest(), Cancellation::default());
        let bounds = requirements(plan).unwrap();
        let mut reservation = reserve(&run, bounds);
        {
            let lease = reservation.lease();
            let owned =
                measure_owned(plan, candidate, Direction::Soundness, &[], work(), &lease).unwrap();
            assert_eq!(
                owned.historical().bounds(),
                &RationalInterval::from_ratio(0, 1, 1, 1).unwrap()
            );
            assert!(owned
                .project_current(plan, candidate, Direction::Soundness, &reservation.lease())
                .is_ok());
            let scratch = lease
                .claim_resources(&[request(ResourceKind::Ram, bounds.scratch_ram)])
                .unwrap();
            assert!(matches!(
                lease.claim_resources(&[request(ResourceKind::Ram, 1)]),
                Err(ResourceError::Exhausted(ResourceKind::Ram))
            ));
            assert!(matches!(
                lease.claim_resources(&[request(ResourceKind::Output, 1)]),
                Err(ResourceError::Exhausted(ResourceKind::Output))
            ));
            drop(scratch);
            drop(owned);
            let _released = lease
                .claim_resources(&[
                    request(ResourceKind::Ram, bounds.scratch_ram + bounds.retained_ram),
                    request(ResourceKind::Output, bounds.retained_output),
                ])
                .unwrap();
        }
        reservation.release_confirmed().unwrap();
        assert_eq!(b.used(ResourceKind::Ram).unwrap(), 0);
        assert_eq!(candidate.context().source(), candidate.revision());
    });
}

#[test]
fn tlcx18_owned_source_mismatch_and_quota_fail_before_measurement() {
    fixture(|plan, candidate| {
        let b = broker();
        let wrong = begin_run(&b, d(99), Cancellation::default());
        let bounds = requirements(plan).unwrap();
        let reservation = reserve(&wrong, bounds);
        let lease = reservation.lease();
        assert!(matches!(
            measure_owned(plan, candidate, Direction::Soundness, &[], work(), &lease),
            Err(OwnedError::Resource(ResourceError::SourceMismatch))
        ));
        let _unclaimed = lease
            .claim_resources(&[request(
                ResourceKind::Ram,
                bounds.scratch_ram + bounds.retained_ram,
            )])
            .unwrap();
        let b = broker();
        let current = begin_run(&b, plan.scope().source.digest(), Cancellation::default());
        let small = current
            .reserve(&[
                request(ResourceKind::Ram, bounds.scratch_ram - 1),
                request(ResourceKind::Output, bounds.retained_output),
            ])
            .unwrap();
        assert!(matches!(
            measure_owned(
                plan,
                candidate,
                Direction::Soundness,
                &[],
                work(),
                &small.lease()
            ),
            Err(OwnedError::Resource(ResourceError::Exhausted(
                ResourceKind::Ram
            )))
        ));
    });
}

#[test]
fn tlcx18_owned_failed_output_claim_refunds_scratch_without_a_result() {
    fixture(|plan, candidate| {
        let b = broker();
        let run = begin_run(&b, plan.scope().source.digest(), Cancellation::default());
        let bounds = requirements(plan).unwrap();
        let reservation = run
            .reserve(&[
                request(ResourceKind::Ram, bounds.scratch_ram + bounds.retained_ram),
                request(ResourceKind::Output, bounds.retained_output - 1),
            ])
            .unwrap();
        let lease = reservation.lease();
        assert!(matches!(
            measure_owned(plan, candidate, Direction::Soundness, &[], work(), &lease),
            Err(OwnedError::Resource(ResourceError::Exhausted(
                ResourceKind::Output
            )))
        ));
        let _returned = lease
            .claim_resources(&[request(
                ResourceKind::Ram,
                bounds.scratch_ram + bounds.retained_ram,
            )])
            .unwrap();
    });
}

#[test]
fn tlcx18_owned_cancelled_current_projection_keeps_historical_bounds_only() {
    fixture(|plan, candidate| {
        let b = broker();
        let cancellation = Cancellation::default();
        let run = begin_run(&b, plan.scope().source.digest(), cancellation.clone());
        let reservation = reserve(&run, requirements(plan).unwrap());
        let lease = reservation.lease();
        let owned = measure_owned(
            plan,
            candidate,
            Direction::Completeness,
            &[],
            work(),
            &lease,
        )
        .unwrap();
        cancellation.cancel();
        assert!(matches!(
            owned.project_current(plan, candidate, Direction::Completeness, &lease),
            Err(OwnedError::Resource(ResourceError::Cancelled))
        ));
        assert_eq!(owned.historical().direction(), Direction::Completeness);
        assert!(matches!(
            measure_owned(
                plan,
                candidate,
                Direction::Completeness,
                &[],
                work(),
                &lease
            ),
            Err(OwnedError::Resource(ResourceError::Cancelled))
        ));
    });
}

#[test]
fn tlcx18_owned_current_requires_actual_plan_candidate_lease_and_direction() {
    fixture(|plan, candidate| {
        let b = broker();
        let run = begin_run(&b, plan.scope().source.digest(), Cancellation::default());
        let bounds = requirements(plan).unwrap();
        let reservation = reserve(&run, bounds);
        let lease = reservation.lease();
        let owned =
            measure_owned(plan, candidate, Direction::Soundness, &[], work(), &lease).unwrap();
        let clone = ProbePlan::checked(
            plan.scope(),
            plan.fixed_probes()[0].template.source,
            plan.fixed_probes(),
            work(),
        )
        .unwrap();
        assert!(matches!(
            owned.project_current(&clone, candidate, Direction::Soundness, &lease),
            Err(OwnedError::BindingMismatch)
        ));
        let clone = Candidate::given(
            candidate.id(),
            plan.scope().comparison,
            plan.fixed_probes()[0].template.target,
            candidate.revision(),
            candidate.context(),
        )
        .unwrap();
        assert!(matches!(
            owned.project_current(plan, &clone, Direction::Soundness, &lease),
            Err(OwnedError::BindingMismatch)
        ));
        assert!(matches!(
            owned.project_current(plan, candidate, Direction::Completeness, &lease),
            Err(OwnedError::BindingMismatch)
        ));
        let other_broker = broker();
        let other_run = begin_run(
            &other_broker,
            plan.scope().source.digest(),
            Cancellation::default(),
        );
        let other_reservation = reserve(&other_run, bounds);
        let other = other_reservation.lease();
        assert_eq!(lease.run_id(), other.run_id());
        assert_eq!(lease.reservation_id(), other.reservation_id());
        assert!(matches!(
            owned.project_current(plan, candidate, Direction::Soundness, &other),
            Err(OwnedError::ForeignReservation)
        ));
    });
}

#[test]
fn tlcx18_owned_measurement_failure_releases_both_claims() {
    fixture(|plan, candidate| {
        let b = broker();
        let run = begin_run(&b, plan.scope().source.digest(), Cancellation::default());
        let bounds = requirements(plan).unwrap();
        let reservation = reserve(&run, bounds);
        let lease = reservation.lease();
        let low = WorkBudget {
            max_work: 1,
            max_payload_bytes: 1_000_000,
        };
        assert!(matches!(
            measure_owned(plan, candidate, Direction::Soundness, &[], low, &lease),
            Err(OwnedError::Measurement(BridgeError::WorkBudget))
        ));
        let _refunded = lease
            .claim_resources(&[
                request(ResourceKind::Ram, bounds.scratch_ram + bounds.retained_ram),
                request(ResourceKind::Output, bounds.retained_output),
            ])
            .unwrap();
    });
}
