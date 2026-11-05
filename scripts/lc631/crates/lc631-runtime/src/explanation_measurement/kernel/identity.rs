//! Canonical bounded lineage, never an authenticated receipt or semantic ranking.
use super::*;
use lc631_solvers::typed_query::{BoolExpr, SmtStatusQuery};
pub(crate) fn field(h: &mut Sha256, bytes: &[u8]) {
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
}
pub(crate) fn hash_binding(h: &mut Sha256, b: Binding, l: Limits) {
    for d in [
        b.selection.source.digest(),
        b.context.interpretation_digest(),
        b.selection.scope.comparison(),
        b.selection.scope.goal(),
        b.selection.scope.probes(),
        b.selection.scope.semantics(),
        b.selection.baseline,
        b.selection.hard_invariants,
        b.selection.policy,
        b.selection.evidence_revision,
        b.suite,
        b.oracle,
        b.definition,
        b.probe_set,
        b.dependency_group,
    ] {
        h.update(d.as_bytes());
    }
    match b.selection.goal_profile {
        Some(d) => {
            h.update([1]);
            h.update(d.as_bytes());
        }
        None => h.update([0]),
    }
    h.update([match b.role {
        DatasetRole::Training => 0,
        DatasetRole::Search => 1,
        DatasetRole::ExplanationValidation => 2,
        DatasetRole::TransferValidation => 3,
        DatasetRole::FinalResearchHoldout => 4,
    }]);
    h.update(l.max_case_work.to_le_bytes());
    h.update(l.max_run_work.to_le_bytes());
    h.update(l.max_dispatches.to_le_bytes());
    h.update(l.max_split_work.to_le_bytes());
}
pub(crate) fn hash_candidate(h: &mut Sha256, b: CandidateBinding) {
    for d in [
        b.candidate_digest,
        b.context.interpretation_digest(),
        b.target,
        b.transform,
    ] {
        h.update(d.as_bytes());
    }
    // Opaque candidate IDs do not become meaning/ranking coordinates.
}
pub(crate) fn hash_formula(h: &mut Sha256, expr: &BoolExpr) {
    match expr {
        BoolExpr::Constant(value) => h.update([0, u8::from(*value)]),
        BoolExpr::Variable(id) => {
            h.update([1]);
            h.update(id.get().to_le_bytes());
        }
        BoolExpr::Not(child) => {
            h.update([2]);
            hash_formula(h, child);
        }
        BoolExpr::And(children) | BoolExpr::Or(children) => {
            h.update([if matches!(expr, BoolExpr::And(_)) {
                3
            } else {
                4
            }]);
            h.update((children.len() as u16).to_le_bytes());
            for child in children {
                hash_formula(h, child);
            }
        }
        // Only admitted Boolean fragments reach hashing; no success fallback.
        BoolExpr::Equal(_, _) | BoolExpr::LessEqual(_, _) | BoolExpr::LessThan(_, _) => {
            h.update([255])
        }
    }
}
pub(crate) fn hash_theory(h: &mut Sha256, theory: &SmtStatusQuery) {
    h.update((theory.boolean_symbols().len() as u16).to_le_bytes());
    for id in theory.boolean_symbols() {
        h.update(id.get().to_le_bytes());
    }
    h.update((theory.assertions().len() as u16).to_le_bytes());
    for formula in theory.assertions() {
        hash_formula(h, formula);
    }
}
pub(crate) fn hash_rational(h: &mut Sha256, r: &Rational) {
    for n in [r.numer(), r.denom()] {
        let (_, bytes) = n.to_bytes_le();
        h.update([u8::from(n < &numeric::BigInt::from(0))]);
        field(h, &bytes);
    }
}
pub(crate) fn hash_submission(h: &mut Sha256, s: Submission<'_>) {
    match s {
        Submission::Missing => h.update([0]),
        Submission::Certificate(c) => {
            h.update([1]);
            h.update((c.steps.len() as u16).to_le_bytes());
            for step in c.steps {
                hash_formula(h, step.conclusion);
                match step.rule {
                    Rule::Premise { index } => h.update([0, index]),
                    Rule::AndElim { prior, child } => {
                        h.update([1]);
                        h.update(prior.0.to_le_bytes());
                        h.update([child]);
                    }
                    Rule::AndIntro { priors } => {
                        h.update([2]);
                        h.update((priors.len() as u16).to_le_bytes());
                        for prior in priors {
                            h.update(prior.0.to_le_bytes());
                        }
                    }
                }
            }
        }
    }
}
pub(crate) struct Pair<'a> {
    pub key: &'a [u8],
    pub weight: &'a Rational,
    pub baseline: &'a ScoreNode,
    pub candidate: &'a ScoreNode,
}
pub(crate) fn make_report(
    metric: ComponentMetric,
    b: Binding,
    c: CandidateBinding,
    input: Digest32,
    pairs: &[Pair<'_>],
) -> Result<ComponentReport, Error> {
    let mut h = Sha256::new();
    h.update(b"actual-certificate-paired-result.v1");
    h.update((metric as u16).to_le_bytes());
    h.update(input.as_bytes());
    hash_candidate(&mut h, c);
    for p in pairs {
        field(&mut h, p.key);
        hash_rational(&mut h, p.weight);
        for node in [p.baseline, p.candidate] {
            hash_rational(&mut h, node.value.lower());
            hash_rational(&mut h, node.value.upper());
        }
    }
    let result = Digest32::from_bytes(h.finalize().into());
    let mut numeric_pairs = vector(pairs.len())?;
    for p in pairs {
        let mut base = Sha256::new();
        base.update(result.as_bytes());
        field(&mut base, p.key);
        base.update([0]);
        let baseline_node = Digest32::from_bytes(base.finalize().into());
        let candidate_node = if std::ptr::eq(p.baseline, p.candidate) {
            baseline_node
        } else {
            let mut next = Sha256::new();
            next.update(result.as_bytes());
            field(&mut next, p.key);
            next.update([1]);
            Digest32::from_bytes(next.finalize().into())
        };
        numeric_pairs.push(ProbePair {
            canonical_payload: p.key,
            weight: p.weight,
            baseline: &p.baseline.value,
            candidate: &p.candidate.value,
            baseline_node,
            candidate_node,
        });
    }
    let value = paired_gain(&numeric_pairs).map_err(Error::Measurement)?;
    Ok(ComponentReport {
        metric,
        definition: b.definition,
        binding: MeasurementBinding {
            scope: b.selection.scope.comparison(),
            candidate: c.candidate_digest,
            source: b.selection.source.digest(),
            target: c.target,
            context: c.context.interpretation_digest(),
            baseline: b.selection.baseline,
            producer: b.definition,
            transform: c.transform,
            result,
        },
        probe_set: b.probe_set,
        oracle: b.oracle,
        dependency_group: b.dependency_group,
        evidence_payload: result,
        basis: InputBasis::Given,
        validation: SemanticValidation::Unassessed,
        independence: IndependenceStatus::SharedDependency,
        state: ComponentState::Measured(value),
    })
}
