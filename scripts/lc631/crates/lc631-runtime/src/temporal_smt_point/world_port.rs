//! 日本語: provided point実例だけを再検査して既存world numeric productionを呼ぶ。
//! English: Recheck provided point instances before existing world numeric production.
//! Never promotes individual instances to world equivalence, verified eligibility,
//! complete-world coverage, recommendation, source truth or execution authority.
#![forbid(unsafe_code)]
use super::*;
use crate::{
    materialized_world_bridge::observe_temporal_world_current,
    semantic_accelerator::selection::{
        tiled::world::{
            run_world_tiled, EligibilityBasis, WorldCorrespondence, WorldInputSet,
            WorldNumericProjection, WorldRunError, WorldTiledOutcome,
        },
        NumericBatch,
    },
};
use lc631_accelerator::{
    semantic_pareto::{GridAxis, ScopedSemanticGpuBudget},
    AcceleratorBroker, CubeClWgpuBackend,
};

const MAX_POINTS: usize = 2048;
const PORT_RAM: u64 = 64 * 1024;

/// This row number binds a descriptor, not semantic identity or ranking.
pub struct PointWorldRow<'owner, 'input, 'view> {
    pub row: usize,
    pub owner: &'owner OwnedPointCorrespondence<'owner, 'input, 'view>,
}
/// Inputs are declarative until each provided row is rechecked. No caller
/// Boolean/digest can stand in for the private native/model/lease owner.
pub struct PointWorldInputs<'links, 'a, 'input, 'view, 'world, 'owners, 'policy> {
    pub inputs: &'links WorldInputSet<'a, 'input, 'view, 'world, 'owners, 'policy>,
    pub points: &'links [PointWorldRow<'a, 'input, 'view>],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointPortScope {
    ProvidedGivenInstancesOnly,
}
#[derive(Debug)]
pub enum PointPortError {
    Shape,
    DuplicateRow,
    Resource(ResourceError),
    Point(PointError),
    World(WorldRunError),
    NonCurrentAfterWork,
}

trait RecheckPoints {
    fn recheck(&self, resources: &ResourceReservationView<'_>) -> Result<usize, PointPortError>;
}
impl RecheckPoints for PointWorldInputs<'_, '_, '_, '_, '_, '_, '_> {
    fn recheck(&self, resources: &ResourceReservationView<'_>) -> Result<usize, PointPortError> {
        resources.check().map_err(PointPortError::Resource)?;
        let n = self.inputs.rows.len();
        if n == 0 || n > MAX_POINTS || self.points.len() != n {
            return Err(PointPortError::Shape);
        }
        // Fixed 256-byte stack set; no unreserved allocation per descriptor.
        let mut seen = [0_u64; MAX_POINTS / 64];
        for point in self.points {
            let row = self
                .inputs
                .rows
                .get(point.row)
                .ok_or(PointPortError::Shape)?;
            let word = point.row / 64;
            let bit = 1_u64 << (point.row % 64);
            if seen[word] & bit != 0 {
                return Err(PointPortError::DuplicateRow);
            }
            seen[word] |= bit;
            let observed = observe_temporal_world_current(
                self.inputs.worlds,
                row.key,
                row.prepared,
                self.inputs.store,
                row.result,
                self.inputs.goal.scope,
                resources,
            )
            .map_err(|error| PointPortError::Point(PointError::World(error)))?;
            point
                .owner
                .check_world_current(&observed, row.plan, row.candidate, resources)
                .map_err(PointPortError::Point)?;
        }
        resources.check().map_err(PointPortError::Resource)?;
        // Count successful reference-row checks, not distinct native results,
        // unique worlds or coverage of anything outside this provided list.
        Ok(n)
    }
}

pub struct PointWorldProjection<'a> {
    numeric: WorldNumericProjection<'a>,
    checked: usize,
}
impl PointWorldProjection<'_> {
    pub fn numeric_frontier(&self) -> &[usize] {
        self.numeric.numeric_frontier()
    }
    pub fn actual_ordered_gpu_words(&self) -> Option<&[u32]> {
        self.numeric.actual_ordered_gpu_words()
    }
    /// Number of rechecked provided reference rows only; aliases are not a
    /// semantic-distinctness claim and do not become new native observations.
    pub fn checked_point_instances(&self) -> usize {
        self.checked
    }
    pub fn point_scope(&self) -> PointPortScope {
        PointPortScope::ProvidedGivenInstancesOnly
    }
    pub fn world_recommendation(&self) -> Option<usize> {
        None
    }
    pub fn correspondence(&self) -> WorldCorrespondence {
        WorldCorrespondence::NeedsEvidence
    }
    pub fn eligibility_basis(&self) -> EligibilityBasis {
        EligibilityBasis::ReportedConditional
    }
}

/// Actual numeric observation and its backing stay retained, including when a
/// post-work point check fails. No public constructor/Clone/Deref/Deserialize.
/// Current getters check at call time; no atomic revocation fence is promised.
pub struct PointWorldTiledOutcome<'a, 'view, 'run, 'bindings> {
    numeric: WorldTiledOutcome<'a, 'view, 'run, 'bindings>,
    points: &'bindings dyn RecheckPoints,
    checked: usize,
    post_point_error: Option<PointPortError>,
    retained: ResourceClaim<'run>,
}
impl PointWorldTiledOutcome<'_, '_, '_, '_> {
    pub fn historical_checked_point_instances(&self) -> usize {
        self.checked
    }
    pub fn historical_gpu_words(&self) -> Option<&[u32]> {
        self.numeric.historical_gpu_words()
    }
    pub fn historical_point_check_error(&self) -> Option<&PointPortError> {
        self.post_point_error.as_ref()
    }
    pub fn source_meaning(&self) -> SourceMeaning {
        SourceMeaning::NeedsEvidence
    }
    pub fn project_current<'a>(
        &'a self,
        batch: &NumericBatch<'_, '_, '_, '_>,
        broker: &AcceleratorBroker<CubeClWgpuBackend>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<PointWorldProjection<'a>, PointPortError> {
        resources.check().map_err(PointPortError::Resource)?;
        self.retained.check().map_err(PointPortError::Resource)?;
        if !self.retained.same_reservation(resources) {
            return Err(PointPortError::Resource(ResourceError::ForeignOwner));
        }
        if self.post_point_error.is_some() {
            return Err(PointPortError::NonCurrentAfterWork);
        }
        self.points.recheck(resources)?;
        let numeric = self
            .numeric
            .project_current(batch, broker, resources)
            .map_err(PointPortError::World)?;
        let checked = self.points.recheck(resources)?;
        self.retained.check().map_err(PointPortError::Resource)?;
        Ok(PointWorldProjection { numeric, checked })
    }
}

/// Opt-in production connection only. Existing world APIs/default None remain
/// unchanged. Missing/duplicate/foreign point descriptors fail before dispatch.
/// A post-work failure retains the actual numeric owner/history instead of
/// pretending no work happened or fabricating fresh current correspondence.
#[allow(
    clippy::too_many_arguments,
    reason = "Retained point backing, numeric batch, grid, native broker, budget and current lease are distinct boundaries"
)]
pub fn run_point_world_tiled<'a, 'view, 'run, 'bindings>(
    inputs: &'bindings PointWorldInputs<'_, '_, '_, '_, '_, '_, '_>,
    batch: &NumericBatch<'a, '_, 'view, '_>,
    axes: &[GridAxis],
    broker: &mut AcceleratorBroker<CubeClWgpuBackend>,
    budget: ScopedSemanticGpuBudget,
    resources: &'run ResourceReservationView<'_>,
) -> Result<PointWorldTiledOutcome<'a, 'view, 'run, 'bindings>, PointPortError> {
    resources.check().map_err(PointPortError::Resource)?;
    let retained = claim(resources, PORT_RAM).map_err(PointPortError::Point)?;
    let before = inputs.recheck(resources)?;
    let numeric = run_world_tiled(inputs.inputs, batch, axes, broker, budget, resources)
        .map_err(PointPortError::World)?;
    let (checked, post_point_error) = match inputs.recheck(resources) {
        Ok(count) => (count, None),
        Err(error) => (before, Some(error)),
    };
    Ok(PointWorldTiledOutcome {
        numeric,
        points: inputs,
        checked,
        post_point_error,
        retained,
    })
}
