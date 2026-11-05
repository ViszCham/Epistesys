//! 日本語: 生成ownerから実finite producer/K1への接続・切断試験。
//! English: Generated-owner to actual finite-producer/K1 connection and cut tests.
//! These tests require central module registration; no execution is claimed here.
use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::context::{
    ContextLimits, ContextPremise, ContextScope, OriginalContext, PremiseBasis,
    SourceRevision,
};
use lc631_core::AuthorityRevision;
use lc631_runtime::creative_candidate_bridge::{measurement::*, *};
use lc631_runtime::formal_boolean as f;
use lc631_runtime::formal_measurement as m;
use lc631_runtime::formal_measurement::produced::ScheduledProbe;
use lc631_runtime::resource::{ResourceBroker, ResourceLimits};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceKind, ResourceRequest,
};
use lc631_substrate::source::{SourceSpan, SourceStore};
use lc631_substrate::Digest32;
use std::{cell::Cell, sync::Arc, time::Duration};

fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn var(n: u32) -> BoolExpr {
    BoolExpr::Variable(BoolSymbolId::new(n))
}
fn query(assertions: Vec<BoolExpr>) -> SmtStatusQuery {
    SmtStatusQuery::checked(
        vec![BoolSymbolId::new(0), BoolSymbolId::new(1)],
        vec![],
        assertions,
        SmtEmitLimits::default(),
    )
    .unwrap()
}
fn original(source: &SourceStore) -> OriginalContext {
    OriginalContext::checked(
        SourceRevision::of(source.text()),
        ContextScope::declared(d(1), d(2), d(3), d(4)),
        AuthorityRevision(7),
        vec![
            ContextPremise::from_source(
                d(5), source, SourceSpan::checked(source, 0, 7).unwrap(),
                PremiseBasis::Given, true,
            ).unwrap(),
            ContextPremise::from_source(
                d(6), source, SourceSpan::checked(source, 8, 9).unwrap(),
                PremiseBasis::Given, false,
            ).unwrap(),
        ],
        ContextLimits {
            max_contexts: 8, max_premises: 2, max_edits: 1, max_statement_bytes: 8192,
        },
    ).unwrap()
}
fn limits() -> BridgeLimits {
    BridgeLimits {
        max_candidates: 4, max_ast_nodes: 128,
        max_validation_checks: 10_000, max_payload_bytes: 8192,
    }
}
fn work() -> m::WorkBudget {
    m::WorkBudget { max_work: 100_000, max_payload_bytes: 1_000_000 }
}
fn resource(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}
#[derive(Clone, Copy)]
enum Variant {
    Full, Cut, Unknown, CopyTarget, CopySource, CandidateId, CandidateContext,
    Scope, ProbeContext, RequestSwap, ForeignBridge, ForeignReservation,
    CancelBefore, CancelInside, OutputExhausted,
}
#[derive(Debug)]
struct Summary {
    sound: RationalInterval,
    complete: RationalInterval,
    produced: usize,
    missing: usize,
    sound_probe: m::ProbeState,
}

// All oracle expectations below are literal hand-calculated values. This helper
// supplies actual queries/operators, never reported statuses or numeric inputs.
fn run(operator: Operator, variant: Variant, calls: &Cell<usize>) -> Result<Summary, Error> {
    let source = SourceStore::new(Arc::from("p and q\np"), 128).unwrap();
    let source_query = query(vec![BoolExpr::And(vec![var(0), var(1)]), var(0)]);
    let source_copy = source_query.clone();
    let goal_query = query(vec![var(0)]);
    let bindings = [
        GivenPremise {
            id: d(5), span: SourceSpan::checked(&source, 0, 7).unwrap(),
            formula: &source_query.assertions()[0],
        },
        GivenPremise {
            id: d(6), span: SourceSpan::checked(&source, 8, 9).unwrap(),
            formula: &source_query.assertions()[1],
        },
    ];
    let broker = ResourceBroker::new(ResourceLimits::uniform(64 * 1024 * 1024).unwrap()).unwrap();
    let cancellation = Cancellation::default();
    let run = broker.begin_run(
        1, source.id().digest(), ResourceLimits::uniform(64 * 1024 * 1024).unwrap(),
        Deadline::after(Duration::from_secs(60)).unwrap(), cancellation.clone(),
    ).unwrap();
    let reservation = run.reserve(&[
        resource(ResourceKind::Ram, 32 * 1024 * 1024),
        resource(ResourceKind::Output, 4 * 1024 * 1024),
    ]).unwrap();
    let view = reservation.lease();
    let goal = GoalPolicy { revision: d(2), query: &goal_query };
    let mut bridge = CreativeCandidateBridge::with_resources(
        &source, original(&source), d(5), &bindings, goal, limits(), &view,
    ).unwrap();
    let generated = bridge.generate(operator, &view).unwrap();
    assert_eq!(bridge.proposals().len(), 1);
    assert_eq!(generated.validation().assignments(), 4);
    assert!(generated.validation().feasible_models() > 0);
    assert_eq!(generated.validation().boundary(),
        ValidationBoundary::SeparateAlgorithmSharedIrGivenSource);
    let identity = generated.measurement_identity_current(&bridge, &view).unwrap();
    let target = generated.project_current(&bridge, &view).unwrap();
    let target_copy = target.clone();
    let source_input = if matches!(variant, Variant::CopySource) { &source_copy } else { &source_query };
    let target_input = if matches!(variant, Variant::CopyTarget) { &target_copy } else { target };
    let parent = generated.prepared().parent();
    let context = generated.prepared().target();
    let predicate = BoolExpr::Not(Box::new(var(1)));
    let other_predicate = BoolExpr::Constant(true);
    let symbols = [(BoolSymbolId::new(0), BoolSymbolId::new(0)),
        (BoolSymbolId::new(1), BoolSymbolId::new(1))];
    let mut request = f::Request {
        source: source_input, target: target_input,
        source_predicate: &predicate, target_predicate: &predicate,
        symbols: &symbols, unconstrained: f::UnconstrainedPolicy::ExplicitlyGiven,
        binding: f::Binding {
            source: parent.source(), target: context.source(), source_context: parent,
            target_context: if matches!(variant, Variant::ProbeContext) { parent } else { context },
            probe_set: d(3), probe_map: d(0),
        },
    };
    request.binding.probe_map = f::probe_map_digest(&request, 10_000).unwrap();
    let weight = RationalInterval::exact_ratio(1, 1).unwrap();
    let fixed = [m::FixedProbe { identity: b"protected-negative-q", weight: weight.lower(), template: &request }];
    let scope = m::PlanScope {
        comparison: if matches!(variant, Variant::Scope) { d(99) } else { d(1) },
        definition: d(9), probe_set: d(3), source: parent.source(), context: parent,
    };
    let plan = m::ProbePlan::checked(scope, source_input, &fixed, work()).unwrap();
    let candidate = m::Candidate::given(
        if matches!(variant, Variant::CandidateId) { d(99) } else { identity },
        scope.comparison, target_input, context.source(),
        if matches!(variant, Variant::CandidateContext) { parent } else { context },
    ).unwrap();
    let swapped = f::Request {
        source_predicate: &other_predicate, ..request
    };
    let schedule = [ScheduledProbe {
        identity: b"protected-negative-q",
        request: if matches!(variant, Variant::RequestSwap) { &swapped } else { &request },
        max_work: if matches!(variant, Variant::Unknown) { 1 } else { 10_000 },
    }];
    let foreign_bridge = CreativeCandidateBridge::with_resources(
        &source, original(&source), d(5), &bindings, goal, limits(), &view,
    ).unwrap();
    let foreign_reservation = run.reserve(&[
        resource(ResourceKind::Ram, 16 * 1024 * 1024),
        resource(ResourceKind::Output, 1024 * 1024),
    ]).unwrap();
    let foreign_view = foreign_reservation.lease();
    let exhausted = if matches!(variant, Variant::OutputExhausted) {
        Some(view.claim_resources(&[resource(ResourceKind::Output,
            4 * 1024 * 1024 - CANDIDATE_OUTPUT)]).unwrap())
    } else { None };
    if matches!(variant, Variant::CancelBefore) { cancellation.cancel(); }
    let result = generated.with_formal_measurement_current(
        if matches!(variant, Variant::ForeignBridge) { &foreign_bridge } else { &bridge },
        MeasurementInput {
            source: source_input, plan: &plan, candidate: &candidate,
            schedule: if matches!(variant, Variant::Cut) { &[] } else { &schedule },
            budget: work(),
        },
        if matches!(variant, Variant::ForeignReservation) { &foreign_view } else { &view },
        |pair| {
            calls.set(calls.get() + 1);
            let sound = pair.soundness();
            let complete = pair.completeness();
            assert_eq!(sound.probes().len(), 1);
            assert_eq!(sound.mapping_basis(), f::MappingBasis::Given);
            assert_eq!(sound.source_interpretation(), f::SourceInterpretation::Unverified);
            assert_eq!(sound.checker_independence(), f::CheckerIndependence::UnassessedSharedIr);
            assert!(std::ptr::eq(sound.plan(), &plan));
            assert!(std::ptr::eq(sound.candidate(), &candidate));
            assert!(std::ptr::eq(sound.probes()[0].fixed().template, &request));
            let summary = Summary {
                sound: sound.bounds().clone(), complete: complete.bounds().clone(),
                produced: pair.produced_count(), missing: pair.missing_count(),
                sound_probe: sound.probes()[0].state(),
            };
            if matches!(variant, Variant::CancelInside) { cancellation.cancel(); }
            summary
        },
    );
    drop(exhausted);
    if !matches!(variant, Variant::CancelBefore | Variant::CancelInside) {
        // Connector-local producer/observation/both K1 claims have all dropped.
        // The two bridge metadata claims and generated packet remain live.
        let returned = view.claim_resources(&[
            resource(ResourceKind::Ram,
                32 * 1024 * 1024 - 2 * METADATA_RAM - CANDIDATE_RAM),
            resource(ResourceKind::Output, 4 * 1024 * 1024 - CANDIDATE_OUTPUT),
        ]).unwrap();
        drop(returned);
    }
    result
}

fn weakening() -> Operator {
    Operator::Recombine { partner: d(6), connective: Connective::Or }
}

#[test]
fn tlcx21_generated_formal_exact_foreign_scope_and_request_cuts_do_not_consume() {
    for variant in [Variant::CopyTarget, Variant::CopySource, Variant::CandidateId,
        Variant::CandidateContext, Variant::Scope, Variant::ProbeContext,
        Variant::RequestSwap, Variant::ForeignBridge, Variant::ForeignReservation] {
        let calls = Cell::new(0);
        let expected = match variant {
            Variant::CopySource => Error::SourceQueryMismatch,
            Variant::Scope => Error::ScopeMismatch,
            Variant::ProbeContext => Error::ProbeBindingMismatch,
            Variant::RequestSwap => Error::Produced(
                m::produced::ProducedError::Bridge(m::BridgeError::LineageMismatch)),
            Variant::ForeignBridge | Variant::ForeignReservation =>
                Error::Creative(CreativeError::BindingMismatch),
            Variant::CopyTarget | Variant::CandidateId | Variant::CandidateContext =>
                Error::CandidateMismatch,
            _ => unreachable!("only the explicit binding variants above are tested"),
        };
        assert_eq!(run(weakening(), variant, &calls).unwrap_err(), expected);
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn tlcx21_generated_formal_reframe_and_soft_weakening_have_actual_distinct_losses() {
    let calls = Cell::new(0);
    let preserved = run(Operator::ReframeDeMorgan, Variant::Full, &calls).unwrap();
    assert_eq!(preserved.sound, RationalInterval::exact_ratio(0, 1).unwrap());
    assert_eq!(preserved.complete, RationalInterval::exact_ratio(0, 1).unwrap());
    let changed = run(weakening(), Variant::Full, &calls).unwrap();
    assert_eq!(changed.sound, RationalInterval::exact_ratio(1, 1).unwrap());
    assert_eq!(changed.complete, RationalInterval::exact_ratio(0, 1).unwrap());
    assert_eq!(changed.sound_probe, m::ProbeState::Countermodel(1));
    assert_eq!((changed.produced, changed.missing), (1, 0));
    assert_eq!(calls.get(), 2);
}

#[test]
fn tlcx21_generated_formal_dispatch_cut_and_work_unknown_retain_fixed_denominator() {
    for variant in [Variant::Cut, Variant::Unknown] {
        let calls = Cell::new(0);
        let measured = run(weakening(), variant, &calls).unwrap();
        let unknown = RationalInterval::from_ratio(0, 1, 1, 1).unwrap();
        assert_eq!(measured.sound, unknown);
        assert_eq!(measured.complete, unknown);
        assert_eq!(calls.get(), 1);
        if matches!(variant, Variant::Cut) {
            assert_eq!((measured.produced, measured.missing), (0, 1));
            assert_eq!(measured.sound_probe, m::ProbeState::Gap(m::Gap::Missing));
        } else {
            assert_eq!((measured.produced, measured.missing), (1, 0));
            assert_eq!(measured.sound_probe, m::ProbeState::Gap(m::Gap::Unknown));
        }
    }
    let restored = run(weakening(), Variant::Full, &Cell::new(0)).unwrap();
    assert_eq!(restored.sound, RationalInterval::exact_ratio(1, 1).unwrap());
}

#[test]
fn tlcx21_generated_formal_cancellation_rejects_result_without_callback_rollback_claim() {
    let calls = Cell::new(0);
    assert!(run(weakening(), Variant::CancelBefore, &calls).is_err());
    assert_eq!(calls.get(), 0);
    assert!(run(weakening(), Variant::CancelInside, &calls).is_err());
    // The read-only callback ran: result rejection does not reverse its effects.
    assert_eq!(calls.get(), 1);
}

#[test]
fn tlcx21_generated_formal_output_exhaustion_prevents_consumption() {
    let calls = Cell::new(0);
    assert!(run(weakening(), Variant::OutputExhausted, &calls).is_err());
    assert_eq!(calls.get(), 0);
}
