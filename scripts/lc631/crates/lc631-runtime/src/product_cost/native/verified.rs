//! Private complete inventory matching and native wrapper around actual charged execution.
use super::*;
pub(super) fn execute<'v>(
    profile: &DeclaredCostProfile<'v>,
    actual: ports::Collected,
    expected: &ReportedProduct<'_>,
    package: InlinePackage<'_>,
    baseline_bits: Option<u64>,
    view: &'v ResourceReservationView<'_>,
    workspace: execution::Workspace<'v>,
) -> Result<Verified<'v>, CostError> {
    profile.check_current(view)?;
    let mut admission = machine::Meter::new(view, profile.limits.fuel_per_direction);
    let dictionary_nodes = codec::scan(package.dictionary, profile.limits, &mut admission)?;
    let raw_nodes = term::nodes(&actual.fields)?;
    let host_bound = profile
        .limits
        .bundle_bytes
        .checked_mul(12)
        .and_then(|n| n.checked_add(profile.limits.semantic_bytes.checked_mul(4)?))
        .and_then(|n| n.checked_add(raw_nodes.checked_add(dictionary_nodes)?.checked_mul(128)?))
        .and_then(|n| n.checked_add(65536))
        .ok_or(CostError::ResourceExhausted)?;
    if host_bound > 8 * 1024 * 1024 {
        return Err(CostError::ResourceExhausted);
    }
    admission.chunks(
        (package.dictionary.len() as u64)
            .checked_add(dictionary_nodes)
            .ok_or(CostError::ResourceExhausted)?,
    )?;
    let dictionary = term::decode(package.dictionary, profile.limits)?;
    let sections = term::children(&dictionary, "dictionary-v1")?;
    if sections.len() != 4 {
        return Err(CostError::InvalidProgram);
    }
    let paid = term::children(&sections[0], "declarations-v1")?;
    let forward = term::children(&actual.declarations, "declarations-v1")?;
    if paid.len() != forward.len() || paid.len() as u64 > profile.limits.local_entries {
        return Err(CostError::CorrespondenceMismatch);
    }
    // Reserve persistent rows before constructing them, while execution scratch
    // still covers the parsed input. Every full entry is compared, including unused.
    let row_overhead = paid
        .len()
        .checked_mul(256)
        .and_then(|n| n.checked_add(4096))
        .ok_or(CostError::ResourceExhausted)?;
    let row_bytes = package
        .dictionary
        .len()
        .checked_mul(4)
        .and_then(|n| n.checked_add(expected.identity_envelope_bytes().len()))
        .and_then(|n| n.checked_add(row_overhead))
        .ok_or(CostError::ResourceExhausted)?;
    let rows_claim = claim(
        view,
        u64::try_from(row_bytes).map_err(|_| CostError::ResourceExhausted)?,
    )?;
    let mut rows = Vec::new();
    let mut previous: Option<(&str, &str)> = None;
    for entry in paid {
        let p = term::children(entry, "declaration-v1")?;
        if p.len() != 3 {
            return Err(CostError::CorrespondenceMismatch);
        }
        let key = (term::symbol(&p[0])?, term::symbol(&p[1])?);
        if let Some(old) = previous {
            admission.chunks((old.0.len() + old.1.len() + key.0.len() + key.1.len()) as u64)?;
        }
        if previous.is_some_and(|old| old >= key) {
            return Err(CostError::CorrespondenceMismatch);
        }
        previous = Some(key);
        let mut matched = None;
        for candidate in forward {
            if codec::equal_term(candidate, entry, &mut admission)?
                && matched.replace(candidate).is_some()
            {
                return Err(CostError::CorrespondenceMismatch);
            }
        }
        let matched = matched.ok_or(CostError::CorrespondenceMismatch)?;
        let f = term::children(matched, "declaration-v1")?;
        admission.chunks(term::work(&f[2], profile.limits)?)?;
        admission.chunks(term::work(&p[2], profile.limits)?)?;
        rows.push(MeaningCorrespondence {
            kind: key.0.to_owned(),
            path: key.1.to_owned(),
            forward: term::encode(&f[2], profile.limits)?,
            dictionary: term::encode(&p[2], profile.limits)?,
        });
    }
    admission.chunks(term::work(&sections[0], profile.limits)?)?;
    let declarations = term::encode(&sections[0], profile.limits)?;
    admission.chunks(expected.identity_envelope_bytes().len() as u64)?;
    let raw = term::copy(expected.identity_envelope_bytes())?;
    let mut ledger = NativeIdentityAdmissionCost {
        work: actual.admission_work,
        raw,
        revision: digest(
            b"product-cost.native-operational-admission.v1",
            b"whole-plan opaque equality + full native current inputs",
        ),
    };
    // Retain a bounded structural copy solely for AFTER-execution reference
    // comparison. It never supplies A's output or any canonical product input.
    let verification_bytes = term::length(&actual.fields, profile.limits)?
        .checked_mul(128)
        .and_then(|n| n.checked_add(4096))
        .ok_or(CostError::ResourceExhausted)?;
    let verification_claim = claim(view, verification_bytes)?;
    admission.chunks(term::work(&actual.fields, profile.limits)?)?;
    let raw_reference = actual.fields.clone();
    let lower = execution::execute_fields(
        profile,
        actual.fields,
        actual.declarations,
        package,
        view,
        workspace,
        admission.used,
    )?;
    lower.check_current(view)?;
    let byte_work = validation::bytes_equal(
        lower.historical_report().semantic_bytes(),
        expected.semantic_bytes(),
        view,
    )?;
    let verification_work = validation::check(&raw_reference, expected.term(), view)?;
    ledger.work = ledger
        .work
        .checked_add(verification_work)
        .and_then(|n| n.checked_add(byte_work))
        .ok_or(CostError::ResourceExhausted)?;
    verification_claim.check()?;
    drop(raw_reference);
    drop(verification_claim);
    let closure_work =
        super::super::programs::require_native_adapter_closure(package, profile.limits, view)?;
    ledger.work = ledger
        .work
        .checked_add(closure_work)
        .ok_or(CostError::ResourceExhausted)?;
    // Charge the owned report clone BEFORE allocation. Persistent rows have their
    // separate nonzero claim; neither owner keeps the 32MiB execution workspace.
    let retained = claim(view, lower.historical_report().retained_bytes()?)?;
    let mut report = lower.historical_report().clone();
    report.baseline_bits = baseline_bits;
    report.basis = InputBasis::ActualOwnerObservedWithGivenMeanings;
    report.independence = IndependenceStatus::SharedDependency;
    report.native_gap = None;
    report.evidence_gaps = vec![
        EvidenceGap::NativeGoalPinMissing,
        EvidenceGap::HardInvariantPinMissing,
        EvidenceGap::EvidencePinMissing,
    ];
    report.declarations = declarations;
    // Bind native applicability/current admission separately from semantic bundle K.
    // Opaque admission changes this report binding, never bundle bytes or semantic D.
    let mut h = sha2::Sha256::new();
    use sha2::Digest;
    h.update(b"product-cost.actual-native-current.v1\0");
    h.update(report.binding.as_bytes());
    h.update(profile.digest.as_bytes());
    h.update(actual.root.as_bytes());
    h.update(&ledger.raw);
    let c = actual.catalog;
    let p = c.selection;
    for pin in [
        p.source.digest(),
        p.scope.comparison(),
        p.scope.goal(),
        p.scope.probes(),
        p.scope.semantics(),
        p.baseline,
        p.hard_invariants,
        p.policy,
        p.evidence_revision,
        c.context.interpretation_digest(),
        c.suite,
        c.attack_scope,
        c.oracle,
        c.definition,
        c.probe_set,
        c.dependency_group,
    ] {
        h.update(pin.as_bytes());
    }
    match p.goal_profile {
        None => h.update([0]),
        Some(pin) => {
            h.update([1]);
            h.update(pin.as_bytes());
        }
    }
    h.update(b"SoftFiniteValidation/AllAdmissibleAssignments");
    report.binding = Digest32::from_bytes(h.finalize().into());
    report.stats.meaning_declarations = rows.len() as u64;
    report.stats.matched_dictionary_declarations = rows.len() as u64;
    rows_claim.check()?;
    retained.check()?;
    lower.check_current(view)?;
    Ok(Verified {
        report,
        ledger,
        rows,
        lower,
        retained,
        rows_claim,
    })
}
fn digest(domain: &[u8], bytes: &[u8]) -> Digest32 {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(domain);
    h.update(bytes);
    Digest32::from_bytes(h.finalize().into())
}
