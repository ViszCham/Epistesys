//! Paid map consumes that actual evaluator output and retained selected path.
use super::{
    pipeline::Data, vec_cap, Cause, Error, EvalRule, EvaluatedFrameProgram, FrameProfile,
    LowerRule, ParseRule, PerspectiveOutcome, ProbeId, Stage,
};
use lc631_substrate::resource::ResourceReservationView;
pub struct PaidFrameMap<'p, 'i, 's, 'v> {
    pub(super) data: Data<'p, 'i, 's, 'v>,
}
pub struct FrameStageTrace {
    parse: ParseRule,
    lower: LowerRule,
    eval: EvalRule,
}
impl FrameStageTrace {
    pub fn parse_rule(&self) -> ParseRule {
        self.parse
    }
    pub fn lower_rule(&self) -> LowerRule {
        self.lower
    }
    pub fn eval_rule(&self) -> EvalRule {
        self.eval
    }
}
impl<'p, 'i, 's, 'v> PaidFrameMap<'p, 'i, 's, 'v> {
    pub fn execute_current(
        evaluated: EvaluatedFrameProgram<'p, 'i, 's, 'v>,
        r: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let mut data = evaluated.data;
        data.next_claim(r)?;
        for i in 0..data.rows.len() {
            if data.rows[i].outcome != PerspectiveOutcome::KnownPass {
                continue;
            }
            let term = data.rows[i].term.ok_or(Error::FramePayloadMismatch)?;
            let mut nodes = vec_cap(16)?;
            match data.rows[i].eval.ok_or(Error::FramePayloadMismatch)? {
                EvalRule::DirectEdgeV1 => {
                    nodes.push(term.edge.0 .0);
                    nodes.push(term.edge.1 .0);
                }
                EvalRule::CheckedPathMacroV1 => {
                    let index = data.rows[i].selected.ok_or(Error::FramePayloadMismatch)?;
                    let path = data.binding.frame.macros[index]
                        .path
                        .ok_or(Error::MissingPathOwner)?;
                    for n in path.nodes() {
                        nodes.push(u16::from(n.0));
                    }
                }
            }
            if nodes.first() != Some(&term.edge.0 .0) || nodes.last() != Some(&term.edge.1 .0) {
                return Err(Error::MapBackEndpoints);
            }
            let mut complete = true;
            for pair in nodes.windows(2) {
                let mut member = false;
                for j in 0..data.plan.input.base.edges().len() {
                    if !data.map_charge(i)? {
                        complete = false;
                        break;
                    }
                    let edge = data.plan.input.base.edges()[j];
                    if edge.0 .0 == pair[0] && edge.1 .0 == pair[1] {
                        member = true;
                        break;
                    }
                }
                if !complete {
                    break;
                }
                if !member {
                    data.rows[i].outcome = PerspectiveOutcome::KnownFailure;
                    data.cause(i, Stage::RootOracle, Cause::InvalidDerivation)?;
                    complete = false;
                    break;
                }
            }
            if complete {
                data.rows[i].mapped = Some(nodes);
            }
        }
        data.check(r)?;
        Ok(Self { data })
    }
    pub fn profile(&self) -> FrameProfile {
        self.data.profile()
    }
    pub fn mapping_work_spent(&self) -> u64 {
        self.data.rows.iter().map(|r| r.map_work).sum()
    }
    pub fn trace_for(&self, id: ProbeId) -> Option<FrameStageTrace> {
        let r = self.data.rows.iter().find(|r| r.id == id)?;
        Some(FrameStageTrace {
            parse: r.term?.rule,
            lower: r.lower?,
            eval: r.eval?,
        })
    }
    pub(super) fn check(&self, r: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.data.check(r)
    }
}
