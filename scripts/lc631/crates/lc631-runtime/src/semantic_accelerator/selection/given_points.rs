//! 日本語: 実point観測とCPU finalから、提供Given instance内の条件付き代表を返す。
//! English: Consume actual point observations and CPU final for a conditional
//! representative among provided Given instances. Reported eligibility is not
//! verified eligibility; this never changes the world-recommendation boundary.
#![forbid(unsafe_code)]
use super::{tiled::world::EligibilityBasis, *};
use crate::temporal_smt_point::world_port::{
    run_point_world_tiled, PointPortError, PointWorldInputs, PointWorldProjection,
    PointWorldTiledOutcome,
};
use lc631_accelerator::{
    semantic_pareto::{GridAxis, ScopedSemanticGpuBudget},
    AcceleratorBroker, CubeClWgpuBackend,
};

/// Additive consumer bound; the existing tiled 2048-row profile is unchanged.
/// The extra fresh CPU-final pass is bounded to 512 rows and the existing work cap.
pub const MAX_GIVEN_POINT_SELECTION_ROWS: usize = 512;

#[derive(Debug)]
pub enum GivenPointSelectionError {
    Shape,
    Point(PointPortError),
    Cpu(ConsumerError),
    FrontierMismatch,
}

/// Native history and its claims survive a later CPU rejection. No detached
/// success constructor, Clone, Deserialize, execution permit or restored state.
pub struct OwnedGivenPointSelection<'a, 'view, 'run, 'bindings> {
    points: PointWorldTiledOutcome<'a, 'view, 'run, 'bindings>,
    cpu: Result<CurrentSelection<'a, 'view, 'run>, ConsumerError>,
}
pub struct GivenPointSelectionProjection<'a> {
    points: PointWorldProjection<'a>,
    recommendation: Option<usize>,
}
impl GivenPointSelectionProjection<'_> {
    /// Operational choice under the declared goal/selection policy, ONLY among
    /// provided Given point instances with caller-reported gates. This is neither
    /// a whole-world recommendation nor semantic equivalence or authority.
    pub fn provided_point_recommendation(&self) -> Option<usize> {
        self.recommendation
    }
    pub fn eligibility_basis(&self) -> EligibilityBasis {
        EligibilityBasis::ReportedConditional
    }
    pub fn world_recommendation(&self) -> Option<usize> {
        None
    }
    pub fn checked_point_instances(&self) -> usize {
        self.points.checked_point_instances()
    }
    pub fn numeric_frontier(&self) -> &[usize] {
        self.points.numeric_frontier()
    }
    pub fn actual_ordered_gpu_words(&self) -> Option<&[u32]> {
        self.points.actual_ordered_gpu_words()
    }
}
impl OwnedGivenPointSelection<'_, '_, '_, '_> {
    pub fn historical_gpu_words(&self) -> Option<&[u32]> {
        self.points.historical_gpu_words()
    }
    pub fn historical_cpu_rejection(&self) -> Option<&ConsumerError> {
        self.cpu.as_ref().err()
    }
    /// Checks at call time, not an atomic revocation/publication fence. A copied
    /// recommendation is a historical decision after this check has returned.
    pub fn project_current<'a>(
        &'a self,
        batch: &NumericBatch<'_, '_, '_, '_>,
        broker: &AcceleratorBroker<CubeClWgpuBackend>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<GivenPointSelectionProjection<'a>, GivenPointSelectionError> {
        let points = self
            .points
            .project_current(batch, broker, resources)
            .map_err(GivenPointSelectionError::Point)?;
        let selection = self
            .cpu
            .as_ref()
            .map_err(|error| GivenPointSelectionError::Cpu(error.clone()))?;
        let report = selection
            .project_current(resources)
            .map_err(GivenPointSelectionError::Cpu)?;
        if points.numeric_frontier() != report.frontier.as_slice() {
            return Err(GivenPointSelectionError::FrontierMismatch);
        }
        let recommendation = report.recommendation;
        // Retain every frontier member and do not manufacture a winner for a
        // missing goal, ambiguous policy or other NoRecommendation condition.
        selection
            .project_current(resources)
            .map_err(GivenPointSelectionError::Cpu)?;
        Ok(GivenPointSelectionProjection {
            points,
            recommendation,
        })
    }
}

/// Same actual point/world/native owner path, followed by a fresh exact CPU
/// selection over the same immutable rational batch. A second CPU computation is
/// a consumer check, not independent corroboration or a performance improvement.
/// Unsupported exact layout uses the existing explicitly fresh CPU-only path.
#[allow(
    clippy::too_many_arguments,
    reason = "Actual point backing, batch, grid, broker, budget and lease are distinct boundaries"
)]
pub fn select_given_points_current<'a, 'view, 'run, 'bindings>(
    inputs: &'bindings PointWorldInputs<'_, '_, '_, '_, '_, '_, '_>,
    batch: &NumericBatch<'a, '_, 'view, '_>,
    axes: &[GridAxis],
    broker: &mut AcceleratorBroker<CubeClWgpuBackend>,
    budget: ScopedSemanticGpuBudget,
    resources: &'run ResourceReservationView<'_>,
) -> Result<OwnedGivenPointSelection<'a, 'view, 'run, 'bindings>, GivenPointSelectionError> {
    if batch.rows.is_empty() || batch.rows.len() > MAX_GIVEN_POINT_SELECTION_ROWS {
        return Err(GivenPointSelectionError::Shape);
    }
    let points = run_point_world_tiled(inputs, batch, axes, broker, budget, resources)
        .map_err(GivenPointSelectionError::Point)?;
    // Keep points inline even if this post-effect allocation/current check fails.
    // The result claim separately accounts the additional CPU report's lifetime.
    let cpu = finish_selection(batch, None, resources);
    Ok(OwnedGivenPointSelection { points, cpu })
}
