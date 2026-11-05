//! Versioned typed streaming digests. They identify Given data, not authority.
use super::*;
use sha2::{Digest, Sha256};
fn start(domain: &[u8]) -> Sha256 {
    let mut h = Sha256::new();
    h.update(domain);
    h
}
fn finish(h: Sha256) -> Digest32 {
    Digest32::from_bytes(h.finalize().into())
}
fn bytes(h: &mut Sha256, b: &[u8]) {
    h.update((b.len() as u64).to_le_bytes());
    h.update(b);
}
fn rational(h: &mut Sha256, q: &Rational) {
    for integer in [q.numer(), q.denom()] {
        bytes(h, &integer.to_signed_bytes_le());
    }
}
pub(super) fn root(
    b: BreakCostBinding,
    cases: &[LegacyCase<'_>],
    rules: &[PropertyRule],
    payloads: &[Vec<u8>],
    order: &[usize],
    baseline: AdapterKind,
    budget: BreakCostBudget,
) -> Digest32 {
    let mut h = start(b"cr-break.fixed-legacy-root.v1\0");
    for d in [
        b.source.digest(),
        b.context.interpretation_digest(),
        b.scope.comparison(),
        b.scope.goal(),
        b.scope.probes(),
        b.scope.semantics(),
        b.baseline,
        b.hard_invariants,
        b.policy,
        b.evidence_revision,
        b.oracle,
        b.dependency_group,
    ] {
        h.update(d.as_bytes());
    }
    match b.goal_profile {
        Some(d) => {
            h.update([1]);
            h.update(d.as_bytes());
        }
        None => h.update([0]),
    }
    h.update([baseline as u8]);
    for &i in order {
        bytes(&mut h, &payloads[i]);
        rational(&mut h, cases[i].weight);
    }
    for r in rules {
        h.update([r.property as u8, r.class as u8]);
        rational(&mut h, &r.weight);
    }
    for n in [
        budget.max_cases,
        budget.max_input_bytes_per_case,
        budget.max_input_bytes_total,
        budget.max_candidate_text_bytes,
        budget.max_output_bytes_per_case,
        budget.max_output_bytes_per_run,
        budget.max_adapter_calls_per_run,
        budget.max_serializations_per_run,
        budget.max_property_checks_per_run,
        budget.max_soft_checks_per_run,
        budget.max_extra_hard_checks,
    ] {
        h.update((n as u64).to_le_bytes());
    }
    h.update(budget.max_weight_bits.to_le_bytes());
    h.update(budget.max_intermediate_bits.to_le_bytes());
    finish(h)
}
pub(super) fn report(
    root: &RootDefinition<'_, '_>,
    candidate: Option<CandidateBinding>,
    baseline_result: Option<Digest32>,
    actual: &execute::Observation,
    state: ComponentState,
) -> (ComponentReport, Digest32) {
    let result = result_digest(root, candidate, baseline_result, actual);
    let definition = digest(b"CR-BREAK/optional-dgcl-legacy-consumer-view-v1/absolute-K1");
    let report = ComponentReport {
        metric: ComponentMetric::BreakCost,
        definition,
        binding: MeasurementBinding {
            scope: root.binding.scope.comparison(),
            candidate: candidate
                .map(|c| c.candidate_digest)
                .unwrap_or(root.binding.baseline),
            source: root.binding.source.digest(),
            target: candidate
                .map(|c| c.target)
                .unwrap_or(root.binding.source.digest()),
            context: candidate
                .map(|c| c.context)
                .unwrap_or(root.binding.context)
                .interpretation_digest(),
            baseline: root.binding.baseline,
            producer: digest(b"existing-dgcl-projector+strict-legacy-serde/v1"),
            transform: candidate.map(|c| c.transform).unwrap_or(root.digest),
            result,
        },
        probe_set: root.binding.scope.probes(),
        oracle: root.binding.oracle,
        dependency_group: root.binding.dependency_group,
        evidence_payload: result,
        basis: InputBasis::Given,
        validation: SemanticValidation::Unassessed,
        independence: IndependenceStatus::SharedDependency,
        state,
    };
    let binding = binding_digest(&report);
    (report, binding)
}
fn result_digest(
    root: &RootDefinition<'_, '_>,
    candidate: Option<CandidateBinding>,
    baseline_result: Option<Digest32>,
    actual: &execute::Observation,
) -> Digest32 {
    let mut h = start(b"cr-break.actual-legacy-observation.v1\0");
    h.update(root.digest.as_bytes());
    if let Some(d) = baseline_result {
        h.update([1]);
        h.update(d.as_bytes());
    } else {
        h.update([0]);
    }
    if let Some(c) = candidate {
        h.update([1]);
        h.update(c.id.0.to_le_bytes());
        for d in [
            c.context.interpretation_digest(),
            c.candidate_digest,
            c.target,
            c.transform,
        ] {
            h.update(d.as_bytes());
        }
    } else {
        h.update([0]);
    }
    for kind in &actual.kinds {
        h.update([*kind as u8]);
    }
    for output in &actual.outputs {
        bytes(&mut h, output);
    }
    for outcomes in &actual.outcomes {
        for outcome in outcomes {
            h.update([outcome_code(*outcome)]);
        }
    }
    h.update([match actual.hard {
        LocalHardStatus::NotRejected => 0,
        LocalHardStatus::Failed => 1,
        LocalHardStatus::Pending => 2,
    }]);
    for n in [
        actual.counters.adapter_calls,
        actual.counters.serializations,
        actual.counters.property_checks,
        actual.counters.soft_checks,
    ] {
        h.update((n as u64).to_le_bytes());
    }
    finish(h)
}
fn binding_digest(report: &ComponentReport) -> Digest32 {
    let mut h = start(b"cr-break.full-measurement-binding.v1\0");
    for d in [
        report.definition,
        report.binding.scope,
        report.binding.candidate,
        report.binding.source,
        report.binding.target,
        report.binding.context,
        report.binding.baseline,
        report.binding.producer,
        report.binding.transform,
        report.binding.result,
        report.probe_set,
        report.oracle,
        report.dependency_group,
    ] {
        h.update(d.as_bytes());
    }
    finish(h)
}
fn digest(b: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(b).into())
}
fn outcome_code(o: PropertyOutcome) -> u8 {
    match o {
        PropertyOutcome::Preserved => 0,
        PropertyOutcome::ConfirmedViolation => 1,
        PropertyOutcome::NotObserved(NotObservedReason::SoftCheckBudget) => 2,
        PropertyOutcome::NotObserved(NotObservedReason::PropertyBudget) => 3,
        PropertyOutcome::NotObserved(NotObservedReason::ProducerFailure) => 4,
    }
}
