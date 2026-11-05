//! Actual direct membership and checked macro evaluators, never root-BFS answers.
use super::{
    pipeline::Data, Cause, Error, EvalRule, LowerRule, LoweredFrameProgram, PerspectiveOutcome,
    Stage,
};
use lc631_substrate::resource::ResourceReservationView;
pub struct EvaluatedFrameProgram<'p, 'i, 's, 'v> {
    pub(super) data: Data<'p, 'i, 's, 'v>,
}
impl<'p, 'i, 's, 'v> EvaluatedFrameProgram<'p, 'i, 's, 'v> {
    pub fn evaluate_current(
        lowered: LoweredFrameProgram<'p, 'i, 's, 'v>,
        r: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let mut data = lowered.data;
        data.next_claim(r)?;
        for i in 0..data.rows.len() {
            match data.rows[i].lower {
                Some(LowerRule::DirectEdgeReferenceV1) => eval_direct_v1(&mut data, i)?,
                Some(LowerRule::CheckedPathReferenceV1) => eval_macro_v1(&mut data, i)?,
                None => {}
            }
        }
        data.check(r)?;
        Ok(Self { data })
    }
}
fn eval_direct_v1(data: &mut Data<'_, '_, '_, '_>, i: usize) -> Result<(), Error> {
    data.rows[i].eval = Some(EvalRule::DirectEdgeV1);
    let term = data.rows[i].term.ok_or(Error::FramePayloadMismatch)?;
    for j in 0..data.plan.input.base.edges().len() {
        if !data.query_charge(i, 1, Stage::FrameEvaluate)? {
            return Ok(());
        }
        if data.plan.input.base.edges()[j] == term.edge {
            data.rows[i].outcome = PerspectiveOutcome::KnownPass;
            return Ok(());
        }
    }
    data.rows[i].outcome = PerspectiveOutcome::KnownFailure;
    data.cause(i, Stage::FrameEvaluate, Cause::NoDirectBaseEdge)
}
fn eval_macro_v1(data: &mut Data<'_, '_, '_, '_>, i: usize) -> Result<(), Error> {
    data.rows[i].eval = Some(EvalRule::CheckedPathMacroV1);
    if !data.query_charge(i, 1, Stage::FrameEvaluate)? {
        return Ok(());
    }
    let Some(index) = data.rows[i].selected else {
        data.rows[i].outcome = PerspectiveOutcome::KnownFailure;
        return data.cause(i, Stage::FrameEvaluate, Cause::NoMacroEntry);
    };
    let entry = &data.binding.frame.macros[index];
    let path = entry.path.ok_or(Error::MissingPathOwner)?;
    if path.nodes().first().map(|n| u16::from(n.0)) != Some(entry.edge.0 .0)
        || path.nodes().last().map(|n| u16::from(n.0)) != Some(entry.edge.1 .0)
    {
        return Err(Error::MapBackEndpoints);
    }
    data.rows[i].outcome = PerspectiveOutcome::KnownPass;
    Ok(())
}
