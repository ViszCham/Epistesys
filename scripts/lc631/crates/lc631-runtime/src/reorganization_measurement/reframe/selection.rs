//! Actual row owner comparisons, with no inferred goal/TL/permission.
use super::{claim, vec_cap, Cause, Error, ReframeRow};
use crate::joint_selection::{compare_rows, AxisKind, AxisValue, PairRelation};
use lc631_core::CandidateId;
use lc631_substrate::resource::ResourceReservationView;
fn common(
    a: &ReframeRow<'_, '_, '_, '_, '_, '_>,
    b: &ReframeRow<'_, '_, '_, '_, '_, '_>,
) -> Result<(), Error> {
    if !std::ptr::eq(a.owner.plan, b.owner.plan)
        || !std::ptr::eq(a.owner.baseline, b.owner.baseline)
    {
        return Err(Error::NoComparable(Cause::ForeignPlan));
    }
    Ok(())
}
fn borrow<'a>(v: &'a AxisValue<'_>) -> AxisValue<'a> {
    match v {
        AxisValue::Exact(x) => AxisValue::Exact(x),
        AxisValue::Bounded(x) => AxisValue::Bounded(x),
        AxisValue::Estimated(x) => AxisValue::Estimated(x),
        AxisValue::Unavailable(x) => AxisValue::Unavailable(*x),
    }
}
pub fn compare_current(
    a: &ReframeRow<'_, '_, '_, '_, '_, '_>,
    b: &ReframeRow<'_, '_, '_, '_, '_, '_>,
    r: &ResourceReservationView<'_>,
) -> Result<PairRelation, Error> {
    a.check(r)?;
    b.check(r)?;
    common(a, b)?;
    a.owner.plan.charge(1)?;
    let out = compare_rows(
        &[AxisKind::Reframe],
        &[borrow(a.value())],
        &[borrow(b.value())],
    )?;
    a.check(r)?;
    b.check(r)?;
    common(a, b)?;
    Ok(out)
}
pub struct CurrentDecision {
    frontier: Vec<CandidateId>,
}
impl CurrentDecision {
    pub fn frontier(&self) -> &[CandidateId] {
        &self.frontier
    }
    pub fn recommendation(&self) -> Option<CandidateId> {
        None
    }
}
pub fn select_current(
    rows: &[ReframeRow<'_, '_, '_, '_, '_, '_>],
    r: &ResourceReservationView<'_>,
) -> Result<CurrentDecision, Error> {
    if rows.is_empty() || rows.len() > 16 {
        return Err(Error::InvalidLimits);
    }
    let retained = claim(r, rows[0].owner.plan.limits.selector_ram)?;
    for (i, row) in rows.iter().enumerate() {
        row.check(r)?;
        common(&rows[0], row)?;
        if rows[..i]
            .iter()
            .any(|x| x.owner.candidate.data.id == row.owner.candidate.data.id)
        {
            return Err(Error::InvalidProbeSet);
        }
    }
    let mut frontier = vec_cap(rows.len())?;
    for (i, row) in rows.iter().enumerate() {
        let mut dominated = false;
        for (j, other) in rows.iter().enumerate() {
            if i != j && compare_current(other, row, r)? == PairRelation::LeftDominates {
                dominated = true;
            }
        }
        if !dominated {
            frontier.push(row.owner.candidate.data.id);
        }
    }
    for row in rows {
        row.check(r)?;
    }
    retained.check()?;
    Ok(CurrentDecision { frontier })
}
