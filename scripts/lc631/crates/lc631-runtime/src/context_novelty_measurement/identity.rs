//! Source/code/root/result bindings; none authenticate Given source semantics.
use super::*;
use sha2::{Digest, Sha256};
pub(super) fn query_shape(q: &SmtStatusQuery, l: Limits, w: &mut Work<'_>) -> Result<(), Error> {
    let mut h = super::native_hash::new(w, l);
    h.query(q)
}
pub(super) fn producer(w: &mut Work<'_>) -> Result<Digest32, Error> {
    let mut h = Sha256::new();
    h.update(b"epistesys.context-novelty.actual-owner.v1\0");
    // Complete source bodies, including this identity implementation, are bound.
    for unit in [
        include_str!("../context_novelty_measurement.rs"),
        include_str!("api.rs"),
        include_str!("archive.rs"),
        include_str!("budget.rs"),
        include_str!("canonical.rs"),
        include_str!("features.rs"),
        include_str!("identity.rs"),
        include_str!("measure.rs"),
        include_str!("native_hash.rs"),
        include_str!("numeric.rs"),
        include_str!("owned.rs"),
        include_str!("plan.rs"),
        include_str!("resources.rs"),
        include_str!("validate.rs"),
        include_str!("../creative_candidate_bridge.rs"),
        include_str!("../creative_candidate_bridge/admission.rs"),
        include_str!("../creative_candidate_bridge/generate.rs"),
        include_str!("../creative_candidate_bridge/validator.rs"),
        include_str!("../creative_candidate_bridge/measurement.rs"),
        include_str!("../../../lc631-analysis/src/creativity_measurement/structure.rs"),
        include_str!("../../../lc631-substrate/src/measurement_numeric/operations.rs"),
        include_str!("../../../lc631-substrate/src/source.rs"),
    ] {
        w.charge(unit.len() as u64 + 1)?;
        h.update((unit.len() as u64).to_le_bytes());
        h.update(unit.as_bytes());
    }
    Ok(Digest32::from_bytes(h.finalize().into()))
}
pub(super) fn root(
    input: &RootInput<'_>,
    d: &Declaration<'_>,
    facts: &Universe,
    constructions: &Universe,
    producer: Digest32,
    w: &mut Work<'_>,
) -> Result<Digest32, Error> {
    let mut h = super::native_hash::new(w, d.limits);
    h.bytes(b"root-fixed-finite-boolean-construction-v2")?;
    for digest in [
        producer,
        input.source.id().digest(),
        input.original.state().revision().interpretation_digest(),
        input.target,
        input.goal.revision,
        d.corpus_revision,
        d.oracle_revision,
        d.policy_revision,
    ] {
        h.bytes(digest.as_bytes())?;
    }
    let scope = input.original.state().scope();
    for digest in [
        scope.comparison(),
        scope.goal(),
        scope.probes(),
        scope.semantics(),
    ] {
        h.bytes(digest.as_bytes())?;
    }
    h.query(input.source_query)?;
    h.query(input.goal.query)?;
    h.n(d.atoms.len() as u64)?;
    for m in d.atoms {
        h.n(u64::from(m.local.get()))?;
        h.n(u64::from(m.meaning))?;
        h.n(m.grounding.start() as u64)?;
        h.n(m.grounding.end() as u64)?;
    }
    premise_policy(input, &mut h)?;
    for u in [facts, constructions] {
        h.n(u.features.len() as u64)?;
        for f in &u.features {
            h.bytes(&f.payload)?;
            h.rational(&f.weight)?;
        }
    }
    match d.archive {
        ArchiveInput::Unread => h.n(0)?,
        ArchiveInput::Unavailable => h.n(1)?,
        ArchiveInput::Read(entries) => {
            h.n(2)?;
            h.n(entries.len() as u64)?;
            for e in entries {
                h.bytes(e.source.id().digest().as_bytes())?;
                h.bytes(e.context.interpretation_digest().as_bytes())?;
                h.query(e.query)?;
            }
        }
    }
    let l = d.limits;
    for n in [
        u64::from(l.max_atoms),
        l.max_assertions as u64,
        l.max_features as u64,
        l.max_archive_entries as u64,
        l.max_ast_nodes as u64,
        l.max_depth as u64,
        l.max_payload_bytes as u64,
        l.max_call_work,
        l.max_total_work,
        l.max_archive_comparisons as u64,
        u64::from(l.max_weight_bits),
        u64::from(l.max_intermediate_bits),
        l.retained_ram,
        l.scratch_ram,
        l.output_bytes,
        input.bridge_limits.max_candidates as u64,
        input.bridge_limits.max_ast_nodes as u64,
        u64::from(input.bridge_limits.max_validation_checks),
        input.bridge_limits.max_payload_bytes as u64,
    ] {
        h.n(n)?;
    }
    Ok(h.finish())
}

/// Core's text-profile digest deliberately ignores occurrence correspondence.
/// Bind the ACTUAL native policy used by validate::input under each given ID;
/// this commits its role without changing Core identity or asserting prose truth.
fn premise_policy(
    input: &RootInput<'_>,
    h: &mut super::native_hash::Hash<'_, '_>,
) -> Result<(), Error> {
    use lc631_core::context::PremiseBasis;
    let original = input.original.state().premises();
    if original.len() != input.premises.len() || original.len() > 16 {
        return Err(Error::ExpectedInputMismatch);
    }
    h.bytes(b"actual-native-premise-policy-v1")?;
    h.n(input.premises.len() as u64)?;
    for given in input.premises {
        h.work.charge(original.len() as u64 + 1)?;
        let actual = original
            .iter()
            .find(|p| p.id() == given.id)
            .ok_or(Error::ExpectedInputMismatch)?;
        if actual.source_span() != Some(given.span) {
            return Err(Error::ExpectedInputMismatch);
        }
        if actual.statement().len() > 8192 {
            return Err(Error::PayloadBudget);
        }
        h.bytes(actual.id().as_bytes())?;
        h.n(match actual.basis() {
            PremiseBasis::Given => 0,
            PremiseBasis::Observed => 1,
            PremiseBasis::Assumed => 2,
            PremiseBasis::Derived => 3,
            PremiseBasis::Proposed => 4,
            PremiseBasis::Refuted => 5,
            PremiseBasis::Conflicted => 6,
        })?;
        h.n(u64::from(actual.changeable()))?;
        h.bytes(actual.statement().as_bytes())?;
        h.bytes(given.span.source_id().digest().as_bytes())?;
        h.n(given.span.start() as u64)?;
        h.n(given.span.end() as u64)?;
    }
    Ok(())
}
pub(super) fn reports(
    root: &RootPlan<'_, '_>,
    candidate: &ValidatedFormalCandidate<'_, '_>,
    view: &ResourceReservationView<'_>,
    states: [ComponentState; 3],
    counts: Counts,
    status: ArchiveStatus,
    w: &mut Work<'_>,
) -> Result<([ComponentReport; 3], Counts), Error> {
    w.charge(CURRENT_WORK)?;
    let actual = candidate
        .measurement_identity_current(&root.bridge, view)
        .map_err(|e| match e {
            crate::creative_candidate_bridge::measurement::Error::Creative(e) => Error::Creative(e),
            _ => Error::ExpectedInputMismatch,
        })?;
    let s = &root.state;
    let prepared = candidate.prepared();
    let mut h = super::native_hash::new(w, s.declaration.limits);
    h.bytes(b"actual-context-novelty-result-v1")?;
    h.bytes(s.digest.as_bytes())?;
    h.bytes(actual.as_bytes())?;
    h.bytes(candidate.historical_payload())?;
    h.n(counts.archive_comparisons as u64)?;
    h.n(counts.assignment_evaluations)?;
    h.n(match status {
        ArchiveStatus::Complete => 0,
        ArchiveStatus::Empty => 1,
        ArchiveStatus::Unread => 2,
        ArchiveStatus::Unavailable => 3,
        ArchiveStatus::Unsupported => 4,
        ArchiveStatus::Partial => 5,
    })?;
    for state in &states {
        match state {
            ComponentState::Measured(v) => {
                h.n(0)?;
                h.rational(v.lower())?;
                h.rational(v.upper())?;
            }
            ComponentState::Unknown(g) => {
                h.n(1)?;
                h.n(match g {
                    MeasurementGap::Unsupported => 0,
                    MeasurementGap::MissingObservation => 1,
                    _ => return Err(Error::ExpectedInputMismatch),
                })?;
            }
            ComponentState::Undefined(MeasurementError::EmptyCorpus) => h.n(2)?,
            ComponentState::ResourceExhausted { partial: Some(v) } => {
                h.n(3)?;
                h.rational(v.lower())?;
                h.rational(v.upper())?;
            }
            _ => return Err(Error::ExpectedInputMismatch),
        }
    }
    // Pay the final fixed-width counter hash BEFORE fixing the actual count.
    h.work.charge(32)?;
    let counts = Counts {
        work_debited: h.work.used(),
        ..counts
    };
    h.hash.update(8u64.to_le_bytes());
    h.hash.update(counts.work_debited.to_le_bytes());
    let result = h.finish();
    let binding = MeasurementBinding {
        scope: s.scope.comparison(),
        candidate: actual,
        source: s.source.id().digest(),
        target: prepared.target().source().digest(),
        context: prepared.target().interpretation_digest(),
        baseline: s.digest,
        producer: s.producer,
        transform: s.declaration.policy_revision,
        result,
    };
    let [dctx, dpar, knownness] = states;
    let report = |metric, state| ComponentReport {
        metric,
        definition: definition(metric),
        binding,
        probe_set: s.digest,
        oracle: s.declaration.oracle_revision,
        dependency_group: s.source.id().digest(),
        evidence_payload: result,
        basis: InputBasis::Given,
        validation: SemanticValidation::Unassessed,
        independence: IndependenceStatus::SharedDependency,
        state,
    };
    Ok((
        [
            report(ComponentMetric::ContextDistance, dctx),
            report(ComponentMetric::FrameworkDistance, dpar),
            report(ComponentMetric::Knownness, knownness),
        ],
        counts,
    ))
}
fn definition(metric: ComponentMetric) -> Digest32 {
    let mut h = Sha256::new();
    h.update(b"finite-formal-v1.4.actual-profile.context-construction.v1\0");
    h.update((metric as u16).to_le_bytes());
    Digest32::from_bytes(h.finalize().into())
}
