//! Current generated owner -> actual self-query reports -> K1 -> scoped consumer.
use super::*;
use lc631_analysis::creativity_measurement::{
    weighted_assessment, ComponentMetric, ComponentState, IndependenceStatus, InputBasis,
    MeasurementBinding, RationalInterval, SemanticValidation, WeightedProbe,
};
use sha2::{Digest, Sha256};
use support::{PairKind, PairObservation, PairSpec};

impl ValidatedFormalCandidate<'_, '_> {
    #[allow(clippy::too_many_arguments)] // Keep actual owner, immutable plan, dispatch and lease explicit.
    pub fn with_configuration_difference_current<'input, 'view, R>(
        &self,
        bridge: &CreativeCandidateBridge<'_, '_>,
        plan: &ConfigurationPlan<'input, '_>,
        dispatch: &[SupportDispatch<'_>],
        budget: WorkBudget,
        resources: &'view ResourceReservationView<'_>,
        consume: impl for<'scope> FnOnce(CurrentConfiguration<'scope>) -> R,
    ) -> Result<R, Error> {
        plan.current(bridge, resources)?;
        let query = self
            .project_current(bridge, resources)
            .map_err(Error::Creative)?;
        if self.prepared.parent() != plan.bindings.context {
            return Err(Error::ScopeMismatch);
        }
        let retained = util::claim(resources, util::RAM, util::OUTPUT)?;
        let check = |view: &ResourceReservationView<'_>| {
            plan.current(bridge, view)?;
            self.project_current(bridge, view)
                .map_err(Error::Creative)?;
            retained.check().map_err(Error::Resource)?;
            if !retained.same_reservation(view) {
                return Err(Error::ForeignPlan);
            }
            Ok(())
        };
        let mut work = util::Work::new(budget)?;
        let specs = preflight(
            plan,
            query,
            self.prepared.target(),
            dispatch,
            budget,
            &mut work,
        )?;
        let identity = self
            .measurement_identity_current(bridge, resources)
            .map_err(Error::Identity)?;
        support::with_batch(
            &specs,
            &plan.map,
            plan.bindings.probes,
            &mut work,
            |actual, work| {
                check(resources)?;
                let hard = hard_rows(plan.hard.len(), actual)?;
                let rows = probe_rows(plan, actual)?;
                let report = report(plan, self, identity, &rows, actual)?;
                let goal = util::goal(plan.goal)?;
                let result = consume(CurrentConfiguration {
                    report: &report,
                    probes: &rows,
                    hard: &hard,
                    queries: [plan.source, query],
                    contexts: [plan.bindings.context, self.prepared.target()],
                    map: &plan.map,
                    probe_set: plan.bindings.probes,
                    goal: &goal,
                    check: &check,
                    work,
                });
                check(resources)?;
                Ok(result)
            },
        )
    }
}
fn preflight<'a>(
    plan: &'a ConfigurationPlan<'a, '_>,
    target: &'a SmtStatusQuery,
    context: ContextRevision,
    dispatch: &[SupportDispatch<'_>],
    budget: WorkBudget,
    work: &mut util::Work,
) -> Result<Vec<PairSpec<'a>>, Error> {
    let (_, target_nodes) = util::theory(target)?;
    if target.boolean_symbols() != plan.source.boolean_symbols() {
        return Err(Error::ScopeMismatch);
    }
    let bytes = plan
        .payload_bytes
        .checked_add(target_nodes * 5)
        .ok_or(Error::PayloadBudget)?;
    if bytes > budget.max_payload_bytes || dispatch.len() > plan.probes.len() * 2 {
        return Err(Error::PayloadBudget);
    }
    work.charge(target_nodes + plan.probes.len() as u64 + 1)?;
    let mut specs = util::vector(plan.hard.len() * 2 + dispatch.len())?;
    for (index, predicate) in plan.hard.iter().enumerate() {
        for side in [ContextSide::Baseline, ContextSide::Candidate] {
            work.pair(plan.declaration.hard_query_work)?;
            specs.push(spec(
                plan,
                target,
                context,
                predicate,
                side,
                plan.declaration.hard_query_work,
                PairKind::Hard(index, side),
            ));
        }
    }
    for (index, item) in dispatch.iter().enumerate() {
        if item.plan != plan.digest {
            return Err(Error::ScopeMismatch);
        }
        if dispatch[..index]
            .iter()
            .any(|p| p.identity == item.identity && p.side == item.side)
        {
            return Err(Error::DuplicateDispatch);
        }
        let (i, probe) = plan
            .probes
            .iter()
            .enumerate()
            .find(|(_, p)| p.identity == item.identity)
            .ok_or(Error::ForeignProbe)?;
        work.pair(item.max_work)?;
        specs.push(spec(
            plan,
            target,
            context,
            probe.predicate,
            item.side,
            item.max_work,
            PairKind::Probe(i, item.side),
        ));
    }
    Ok(specs)
}
fn spec<'a>(
    plan: &'a ConfigurationPlan<'a, '_>,
    target: &'a SmtStatusQuery,
    context: ContextRevision,
    predicate: &'a BoolExpr,
    side: ContextSide,
    allowance: u64,
    kind: PairKind,
) -> PairSpec<'a> {
    PairSpec {
        query: if side == ContextSide::Baseline {
            plan.source
        } else {
            target
        },
        predicate,
        context: if side == ContextSide::Baseline {
            plan.bindings.context
        } else {
            context
        },
        allowance,
        kind,
    }
}
fn hard_rows<'a>(
    count: usize,
    actual: &'a [PairObservation<'a>],
) -> Result<Vec<HardOutcome<'a>>, Error> {
    let mut rows = util::vector(count)?;
    for index in 0..count {
        let find = |side| {
            actual
                .iter()
                .find(|r| matches!(r.kind, PairKind::Hard(i, s) if i == index && s == side))
                .ok_or(Error::InvalidPlan)
        };
        let baseline = find(ContextSide::Baseline)?;
        let candidate = find(ContextSide::Candidate)?;
        for result in [baseline.outcome, candidate.outcome] {
            match result {
                CanonicalOutcome::Supported => {}
                CanonicalOutcome::Unknown(gap) => return Err(Error::RequiredPending(gap)),
                CanonicalOutcome::Refuted | CanonicalOutcome::Undetermined => {
                    return Err(Error::RequiredViolation)
                }
            }
        }
        rows.push(HardOutcome {
            outcomes: [baseline.outcome, candidate.outcome],
            evaluations: [baseline.evaluations, candidate.evaluations],
        });
    }
    Ok(rows)
}
fn probe_rows<'a>(
    plan: &'a ConfigurationPlan<'a, '_>,
    actual: &[PairObservation<'_>],
) -> Result<Vec<ProbeOutcome<'a>>, Error> {
    let mut rows = util::vector(plan.probes.len())?;
    for (i, fixed) in plan.probes.iter().enumerate() {
        let mut outcomes = [CanonicalOutcome::Unknown(OutcomeGap::MissingDispatch); 2];
        for item in actual {
            if let PairKind::Probe(index, side) = item.kind {
                if index == i {
                    outcomes[if side == ContextSide::Baseline { 0 } else { 1 }] = item.outcome;
                }
            }
        }
        rows.push(ProbeOutcome { fixed, outcomes });
    }
    Ok(rows)
}
fn report(
    plan: &ConfigurationPlan<'_, '_>,
    candidate: &ValidatedFormalCandidate<'_, '_>,
    identity: Digest32,
    rows: &[ProbeOutcome<'_>],
    actual: &[PairObservation<'_>],
) -> Result<ComponentReport, Error> {
    let mut values = util::vector(rows.len())?;
    let mut complete = true;
    for row in rows {
        let unknown = row
            .outcomes
            .iter()
            .any(|x| matches!(x, CanonicalOutcome::Unknown(_)));
        complete &= !unknown;
        values.push(
            if unknown {
                RationalInterval::from_ratio(0, 1, 1, 1)
            } else {
                RationalInterval::exact_ratio(i64::from(row.outcomes[0] != row.outcomes[1]), 1)
            }
            .map_err(|e| Error::Arithmetic(e.into()))?,
        );
    }
    let mut weighted = util::vector(rows.len())?;
    for (row, value) in rows.iter().zip(&values) {
        weighted.push(WeightedProbe {
            canonical_payload: row.fixed.identity,
            weight: row.fixed.weight,
            value,
        });
    }
    let bounds = weighted_assessment(&weighted).map_err(Error::Arithmetic)?;
    let result = result_digest(plan, actual);
    let target = util::digest(
        b"epistesys.dsem.generated-payload.v1\0",
        &[candidate.historical_payload()],
    );
    let binding = MeasurementBinding {
        scope: plan.bindings.scope.comparison(),
        candidate: identity,
        source: plan.bindings.source.digest(),
        target,
        context: candidate.prepared.target().interpretation_digest(),
        baseline: plan.bindings.baseline,
        producer: plan.bindings.definition,
        transform: util::digest(
            b"epistesys.dsem.transform.v1\0",
            &[plan.digest.as_bytes(), target.as_bytes()],
        ),
        result,
    };
    Ok(ComponentReport {
        metric: ComponentMetric::SemanticConfigurationDistance,
        definition: plan.bindings.definition,
        binding,
        probe_set: plan.bindings.probes,
        oracle: plan.bindings.oracle,
        dependency_group: util::digest(
            b"epistesys.dsem.shared-ir.v1\0",
            &[b"formal_boolean;GivenSource"],
        ),
        evidence_payload: result,
        basis: InputBasis::Given,
        validation: if complete {
            SemanticValidation::FiniteExhaustive
        } else {
            SemanticValidation::Unassessed
        },
        independence: IndependenceStatus::SharedDependency,
        state: ComponentState::Measured(bounds),
    })
}
fn result_digest(plan: &ConfigurationPlan<'_, '_>, actual: &[PairObservation<'_>]) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(b"epistesys.dsem.actual-results.v1\0");
    hash.update(plan.digest.as_bytes());
    for item in actual {
        match item.kind {
            PairKind::Hard(i, s) => {
                hash.update([0]);
                hash.update((i as u64).to_le_bytes());
                hash.update([u8::from(s == ContextSide::Candidate)]);
            }
            PairKind::Probe(i, s) => {
                hash.update([1]);
                hash.update((i as u64).to_le_bytes());
                hash.update([u8::from(s == ContextSide::Candidate)]);
            }
        }
        for value in [item.evaluations.0, item.evaluations.1] {
            match value {
                f::Evaluation::Computed(r) => {
                    hash.update([0]);
                    hash.update(r.source_theory_digest().as_bytes());
                    hash.update(r.target_theory_digest().as_bytes());
                    hash.update(r.binding().probe_map.as_bytes());
                    hash.update(r.ast_visits().to_le_bytes());
                    match r.completeness() {
                        f::DirectionResult::Preserved => hash.update([0]),
                        f::DirectionResult::Countermodel(row) => {
                            hash.update([1]);
                            hash.update(row.to_le_bytes());
                        }
                    }
                }
                f::Evaluation::Unknown { ast_visits } => {
                    hash.update([1]);
                    hash.update(ast_visits.to_le_bytes());
                }
                f::Evaluation::NoComparable { ast_visits, .. } => {
                    hash.update([2]);
                    hash.update(ast_visits.to_le_bytes());
                }
            }
        }
    }
    util::finish(hash)
}
