//! Resource-owned formal loss. Accounting bounds are not RSS or source truth.
use super::{
    BridgeError, Candidate, Direction, MeasuredFormalLoss, ProbeLineage, ProbeObservation,
    ProbePlan, WorkBudget,
};
use lc631_substrate::measurement_numeric::MAX_RATIONAL_BITS;
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};

const SCRATCH_RAM: u64 = 8 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceBounds {
    pub scratch_ram: u64,
    pub retained_ram: u64,
    pub retained_output: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnedError {
    Resource(ResourceError),
    Measurement(BridgeError),
    BindingMismatch,
    ForeignReservation,
    ArithmeticOverflow,
    AllocationFailed,
}

/// Conservative logical accounting profile, excluding caller-owned query/AST/
/// map/observation backing, allocator headers, native stacks and total RSS.
/// K7 caps rational numerator/denominator bits, not every allocator operation.
pub fn requirements(plan: &ProbePlan<'_>) -> Result<ResourceBounds, OwnedError> {
    let count =
        u64::try_from(plan.fixed_probes().len()).map_err(|_| OwnedError::ArithmeticOverflow)?;
    if count == 0 || count > 256 {
        return Err(OwnedError::Measurement(BridgeError::ProbeLimit));
    }
    let limb = MAX_RATIONAL_BITS.div_ceil(8);
    // Fixed 8MiB profile covers a deliberately generous 128 double-cap numeric
    // buffers plus 512 bytes/probe of vectors/BTree metadata and 1MiB slack.
    let modeled_scratch = limb
        .checked_mul(256)
        .and_then(|n| {
            count
                .checked_mul(512)
                .and_then(|entries| n.checked_add(entries))
        })
        .and_then(|n| n.checked_add(1024 * 1024))
        .ok_or(OwnedError::ArithmeticOverflow)?;
    if modeled_scratch > SCRATCH_RAM {
        return Err(OwnedError::ArithmeticOverflow);
    }
    let entry = std::mem::size_of::<ProbeLineage<'static>>() as u64;
    let retained_ram = count
        .checked_mul(entry)
        .and_then(|n| {
            limb.checked_mul(4)
                .and_then(|numeric| n.checked_add(numeric))
        })
        .and_then(|n| n.checked_add(std::mem::size_of::<MeasuredFormalLoss<'static>>() as u64))
        .and_then(|n| n.checked_add(512))
        .ok_or(OwnedError::ArithmeticOverflow)?;
    Ok(ResourceBounds {
        scratch_ram: SCRATCH_RAM,
        retained_ram,
        retained_output: retained_ram,
    })
}

/// No detached measured-report constructor, mutation or Deserialize is provided.
/// Fields drop in declaration order: the owned value drops BEFORE its claim.
/// Historical reading never reactivates a cancelled/expired lease.
///
/// ```compile_fail
/// use lc631_runtime::formal_measurement::{MeasuredFormalLoss, owned::OwnedFormalLoss};
/// fn inject(report: MeasuredFormalLoss<'_>) {
///     OwnedFormalLoss::from_measured(report);
/// }
/// ```
pub struct OwnedFormalLoss<'input, 'view> {
    measured: MeasuredFormalLoss<'input>,
    retained: ResourceClaim<'view>,
}
impl<'input> OwnedFormalLoss<'input, '_> {
    /// Past arithmetic and lineage only. No current-resource eligibility.
    pub fn historical(&self) -> &MeasuredFormalLoss<'input> {
        &self.measured
    }

    pub fn project_current(
        &self,
        plan: &ProbePlan<'_>,
        candidate: &Candidate<'_>,
        direction: Direction,
        resources: &ResourceReservationView<'_>,
    ) -> Result<&MeasuredFormalLoss<'input>, OwnedError> {
        resources.check().map_err(OwnedError::Resource)?;
        self.retained.check().map_err(OwnedError::Resource)?;
        if !self.retained.same_reservation(resources) {
            return Err(OwnedError::ForeignReservation);
        }
        if resources.source_revision() != self.measured.plan().scope().source.digest() {
            return Err(OwnedError::Resource(ResourceError::SourceMismatch));
        }
        if !std::ptr::eq(plan, self.measured.plan())
            || !std::ptr::eq(candidate, self.measured.candidate())
            || direction != self.measured.direction()
        {
            return Err(OwnedError::BindingMismatch);
        }
        Ok(&self.measured)
    }
}

/// Claims scratch AND retained RAM/output before measurement. Source/input
/// attribution is Given; this issues no proof, calibration, hard gate or grant.
/// Rechecks before publication are not an atomic revocation fence: cancellation
/// may race after a check. BigInt allocation is not claimed wholly fallible.
pub fn measure_owned<'input, 'view>(
    plan: &'input ProbePlan<'input>,
    candidate: &'input Candidate<'input>,
    direction: Direction,
    observations: &'input [ProbeObservation<'input>],
    budget: WorkBudget,
    resources: &'view ResourceReservationView<'_>,
) -> Result<OwnedFormalLoss<'input, 'view>, OwnedError> {
    resources.check().map_err(OwnedError::Resource)?;
    if resources.source_revision() != plan.scope().source.digest() {
        return Err(OwnedError::Resource(ResourceError::SourceMismatch));
    }
    let bounds = requirements(plan)?;
    let scratch = resources
        .claim_resources(&[request(ResourceKind::Ram, bounds.scratch_ram)?])
        .map_err(OwnedError::Resource)?;
    let retained = resources
        .claim_resources(&[
            request(ResourceKind::Ram, bounds.retained_ram)?,
            request(ResourceKind::Output, bounds.retained_output)?,
        ])
        .map_err(OwnedError::Resource)?;
    let measured = plan
        .measure(candidate, direction, observations, budget)
        .map_err(OwnedError::Measurement)?;
    if measured.probes.capacity() > plan.fixed_probes().len() {
        return Err(OwnedError::AllocationFailed);
    }
    scratch.check().map_err(OwnedError::Resource)?;
    retained.check().map_err(OwnedError::Resource)?;
    Ok(OwnedFormalLoss { measured, retained })
}
fn request(kind: ResourceKind, amount: u64) -> Result<ResourceRequest, OwnedError> {
    Ok(ResourceRequest::new(
        kind,
        ResourceAmount::checked(amount).map_err(OwnedError::Resource)?,
    ))
}
