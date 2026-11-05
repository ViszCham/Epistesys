//! 日本語: 実際に検査済みの共通正規化policy。候補固有のgraphやcostは含めない。
//! English: Common normalization policy from actual validated inputs, excluding
//! candidate-specific graphs/costs. Reported policy digests alone cannot bind it.
use super::*;
/// Called only after registry::freeze validated κ/unit/budget, while the producer's
/// real scratch claim remains live. This is an identity, not semantic authority.
/// dependency_group is candidate evidence provenance, not a shared policy pin.
pub(super) fn derive(
    plan: &RegistryPlan<'_>,
    model: &registry::FrozenModel,
) -> Result<Digest32, Error> {
    let b = plan.binding;
    let p = b.selection;
    let mut h = Sha256::new();
    h.update(b"CR-BMEM.actual-common-normalization-policy.v1");
    h.update(b"finite-formal-v1.4/exact-canonical-rational/nonnegative-interval-cost");
    h.update(b"finite-given-model/no-self-transitions/two-endpoint-dijkstra");
    h.update(b"lex-sequence-semantic-edge-keys/simple-optimal-witness/zero-cycle.v1");
    for d in [
        p.source.digest(),
        p.scope.comparison(),
        p.scope.goal(),
        p.scope.probes(),
        p.scope.semantics(),
        p.baseline,
        p.hard_invariants,
        p.policy,
        p.evidence_revision,
        b.definition,
        b.probe_set,
        b.oracle,
        plan.unit.0,
    ] {
        registry::hash_field(&mut h, d.as_bytes());
    }
    match p.goal_profile {
        Some(d) => {
            h.update([1]);
            h.update(d.as_bytes());
        }
        None => h.update([0]),
    };
    registry::hash_rational(&mut h, &model.kappa)?;
    for n in [
        plan.budget.max_steps,
        numeric::MAX_RATIONAL_BITS,
        registry::MAX_STATES as u64,
        registry::MAX_PAIRS as u64,
        registry::MAX_PAYLOAD as u64,
        SCRATCH_RAM,
        RETAINED_RAM,
        RETAINED_OUTPUT,
    ] {
        h.update(n.to_le_bytes());
    }
    Ok(Digest32::from_bytes(h.finalize().into()))
}
