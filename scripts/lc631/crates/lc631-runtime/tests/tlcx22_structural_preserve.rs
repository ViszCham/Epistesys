//! 日本語: 未登録対応・source差替え・歴史report注入を先に切る。
//! English: First cut unregistered mappings, source swaps and historical report injection.
use lc631_analysis::creativity_measurement::{
    ComponentState, IndependenceStatus, InputBasis, MeasurementGap,
};
use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    CandidateId,
};
use lc631_runtime::structural_gain_measurement::*;
use lc631_runtime::{joint_selection::*, resource::*};
use lc631_substrate::{measurement_numeric::RationalInterval, source::SourceStore, Digest32};
use std::{sync::Arc, time::Duration};

fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
fn e(a: u16, b: u16) -> Edge {
    Edge(Vertex(a), Vertex(b))
}
fn source() -> SourceStore {
    SourceStore::new(Arc::from("Given closed directed graph model"), 1024).unwrap()
}
fn context(s: &SourceStore) -> ContextRevision {
    ContextRevision::original(SourceRevision::of(s.text()))
}
fn scope() -> ContextScope {
    ContextScope::declared(d(1), d(2), d(3), d(4))
}
fn pins(s: &SourceStore) -> SelectionPins {
    SelectionPins {
        source: context(s).source(),
        scope: scope(),
        baseline: d(5),
        hard_invariants: d(6),
        policy: d(7),
        goal_profile: Some(d(8)),
        evidence_revision: d(9),
    }
}
fn binding(s: &SourceStore, id: u64) -> CandidateBinding {
    CandidateBinding {
        id: CandidateId(id),
        identity: d(20 + id as u8),
        context: context(s),
        pins: pins(s),
    }
}
fn probes() -> [ProtectedEdge; 2] {
    [
        ProtectedEdge {
            edge: e(0, 1),
            weight: 1,
        },
        ProtectedEdge {
            edge: e(1, 2),
            weight: 3,
        },
    ]
}
fn map() -> [Correspondence; 3] {
    [
        Correspondence {
            source: Vertex(0),
            target: Vertex(0),
        },
        Correspondence {
            source: Vertex(1),
            target: Vertex(1),
        },
        Correspondence {
            source: Vertex(2),
            target: Vertex(2),
        },
    ]
}
fn run(s: &SourceStore, cancel: Cancellation) -> (ResourceBroker, RunResources, Reservation) {
    let cap = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker
        .begin_run(
            22,
            s.id().digest(),
            cap,
            Deadline::after(Duration::from_secs(120)).unwrap(),
            cancel,
        )
        .unwrap();
    let res = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    (broker, run, res)
}

#[test]
fn tlcx22_structural_contract_is_given_not_rebind_or_f2_completion() {
    assert_eq!(
        METRIC,
        lc631_analysis::creativity_measurement::ComponentMetric::Preserve
    );
}

#[test]
fn tlcx22_structural_negative_invalid_source_probe_map_and_scope() {
    let s = source();
    let edges = [e(0, 1), e(1, 2)];
    let graph = ClosedGraph::checked(&s, context(&s), 3, &edges).unwrap();
    let p = probes();
    let m = map();
    assert!(ClosedGraph::checked(
        &s,
        ContextRevision::original(SourceRevision::of("foreign")),
        3,
        &edges
    )
    .is_err());
    assert!(ClosedGraph::checked(&s, context(&s), 3, &[e(1, 2), e(0, 1)]).is_err());
    assert!(ClosedGraph::checked(&s, context(&s), 3, &[e(0, 4)]).is_err());
    assert!(PreservationPlan::declared(&graph, scope(), 3, &[], Some(&m)).is_err());
    assert!(PreservationPlan::declared(
        &graph,
        scope(),
        3,
        &[ProtectedEdge {
            edge: e(2, 0),
            weight: 1
        }],
        Some(&m)
    )
    .is_err());
    let duplicate_map = [
        Correspondence {
            source: Vertex(0),
            target: Vertex(0),
        },
        Correspondence {
            source: Vertex(1),
            target: Vertex(0),
        },
    ];
    assert!(PreservationPlan::declared(&graph, scope(), 3, &p, Some(&duplicate_map)).is_err());
    let plan = PreservationPlan::declared(&graph, scope(), 3, &p, Some(&m)).unwrap();
    let (_broker, _run, res) = run(&s, Cancellation::default());
    let view = res.lease();
    let mut b = binding(&s, 1);
    b.pins.scope = ContextScope::declared(d(99), d(2), d(3), d(4));
    assert!(matches!(
        OwnedPreservation::produce(&plan, Some(&graph), b, &view),
        Err(Error::BindingMismatch)
    ));
    let foreign = source();
    let (_other_broker, _other_run, other_res) = run(&foreign, Cancellation::default());
    let other_view = other_res.lease();
    let actual = OwnedPreservation::produce(&plan, Some(&graph), binding(&s, 1), &view).unwrap();
    assert_eq!(view.run_id(), other_view.run_id());
    assert_eq!(view.reservation_id(), other_view.reservation_id());
    assert!(matches!(
        actual.report_current(&other_view),
        Err(Error::ForeignReservation)
    ));
}

#[test]
fn tlcx22_structural_manual_edge_golden_identity_drop_rename_and_unknown() {
    let s = source();
    let edges = [e(0, 1), e(1, 2)];
    let deleted = [e(0, 1)];
    let graph = ClosedGraph::checked(&s, context(&s), 3, &edges).unwrap();
    let target = ClosedGraph::checked(&s, context(&s), 3, &deleted).unwrap();
    let p = probes();
    let m = map();
    let plan = PreservationPlan::declared(&graph, scope(), 3, &p, Some(&m)).unwrap();
    let incomplete = PreservationPlan::declared(&graph, scope(), 3, &p, Some(&m[..2])).unwrap();
    let absent = PreservationPlan::declared(&graph, scope(), 3, &p, None).unwrap();
    let renamed_edges = [e(0, 1), e(2, 0)];
    let renamed = ClosedGraph::checked(&s, context(&s), 3, &renamed_edges).unwrap();
    let rename = [
        Correspondence {
            source: Vertex(0),
            target: Vertex(2),
        },
        Correspondence {
            source: Vertex(1),
            target: Vertex(0),
        },
        Correspondence {
            source: Vertex(2),
            target: Vertex(1),
        },
    ];
    let rename_plan = PreservationPlan::declared(&graph, scope(), 3, &p, Some(&rename)).unwrap();
    let (_broker, _run, res) = run(&s, Cancellation::default());
    let view = res.lease();
    let identity = OwnedPreservation::produce(&plan, Some(&graph), binding(&s, 1), &view).unwrap();
    let drop = OwnedPreservation::produce(&plan, Some(&target), binding(&s, 2), &view).unwrap();
    let rename =
        OwnedPreservation::produce(&rename_plan, Some(&renamed), binding(&s, 3), &view).unwrap();
    let unknown = OwnedPreservation::produce(&plan, None, binding(&s, 4), &view).unwrap();
    let unsupported =
        OwnedPreservation::produce(&incomplete, Some(&target), binding(&s, 5), &view).unwrap();
    let unregistered =
        OwnedPreservation::produce(&absent, Some(&target), binding(&s, 6), &view).unwrap();
    // Independent handwritten set oracle: 0→1 survives(weight1),1→2 absent(weight3).
    assert_eq!(plan.denominator(), 4);
    assert_eq!(
        identity.report_current(&view).unwrap().state,
        ComponentState::Measured(RationalInterval::exact_ratio(1, 1).unwrap())
    );
    assert_eq!(
        drop.report_current(&view).unwrap().state,
        ComponentState::Measured(RationalInterval::exact_ratio(1, 4).unwrap())
    );
    assert_eq!(
        rename.report_current(&view).unwrap().state,
        ComponentState::Measured(RationalInterval::exact_ratio(1, 1).unwrap())
    );
    assert_eq!(
        unknown.report_current(&view).unwrap().state,
        ComponentState::Measured(RationalInterval::from_ratio(0, 1, 1, 1).unwrap())
    );
    assert_eq!(
        unsupported.report_current(&view).unwrap().state,
        ComponentState::Unknown(MeasurementGap::Unsupported)
    );
    assert_eq!(
        unregistered.report_current(&view).unwrap().state,
        ComponentState::Unknown(MeasurementGap::Unsupported)
    );
    assert_eq!(
        identity.report_current(&view).unwrap().basis,
        InputBasis::Given
    );
    assert_eq!(
        identity.report_current(&view).unwrap().independence,
        IndependenceStatus::SharedDependency
    );
    // A real Preserve leaf cannot satisfy strict F2's missing Rebind/etc.
    let required = [
        METRIC,
        lc631_analysis::creativity_measurement::ComponentMetric::Rebind,
    ];
    let strict = lc631_analysis::creativity_measurement::AssessmentPolicy {
        scope: scope().comparison(),
        baseline: pins(&s).baseline,
        required: &required,
    };
    assert!(matches!(
        lc631_analysis::creativity_measurement::CreativeAssessment::from_reports(
            &strict,
            std::slice::from_ref(identity.report_current(&view).unwrap())
        ),
        Err(lc631_analysis::creativity_measurement::MeasurementError::MissingMetric)
    ));
}

fn row<'a>(
    b: CandidateBinding,
    axes: &'a [AxisValue<'a>],
    total: &'a RationalInterval,
    key: &'a [u8],
    digest: Digest32,
) -> JointCandidate<'a> {
    JointCandidate {
        id: b.id,
        context: b.context,
        pins: b.pins,
        hard_gate: ReportedGate::Pass,
        map_back: ReportedGate::Pass,
        axes,
        total_loss: AxisValue::Exact(total),
        canonical_key: key,
        measurement_binding: digest,
    }
}

#[test]
fn tlcx22_structural_actual_component_selection_flip_and_current_cuts() {
    use lc631_runtime::{spatiotemporal_selection::TemporalOwner, temporal_preservation::*};
    use lc631_solvers::temporal::*;
    for flipped in [false, true] {
        let s = source();
        let edges = [e(0, 1), e(1, 2)];
        let deleted = [e(0, 1)];
        let graph = ClosedGraph::checked(&s, context(&s), 3, &edges).unwrap();
        let changed = ClosedGraph::checked(&s, context(&s), 3, &deleted).unwrap();
        let p = probes();
        let m = map();
        let plan = PreservationPlan::declared(&graph, scope(), 3, &p, Some(&m)).unwrap();
        let cancel = Cancellation::default();
        let (_broker, _run, res) = run(&s, cancel.clone());
        let view = res.lease();
        let a = OwnedPreservation::produce(
            &plan,
            Some(if flipped { &changed } else { &graph }),
            binding(&s, 1),
            &view,
        )
        .unwrap();
        let b = OwnedPreservation::produce(
            &plan,
            Some(if flipped { &graph } else { &changed }),
            binding(&s, 2),
            &view,
        )
        .unwrap();
        // A real identity temporal producer supplies the sole active loss=0;
        // no absent aggregate/F2 components are assigned zero by this fixture.
        let q = TemporalQuery::checked(
            context(&s),
            vec![TemporalNode::Atom(0)],
            0,
            TemporalProfile::ClosedFiniteKleeneV1,
        )
        .unwrap();
        let clock = ClockSpec::exact(d(40), TimeScale::Monotonic, d(41), d(42));
        let trace = FiniteTrace::checked(
            vec![TraceStep::new(
                Instant::absolute(clock, 0, TimeUnit::Second).unwrap(),
                vec![Truth::True],
            )],
            TraceClosure::Closed,
        )
        .unwrap();
        let input = TemporalModelInput {
            query: &q,
            trace: &trace,
            scope: scope(),
        };
        let temporal_plan = TemporalPreservationPolicy::declared(
            input,
            input,
            vec![ProtectedTemporalProbe {
                id: d(44),
                weight: 1,
                relation: ProtectedTemporalRelation::MappedPredicateTruth,
            }],
            Some(DeclaredTimeTranslation {
                source_clock: clock,
                target_clock: clock,
                offset_nanoseconds: 0,
            }),
        )
        .unwrap();
        let temporal = TemporalOwner::produce(
            &temporal_plan,
            input,
            input,
            &view,
            &view,
            EvaluationBudget::default(),
        )
        .unwrap();
        let loss = temporal.historical_report().loss().unwrap();
        let zero = RationalInterval::exact_ratio(0, 1).unwrap();
        let kinds = [
            AxisKind::Goal(d(8)),
            AxisKind::TemporalLoss,
            AxisKind::Preserve,
        ];
        let policy = SelectionPolicy {
            pins: pins(&s),
            axes: &kinds,
            representative_order: None,
            declared_goal_solve_overlap: false,
            max_candidates: 2,
            max_canonical_bytes: 64,
        };
        let aa = [
            AxisValue::Exact(&zero),
            temporal.axis_current(&view).unwrap(),
            a.axis_current(&view).unwrap().value,
        ];
        let ab = [
            AxisValue::Exact(&zero),
            temporal.axis_current(&view).unwrap(),
            b.axis_current(&view).unwrap().value,
        ];
        let rows = [
            row(binding(&s, 1), &aa, loss, b"a", a.measurement_binding()),
            row(binding(&s, 2), &ab, loss, b"b", b.measurement_binding()),
        ];
        let owners = [
            PreservationRow { row: 0, owner: &a },
            PreservationRow { row: 1, owner: &b },
        ];
        let selected =
            select_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, &view).unwrap();
        assert_eq!(
            selected.report_current(&view).unwrap().recommendation,
            Some(usize::from(flipped))
        );
        assert!(select_current(
            &policy,
            &rows,
            &owners[..1],
            SearchCoverage::EvaluatedSet,
            &view
        )
        .is_err());
        let detached = a.historical_report().clone();
        let copied = lc631_runtime::joint_selection::project_component_reported(&detached).unwrap();
        let forged_axes = [
            AxisValue::Exact(&zero),
            temporal.axis_current(&view).unwrap(),
            copied.value,
        ];
        let forged = [
            row(
                binding(&s, 1),
                &forged_axes,
                loss,
                b"a",
                a.measurement_binding(),
            ),
            row(binding(&s, 2), &ab, loss, b"b", b.measurement_binding()),
        ];
        assert!(matches!(
            select_current(
                &policy,
                &forged,
                &owners,
                SearchCoverage::EvaluatedSet,
                &view
            ),
            Err(Error::BindingMismatch)
        ));
        cancel.cancel();
        assert!(selected.report_current(&view).is_err());
        assert!(a.report_current(&view).is_err());
        assert_eq!(
            selected.historical_report().recommendation,
            Some(usize::from(flipped))
        );
    }
}

#[test]
fn tlcx22_structural_actual_source_baseline_and_outcome_substitution_cut() {
    let s = source();
    let edges = [e(0, 1), e(1, 2)];
    let foreign_edges = [e(0, 1), e(1, 2), e(2, 0)];
    let deleted = [e(0, 1)];
    let graph = ClosedGraph::checked(&s, context(&s), 3, &edges).unwrap();
    let foreign_graph = ClosedGraph::checked(&s, context(&s), 3, &foreign_edges).unwrap();
    let target = ClosedGraph::checked(&s, context(&s), 3, &deleted).unwrap();
    let p = probes();
    let m = map();
    let plan = PreservationPlan::declared(&graph, scope(), 3, &p, Some(&m)).unwrap();
    let foreign_plan =
        PreservationPlan::declared(&foreign_graph, scope(), 3, &p, Some(&m)).unwrap();
    let (_broker, _run, res) = run(&s, Cancellation::default());
    let view = res.lease();
    let a = OwnedPreservation::produce(&plan, Some(&graph), binding(&s, 1), &view).unwrap();
    let b = OwnedPreservation::produce(&foreign_plan, Some(&foreign_graph), binding(&s, 2), &view)
        .unwrap();
    let cut = OwnedPreservation::produce(&plan, Some(&target), binding(&s, 1), &view).unwrap();
    let zero = RationalInterval::exact_ratio(0, 1).unwrap();
    let kinds = [AxisKind::Goal(d(8)), AxisKind::Preserve];
    let policy = SelectionPolicy {
        pins: pins(&s),
        axes: &kinds,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 2,
        max_canonical_bytes: 64,
    };
    let aa = [
        AxisValue::Exact(&zero),
        a.axis_current(&view).unwrap().value,
    ];
    let ab = [
        AxisValue::Exact(&zero),
        b.axis_current(&view).unwrap().value,
    ];
    let mut rows = [
        row(binding(&s, 1), &aa, &zero, b"a", a.measurement_binding()),
        row(binding(&s, 2), &ab, &zero, b"b", b.measurement_binding()),
    ];
    // No aggregate exists here: these negative cases must fail ownership
    // admission before any recommendation; never invent L_trans=0.
    rows[0].total_loss = AxisValue::Unavailable(MissingAxis::Undefined);
    rows[1].total_loss = AxisValue::Unavailable(MissingAxis::Undefined);
    let owners = [
        PreservationRow { row: 0, owner: &a },
        PreservationRow { row: 1, owner: &b },
    ];
    assert!(matches!(
        select_current(&policy, &rows, &owners, SearchCoverage::EvaluatedSet, &view),
        Err(Error::BindingMismatch)
    ));
    let outcome_swap = [PreservationRow {
        row: 0,
        owner: &cut,
    }];
    let mut swapped = [row(
        binding(&s, 1),
        &aa,
        &zero,
        b"a",
        cut.measurement_binding(),
    )];
    swapped[0].total_loss = AxisValue::Unavailable(MissingAxis::Undefined);
    assert!(matches!(
        select_current(
            &policy,
            &swapped,
            &outcome_swap,
            SearchCoverage::EvaluatedSet,
            &view
        ),
        Err(Error::BindingMismatch)
    ));
}
