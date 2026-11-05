//! Compare only rows from ONE actual fixed plan and baseline, not matching pins.
use super::{claim, vector, Error, RebindRow};
use crate::joint_selection::{
    compare_rows, AxisKind, AxisValue, PairRelation, RecommendationState,
};
use lc631_core::CandidateId;
use lc631_substrate::resource::ResourceReservationView;

fn same_plan(
    a: &RebindRow<'_, '_, '_, '_, '_, '_>,
    b: &RebindRow<'_, '_, '_, '_, '_, '_>,
) -> Result<(), Error> {
    let ap = a.owner.queries.program.plan;
    let bp = b.owner.queries.program.plan;
    // Actual root/baseline/suite identity is retained even when content/pins match.
    if !std::ptr::eq(ap, bp)
        || !std::ptr::eq(ap.baseline, bp.baseline)
        || !std::ptr::eq(ap.probes, bp.probes)
        || ap.pins != bp.pins
    {
        return Err(Error::ForeignPlan);
    }
    Ok(())
}
pub fn compare_current(
    a: &RebindRow<'_, '_, '_, '_, '_, '_>,
    b: &RebindRow<'_, '_, '_, '_, '_, '_>,
    resources: &ResourceReservationView<'_>,
) -> Result<PairRelation, Error> {
    a.check(resources)?;
    b.check(resources)?;
    same_plan(a, b)?;
    a.owner.queries.program.plan.charge(1)?;
    let result = compare_rows(
        &[AxisKind::Rebind],
        &[borrow(a.value())],
        &[borrow(b.value())],
    )?;
    a.check(resources)?;
    b.check(resources)?;
    same_plan(a, b)?;
    Ok(result)
}
fn borrow<'a>(v: &'a AxisValue<'_>) -> AxisValue<'a> {
    match v {
        AxisValue::Exact(r) => AxisValue::Exact(r),
        AxisValue::Bounded(r) => AxisValue::Bounded(r),
        AxisValue::Estimated(r) => AxisValue::Estimated(r),
        AxisValue::Unavailable(r) => AxisValue::Unavailable(*r),
    }
}
/// Bounded historical decision summary, not an authority/current-owner token.
pub struct CurrentDecision {
    frontier: Vec<CandidateId>,
    state: RecommendationState,
}
impl CurrentDecision {
    pub fn frontier(&self) -> &[CandidateId] {
        &self.frontier
    }
    pub fn recommendation(&self) -> Option<CandidateId> {
        None
    }
    pub fn recommendation_state(&self) -> RecommendationState {
        self.state
    }
}
pub fn select_current(
    rows: &[RebindRow<'_, '_, '_, '_, '_, '_>],
    resources: &ResourceReservationView<'_>,
) -> Result<CurrentDecision, Error> {
    if rows.is_empty() || rows.len() > 16 {
        return Err(Error::Selection(
            crate::joint_selection::SelectionError::InvalidLimits,
        ));
    }
    let plan = rows[0].owner.queries.program.plan;
    let retained = claim(resources, plan.limits.selector_ram)?;
    for (i, row) in rows.iter().enumerate() {
        row.check(resources)?;
        same_plan(&rows[0], row)?;
        if rows[..i]
            .iter()
            .any(|r| r.owner.queries.program.id == row.owner.queries.program.id)
        {
            return Err(Error::Selection(
                crate::joint_selection::SelectionError::DuplicateCandidateId,
            ));
        }
    }
    let mut frontier = vector(rows.len())?;
    for (i, row) in rows.iter().enumerate() {
        let mut dominated = false;
        for (j, other) in rows.iter().enumerate() {
            if i != j && compare_current(other, row, resources)? == PairRelation::LeftDominates {
                dominated = true;
            }
        }
        if !dominated {
            frontier.push(row.owner.queries.program.id);
        }
    }
    for row in rows {
        row.check(resources)?;
        same_plan(&rows[0], row)?;
    }
    retained.check()?;
    Ok(CurrentDecision {
        frontier,
        state: if plan.pins.goal_profile.is_none() {
            RecommendationState::NoGoalProfile
        } else {
            RecommendationState::NoGoalObservation
        },
    })
}
