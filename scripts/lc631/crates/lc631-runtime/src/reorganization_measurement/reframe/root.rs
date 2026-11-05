//! Fixed explicit goal/suite/registry. A plan-local work account is not global.
use super::{
    claim, transition, vec_cap, Cause, Error, ExactLimits, ExactRootInput, FrameProfile, OracleId,
    RootGoal, SourceBasis,
};
use crate::structural_gain_measurement::{ClosedGraph, Edge};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceReservationView},
    Digest32,
};
use sha2::{Digest, Sha256};
use std::cell::Cell;
pub struct FrameFamily<'i, 'v> {
    pub(super) base: &'i ClosedGraph<'i>,
    profiles: Vec<FrameProfile>,
    pub(super) oracle: OracleId,
    retained: ResourceClaim<'v>,
}
impl<'i, 'v> FrameFamily<'i, 'v> {
    pub fn register_exact(
        base: &'i ClosedGraph<'i>,
        profiles: &[FrameProfile],
        oracle: OracleId,
        resources: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let retained = claim(resources, 65536)?;
        if profiles.is_empty() || profiles.len() > 2 {
            return Err(Error::InvalidLimits);
        }
        let mut owned = vec_cap(profiles.len())?;
        for p in profiles {
            if matches!(p, FrameProfile::Unregistered(_)) {
                return Err(Error::Unsupported(Cause::UnsupportedProfile));
            }
            if owned.contains(p) {
                return Err(Error::InvalidLimits);
            }
            owned.push(*p);
        }
        Ok(Self {
            base,
            profiles: owned,
            oracle,
            retained,
        })
    }
    pub(super) fn check(&self, r: &ResourceReservationView<'_>) -> Result<(), Error> {
        if !self.retained.same_reservation(r) {
            return Err(Error::ForeignReservation);
        }
        self.retained.check()?;
        r.check()?;
        Ok(())
    }
    pub(super) fn supports(&self, p: FrameProfile) -> bool {
        self.profiles.contains(&p)
    }
}
pub struct ReframePlan<'i, 's, 'v> {
    pub(super) input: ExactRootInput<'i, 's, 'v>,
    pub(super) limits: ExactLimits,
    pub(super) digest: Digest32,
    pub(super) root_before: bool,
    denominator: u64,
    spent: Cell<u64>,
    retained: ResourceClaim<'v>,
}
impl<'i, 's, 'v> ReframePlan<'i, 's, 'v> {
    pub fn capture_exact(
        input: ExactRootInput<'i, 's, 'v>,
        limits: ExactLimits,
        resources: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let limits = limits.checked()?;
        let retained = claim(resources, limits.retained_ram)?;
        if input.source_basis == SourceBasis::Unobserved {
            return Err(Error::NeedsEvidence(Cause::SourceUnobserved));
        }
        if !std::ptr::eq(input.source, input.base.source_store()) {
            return Err(Error::ForeignSourceOwner);
        }
        if !std::ptr::eq(input.base, input.family.base) {
            return Err(Error::NoComparable(Cause::ForeignPlan));
        }
        input.family.check(resources)?;
        if input.perspectives.is_empty()
            || input.perspectives.len() > 16
            || input.baseline_terms.len() != input.perspectives.len()
        {
            return Err(Error::InvalidProbeSet);
        }
        let n = input.base.node_count();
        if n == 0 || n > 16 || input.base.edges().len() > 64 {
            return Err(Error::InvalidLimits);
        }
        let mut den = 0u64;
        for (i, q) in input.perspectives.iter().enumerate() {
            if input.perspectives[..i].iter().any(|x| x.id == q.id)
                || q.edge.0 .0 >= n
                || q.edge.1 .0 >= n
            {
                return Err(Error::InvalidProbeSet);
            }
            if q.weight == 0 || q.weight > 1000000 {
                return Err(Error::InvalidWeight);
            }
            if input
                .baseline_terms
                .iter()
                .filter(|t| t.query == q.id)
                .count()
                != 1
            {
                return Err(Error::InvalidProbeSet);
            }
            den = den
                .checked_add(u64::from(q.weight))
                .ok_or(Error::ArithmeticOverflow)?;
        }
        let mut h = Sha256::new();
        h.update(b"epistesys.reframe.exact-root.v2\0");
        h.update([input.family.oracle as u8]);
        h.update(input.source.id().digest().as_bytes());
        h.update(input.base.digest().as_bytes());
        h.update(
            input
                .original
                .state()
                .revision()
                .interpretation_digest()
                .as_bytes(),
        );
        let RootGoal::Reachability(goal) = *input.root_goal;
        h.update(goal.0 .0.to_le_bytes());
        h.update(goal.1 .0.to_le_bytes());
        h.update(input.baseline_frame.declaration.as_bytes());
        for q in input.perspectives {
            h.update(q.id.0.to_le_bytes());
            h.update(q.edge.0 .0.to_le_bytes());
            h.update(q.edge.1 .0.to_le_bytes());
            h.update(q.weight.to_le_bytes());
        }
        for t in input.baseline_terms {
            h.update(t.query.0.to_le_bytes());
            h.update((t.text.len() as u64).to_le_bytes());
            h.update(t.text.as_bytes());
        }
        for v in [
            input.pins.scope.comparison(),
            input.pins.scope.goal(),
            input.pins.scope.probes(),
            input.pins.scope.semantics(),
            input.pins.baseline,
            input.pins.hard_invariants,
            input.pins.policy,
            input.pins.evidence_revision,
        ] {
            h.update(v.as_bytes());
        }
        for v in [
            limits.max_root_work,
            limits.max_query_work,
            limits.max_map_work,
            limits.max_total_work,
        ] {
            h.update(v.to_le_bytes());
        }
        let mut root = Self {
            input,
            limits,
            digest: Digest32::from_bytes(h.finalize().into()),
            root_before: false,
            denominator: den,
            spent: Cell::new(0),
            retained,
        };
        root.check(resources)?;
        transition::validate_frame(&root, input.baseline_frame, input.state)?;
        let premise = input
            .original
            .state()
            .premises()
            .iter()
            .find(|p| p.id() == input.frame_premise)
            .ok_or(Error::FrameDeltaMismatch)?;
        if !premise.changeable() || premise.statement() != input.baseline_frame.declaration {
            return Err(Error::FrameDeltaMismatch);
        }
        root.root_before = root.reachable(goal)?;
        if !root.root_before {
            return Err(Error::NeedsEvidence(Cause::RootMismatch));
        }
        Ok(root)
    }
    pub fn denominator(&self) -> u64 {
        self.denominator
    }
    pub fn work_spent(&self) -> u64 {
        self.spent.get()
    }
    pub fn task_run_work_status(&self) -> super::super::WorkQuotaStatus {
        super::super::WorkQuotaStatus::NeedsEvidenceAcrossPlans
    }
    pub(super) fn charge(&self, n: u64) -> Result<(), Error> {
        let next = self.spent.get().checked_add(n).ok_or(Error::WorkBudget)?;
        if next > self.limits.max_total_work {
            return Err(Error::WorkBudget);
        }
        self.spent.set(next);
        Ok(())
    }
    pub(super) fn check(&self, r: &ResourceReservationView<'_>) -> Result<(), Error> {
        if !self.retained.same_reservation(r) {
            return Err(Error::ForeignReservation);
        }
        self.retained.check()?;
        self.input.family.check(r)?;
        let p = self.input.original.state();
        if self.input.source.id().digest() != p.revision().source().digest()
            || r.source_revision() != self.input.source.id().digest()
            || p.scope() != self.input.pins.scope
            || self.input.pins.source != p.revision().source()
            || self.input.base.context() != p.revision()
        {
            return Err(Error::InvalidRoot);
        }
        if !std::ptr::eq(self.input.source, self.input.base.source_store()) {
            return Err(Error::ForeignSourceOwner);
        }
        let basis = self.input.state.basis();
        self.charge((self.input.base.edges().len() + 1) as u64)?;
        if basis.context() != p.revision()
            || basis.scope() != p.scope()
            || u16::from(basis.node_count()) != self.input.base.node_count()
            || basis.edges().len() != self.input.base.edges().len()
            || basis
                .edges()
                .iter()
                .zip(self.input.base.edges())
                .any(|(a, b)| u16::from(a.from.0) != b.0 .0 || u16::from(a.to.0) != b.1 .0)
        {
            return Err(Error::FramePayloadMismatch);
        }
        r.check()?;
        Ok(())
    }
    pub(super) fn reachable(&self, e: Edge) -> Result<bool, Error> {
        let n = self.input.base.node_count();
        if e.0 .0 >= n || e.1 .0 >= n {
            return Err(Error::InvalidRoot);
        }
        let mut seen = [false; 16];
        let mut queue = [0u16; 16];
        let (mut head, mut tail, mut work) = (0, 1, 0u64);
        queue[0] = e.0 .0;
        seen[usize::from(e.0 .0)] = true;
        while head < tail {
            let at = queue[head];
            head += 1;
            if at == e.1 .0 {
                return Ok(true);
            }
            for edge in self.input.base.edges() {
                if work == self.limits.max_root_work {
                    return Err(Error::NeedsEvidence(Cause::WorkExhausted));
                }
                self.charge(1)?;
                work += 1;
                if edge.0 .0 == at && !seen[usize::from(edge.1 .0)] {
                    seen[usize::from(edge.1 .0)] = true;
                    queue[tail] = edge.1 .0;
                    tail += 1;
                }
            }
        }
        Ok(false)
    }
}
