use super::*;
use lc631_substrate::{
    resource::{ResourceClaim, ResourceReservationView},
    Digest32,
};
pub struct DeclaredCostProfile<'v> {
    pub(super) limits: CostLimits,
    pub(super) runtime_bytes: Vec<u8>,
    pub(super) digest: Digest32,
    retained: ResourceClaim<'v>,
    source: Digest32,
}
impl DeclaredCostProfile<'_> {
    pub fn digest(&self) -> Digest32 {
        self.digest
    }
    pub(super) fn check_alive(&self) -> Result<(), CostError> {
        Ok(self.retained.check()?)
    }
    pub(super) fn check_current(
        &self,
        view: &ResourceReservationView<'_>,
    ) -> Result<(), CostError> {
        view.check()?;
        self.retained.check()?;
        if !self.retained.same_reservation(view) {
            return Err(CostError::ForeignOwner);
        }
        if self.source != view.source_revision() {
            return Err(CostError::ForeignOwner);
        }
        Ok(())
    }
}
pub fn declare_cost_profile<'v>(
    spec: ProfileSpec<'_>,
    view: &'v ResourceReservationView<'_>,
) -> Result<DeclaredCostProfile<'v>, CostError> {
    view.check()?;
    let limits = spec.limits.checked()?;
    if spec.goal == SemanticGoal::ExactNativeIdentity {
        return Err(CostError::NeedsEvidence(CostGap::NeedsProfileDecision));
    }
    if spec.runtime.is_empty() {
        return Err(CostError::MissingDependency);
    }
    if spec.runtime.len() as u64 > limits.bundle_bytes {
        return Err(CostError::ResourceExhausted);
    }
    let storage = (spec.runtime.len() as u64)
        .checked_mul(128)
        .and_then(|n| n.checked_add(4096))
        .ok_or(CostError::ResourceExhausted)?;
    if storage > limits.additional_ram_bytes {
        return Err(CostError::ResourceExhausted);
    }
    let retained = claim(view, storage)?;
    let mut meter = machine::Meter::new(view, limits.fuel_per_direction);
    let n = codec::scan(spec.runtime, limits, &mut meter)?;
    meter.chunks(2 * (spec.runtime.len() as u64 + n))?;
    let runtime = term::decode(spec.runtime, limits)?;
    machine::validate_runtime(&runtime, limits, &mut meter)?;
    let runtime_bytes = term::copy(spec.runtime)?;
    let mut h = Sha256::new();
    h.update(b"CopyPackTree-v1/SemanticLocalRoles/B_boot-k6-tree-v1/phase-fuel-v1");
    for n in [
        limits.semantic_bytes,
        limits.bundle_bytes,
        limits.total_nodes,
        limits.instructions,
        limits.local_entries,
        limits.value_cells,
        limits.fuel_per_direction,
        limits.iteration_bound,
        limits.depth,
        limits.additional_ram_bytes,
    ] {
        h.update(n.to_le_bytes())
    }
    h.update(&runtime_bytes);
    let digest = Digest32::from_bytes(h.finalize().into());
    retained.check()?;
    view.check()?;
    Ok(DeclaredCostProfile {
        limits,
        runtime_bytes,
        digest,
        retained,
        source: view.source_revision(),
    })
}
