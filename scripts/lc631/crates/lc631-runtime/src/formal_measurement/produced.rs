//! 実有限evaluatorから固定K1へ。caller statusを観測へ昇格しない。
//! Actual finite evaluation to the fixed K1 consumer, never caller-status promotion.
#![forbid(unsafe_code)]
use super::{
    f, plan, reserved, BridgeError, Candidate, Gap, ObservationValue, ProbeObservation, ProbePlan,
    Tracker, WorkBudget,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};

pub const EVALUATION_SCRATCH_RAM: u64 = 64 * 1024;

/// Declared work is a query allowance, never a supplied evaluation status.
#[derive(Clone, Copy, Debug)]
pub struct ScheduledProbe<'input> {
    pub identity: &'input [u8],
    pub request: &'input f::Request<'input>,
    pub max_work: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProducedError {
    Bridge(BridgeError),
    Resource(ResourceError),
    ForeignReservation,
    ArithmeticOverflow,
    NoComparable(f::NoComparable),
    Unsupported(f::Failure),
}

struct ProducedProbe<'input> {
    identity: &'input [u8],
    outcome: Result<f::Evaluation<'input>, f::Failure>,
}

/// Result backing drops before its retained claim. All query/AST/map backing is
/// borrowed; this type contains no self-reference, detached report constructor,
/// Deserialize implementation or source-truth/authority issuer.
pub struct ProducedBatch<'input, 'view> {
    produced: Vec<ProducedProbe<'input>>,
    plan: &'input ProbePlan<'input>,
    candidate: &'input Candidate<'input>,
    retained: ResourceClaim<'view>,
}

/// Observation references cannot outlive the batch owning actual reports.
/// The vector drops before its own claim; caller-owned AST/Rational storage,
/// allocator metadata and RSS are outside these logical-byte claims.
pub struct ProducedObservations<'batch, 'view> {
    values: Vec<ProbeObservation<'batch>>,
    retained: ResourceClaim<'view>,
}
impl<'batch> ProducedObservations<'batch, '_> {
    pub fn as_slice(&self) -> &[ProbeObservation<'batch>] {
        &self.values
    }
    pub fn check_current(&self) -> Result<(), ProducedError> {
        self.retained.check().map_err(ProducedError::Resource)
    }
}

impl<'input, 'view> ProducedBatch<'input, 'view> {
    /// Historical actual gap only; does not reactivate a cancelled claim.
    pub fn gap(&self, index: usize) -> Option<Gap> {
        match &self.produced.get(index)?.outcome {
            Ok(f::Evaluation::Computed(_)) => None,
            Ok(f::Evaluation::Unknown { .. }) => Some(Gap::Unknown),
            Ok(f::Evaluation::NoComparable { reason, .. }) => Some(Gap::NoComparable(*reason)),
            Err(reason) => Some(Gap::Unsupported(*reason)),
        }
    }
    pub fn produced_count(&self) -> usize {
        self.produced.len()
    }
    pub fn missing_count(&self) -> usize {
        self.plan.fixed_probes().len() - self.produced.len()
    }
    pub fn plan(&self) -> &ProbePlan<'input> {
        self.plan
    }
    pub fn candidate(&self) -> &Candidate<'input> {
        self.candidate
    }

    /// Produce typed borrowed observations from internally obtained results.
    /// Omitted identities remain omitted so ProbePlan retains the fixed K1
    /// denominator. Cancellation may race after these checks; no commit fence.
    pub fn observations_current<'batch>(
        &'batch self,
        resources: &'view ResourceReservationView<'_>,
    ) -> Result<ProducedObservations<'batch, 'view>, ProducedError> {
        check_owner(&self.retained, self.plan, resources)?;
        // A missing observation in a fixed quantity has a declared [0,1]
        // enclosure; an undefined/incomparable quantity has no scalar here.
        for item in &self.produced {
            match &item.outcome {
                Ok(f::Evaluation::NoComparable { reason, .. }) => {
                    return Err(ProducedError::NoComparable(*reason))
                }
                Err(reason) => return Err(ProducedError::Unsupported(*reason)),
                Ok(f::Evaluation::Computed(_) | f::Evaluation::Unknown { .. }) => {}
            }
        }
        let amount = backing::<ProbeObservation<'batch>>(self.produced.len())?;
        let retained = claim(resources, amount, amount)?;
        let mut values = reserved(self.produced.len()).map_err(ProducedError::Bridge)?;
        for produced in &self.produced {
            let value = match &produced.outcome {
                Ok(f::Evaluation::Computed(report)) => ObservationValue::Computed(report),
                Ok(f::Evaluation::Unknown { .. }) => ObservationValue::Gap(Gap::Unknown),
                Ok(f::Evaluation::NoComparable { reason, .. }) => {
                    return Err(ProducedError::NoComparable(*reason))
                }
                Err(reason) => return Err(ProducedError::Unsupported(*reason)),
            };
            values.push(ProbeObservation {
                identity: produced.identity,
                candidate: self.candidate.id(),
                scope: self.plan.scope().comparison,
                value,
            });
        }
        check_owner(&self.retained, self.plan, resources)?;
        retained.check().map_err(ProducedError::Resource)?;
        Ok(ProducedObservations { values, retained })
    }
}

/// Execute the EXISTING finite Boolean producer after validating the complete
/// schedule. Resource claims precede vector allocation and actual evaluation.
/// No missing probe is removed from the immutable plan or given loss zero.
pub fn execute<'input, 'view>(
    plan: &'input ProbePlan<'input>,
    candidate: &'input Candidate<'input>,
    schedule: &[ScheduledProbe<'input>],
    budget: WorkBudget,
    resources: &'view ResourceReservationView<'_>,
) -> Result<ProducedBatch<'input, 'view>, ProducedError> {
    resources.check().map_err(ProducedError::Resource)?;
    if resources.source_revision() != plan.scope().source.digest() {
        return Err(ProducedError::Resource(ResourceError::SourceMismatch));
    }
    // Scratch is owned before validation traversals or output allocation.
    let scratch = claim(resources, EVALUATION_SCRATCH_RAM, 0)?;
    validate_schedule(plan, candidate, schedule, budget).map_err(ProducedError::Bridge)?;
    let retained = claim(
        resources,
        backing::<ProducedProbe<'input>>(schedule.len())?,
        0,
    )?;
    let mut produced = reserved(schedule.len()).map_err(ProducedError::Bridge)?;
    for item in schedule {
        resources.check().map_err(ProducedError::Resource)?;
        let outcome = f::evaluate(item.request, item.max_work);
        resources.check().map_err(ProducedError::Resource)?;
        produced.push(ProducedProbe {
            identity: item.identity,
            outcome,
        });
    }
    scratch.check().map_err(ProducedError::Resource)?;
    retained.check().map_err(ProducedError::Resource)?;
    Ok(ProducedBatch {
        produced,
        plan,
        candidate,
        retained,
    })
}

fn validate_schedule(
    plan: &ProbePlan<'_>,
    candidate: &Candidate<'_>,
    schedule: &[ScheduledProbe<'_>],
    budget: WorkBudget,
) -> Result<(), BridgeError> {
    let mut work = Tracker::new(budget)?;
    if candidate.comparison != plan.scope().comparison {
        return Err(BridgeError::ScopeMismatch);
    }
    if schedule.len() > 256 {
        return Err(BridgeError::ProbeLimit);
    }
    work.bytes(plan.payload_bytes())?;
    plan::count_query(candidate.target, &mut work)?;
    for (index, item) in schedule.iter().enumerate() {
        if item.identity.is_empty() || item.identity.len() > 4096 {
            return Err(BridgeError::ForeignProbe);
        }
        work.bytes(item.identity.len() as u64 + 24)?;
        work.charge((plan.fixed_probes().len() + index + 1) as u64)?;
        if schedule[..index]
            .iter()
            .any(|previous| previous.identity == item.identity)
        {
            return Err(BridgeError::DuplicateProbe);
        }
        let fixed = plan
            .fixed_probes()
            .iter()
            .find(|probe| probe.identity == item.identity)
            .ok_or(BridgeError::ForeignProbe)?;
        if item.max_work == 0 || item.max_work > 1_000_000 {
            return Err(BridgeError::InvalidBudget);
        }
        // Worst-case dispatch allowances share ONE run budget. Unknown does
        // not mint another allowance or silently retry the same query.
        work.charge(item.max_work)?;
        validate_request(plan, candidate, fixed.template, item.request)?;
    }
    Ok(())
}

fn validate_request(
    plan: &ProbePlan<'_>,
    candidate: &Candidate<'_>,
    expected: &f::Request<'_>,
    request: &f::Request<'_>,
) -> Result<(), BridgeError> {
    if !std::ptr::eq(request.source, plan.source)
        || !std::ptr::eq(request.target, candidate.target)
        || !std::ptr::eq(request.source_predicate, expected.source_predicate)
        || !std::ptr::eq(request.target_predicate, expected.target_predicate)
        || request.symbols != expected.symbols
        || request.unconstrained != expected.unconstrained
        || request.target.boolean_symbols() != expected.target.boolean_symbols()
    {
        return Err(BridgeError::LineageMismatch);
    }
    let b = request.binding;
    if b.source != plan.scope().source
        || b.source_context != plan.scope().context
        || b.target != candidate.revision()
        || b.target_context != candidate.context()
        || b.probe_set != plan.scope().probe_set
        || b.probe_map != expected.binding.probe_map
    {
        return Err(BridgeError::LineageMismatch);
    }
    Ok(())
}

fn check_owner(
    retained: &ResourceClaim<'_>,
    plan: &ProbePlan<'_>,
    resources: &ResourceReservationView<'_>,
) -> Result<(), ProducedError> {
    resources.check().map_err(ProducedError::Resource)?;
    retained.check().map_err(ProducedError::Resource)?;
    if !retained.same_reservation(resources) {
        return Err(ProducedError::ForeignReservation);
    }
    if resources.source_revision() != plan.scope().source.digest() {
        return Err(ProducedError::Resource(ResourceError::SourceMismatch));
    }
    Ok(())
}
fn backing<T>(count: usize) -> Result<u64, ProducedError> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|n| n.checked_add(512))
        .ok_or(ProducedError::ArithmeticOverflow)?;
    u64::try_from(bytes).map_err(|_| ProducedError::ArithmeticOverflow)
}
fn claim<'view>(
    resources: &'view ResourceReservationView<'_>,
    ram: u64,
    output: u64,
) -> Result<ResourceClaim<'view>, ProducedError> {
    let ram = ResourceAmount::checked(ram).map_err(ProducedError::Resource)?;
    if output == 0 {
        resources
            .claim_resources(&[ResourceRequest::new(ResourceKind::Ram, ram)])
            .map_err(ProducedError::Resource)
    } else {
        resources
            .claim_resources(&[
                ResourceRequest::new(ResourceKind::Ram, ram),
                ResourceRequest::new(
                    ResourceKind::Output,
                    ResourceAmount::checked(output).map_err(ProducedError::Resource)?,
                ),
            ])
            .map_err(ProducedError::Resource)
    }
}
