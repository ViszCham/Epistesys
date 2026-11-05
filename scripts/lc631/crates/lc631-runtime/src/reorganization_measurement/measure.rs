//! Actual finite Evaluations retain their exact Request backing until drop.
use super::{claim, dependence, finite, vector, CheckKind, Error, RebindQueries, UnitId};
use crate::formal_boolean::{self as formal, DirectionResult, Evaluation, Request};
use lc631_analysis::creativity_measurement::{
    ComponentMetric, ComponentReport, ComponentState, IndependenceStatus, InputBasis,
    MeasurementBinding, RationalInterval, SemanticValidation,
};
use lc631_substrate::resource::{ResourceClaim, ResourceReservationView};
use sha2::{Digest, Sha256};

pub struct CheckRecord<'q> {
    unit: UnitId,
    kind: CheckKind,
    request: &'q Request<'q>,
    evaluation: Evaluation<'q>,
}
impl<'q> CheckRecord<'q> {
    pub fn unit(&self) -> UnitId {
        self.unit
    }
    pub fn kind(&self) -> CheckKind {
        self.kind
    }
    pub fn request(&self) -> &'q Request<'q> {
        self.request
    }
    pub fn evaluation(&self) -> &Evaluation<'q> {
        &self.evaluation
    }
}
pub struct OwnedRebind<'q, 'g, 'p, 'i, 'v> {
    pub(super) queries: &'q RebindQueries<'g, 'p, 'i, 'v>,
    pub(super) checks: Vec<CheckRecord<'q>>,
    pub(super) report: ComponentReport,
    pub(super) gaps: bool,
    use_proofs: Vec<dependence::MapUseProof<'q>>,
    retained: ResourceClaim<'v>,
}
impl<'q, 'g, 'p, 'i, 'v> OwnedRebind<'q, 'g, 'p, 'i, 'v> {
    pub fn evaluate(
        queries: &'q RebindQueries<'g, 'p, 'i, 'v>,
        resources: &'v ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        queries.check(resources)?;
        let plan = queries.program.plan;
        let retained = claim(resources, plan.limits.owner_ram)?;
        let scratch = claim(resources, plan.limits.scratch_ram)?;
        let storage = queries
            .requests
            .len()
            .checked_mul(std::mem::size_of::<CheckRecord<'q>>())
            .and_then(|n| {
                plan.probes
                    .len()
                    .checked_mul(std::mem::size_of::<dependence::MapUseProof<'q>>())
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or(Error::ArithmeticOverflow)?;
        if storage as u64 > plan.limits.owner_ram {
            return Err(Error::PayloadLimit);
        }
        let mut checks = vector(queries.requests.len())?;
        let mut gaps = false;
        for (request, key) in queries.requests.iter().zip(&queries.keys) {
            queries.check(resources)?;
            let cost = key
                .nodes
                .checked_mul(1 + (1u64 << request.symbols.len()))
                .ok_or(Error::ArithmeticOverflow)?;
            plan.charge(cost.min(plan.limits.max_check_work))?;
            let evaluation = formal::evaluate(request, plan.limits.max_check_work)?;
            match &evaluation {
                Evaluation::Computed(result)
                    if key.kind != CheckKind::RoleDifference
                        && (result.soundness() != DirectionResult::Preserved
                            || result.completeness() != DirectionResult::Preserved) =>
                {
                    return Err(Error::PrerequisiteFailed {
                        unit: key.unit,
                        check: key.kind,
                    });
                }
                Evaluation::NoComparable { .. } => {
                    // Full unconditioned admitted theories should be consistent.
                    // A failure remains ineligible, never fabricated as measured0.
                    return Err(Error::PrerequisiteFailed {
                        unit: key.unit,
                        check: key.kind,
                    });
                }
                Evaluation::Unknown { .. } => gaps = true,
                Evaluation::Computed(_) => {}
            }
            checks.push(CheckRecord {
                unit: key.unit,
                kind: key.kind,
                request,
                evaluation,
            });
        }
        // Preserve legacy known MapBack/RootUse failure causes before this new
        // prerequisite. Never bypass use evidence for an equivalent role.
        let use_proofs = dependence::evaluate(queries, resources)?;
        gaps |= use_proofs.iter().any(dependence::MapUseProof::pending);
        let lineage_work = (use_proofs.len() as u64)
            .checked_mul(
                plan.limits
                    .max_ast_nodes
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)?;
        plan.charge(lineage_work)?;
        let interval = aggregate(queries, &checks, &use_proofs)?;
        let report = report(queries, &checks, &use_proofs, interval, gaps);
        scratch.check()?;
        queries.check(resources)?;
        let owner = Self {
            queries,
            checks,
            report,
            gaps,
            use_proofs,
            retained,
        };
        owner.check(resources)?;
        Ok(owner)
    }
    /// Historical inspection only. This report cannot construct checked rows.
    pub fn historical_report(&self) -> &ComponentReport {
        &self.report
    }
    pub(super) fn check(&self, resources: &ResourceReservationView<'_>) -> Result<(), Error> {
        if !self.retained.same_reservation(resources) {
            return Err(Error::ForeignReservation);
        }
        self.retained.check()?;
        self.queries.check(resources)?;
        if self.checks.len() != self.queries.requests.len() {
            return Err(Error::ProbeSetMismatch);
        }
        self.queries.program.plan.charge(self.checks.len() as u64)?;
        for ((check, request), key) in self
            .checks
            .iter()
            .zip(&self.queries.requests)
            .zip(&self.queries.keys)
        {
            if !std::ptr::eq(check.request, request)
                || check.unit != key.unit
                || check.kind != key.kind
            {
                return Err(Error::ForeignQueries);
            }
            if let Evaluation::Computed(result) = &check.evaluation {
                if !std::ptr::eq(result.request(), request) {
                    return Err(Error::ForeignQueries);
                }
            }
        }
        dependence::validate(&self.use_proofs, self.queries)?;
        self.retained.check()?;
        resources.check()?;
        Ok(())
    }
}
fn aggregate(
    queries: &RebindQueries<'_, '_, '_, '_>,
    checks: &[CheckRecord<'_>],
    use_proofs: &[dependence::MapUseProof<'_>],
) -> Result<RationalInterval, Error> {
    let plan = queries.program.plan;
    let mut low = 0u64;
    let mut high = 0u64;
    for probe in plan.probes {
        let mut changed = None;
        let use_proof = use_proofs
            .iter()
            .find(|proof| proof.unit() == probe.unit)
            .ok_or(Error::ProbeSetMismatch)?;
        let mut complete = match use_proof.status()? {
            dependence::UseStatus::Dependent => true,
            dependence::UseStatus::Unknown | dependence::UseStatus::NoComparable => false,
        };
        for check in checks.iter().filter(|c| c.unit == probe.unit) {
            match &check.evaluation {
                Evaluation::Computed(result) if check.kind == CheckKind::RoleDifference => {
                    changed = Some(
                        result.soundness() != DirectionResult::Preserved
                            || result.completeness() != DirectionResult::Preserved,
                    );
                }
                Evaluation::Computed(_) => {}
                Evaluation::Unknown { .. } | Evaluation::NoComparable { .. } => complete = false,
            }
        }
        let weight = u64::from(probe.weight);
        match (complete, changed) {
            (true, Some(true)) => {
                low = low.checked_add(weight).ok_or(Error::ArithmeticOverflow)?;
                high = high.checked_add(weight).ok_or(Error::ArithmeticOverflow)?;
            }
            (true, Some(false)) => {}
            (false, _) | (true, None) => {
                high = high.checked_add(weight).ok_or(Error::ArithmeticOverflow)?
            }
        }
    }
    let l = i64::try_from(low).map_err(|_| Error::ArithmeticOverflow)?;
    let h = i64::try_from(high).map_err(|_| Error::ArithmeticOverflow)?;
    let den = i64::try_from(plan.denominator()).map_err(|_| Error::ArithmeticOverflow)?;
    Ok(RationalInterval::from_ratio(l, den, h, den)?)
}
fn report(
    queries: &RebindQueries<'_, '_, '_, '_>,
    checks: &[CheckRecord<'_>],
    use_proofs: &[dependence::MapUseProof<'_>],
    interval: RationalInterval,
    gaps: bool,
) -> ComponentReport {
    let program = queries.program;
    let plan = program.plan;
    let mut h = Sha256::new();
    h.update(b"epistesys.rebind.actual-formal-results.v2-input-dependence\0");
    h.update(program.digest.as_bytes());
    h.update((checks.len() as u64).to_le_bytes());
    for c in checks {
        h.update(c.unit.0.to_le_bytes());
        h.update([match c.kind {
            CheckKind::RoleDifference => 0,
            CheckKind::MapBack => 1,
            CheckKind::RootUse => 2,
        }]);
        match &c.evaluation {
            Evaluation::Computed(r) => {
                h.update([0]);
                h.update(r.assignments().to_le_bytes());
                h.update(r.ast_visits().to_le_bytes());
                direction(&mut h, r.soundness());
                direction(&mut h, r.completeness());
                h.update(r.source_theory_digest().as_bytes());
                h.update(r.target_theory_digest().as_bytes());
                h.update(c.request.binding.probe_map.as_bytes());
            }
            Evaluation::Unknown { ast_visits } => {
                h.update([1]);
                h.update(ast_visits.to_le_bytes());
            }
            Evaluation::NoComparable { ast_visits, .. } => {
                h.update([2]);
                h.update(ast_visits.to_le_bytes());
            }
        }
    }
    dependence::append_digest(&mut h, use_proofs);
    let result = finite::finish(h);
    let producer = finite::tag(b"epistesys.rebind.private-finite-producer.v2-input-dependence");
    let no_comparable = use_proofs
        .iter()
        .find_map(dependence::MapUseProof::no_comparable_gap);
    ComponentReport {
        metric: ComponentMetric::Rebind,
        definition: finite::tag(b"CR-REBIND/finite-bool-role-v1"),
        binding: MeasurementBinding {
            scope: plan.pins.scope.comparison(),
            candidate: program.digest,
            source: plan.baseline.source.id().digest(),
            target: program.candidate.digest,
            context: program.candidate.context.interpretation_digest(),
            baseline: plan.pins.baseline,
            producer,
            transform: program.digest,
            result,
        },
        probe_set: plan.digest,
        oracle: finite::tag(b"FiniteBoolRoleRebind-v1/full-domain+input-dependence-v1"),
        dependency_group: plan.digest,
        evidence_payload: result,
        basis: InputBasis::Given,
        validation: SemanticValidation::Unassessed,
        independence: IndependenceStatus::SharedDependency,
        state: if let Some(reason) = no_comparable {
            ComponentState::Unknown(reason)
        } else if gaps {
            ComponentState::ResourceExhausted {
                partial: Some(interval),
            }
        } else {
            ComponentState::Measured(interval)
        },
    }
}
fn direction(h: &mut Sha256, d: DirectionResult) {
    match d {
        DirectionResult::Preserved => h.update([0]),
        DirectionResult::Countermodel(row) => {
            h.update([1]);
            h.update(row.to_le_bytes());
        }
    }
}
