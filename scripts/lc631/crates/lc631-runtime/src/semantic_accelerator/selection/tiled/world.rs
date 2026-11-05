//! Current actual world/finite-store and owned semantic-loss row correspondence.
//! Eligibility and source interpretation remain declared/unverified; no authority.
#![forbid(unsafe_code)]
use super::*;
use crate::finite_results::{FiniteResultId, FiniteResultStore};
use crate::formal_measurement::{Candidate, ProbePlan};
use crate::joint_selection::formal::{BothDirectionReduction, FormalAxisError, FormalSemanticAxis};
use crate::materialized_world_bridge::{
    observe_temporal_world_current, PreparedTemporalWorld, SourceMeaning, WorldBridgeError,
};
use lc631_core::context::ContextScope;
use lc631_solvers::temporal::Truth;
use lc631_world::materialized_worlds::{MaterializedKey, MaterializedWorlds};

/// Explicit mapping profile, not a universal natural-language goal oracle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TruthGoalMap {
    TrueOneFalseZeroV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TotalLossProfile {
    SingleSemanticAxisTotal,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclaredTemporalGoal {
    pub metric: Digest32,
    pub profile: Digest32,
    pub scope: ContextScope,
    pub map: TruthGoalMap,
    pub total: TotalLossProfile,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldWorkBudget {
    pub max_rows: usize,
    pub max_payload_bytes: usize,
    pub max_pairs: usize,
    pub max_comparison_bytes: usize,
}
pub struct WorldRow<'a, 'i, 'view, 'owners, 'policy> {
    pub key: MaterializedKey,
    pub prepared: &'a PreparedTemporalWorld<'i, 'view>,
    pub result: FiniteResultId,
    pub semantic: &'a FormalSemanticAxis<'owners, 'i, 'view, 'policy>,
    pub reduction: &'a BothDirectionReduction<'policy>,
    pub plan: &'a ProbePlan<'i>,
    pub candidate: &'a Candidate<'i>,
    pub expected_measurement_binding: Digest32,
}
/// Immutable actual backing remains borrowed through returned-result consumption.
/// This is not a checked constructor; every use rechecks all real owners.
pub struct WorldInputSet<'a, 'i, 'view, 'world, 'owners, 'policy> {
    pub worlds: &'a MaterializedWorlds<'world>,
    pub store: &'a FiniteResultStore<'i, 'view>,
    pub rows: &'a [WorldRow<'a, 'i, 'view, 'owners, 'policy>],
    pub goal: DeclaredTemporalGoal,
    pub budget: WorldWorkBudget,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldCoverageAtCheck {
    pub evaluated_rows: usize,
    pub duplicate_full_inputs: usize,
    pub unique_full_inputs: usize,
    /// Semantic distinctness is not derived from IDs, rowcount or equal/different Boolean outcomes.
    pub logical_distinct_unconfirmed: usize,
}
#[derive(Debug)]
pub enum WorldRunError {
    Shape,
    Binding,
    UnknownGoal,
    Work,
    World(WorldBridgeError),
    Formal(FormalAxisError),
    Numeric(ConsumerError),
    Run(TiledRunError),
}
trait RecheckWorldRows {
    fn recheck(
        &self,
        batch: &NumericBatch<'_, '_, '_, '_>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<WorldCoverageAtCheck, WorldRunError>;
}
impl RecheckWorldRows for WorldInputSet<'_, '_, '_, '_, '_, '_> {
    fn recheck(
        &self,
        batch: &NumericBatch<'_, '_, '_, '_>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<WorldCoverageAtCheck, WorldRunError> {
        current(&batch.claim, resources).map_err(WorldRunError::Numeric)?;
        let n = batch.rows.len();
        let limit = self.budget;
        if n == 0
            || n != self.rows.len()
            || n > limit.max_rows
            || limit.max_rows > 2048
            || limit.max_payload_bytes > 1024 * 1024
            || limit.max_pairs > 2048 * 2048
            || limit.max_comparison_bytes > 1024 * 1024 * 1024
            || n.checked_mul(n).is_none_or(|v| v > limit.max_pairs)
        {
            return Err(WorldRunError::Work);
        }
        if batch.policy.axes != [AxisKind::Goal(self.goal.metric), AxisKind::SemanticLoss]
            || batch.policy.pins.scope != self.goal.scope
            || batch.policy.pins.goal_profile != Some(self.goal.profile)
        {
            return Err(WorldRunError::Binding);
        }
        match (self.goal.map, self.goal.total) {
            (TruthGoalMap::TrueOneFalseZeroV1, TotalLossProfile::SingleSemanticAxisTotal) => {}
        }
        let (mut payload, mut compared, mut duplicates) = (0usize, 0usize, 0usize);
        for (index, (input, row)) in self.rows.iter().zip(batch.rows).enumerate() {
            let canonical = input.prepared.world_query();
            let bytes = canonical.query_bytes();
            payload = payload
                .checked_add(bytes.len())
                .ok_or(WorldRunError::Work)?;
            if payload > limit.max_payload_bytes {
                return Err(WorldRunError::Work);
            }
            let branch = self
                .worlds
                .get(input.key)
                .map_err(|e| WorldRunError::World(WorldBridgeError::World(e)))?;
            if branch.context().revision() != row.context
                || branch.context().scope() != batch.policy.pins.scope
                || row.canonical_key != bytes
                || row.pins != batch.policy.pins
                || row.context.source() != batch.policy.pins.source
            {
                return Err(WorldRunError::Binding);
            }
            // Construct this private view internally from the exact descriptor. A caller
            // cannot pair a checked wrapper with unrelated prepared bytes/scope.
            let observed = observe_temporal_world_current(
                self.worlds,
                input.key,
                input.prepared,
                self.store,
                input.result,
                batch.policy.pins.scope,
                resources,
            )
            .map_err(WorldRunError::World)?;
            let expected = match observed
                .report_current(resources)
                .map_err(WorldRunError::World)?
                .truth()
            {
                Truth::True => 1,
                Truth::False => 0,
                Truth::Unknown => return Err(WorldRunError::UnknownGoal),
            };
            let actual_goal = match row.axes.first() {
                Some(AxisValue::Exact(v)) if v.is_exact() => v,
                _ => return Err(WorldRunError::Binding),
            };
            if actual_goal.lower() != &Rational::from_integer(BigInt::from(expected)) {
                return Err(WorldRunError::Binding);
            }
            input
                .semantic
                .check_selection_row_current(
                    input.reduction,
                    batch.policy,
                    row,
                    1,
                    input.expected_measurement_binding,
                    input.plan,
                    input.candidate,
                    resources,
                )
                .map_err(WorldRunError::Formal)?;
            // Explicit one-active-TL-axis reduction: total must borrow that exact owner,
            // not a numerically equal copied endpoint or a fabricated zero.
            let same = match (&row.total_loss, row.axes.get(1)) {
                (AxisValue::Exact(total), Some(AxisValue::Exact(axis)))
                | (AxisValue::Bounded(total), Some(AxisValue::Bounded(axis))) => {
                    std::ptr::eq(*total, *axis)
                }
                _ => false,
            };
            if !same {
                return Err(WorldRunError::Binding);
            }
            let mut duplicate = false;
            for prior in &self.rows[..index] {
                let prior_bytes = prior.prepared.world_query().query_bytes();
                compared = compared
                    .checked_add(bytes.len().min(prior_bytes.len()))
                    .ok_or(WorldRunError::Work)?;
                if compared > limit.max_comparison_bytes {
                    return Err(WorldRunError::Work);
                }
                if prior.prepared.world_query() == canonical {
                    duplicate = true;
                    break;
                }
            }
            if duplicate {
                duplicates += 1;
            }
            observed
                .report_current(resources)
                .map_err(WorldRunError::World)?;
        }
        current(&batch.claim, resources).map_err(WorldRunError::Numeric)?;
        Ok(WorldCoverageAtCheck {
            evaluated_rows: n,
            duplicate_full_inputs: duplicates,
            unique_full_inputs: n - duplicates,
            logical_distinct_unconfirmed: n,
        })
    }
}
impl WorldInputSet<'_, '_, '_, '_, '_, '_> {
    pub fn check_rows_current(
        &self,
        batch: &NumericBatch<'_, '_, '_, '_>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<WorldCoverageAtCheck, WorldRunError> {
        self.recheck(batch, resources)
    }
}
/// The backing is retained by borrow and rechecked at every current projection.
/// Historical numeric inspection does not claim current world correspondence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EligibilityBasis {
    ReportedConditional,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldCorrespondence {
    NeedsEvidence,
}
pub struct WorldNumericProjection<'a> {
    frontier: &'a [usize],
    words: Option<&'a [u32]>,
}
impl WorldNumericProjection<'_> {
    pub fn numeric_frontier(&self) -> &[usize] {
        self.frontier
    }
    pub fn actual_ordered_gpu_words(&self) -> Option<&[u32]> {
        self.words
    }
    pub fn world_recommendation(&self) -> Option<usize> {
        None
    }
    pub fn correspondence(&self) -> WorldCorrespondence {
        WorldCorrespondence::NeedsEvidence
    }
}
pub struct WorldTiledOutcome<'a, 'v, 'r, 'bindings> {
    numeric: TiledOutcome<'a, 'v, 'r>,
    backing: &'bindings dyn RecheckWorldRows,
    token: Digest32,
    coverage: WorldCoverageAtCheck,
    post_check_error: Option<WorldRunError>,
}
impl WorldTiledOutcome<'_, '_, '_, '_> {
    pub fn historical_coverage(&self) -> WorldCoverageAtCheck {
        self.coverage
    }
    pub fn source_meaning(&self) -> SourceMeaning {
        SourceMeaning::NeedsEvidence
    }
    pub fn eligibility_basis(&self) -> EligibilityBasis {
        EligibilityBasis::ReportedConditional
    }
    pub fn historical_post_check_error(&self) -> Option<&WorldRunError> {
        self.post_check_error.as_ref()
    }
    pub fn historical_gpu_words(&self) -> Option<&[u32]> {
        match &self.numeric {
            TiledOutcome::ObservedCurrent { observation, .. }
            | TiledOutcome::ObservedRejected { observation, .. } => {
                Some(observation.historical_frontier_words())
            }
            TiledOutcome::FreshCpuUnsupported { .. } => None,
        }
    }
    pub fn project_current<'a>(
        &'a self,
        batch: &NumericBatch<'_, '_, '_, '_>,
        broker: &AcceleratorBroker<CubeClWgpuBackend>,
        resources: &ResourceReservationView<'_>,
    ) -> Result<WorldNumericProjection<'a>, WorldRunError> {
        if self.post_check_error.is_some() {
            return Err(WorldRunError::Binding);
        }
        if batch.token != self.token {
            return Err(WorldRunError::Binding);
        }
        self.backing.recheck(batch, resources)?;
        match &self.numeric {
            TiledOutcome::ObservedCurrent {
                selection,
                observation,
            } => {
                observation
                    .check_current(broker, resources)
                    .map_err(WorldRunError::Run)?;
                selection
                    .project_current(resources)
                    .map_err(WorldRunError::Numeric)?;
            }
            TiledOutcome::ObservedRejected { observation, .. } => {
                observation
                    .check_current(broker, resources)
                    .map_err(WorldRunError::Run)?;
            }
            TiledOutcome::FreshCpuUnsupported { selection, .. } => {
                selection
                    .project_current(resources)
                    .map_err(WorldRunError::Numeric)?;
            }
        }
        self.backing.recheck(batch, resources)?;
        match &self.numeric {
            TiledOutcome::ObservedCurrent {
                selection,
                observation,
            } => Ok(WorldNumericProjection {
                frontier: &selection
                    .project_current(resources)
                    .map_err(WorldRunError::Numeric)?
                    .frontier,
                words: Some(observation.historical_frontier_words()),
            }),
            TiledOutcome::FreshCpuUnsupported { selection, .. } => Ok(WorldNumericProjection {
                frontier: &selection
                    .project_current(resources)
                    .map_err(WorldRunError::Numeric)?
                    .frontier,
                words: None,
            }),
            TiledOutcome::ObservedRejected { .. } => Err(WorldRunError::Binding),
        }
    }
}
#[allow(
    clippy::too_many_arguments,
    reason = "Actual owner backing, numeric input, grid, broker, budget and current lease are separate boundaries"
)]
pub fn run_world_tiled<'a, 'v, 'r, 'bindings>(
    inputs: &'bindings WorldInputSet<'_, '_, '_, '_, '_, '_>,
    batch: &NumericBatch<'a, '_, 'v, '_>,
    axes: &[GridAxis],
    broker: &mut AcceleratorBroker<CubeClWgpuBackend>,
    budget: ScopedSemanticGpuBudget,
    resources: &'r ResourceReservationView<'_>,
) -> Result<WorldTiledOutcome<'a, 'v, 'r, 'bindings>, WorldRunError> {
    let before = inputs.recheck(batch, resources)?;
    let numeric =
        run_semantic_tiled(batch, axes, broker, budget, resources).map_err(WorldRunError::Run)?;
    // Preserve native observation and its claim if cancellation/invalidation
    // happens during/after actual work; later projection remains non-current.
    let (coverage, post_check_error) = match inputs.recheck(batch, resources) {
        Ok(v) => (v, None),
        Err(e) => (before, Some(e)),
    };
    Ok(WorldTiledOutcome {
        numeric,
        backing: inputs,
        token: batch.token,
        coverage,
        post_check_error,
    })
}
