//! 日本語: 固定query/実生成ownerだけを供給するfixture。観測statusは注入しない。
//! English: Supply fixed queries and actual generated owners, never reported outcomes.
use lc631_analysis::creativity_measurement::{ComponentReport, Rational, RationalInterval};
use lc631_core::{context::*, AuthorityRevision};
use lc631_runtime::{
    creative_candidate_bridge::{semantic_configuration::*, *},
    formal_measurement::WorkBudget,
    resource::{ResourceBroker, ResourceLimits},
};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, IntExpr, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::{
    resource::*, source::{SourceSpan, SourceStore}, Digest32,
};
use std::{cell::Cell, sync::Arc, time::Duration};

pub fn d(n: u8) -> Digest32 { Digest32::from_bytes([n; 32]) }
pub fn var(n: u32) -> BoolExpr { BoolExpr::Variable(BoolSymbolId::new(n)) }
fn query(assertions: Vec<BoolExpr>) -> SmtStatusQuery {
    SmtStatusQuery::checked(
        (0..3).map(BoolSymbolId::new).collect(), vec![], assertions, SmtEmitLimits::default(),
    ).unwrap()
}
pub fn work() -> WorkBudget {
    WorkBudget { max_work: 1_000_000, max_payload_bytes: 1_000_000 }
}
fn resource(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}
#[derive(Clone, Copy, Debug)]
pub enum Case { Change, Equivalent, Negative, InconsistentBaseline }
#[derive(Clone, Copy, Debug)]
pub enum Cut {
    None, MissingCandidateP, UnknownCandidateP, CopySource, ForeignBridge, ForeignLease,
    ForeignPlan, StaleDomain, RequiredRole, DisguisedHardR, EmptyProbes, DuplicateProbe, ZeroWeight,
    NumericBudget, MalformedWeight, UnsupportedProbe, ForeignDispatch, DuplicateDispatch,
    Tie, TieRenamed, TieReordered, Work, FollowUpWork, Payload, Output,
    CancelBefore, CancelInside, CancelFresh,
}
#[derive(Debug)]
pub struct Summary {
    pub report: ComponentReport,
    pub baseline_p: CanonicalOutcome,
    pub candidate_p: CanonicalOutcome,
    pub candidate_q: CanonicalOutcome,
    pub initial_queries: usize,
    pub follow_up: Option<CanonicalOutcome>,
    pub follow_up_counts: Option<(usize, usize)>,
    pub fresh_current_rejected: bool,
    pub candidate_models: u16,
    pub ambiguous_follow_up: bool,
}
fn original(source: &SourceStore, spans: &[(usize, usize); 3]) -> OriginalContext {
    OriginalContext::checked(
        SourceRevision::of(source.text()), ContextScope::declared(d(1), d(2), d(3), d(4)),
        AuthorityRevision(7),
        spans.iter().enumerate().map(|(i, &(start, end))| {
            ContextPremise::from_source(
                d(5 + i as u8), source, SourceSpan::checked(source, start, end).unwrap(),
                PremiseBasis::Given, i != 0,
            ).unwrap()
        }).collect(),
        ContextLimits { max_contexts: 8, max_premises: 3, max_edits: 1, max_statement_bytes: 8192 },
    ).unwrap()
}

pub fn run(case: Case, cut: Cut, calls: &Cell<usize>) -> Result<Summary, Error> {
    let equivalent = matches!(case, Case::Equivalent);
    let inconsistent = matches!(case, Case::InconsistentBaseline);
    let tied = matches!(cut, Cut::Tie | Cut::TieRenamed | Cut::TieReordered);
    let p_id: &[u8] = if matches!(cut, Cut::TieRenamed) { b"z" } else { b"optional-p" };
    let q_id: &[u8] = if matches!(cut, Cut::TieRenamed) { b"a" } else { b"optional-q" };
    let text = if equivalent { "r\np and q\nq" } else if inconsistent { "r\np\nnot p" } else { "r\np\nq" };
    let spans = if equivalent { [(0, 1), (2, 9), (10, 11)] } else if inconsistent {
        [(0, 1), (2, 3), (4, 9)]
    } else { [(0, 1), (2, 3), (4, 5)] };
    let source = SourceStore::new(Arc::from(text), 128).unwrap();
    let source_query = query(vec![var(0), if equivalent {
        BoolExpr::And(vec![var(1), var(2)])
    } else { var(1) }, if inconsistent { BoolExpr::Not(Box::new(var(1))) } else { var(2) }]);
    let source_copy = source_query.clone();
    let goal = query(vec![var(0)]);
    let bindings: Vec<_> = spans.iter().enumerate().map(|(i, &(start, end))| GivenPremise {
        id: d(5 + i as u8), span: SourceSpan::checked(&source, start, end).unwrap(),
        formula: &source_query.assertions()[i],
    }).collect();
    let broker = ResourceBroker::new(ResourceLimits::uniform(128 * 1024 * 1024).unwrap()).unwrap();
    let cancel = Cancellation::default();
    let run = broker.begin_run(1, source.id().digest(),
        ResourceLimits::uniform(128 * 1024 * 1024).unwrap(),
        Deadline::after(Duration::from_secs(60)).unwrap(), cancel.clone()).unwrap();
    let reservation = run.reserve(&[resource(ResourceKind::Ram, 64 * 1024 * 1024),
        resource(ResourceKind::Output, 8 * 1024 * 1024)]).unwrap();
    let view = reservation.lease();
    let limits = BridgeLimits { max_candidates: 4, max_ast_nodes: 128,
        max_validation_checks: 10_000, max_payload_bytes: 8192 };
    let goal_policy = GoalPolicy { revision: d(2), query: &goal };
    let mut bridge = CreativeCandidateBridge::with_resources(&source, original(&source, &spans),
        d(6), &bindings, goal_policy, limits, &view).unwrap();
    let foreign = CreativeCandidateBridge::with_resources(&source, original(&source, &spans),
        d(6), &bindings, goal_policy, limits, &view).unwrap();
    let foreign_reservation = run.reserve(&[resource(ResourceKind::Ram, 32 * 1024 * 1024),
        resource(ResourceKind::Output, 4 * 1024 * 1024)]).unwrap();
    let foreign_view = foreign_reservation.lease();
    let p = if matches!(case, Case::Negative) { BoolExpr::Not(Box::new(var(1))) } else { var(1) };
    let q = var(2);
    let unsupported = BoolExpr::LessEqual(IntExpr::Constant(0), IntExpr::Constant(1));
    // r AND r is semantically the actual hard/goal r, despite a different AST.
    let disguised_hard = BoolExpr::And(vec![var(0), var(0)]);
    let three = RationalInterval::exact_ratio(if matches!(cut, Cut::ZeroWeight) { 0 } else if tied { 1 } else { 3 }, 1).unwrap();
    let one = RationalInterval::exact_ratio(1, 1).unwrap();
    let oversized = Rational::from_integer(
        lc631_substrate::measurement_numeric::BigInt::from(1u8) << 65_537usize,
    );
    let malformed = Rational::new_raw(1.into(), 0.into());
    let mut probes = [ConfigurationProbe {
        identity: p_id, predicate: if matches!(cut, Cut::DisguisedHardR) { &disguised_hard }
            else if matches!(cut, Cut::UnsupportedProbe) { &unsupported } else { &p },
        weight: if matches!(cut, Cut::NumericBudget) { &oversized }
            else if matches!(cut, Cut::MalformedWeight) { &malformed } else { three.lower() },
        role: if matches!(cut, Cut::RequiredRole) { ProbeRole::Required } else { ProbeRole::Nonmandatory },
    }, ConfigurationProbe {
        identity: if matches!(cut, Cut::DuplicateProbe) { p_id } else { q_id },
        predicate: &q, weight: one.lower(), role: ProbeRole::Nonmandatory,
    }];
    if matches!(cut, Cut::TieReordered) { probes.swap(0, 1); }
    let declaration = ConfigurationDeclaration { oracle: d(21), domain: d(22), hard_query_work: 10_000 };
    let captured = ConfigurationPlan::capture_current(&bridge,
        if matches!(cut, Cut::CopySource) { &source_copy } else { &source_query },
        if matches!(cut, Cut::EmptyProbes) { &[] } else { &probes }, declaration, work(), &view);
    let plan = if inconsistent { captured.expect("structural capture must precede Gamma checking") } else { captured? };
    let other_plan = ConfigurationPlan::capture_current(&foreign, &source_query,
        &probes, declaration, work(), &view).expect("valid structural foreign plan capture");
    let stale_plan = ConfigurationPlan::capture_current(&bridge, &source_query,
        &probes, ConfigurationDeclaration { domain: d(99), ..declaration }, work(), &view)
        .expect("valid structural stale-domain plan capture");
    let generated = bridge.generate(if equivalent { Operator::ReframeDeMorgan } else {
        Operator::Recombine { partner: d(7), connective: Connective::Or }
    }, &view).unwrap();
    let candidate_models = generated.validation().feasible_models();
    if inconsistent {
        assert_eq!(candidate_models, 2, "consistent generated candidate must actually exist");
        assert_eq!(generated.validation().assignments(), 8);
    }
    assert_eq!(generated.validation().boundary(), ValidationBoundary::SeparateAlgorithmSharedIrGivenSource);
    let parent = generated.prepared().parent();
    let child = generated.prepared().target();
    assert_ne!(parent, child);
    assert_eq!(child.source(), parent.source());
    assert_eq!(bridge.contexts().get(parent).unwrap().premises()[0].basis(), PremiseBasis::Given);
    assert_eq!(bridge.contexts().get(child).unwrap().premises().iter()
        .find(|p| p.id() == d(6)).unwrap().basis(), PremiseBasis::Proposed);
    let candidate_identity = generated.measurement_identity_current(&bridge, &view).unwrap();
    let full = [
        SupportDispatch { plan: plan.digest(), identity: p_id, side: ContextSide::Baseline, max_work: 10_000 },
        SupportDispatch { plan: plan.digest(), identity: q_id, side: ContextSide::Baseline, max_work: 10_000 },
        SupportDispatch { plan: plan.digest(), identity: q_id, side: ContextSide::Candidate, max_work: 10_000 },
        SupportDispatch { plan: plan.digest(), identity: if matches!(cut, Cut::ForeignDispatch) { b"foreign" } else { p_id },
            side: if matches!(cut, Cut::DuplicateDispatch) { ContextSide::Baseline } else { ContextSide::Candidate },
            max_work: if matches!(cut, Cut::UnknownCandidateP) { 1 } else { 10_000 } },
    ];
    let budget = WorkBudget {
        max_work: if matches!(cut, Cut::Work) { 1 } else { work().max_work },
        max_payload_bytes: if matches!(cut, Cut::Payload) { 1 } else { work().max_payload_bytes },
    };
    let exhausted = if matches!(cut, Cut::Output) {
        Some(view.claim_resources(&[resource(ResourceKind::Output, 8 * 1024 * 1024 - CANDIDATE_OUTPUT)]).unwrap())
    } else { None };
    if matches!(cut, Cut::CancelBefore) { cancel.cancel(); }
    let result = generated.with_configuration_difference_current(
        if matches!(cut, Cut::ForeignBridge) { &foreign } else { &bridge },
        if matches!(cut, Cut::ForeignPlan) { &other_plan } else if matches!(cut, Cut::StaleDomain) { &stale_plan } else { &plan },
        if tied { &[] } else if matches!(cut, Cut::MissingCandidateP) { &full[..3] } else { &full },
        budget, if matches!(cut, Cut::ForeignLease) { &foreign_view } else { &view },
        |mut current| {
            calls.set(calls.get() + 1);
            let report = current.report();
            assert_eq!(report.binding.source, source.id().digest());
            assert_eq!(report.binding.context, child.interpretation_digest());
            assert_eq!(report.binding.candidate, candidate_identity);
            assert_eq!(report.binding.baseline, plan.bindings().baseline);
            assert_eq!(current.source_interpretation(), lc631_runtime::formal_boolean::SourceInterpretation::Unverified);
            assert_eq!(current.hard_outcomes().len(), 1);
            for hard in current.hard_outcomes() {
                assert_eq!(hard.baseline(), CanonicalOutcome::Supported);
                assert_eq!(hard.candidate(), CanonicalOutcome::Supported);
                let (base_positive, _) = hard.baseline_evaluations();
                let (candidate_positive, _) = hard.candidate_evaluations();
                for actual in [base_positive, candidate_positive] {
                    match actual {
                        lc631_runtime::formal_boolean::Evaluation::Computed(report) => {
                            assert_eq!(report.completeness(), lc631_runtime::formal_boolean::DirectionResult::Preserved);
                            assert!(std::ptr::eq(report.request().source, report.request().target));
                        }
                        other => panic!("hard root must have actual computed support: {other:?}"),
                    }
                }
            }
            let mut summary = Summary { report: report.clone(),
                baseline_p: current.probe(p_id).unwrap().baseline(),
                candidate_p: current.probe(p_id).unwrap().candidate(),
                candidate_q: current.probe(q_id).unwrap().candidate(),
                initial_queries: current.query_count(), follow_up: None,
                follow_up_counts: None, fresh_current_rejected: false, candidate_models,
                ambiguous_follow_up: false };
            if matches!(cut, Cut::CancelInside) { cancel.cancel(); }
            if tied {
                let fresh_calls = Cell::new(0);
                let before = (current.query_count(), current.remaining_work());
                assert!(matches!(current.execute_follow_up_current(10_000, &view, |_| {
                    fresh_calls.set(fresh_calls.get() + 1);
                }), Err(Error::AmbiguousFollowUp)));
                assert_eq!(fresh_calls.get(), 0);
                assert_eq!((current.query_count(), current.remaining_work()), before);
                summary.ambiguous_follow_up = true;
            } else if matches!(cut, Cut::FollowUpWork) {
                let fresh_calls = Cell::new(0);
                let before = current.query_count();
                let too_much = current.remaining_work() / 2 + 1;
                assert!(matches!(current.execute_follow_up_current(too_much, &view, |_| {
                    fresh_calls.set(fresh_calls.get() + 1);
                }), Err(Error::WorkBudget)));
                assert_eq!(fresh_calls.get(), 0);
                assert_eq!(current.query_count(), before);
            } else if !matches!(cut, Cut::CancelInside) {
                current.execute_follow_up_current(10_000, &view, |fresh| match fresh {
                    FollowUpResult::NoQuery { query_count } => assert_eq!(query_count, summary.initial_queries),
                    FollowUpResult::Executed(observed) => {
                        assert_eq!(observed.query_count(), 2);
                        let (positive, negative) = observed.evaluations();
                        assert!(matches!(positive, lc631_runtime::formal_boolean::Evaluation::Computed(_)));
                        assert!(matches!(negative, lc631_runtime::formal_boolean::Evaluation::Computed(_)));
                        for actual in [positive, negative] {
                            if let lc631_runtime::formal_boolean::Evaluation::Computed(report) = actual {
                                let request = report.request();
                                assert_eq!(request.binding.source_context, child);
                                assert_eq!(request.binding.target_context, child);
                                assert!(std::ptr::eq(request.source, request.target));
                                assert!(std::ptr::eq(request.source, generated.project_current(&bridge, &view).unwrap()));
                                assert_eq!(request.source_predicate, &BoolExpr::Constant(true));
                                assert_eq!(report.assignments(), 8);
                                assert!(report.ast_visits() > 0);
                            }
                        }
                        if let lc631_runtime::formal_boolean::Evaluation::Computed(report) = positive {
                            let expected = if matches!(cut, Cut::MissingCandidateP | Cut::UnknownCandidateP) { var(1) } else { var(0) };
                            assert_eq!(report.request().target_predicate, &expected);
                        }
                        summary.follow_up = Some(observed.outcome());
                        summary.follow_up_counts = Some((observed.before_count(), observed.after_count()));
                        if matches!(cut, Cut::CancelFresh) { cancel.cancel(); }
                        summary.fresh_current_rejected = observed.current_outcome(&view).is_err();
                    }
                }).unwrap_or_else(|error| {
                    if !matches!(cut, Cut::CancelFresh) { panic!("unexpected follow-up error: {error:?}"); }
                });
            }
            assert_eq!(current.report(), &summary.report);
            summary
        },
    );
    drop(exhausted);
    result
}
