//! Owned stage data retain predecessors without self-reference or report inputs.
use super::{
    claim, syntax::Term, vec_cap, Cause, CauseRecord, CheckedFrameMap, Error, EvalRule,
    FrameProfile, LowerRule, PerspectiveOutcome, ProbeId, QueryText, ReframePlan, Stage,
};
use lc631_core::CandidateId;
use lc631_substrate::resource::{ResourceClaim, ResourceReservationView};
#[derive(Clone)]
pub(super) struct Row {
    pub id: ProbeId,
    pub term: Option<Term>,
    pub selected: Option<usize>,
    pub lower: Option<LowerRule>,
    pub eval: Option<EvalRule>,
    pub outcome: PerspectiveOutcome,
    pub mapped: Option<Vec<u16>>,
    pub query_work: u64,
    pub map_work: u64,
}
pub(super) struct Data<'p, 'i, 's, 'v> {
    pub plan: &'p ReframePlan<'i, 's, 'v>,
    pub binding: &'p CheckedFrameMap<'p, 'i, 's, 'v>,
    pub texts: &'i [QueryText<'i>],
    pub id: CandidateId,
    pub rows: Vec<Row>,
    pub causes: Vec<CauseRecord>,
    claims: Vec<ResourceClaim<'v>>,
}
impl<'p, 'i, 's, 'v> Data<'p, 'i, 's, 'v> {
    pub fn new(
        plan: &'p ReframePlan<'i, 's, 'v>,
        binding: &'p CheckedFrameMap<'p, 'i, 's, 'v>,
        texts: &'i [QueryText<'i>],
        id: CandidateId,
        r: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        binding.check(r)?;
        if !std::ptr::eq(plan, binding.plan) {
            return Err(Error::NoComparable(Cause::ForeignPlan));
        }
        if id.0 == 0 || texts.len() != plan.input.perspectives.len() {
            return Err(Error::InvalidProbeSet);
        }
        if binding.baseline && !std::ptr::eq(texts, plan.input.baseline_terms) {
            return Err(Error::ForeignOwner);
        }
        for q in plan.input.perspectives {
            if texts.iter().filter(|t| t.query == q.id).count() != 1 {
                return Err(Error::InvalidProbeSet);
            }
        }
        let mut claims = vec_cap(4)?;
        claims.push(claim(r, plan.limits.retained_ram)?);
        let mut rows = vec_cap(plan.input.perspectives.len())?;
        for q in plan.input.perspectives {
            rows.push(Row {
                id: q.id,
                term: None,
                selected: None,
                lower: None,
                eval: None,
                outcome: PerspectiveOutcome::Unknown,
                mapped: None,
                query_work: 0,
                map_work: 0,
            });
        }
        Ok(Self {
            plan,
            binding,
            texts,
            id,
            rows,
            causes: vec_cap(64)?,
            claims,
        })
    }
    pub fn next_claim(&mut self, r: &'v ResourceReservationView<'_>) -> Result<(), Error> {
        self.check(r)?;
        if self.claims.len() == 4 {
            return Err(Error::InvalidLimits);
        }
        self.claims.push(claim(r, self.plan.limits.retained_ram)?);
        Ok(())
    }
    pub fn check(&self, r: &ResourceReservationView<'_>) -> Result<(), Error> {
        self.binding.check(r)?;
        for c in &self.claims {
            if !c.same_reservation(r) {
                return Err(Error::ForeignReservation);
            }
            c.check()?;
        }
        if self.rows.len() != self.plan.input.perspectives.len() {
            return Err(Error::InvalidProbeSet);
        }
        self.plan.charge(self.rows.len() as u64)?;
        for (row, q) in self.rows.iter().zip(self.plan.input.perspectives) {
            if row.id != q.id {
                return Err(Error::InvalidProbeSet);
            }
            if let Some(i) = row.selected {
                if i >= self.binding.frame.macros.len() {
                    return Err(Error::FramePayloadMismatch);
                }
            }
        }
        r.check()?;
        Ok(())
    }
    pub fn cause(&mut self, i: usize, stage: Stage, cause: Cause) -> Result<(), Error> {
        if self.causes.len() >= 64 {
            return Err(Error::PayloadLimit);
        }
        self.causes.push(CauseRecord {
            query: self.rows[i].id,
            stage,
            cause,
        });
        Ok(())
    }
    pub fn query_charge(&mut self, i: usize, n: u64, stage: Stage) -> Result<bool, Error> {
        let remaining = self
            .plan
            .limits
            .max_query_work
            .checked_sub(self.rows[i].query_work)
            .ok_or(Error::WorkBudget)?;
        let spent = n.min(remaining);
        self.plan.charge(spent)?;
        self.rows[i].query_work += spent;
        if spent < n {
            self.rows[i].outcome = PerspectiveOutcome::Unknown;
            self.cause(i, stage, Cause::WorkExhausted)?;
            return Ok(false);
        }
        Ok(true)
    }
    pub fn map_charge(&mut self, i: usize) -> Result<bool, Error> {
        if self.rows[i].map_work == self.plan.limits.max_map_work {
            self.rows[i].outcome = PerspectiveOutcome::Unknown;
            self.cause(i, Stage::PaidMap, Cause::WorkExhausted)?;
            return Ok(false);
        }
        self.plan.charge(1)?;
        self.rows[i].map_work += 1;
        Ok(true)
    }
    pub fn profile(&self) -> FrameProfile {
        self.binding.frame.profile
    }
}
