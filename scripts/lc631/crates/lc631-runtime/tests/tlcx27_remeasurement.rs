//! 日本語: 新関係の明示化から固定probeを再測定し、実ownerで選択する。
//! English: Remeasure fixed probes after relation materialization using actual owners.
use lc631_analysis::creativity_measurement::{ComponentState, IndependenceStatus, InputBasis};
use lc631_core::{context::{ContextRevision, ContextScope, SourceRevision}, CandidateId};
use lc631_runtime::{
    contextual_distillation::{consumer::{measurement::*, *}, *},
    joint_selection::*, resource::*, structural_gain_measurement as structural,
};
use lc631_substrate::{measurement_numeric::RationalInterval, source::SourceStore, Digest32};
use std::{sync::Arc, time::Duration};

fn d(n: u8) -> Digest32 { Digest32::from_bytes([n; 32]) }
fn e(a: u8, b: u8) -> RelationEdit {
    RelationEdit { from: RelationNode(a), to: RelationNode(b) }
}
fn g(a: u16, b: u16) -> structural::Edge {
    structural::Edge(structural::Vertex(a), structural::Vertex(b))
}
fn source() -> SourceStore {
    SourceStore::new(Arc::from("Given finite materialized dependency model"), 1024).unwrap()
}
fn context(s: &SourceStore) -> ContextRevision {
    ContextRevision::original(SourceRevision::of(s.text()))
}
fn scope() -> ContextScope { ContextScope::declared(d(1), d(2), d(3), d(4)) }
fn engine(s: &SourceStore, availability: BasisAvailability) -> ContextualDistillation {
    ContextualDistillation::new(
        ImmutableRelationBasis::checked(context(s), scope(), 4,
            &[e(0, 1), e(1, 2), e(2, 3)], availability).unwrap(),
        DistillationLimits { max_epochs: 4, max_proposals: 8, max_relations: 16 },
    ).unwrap()
}
fn add(state: &mut ContextualDistillation, relation: RelationEdit) -> NextRelationQuery {
    state.epoch(&[RelationProposal { context: state.basis().context(),
        scope: state.basis().scope(), edit: relation }]).unwrap().next_query.unwrap()
}
fn run(s: &SourceStore, cancel: Cancellation) -> (ResourceBroker, RunResources, Reservation) {
    let cap = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker.begin_run(27, s.id().digest(), cap,
        Deadline::after(Duration::from_secs(120)).unwrap(), cancel).unwrap();
    let res = run.reserve(&[ResourceRequest::new(ResourceKind::Ram,
        ResourceAmount::checked(16 * 1024 * 1024).unwrap())]).unwrap();
    (broker, run, res)
}
fn checked_path(state: &ContextualDistillation, request: NextRelationQuery) -> DependencyPath<'_> {
    consume_dependency_path(revalidate_next_query(state, request, 1000).unwrap()).unwrap()
}
fn pins(s: &SourceStore) -> SelectionPins {
    SelectionPins { source: context(s).source(), scope: scope(), baseline: d(5),
        hard_invariants: d(6), policy: d(7), goal_profile: Some(d(8)), evidence_revision: d(9) }
}
fn binding(s: &SourceStore, id: u64) -> structural::CandidateBinding {
    structural::CandidateBinding { id: CandidateId(id), identity: d(20 + id as u8),
        context: context(s), pins: pins(s) }
}
fn row<'a>(s: &SourceStore, id: u64, axes: &'a [AxisValue<'a>], loss: &'a RationalInterval,
    key: &'a [u8], measurement: Digest32) -> JointCandidate<'a> {
    JointCandidate { id: CandidateId(id), context: context(s), pins: pins(s),
        hard_gate: ReportedGate::Pass, map_back: ReportedGate::Pass, axes,
        total_loss: AxisValue::Exact(loss), canonical_key: key, measurement_binding: measurement }
}

#[test]
fn tlcx27_remeasurement_negative_foreign_source_resource_and_path_cuts() {
    let s = source();
    let mut state = engine(&s, BasisAvailability::Given);
    let request = add(&mut state, e(0, 2));
    assert!(matches!(revalidate_next_query(&state, request, 0),
        Err(RelationConsumerHold::ResourceExhausted)));
    let foreign = SourceStore::new(Arc::from("foreign"), 1024).unwrap();
    let (_broker, _run, res) = run(&s, Cancellation::default());
    let view = res.lease();
    assert!(matches!(project_relation_graph(checked_path(&state, request), &foreign, &view),
        Err(RelationMeasurementError::SourceMismatch)));
    let (_other_broker, _other_run, other_res) = run(&s, Cancellation::default());
    let other = other_res.lease();
    let snapshot = project_relation_graph(checked_path(&state, request), &s, &view).unwrap();
    assert!(matches!(snapshot.graph_current(&other),
        Err(RelationMeasurementError::ForeignReservation)));
    let mut wrong_scope = request;
    wrong_scope.scope = ContextScope::declared(d(99), d(2), d(3), d(4));
    assert!(matches!(revalidate_next_query(&state, wrong_scope, 1000),
        Err(RelationConsumerHold::ForeignScope)));
}

#[test]
fn tlcx27_remeasurement_negative_validator_cut_never_manufactures_graph() {
    for availability in [BasisAvailability::Unavailable, BasisAvailability::Unsupported] {
        let s = source();
        let mut state = engine(&s, availability);
        let before = state.state_revision();
        let report = state.epoch(&[RelationProposal { context: context(&s), scope: scope(),
            edit: e(0, 2) }]).unwrap();
        assert!(report.next_query.is_none());
        assert_eq!(report.before, before);
        assert_eq!(report.after, before);
        assert_eq!(report.residuals().count(), 1);
        let forged = NextRelationQuery { context: context(&s), scope: scope(),
            state_revision: before, basis_digest: state.basis().digest(),
            relation: e(0, 2), basis: ValidationBasis::GivenFiniteReachability };
        assert!(matches!(revalidate_next_query(&state, forged, 1000),
            Err(RelationConsumerHold::NeedsEvidence | RelationConsumerHold::Unsupported)));
    }
}

#[test]
fn tlcx27_remeasurement_negative_stale_and_membership_do_not_dispatch() {
    let s = source();
    let mut state = engine(&s, BasisAvailability::Given);
    let stale = add(&mut state, e(1, 3));
    add(&mut state, e(0, 2));
    assert!(matches!(revalidate_next_query(&state, stale, 1000),
        Err(RelationConsumerHold::StaleState)));
    let before = state.state_revision();
    let report = state.epoch(&[RelationProposal { context: context(&s), scope: scope(),
        edit: e(0, 1) }]).unwrap();
    assert!(report.next_query.is_none());
    assert_eq!(report.before, report.after);
    assert_eq!(state.state_revision(), before);
}

#[test]
fn tlcx27_remeasurement_actual_materialization_changes_selection_and_current_cuts() {
    use lc631_runtime::{spatiotemporal_selection::TemporalOwner, temporal_preservation::*};
    use lc631_solvers::temporal::*;
    for materialize in [false, true] {
        let s = source();
        let mut state = engine(&s, BasisAvailability::Given);
        // Independently fixed expected representation and probes BEFORE any epoch.
        // This measures explicit edges, not a change of the DAG's transitive truth.
        let expected_edges = [g(0, 1), g(0, 2), g(0, 3), g(1, 2), g(2, 3)];
        let expected = structural::ClosedGraph::checked(&s, context(&s), 4, &expected_edges).unwrap();
        let fixed = [structural::ProtectedEdge { edge: g(0, 2), weight: 3 },
            structural::ProtectedEdge { edge: g(0, 3), weight: 1 }];
        let mapping = [0, 1, 2, 3].map(|n| structural::Correspondence {
            source: structural::Vertex(n), target: structural::Vertex(n) });
        let plan = structural::PreservationPlan::declared(&expected, scope(), 4,
            &fixed, Some(&mapping)).unwrap();
        let initial_request = add(&mut state, e(1, 3)); // Deliberately outside fixed probe set.
        let request = if materialize { add(&mut state, e(0, 2)) } else { initial_request };
        let cancel = Cancellation::default();
        let (_broker, _run, res) = run(&s, cancel.clone());
        let view = res.lease();
        let snapshot = project_relation_graph(checked_path(&state, request), &s, &view).unwrap();
        assert_eq!(snapshot.request(), request);
        assert_eq!(snapshot.path().input_basis(), RelationInputBasis::Given);
        assert_eq!(snapshot.path().independence(), RelationIndependence::SharedBasisUnassessed);
        let graph = snapshot.graph_current(&view).unwrap();
        let competitor_edges = [g(0, 1), g(0, 3), g(1, 2), g(2, 3)];
        let competitor = structural::ClosedGraph::checked(&s, context(&s), 4,
            &competitor_edges).unwrap();
        let a = structural::OwnedPreservation::produce(&plan, Some(&graph), binding(&s, 1), &view).unwrap();
        let b = structural::OwnedPreservation::produce(&plan, Some(&competitor), binding(&s, 2), &view).unwrap();
        // Independent weighted-set oracle: A=0 or 3/4; competitor=1/4.
        let wanted = RationalInterval::exact_ratio(if materialize { 3 } else { 0 }, 4).unwrap();
        let report = a.report_current(&view).unwrap();
        assert_eq!(report.state, ComponentState::Measured(wanted));
        assert_eq!(report.basis, InputBasis::Given);
        assert_eq!(report.independence, IndependenceStatus::SharedDependency);
        assert_eq!(plan.denominator(), 4);

        // One actual finite temporal identity supplies the only active loss.
        // No absent L_trans/F2 component is replaced by zero.
        let q = TemporalQuery::checked(context(&s), vec![TemporalNode::Atom(0)], 0,
            TemporalProfile::ClosedFiniteKleeneV1).unwrap();
        let clock = ClockSpec::exact(d(40), TimeScale::Monotonic, d(41), d(42));
        let trace = FiniteTrace::checked(vec![TraceStep::new(
            Instant::absolute(clock, 0, TimeUnit::Second).unwrap(), vec![Truth::True])],
            TraceClosure::Closed).unwrap();
        let input = TemporalModelInput { query: &q, trace: &trace, scope: scope() };
        let temporal_plan = TemporalPreservationPolicy::declared(input, input,
            vec![ProtectedTemporalProbe { id: d(44), weight: 1,
                relation: ProtectedTemporalRelation::MappedPredicateTruth }],
            Some(DeclaredTimeTranslation { source_clock: clock, target_clock: clock,
                offset_nanoseconds: 0 })).unwrap();
        let temporal = TemporalOwner::produce(&temporal_plan, input, input, &view, &view,
            EvaluationBudget::default()).unwrap();
        let loss = temporal.historical_report().loss().unwrap();
        let kinds = [AxisKind::Goal(d(8)), AxisKind::TemporalLoss, AxisKind::Preserve];
        let policy = SelectionPolicy { pins: pins(&s), axes: &kinds, representative_order: None,
            declared_goal_solve_overlap: false, max_candidates: 2, max_canonical_bytes: 128 };
        let aa = [a.axis_current(&view).unwrap().value, temporal.axis_current(&view).unwrap(),
            a.axis_current(&view).unwrap().value];
        let ab = [b.axis_current(&view).unwrap().value, temporal.axis_current(&view).unwrap(),
            b.axis_current(&view).unwrap().value];
        // The explicitly declared goal is binary-edge preservation, identity map
        // on [0,1]; Goal and Preserve share this SAME primitive, not two observations.
        let rows = [row(&s, 1, &aa, loss, b"materialized", a.measurement_binding()),
            row(&s, 2, &ab, loss, b"competitor", b.measurement_binding())];
        let owners = [structural::PreservationRow { row: 0, owner: &a },
            structural::PreservationRow { row: 1, owner: &b }];
        let selected = select_relation_current(&snapshot, 0, &policy, &rows, &owners,
            SearchCoverage::EvaluatedSet, &view).unwrap();
        assert_eq!(selected.report_current(&view).unwrap().recommendation,
            Some(if materialize { 0 } else { 1 }));
        // A different actual graph owner cannot stand in for the state snapshot.
        assert!(matches!(select_relation_current(&snapshot, 1, &policy, &rows, &owners,
            SearchCoverage::EvaluatedSet, &view), Err(RelationMeasurementError::BindingMismatch)));
        assert!(select_relation_current(&snapshot, 0, &policy, &rows, &owners[..1],
            SearchCoverage::EvaluatedSet, &view).is_err());
        let detached = a.historical_report().clone();
        let detached_axes = [a.axis_current(&view).unwrap().value,
            temporal.axis_current(&view).unwrap(), project_component_reported(&detached).unwrap().value];
        let forged = [row(&s, 1, &detached_axes, loss, b"materialized", a.measurement_binding()),
            row(&s, 2, &ab, loss, b"competitor", b.measurement_binding())];
        assert!(select_relation_current(&snapshot, 0, &policy, &forged, &owners,
            SearchCoverage::EvaluatedSet, &view).is_err());
        cancel.cancel();
        assert!(snapshot.graph_current(&view).is_err());
        assert!(selected.report_current(&view).is_err());
        assert!(select_relation_current(&snapshot, 0, &policy, &rows, &owners,
            SearchCoverage::EvaluatedSet, &view).is_err());
    }
}
