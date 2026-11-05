//! Regression: equal missing-observation content is not a shared observed node.
use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::context::{ContextRevision, SourceRevision};
use lc631_runtime::{
    facet_ledger as ledger, facet_measurement_bridge as bridge, formal_boolean as boolean,
    formal_measurement as measurement,
};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::Digest32;

fn digest(byte: u8) -> Digest32 {
    Digest32::from_bytes([byte; 32])
}

fn with_missing_pair(check: impl FnOnce(&bridge::MeasuredFacetBatch<'_>)) {
    let source = SourceRevision::of("declared source");
    let target = SourceRevision::of("declared target");
    let source_context = ContextRevision::original(source);
    let target_context = ContextRevision::original(target);
    let query = SmtStatusQuery::checked(
        vec![BoolSymbolId::new(0)],
        vec![],
        vec![],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let predicate = BoolExpr::Variable(BoolSymbolId::new(0));
    let symbols = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let mut request = boolean::Request {
        source: &query,
        target: &query,
        source_predicate: &predicate,
        target_predicate: &predicate,
        symbols: &symbols,
        unconstrained: boolean::UnconstrainedPolicy::ExplicitlyGiven,
        binding: boolean::Binding {
            source,
            target,
            source_context,
            target_context,
            probe_set: digest(3),
            probe_map: digest(0),
        },
    };
    request.binding.probe_map = boolean::probe_map_digest(&request, 10_000).unwrap();
    let weight = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [measurement::FixedProbe {
        identity: b"protected-p",
        weight: weight.lower(),
        template: &request,
    }];
    let scope = measurement::PlanScope {
        comparison: digest(1),
        definition: digest(2),
        probe_set: digest(3),
        source,
        context: source_context,
    };
    let budget = measurement::WorkBudget {
        max_work: 100_000,
        max_payload_bytes: 1_000_000,
    };
    let plan = measurement::ProbePlan::checked(scope, &query, &probes, budget).unwrap();
    let candidate =
        measurement::Candidate::given(digest(7), digest(1), &query, target, target_context)
            .unwrap();
    // Separate invocations both lack the observation. Equal content does not
    // establish one shared valid observation node or correlate their unknowns.
    let before = plan
        .measure(&candidate, measurement::Direction::Soundness, &[], budget)
        .unwrap();
    let after = plan
        .measure(&candidate, measurement::Direction::Soundness, &[], budget)
        .unwrap();
    assert!(!std::ptr::eq(&before, &after));
    let declared = bridge::DeclaredFacetScope {
        baseline: ledger::Revision([7; 32]),
        root_goal: ledger::Revision([8; 32]),
        policy: ledger::Revision([9; 32]),
    };
    let obligations = [bridge::ProbeObligation {
        identity: b"protected-p",
        root: ledger::ObligationId([10; 32]),
        property: ledger::PropertyId([11; 32]),
    }];
    let batch = bridge::MeasuredFacetBatch::from_loss(&after, declared, &obligations)
        .unwrap()
        .with_paired_preservation_gain(&before)
        .unwrap();
    check(&batch);
}

#[test]
fn tlcx17_independent_missing_pairs_do_not_become_exact_zero() {
    with_missing_pair(|batch| {
        assert_eq!(
            batch.gain_bounds(),
            Some(&RationalInterval::from_ratio(-1, 1, 1, 1).unwrap())
        );
    });
}

#[test]
fn tlcx17_missing_pairs_are_uncertainty_not_observed_or_predicted_gain() {
    with_missing_pair(|batch| {
        assert!(!batch
            .entries()
            .iter()
            .any(|e| matches!(e.facet, ledger::Facet::Gain(_))));
        assert!(batch.entries().iter().any(|e| matches!(
            e.facet,
            ledger::Facet::Uncertainty(ledger::Gap {
                reason: ledger::GapReason::Unobserved,
                ..
            })
        )));
    });
}

#[test]
fn tlcx17_shared_valid_observation_keeps_self_difference_zero() {
    let source = SourceRevision::of("declared source");
    let target = SourceRevision::of("declared target");
    let source_context = ContextRevision::original(source);
    let target_context = ContextRevision::original(target);
    let query = SmtStatusQuery::checked(
        vec![BoolSymbolId::new(0)],
        vec![],
        vec![],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let predicate = BoolExpr::Variable(BoolSymbolId::new(0));
    let symbols = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let mut request = boolean::Request {
        source: &query,
        target: &query,
        source_predicate: &predicate,
        target_predicate: &predicate,
        symbols: &symbols,
        unconstrained: boolean::UnconstrainedPolicy::ExplicitlyGiven,
        binding: boolean::Binding {
            source,
            target,
            source_context,
            target_context,
            probe_set: digest(3),
            probe_map: digest(0),
        },
    };
    request.binding.probe_map = boolean::probe_map_digest(&request, 10_000).unwrap();
    let actual = match boolean::evaluate(&request, 10_000).unwrap() {
        boolean::Evaluation::Computed(report) => report,
        _ => panic!("expected actual finite result"),
    };
    let weight = RationalInterval::exact_ratio(1, 1).unwrap();
    let probes = [measurement::FixedProbe {
        identity: b"protected-p",
        weight: weight.lower(),
        template: &request,
    }];
    let scope = measurement::PlanScope {
        comparison: digest(1),
        definition: digest(2),
        probe_set: digest(3),
        source,
        context: source_context,
    };
    let budget = measurement::WorkBudget {
        max_work: 100_000,
        max_payload_bytes: 1_000_000,
    };
    let plan = measurement::ProbePlan::checked(scope, &query, &probes, budget).unwrap();
    let candidate =
        measurement::Candidate::given(digest(7), digest(1), &query, target, target_context)
            .unwrap();
    let observation = [measurement::ProbeObservation {
        identity: b"protected-p",
        candidate: digest(7),
        scope: digest(1),
        value: measurement::ObservationValue::Computed(&actual),
    }];
    let before = plan
        .measure(
            &candidate,
            measurement::Direction::Soundness,
            &observation,
            budget,
        )
        .unwrap();
    let after = plan
        .measure(
            &candidate,
            measurement::Direction::Soundness,
            &observation,
            budget,
        )
        .unwrap();
    let declared = bridge::DeclaredFacetScope {
        baseline: ledger::Revision([7; 32]),
        root_goal: ledger::Revision([8; 32]),
        policy: ledger::Revision([9; 32]),
    };
    let obligations = [bridge::ProbeObligation {
        identity: b"protected-p",
        root: ledger::ObligationId([10; 32]),
        property: ledger::PropertyId([11; 32]),
    }];
    let batch = bridge::MeasuredFacetBatch::from_loss(&after, declared, &obligations)
        .unwrap()
        .with_paired_preservation_gain(&before)
        .unwrap();
    assert_eq!(
        batch.gain_bounds(),
        Some(&RationalInterval::exact_ratio(0, 1).unwrap())
    );
    assert!(batch.entries().iter().any(|entry| matches!(
        entry.facet,
        ledger::Facet::Gain(ledger::Gain {
            kind: ledger::GainKind::ObservedReported,
            ..
        })
    )));
}

#[derive(Clone, Copy)]
enum Case {
    BaselineMissing,
    CandidateMissing,
    BothMissing,
    DistinctComputed,
}

fn check_two_probe_case(case: Case) {
    let source = SourceRevision::of("declared source");
    let target = SourceRevision::of("declared target");
    let source_context = ContextRevision::original(source);
    let target_context = ContextRevision::original(target);
    let query = SmtStatusQuery::checked(
        vec![BoolSymbolId::new(0)],
        vec![],
        vec![],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let predicate = BoolExpr::Variable(BoolSymbolId::new(0));
    let symbols = [(BoolSymbolId::new(0), BoolSymbolId::new(0))];
    let mut request = boolean::Request {
        source: &query,
        target: &query,
        source_predicate: &predicate,
        target_predicate: &predicate,
        symbols: &symbols,
        unconstrained: boolean::UnconstrainedPolicy::ExplicitlyGiven,
        binding: boolean::Binding {
            source,
            target,
            source_context,
            target_context,
            probe_set: digest(3),
            probe_map: digest(0),
        },
    };
    request.binding.probe_map = boolean::probe_map_digest(&request, 10_000).unwrap();
    let first = match boolean::evaluate(&request, 10_000).unwrap() {
        boolean::Evaluation::Computed(report) => report,
        _ => panic!("expected finite result"),
    };
    let second = match boolean::evaluate(&request, 10_000).unwrap() {
        boolean::Evaluation::Computed(report) => report,
        _ => panic!("expected independent finite result"),
    };
    assert!(!std::ptr::eq(&first, &second));
    let one = RationalInterval::exact_ratio(1, 1).unwrap();
    let three = RationalInterval::exact_ratio(3, 1).unwrap();
    let probes = [
        measurement::FixedProbe {
            identity: b"p",
            weight: one.lower(),
            template: &request,
        },
        measurement::FixedProbe {
            identity: b"q",
            weight: three.lower(),
            template: &request,
        },
    ];
    let scope = measurement::PlanScope {
        comparison: digest(1),
        definition: digest(2),
        probe_set: digest(3),
        source,
        context: source_context,
    };
    let budget = measurement::WorkBudget {
        max_work: 100_000,
        max_payload_bytes: 1_000_000,
    };
    let plan = measurement::ProbePlan::checked(scope, &query, &probes, budget).unwrap();
    let candidate =
        measurement::Candidate::given(digest(7), digest(1), &query, target, target_context)
            .unwrap();
    let baseline_q = measurement::ProbeObservation {
        identity: b"q",
        candidate: digest(7),
        scope: digest(1),
        value: measurement::ObservationValue::Computed(&first),
    };
    let candidate_q = measurement::ProbeObservation {
        identity: b"q",
        candidate: digest(7),
        scope: digest(1),
        value: measurement::ObservationValue::Computed(&first),
    };
    let baseline_p = measurement::ProbeObservation {
        identity: b"p",
        candidate: digest(7),
        scope: digest(1),
        value: measurement::ObservationValue::Computed(&first),
    };
    let candidate_p = measurement::ProbeObservation {
        identity: b"p",
        candidate: digest(7),
        scope: digest(1),
        value: measurement::ObservationValue::Computed(&second),
    };
    let mut before_obs = vec![baseline_q];
    let mut after_obs = vec![candidate_q];
    if matches!(case, Case::CandidateMissing | Case::DistinctComputed) {
        before_obs.push(baseline_p);
    }
    if matches!(case, Case::BaselineMissing | Case::DistinctComputed) {
        after_obs.push(candidate_p);
    }
    let before = plan
        .measure(
            &candidate,
            measurement::Direction::Soundness,
            &before_obs,
            budget,
        )
        .unwrap();
    let after = plan
        .measure(
            &candidate,
            measurement::Direction::Soundness,
            &after_obs,
            budget,
        )
        .unwrap();
    let declared = bridge::DeclaredFacetScope {
        baseline: ledger::Revision([7; 32]),
        root_goal: ledger::Revision([8; 32]),
        policy: ledger::Revision([9; 32]),
    };
    let obligations = [
        bridge::ProbeObligation {
            identity: b"p",
            root: ledger::ObligationId([10; 32]),
            property: ledger::PropertyId([11; 32]),
        },
        bridge::ProbeObligation {
            identity: b"q",
            root: ledger::ObligationId([12; 32]),
            property: ledger::PropertyId([13; 32]),
        },
    ];
    let batch = bridge::MeasuredFacetBatch::from_loss(&after, declared, &obligations)
        .unwrap()
        .with_paired_preservation_gain(&before)
        .unwrap();
    let expected = match case {
        Case::BaselineMissing => RationalInterval::from_ratio(0, 1, 1, 4).unwrap(),
        Case::CandidateMissing => RationalInterval::from_ratio(-1, 4, 0, 1).unwrap(),
        Case::BothMissing => RationalInterval::from_ratio(-1, 4, 1, 4).unwrap(),
        Case::DistinctComputed => RationalInterval::exact_ratio(0, 1).unwrap(),
    };
    assert_eq!(batch.gain_bounds(), Some(&expected));
    let gaps = batch
        .entries()
        .iter()
        .filter(|e| {
            matches!(
                e.facet,
                ledger::Facet::Uncertainty(ledger::Gap {
                    reason: ledger::GapReason::Unobserved,
                    ..
                })
            )
        })
        .count();
    let gains = batch
        .entries()
        .iter()
        .filter(|e| matches!(e.facet, ledger::Facet::Gain(_)))
        .count();
    assert_eq!(
        gaps,
        match case {
            Case::BothMissing => 2,
            Case::DistinctComputed => 0,
            _ => 1,
        }
    );
    assert_eq!(gains, usize::from(matches!(case, Case::DistinctComputed)));
    let expected_entries = batch.entries().len();
    let mut store = bridge::FacetMeasurementLedger::new(2, 16).unwrap();
    let id = store.admit(batch).unwrap();
    assert_eq!(store.records().len(), 1);
    assert_eq!(store.ledger().for_transform(id).count(), expected_entries);
    assert_eq!(
        store
            .ledger()
            .unique_gains(store.ledger().transforms()[0].scope)
            .count(),
        gains
    );
}

#[test]
fn tlcx17_baseline_gap_keeps_uncertainty_through_ledger() {
    check_two_probe_case(Case::BaselineMissing);
}
#[test]
fn tlcx17_candidate_gap_keeps_uncertainty_through_ledger() {
    check_two_probe_case(Case::CandidateMissing);
}
#[test]
fn tlcx17_two_gaps_keep_fixed_denominator_through_ledger() {
    check_two_probe_case(Case::BothMissing);
}
#[test]
fn tlcx17_distinct_computed_reports_keep_actual_gain_through_ledger() {
    check_two_probe_case(Case::DistinctComputed);
}
