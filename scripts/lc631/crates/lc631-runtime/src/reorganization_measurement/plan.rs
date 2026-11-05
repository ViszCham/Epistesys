//! Immutable fixed suite plus a nonrefunding account for this exact plan only.
use super::{claim, finite, Error, RebindProbe, RoleModel};
use crate::{
    formal_boolean::{
        self as formal, Binding, DirectionResult, Evaluation, Request, UnconstrainedPolicy,
    },
    joint_selection::SelectionPins,
};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceReservationView},
    Digest32,
};
use sha2::{Digest, Sha256};
use std::cell::Cell;

#[derive(Clone, Copy)]
pub struct Limits {
    pub max_units: usize,
    pub max_atoms: usize,
    pub max_probes: usize,
    pub max_ast_nodes: u64,
    pub max_ast_depth: u16,
    pub max_payload_bytes: usize,
    pub max_weight: u32,
    pub max_check_work: u64,
    pub max_total_work: u64,
    pub plan_ram: u64,
    pub program_ram: u64,
    pub queries_ram: u64,
    pub owner_ram: u64,
    pub scratch_ram: u64,
    pub selector_ram: u64,
}
impl Limits {
    pub(super) fn checked(self) -> Result<Self, Error> {
        if self.max_units == 0
            || self.max_units > 16
            || self.max_atoms == 0
            || self.max_atoms > 2
            || self.max_probes == 0
            || self.max_probes > 64
            || self.max_ast_nodes == 0
            || self.max_ast_nodes > 64
            || self.max_ast_depth == 0
            || self.max_ast_depth > 16
            || self.max_payload_bytes == 0
            || self.max_payload_bytes > 16 * 1024
            || self.max_weight == 0
            || self.max_weight > 1_000_000
            || self.max_check_work == 0
            || self.max_check_work > 4096
            || self.max_total_work == 0
            || self.max_total_work > 262_144
            || [
                self.plan_ram,
                self.program_ram,
                self.queries_ram,
                self.owner_ram,
            ] != [65_536; 4]
            || self.scratch_ram != 262_144
            || self.selector_ram != 262_144
        {
            return Err(Error::InvalidLimits);
        }
        Ok(self)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkQuotaStatus {
    NeedsEvidenceAcrossPlans,
}

pub struct RebindPlan<'i, 'v> {
    pub(super) baseline: &'i RoleModel<'i>,
    pub(super) probes: &'i [RebindProbe<'i>],
    pub(super) pins: SelectionPins,
    pub(super) limits: Limits,
    pub(super) digest: Digest32,
    denominator: u64,
    spent: Cell<u64>,
    retained: ResourceClaim<'v>,
}
impl<'i, 'v> RebindPlan<'i, 'v> {
    pub fn capture_fixed(
        baseline: &'i RoleModel<'i>,
        probes: &'i [RebindProbe<'i>],
        pins: SelectionPins,
        limits: Limits,
        resources: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        let limits = limits.checked()?;
        let retained = claim(resources, limits.plan_ram)?;
        if resources.source_revision() != baseline.source.id().digest()
            || pins.source != baseline.context.source()
        {
            return Err(Error::ForeignSourceOwner);
        }
        if probes.is_empty() {
            return Err(Error::EmptyProbeSet);
        }
        if probes.len() > limits.max_probes
            || baseline.bindings.len() > limits.max_units
            || baseline.theory.boolean_symbols().len() > limits.max_atoms
        {
            return Err(Error::ShapeLimit);
        }
        if !baseline.theory.assertions().is_empty() {
            return Err(Error::TheoryMismatch);
        }
        for b in baseline.bindings {
            finite::scan(
                b.body,
                &[b.parameter],
                limits.max_ast_nodes,
                limits.max_ast_depth,
            )?;
        }
        let mut denominator = 0u64;
        for (i, p) in probes.iter().enumerate() {
            if probes[..i].iter().any(|x| x.unit == p.unit) {
                return Err(Error::DuplicateProbe);
            }
            if p.weight == 0 || p.weight > limits.max_weight {
                return Err(Error::InvalidWeight);
            }
            baseline.binding(p.unit)?;
            finite::scan(
                p.root,
                baseline.theory.boolean_symbols(),
                limits.max_ast_nodes,
                limits.max_ast_depth,
            )?;
            denominator = denominator
                .checked_add(u64::from(p.weight))
                .ok_or(Error::ArithmeticOverflow)?;
        }
        if probes.len() != baseline.bindings.len() {
            return Err(Error::ProbeSetMismatch);
        }
        let digest = plan_digest(baseline, probes, pins, limits);
        let plan = Self {
            baseline,
            probes,
            pins,
            limits,
            digest,
            denominator,
            spent: Cell::new(0),
            retained,
        };
        plan.charge(baseline.nodes)?;
        for p in probes {
            plan.admit_root(p)?;
        }
        plan.check(resources)?;
        Ok(plan)
    }
    fn admit_root(&self, p: &RebindProbe<'_>) -> Result<(), Error> {
        let b = self.baseline.binding(p.unit)?;
        let mut symbols = super::vector(self.baseline.theory.boolean_symbols().len())?;
        for s in self.baseline.theory.boolean_symbols() {
            symbols.push((*s, *s));
        }
        let mut r = Request {
            source: self.baseline.theory,
            target: self.baseline.theory,
            source_predicate: b.body,
            target_predicate: p.root,
            symbols: &symbols,
            unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
            binding: Binding {
                source: self.pins.source,
                target: self.pins.source,
                source_context: self.baseline.context,
                target_context: self.baseline.context,
                probe_set: self.digest,
                probe_map: self.digest,
            },
        };
        let n = finite::scan(b.body, self.baseline.theory.boolean_symbols(), 64, 16)?
            + finite::scan(p.root, self.baseline.theory.boolean_symbols(), 64, 16)?;
        self.charge(n)?;
        r.binding.probe_map = formal::probe_map_digest(&r, 4096)?;
        let cost = n
            .checked_mul(1 + (1u64 << symbols.len()))
            .ok_or(Error::ArithmeticOverflow)?;
        self.charge(cost)?;
        match formal::evaluate(&r, 4096)? {
            Evaluation::Computed(result)
                if result.soundness() == DirectionResult::Preserved
                    && result.completeness() == DirectionResult::Preserved =>
            {
                Ok(())
            }
            Evaluation::Computed(_)
            | Evaluation::NoComparable { .. }
            | Evaluation::Unknown { .. } => Err(Error::RootBaselineMismatch),
        }
    }
    pub fn denominator(&self) -> u64 {
        self.denominator
    }
    /// Conservative registered logical work charged before bounded operations.
    /// It is not a wall-time/OS instruction count and is not refunded on failure.
    pub fn work_spent(&self) -> u64 {
        self.spent.get()
    }
    pub fn task_run_work_status(&self) -> WorkQuotaStatus {
        WorkQuotaStatus::NeedsEvidenceAcrossPlans
    }
    pub(super) fn charge(&self, amount: u64) -> Result<(), Error> {
        let next = self
            .spent
            .get()
            .checked_add(amount)
            .ok_or(Error::WorkBudget)?;
        if next > self.limits.max_total_work {
            return Err(Error::WorkBudget);
        }
        self.spent.set(next);
        Ok(())
    }
    pub(super) fn check(&self, resources: &ResourceReservationView<'_>) -> Result<(), Error> {
        if !self.retained.same_reservation(resources) {
            return Err(Error::ForeignReservation);
        }
        self.retained.check()?;
        resources.check()?;
        Ok(())
    }
}
fn plan_digest(
    b: &RoleModel<'_>,
    probes: &[RebindProbe<'_>],
    pins: SelectionPins,
    l: Limits,
) -> Digest32 {
    let mut h = Sha256::new();
    h.update(b"epistesys.rebind.fixed-plan.finite-bool-v1\0");
    h.update(b.digest.as_bytes());
    for p in [
        pins.source.digest(),
        pins.scope.comparison(),
        pins.scope.goal(),
        pins.scope.probes(),
        pins.scope.semantics(),
        pins.baseline,
        pins.hard_invariants,
        pins.policy,
        pins.evidence_revision,
    ] {
        h.update(p.as_bytes());
    }
    match pins.goal_profile {
        Some(d) => {
            h.update([1]);
            h.update(d.as_bytes());
        }
        None => h.update([0]),
    }
    h.update((probes.len() as u64).to_le_bytes());
    let mut order: [usize; 16] = [0; 16];
    for (i, o) in order.iter_mut().take(probes.len()).enumerate() {
        *o = i;
    }
    order[..probes.len()].sort_unstable_by_key(|i| probes[*i].unit);
    for i in &order[..probes.len()] {
        let p = &probes[*i];
        h.update(p.unit.0.to_le_bytes());
        h.update(p.weight.to_le_bytes());
        finite::expression(&mut h, p.root);
    }
    for n in [
        l.max_units as u64,
        l.max_atoms as u64,
        l.max_probes as u64,
        l.max_ast_nodes,
        u64::from(l.max_ast_depth),
        l.max_payload_bytes as u64,
        u64::from(l.max_weight),
        l.max_check_work,
        l.max_total_work,
        l.plan_ram,
        l.program_ram,
        l.queries_ram,
        l.owner_ram,
        l.scratch_ram,
        l.selector_ram,
    ] {
        h.update(n.to_le_bytes());
    }
    finite::finish(h)
}
