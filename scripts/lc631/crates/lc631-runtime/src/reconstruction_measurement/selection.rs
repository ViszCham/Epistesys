//! Actual owned reconstruction -> existing reported joint selector.
//! The fixed goal1/hard feasibility are DECLARED finite hypotheses, not observed
//! NLP truth, independently validated semantics, proof, permission or authority.
use super::{claim, hash, Error, OwnedReconstruction, ProtectedFeatureSet, RationalInterval};
use crate::joint_selection::*;
use lc631_core::{
    context::{ContextRevision, ContextScope},
    CandidateId,
};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceReservationView},
    Digest32,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionGoal {
    DeclaredHypothesisSatisfied,
}
/// A past operational selection snapshot; only OwnedSelection::current checks
/// present eligibility. All semantic/input/shared-dependency labels are retained
/// by the actual borrowed reports, which must outlive this owner.
pub struct SelectionSnapshot {
    recommendation: Option<usize>,
    dispositions: [Disposition; 2],
    frontier: [bool; 2],
    state: RecommendationState,
    goal: SelectionGoal,
}
impl SelectionSnapshot {
    pub fn recommendation(&self) -> Option<usize> {
        self.recommendation
    }
    pub fn dispositions(&self) -> &[Disposition; 2] {
        &self.dispositions
    }
    pub fn frontier(&self) -> [bool; 2] {
        self.frontier
    }
    pub fn recommendation_state(&self) -> RecommendationState {
        self.state
    }
    pub fn goal(&self) -> SelectionGoal {
        self.goal
    }
}
pub struct OwnedSelection<'owner, 'plan, 'view, 'target, 'claim> {
    snapshot: SelectionSnapshot,
    owners: [&'owner OwnedReconstruction<'plan, 'view>; 2],
    plan: &'plan ProtectedFeatureSet<'view>,
    targets: [&'target [u8]; 2],
    context: ContextRevision,
    held: ResourceClaim<'claim>,
}
impl OwnedSelection<'_, '_, '_, '_, '_> {
    /// Explicit historical snapshot. Never reactivates old current eligibility.
    pub fn historical(&self) -> &SelectionSnapshot {
        &self.snapshot
    }
    pub fn current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&SelectionSnapshot, Error> {
        self.held.check().map_err(Error::Resource)?;
        if !self.held.same_reservation(resources) {
            return Err(Error::Binding);
        }
        for (owner, target) in self.owners.iter().zip(self.targets) {
            owner.check(self.plan, target, self.context, resources)?;
        }
        Ok(&self.snapshot)
    }
}

/// Fixed two-row declared finite profile. No loss/scalar/status argument is
/// accepted: both axes are freshly projected from actual private producer
/// reports. Unavailable axes require evidence; cancelled/foreign views fail.
pub fn select_current<'owner, 'plan, 'view, 'target, 'claim>(
    plan: &'plan ProtectedFeatureSet<'view>,
    owners: [&'owner OwnedReconstruction<'plan, 'view>; 2],
    targets: [&'target [u8]; 2],
    context: ContextRevision,
    goal: SelectionGoal,
    resources: &'claim ResourceReservationView<'_>,
) -> Result<OwnedSelection<'owner, 'plan, 'view, 'target, 'claim>, Error> {
    for (owner, target) in owners.iter().zip(targets) {
        owner.check(plan, target, context, resources)?;
    }
    let scratch = claim(resources, 256 * 1024, false)?;
    let held = claim(resources, 16 * 1024, true)?;
    let goal_value = match goal {
        SelectionGoal::DeclaredHypothesisSatisfied => {
            RationalInterval::exact_ratio(1, 1).map_err(|_| Error::Arithmetic)?
        }
    };
    let pins = pins(plan, owners);
    let kinds = [
        AxisKind::Goal(pins.scope.goal()),
        AxisKind::ReconstructionLoss,
    ];
    let left = [
        AxisValue::Exact(&goal_value),
        owners[0].axis_current(plan, targets[0], context, resources)?,
    ];
    let right = [
        AxisValue::Exact(&goal_value),
        owners[1].axis_current(plan, targets[1], context, resources)?,
    ];
    let axes = [&left[..], &right[..]];
    let row = |i: usize| -> Result<JointCandidate<'_>, Error> {
        Ok(JointCandidate {
            id: CandidateId(i as u64 + 1),
            context,
            pins,
            hard_gate: if axes[i][1].interval().is_some() {
                ReportedGate::Pass
            } else {
                ReportedGate::NeedsEvidence
            },
            map_back: ReportedGate::Pass,
            axes: axes[i],
            total_loss: owners[i].axis_current(plan, targets[i], context, resources)?,
            canonical_key: targets[i],
            measurement_binding: owners[i].report.result,
        })
    };
    let rows = [row(0)?, row(1)?];
    let policy = SelectionPolicy {
        pins,
        axes: &kinds,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 2,
        max_canonical_bytes: 2 * 65_536,
    };
    let selected = select_joint_reported(&policy, &rows, SearchCoverage::EvaluatedSet)
        .map_err(Error::Selection)?;
    let snapshot = SelectionSnapshot {
        recommendation: selected.recommendation,
        dispositions: [selected.dispositions[0], selected.dispositions[1]],
        frontier: [
            selected.frontier.contains(&0),
            selected.frontier.contains(&1),
        ],
        state: selected.recommendation_state,
        goal,
    };
    scratch.check().map_err(Error::Resource)?;
    held.check().map_err(Error::Resource)?;
    let owned = OwnedSelection {
        snapshot,
        owners,
        plan,
        targets,
        context,
        held,
    };
    owned.current(resources)?;
    Ok(owned)
}
fn pins(
    plan: &ProtectedFeatureSet<'_>,
    owners: [&OwnedReconstruction<'_, '_>; 2],
) -> SelectionPins {
    // Lengths fixed: no concatenation ambiguity or opaque arrival-ID ranking.
    let mut evidence = [0_u8; 64];
    evidence[..32].copy_from_slice(owners[0].report.result.as_bytes());
    evidence[32..].copy_from_slice(owners[1].report.result.as_bytes());
    SelectionPins {
        source: plan.context.source(),
        scope: ContextScope::declared(
            Digest32::from_bytes(plan.scope.id.0),
            Digest32::from_bytes(plan.scope.root_goal.0),
            plan.query,
            hash(super::decoder::COMMON_RUNTIME.as_bytes()),
        ),
        baseline: Digest32::from_bytes(plan.scope.baseline.0),
        hard_invariants: hash(b"tlcx18-finite-declared-hypotheses-no-external-hard-claims-v1"),
        policy: Digest32::from_bytes(plan.scope.policy.0),
        goal_profile: Some(Digest32::from_bytes(plan.scope.root_goal.0)),
        evidence_revision: hash(&evidence),
    }
}
