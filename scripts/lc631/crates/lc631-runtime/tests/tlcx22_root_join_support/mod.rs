//! Conditional REPORTED goal/TL/gate fixture, not verified goal observations.
//! Actual gain intervals are supplied only as current borrows from real owners.
use lc631_core::measurement_numeric::RationalInterval;
use lc631_runtime::{explanation_measurement::CandidateBinding, joint_selection::*};
use lc631_substrate::Digest32;

pub struct ConditionalReportedReferences {
    pub goal: RationalInterval,
    pub total_loss: RationalInterval,
}
impl ConditionalReportedReferences {
    pub fn given_goal_one_tl_zero() -> Self {
        Self {
            goal: RationalInterval::exact_ratio(1, 1).unwrap(),
            total_loss: RationalInterval::exact_ratio(0, 1).unwrap(),
        }
    }
}
pub fn policy(pins: SelectionPins, axes: &[AxisKind]) -> SelectionPolicy<'_> {
    SelectionPolicy {
        pins,
        axes,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: 64,
        max_canonical_bytes: 4096,
    }
}
pub fn reported_row<'a>(
    binding: CandidateBinding,
    pins: SelectionPins,
    axes: &'a [AxisValue<'a>],
    reported_total_loss: &'a RationalInterval,
    canonical_key: &'a [u8],
    actual_measurement_binding: Digest32,
) -> JointCandidate<'a> {
    JointCandidate {
        id: binding.id,
        context: binding.context,
        pins,
        hard_gate: ReportedGate::Pass,
        map_back: ReportedGate::Pass,
        axes,
        total_loss: AxisValue::Exact(reported_total_loss),
        canonical_key,
        measurement_binding: actual_measurement_binding,
    }
}
