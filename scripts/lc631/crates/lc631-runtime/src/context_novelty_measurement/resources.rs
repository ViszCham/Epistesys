//! Scoped logical payload claims, not RSS/OOM/global enforcement evidence.
use super::*;
use lc631_substrate::resource::{ResourceAmount, ResourceKind, ResourceRequest};
pub(super) fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    ram: u64,
    output: Option<u64>,
) -> Result<ResourceClaim<'v>, Error> {
    let request = |kind, n| {
        ResourceAmount::checked(n)
            .map(|a| ResourceRequest::new(kind, a))
            .map_err(Error::Resource)
    };
    let r = request(ResourceKind::Ram, ram)?;
    match output {
        Some(n) => view.claim_resources(&[r, request(ResourceKind::Output, n)?]),
        None => view.claim_resources(&[r]),
    }
    .map_err(Error::Resource)
}
pub(super) fn current(
    claim: &ResourceClaim<'_>,
    view: &ResourceReservationView<'_>,
    source: Digest32,
) -> Result<(), Error> {
    claim.check().map_err(Error::Resource)?;
    if !claim.same_reservation(view) {
        return Err(Error::ForeignReservation);
    }
    if view.source_revision() != source {
        return Err(Error::SourceMismatch);
    }
    view.check().map_err(Error::Resource)
}
pub(super) fn vector<T>(n: usize, max: usize, work: &mut Work<'_>) -> Result<Vec<T>, Error> {
    if n > max {
        return Err(Error::PayloadBudget);
    }
    let bytes = n
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(Error::PayloadBudget)?;
    work.charge(
        u64::try_from(bytes)
            .map_err(|_| Error::PayloadBudget)?
            .checked_add(1)
            .ok_or(Error::WorkBudget)?,
    )?;
    let mut out = Vec::new();
    out.try_reserve_exact(n)
        .map_err(|_| Error::Arithmetic(MeasurementError::AllocationFailed))?;
    Ok(out)
}
pub(super) fn limits(v: Limits) -> Result<(), Error> {
    if v.max_atoms > 8
        || v.max_assertions == 0
        || v.max_assertions > 16
        || v.max_features == 0
        || v.max_features > 256
        || v.max_archive_entries > 64
        || v.max_ast_nodes == 0
        || v.max_ast_nodes > 512
        || v.max_depth == 0
        || v.max_depth > 32
        || v.max_payload_bytes == 0
        || v.max_payload_bytes > 1_048_576
        || v.max_call_work == 0
        || v.max_call_work > 1_000_000
        || v.max_total_work == 0
        || v.max_total_work > 8_000_000
        || v.max_archive_comparisons > 64
        || v.max_weight_bits == 0
        || v.max_weight_bits > 1024
        || v.max_intermediate_bits == 0
        || v.max_intermediate_bits > 4096
        || v.retained_ram == 0
        || v.retained_ram > 4 * MIB
        || v.scratch_ram == 0
        || v.scratch_ram > 8 * MIB
        || v.output_bytes == 0
        || v.output_bytes > 65536
    {
        return Err(Error::InvalidLimits);
    }
    Ok(())
}
pub(super) fn root_capacity(v: Limits) -> Result<(), Error> {
    if v.retained_ram < 4 * MIB || v.scratch_ram < 8 * MIB {
        return Err(Error::Resource(ResourceError::Exhausted(
            lc631_substrate::resource::ResourceKind::Ram,
        )));
    }
    Ok(())
}
