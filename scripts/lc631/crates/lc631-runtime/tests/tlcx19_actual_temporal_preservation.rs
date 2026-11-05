use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_runtime::finite_results::FiniteResultStore;
use lc631_runtime::resource::*;
use lc631_runtime::temporal_preservation::*;
use lc631_solvers::temporal::*;
use lc631_substrate::{measurement_numeric::RationalInterval, Digest32};
use std::time::Duration;

fn id(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn scope() -> ContextScope {
    ContextScope::declared(id(1), id(2), id(3), id(4))
}
fn clock() -> ClockSpec {
    ClockSpec::exact(id(5), TimeScale::Monotonic, id(6), id(7))
}
fn query(source: &str) -> TemporalQuery {
    TemporalQuery::checked(
        ContextRevision::original(SourceRevision::of(source)),
        vec![TemporalNode::Atom(0), TemporalNode::Always(0)],
        1,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap()
}
fn trace(times: [i64; 4], truth: Truth) -> FiniteTrace {
    FiniteTrace::checked(
        times
            .into_iter()
            .map(|t| {
                TraceStep::new(
                    Instant::absolute(clock(), t, TimeUnit::Second).unwrap(),
                    vec![truth],
                )
            })
            .collect(),
        TraceClosure::Closed,
    )
    .unwrap()
}
fn plans() -> Vec<ProtectedTemporalProbe> {
    vec![
        ProtectedTemporalProbe {
            id: id(10),
            weight: 1,
            relation: ProtectedTemporalRelation::MappedPredicateTruth,
        },
        ProtectedTemporalProbe {
            id: id(11),
            weight: 3,
            relation: ProtectedTemporalRelation::Intervals {
                relation: TemporalProbe::EqualDuration,
                source: [0, 1, 2, 3],
                target: [0, 1, 2, 3],
            },
        },
    ]
}
fn run(
    policy: &TemporalPreservationPolicy,
    a: TemporalModelInput<'_>,
    b: TemporalModelInput<'_>,
) -> Result<TemporalPreservationReport, TemporalPreservationError> {
    run_checked(policy, a, b, false)
}
fn run_checked(
    policy: &TemporalPreservationPolicy,
    a: TemporalModelInput<'_>,
    b: TemporalModelInput<'_>,
    cancelled: bool,
) -> Result<TemporalPreservationReport, TemporalPreservationError> {
    let limits = ResourceLimits::uniform(8 * 1024 * 1024).unwrap();
    let owner = ResourceBroker::new(limits).unwrap();
    let cancellation = Cancellation::default();
    let root = owner
        .begin_run(
            19,
            a.query.context().source().digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancellation.clone(),
        )
        .unwrap();
    let requests = [ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(2 * 1024 * 1024).unwrap(),
    )];
    let mut source = root
        .reserve_for_source(a.query.context().source().digest(), &requests)
        .unwrap();
    let mut target = root
        .reserve_for_source(b.query.context().source().digest(), &requests)
        .unwrap();
    let result = {
        let av = source.lease();
        let bv = target.lease();
        let mut ast = FiniteResultStore::with_resources(1, &av).unwrap();
        let mut bst = FiniteResultStore::with_resources(1, &bv).unwrap();
        if cancelled {
            cancellation.cancel();
        }
        evaluate_temporal_preservation(
            policy,
            a,
            b,
            &mut ast,
            &mut bst,
            &av,
            &bv,
            EvaluationBudget::default(),
        )
    };
    source.release_confirmed().unwrap();
    target.release_confirmed().unwrap();
    result
}

#[test]
fn tlcx19_negative_scope_and_trace_substitution_are_not_zero_loss() {
    let aq = query("source");
    let bq = query("target");
    let at = trace([0, 1, 2, 3], Truth::True);
    let bt = trace([10, 11, 12, 13], Truth::True);
    let a = TemporalModelInput {
        query: &aq,
        trace: &at,
        scope: scope(),
    };
    let b = TemporalModelInput {
        query: &bq,
        trace: &bt,
        scope: scope(),
    };
    let policy = TemporalPreservationPolicy::declared(
        a,
        b,
        plans(),
        Some(DeclaredTimeTranslation {
            source_clock: clock(),
            target_clock: clock(),
            offset_nanoseconds: 10_000_000_000,
        }),
    )
    .unwrap();
    let foreign = TemporalModelInput {
        scope: ContextScope::declared(id(99), id(2), id(3), id(4)),
        ..b
    };
    let result = run(&policy, a, foreign).unwrap();
    assert!(matches!(
        result.state(),
        TemporalPreservationState::Incomparable(IncomparableReason::BindingMismatch)
    ));
    assert!(result.loss().is_none());
    let changed_trace = trace([10, 11, 12, 14], Truth::True);
    let changed = TemporalModelInput {
        trace: &changed_trace,
        ..b
    };
    assert!(matches!(
        run(&policy, a, changed).unwrap().state(),
        TemporalPreservationState::Incomparable(IncomparableReason::BindingMismatch)
    ));
    let context_swap = TemporalQuery::checked(
        bq.context().reconstructed([99; 32]),
        bq.nodes().to_vec(),
        bq.root(),
        bq.profile(),
    )
    .unwrap();
    let changed_context = TemporalModelInput {
        query: &context_swap,
        ..b
    };
    assert!(matches!(
        run(&policy, a, changed_context).unwrap().state(),
        TemporalPreservationState::Incomparable(IncomparableReason::BindingMismatch)
    ));
}

#[test]
fn tlcx19_actual_preserved_changed_unknown_use_fixed_weighted_denominator() {
    let aq = query("source");
    let bq = query("target");
    let at = trace([0, 1, 2, 3], Truth::True);
    let a = TemporalModelInput {
        query: &aq,
        trace: &at,
        scope: scope(),
    };
    let cases = [
        ([10, 11, 12, 13], Truth::True, (0, 1), (0, 1)),
        ([10, 11, 12, 14], Truth::True, (3, 4), (3, 4)),
        ([10, 11, 12, 14], Truth::Unknown, (3, 4), (1, 1)),
    ];
    let mut fixed_policy_digest = None;
    for (times, truth, lo, hi) in cases {
        let bt = trace(times, truth);
        let b = TemporalModelInput {
            query: &bq,
            trace: &bt,
            scope: scope(),
        };
        let policy = TemporalPreservationPolicy::declared(
            a,
            b,
            plans(),
            Some(DeclaredTimeTranslation {
                source_clock: clock(),
                target_clock: clock(),
                offset_nanoseconds: 10_000_000_000,
            }),
        )
        .unwrap();
        if let Some(expected) = fixed_policy_digest {
            assert_eq!(policy.digest(), expected);
        } else {
            fixed_policy_digest = Some(policy.digest());
        }
        let report = run(&policy, a, b).unwrap();
        assert_eq!(report.denominator(), 4);
        assert_eq!(
            report.loss().unwrap(),
            &RationalInterval::from_ratio(lo.0, lo.1, hi.0, hi.1).unwrap()
        );
        assert!(report.source_observation().is_some());
        assert!(report.target_observation().is_some());
        assert_eq!(report.validation_gap(), Gap::MissingEvidence);
    }
}

#[test]
fn tlcx19_actual_clock_mapping_and_absolute_offset_are_explicit() {
    let aq = query("source");
    let bq = query("target");
    let at = trace([0, 1, 2, 3], Truth::True);
    let bt = trace([10, 11, 12, 13], Truth::True);
    let a = TemporalModelInput {
        query: &aq,
        trace: &at,
        scope: scope(),
    };
    let b = TemporalModelInput {
        query: &bq,
        trace: &bt,
        scope: scope(),
    };
    let absolute = || {
        vec![ProtectedTemporalProbe {
            id: id(20),
            weight: 1,
            relation: ProtectedTemporalRelation::Intervals {
                relation: TemporalProbe::MappedAbsoluteStart,
                source: [0, 1, 2, 3],
                target: [0, 1, 2, 3],
            },
        }]
    };
    for (offset, expected) in [(0, 1), (10_000_000_000, 0)] {
        let policy = TemporalPreservationPolicy::declared(
            a,
            b,
            absolute(),
            Some(DeclaredTimeTranslation {
                source_clock: clock(),
                target_clock: clock(),
                offset_nanoseconds: offset,
            }),
        )
        .unwrap();
        assert_eq!(
            run(&policy, a, b).unwrap().loss().unwrap(),
            &RationalInterval::exact_ratio(expected, 1).unwrap()
        );
    }
    let missing = TemporalPreservationPolicy::declared(a, b, absolute(), None).unwrap();
    let missing_report = run(&missing, a, b).unwrap();
    assert!(
        missing_report.loss().is_none(),
        "unregistered map must not produce a scalar interval"
    );
    assert!(matches!(
        missing_report.state(),
        TemporalPreservationState::NoScalar {
            reason: Gap::MissingCalibration,
            ..
        }
    ));
    assert!(missing_report.source_observation().is_some());
    assert!(missing_report.target_observation().is_some());
    let wrong = ClockSpec::exact(id(99), TimeScale::Monotonic, id(6), id(7));
    let invalid = TemporalPreservationPolicy::declared(
        a,
        b,
        absolute(),
        Some(DeclaredTimeTranslation {
            source_clock: wrong,
            target_clock: clock(),
            offset_nanoseconds: 0,
        }),
    )
    .unwrap();
    assert!(matches!(
        run(&invalid, a, b).unwrap().state(),
        TemporalPreservationState::Incomparable(IncomparableReason::ClockMismatch)
    ));
}

#[test]
fn tlcx19_negative_empty_duplicate_and_zero_weight_probe_plans_are_rejected() {
    let q = query("input");
    let t = trace([0, 1, 2, 3], Truth::True);
    let input = TemporalModelInput {
        query: &q,
        trace: &t,
        scope: scope(),
    };
    assert!(TemporalPreservationPolicy::declared(input, input, vec![], None).is_err());
    let mut duplicate = plans();
    duplicate[1].id = duplicate[0].id;
    assert!(TemporalPreservationPolicy::declared(input, input, duplicate, None).is_err());
    let mut zero = plans();
    zero[0].weight = 0;
    assert!(TemporalPreservationPolicy::declared(input, input, zero, None).is_err());
    let mut invalid_indices = plans();
    invalid_indices[1].relation = ProtectedTemporalRelation::Intervals {
        relation: TemporalProbe::Before,
        source: [0, 1, 2, 4],
        target: [0, 1, 2, 3],
    };
    assert!(TemporalPreservationPolicy::declared(input, input, invalid_indices, None).is_err());
}

#[test]
fn tlcx19_actual_order_reversal_changes_loss_and_cancelled_lease_never_admits_pair() {
    let q = query("same source");
    let t = trace([0, 1, 2, 3], Truth::True);
    let input = TemporalModelInput {
        query: &q,
        trace: &t,
        scope: scope(),
    };
    let probes = vec![ProtectedTemporalProbe {
        id: id(30),
        weight: 1,
        relation: ProtectedTemporalRelation::Intervals {
            relation: TemporalProbe::Before,
            source: [0, 1, 2, 3],
            target: [2, 3, 0, 1],
        },
    }];
    let map = Some(DeclaredTimeTranslation {
        source_clock: clock(),
        target_clock: clock(),
        offset_nanoseconds: 0,
    });
    let policy = TemporalPreservationPolicy::declared(input, input, probes, map).unwrap();
    assert_eq!(
        run(&policy, input, input).unwrap().loss().unwrap(),
        &RationalInterval::exact_ratio(1, 1).unwrap()
    );
    assert!(matches!(
        run_checked(&policy, input, input, true),
        Err(TemporalPreservationError::Resource(
            ResourceError::Cancelled
        ))
    ));
}

#[test]
fn tlcx19_unsupported_legacy_timestamp_range_is_no_scalar_not_bounded_loss() {
    let q = query("extreme timestamps");
    let t = trace(
        [i64::MAX - 3, i64::MAX - 2, i64::MAX - 1, i64::MAX],
        Truth::True,
    );
    let input = TemporalModelInput {
        query: &q,
        trace: &t,
        scope: scope(),
    };
    let probes = plans();
    let policy = TemporalPreservationPolicy::declared(
        input,
        input,
        probes,
        Some(DeclaredTimeTranslation {
            source_clock: clock(),
            target_clock: clock(),
            offset_nanoseconds: 0,
        }),
    )
    .unwrap();
    let report = run(&policy, input, input).unwrap();
    assert!(
        report.loss().is_none(),
        "unsupported input range must not produce a scalar interval"
    );
    assert_eq!(report.denominator(), 4);
    assert!(matches!(
        report.state(),
        TemporalPreservationState::NoScalar {
            reason: Gap::UnsupportedQuantity,
            uncertainty: TemporalUncertainty {
                unsupported_quantity: 1,
                ..
            },
        }
    ));
    assert!(report.source_observation().is_some());
    assert!(report.target_observation().is_some());
}

#[test]
fn tlcx19_missing_mapping_is_no_scalar_not_unknown_observation() {
    let q = query("missing map");
    let t = trace([0, 1, 2, 3], Truth::Unknown);
    let input = TemporalModelInput {
        query: &q,
        trace: &t,
        scope: scope(),
    };
    let policy = TemporalPreservationPolicy::declared(input, input, plans(), None).unwrap();
    let report = run(&policy, input, input).unwrap();
    assert!(
        report.loss().is_none(),
        "lack of a mapping is not merely missing truth observation"
    );
    assert_eq!(report.denominator(), 4);
    assert!(matches!(
        report.state(),
        TemporalPreservationState::NoScalar {
            reason: Gap::MissingCalibration,
            uncertainty: TemporalUncertainty {
                missing_calibration: 2,
                ..
            },
        }
    ));
    assert!(report.source_observation().is_some());
    assert!(report.target_observation().is_some());
}
