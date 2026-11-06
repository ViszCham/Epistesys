//! Bounded claims and the existing actual root meter,not another resettable ledger.
use super::*;
pub(super) const PLAN: u64 = 2 * 1024 * 1024;
pub(super) const ASSESSMENT: u64 = 4 * 1024 * 1024;
pub(super) const SCRATCH: u64 = 8 * 1024 * 1024;
pub(super) fn limits(l: Limits) -> Result<(), Error> {
    if l.max_features == 0
        || l.max_features > 256
        || l.max_entries > 64
        || l.max_comparisons > 64
        || l.max_archive_dispatches > 4096
        || l.max_assessments == 0
        || l.max_assessments > 4
        || l.max_current_checks == 0
        || l.max_current_checks > 16
        || l.max_feature_bytes == 0
        || l.max_feature_bytes > 4096
        || l.max_payload_bytes == 0
        || l.max_payload_bytes > 1_048_576
        || l.max_weight_bits == 0
        || l.max_weight_bits > 1024
        || l.max_intermediate_bits == 0
        || l.max_intermediate_bits > 4096
        || l.plan_ram == 0
        || l.plan_ram > PLAN
        || l.assessment_ram == 0
        || l.assessment_ram > ASSESSMENT
        || l.scratch_ram == 0
        || l.scratch_ram > SCRATCH
        || l.output_bytes == 0
        || l.output_bytes > 65536
    {
        return Err(Error::InvalidLimits);
    }
    if l.plan_ram < PLAN || l.assessment_ram < ASSESSMENT || l.scratch_ram < SCRATCH {
        return Err(Error::RamBudget);
    }
    // Resource admission ONLY;these constants cannot produce signature contents.
    if l.max_feature_bytes < 296 || l.max_payload_bytes < 481 {
        return Err(Error::PayloadBudget);
    }
    Ok(())
}
pub(super) fn debit(root: &f::FrameworkRoot<'_, '_>, n: usize) -> Result<(), Error> {
    f::bounds::debit(&root.native, n).map_err(framework)
}
/// The native root meter enforces every nested predebit, including P1 guards.
pub(super) fn with_call<T>(root: &f::FrameworkRoot<'_, '_>, body: impl FnOnce() -> T) -> T {
    root.native
        .meter
        .with_call(root.native.state.declaration.limits.max_call_work, body)
}
pub(super) fn vector<T>(n: usize, cap: usize) -> Result<Vec<T>, Error> {
    f::bounds::reserve(n, cap).map_err(framework)
}
pub(super) fn claim<'v>(
    v: &'v ResourceReservationView<'_>,
    ram: u64,
    output: Option<u64>,
) -> Result<ResourceClaim<'v>, Error> {
    f::bounds::claim(v, ram, output).map_err(framework)
}
pub(super) fn current(
    c: &ResourceClaim<'_>,
    v: &ResourceReservationView<'_>,
    source: Digest32,
) -> Result<(), Error> {
    f::bounds::current(c, v, source).map_err(framework)
}
pub(super) fn native_limits(root: &f::FrameworkRoot<'_, '_>, l: Limits) -> n::Limits {
    let mut out = root.native.state.declaration.limits;
    out.max_features = l.max_features;
    out.max_ast_nodes = 128;
    out.max_depth = 32;
    out.max_payload_bytes = l.max_payload_bytes;
    out.max_weight_bits = l.max_weight_bits;
    out.max_intermediate_bits = l.max_intermediate_bits;
    out
}
pub(super) fn weight(value: &Rational, l: Limits, w: &mut Work<'_>) -> Result<(), Error> {
    let bits = value.numer().bits().max(value.denom().bits());
    if bits > u64::from(l.max_weight_bits) {
        return Err(Error::NumericBudget);
    }
    w.charge(
        bits.checked_mul(8)
            .and_then(|v| v.checked_add(64))
            .ok_or(Error::WorkBudget)?,
    )
    .map_err(native)?;
    numeric::check(value).map_err(Error::Numeric)?;
    if value.numer() <= &0.into() {
        return Err(Error::InvalidWeight);
    }
    Ok(())
}
