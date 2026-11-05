//! 日本語: 生産元所有と現在leaseの切断を先に検査する。Givenの有限算術のみ。
//! English: Producer ownership/current-lease cuts first; Given finite arithmetic only.

use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    CandidateId,
};
use lc631_runtime::{
    joint_selection::*,
    resource::*,
    spatiotemporal_selection::{self as current, *},
};
use lc631_runtime::{spatial_preservation::*, temporal_preservation::*};
use lc631_solvers::{
    causal_spatial::{FrameId, Rectangle, SpatialRelation},
    temporal::*,
};
use lc631_substrate::{measurement_numeric::RationalInterval, Digest32};
use std::time::Duration;

fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn context() -> ContextRevision {
    ContextRevision::original(SourceRevision::of("Given temporal spatial model"))
}
fn scope() -> ContextScope {
    ContextScope::declared(d(1), d(2), d(3), d(4))
}
fn pins() -> SelectionPins {
    SelectionPins {
        source: context().source(),
        scope: scope(),
        baseline: d(5),
        hard_invariants: d(6),
        policy: d(7),
        goal_profile: Some(d(8)),
        evidence_revision: d(9),
    }
}
fn run(cancel: Cancellation) -> (ResourceBroker, RunResources, Reservation) {
    let cap = ResourceLimits::uniform(16 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker
        .begin_run(
            19,
            context().source().digest(),
            cap,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            cancel,
        )
        .unwrap();
    let res = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(8 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    (broker, run, res)
}
fn clock() -> ClockSpec {
    ClockSpec::exact(d(40), TimeScale::Monotonic, d(41), d(42))
}
fn query() -> TemporalQuery {
    TemporalQuery::checked(
        context(),
        vec![TemporalNode::Atom(0)],
        0,
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
fn temporal_policy(
    a: TemporalModelInput<'_>,
    b: TemporalModelInput<'_>,
    mapped: bool,
) -> TemporalPreservationPolicy {
    TemporalPreservationPolicy::declared(
        a,
        b,
        vec![
            ProtectedTemporalProbe {
                id: d(50),
                weight: 1,
                relation: ProtectedTemporalRelation::MappedPredicateTruth,
            },
            ProtectedTemporalProbe {
                id: d(51),
                weight: 3,
                relation: ProtectedTemporalRelation::Intervals {
                    relation: TemporalProbe::EqualDuration,
                    source: [0, 1, 2, 3],
                    target: [0, 1, 2, 3],
                },
            },
        ],
        mapped.then_some(DeclaredTimeTranslation {
            source_clock: clock(),
            target_clock: clock(),
            offset_nanoseconds: 0,
        }),
    )
    .unwrap()
}
fn model(broken: bool) -> SpatialModelInput {
    let frame = FrameId([60; 32]);
    // Handwritten relation gold: disjoint A=[0,1]²,B=[3,4]×[0,1]
    // has strictly-left=true; swapping A/B changes only that protected bit.
    let a = Rectangle::new(frame, [0, 0], [1, 1]).unwrap();
    let b = Rectangle::new(frame, [3, 0], [4, 1]).unwrap();
    SpatialModelInput {
        left: Some(if broken { b } else { a }),
        right: Some(if broken { a } else { b }),
        frame,
        unit: SpatialUnit::Pixel,
        context: context(),
        scope: scope(),
    }
}
fn spatial_policy(
    a: SpatialModelInput,
    b: SpatialModelInput,
    mapped: bool,
    unsupported: bool,
) -> SpatialPreservationPolicy {
    let relations = if unsupported {
        vec![ProtectedSpatialRelation::ContinuousDistance]
    } else {
        vec![
            ProtectedSpatialRelation::Kernel(SpatialRelation::Connected),
            ProtectedSpatialRelation::Kernel(SpatialRelation::StrictlyLeftOf),
        ]
    };
    SpatialPreservationPolicy::declared(
        a,
        b,
        relations
            .into_iter()
            .enumerate()
            .map(|(i, relation)| ProtectedSpatialProbe {
                id: d(70 + i as u8),
                weight: if i == 0 { 1 } else { 3 },
                relation,
            })
            .collect(),
        mapped.then_some(DeclaredSpatialMap {
            source_frame: a.frame,
            target_frame: b.frame,
            source_unit: a.unit,
            target_unit: b.unit,
            signs: (AxisSign::Positive, AxisSign::Positive),
            translation: (0, 0),
        }),
    )
    .unwrap()
}
fn row<'a>(
    id: u64,
    axes: &'a [AxisValue<'a>],
    loss: &'a RationalInterval,
    key: &'a [u8],
    binding: Digest32,
) -> JointCandidate<'a> {
    JointCandidate {
        id: CandidateId(id),
        context: context(),
        pins: pins(),
        hard_gate: ReportedGate::Pass,
        map_back: ReportedGate::Pass,
        axes,
        total_loss: AxisValue::Exact(loss),
        canonical_key: key,
        measurement_binding: binding,
    }
}
fn policy(kinds: &[AxisKind]) -> SelectionPolicy<'_> {
    SelectionPolicy {
        pins: pins(),
        axes: kinds,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 4,
        max_canonical_bytes: 512,
    }
}

#[test]
fn tlcx19_current_negative_detached_same_value_foreign_lease_scope_and_cancel() {
    let q = query();
    let t = trace([0, 1, 2, 3], Truth::True);
    let input = TemporalModelInput {
        query: &q,
        trace: &t,
        scope: scope(),
    };
    let plan = temporal_policy(input, input, true);
    let cancel = Cancellation::default();
    let (_broker, _run, res) = run(cancel.clone());
    let view = res.lease();
    let owner = TemporalOwner::produce(
        &plan,
        input,
        input,
        &view,
        &view,
        EvaluationBudget::default(),
    )
    .unwrap();
    let kinds = [AxisKind::Goal(d(8)), AxisKind::TemporalLoss];
    let pol = policy(&kinds);
    let z = RationalInterval::exact_ratio(0, 1).unwrap();
    let copied = owner.historical_report().clone();
    let owned = [RowOwners {
        row: 0,
        temporal: Some(&owner),
        spatial: None,
    }];
    let detached_axes = [
        AxisValue::Exact(&z),
        AxisValue::Exact(copied.loss().unwrap()),
    ];
    let detached = [row(1, &detached_axes, &z, b"a", owned[0].binding_digest())];
    assert!(matches!(
        current::select_current(&pol, &detached, &owned, SearchCoverage::EvaluatedSet, &view),
        Err(Error::BindingMismatch)
    ));
    let axes = [AxisValue::Exact(&z), owner.axis_current(&view).unwrap()];
    let mut rows = [row(1, &axes, &z, b"a", owned[0].binding_digest())];
    assert!(
        current::select_current(&pol, &rows, &[], SearchCoverage::EvaluatedSet, &view).is_err()
    );
    rows[0].pins.scope = ContextScope::declared(d(99), d(2), d(3), d(4));
    assert!(
        current::select_current(&pol, &rows, &owned, SearchCoverage::EvaluatedSet, &view).is_err()
    );
    rows[0].pins = pins();
    let selection =
        current::select_current(&pol, &rows, &owned, SearchCoverage::EvaluatedSet, &view).unwrap();
    let (_foreign_broker, _foreign_run, foreign_res) = run(Cancellation::default());
    let foreign = foreign_res.lease();
    assert_eq!(view.run_id(), foreign.run_id());
    assert_eq!(view.reservation_id(), foreign.reservation_id());
    assert!(matches!(
        owner.axis_current(&foreign),
        Err(Error::ForeignReservation)
    ));
    assert!(matches!(
        selection.report_current(&foreign),
        Err(Error::ForeignReservation)
    ));
    assert_eq!(
        selection.report_current(&view).unwrap().recommendation,
        Some(0)
    );
    cancel.cancel();
    assert!(owner.axis_current(&view).is_err());
    assert!(selection.report_current(&view).is_err());
    assert_eq!(selection.historical_report().recommendation, Some(0));
}

#[test]
fn tlcx19_current_actual_temporal_and_spatial_goldens_joint_winner_flip() {
    for flipped in [false, true] {
        let q = query();
        let base = trace([0, 1, 2, 3], Truth::True);
        let good = trace([10, 11, 12, 13], Truth::True);
        let bad = trace([10, 12, 13, 16], Truth::True);
        let a = TemporalModelInput {
            query: &q,
            trace: &base,
            scope: scope(),
        };
        let b = TemporalModelInput {
            query: &q,
            trace: if flipped { &bad } else { &good },
            scope: scope(),
        };
        let c = TemporalModelInput {
            query: &q,
            trace: if flipped { &good } else { &bad },
            scope: scope(),
        };
        let pb = temporal_policy(a, b, true);
        let pc = temporal_policy(a, c, true);
        let sa = model(false);
        let sb = model(!flipped);
        let sc = model(flipped);
        let spb = spatial_policy(sa, sb, true, false);
        let spc = spatial_policy(sa, sc, true, false);
        let (_broker, _run, res) = run(Cancellation::default());
        let view = res.lease();
        let tb =
            TemporalOwner::produce(&pb, a, b, &view, &view, EvaluationBudget::default()).unwrap();
        let tc =
            TemporalOwner::produce(&pc, a, c, &view, &view, EvaluationBudget::default()).unwrap();
        let xb = SpatialOwner::produce(&spb, sa, sb, &view, &view).unwrap();
        let xc = SpatialOwner::produce(&spc, sa, sc, &view, &view).unwrap();
        let zero = RationalInterval::exact_ratio(0, 1).unwrap();
        let three_quarters = RationalInterval::exact_ratio(3, 4).unwrap();
        // Independent hand arithmetic: truth unchanged(weight1), unequal
        // durations(weight3) =>3/4. Connected unchanged(1), left bit lost(3)=>3/4.
        assert_eq!(
            tb.historical_report().loss(),
            Some(if flipped { &three_quarters } else { &zero })
        );
        assert_eq!(
            xb.historical_report().loss(),
            Some(if flipped { &zero } else { &three_quarters })
        );
        // Separate single-axis actual consumers prove each axis can flip a
        // real winner. Combined equal tradeoff remains a declared representative.
        for temporal_only in [true, false] {
            let kinds = [
                AxisKind::Goal(d(8)),
                if temporal_only {
                    AxisKind::TemporalLoss
                } else {
                    AxisKind::SpatialLoss
                },
            ];
            let pol = policy(&kinds);
            let owners = [
                RowOwners {
                    row: 0,
                    temporal: temporal_only.then_some(&tb),
                    spatial: (!temporal_only).then_some(&xb),
                },
                RowOwners {
                    row: 1,
                    temporal: temporal_only.then_some(&tc),
                    spatial: (!temporal_only).then_some(&xc),
                },
            ];
            let ab = [
                AxisValue::Exact(&zero),
                if temporal_only {
                    tb.axis_current(&view).unwrap()
                } else {
                    xb.axis_current(&view).unwrap()
                },
            ];
            let ac = [
                AxisValue::Exact(&zero),
                if temporal_only {
                    tc.axis_current(&view).unwrap()
                } else {
                    xc.axis_current(&view).unwrap()
                },
            ];
            let lb = if temporal_only {
                tb.historical_report().loss().unwrap()
            } else {
                xb.historical_report().loss().unwrap()
            };
            let lc = if temporal_only {
                tc.historical_report().loss().unwrap()
            } else {
                xc.historical_report().loss().unwrap()
            };
            let rows = [
                row(1, &ab, lb, b"a", owners[0].binding_digest()),
                row(2, &ac, lc, b"b", owners[1].binding_digest()),
            ];
            let result =
                current::select_current(&pol, &rows, &owners, SearchCoverage::EvaluatedSet, &view)
                    .unwrap();
            let expected = if temporal_only {
                usize::from(flipped)
            } else {
                usize::from(!flipped)
            };
            assert_eq!(
                result.report_current(&view).unwrap().recommendation,
                Some(expected)
            );
        }
        let kinds = [
            AxisKind::Goal(d(8)),
            AxisKind::TemporalLoss,
            AxisKind::SpatialLoss,
        ];
        let pol = policy(&kinds);
        let owners = [
            RowOwners {
                row: 0,
                temporal: Some(&tb),
                spatial: Some(&xb),
            },
            RowOwners {
                row: 1,
                temporal: Some(&tc),
                spatial: Some(&xc),
            },
        ];
        let ab = [
            AxisValue::Exact(&zero),
            tb.axis_current(&view).unwrap(),
            xb.axis_current(&view).unwrap(),
        ];
        let ac = [
            AxisValue::Exact(&zero),
            tc.axis_current(&view).unwrap(),
            xc.axis_current(&view).unwrap(),
        ];
        let mut rows = [
            row(1, &ab, &zero, b"a", owners[0].binding_digest()),
            row(2, &ac, &zero, b"b", owners[1].binding_digest()),
        ];
        // No registered aggregate-reduction policy in this profile. Preserve
        // both actual loss axes/frontier but do not fabricate L_trans.
        rows[0].total_loss = AxisValue::Unavailable(MissingAxis::Undefined);
        rows[1].total_loss = AxisValue::Unavailable(MissingAxis::Undefined);
        let result =
            current::select_current(&pol, &rows, &owners, SearchCoverage::EvaluatedSet, &view)
                .unwrap();
        assert_eq!(result.report_current(&view).unwrap().frontier.len(), 2);
        assert_eq!(result.report_current(&view).unwrap().recommendation, None);
    }
}

#[test]
fn tlcx19_current_missing_map_unsupported_and_unknown_keep_distinct_states() {
    let q = query();
    let known = trace([0, 1, 2, 3], Truth::True);
    let unknown = trace([0, 1, 2, 3], Truth::Unknown);
    let a = TemporalModelInput {
        query: &q,
        trace: &known,
        scope: scope(),
    };
    let b = TemporalModelInput {
        trace: &unknown,
        ..a
    };
    let no_map = temporal_policy(a, b, false);
    let mapped = temporal_policy(a, b, true);
    let sa = model(false);
    let missing = SpatialModelInput { right: None, ..sa };
    let sp_missing = spatial_policy(sa, missing, true, false);
    let sp_unsupported = spatial_policy(sa, sa, true, true);
    let (_broker, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let t =
        TemporalOwner::produce(&no_map, a, b, &view, &view, EvaluationBudget::default()).unwrap();
    let u =
        TemporalOwner::produce(&mapped, a, b, &view, &view, EvaluationBudget::default()).unwrap();
    let s = SpatialOwner::produce(&sp_missing, sa, missing, &view, &view).unwrap();
    let x = SpatialOwner::produce(&sp_unsupported, sa, sa, &view, &view).unwrap();
    assert!(matches!(
        t.axis_current(&view).unwrap(),
        AxisValue::Unavailable(MissingAxis::Uncalibrated)
    ));
    assert!(matches!(
        x.axis_current(&view).unwrap(),
        AxisValue::Unavailable(MissingAxis::Unsupported)
    ));
    assert_eq!(
        u.historical_report().loss(),
        Some(&RationalInterval::from_ratio(0, 1, 1, 4).unwrap())
    );
    assert_eq!(
        s.historical_report().loss(),
        Some(&RationalInterval::from_ratio(0, 1, 1, 1).unwrap())
    );
    assert!(matches!(
        u.axis_current(&view).unwrap(),
        AxisValue::Bounded(_)
    ));
    assert!(matches!(
        s.axis_current(&view).unwrap(),
        AxisValue::Bounded(_)
    ));
    let kinds = [AxisKind::Goal(d(8)), AxisKind::TemporalLoss];
    let pol = policy(&kinds);
    let z = RationalInterval::exact_ratio(0, 1).unwrap();
    let owners = [RowOwners {
        row: 0,
        temporal: Some(&t),
        spatial: None,
    }];
    let axes = [AxisValue::Exact(&z), t.axis_current(&view).unwrap()];
    let mut rows = [row(1, &axes, &z, b"a", owners[0].binding_digest())];
    rows[0].total_loss = AxisValue::Unavailable(MissingAxis::Uncalibrated);
    let result =
        current::select_current(&pol, &rows, &owners, SearchCoverage::EvaluatedSet, &view).unwrap();
    assert_eq!(result.report_current(&view).unwrap().recommendation, None);
}

#[test]
fn tlcx19_current_source_policy_map_and_real_outcome_swap_cuts() {
    let q = query();
    let good = trace([0, 1, 2, 3], Truth::True);
    let false_trace = trace([0, 1, 2, 3], Truth::False);
    let a = TemporalModelInput {
        query: &q,
        trace: &good,
        scope: scope(),
    };
    let b = TemporalModelInput {
        trace: &false_trace,
        ..a
    };
    let plan = temporal_policy(a, a, true);
    let altered = temporal_policy(a, b, true);
    let alternate_map = TemporalPreservationPolicy::declared(
        a,
        a,
        plan.probes().to_vec(),
        Some(DeclaredTimeTranslation {
            source_clock: clock(),
            target_clock: clock(),
            offset_nanoseconds: 99,
        }),
    )
    .unwrap();
    let (_broker, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let right =
        TemporalOwner::produce(&plan, a, a, &view, &view, EvaluationBudget::default()).unwrap();
    let wrong =
        TemporalOwner::produce(&altered, a, b, &view, &view, EvaluationBudget::default()).unwrap();
    let changed_map = TemporalOwner::produce(
        &alternate_map,
        a,
        a,
        &view,
        &view,
        EvaluationBudget::default(),
    )
    .unwrap();
    // Actual finite solver output differs: true→false loses the weight-one
    // mapped predicate while both duration probes remain equal: exactly 1/4.
    assert_eq!(
        wrong.historical_report().loss(),
        Some(&RationalInterval::exact_ratio(1, 4).unwrap())
    );
    let foreign_q = TemporalQuery::checked(
        ContextRevision::original(SourceRevision::of("foreign source")),
        vec![TemporalNode::Atom(0)],
        0,
        TemporalProfile::ClosedFiniteKleeneV1,
    )
    .unwrap();
    let foreign = TemporalModelInput {
        query: &foreign_q,
        ..a
    };
    assert!(matches!(
        TemporalOwner::produce(&plan, foreign, a, &view, &view, EvaluationBudget::default()),
        Err(Error::BindingMismatch)
    ));
    let spatial = model(false);
    let spatial_plan = spatial_policy(spatial, spatial, true, false);
    let foreign_spatial = SpatialModelInput {
        context: foreign_q.context(),
        ..spatial
    };
    assert!(matches!(
        SpatialOwner::produce(&spatial_plan, foreign_spatial, spatial, &view, &view),
        Err(Error::BindingMismatch)
    ));
    let kinds = [AxisKind::Goal(d(8)), AxisKind::TemporalLoss];
    let pol = policy(&kinds);
    let z = RationalInterval::exact_ratio(0, 1).unwrap();
    let axes = [AxisValue::Exact(&z), right.axis_current(&view).unwrap()];
    let swapped = [RowOwners {
        row: 0,
        temporal: Some(&wrong),
        spatial: None,
    }];
    let rows = [row(1, &axes, &z, b"a", swapped[0].binding_digest())];
    // Even the freshly recomputed binding cannot launder the old result axis.
    assert!(matches!(
        current::select_current(&pol, &rows, &swapped, SearchCoverage::EvaluatedSet, &view),
        Err(Error::BindingMismatch)
    ));
    let mixed = [
        RowOwners {
            row: 0,
            temporal: Some(&right),
            spatial: None,
        },
        RowOwners {
            row: 1,
            temporal: Some(&changed_map),
            spatial: None,
        },
    ];
    let changed_axes = [
        AxisValue::Exact(&z),
        changed_map.axis_current(&view).unwrap(),
    ];
    let rows = [
        row(1, &axes, &z, b"a", mixed[0].binding_digest()),
        row(2, &changed_axes, &z, b"b", mixed[1].binding_digest()),
    ];
    assert!(matches!(
        current::select_current(&pol, &rows, &mixed, SearchCoverage::EvaluatedSet, &view),
        Err(Error::BindingMismatch)
    ));
    let owners = [RowOwners {
        row: 0,
        temporal: Some(&right),
        spatial: None,
    }];
    let mut rows = [row(1, &axes, &z, b"a", owners[0].binding_digest())];
    rows[0].map_back = ReportedGate::Fail;
    let blocked =
        current::select_current(&pol, &rows, &owners, SearchCoverage::EvaluatedSet, &view).unwrap();
    assert_eq!(blocked.report_current(&view).unwrap().recommendation, None);
    assert_eq!(
        blocked.report_current(&view).unwrap().dispositions,
        [Disposition::MapBackFailure]
    );
}

#[test]
fn tlcx19_current_rejects_actual_temporal_source_baseline_substitution() {
    let q = query();
    let truth = trace([0, 1, 2, 3], Truth::True);
    let falsehood = trace([0, 1, 2, 3], Truth::False);
    let a = TemporalModelInput {
        query: &q,
        trace: &truth,
        scope: scope(),
    };
    let changed_source = TemporalModelInput {
        trace: &falsehood,
        ..a
    };
    let pa = temporal_policy(a, a, true);
    let pb = temporal_policy(changed_source, changed_source, true);
    // Same declared probes/map/scope; the actual root model is different.
    assert_eq!(pa.digest(), pb.digest());
    let (_broker, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let oa = TemporalOwner::produce(&pa, a, a, &view, &view, EvaluationBudget::default()).unwrap();
    let ob = TemporalOwner::produce(
        &pb,
        changed_source,
        changed_source,
        &view,
        &view,
        EvaluationBudget::default(),
    )
    .unwrap();
    let zero = RationalInterval::exact_ratio(0, 1).unwrap();
    assert_eq!(oa.historical_report().loss(), Some(&zero));
    assert_eq!(ob.historical_report().loss(), Some(&zero));
    let common_root_plan = temporal_policy(a, changed_source, true);
    let common_root = TemporalOwner::produce(
        &common_root_plan,
        a,
        changed_source,
        &view,
        &view,
        EvaluationBudget::default(),
    )
    .unwrap();
    // Hand oracle: true→false predicate(weight1), durations preserved(weight3).
    assert_eq!(
        common_root.historical_report().loss(),
        Some(&RationalInterval::exact_ratio(1, 4).unwrap())
    );
    let kinds = [AxisKind::Goal(d(8)), AxisKind::TemporalLoss];
    let pol = policy(&kinds);
    let owners = [
        RowOwners {
            row: 0,
            temporal: Some(&oa),
            spatial: None,
        },
        RowOwners {
            row: 1,
            temporal: Some(&ob),
            spatial: None,
        },
    ];
    let aa = [AxisValue::Exact(&zero), oa.axis_current(&view).unwrap()];
    let ab = [AxisValue::Exact(&zero), ob.axis_current(&view).unwrap()];
    let rows = [
        row(1, &aa, &zero, b"a", owners[0].binding_digest()),
        row(2, &ab, &zero, b"b", owners[1].binding_digest()),
    ];
    assert!(matches!(
        current::select_current(&pol, &rows, &owners, SearchCoverage::EvaluatedSet, &view),
        Err(Error::BindingMismatch)
    ));
}

#[test]
fn tlcx19_current_rejects_actual_spatial_source_baseline_substitution() {
    let a = model(false);
    let changed_source = model(true);
    let pa = spatial_policy(a, a, true, false);
    let pb = spatial_policy(changed_source, changed_source, true, false);
    assert_eq!(pa.digest(), pb.digest());
    let (_broker, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let oa = SpatialOwner::produce(&pa, a, a, &view, &view).unwrap();
    let ob = SpatialOwner::produce(&pb, changed_source, changed_source, &view, &view).unwrap();
    let zero = RationalInterval::exact_ratio(0, 1).unwrap();
    assert_eq!(oa.historical_report().loss(), Some(&zero));
    assert_eq!(ob.historical_report().loss(), Some(&zero));
    let common_root_plan = spatial_policy(a, changed_source, true, false);
    let common_root =
        SpatialOwner::produce(&common_root_plan, a, changed_source, &view, &view).unwrap();
    // Hand oracle: disjoint remains disconnected(weight1), left bit lost(weight3).
    assert_eq!(
        common_root.historical_report().loss(),
        Some(&RationalInterval::exact_ratio(3, 4).unwrap())
    );
    let kinds = [AxisKind::Goal(d(8)), AxisKind::SpatialLoss];
    let pol = policy(&kinds);
    let owners = [
        RowOwners {
            row: 0,
            temporal: None,
            spatial: Some(&oa),
        },
        RowOwners {
            row: 1,
            temporal: None,
            spatial: Some(&ob),
        },
    ];
    let aa = [AxisValue::Exact(&zero), oa.axis_current(&view).unwrap()];
    let ab = [AxisValue::Exact(&zero), ob.axis_current(&view).unwrap()];
    let rows = [
        row(1, &aa, &zero, b"a", owners[0].binding_digest()),
        row(2, &ab, &zero, b"b", owners[1].binding_digest()),
    ];
    assert!(matches!(
        current::select_current(&pol, &rows, &owners, SearchCoverage::EvaluatedSet, &view),
        Err(Error::BindingMismatch)
    ));
}
