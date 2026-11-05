use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::{formal_boolean as f, formal_measurement::*};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::Digest32;

fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
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
fn budget() -> WorkBudget {
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
fn request<'a>(
    q: &'a SmtStatusQuery,
    source: &'a BoolExpr,
    target: &'a BoolExpr,
    map: &'a [(BoolSymbolId, BoolSymbolId)],
) -> f::Request<'a> {
    let s = scope();
    let target_revision = SourceRevision::of("target");
    let mut r = f::Request {
        source: q,
        target: q,
        source_predicate: source,
        target_predicate: target,
        symbols: map,
        unconstrained: f::UnconstrainedPolicy::ExplicitlyGiven,
        binding: f::Binding {
            source: s.source,
            target: target_revision,
            source_context: s.context,
            target_context: ContextRevision::original(target_revision),
            probe_set: s.probe_set,
            probe_map: d(0),
        },
    };
    r.binding.probe_map = f::probe_map_digest(&r, 10000).unwrap();
    r
}
fn computed<'a>(r: &'a f::Request<'a>) -> f::ComputedReport<'a> {
    match f::evaluate(r, 10000).unwrap() {
        f::Evaluation::Computed(x) => x,
        other => panic!("{other:?}"),
    }
}
fn candidate(q: &SmtStatusQuery, id: u8) -> Candidate<'_> {
    let revision = SourceRevision::of("target");
    Candidate::given(
        d(id),
        scope().comparison,
        q,
        revision,
        ContextRevision::original(revision),
    )
    .unwrap()
}

#[test]
fn tlcx18_formal_k1_three_probe_hand_fraction_and_missing_denominator() {
    let q = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let n = BoolExpr::Not(Box::new(p.clone()));
    let yes = BoolExpr::Constant(true);
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let a = request(&q, &p, &p, &map);
    let b = request(&q, &n, &yes, &map);
    let c = request(&q, &yes, &p, &map);
    let weights = [
        RationalInterval::exact_ratio(1, 1).unwrap(),
        RationalInterval::exact_ratio(2, 1).unwrap(),
        RationalInterval::exact_ratio(3, 1).unwrap(),
    ];
    let probes = [
        FixedProbe {
            identity: b"a",
            weight: weights[0].lower(),
            template: &a,
        },
        FixedProbe {
            identity: b"b",
            weight: weights[1].lower(),
            template: &b,
        },
        FixedProbe {
            identity: b"c",
            weight: weights[2].lower(),
            template: &c,
        },
    ];
    let plan = ProbePlan::checked(scope(), &q, &probes, budget()).unwrap();
    let pass = computed(&a);
    let fail = computed(&b);
    let candidate = candidate(&q, 7);
    let observations = [
        ProbeObservation {
            identity: b"a",
            candidate: d(7),
            scope: scope().comparison,
            value: ObservationValue::Computed(&pass),
        },
        ProbeObservation {
            identity: b"b",
            candidate: d(7),
            scope: scope().comparison,
            value: ObservationValue::Computed(&fail),
        },
    ];
    let loss = plan
        .measure(&candidate, Direction::Soundness, &observations, budget())
        .unwrap();
    assert_eq!(
        loss.bounds(),
        &RationalInterval::from_ratio(1, 3, 5, 6).unwrap()
    );
    assert_eq!(loss.probes().len(), 3);
    assert_eq!(loss.probes()[2].state(), ProbeState::Gap(Gap::Missing));
    assert_eq!(loss.mapping_basis(), f::MappingBasis::Given);
    assert_eq!(
        loss.source_interpretation(),
        f::SourceInterpretation::Unverified
    );
    assert_eq!(
        loss.checker_independence(),
        f::CheckerIndependence::UnassessedSharedIr
    );
    assert!(std::ptr::eq(loss.plan(), &plan));
    assert_eq!(loss.candidate().id(), d(7));
}

#[test]
fn tlcx18_formal_k1_wrong_candidate_scope_query_and_probe_rejected() {
    let q = query();
    let other = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let r = request(&q, &p, &p, &map);
    let w = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [FixedProbe {
        identity: b"a",
        weight: w.lower(),
        template: &r,
    }];
    let plan = ProbePlan::checked(scope(), &q, &probes, budget()).unwrap();
    let report = computed(&r);
    let c = candidate(&q, 7);
    let mut observations = [ProbeObservation {
        identity: b"a",
        candidate: d(8),
        scope: scope().comparison,
        value: ObservationValue::Computed(&report),
    }];
    assert!(matches!(
        plan.measure(&c, Direction::Soundness, &observations, budget()),
        Err(BridgeError::CandidateMismatch)
    ));
    observations[0].candidate = d(7);
    observations[0].scope = d(9);
    assert!(matches!(
        plan.measure(&c, Direction::Soundness, &observations, budget()),
        Err(BridgeError::ScopeMismatch)
    ));
    observations[0].scope = scope().comparison;
    let wrong = candidate(&other, 7);
    assert!(matches!(
        plan.measure(&wrong, Direction::Soundness, &observations, budget()),
        Err(BridgeError::LineageMismatch)
    ));
    observations[0].identity = b"foreign";
    assert!(matches!(
        plan.measure(&c, Direction::Soundness, &observations, budget()),
        Err(BridgeError::ForeignProbe)
    ));
}

#[test]
fn tlcx18_formal_k1_only_defined_missing_observations_have_full_bounds() {
    let q = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let r = request(&q, &p, &p, &map);
    let w = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [FixedProbe {
        identity: b"a",
        weight: w.lower(),
        template: &r,
    }];
    let plan = ProbePlan::checked(scope(), &q, &probes, budget()).unwrap();
    let c = candidate(&q, 7);
    for gap in [
        Gap::Unknown,
        Gap::NoComparable(f::NoComparable::SourceContextInconsistent),
        Gap::Unsupported(f::Failure::UnsupportedInteger),
    ] {
        let obs = [ProbeObservation {
            identity: b"a",
            candidate: d(7),
            scope: scope().comparison,
            value: ObservationValue::Gap(gap),
        }];
        match gap {
            Gap::Unknown => {
                let loss = plan
                    .measure(&c, Direction::Completeness, &obs, budget())
                    .unwrap();
                assert_eq!(
                    loss.bounds(),
                    &RationalInterval::from_ratio(0, 1, 1, 1).unwrap()
                );
                assert_eq!(loss.probes()[0].state(), ProbeState::Gap(gap));
            }
            Gap::NoComparable(_) | Gap::Unsupported(_) => assert!(
                plan.measure(&c, Direction::Completeness, &obs, budget())
                    .is_err(),
                "a nonnumeric gap must not become a numeric loss enclosure"
            ),
            Gap::Missing => unreachable!("this fixture supplies an explicit observation"),
        }
    }
}

#[test]
fn tlcx18_formal_k1_plan_budget_weight_duplicates_and_empty_are_invalid() {
    let q = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let r = request(&q, &p, &p, &map);
    let w = RationalInterval::exact_ratio(0, 1).unwrap();
    let probes = [FixedProbe {
        identity: b"a",
        weight: w.lower(),
        template: &r,
    }];
    assert!(matches!(
        ProbePlan::checked(scope(), &q, &probes, budget()),
        Err(BridgeError::InvalidWeight)
    ));
    assert!(matches!(
        ProbePlan::checked(scope(), &q, &[], budget()),
        Err(BridgeError::EmptyPlan)
    ));
    let w = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [FixedProbe {
        identity: b"a",
        weight: w.lower(),
        template: &r,
    }];
    assert!(matches!(
        ProbePlan::checked(
            scope(),
            &q,
            &probes,
            WorkBudget {
                max_work: 1,
                max_payload_bytes: 1_000_000
            }
        ),
        Err(BridgeError::WorkBudget)
    ));
    assert!(matches!(
        ProbePlan::checked(
            scope(),
            &q,
            &probes,
            WorkBudget {
                max_work: 100_000,
                max_payload_bytes: 1
            }
        ),
        Err(BridgeError::PayloadBudget)
    ));
    let duplicates = [
        FixedProbe {
            identity: b"a",
            weight: w.lower(),
            template: &r,
        },
        FixedProbe {
            identity: b"a",
            weight: w.lower(),
            template: &r,
        },
    ];
    assert!(matches!(
        ProbePlan::checked(scope(), &q, &duplicates, budget()),
        Err(BridgeError::DuplicateProbe)
    ));
}

#[test]
fn tlcx18_formal_k1_same_id_replacement_and_duplicate_observation_rejected() {
    let q = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let clone = p.clone();
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let r = request(&q, &p, &p, &map);
    let changed = request(&q, &clone, &p, &map);
    let report = computed(&changed);
    let w = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [FixedProbe {
        identity: b"a",
        weight: w.lower(),
        template: &r,
    }];
    let plan = ProbePlan::checked(scope(), &q, &probes, budget()).unwrap();
    let c = candidate(&q, 7);
    let obs = [ProbeObservation {
        identity: b"a",
        candidate: d(7),
        scope: scope().comparison,
        value: ObservationValue::Computed(&report),
    }];
    assert!(matches!(
        plan.measure(&c, Direction::Soundness, &obs, budget()),
        Err(BridgeError::LineageMismatch)
    ));
    let obs = [
        ProbeObservation {
            identity: b"a",
            candidate: d(7),
            scope: scope().comparison,
            value: ObservationValue::Gap(Gap::Unknown),
        },
        ProbeObservation {
            identity: b"a",
            candidate: d(7),
            scope: scope().comparison,
            value: ObservationValue::Gap(Gap::Unknown),
        },
    ];
    assert!(matches!(
        plan.measure(&c, Direction::Soundness, &obs, budget()),
        Err(BridgeError::DuplicateProbe)
    ));
}

#[test]
fn tlcx18_formal_k1_candidate_coherence_is_checked_before_all_missing_rows() {
    let q = query();
    let revision = SourceRevision::of("target");
    let foreign = ContextRevision::original(SourceRevision::of("foreign"));
    assert!(matches!(
        Candidate::given(d(7), scope().comparison, &q, revision, foreign),
        Err(BridgeError::LineageMismatch)
    ));
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let r = request(&q, &p, &p, &map);
    let w = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [FixedProbe {
        identity: b"a",
        weight: w.lower(),
        template: &r,
    }];
    let plan = ProbePlan::checked(scope(), &q, &probes, budget()).unwrap();
    let valid = candidate(&q, 7);
    let result = plan
        .measure(&valid, Direction::Soundness, &[], budget())
        .unwrap();
    assert_eq!(
        result.bounds(),
        &RationalInterval::from_ratio(0, 1, 1, 1).unwrap()
    );
}

#[test]
fn tlcx18_formal_k1_foreign_boolean_and_integer_domains_fail_even_with_only_gaps() {
    use lc631_solvers::typed_query::{IntExpr, IntSymbolId};
    let q = query();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let r = request(&q, &p, &p, &map);
    let w = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [FixedProbe {
        identity: b"a",
        weight: w.lower(),
        template: &r,
    }];
    let plan = ProbePlan::checked(scope(), &q, &probes, budget()).unwrap();
    let foreign = SmtStatusQuery::checked(
        vec![BoolSymbolId::new(9)],
        vec![],
        vec![],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let c = candidate(&foreign, 7);
    assert!(matches!(
        plan.measure(&c, Direction::Soundness, &[], budget()),
        Err(BridgeError::DomainMismatch)
    ));
    let declared = SmtStatusQuery::checked(
        vec![BoolSymbolId::new(0)],
        vec![IntSymbolId::new(1)],
        vec![],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let revision = SourceRevision::of("target");
    assert!(matches!(
        Candidate::given(
            d(7),
            scope().comparison,
            &declared,
            revision,
            ContextRevision::original(revision)
        ),
        Err(BridgeError::Formal(f::Failure::UnsupportedInteger))
    ));
    let oversized = SmtStatusQuery::checked(
        (0..17).map(BoolSymbolId::new).collect(),
        vec![],
        vec![],
        SmtEmitLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        Candidate::given(
            d(7),
            scope().comparison,
            &oversized,
            revision,
            ContextRevision::original(revision)
        ),
        Err(BridgeError::Formal(f::Failure::AtomLimit))
    ));
    let hidden = SmtStatusQuery::checked(
        vec![BoolSymbolId::new(0)],
        vec![],
        vec![BoolExpr::Equal(IntExpr::Constant(1), IntExpr::Constant(1))],
        SmtEmitLimits::default(),
    )
    .unwrap();
    {
        let c = candidate(&hidden, 7);
        let gaps = [ProbeObservation {
            identity: b"a",
            candidate: d(7),
            scope: scope().comparison,
            value: ObservationValue::Gap(Gap::Unknown),
        }];
        assert!(matches!(
            plan.measure(&c, Direction::Soundness, &gaps, budget()),
            Err(BridgeError::Formal(f::Failure::UnsupportedInteger))
        ));
    }
}

#[test]
fn tlcx18_formal_k1_plan_rejects_different_target_signatures() {
    let q = query();
    let foreign = SmtStatusQuery::checked(
        vec![BoolSymbolId::new(9)],
        vec![],
        vec![],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let p = BoolExpr::Variable(BoolSymbolId::new(0));
    let p9 = BoolExpr::Variable(BoolSymbolId::new(9));
    let map = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let map9 = [(BoolSymbolId::new(0), BoolSymbolId::new(9))];
    let r = request(&q, &p, &p, &map);
    let mut other = f::Request {
        source: &q,
        target: &foreign,
        source_predicate: &p,
        target_predicate: &p9,
        symbols: &map9,
        unconstrained: f::UnconstrainedPolicy::ExplicitlyGiven,
        binding: r.binding,
    };
    other.binding.probe_map = f::probe_map_digest(&other, 10000).unwrap();
    let w = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [
        FixedProbe {
            identity: b"a",
            weight: w.lower(),
            template: &r,
        },
        FixedProbe {
            identity: b"b",
            weight: w.lower(),
            template: &other,
        },
    ];
    assert!(matches!(
        ProbePlan::checked(scope(), &q, &probes, budget()),
        Err(BridgeError::DomainMismatch)
    ));
}
