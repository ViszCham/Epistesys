//! Scoped K1 over actual baseline/candidate paid owners and retained causes.
use super::{
    claim, vec_cap, Cause, CauseRecord, Error, FrameProfile, PaidFrameMap, PerspectiveOutcome,
    ProbeId, ReframePlan, RootGoal,
};
use lc631_analysis::creativity_measurement::{
    ComponentMetric, ComponentReport, ComponentState, IndependenceStatus, InputBasis,
    MeasurementBinding, RationalInterval, SemanticValidation,
};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceReservationView},
    Digest32,
};
use sha2::{Digest, Sha256};
pub struct PairInput<'o, 'p, 'i, 's, 'v> {
    pub baseline: Option<&'o PaidFrameMap<'p, 'i, 's, 'v>>,
    pub candidate: Option<&'o PaidFrameMap<'p, 'i, 's, 'v>>,
}
pub struct OwnedReframe<'o, 'p, 'i, 's, 'v> {
    pub(super) plan: &'p ReframePlan<'i, 's, 'v>,
    pub(super) baseline: &'o PaidFrameMap<'p, 'i, 's, 'v>,
    pub(super) candidate: &'o PaidFrameMap<'p, 'i, 's, 'v>,
    pub(super) report: ComponentReport,
    pub(super) causes: Vec<CauseRecord>,
    pub(super) base_outcomes: Vec<PerspectiveOutcome>,
    pub(super) candidate_outcomes: Vec<PerspectiveOutcome>,
    pub(super) changed: bool,
    pub(super) gaps: bool,
    pub(super) root_after: bool,
    retained: ResourceClaim<'v>,
}
impl<'o, 'p, 'i, 's, 'v> OwnedReframe<'o, 'p, 'i, 's, 'v> {
    pub fn measure_pair_current(
        plan: &'p ReframePlan<'i, 's, 'v>,
        input: PairInput<'o, 'p, 'i, 's, 'v>,
        r: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let baseline = input.baseline.ok_or(Error::MissingPaidMapOwner)?;
        let candidate = input.candidate.ok_or(Error::MissingPaidMapOwner)?;
        plan.check(r)?;
        let retained = claim(r, plan.limits.retained_ram)?;
        let scratch = claim(r, plan.limits.scratch_ram)?;
        validate(plan, baseline, candidate, r)?;
        // Profiles bind actual closed parser/lowering/evaluator implementations.
        // Display name and mere answer-program changes do not change a frame.
        let changed = baseline.profile() != candidate.profile();
        let mut low = 0u64;
        let mut high = 0u64;
        let mut gaps = false;
        let mut bo = vec_cap(plan.input.perspectives.len())?;
        let mut co = vec_cap(plan.input.perspectives.len())?;
        for ((q, b), c) in plan
            .input
            .perspectives
            .iter()
            .zip(&baseline.data.rows)
            .zip(&candidate.data.rows)
        {
            bo.push(b.outcome);
            co.push(c.outcome);
            if b.outcome == PerspectiveOutcome::Unknown || c.outcome == PerspectiveOutcome::Unknown
            {
                gaps = true;
                high = high
                    .checked_add(u64::from(q.weight))
                    .ok_or(Error::ArithmeticOverflow)?;
            } else if changed
                && b.outcome == PerspectiveOutcome::KnownFailure
                && c.outcome == PerspectiveOutcome::KnownPass
            {
                low = low
                    .checked_add(u64::from(q.weight))
                    .ok_or(Error::ArithmeticOverflow)?;
                high = high
                    .checked_add(u64::from(q.weight))
                    .ok_or(Error::ArithmeticOverflow)?;
            }
        }
        let mut causes = vec_cap(128)?;
        causes.extend_from_slice(&baseline.data.causes);
        causes.extend_from_slice(&candidate.data.causes);
        let RootGoal::Reachability(goal) = *plan.input.root_goal;
        let root_after = plan.reachable(goal)?;
        if !root_after {
            return Err(Error::NoComparable(Cause::RootMismatch));
        }
        let den = i64::try_from(plan.denominator()).map_err(|_| Error::ArithmeticOverflow)?;
        let interval = RationalInterval::from_ratio(
            i64::try_from(low).map_err(|_| Error::ArithmeticOverflow)?,
            den,
            i64::try_from(high).map_err(|_| Error::ArithmeticOverflow)?,
            den,
        )?;
        plan.charge((baseline.data.rows.len() + candidate.data.rows.len()) as u64)?;
        let result = pair_digest(plan, baseline, candidate);
        let report = ComponentReport {
            metric: ComponentMetric::Reframe,
            definition: tag(b"CR-REFRAME/exact-frame-v2"),
            binding: MeasurementBinding {
                scope: plan.input.pins.scope.comparison(),
                candidate: result,
                source: plan.input.source.id().digest(),
                target: program_digest(candidate),
                context: candidate
                    .data
                    .binding
                    .context()
                    .revision()
                    .interpretation_digest(),
                baseline: plan.input.pins.baseline,
                producer: tag(b"epistesys.reframe.private-exact-producer.v2"),
                transform: program_digest(candidate),
                result,
            },
            probe_set: plan.digest,
            oracle: tag(b"ExpandOneOuterStepToBasePathV1/exact-v2"),
            dependency_group: plan.digest,
            evidence_payload: result,
            basis: InputBasis::Given,
            validation: SemanticValidation::Unassessed,
            independence: IndependenceStatus::SharedDependency,
            state: if gaps {
                ComponentState::ResourceExhausted {
                    partial: Some(interval),
                }
            } else {
                ComponentState::Measured(interval)
            },
        };
        scratch.check()?;
        let owner = Self {
            plan,
            baseline,
            candidate,
            report,
            causes,
            base_outcomes: bo,
            candidate_outcomes: co,
            changed,
            gaps,
            root_after,
            retained,
        };
        owner.check(r)?;
        Ok(owner)
    }
    pub(super) fn check(&self, r: &ResourceReservationView<'_>) -> Result<(), Error> {
        if !self.retained.same_reservation(r) {
            return Err(Error::ForeignReservation);
        }
        self.retained.check()?;
        validate(self.plan, self.baseline, self.candidate, r)?;
        r.check()?;
        Ok(())
    }
    pub fn historical_report(&self) -> &ComponentReport {
        &self.report
    }
    pub(super) fn used_path(&self, id: ProbeId) -> Option<&[u16]> {
        self.candidate
            .data
            .rows
            .iter()
            .find(|q| q.id == id)?
            .mapped
            .as_deref()
    }
}
fn validate(
    plan: &ReframePlan<'_, '_, '_>,
    b: &PaidFrameMap<'_, '_, '_, '_>,
    c: &PaidFrameMap<'_, '_, '_, '_>,
    r: &ResourceReservationView<'_>,
) -> Result<(), Error> {
    if !std::ptr::eq(plan, b.data.plan) || !std::ptr::eq(plan, c.data.plan) {
        return Err(Error::NoComparable(Cause::ForeignPlan));
    }
    b.check(r)?;
    c.check(r)?;
    if !b.data.binding.baseline
        || !std::ptr::eq(b.data.binding.frame, plan.input.baseline_frame)
        || c.data.binding.baseline
    {
        return Err(Error::ForeignOwner);
    }
    Ok(())
}
fn tag(s: &[u8]) -> Digest32 {
    let mut h = Sha256::new();
    h.update(s);
    Digest32::from_bytes(h.finalize().into())
}
fn program_digest(p: &PaidFrameMap<'_, '_, '_, '_>) -> Digest32 {
    let mut h = Sha256::new();
    h.update(b"epistesys.reframe.actual-paid.v2\0");
    h.update(p.data.plan.digest.as_bytes());
    h.update(p.data.id.0.to_le_bytes());
    h.update(p.data.binding.frame.declaration.as_bytes());
    h.update([match p.profile() {
        FrameProfile::DirectEdgeV1 => 0,
        FrameProfile::CheckedPathMacroV1 => 1,
        FrameProfile::Unregistered(_) => 255,
    }]);
    for t in p.data.texts {
        h.update(t.query.0.to_le_bytes());
        h.update((t.text.len() as u64).to_le_bytes());
        h.update(t.text.as_bytes());
    }
    for row in &p.data.rows {
        h.update(row.id.0.to_le_bytes());
        h.update(row.query_work.to_le_bytes());
        h.update(row.map_work.to_le_bytes());
        h.update([match row.outcome {
            PerspectiveOutcome::KnownPass => 1,
            PerspectiveOutcome::KnownFailure => 0,
            PerspectiveOutcome::Unknown => 2,
        }]);
        match &row.mapped {
            Some(nodes) => {
                h.update([1]);
                h.update((nodes.len() as u64).to_le_bytes());
                for n in nodes {
                    h.update(n.to_le_bytes());
                }
            }
            None => h.update([0]),
        }
    }
    for c in &p.data.causes {
        h.update(c.query().0.to_le_bytes());
        h.update([c.stage() as u8, c.cause() as u8]);
    }
    Digest32::from_bytes(h.finalize().into())
}
fn pair_digest(
    plan: &ReframePlan<'_, '_, '_>,
    b: &PaidFrameMap<'_, '_, '_, '_>,
    c: &PaidFrameMap<'_, '_, '_, '_>,
) -> Digest32 {
    let mut h = Sha256::new();
    h.update(b"epistesys.reframe.paired-result.v2\0");
    h.update(plan.digest.as_bytes());
    h.update(program_digest(b).as_bytes());
    h.update(program_digest(c).as_bytes());
    Digest32::from_bytes(h.finalize().into())
}
