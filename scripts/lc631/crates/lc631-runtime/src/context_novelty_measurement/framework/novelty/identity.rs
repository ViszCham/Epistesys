//! Root/pair/code/leaf lineage is historical;pointer authority is checked separately.
use super::*;
use sha2::{Digest, Sha256};
pub(super) fn root(
    fw: &f::FrameworkRoot<'_, '_>,
    r: &Registration<'_>,
    u: &n::Universe,
    s: &signature::Signature,
    w: &mut Work<'_>,
) -> Result<(Digest32, Digest32), Error> {
    let mut producer = Sha256::new();
    producer.update(b"tlcx.framework.p2.actual.v1\0");
    for unit in [
        include_bytes!("../novelty.rs").as_slice(),
        include_bytes!("types.rs").as_slice(),
        include_bytes!("admission.rs").as_slice(),
        include_bytes!("budget.rs").as_slice(),
        include_bytes!("construction.rs").as_slice(),
        include_bytes!("signature.rs").as_slice(),
        include_bytes!("archive.rs").as_slice(),
        include_bytes!("archive_native.rs").as_slice(),
        include_bytes!("current.rs").as_slice(),
        include_bytes!("identity.rs").as_slice(),
        include_bytes!("measure.rs").as_slice(),
        include_bytes!("plan.rs").as_slice(),
    ] {
        w.charge(unit.len() as u64 + 1).map_err(native)?;
        producer.update((unit.len() as u64).to_le_bytes());
        producer.update(unit);
    }
    let producer = Digest32::from_bytes(producer.finalize().into());
    let mut h = n::native_hash::new(w, budget::native_limits(fw, r.limits));
    h.bytes(b"p2-same-source-positive-corpus.v1")
        .map_err(native)?;
    for pin in [
        fw.digest,
        producer,
        r.corpus_revision,
        r.definition_revision,
        r.oracle_revision,
        r.policy_revision,
    ] {
        h.bytes(pin.as_bytes()).map_err(native)?;
    }
    for universe in [u, &s.universe] {
        for feature in &universe.features {
            h.bytes(&feature.payload).map_err(native)?;
            h.rational(&feature.weight).map_err(native)?;
        }
    }
    match r.archive {
        ArchiveInput::Unread => h.n(0).map_err(native)?,
        ArchiveInput::Unavailable => h.n(1).map_err(native)?,
        ArchiveInput::Read(entries) => {
            h.n(2).map_err(native)?;
            h.n(entries.len() as u64).map_err(native)?;
            for e in entries {
                h.n(e.kernel as u64).map_err(native)?;
                h.bytes(e.source.id().digest().as_bytes()).map_err(native)?;
                h.query(e.query).map_err(native)?;
                for a in e.atoms {
                    h.n(u64::from(a.local.get())).map_err(native)?;
                    h.n(u64::from(a.meaning)).map_err(native)?;
                    h.n(a.grounding.start() as u64).map_err(native)?;
                    h.n(a.grounding.end() as u64).map_err(native)?;
                }
            }
        }
    }
    let l = r.limits;
    for v in [
        l.max_features as u64,
        l.max_entries as u64,
        l.max_comparisons as u64,
        l.max_archive_dispatches as u64,
        l.max_assessments as u64,
        l.max_current_checks as u64,
        l.max_feature_bytes as u64,
        l.max_payload_bytes as u64,
        u64::from(l.max_weight_bits),
        u64::from(l.max_intermediate_bits),
        l.plan_ram,
        l.assessment_ram,
        l.scratch_ram,
        l.output_bytes,
    ] {
        h.n(v).map_err(native)?;
    }
    Ok((h.finish(), producer))
}
fn case(root: &Root<'_, '_>, c: CaseRef<'_, '_, '_>, w: &mut Work<'_>) -> Result<Digest32, Error> {
    let mut h = n::native_hash::new(
        w,
        budget::native_limits(&root.framework, root.registration.limits),
    );
    h.bytes(b"actual-framework-case.v2").map_err(native)?;
    h.n(c.expected.kernel as u64).map_err(native)?;
    h.n(match plan::case_key(c).subject {
        f::SubjectKind::Original => 0,
        f::SubjectKind::Generated => 1,
    })
    .map_err(native)?;
    let context = match c.expected.subject {
        f::Subject::Original => root.framework.native.state.context,
        f::Subject::Generated(c) => c.prepared().target(),
    };
    h.bytes(context.interpretation_digest().as_bytes())
        .map_err(native)?;
    h.query(c.expected.subject_query).map_err(native)?;
    Ok(h.finish())
}
fn comparison_baseline(
    root: &Root<'_, '_>,
    reference: Digest32,
    role: plan::ComparisonRole,
    w: &mut Work<'_>,
) -> Result<Digest32, Error> {
    let mut h = n::native_hash::new(
        w,
        budget::native_limits(&root.framework, root.registration.limits),
    );
    h.bytes(b"tlcx.framework.p2.reference-role.v1").map_err(native)?;
    h.bytes(root.digest.as_bytes()).map_err(native)?;
    h.bytes(reference.as_bytes()).map_err(native)?;
    h.n(match role {
        plan::ComparisonRole::Primary => 0,
        plan::ComparisonRole::DiagnosticControl => 1,
    })
    .map_err(native)?;
    Ok(h.finish())
}
pub(super) fn reports(
    root: &Root<'_, '_>,
    e: ExpectedPair<'_, '_, '_>,
    states: [ComponentState; 2],
    archive: &archive::Data<'_, '_>,
    union: &Rational,
    w: &mut Work<'_>,
) -> Result<[ComponentReport; 2], Error> {
    let target = case(root, e.target, w)?;
    let reference = case(root, e.source, w)?;
    let baseline = comparison_baseline(root, reference, root.comparison_role(e)?, w)?;
    let context = match e.target.expected.subject {
        f::Subject::Original => root.framework.native.state.context,
        f::Subject::Generated(c) => c.prepared().target(),
    };
    let mut result = n::native_hash::new(
        w,
        budget::native_limits(&root.framework, root.registration.limits),
    );
    for d in [root.digest, root.producer, target, reference, baseline] {
        result.bytes(d.as_bytes()).map_err(native)?;
    }
    let c = archive.counts;
    for v in [
        c.archive_entries_seen,
        c.archive_comparisons,
        c.native_archive_dispatches,
        c.native_archive_store_entries,
    ] {
        result.n(v as u64).map_err(native)?;
    }
    result
        .n(match archive.status {
            ArchiveStatus::Complete => 0,
            ArchiveStatus::Empty => 1,
            ArchiveStatus::Unread => 2,
            ArchiveStatus::Unavailable => 3,
            ArchiveStatus::Unsupported => 4,
            ArchiveStatus::NoComparable => 5,
            ArchiveStatus::Partial => 6,
        })
        .map_err(native)?;
    result.rational(union).map_err(native)?;
    for weight in &archive.unions {
        result.rational(weight).map_err(native)?;
    }
    for state in &states {
        bind_state(state, &mut result)?;
    }
    for held in &archive.held {
        held.bind(&mut result)?;
    }
    let result = result.finish();
    let mut make =
        |metric: ComponentMetric, state: ComponentState| -> Result<ComponentReport, Error> {
            w.charge(512).map_err(native)?;
            let mut h = Sha256::new();
            h.update(root.producer.as_bytes());
            h.update((metric as u16).to_le_bytes());
            h.update(reference.as_bytes());
            h.update(target.as_bytes());
            h.update(baseline.as_bytes());
            let producer = Digest32::from_bytes(h.finalize().into());
            Ok(ComponentReport {
                metric,
                definition: root.registration.definition_revision,
                binding: MeasurementBinding {
                    scope: root.digest,
                    candidate: target,
                    source: root.framework.native.state.source.id().digest(),
                    target,
                    context: context.interpretation_digest(),
                    baseline,
                    producer,
                    transform: root.registration.policy_revision,
                    result,
                },
                probe_set: root.digest,
                oracle: root.registration.oracle_revision,
                dependency_group: root.producer,
                evidence_payload: result,
                basis: InputBasis::Given,
                validation: SemanticValidation::Unassessed,
                independence: IndependenceStatus::SharedDependency,
                state,
            })
        };
    let [dpar, knownness] = states;
    Ok([
        make(ComponentMetric::FrameworkDistance, dpar)?,
        make(ComponentMetric::Knownness, knownness)?,
    ])
}
fn bind_state(state: &ComponentState, h: &mut n::native_hash::Hash<'_, '_>) -> Result<(), Error> {
    match state {
        ComponentState::Measured(v) => {
            h.n(0).map_err(native)?;
            h.rational(v.lower()).map_err(native)?;
            h.rational(v.upper()).map_err(native)?;
        }
        ComponentState::Unknown(MeasurementGap::MissingObservation) => h.n(1).map_err(native)?,
        ComponentState::Unknown(MeasurementGap::Unsupported) => h.n(2).map_err(native)?,
        ComponentState::Undefined(MeasurementError::EmptyCorpus) => h.n(3).map_err(native)?,
        ComponentState::ResourceExhausted { partial: Some(v) } => {
            h.n(4).map_err(native)?;
            h.rational(v.lower()).map_err(native)?;
            h.rational(v.upper()).map_err(native)?;
        }
        ComponentState::Incomparable { scope } => {
            h.n(5).map_err(native)?;
            h.bytes(scope.as_bytes()).map_err(native)?;
        }
        _ => return Err(Error::ExpectedInputMismatch),
    }
    Ok(())
}
