//! Bounded common mechanics, not an oracle-independence or global work token.
use super::*;
use lc631_substrate::resource::{ResourceAmount, ResourceError, ResourceKind, ResourceRequest};
use std::cell::Cell;
pub(crate) mod admission;
pub(crate) mod certificate;
pub(crate) mod identity;
pub(crate) use admission::{admit_case, admit_submission, weight_sum};
pub(crate) use certificate::{consistent, replay, CaseBudget};
pub(crate) use identity::{
    field, hash_binding, hash_candidate, hash_formula, hash_submission, make_report,
};

pub(crate) fn digest(bytes: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(bytes).into())
}
pub(crate) fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    ram: u64,
    output: bool,
) -> Result<ResourceClaim<'v>, Error> {
    let r = ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(ram).map_err(Error::Resource)?,
    );
    if output {
        let o = ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(OUTPUT_BYTES).map_err(Error::Resource)?,
        );
        view.claim_resources(&[r, o]).map_err(Error::Resource)
    } else {
        view.claim_resources(&[r]).map_err(Error::Resource)
    }
}
pub(crate) fn check_claim(
    claim: &ResourceClaim<'_>,
    view: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    view.check().map_err(Error::Resource)?;
    claim.check().map_err(Error::Resource)?;
    if !claim.same_reservation(view) {
        return Err(Error::ForeignReservation);
    }
    Ok(())
}
pub(crate) fn source(binding: Binding, view: &ResourceReservationView<'_>) -> Result<(), Error> {
    view.check().map_err(Error::Resource)?;
    if binding.selection.source.digest() != view.source_revision() {
        return Err(Error::Resource(ResourceError::SourceMismatch));
    }
    if binding.context.source() != binding.selection.source
        || binding.probe_set != binding.selection.scope.probes()
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}
pub(crate) fn check_limits(l: Limits) -> Result<(), Error> {
    if l.max_case_work > 1_000_000
        || l.max_run_work == 0
        || l.max_run_work > 16_000_000
        || l.max_dispatches == 0
        || l.max_dispatches > 16
        || l.max_split_work > 8_000_000
    {
        return Err(Error::InvalidInput(InvalidInput::InvalidLimits));
    }
    Ok(())
}
/// Local nonrefund book. No global static, cross-root registration or refund.
pub(crate) struct WorkBook {
    work: Cell<u64>,
    dispatches: Cell<u16>,
    limits: Limits,
}
impl WorkBook {
    pub(crate) fn new(limits: Limits) -> Result<Self, Error> {
        check_limits(limits)?;
        Ok(Self {
            work: Cell::new(0),
            dispatches: Cell::new(0),
            limits,
        })
    }
    pub(crate) fn charge(&self, count: usize) -> Result<(), Error> {
        let count = u16::try_from(count).map_err(|_| Error::WorkAdmissionExhausted)?;
        let dispatches = self
            .dispatches
            .get()
            .checked_add(count)
            .ok_or(Error::WorkAdmissionExhausted)?;
        let work = self
            .limits
            .max_case_work
            .checked_mul(u64::from(count))
            .and_then(|n| n.checked_add(self.work.get()))
            .ok_or(Error::WorkAdmissionExhausted)?;
        if work > self.limits.max_run_work || dispatches > self.limits.max_dispatches {
            return Err(Error::WorkAdmissionExhausted);
        }
        self.work.set(work);
        self.dispatches.set(dispatches);
        Ok(())
    }
    pub(crate) fn charged(&self) -> u64 {
        self.work.get()
    }
}
pub(crate) fn vector<T>(capacity: usize) -> Result<Vec<T>, Error> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(capacity)
        .map_err(|_| Error::AllocationFailed)?;
    Ok(result)
}
pub(crate) fn evaluate(
    theory: &lc631_solvers::typed_query::SmtStatusQuery,
    goal: &lc631_solvers::typed_query::BoolExpr,
    submission: Submission<'_>,
    b: Binding,
    limit: u64,
) -> Result<types::Outcome, Error> {
    let mut budget = CaseBudget::new(limit);
    let state = if consistent(theory, b, &mut budget)?.is_none() {
        CaseState::Unknown(Gap::WorkLimit)
    } else {
        replay(theory, goal, submission, &mut budget)?
    };
    Ok(types::Outcome {
        state,
        node: ScoreNode::state(state)?,
    })
}
