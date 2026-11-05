//! Fixed finite admission and shared root nonrefundable accounting, no new meter.
use super::*;
pub(super) const PLAN_RAM: u64 = 2 * 1024 * 1024;
pub(super) const CASE_RAM: u64 = 2 * 1024 * 1024;
pub(super) const SCRATCH_RAM: u64 = 8 * 1024 * 1024;
pub(super) fn limits(l: Limits, atoms: usize) -> Result<(), Error> {
    if l.max_atoms > 3 || atoms > 3 || atoms > usize::from(l.max_atoms) {
        return Err(Error::UnsupportedProfile);
    }
    if l.max_models == 0
        || l.max_models > 64
        || l.max_cases == 0
        || l.max_cases > 4
        || l.max_queries == 0
        || l.max_queries > 32
        || l.max_lowered_nodes == 0
        || l.max_lowered_nodes > 128
        || l.max_native_dispatches_per_case == 0
        || l.max_native_dispatches_per_case > 2048
        || l.max_current_checks == 0
        || l.max_current_checks > 16
        || l.plan_ram == 0
        || l.plan_ram > PLAN_RAM
        || l.case_ram == 0
        || l.case_ram > CASE_RAM
        || l.scratch_ram == 0
        || l.scratch_ram > SCRATCH_RAM
        || l.output_bytes == 0
        || l.output_bytes > 65536
    {
        return Err(Error::InvalidLimits);
    }
    // Whole profile includes n=3 even when this source has fewer atoms.
    if l.max_models < 27 {
        return Err(Error::ModelBudget);
    }
    if l.plan_ram < PLAN_RAM || l.case_ram < CASE_RAM || l.scratch_ram < SCRATCH_RAM {
        return Err(Error::RamBudget);
    }
    Ok(())
}
pub(super) fn reserve<T>(n: usize, cap: usize) -> Result<Vec<T>, Error> {
    if n > cap {
        return Err(Error::PayloadBudget);
    }
    let _ = n
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(Error::PayloadBudget)?;
    let mut v = Vec::new();
    v.try_reserve_exact(n).map_err(|_| Error::PayloadBudget)?;
    Ok(v)
}
pub(super) fn debit(root: &RootPlan<'_, '_>, work: usize) -> Result<(), Error> {
    root.meter
        .debit(u64::try_from(work).map_err(|_| Error::WorkBudget)?)
        .map_err(context)
}
pub(super) fn claim<'v>(
    view: &'v ResourceReservationView<'_>,
    ram: u64,
    output: Option<u64>,
) -> Result<ResourceClaim<'v>, Error> {
    resources::claim(view, ram, output).map_err(context)
}
pub(super) fn current(
    claim: &ResourceClaim<'_>,
    view: &ResourceReservationView<'_>,
    source: Digest32,
) -> Result<(), Error> {
    resources::current(claim, view, source).map_err(context)
}
pub(super) fn models(kernel: Kernel, atoms: usize) -> Result<usize, Error> {
    if atoms > 3 {
        return Err(Error::UnsupportedProfile);
    }
    let radix: usize = match kernel {
        Kernel::ClassicalPointV1 => 2,
        Kernel::PersistentChain2V1 => 3,
    };
    radix
        .checked_pow(u32::try_from(atoms).map_err(|_| Error::ModelBudget)?)
        .ok_or(Error::ModelBudget)
}
