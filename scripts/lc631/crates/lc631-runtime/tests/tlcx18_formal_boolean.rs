use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::formal_boolean::*;
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, IntExpr, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::Digest32;

fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn v(n: u32) -> BoolExpr {
    BoolExpr::Variable(BoolSymbolId::new(n))
}
fn theory(symbols: &[u32], assertions: Vec<BoolExpr>) -> SmtStatusQuery {
    SmtStatusQuery::checked(
        symbols.iter().map(|&n| BoolSymbolId::new(n)).collect(),
        vec![],
        assertions,
        SmtEmitLimits::default(),
    )
    .unwrap()
}
fn binding() -> Binding {
    let source = SourceRevision::of("source formula");
    let target = SourceRevision::of("target formula");
    Binding {
        source,
        target,
        source_context: ContextRevision::original(source),
        target_context: ContextRevision::original(target),
        probe_set: d(3),
        probe_map: d(0),
    }
}
fn seal(request: &mut Request<'_>) {
    request.binding.probe_map = probe_map_digest(request, 100_000).unwrap();
}
fn completed<'a>(request: &'a Request<'a>) -> ComputedReport<'a> {
    match evaluate(request, 1_000_000).unwrap() {
        Evaluation::Computed(report) => report,
        other => panic!("expected computed finite result: {other:?}"),
    }
}

#[test]
fn tlcx18_formal_directional_countermodels_and_lineage() {
    let s = theory(&[0], vec![BoolExpr::Constant(true)]);
    let t = theory(&[9], vec![BoolExpr::Constant(true)]);
    let source = v(0);
    let target = BoolExpr::Constant(true);
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(9))];
    let mut request = Request {
        source: &s,
        target: &t,
        source_predicate: &source,
        target_predicate: &target,
        symbols: &map,
        unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
        binding: binding(),
    };
    seal(&mut request);
    let report = completed(&request);
    assert_eq!(report.soundness(), DirectionResult::Countermodel(0));
    assert_eq!(report.completeness(), DirectionResult::Preserved);
    assert_eq!(report.assignments(), 2);
    assert!(report.ast_visits() > 0);
    assert_eq!(report.binding(), request.binding);
    assert!(std::ptr::eq(report.request(), &request));
    assert_ne!(report.source_theory_digest(), report.target_theory_digest());
    assert_eq!(
        report.validation(),
        FormalValidation::GivenMappingSourceInterpretationUnverified
    );
    assert_eq!(report.mapping_basis(), MappingBasis::Given);
    assert_eq!(
        report.source_interpretation(),
        SourceInterpretation::Unverified
    );
    assert_eq!(
        report.checker_independence(),
        CheckerIndependence::UnassessedSharedIr
    );
}

// Independent oracle: the u8 truth bits are the specification, not an AST evaluator.
fn formula(bits: u8, ids: [u32; 2]) -> BoolExpr {
    let mut terms = Vec::new();
    for row in 0..4 {
        if bits & (1 << row) != 0 {
            terms.push(BoolExpr::And(
                ids.iter()
                    .enumerate()
                    .map(|(bit, &id)| {
                        if row & (1 << bit) != 0 {
                            v(id)
                        } else {
                            BoolExpr::Not(Box::new(v(id)))
                        }
                    })
                    .collect(),
            ));
        }
    }
    if terms.is_empty() {
        BoolExpr::Constant(false)
    } else {
        BoolExpr::Or(terms)
    }
}

#[test]
fn tlcx18_formal_all_small_boolean_functions_match_independent_truth_bits() {
    let s = theory(&[0, 1], vec![BoolExpr::Constant(true)]);
    let t = theory(&[40, 20], vec![BoolExpr::Constant(true)]);
    let map = [
        (BoolSymbolId::new(0), BoolSymbolId::new(40)),
        (BoolSymbolId::new(1), BoolSymbolId::new(20)),
    ];
    for left in 0u8..16 {
        for right in 0u8..16 {
            let source = formula(left, [0, 1]);
            let target = formula(right, [40, 20]);
            let mut request = Request {
                source: &s,
                target: &t,
                source_predicate: &source,
                target_predicate: &target,
                symbols: &map,
                unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
                binding: binding(),
            };
            seal(&mut request);
            let report = completed(&request);
            let expected = |counter: u8| match (0..4).find(|row| counter & (1 << row) != 0) {
                Some(row) => DirectionResult::Countermodel(row),
                None => DirectionResult::Preserved,
            };
            assert_eq!(report.soundness(), expected(right & !left & 15));
            assert_eq!(report.completeness(), expected(left & !right & 15));
        }
    }
}

#[test]
fn tlcx18_formal_explicit_relabel_and_mapping_cut_change_result() {
    let s = theory(&[0, 1], vec![BoolExpr::Constant(true)]);
    let t = theory(&[99, 7], vec![BoolExpr::Constant(true)]);
    let a = v(0);
    let b = v(99);
    let map = [
        (BoolSymbolId::new(0), BoolSymbolId::new(99)),
        (BoolSymbolId::new(1), BoolSymbolId::new(7)),
    ];
    let mut r = Request {
        source: &s,
        target: &t,
        source_predicate: &a,
        target_predicate: &b,
        symbols: &map,
        unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
        binding: binding(),
    };
    seal(&mut r);
    assert_eq!(completed(&r).soundness(), DirectionResult::Preserved);
    let swapped = [
        (BoolSymbolId::new(0), BoolSymbolId::new(7)),
        (BoolSymbolId::new(1), BoolSymbolId::new(99)),
    ];
    r.symbols = &swapped;
    assert!(matches!(
        evaluate(&r, 100_000),
        Err(Failure::ProbeMapMismatch)
    ));
    seal(&mut r);
    assert!(matches!(
        completed(&r).soundness(),
        DirectionResult::Countermodel(_)
    ));
    r.symbols = &map[..1];
    assert!(matches!(evaluate(&r, 100_000), Err(Failure::IncompleteMap)));
}

#[test]
fn tlcx18_formal_inconsistent_context_and_explicit_unconstrained_scope_differ() {
    let s = theory(&[0], vec![v(0), BoolExpr::Not(Box::new(v(0)))]);
    let t = theory(&[0], vec![BoolExpr::Constant(true)]);
    let p = v(0);
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let mut r = Request {
        source: &s,
        target: &t,
        source_predicate: &p,
        target_predicate: &p,
        symbols: &map,
        unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
        binding: binding(),
    };
    seal(&mut r);
    assert!(matches!(
        evaluate(&r, 100_000).unwrap(),
        Evaluation::NoComparable {
            reason: NoComparable::SourceContextInconsistent,
            ..
        }
    ));
    let empty = theory(&[0], vec![]);
    r.source = &empty;
    seal(&mut r);
    assert_eq!(completed(&r).soundness(), DirectionResult::Preserved);
    r.unconstrained = UnconstrainedPolicy::MissingScope;
    assert!(matches!(
        evaluate(&r, 100_000).unwrap(),
        Evaluation::NoComparable {
            reason: NoComparable::MissingTheoryScope,
            ..
        }
    ));
}

#[test]
fn tlcx18_formal_integer_fragment_and_hidden_integer_probe_are_unsupported() {
    let q = theory(&[], vec![BoolExpr::Constant(true)]);
    let p = BoolExpr::And(vec![
        BoolExpr::Constant(false),
        BoolExpr::Equal(IntExpr::Constant(1), IntExpr::Constant(1)),
    ]);
    let c = BoolExpr::Constant(true);
    let r = Request {
        source: &q,
        target: &q,
        source_predicate: &p,
        target_predicate: &c,
        symbols: &[],
        unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
        binding: binding(),
    };
    assert_eq!(probe_map_digest(&r, 1000), Err(Failure::UnsupportedInteger));
}

#[test]
fn tlcx18_formal_budget_cut_is_unknown_not_zero() {
    let q = theory(&[0], vec![BoolExpr::Constant(true)]);
    let p = v(0);
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let mut r = Request {
        source: &q,
        target: &q,
        source_predicate: &p,
        target_predicate: &p,
        symbols: &map,
        unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
        binding: binding(),
    };
    seal(&mut r);
    assert!(matches!(
        evaluate(&r, 1).unwrap(),
        Evaluation::Unknown { ast_visits: 1 }
    ));
    assert!(matches!(evaluate(&r, 0), Err(Failure::InvalidBudget)));
    assert!(matches!(
        evaluate(&r, 1_000_001),
        Err(Failure::InvalidBudget)
    ));
}

#[test]
fn tlcx18_formal_foreign_binding_duplicate_map_and_undeclared_probe_rejected() {
    let q = theory(&[0, 1], vec![BoolExpr::Constant(true)]);
    let p = v(0);
    let duplicates = [
        (BoolSymbolId::new(0), BoolSymbolId::new(0)),
        (BoolSymbolId::new(0), BoolSymbolId::new(1)),
    ];
    let mut r = Request {
        source: &q,
        target: &q,
        source_predicate: &p,
        target_predicate: &p,
        symbols: &duplicates,
        unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
        binding: binding(),
    };
    assert_eq!(probe_map_digest(&r, 1000), Err(Failure::DuplicateMap));
    let map = [
        (BoolSymbolId::new(0), BoolSymbolId::new(0)),
        (BoolSymbolId::new(1), BoolSymbolId::new(1)),
    ];
    r.symbols = &map;
    let bad = v(9);
    r.source_predicate = &bad;
    assert_eq!(probe_map_digest(&r, 1000), Err(Failure::UndeclaredSymbol));
    r.source_predicate = &p;
    seal(&mut r);
    r.binding.source = SourceRevision::of("foreign");
    assert!(matches!(
        evaluate(&r, 1000),
        Err(Failure::SourceBindingMismatch)
    ));
}

#[test]
fn tlcx18_formal_context_changes_are_model_set_changes_not_permissions() {
    let s = theory(&[0], vec![v(0)]);
    let t = theory(&[0], vec![]);
    let p = BoolExpr::Constant(true);
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let mut r = Request {
        source: &s,
        target: &t,
        source_predicate: &p,
        target_predicate: &p,
        symbols: &map,
        unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
        binding: binding(),
    };
    seal(&mut r);
    assert_eq!(completed(&r).soundness(), DirectionResult::Countermodel(0));
    assert_eq!(completed(&r).completeness(), DirectionResult::Preserved);
    let inconsistent = theory(&[0], vec![BoolExpr::Constant(false)]);
    r.target = &inconsistent;
    seal(&mut r);
    assert!(matches!(
        evaluate(&r, 100_000).unwrap(),
        Evaluation::NoComparable {
            reason: NoComparable::TargetContextInconsistent,
            ..
        }
    ));
}

#[test]
fn tlcx18_formal_complexity_admission_rejects_atom_and_depth_excess() {
    let ids: Vec<u32> = (0..17).collect();
    let q = theory(&ids, vec![BoolExpr::Constant(true)]);
    let p = BoolExpr::Constant(true);
    let map: Vec<_> = ids
        .iter()
        .map(|&x| (BoolSymbolId::new(x), BoolSymbolId::new(x)))
        .collect();
    let r = Request {
        source: &q,
        target: &q,
        source_predicate: &p,
        target_predicate: &p,
        symbols: &map,
        unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
        binding: binding(),
    };
    assert_eq!(probe_map_digest(&r, 1000), Err(Failure::AtomLimit));
    let q = theory(&[], vec![BoolExpr::Constant(true)]);
    let mut deep = BoolExpr::Constant(true);
    for _ in 0..129 {
        deep = BoolExpr::Not(Box::new(deep));
    }
    let r = Request {
        source: &q,
        target: &q,
        source_predicate: &deep,
        target_predicate: &p,
        symbols: &[],
        unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
        binding: binding(),
    };
    assert_eq!(probe_map_digest(&r, 1000), Err(Failure::DepthLimit));
}

#[test]
fn tlcx18_formal_declared_integer_and_empty_connective_are_not_boolean_truth() {
    use lc631_solvers::typed_query::IntSymbolId;
    let q = SmtStatusQuery::checked(
        vec![],
        vec![IntSymbolId::new(0)],
        vec![],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let p = BoolExpr::Constant(true);
    let r = Request {
        source: &q,
        target: &q,
        source_predicate: &p,
        target_predicate: &p,
        symbols: &[],
        unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
        binding: binding(),
    };
    assert_eq!(probe_map_digest(&r, 1000), Err(Failure::UnsupportedInteger));
    let q = theory(&[], vec![]);
    let empty = BoolExpr::And(vec![]);
    let r = Request {
        source: &q,
        target: &q,
        source_predicate: &empty,
        target_predicate: &p,
        symbols: &[],
        unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
        binding: binding(),
    };
    assert_eq!(probe_map_digest(&r, 1000), Err(Failure::EmptyConnective));
}
