//! Actual finite evaluator -> fixed K1 producer -> four-record consumer.
use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::{
    facet_ledger as l, facet_measurement_bridge as bridge, formal_boolean as f,
    formal_measurement as m,
};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::Digest32;

fn d(value: u8) -> Digest32 {
    Digest32::from_bytes([value; 32])
}
fn budget() -> m::WorkBudget {
    m::WorkBudget {
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
fn plan_scope() -> m::PlanScope {
    let source = SourceRevision::of("declared source");
    m::PlanScope {
        comparison: d(1),
        definition: d(2),
        probe_set: d(3),
        source,
        context: ContextRevision::original(source),
    }
}
fn declared() -> bridge::DeclaredFacetScope {
    bridge::DeclaredFacetScope {
        baseline: l::Revision([7; 32]),
        root_goal: l::Revision([8; 32]),
        policy: l::Revision([9; 32]),
    }
}
fn obligations() -> [bridge::ProbeObligation<'static>; 1] {
    [bridge::ProbeObligation {
        identity: b"protected-p",
        root: l::ObligationId([10; 32]),
        property: l::PropertyId([11; 32]),
    }]
}
fn request<'a>(
    source: &'a SmtStatusQuery,
    target: &'a SmtStatusQuery,
    predicate: &'a BoolExpr,
    target_predicate: &'a BoolExpr,
    symbols: &'a [(BoolSymbolId, BoolSymbolId)],
) -> f::Request<'a> {
    let revision = SourceRevision::of("declared target");
    let mut r = f::Request {
        source,
        target,
        source_predicate: predicate,
        target_predicate,
        symbols,
        unconstrained: f::UnconstrainedPolicy::ExplicitlyGiven,
        binding: f::Binding {
            source: plan_scope().source,
            target: revision,
            source_context: plan_scope().context,
            target_context: ContextRevision::original(revision),
            probe_set: d(3),
            probe_map: d(0),
        },
    };
    r.binding.probe_map = f::probe_map_digest(&r, 10000).unwrap();
    r
}
fn computed<'a>(request: &'a f::Request<'a>) -> f::ComputedReport<'a> {
    match f::evaluate(request, 10000).unwrap() {
        f::Evaluation::Computed(report) => report,
        other => panic!("expected actual computed report: {other:?}"),
    }
}
fn candidate(query: &SmtStatusQuery, id: u8) -> m::Candidate<'_> {
    let revision = SourceRevision::of("declared target");
    m::Candidate::given(
        d(id),
        d(1),
        query,
        revision,
        ContextRevision::original(revision),
    )
    .unwrap()
}

#[test]
fn tlcx17_ingress_actual_countermodel_creates_formal_defect_and_source_unverified_gap() {
    let q = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let yes = BoolExpr::Constant(true);
    let symbols = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let r = request(&q, &q, &p, &yes, &symbols);
    let computed = computed(&r);
    let weight = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [m::FixedProbe {
        identity: b"protected-p",
        weight: weight.lower(),
        template: &r,
    }];
    let plan = m::ProbePlan::checked(plan_scope(), &q, &probes, budget()).unwrap();
    let candidate = candidate(&q, 12);
    let observations = [m::ProbeObservation {
        identity: b"protected-p",
        candidate: d(12),
        scope: d(1),
        value: m::ObservationValue::Computed(&computed),
    }];
    let measured = plan
        .measure(&candidate, m::Direction::Soundness, &observations, budget())
        .unwrap();
    let obligations = obligations();
    let batch = bridge::MeasuredFacetBatch::from_loss(&measured, declared(), &obligations).unwrap();
    assert_eq!(
        batch.measurement().bounds(),
        &RationalInterval::exact_ratio(1, 1).unwrap()
    );
    assert_eq!(
        batch.source_interpretation(),
        f::SourceInterpretation::Unverified
    );
    assert_eq!(batch.mapping_basis(), f::MappingBasis::Given);
    assert_eq!(
        batch.checker_independence(),
        f::CheckerIndependence::UnassessedSharedIr
    );
    let id = batch.binding().id;
    let mut store = bridge::FacetMeasurementLedger::new(2, 8).unwrap();
    store.admit(batch).unwrap();
    assert_eq!(store.records().len(), 1);
    assert!(std::ptr::eq(store.records()[0].measurement(), &measured));
    assert_eq!(
        store
            .ledger()
            .for_transform(id)
            .filter(|e| matches!(e.facet, l::Facet::Defect(_)))
            .count(),
        1
    );
    assert!(store.ledger().for_transform(id).any(|e| matches!(
        e.facet,
        l::Facet::Uncertainty(l::Gap {
            reason: l::GapReason::Unverified,
            ..
        })
    )));
}

#[test]
fn tlcx17_all_missing_measurement_never_becomes_zero_or_confirmed_defect() {
    let q = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let symbols = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let r = request(&q, &q, &p, &p, &symbols);
    let weight = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [m::FixedProbe {
        identity: b"protected-p",
        weight: weight.lower(),
        template: &r,
    }];
    let plan = m::ProbePlan::checked(plan_scope(), &q, &probes, budget()).unwrap();
    let candidate = candidate(&q, 12);
    let measured = plan
        .measure(&candidate, m::Direction::Completeness, &[], budget())
        .unwrap();
    let obligations = obligations();
    let batch = bridge::MeasuredFacetBatch::from_loss(&measured, declared(), &obligations).unwrap();
    assert_eq!(
        batch.measurement().bounds(),
        &RationalInterval::from_ratio(0, 1, 1, 1).unwrap()
    );
    assert!(!batch
        .entries()
        .iter()
        .any(|e| matches!(e.facet, l::Facet::Defect(_))));
    assert!(batch.entries().iter().any(|e| matches!(
        e.facet,
        l::Facet::Uncertainty(l::Gap {
            reason: l::GapReason::Unobserved,
            ..
        })
    )));
}

#[test]
fn tlcx17_capacity_duplicate_and_foreign_obligations_leave_store_unchanged() {
    let q = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let symbols = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let r = request(&q, &q, &p, &p, &symbols);
    let weight = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [m::FixedProbe {
        identity: b"protected-p",
        weight: weight.lower(),
        template: &r,
    }];
    let plan = m::ProbePlan::checked(plan_scope(), &q, &probes, budget()).unwrap();
    let c = candidate(&q, 12);
    let measured = plan
        .measure(&c, m::Direction::Soundness, &[], budget())
        .unwrap();
    let obligations = obligations();
    let batch =
        || bridge::MeasuredFacetBatch::from_loss(&measured, declared(), &obligations).unwrap();
    let mut too_small = bridge::FacetMeasurementLedger::new(2, 1).unwrap();
    assert!(too_small.admit(batch()).is_err());
    assert!(too_small.ledger().transforms().is_empty());
    assert!(too_small.records().is_empty());
    assert!(too_small.ledger().entries().is_empty());
    let mut store = bridge::FacetMeasurementLedger::new(2, 10).unwrap();
    store.admit(batch()).unwrap();
    let before = (
        store.ledger().transforms().to_vec(),
        store.ledger().entries().to_vec(),
    );
    assert!(store.admit(batch()).is_err());
    assert_eq!(before.0, store.ledger().transforms());
    assert_eq!(before.1, store.ledger().entries());
    let foreign = [bridge::ProbeObligation {
        identity: b"foreign",
        ..obligations[0]
    }];
    assert!(bridge::MeasuredFacetBatch::from_loss(&measured, declared(), &foreign).is_err());
    assert!(bridge::MeasuredFacetBatch::from_loss(&measured, declared(), &[]).is_err());
}

#[test]
fn tlcx17_paired_actual_loss_gain_uses_fixed_probe_nodes_and_preserves_basis() {
    let q = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let target = SmtStatusQuery::checked(
        vec![BoolSymbolId::new(0)],
        vec![],
        vec![BoolExpr::Not(Box::new(p.clone()))],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let symbols = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let a = request(&q, &q, &p, &p, &symbols);
    let b = request(&q, &target, &p, &p, &symbols);
    let ra = computed(&a);
    let rb = computed(&b);
    let weight = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [m::FixedProbe {
        identity: b"protected-p",
        weight: weight.lower(),
        template: &a,
    }];
    let plan = m::ProbePlan::checked(plan_scope(), &q, &probes, budget()).unwrap();
    let ca = candidate(&q, 7);
    let cb = candidate(&target, 12);
    let oa = [m::ProbeObservation {
        identity: b"protected-p",
        candidate: d(7),
        scope: d(1),
        value: m::ObservationValue::Computed(&ra),
    }];
    let ob = [m::ProbeObservation {
        identity: b"protected-p",
        candidate: d(12),
        scope: d(1),
        value: m::ObservationValue::Computed(&rb),
    }];
    let baseline = plan
        .measure(&ca, m::Direction::Completeness, &oa, budget())
        .unwrap();
    let measured = plan
        .measure(&cb, m::Direction::Completeness, &ob, budget())
        .unwrap();
    let obligations = obligations();
    let batch = bridge::MeasuredFacetBatch::from_loss(&measured, declared(), &obligations)
        .unwrap()
        .with_paired_preservation_gain(&baseline)
        .unwrap();
    assert_eq!(
        batch.gain_bounds(),
        Some(&RationalInterval::exact_ratio(-1, 1).unwrap())
    );
    assert!(batch.entries().iter().any(|e| matches!(
        e.facet,
        l::Facet::Gain(l::Gain {
            kind: l::GainKind::ObservedReported,
            ..
        })
    )));
    let wrong = bridge::DeclaredFacetScope {
        baseline: l::Revision([99; 32]),
        ..declared()
    };
    assert!(
        bridge::MeasuredFacetBatch::from_loss(&measured, wrong, &obligations)
            .unwrap()
            .with_paired_preservation_gain(&baseline)
            .is_err()
    );
}

#[test]
fn tlcx17_checked_context_delta_is_consumed_without_erasing_source_gap() {
    use lc631_core::{
        context::{
            ContextAdmission, ContextApplicability, ContextConsumer, ContextDelta, ContextLimits,
            ContextPremise, ContextScope, OriginalContext, PremiseBasis, PremiseEdit,
        },
        AuthorityRevision,
    };
    use std::sync::Arc;
    let source = SourceRevision::of("declared source");
    let scope = ContextScope::declared(d(1), d(8), d(3), d(2));
    let authority = AuthorityRevision(1);
    let original = OriginalContext::checked(
        source,
        scope,
        authority,
        vec![],
        ContextLimits {
            max_contexts: 4,
            max_premises: 4,
            max_edits: 4,
            max_statement_bytes: 1024,
        },
    )
    .unwrap();
    let parent = original.state().revision();
    let mut contexts = ContextConsumer::new(original).unwrap();
    let delta = ContextDelta::proposed(
        parent,
        scope,
        authority,
        ContextApplicability::Unconditional,
        vec![PremiseEdit::Add(ContextPremise::reported(
            d(11),
            Arc::from("proposed premise"),
            PremiseBasis::Proposed,
            true,
        ))],
    );
    let child = match contexts.admit(delta).unwrap() {
        ContextAdmission::Inserted(revision) => revision,
        other => panic!("expected inserted child: {other:?}"),
    };
    let child = contexts.get(child).unwrap();
    let q = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let symbols = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let mut r = f::Request {
        source: &q,
        target: &q,
        source_predicate: &p,
        target_predicate: &p,
        symbols: &symbols,
        unconstrained: f::UnconstrainedPolicy::ExplicitlyGiven,
        binding: f::Binding {
            source,
            target: source,
            source_context: parent,
            target_context: child.revision(),
            probe_set: d(3),
            probe_map: d(0),
        },
    };
    r.binding.probe_map = f::probe_map_digest(&r, 10000).unwrap();
    let report = computed(&r);
    let weight = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [m::FixedProbe {
        identity: b"protected-p",
        weight: weight.lower(),
        template: &r,
    }];
    let plan = m::ProbePlan::checked(
        m::PlanScope {
            source,
            context: parent,
            ..plan_scope()
        },
        &q,
        &probes,
        budget(),
    )
    .unwrap();
    let candidate = m::Candidate::given(d(12), d(1), &q, source, child.revision()).unwrap();
    let obs = [m::ProbeObservation {
        identity: b"protected-p",
        candidate: d(12),
        scope: d(1),
        value: m::ObservationValue::Computed(&report),
    }];
    let measured = plan
        .measure(&candidate, m::Direction::Soundness, &obs, budget())
        .unwrap();
    let obligations = obligations();
    let references = bridge::DeclaredDeltaReferences {
        reason: l::EvidenceRef([21; 32]),
        permission_basis: l::EvidenceRef([22; 32]),
        map_back: l::EvidenceRef([23; 32]),
    };
    let batch = bridge::MeasuredFacetBatch::from_loss(&measured, declared(), &obligations)
        .unwrap()
        .with_context_delta(child, references)
        .unwrap();
    assert!(std::ptr::eq(batch.context_delta().unwrap(), child));
    assert!(batch.entries().iter().any(|e| matches!(
        e.facet,
        l::Facet::Delta(l::Delta {
            kind: l::EditKind::AddPremise,
            ..
        })
    )));
    assert!(batch.entries().iter().any(|e| matches!(
        e.facet,
        l::Facet::Uncertainty(l::Gap {
            reason: l::GapReason::Unverified,
            ..
        })
    )));
    assert!(
        bridge::MeasuredFacetBatch::from_loss(&measured, declared(), &obligations)
            .unwrap()
            .with_context_delta(contexts.get(parent).unwrap(), references)
            .is_err()
    );
}

#[test]
fn tlcx17_gap_only_records_bind_actual_target_theory_instead_of_candidate_label() {
    let q = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let other = SmtStatusQuery::checked(
        vec![BoolSymbolId::new(0)],
        vec![],
        vec![p.clone()],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let symbols = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let r = request(&q, &q, &p, &p, &symbols);
    let weight = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [m::FixedProbe {
        identity: b"protected-p",
        weight: weight.lower(),
        template: &r,
    }];
    let plan = m::ProbePlan::checked(plan_scope(), &q, &probes, budget()).unwrap();
    let a = candidate(&q, 12);
    let b = candidate(&other, 12);
    let ma = plan
        .measure(&a, m::Direction::Soundness, &[], budget())
        .unwrap();
    let mb = plan
        .measure(&b, m::Direction::Soundness, &[], budget())
        .unwrap();
    let obligations = obligations();
    let ba = bridge::MeasuredFacetBatch::from_loss(&ma, declared(), &obligations).unwrap();
    let bb = bridge::MeasuredFacetBatch::from_loss(&mb, declared(), &obligations).unwrap();
    assert_ne!(ba.binding().query, bb.binding().query);
    assert_ne!(ba.binding().id, bb.binding().id);
    assert_eq!(ba.measurement().bounds(), bb.measurement().bounds());
}
