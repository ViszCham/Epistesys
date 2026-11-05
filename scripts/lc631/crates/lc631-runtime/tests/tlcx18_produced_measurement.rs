//! 実producer→K1 consumerの負例先行試験。 / Negative-first actual producer to K1 tests.
use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::formal_boolean as f;
use lc631_runtime::formal_measurement::{owned::measure_owned, produced::*, *};
use lc631_runtime::resource::{ResourceBroker, ResourceLimits};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceKind, ResourceRequest,
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
fn query() -> SmtStatusQuery {
    SmtStatusQuery::checked(
        vec![BoolSymbolId::new(0)],
        vec![],
        vec![],
        SmtEmitLimits::default(),
    )
    .unwrap()
}
fn scope() -> PlanScope {
    let source = SourceRevision::of("source");
    PlanScope {
        comparison: d(1),
        definition: d(2),
        probe_set: d(3),
        source,
        context: ContextRevision::original(source),
    }
}
fn req<'a>(
    q: &'a SmtStatusQuery,
    source: &'a BoolExpr,
    target: &'a BoolExpr,
    map: &'a [(BoolSymbolId, BoolSymbolId)],
) -> f::Request<'a> {
    let target_revision = SourceRevision::of("target");
    let mut request = f::Request {
        source: q,
        target: q,
        source_predicate: source,
        target_predicate: target,
        symbols: map,
        unconstrained: f::UnconstrainedPolicy::ExplicitlyGiven,
        binding: f::Binding {
            source: scope().source,
            target: target_revision,
            source_context: scope().context,
            target_context: ContextRevision::original(target_revision),
            probe_set: d(3),
            probe_map: d(0),
        },
    };
    request.binding.probe_map = f::probe_map_digest(&request, 10_000).unwrap();
    request
}
fn candidate(q: &SmtStatusQuery) -> Candidate<'_> {
    let revision = SourceRevision::of("target");
    Candidate::given(d(7), d(1), q, revision, ContextRevision::original(revision)).unwrap()
}
fn resource(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}

#[test]
fn tlcx18_produced_actual_reports_feed_fixed_k1_and_cut_keeps_denominator() {
    let q = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let yes = BoolExpr::Constant(true);
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let preserved = req(&q, &p, &p, &map);
    let broken = req(&q, &p, &yes, &map);
    let weights = [
        RationalInterval::exact_ratio(1, 1).unwrap(),
        RationalInterval::exact_ratio(3, 1).unwrap(),
    ];
    let probes = [
        FixedProbe {
            identity: b"preserved",
            weight: weights[0].lower(),
            template: &preserved,
        },
        FixedProbe {
            identity: b"broken",
            weight: weights[1].lower(),
            template: &broken,
        },
    ];
    let plan = ProbePlan::checked(scope(), &q, &probes, work()).unwrap();
    let candidate = candidate(&q);
    let broker = ResourceBroker::new(ResourceLimits::uniform(64 * 1024 * 1024).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            scope().source.digest(),
            ResourceLimits::uniform(64 * 1024 * 1024).unwrap(),
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            resource(ResourceKind::Ram, 32 * 1024 * 1024),
            resource(ResourceKind::Output, 4 * 1024 * 1024),
        ])
        .unwrap();
    let lease = reservation.lease();
    let full = [
        ScheduledProbe {
            identity: b"preserved",
            request: &preserved,
            max_work: 10_000,
        },
        ScheduledProbe {
            identity: b"broken",
            request: &broken,
            max_work: 10_000,
        },
    ];
    let produced = execute(&plan, &candidate, &full, work(), &lease).unwrap();
    assert_eq!(produced.produced_count(), 2);
    assert_eq!(produced.missing_count(), 0);
    let observations = produced.observations_current(&lease).unwrap();
    let measured = measure_owned(
        &plan,
        &candidate,
        Direction::Soundness,
        observations.as_slice(),
        work(),
        &lease,
    )
    .unwrap();
    assert_eq!(
        measured.historical().bounds(),
        &RationalInterval::exact_ratio(3, 4).unwrap()
    );
    assert_eq!(
        measured.historical().source_interpretation(),
        f::SourceInterpretation::Unverified
    );
    assert_eq!(
        measured.historical().checker_independence(),
        f::CheckerIndependence::UnassessedSharedIr
    );
    let cut = execute(&plan, &candidate, &full[..1], work(), &lease).unwrap();
    assert_eq!(cut.missing_count(), 1);
    let observations = cut.observations_current(&lease).unwrap();
    let cut_loss = measure_owned(
        &plan,
        &candidate,
        Direction::Soundness,
        observations.as_slice(),
        work(),
        &lease,
    )
    .unwrap();
    assert_eq!(
        cut_loss.historical().bounds(),
        &RationalInterval::from_ratio(0, 1, 3, 4).unwrap()
    );
    assert_eq!(
        cut_loss.historical().probes()[1].state(),
        ProbeState::Gap(Gap::Missing)
    );
}

#[test]
fn tlcx18_produced_rejects_foreign_duplicate_binding_and_work_before_publication() {
    let q = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let request = req(&q, &p, &p, &map);
    let weight = RationalInterval::exact_ratio(1, 1).unwrap();
    let fixed = [FixedProbe {
        identity: b"p",
        weight: weight.lower(),
        template: &request,
    }];
    let plan = ProbePlan::checked(scope(), &q, &fixed, work()).unwrap();
    let candidate = candidate(&q);
    let broker = ResourceBroker::new(ResourceLimits::uniform(64 * 1024 * 1024).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            scope().source.digest(),
            ResourceLimits::uniform(64 * 1024 * 1024).unwrap(),
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            resource(ResourceKind::Ram, 32 * 1024 * 1024),
            resource(ResourceKind::Output, 4 * 1024 * 1024),
        ])
        .unwrap();
    let lease = reservation.lease();
    let foreign = [ScheduledProbe {
        identity: b"foreign",
        request: &request,
        max_work: 10_000,
    }];
    assert!(matches!(
        execute(&plan, &candidate, &foreign, work(), &lease),
        Err(ProducedError::Bridge(BridgeError::ForeignProbe))
    ));
    let duplicate = [
        ScheduledProbe {
            identity: b"p",
            request: &request,
            max_work: 100,
        },
        ScheduledProbe {
            identity: b"p",
            request: &request,
            max_work: 100,
        },
    ];
    assert!(execute(&plan, &candidate, &duplicate, work(), &lease).is_err());
    let scheduled = [ScheduledProbe {
        identity: b"p",
        request: &request,
        max_work: 10_000,
    }];
    assert!(matches!(
        execute(
            &plan,
            &candidate,
            &scheduled,
            WorkBudget {
                max_work: 100,
                ..work()
            },
            &lease
        ),
        Err(ProducedError::Bridge(BridgeError::WorkBudget))
    ));
    let mut wrong = req(&q, &p, &p, &map);
    wrong.binding.target_context = wrong.binding.target_context.reconstructed([99; 32]);
    let stale = [ScheduledProbe {
        identity: b"p",
        request: &wrong,
        max_work: 100,
    }];
    assert!(matches!(
        execute(&plan, &candidate, &stale, work(), &lease),
        Err(ProducedError::Bridge(BridgeError::LineageMismatch))
    ));
}

#[test]
fn tlcx18_produced_budget_unknown_empty_coverage_and_cancel_remain_gaps() {
    let q = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let request = req(&q, &p, &p, &map);
    let weight = RationalInterval::exact_ratio(1, 1).unwrap();
    let fixed = [FixedProbe {
        identity: b"p",
        weight: weight.lower(),
        template: &request,
    }];
    let plan = ProbePlan::checked(scope(), &q, &fixed, work()).unwrap();
    let candidate = candidate(&q);
    let broker = ResourceBroker::new(ResourceLimits::uniform(64 * 1024 * 1024).unwrap()).unwrap();
    let cancel = Cancellation::default();
    let run = broker
        .begin_run(
            1,
            scope().source.digest(),
            ResourceLimits::uniform(64 * 1024 * 1024).unwrap(),
            Deadline::after(Duration::from_secs(60)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            resource(ResourceKind::Ram, 32 * 1024 * 1024),
            resource(ResourceKind::Output, 4 * 1024 * 1024),
        ])
        .unwrap();
    let lease = reservation.lease();
    let scheduled = [ScheduledProbe {
        identity: b"p",
        request: &request,
        max_work: 1,
    }];
    let produced = execute(&plan, &candidate, &scheduled, work(), &lease).unwrap();
    let observations = produced.observations_current(&lease).unwrap();
    let measured = measure_owned(
        &plan,
        &candidate,
        Direction::Soundness,
        observations.as_slice(),
        work(),
        &lease,
    )
    .unwrap();
    assert_eq!(
        measured.historical().bounds(),
        &RationalInterval::from_ratio(0, 1, 1, 1).unwrap()
    );
    assert_eq!(
        measured.historical().probes()[0].state(),
        ProbeState::Gap(Gap::Unknown)
    );
    let missing = execute(&plan, &candidate, &[], work(), &lease).unwrap();
    assert_eq!(missing.missing_count(), 1);
    cancel.cancel();
    assert!(produced.observations_current(&lease).is_err());
    assert!(missing.observations_current(&lease).is_err());
}

#[test]
fn tlcx18_produced_incomparable_missing_theory_scope_never_becomes_a_bound() {
    let q = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let mut request = req(&q, &p, &p, &map);
    request.unconstrained = f::UnconstrainedPolicy::MissingScope;
    request.binding.probe_map = f::probe_map_digest(&request, 10_000).unwrap();
    let weight = RationalInterval::exact_ratio(1, 1).unwrap();
    let fixed = [FixedProbe {
        identity: b"p",
        weight: weight.lower(),
        template: &request,
    }];
    let plan = ProbePlan::checked(scope(), &q, &fixed, work()).unwrap();
    let candidate = candidate(&q);
    let broker = ResourceBroker::new(ResourceLimits::uniform(64 * 1024 * 1024).unwrap()).unwrap();
    let run = broker
        .begin_run(
            1,
            scope().source.digest(),
            ResourceLimits::uniform(64 * 1024 * 1024).unwrap(),
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            resource(ResourceKind::Ram, 32 * 1024 * 1024),
            resource(ResourceKind::Output, 4 * 1024 * 1024),
        ])
        .unwrap();
    let lease = reservation.lease();
    let schedule = [ScheduledProbe {
        identity: b"p",
        request: &request,
        max_work: 10_000,
    }];
    let produced = execute(&plan, &candidate, &schedule, work(), &lease).unwrap();
    assert_eq!(
        produced.gap(0),
        Some(Gap::NoComparable(f::NoComparable::MissingTheoryScope))
    );
    assert!(matches!(
        produced.observations_current(&lease),
        Err(ProducedError::NoComparable(
            f::NoComparable::MissingTheoryScope
        ))
    ));
}
