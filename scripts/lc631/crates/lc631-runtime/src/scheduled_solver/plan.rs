//! Immutable work plans for the scheduled native SMT model profile.
use super::DispatchError;
use lc631_core::context::ContextState;
use lc631_solvers::{
    protocol::sexpr::SExprLimits, runner::ExecutionBudget, typed_query::SmtStatusQuery,
};
use lc631_world::exploration_schedule::{
    priority::PriorityInput, ScheduleBinding, SchedulingProposal, MAX_KEY_BYTES,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeDemand {
    pub(super) local: [u64; 3],
    pub(super) ram: u64,
    pub(super) output: u64,
    pub(super) cleanup: u64,
}

impl NativeDemand {
    /// Local CPU-query/child slots are not OS CPU quotas. GPU is unsupported.
    pub fn checked(
        local: [u64; 3],
        ram: u64,
        output: u64,
        cleanup: u64,
    ) -> Result<Self, DispatchError> {
        if local[1] != 0 {
            return Err(DispatchError::GpuUnsupported);
        }
        if local != [1, 0, 1] || ram == 0 || output == 0 || cleanup == 0 {
            return Err(DispatchError::InvalidDemand);
        }
        Ok(Self {
            local,
            ram,
            output,
            cleanup,
        })
    }
}

pub struct NativeWorkSpec<'a> {
    pub binding: ScheduleBinding,
    /// Caller canonical-content key; canonicalization remains a declared policy.
    pub canonical_key: &'a [u8],
    pub priority: &'a PriorityInput,
    pub estimated_query_slots: u64,
    pub query: &'a SmtStatusQuery,
    pub context: &'a ContextState,
    pub budget: ExecutionBudget,
    pub parser: SExprLimits,
    pub demand: NativeDemand,
}

/// Checked shape and immutable references, not source-interpretation validation.
pub struct NativeWork<'a> {
    pub(super) spec: NativeWorkSpec<'a>,
}

impl<'a> NativeWork<'a> {
    pub fn checked(spec: NativeWorkSpec<'a>) -> Result<Self, DispatchError> {
        if spec.canonical_key.is_empty() || spec.canonical_key.len() > MAX_KEY_BYTES {
            return Err(DispatchError::InvalidKey);
        }
        if spec.binding.comparison_scope != *spec.context.scope().comparison().as_bytes() {
            return Err(DispatchError::ContextMismatch);
        }
        spec.priority.validate().map_err(DispatchError::Schedule)?;
        let output = spec
            .budget
            .max_stdout_bytes()
            .checked_add(spec.budget.max_stderr_bytes())
            .and_then(|n| n.checked_mul(2))
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(DispatchError::ArithmeticOverflow)?;
        if spec.budget.disk_bytes() != 0
            || spec.query.limits().max_output_bytes() > spec.budget.max_stdin_bytes()
            || spec.parser.max_input_bytes() < spec.budget.max_stdout_bytes()
            || spec.demand.ram < spec.budget.ram_bytes()
            || spec.demand.output < output
            || spec.demand.cleanup < spec.budget.cleanup_bytes()
        {
            return Err(DispatchError::InvalidDemand);
        }
        Ok(Self { spec })
    }

    pub fn canonical_key(&self) -> &[u8] {
        self.spec.canonical_key
    }

    /// Copy immutable references/value configuration, never source or results.
    pub(super) fn borrowed(&self) -> Self {
        Self {
            spec: NativeWorkSpec {
                binding: self.spec.binding,
                canonical_key: self.spec.canonical_key,
                priority: self.spec.priority,
                estimated_query_slots: self.spec.estimated_query_slots,
                query: self.spec.query,
                context: self.spec.context,
                budget: self.spec.budget,
                parser: self.spec.parser,
                demand: self.spec.demand,
            },
        }
    }

    pub(super) fn proposal(&self) -> SchedulingProposal<'_> {
        SchedulingProposal {
            binding: self.spec.binding,
            canonical_key: self.spec.canonical_key,
            priority: self.spec.priority,
            estimated_query_slots: self.spec.estimated_query_slots,
            demand: self.spec.demand.local,
        }
    }

    pub(super) fn same_payload(&self, other: &Self) -> bool {
        self.spec.binding == other.spec.binding
            && self.spec.query == other.spec.query
            && self.spec.context.revision() == other.spec.context.revision()
            && self.spec.context.scope() == other.spec.context.scope()
            && self.spec.budget == other.spec.budget
            && self.spec.parser == other.spec.parser
            && self.spec.demand == other.spec.demand
            && self.spec.priority == other.spec.priority
            && self.spec.estimated_query_slots == other.spec.estimated_query_slots
    }
}
