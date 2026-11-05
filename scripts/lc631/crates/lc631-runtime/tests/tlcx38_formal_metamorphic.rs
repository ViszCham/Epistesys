//! Hand-written Boolean laws and explicit map transport, not generated solver gold.
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::formal_boolean::*;
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::Digest32;

fn var(id: u32) -> BoolExpr {
    BoolExpr::Variable(BoolSymbolId::new(id))
}
fn not(expr: BoolExpr) -> BoolExpr {
    BoolExpr::Not(Box::new(expr))
}
fn theory(ids: &[u32], assertions: Vec<BoolExpr>) -> SmtStatusQuery {
    SmtStatusQuery::checked(
        ids.iter().map(|&id| BoolSymbolId::new(id)).collect(),
        vec![],
        assertions,
        SmtEmitLimits::default(),
    )
    .unwrap()
}
fn binding() -> Binding {
    let revision = SourceRevision::of("fixed finite Boolean laws");
    Binding {
        source: revision,
        target: revision,
        source_context: ContextRevision::original(revision),
        target_context: ContextRevision::original(revision),
        probe_set: Digest32::from_bytes([1; 32]),
        probe_map: Digest32::from_bytes([0; 32]),
    }
}
fn checked_request<'a>(
    source: &'a SmtStatusQuery,
    target: &'a SmtStatusQuery,
    left: &'a BoolExpr,
    right: &'a BoolExpr,
    map: &'a [(BoolSymbolId, BoolSymbolId)],
) -> Request<'a> {
    let mut r = Request {
        source,
        target,
        source_predicate: left,
        target_predicate: right,
        symbols: map,
        unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
        binding: binding(),
    };
    r.binding.probe_map = probe_map_digest(&r, 100_000).unwrap();
    r
}
fn result<'a>(r: &'a Request<'a>) -> ComputedReport<'a> {
    match evaluate(r, 1_000_000).unwrap() {
        Evaluation::Computed(report) => report,
        other => panic!("expected actual finite computation, got {other:?}"),
    }
}

#[test]
fn tlcx38_hand_boolean_laws_survive_reordering_and_negation_rewrite() {
    let source = theory(&[0, 1], vec![]);
    let target = theory(&[9, 4], vec![]);
    let map = [
        (BoolSymbolId::new(0), BoolSymbolId::new(9)),
        (BoolSymbolId::new(1), BoolSymbolId::new(4)),
    ];
    let laws = [
        (
            BoolExpr::And(vec![var(0), var(1)]),
            BoolExpr::And(vec![var(4), var(9)]),
        ),
        (
            BoolExpr::Or(vec![var(0), var(1)]),
            BoolExpr::Or(vec![var(4), var(9)]),
        ),
        (
            not(BoolExpr::And(vec![var(0), var(1)])),
            BoolExpr::Or(vec![not(var(9)), not(var(4))]),
        ),
        (not(not(var(0))), var(9)),
    ];
    for (left, right) in &laws {
        let request = checked_request(&source, &target, left, right, &map);
        let report = result(&request);
        assert_eq!(
            (report.soundness(), report.completeness()),
            (DirectionResult::Preserved, DirectionResult::Preserved)
        );
        assert_eq!(report.assignments(), 4);
        assert_eq!(
            report.checker_independence(),
            CheckerIndependence::UnassessedSharedIr
        );
    }
}

#[test]
fn tlcx38_countermodel_bits_transport_under_explicit_mapping_permutation() {
    let s = theory(&[0, 1], vec![]);
    let t = theory(&[9, 4], vec![]);
    let left = var(0);
    let right = var(4);
    let forward = [
        (BoolSymbolId::new(0), BoolSymbolId::new(9)),
        (BoolSymbolId::new(1), BoolSymbolId::new(4)),
    ];
    let reverse = [forward[1], forward[0]];
    let a = checked_request(&s, &t, &left, &right, &forward);
    let b = checked_request(&s, &t, &left, &right, &reverse);
    // Truth-bit oracle: target-only model is (source-p=false, source-q=true).
    assert_eq!(result(&a).soundness(), DirectionResult::Countermodel(2));
    assert_eq!(result(&a).completeness(), DirectionResult::Countermodel(1));
    assert_eq!(result(&b).soundness(), DirectionResult::Countermodel(1));
    assert_eq!(result(&b).completeness(), DirectionResult::Countermodel(2));
    assert_ne!(a.binding.probe_map, b.binding.probe_map);
}

#[test]
fn tlcx38_map_cut_and_stale_map_digest_cannot_reuse_old_success() {
    let s = theory(&[0, 1], vec![]);
    let t = theory(&[9, 4], vec![]);
    let left = var(0);
    let right = var(9);
    let map = [
        (BoolSymbolId::new(0), BoolSymbolId::new(9)),
        (BoolSymbolId::new(1), BoolSymbolId::new(4)),
    ];
    let mut r = checked_request(&s, &t, &left, &right, &map);
    assert_eq!(result(&r).soundness(), DirectionResult::Preserved);
    let changed = [map[1], map[0]];
    r.symbols = &changed;
    assert!(matches!(
        evaluate(&r, 100_000),
        Err(Failure::ProbeMapMismatch)
    ));
    r.symbols = &map[..1];
    assert!(matches!(evaluate(&r, 100_000), Err(Failure::IncompleteMap)));
    r.symbols = &map;
    assert_eq!(result(&r).completeness(), DirectionResult::Preserved);
}

#[test]
fn tlcx38_context_strengthening_changes_only_the_expected_preservation_direction() {
    let s = theory(&[0], vec![]);
    let t = theory(&[9], vec![var(9)]);
    let yes = BoolExpr::Constant(true);
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(9))];
    let mut r = checked_request(&s, &t, &yes, &yes, &map);
    r.binding.target_context = r.binding.source_context.reconstructed([42; 32]);
    let report = result(&r);
    assert_eq!(report.soundness(), DirectionResult::Preserved);
    assert_eq!(report.completeness(), DirectionResult::Countermodel(0));
    assert_eq!(report.binding().target_context, r.binding.target_context);
}

#[test]
fn tlcx38_budget_cut_and_inconsistent_context_never_become_vacuous_success() {
    let s = theory(&[0], vec![]);
    let impossible = theory(&[0], vec![var(0), not(var(0))]);
    let p = var(0);
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let mut r = checked_request(&s, &s, &p, &p, &map);
    assert!(matches!(
        evaluate(&r, 1).unwrap(),
        Evaluation::Unknown { .. }
    ));
    r.target = &impossible;
    assert!(matches!(
        evaluate(&r, 100_000).unwrap(),
        Evaluation::NoComparable {
            reason: NoComparable::TargetContextInconsistent,
            ..
        }
    ));
    r.target = &s;
    assert_eq!(result(&r).soundness(), DirectionResult::Preserved);
}
