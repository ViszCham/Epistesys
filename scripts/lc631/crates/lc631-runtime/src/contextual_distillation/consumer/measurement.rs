//! 日本語: 検査済みstateの明示関係を既存の固定構造測定器へ渡す。
//! English: Project checked explicit relations into existing fixed structural probes.
//! Materialization changes representation, not transitive truth or source meaning.
#![forbid(unsafe_code)]

use super::{check_binding, DependencyPath, NextRelationQuery, RelationConsumerHold};
use crate::{joint_selection::*, structural_gain_measurement as structural};
use lc631_substrate::{
    resource::{ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
        ResourceReservationView},
    source::SourceStore,
};

/// Covers at most 4032 four-byte edges plus the small borrowed-path wrapper.
/// Logical owned payload only, excluding allocator metadata, source/state and RSS.
pub const RELATION_SNAPSHOT_RAM: u64 = 32 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelationMeasurementError {
    SourceMismatch,
    ForeignReservation,
    BindingMismatch,
    AllocationFailed,
    Relation(RelationConsumerHold),
    Resource(ResourceError),
    Structural(structural::Error),
}

/// Owns only the projected edge copy; immutable state/source remain borrowed.
/// No report/bool/Deserialize constructor can fabricate this snapshot.
/// ```compile_fail
/// use lc631_runtime::contextual_distillation::consumer::measurement::RelationGraphSnapshot;
/// let snapshot = RelationGraphSnapshot { };
/// ```
pub struct RelationGraphSnapshot<'input, 'view> {
    edges: Vec<structural::Edge>,
    path: DependencyPath<'input>,
    source: &'input SourceStore,
    retained: ResourceClaim<'view>,
}

/// Consume an actually revalidated admitted query, never an arbitrary edge list.
/// The one-to-one node projection preserves numeric node correspondence only;
/// it does not infer semantic identity or independently validate the Given basis.
pub fn project_relation_graph<'input, 'view>(
    path: DependencyPath<'input>,
    source: &'input SourceStore,
    resources: &'view ResourceReservationView<'_>,
) -> Result<RelationGraphSnapshot<'input, 'view>, RelationMeasurementError> {
    resources.check().map_err(RelationMeasurementError::Resource)?;
    let request = path.request();
    if source.id().digest() != request.context.source().digest()
        || resources.source_revision() != source.id().digest()
    {
        return Err(RelationMeasurementError::SourceMismatch);
    }
    check_binding(path.checked.state, request).map_err(RelationMeasurementError::Relation)?;
    let retained = resources.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(RELATION_SNAPSHOT_RAM)
            .map_err(RelationMeasurementError::Resource)?,
    )]).map_err(RelationMeasurementError::Resource)?;
    let relations = path.checked.state.relations();
    let mut edges = Vec::new();
    edges.try_reserve_exact(relations.len()).map_err(|_| RelationMeasurementError::AllocationFailed)?;
    if edges.capacity() > relations.len() {
        return Err(RelationMeasurementError::AllocationFailed);
    }
    edges.extend(relations.iter().map(|relation| structural::Edge(
        structural::Vertex(u16::from(relation.from.0)),
        structural::Vertex(u16::from(relation.to.0)),
    )));
    let snapshot = RelationGraphSnapshot { edges, path, source, retained };
    snapshot.check_current(resources)?;
    // Also run the existing graph's canonical order/domain/source checks.
    snapshot.graph_current(resources)?;
    Ok(snapshot)
}

impl RelationGraphSnapshot<'_, '_> {
    pub const fn path(&self) -> &DependencyPath<'_> { &self.path }

    pub const fn request(&self) -> NextRelationQuery { self.path.request() }

    pub fn graph_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<structural::ClosedGraph<'_>, RelationMeasurementError> {
        self.check_current(resources)?;
        let graph = structural::ClosedGraph::checked(
            self.source,
            self.request().context,
            u16::from(self.path.checked.state.basis().node_count()),
            &self.edges,
        ).map_err(RelationMeasurementError::Structural)?;
        self.retained.check().map_err(RelationMeasurementError::Resource)?;
        Ok(graph)
    }

    fn check_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<(), RelationMeasurementError> {
        resources.check().map_err(RelationMeasurementError::Resource)?;
        self.retained.check().map_err(RelationMeasurementError::Resource)?;
        if !self.retained.same_reservation(resources) {
            return Err(RelationMeasurementError::ForeignReservation);
        }
        if resources.source_revision() != self.source.id().digest() {
            return Err(RelationMeasurementError::SourceMismatch);
        }
        check_binding(self.path.checked.state, self.request())
            .map_err(RelationMeasurementError::Relation)
    }
}

/// Keeps the state snapshot AND actual preservation owners live through reads.
/// Existing selector inputs for goals/gates/loss remain explicit declarations;
/// this wrapper checks only state/graph/CR-PRESERVE/current-owner correspondence.
/// It issues no proof, condition observation, authority or completion status.
pub struct RelationSelection<'a, 'state, 'input, 'view> {
    snapshot: &'a RelationGraphSnapshot<'state, 'view>,
    materialized_row: usize,
    materialized_owner: &'a structural::OwnedPreservation<'input, 'view>,
    selected: structural::CurrentSelection<'a, 'input, 'view>,
}

impl RelationSelection<'_, '_, '_, '_> {
    pub fn report_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&JointSelectionReport<'_, '_>, RelationMeasurementError> {
        let report = self.selected.report_current(resources)
            .map_err(RelationMeasurementError::Structural)?;
        let row = report.candidates.get(self.materialized_row)
            .ok_or(RelationMeasurementError::BindingMismatch)?;
        check_target(self.snapshot, report.pins, row, self.materialized_owner, resources)?;
        Ok(report)
    }
}

/// Connect the exact current snapshot to the existing fixed-plan selector.
/// `materialized_row` identifies the actual edge-result owner of this snapshot;
/// another graph's equal score or a detached report is not interchangeable.
#[allow(clippy::too_many_arguments)] // The existing selector's explicit bindings stay separate.
pub fn select_relation_current<'a, 'state, 'input, 'view>(
    snapshot: &'a RelationGraphSnapshot<'state, 'view>,
    materialized_row: usize,
    policy: &'a SelectionPolicy<'a>,
    rows: &'a [JointCandidate<'a>],
    owners: &'a [structural::PreservationRow<'a, 'input, 'view>],
    coverage: SearchCoverage,
    resources: &'view ResourceReservationView<'_>,
) -> Result<RelationSelection<'a, 'state, 'input, 'view>, RelationMeasurementError> {
    let row = rows.get(materialized_row).ok_or(RelationMeasurementError::BindingMismatch)?;
    let owner = owners.iter().find(|binding| binding.row == materialized_row)
        .ok_or(RelationMeasurementError::BindingMismatch)?.owner;
    check_target(snapshot, policy.pins, row, owner, resources)?;
    let selected = structural::select_current(policy, rows, owners, coverage, resources)
        .map_err(RelationMeasurementError::Structural)?;
    check_target(snapshot, policy.pins, row, owner, resources)?;
    Ok(RelationSelection { snapshot, materialized_row, materialized_owner: owner, selected })
}

fn check_target(
    snapshot: &RelationGraphSnapshot<'_, '_>,
    pins: SelectionPins,
    row: &JointCandidate<'_>,
    owner: &structural::OwnedPreservation<'_, '_>,
    resources: &ResourceReservationView<'_>,
) -> Result<(), RelationMeasurementError> {
    let graph = snapshot.graph_current(resources)?;
    let request = snapshot.request();
    let actual = owner.report_current(resources).map_err(RelationMeasurementError::Structural)?;
    if pins.source != request.context.source()
        || pins.scope != request.scope
        || row.context != request.context
        || actual.binding.context != request.context.interpretation_digest()
        || actual.binding.target != graph.digest()
        || actual.probe_set != request.scope.probes()
    {
        return Err(RelationMeasurementError::BindingMismatch);
    }
    Ok(())
}
