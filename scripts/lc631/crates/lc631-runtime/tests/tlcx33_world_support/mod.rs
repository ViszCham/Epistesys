//! Shared finite-Boolean fixture and real resource reservations for two suites.
#![allow(dead_code)] // Each integration suite exercises a different fixture subset.
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_core::measurement_numeric::{Rational, RationalInterval as Interval};
use lc631_runtime::formal_measurement::owned;
use lc631_runtime::joint_selection::formal::{
    reduction_policy_digest, BothDirectionReduction as Reduction,
};
use lc631_runtime::joint_selection::SelectionPins;
use lc631_runtime::resource::{Reservation, ResourceBroker, ResourceLimits, RunResources};
use lc631_runtime::{formal_boolean as f, formal_measurement as m};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceKind, ResourceRequest, ResourceReservationView,
};
use lc631_substrate::Digest32;
use std::time::Duration;

pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn work() -> m::WorkBudget {
    m::WorkBudget {
        max_work: 100_000,
        max_payload_bytes: 1_000_000,
    }
}
pub fn req(kind: ResourceKind, n: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap())
}
pub fn pins(plan: &m::ProbePlan<'_>) -> SelectionPins {
    let s = plan.scope();
    SelectionPins {
        source: s.source,
        scope: ContextScope::declared(s.comparison, d(20), s.probe_set, d(21)),
        baseline: d(22),
        hard_invariants: d(23),
        policy: d(24),
        goal_profile: Some(d(25)),
        evidence_revision: d(26),
    }
}
pub fn broker() -> ResourceBroker {
    ResourceBroker::new(ResourceLimits::uniform(64 * 1024 * 1024).unwrap()).unwrap()
}
pub fn run(b: &ResourceBroker, source: Digest32, cancel: Cancellation) -> RunResources {
    b.begin_run(
        1,
        source,
        ResourceLimits::uniform(64 * 1024 * 1024).unwrap(),
        Deadline::after(Duration::from_secs(60)).unwrap(),
        cancel,
    )
    .unwrap()
}
pub fn reserve(run: &RunResources, ram: u64, output: u64) -> Reservation {
    run.reserve(&[
        req(ResourceKind::Ram, ram),
        req(ResourceKind::Output, output),
    ])
    .unwrap()
}
pub fn setup(c: &Cases<'_>, cancel: Cancellation) -> (ResourceBroker, RunResources, Reservation) {
    let b = broker();
    let run = run(&b, c.plan.scope().source.digest(), cancel);
    let r = reserve(&run, 32 * 1024 * 1024, 1024 * 1024);
    (b, run, r)
}
fn computed<'a>(r: &'a f::Request<'a>) -> f::ComputedReport<'a> {
    match f::evaluate(r, 100_000).unwrap() {
        f::Evaluation::Computed(report) => report,
        other => panic!("{other:?}"),
    }
}
pub struct Cases<'a> {
    pub plan: &'a m::ProbePlan<'a>,
    pub good: &'a m::Candidate<'a>,
    pub bad: &'a m::Candidate<'a>,
    pub good_obs: &'a [m::ProbeObservation<'a>],
    pub bad_obs: &'a [m::ProbeObservation<'a>],
}
pub fn missing_pair<'a, 'v>(
    c: &Cases<'a>,
    view: &'v ResourceReservationView<'_>,
) -> (
    owned::OwnedFormalLoss<'a, 'v>,
    owned::OwnedFormalLoss<'a, 'v>,
) {
    (
        owned::measure_owned(c.plan, c.good, m::Direction::Soundness, &[], work(), view).unwrap(),
        owned::measure_owned(
            c.plan,
            c.good,
            m::Direction::Completeness,
            &[],
            work(),
            view,
        )
        .unwrap(),
    )
}
pub fn policy<'a>(
    mut p: SelectionPins,
    s: &'a Rational,
    c: &'a Rational,
    view: &ResourceReservationView<'_>,
) -> Reduction<'a> {
    p.policy = reduction_policy_digest(p, d(2), s, c, view).unwrap();
    Reduction::declared_registered(p, p.policy, d(2), s, c)
}
pub fn cases_in_context(context: ContextRevision, body: impl for<'a> FnOnce(Cases<'a>)) {
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let source = SmtStatusQuery::checked(
        vec![BoolSymbolId::new(0)],
        vec![],
        vec![],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let good = SmtStatusQuery::checked(
        vec![BoolSymbolId::new(0)],
        vec![],
        vec![],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let bad = SmtStatusQuery::checked(
        vec![BoolSymbolId::new(0)],
        vec![],
        vec![BoolExpr::Not(Box::new(p.clone()))],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let revision = SourceRevision::of("given source interpreted into two target theories");
    let bad_context = context.reconstructed([31; 32]);
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let mut good_r = f::Request {
        source: &source,
        target: &good,
        source_predicate: &p,
        target_predicate: &p,
        symbols: &map,
        unconstrained: f::UnconstrainedPolicy::ExplicitlyGiven,
        binding: f::Binding {
            source: revision,
            target: revision,
            source_context: context,
            target_context: context,
            probe_set: d(3),
            probe_map: d(0),
        },
    };
    good_r.binding.probe_map = f::probe_map_digest(&good_r, 100_000).unwrap();
    let bad_r = f::Request {
        source: &source,
        target: &bad,
        source_predicate: &p,
        target_predicate: &p,
        symbols: &map,
        unconstrained: f::UnconstrainedPolicy::ExplicitlyGiven,
        binding: f::Binding {
            target_context: bad_context,
            ..good_r.binding
        },
    };
    let good_report = computed(&good_r);
    let bad_report = computed(&bad_r);
    assert_ne!(
        good_report.target_theory_digest(),
        bad_report.target_theory_digest()
    );
    assert_eq!(bad_report.soundness(), f::DirectionResult::Preserved);
    assert!(matches!(
        bad_report.completeness(),
        f::DirectionResult::Countermodel(_)
    ));
    let weight = Interval::exact_ratio(1, 1).unwrap();
    let probes = [m::FixedProbe {
        identity: b"protected-p",
        weight: weight.lower(),
        template: &good_r,
    }];
    let scope = m::PlanScope {
        comparison: d(1),
        definition: d(2),
        probe_set: d(3),
        source: revision,
        context,
    };
    let plan = m::ProbePlan::checked(scope, &source, &probes, work()).unwrap();
    let good_candidate = m::Candidate::given(d(4), d(1), &good, revision, context).unwrap();
    let bad_candidate = m::Candidate::given(d(5), d(1), &bad, revision, bad_context).unwrap();
    let good_obs = [m::ProbeObservation {
        identity: b"protected-p",
        candidate: d(4),
        scope: d(1),
        value: m::ObservationValue::Computed(&good_report),
    }];
    let bad_obs = [m::ProbeObservation {
        identity: b"protected-p",
        candidate: d(5),
        scope: d(1),
        value: m::ObservationValue::Computed(&bad_report),
    }];
    body(Cases {
        plan: &plan,
        good: &good_candidate,
        bad: &bad_candidate,
        good_obs: &good_obs,
        bad_obs: &bad_obs,
    });
}
