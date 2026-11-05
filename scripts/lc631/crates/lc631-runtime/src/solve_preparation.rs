//! 実source解析→文脈→論理の資源所有準備。solver実行・source真理・権限ではない。
//! Resource-owned actual source/context/logical preparation; no solver truth or permit.
#![forbid(unsafe_code)]

use lc631_analysis::context_lowering::{lower_dgcl_context, ContextLoweringError, LoweredContext};
use lc631_analysis::logical_lowering::{
    lower_dgcl_logical, LogicProfile, LogicalLoweringBudget, LogicalLoweringError, LogicalTask,
};
use lc631_analysis::{build_dgcl_pipeline, DgclBackendSet, DgclPipelineError};
use lc631_core::{
    context::{ContextLimits, ContextRevision, ContextScope, SourceRevision, TaskKind},
    AuthorityRevision,
};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};

/// Deliberately bounded initial legacy-construction engineering profile.
/// The outer CLI may accept more bytes; this profile never truncates them.
pub const MAX_PREPARATION_SOURCE_BYTES: usize = 16 * 1024;
pub const PREPARATION_RAM_FLOOR: u64 = 32 * 1024 * 1024;
pub const PREPARATION_RAM_CEILING: u64 = 64 * 1024 * 1024;
pub const PREPARATION_PROFILE: &str = "canonical-prepare-16k-logical-ram-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparationBudget {
    logical_ram: u64,
}
impl PreparationBudget {
    pub fn checked(logical_ram: u64) -> Result<Self, PreparationError> {
        if !(PREPARATION_RAM_FLOOR..=PREPARATION_RAM_CEILING).contains(&logical_ram) {
            return Err(PreparationError::InvalidBudget);
        }
        Ok(Self { logical_ram })
    }
    pub const fn logical_ram_bytes(self) -> u64 {
        self.logical_ram
    }
}

/// Explicit metadata supplied by the caller. AuthorityRevision is not a current
/// receipt or execution permit; scope pins are not a validated GoalProfile.
#[derive(Clone, Copy, Debug)]
pub struct ProvidedPreparation {
    pub scope: ContextScope,
    pub authority_revision: AuthorityRevision,
    pub task: TaskKind,
    pub logic_profile: LogicProfile,
    pub context_limits: ContextLimits,
    pub logical_budget: LogicalLoweringBudget,
}
#[derive(Clone, Copy)]
pub struct PreparationRequest<'a> {
    pub source: &'a str,
    pub source_revision: SourceRevision,
    pub expected_lowered_context: Option<ContextRevision>,
    pub provided: ProvidedPreparation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreparationError {
    EmptySource,
    SourceLimit,
    SourceMismatch,
    ContextMismatch,
    InvalidBudget,
    InvalidPolicy,
    ForeignReservation,
    Resource(ResourceError),
    Pipeline(DgclPipelineError),
    Context(ContextLoweringError),
    Logical(LogicalLoweringError),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationState {
    Clarify,
    NextRequiredStageSolverPortfolio,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedContextBinding {
    NotProvided,
    Matched(ContextRevision),
}

/// Immutable actual artifacts precede the retained claim in drop order.
/// There is no public constructor, detached-report admission, receipt issuer,
/// backend invocation, selection, completion flag or authority conversion.
pub struct PreparedSolve<'view> {
    context: LoweredContext,
    logical: LogicalTask,
    provided: ProvidedPreparation,
    expected_binding: ExpectedContextBinding,
    state: PreparationState,
    claim: ResourceClaim<'view>,
}
impl PreparedSolve<'_> {
    /// Historical immutable inspection only, not fresh execution eligibility.
    pub fn source(&self) -> &str {
        self.context.source_store().text()
    }
    pub const fn provided(&self) -> ProvidedPreparation {
        self.provided
    }
    pub const fn expected_context_binding(&self) -> ExpectedContextBinding {
        self.expected_binding
    }
    pub const fn state(&self) -> PreparationState {
        self.state
    }
    pub const fn source_interpretation_verified(&self) -> bool {
        false
    }
    pub fn context_historical(&self) -> &LoweredContext {
        &self.context
    }
    pub fn logical_historical(&self) -> &LogicalTask {
        &self.logical
    }
    /// Rechecks actual owner/source/current lifetime before exposing the next
    /// projection seam. This is not an atomic revocation fence or a permit.
    pub fn project_current(
        &self,
        resources: &ResourceReservationView<'_>,
    ) -> Result<PreparedProjection<'_>, PreparationError> {
        resources.check().map_err(PreparationError::Resource)?;
        self.claim.check().map_err(PreparationError::Resource)?;
        if !self.claim.same_reservation(resources) {
            return Err(PreparationError::ForeignReservation);
        }
        if resources.source_revision() != self.logical.context_revision().source().digest() {
            return Err(PreparationError::SourceMismatch);
        }
        check_artifact_binding(&self.context, &self.logical)?;
        Ok(PreparedProjection {
            context: &self.context,
            logical: &self.logical,
            state: self.state,
        })
    }
}
pub struct PreparedProjection<'a> {
    context: &'a LoweredContext,
    logical: &'a LogicalTask,
    state: PreparationState,
}
impl PreparedProjection<'_> {
    pub const fn context(&self) -> &LoweredContext {
        self.context
    }
    pub const fn logical(&self) -> &LogicalTask {
        self.logical
    }
    pub const fn state(&self) -> PreparationState {
        self.state
    }
}

/// Executes each existing producer once. No new broker, pure solve call,
/// external model worker, JSON rendering or fallback is performed here.
/// The fixed allowance is reserved for the entire declared construction profile,
/// including transient legacy pipeline and retained context/task copies.
/// Actual allocator/RSS exhaustion is NOT controlled by this logical claim;
/// existing pipeline JSON construction seals remain a measured-cost gap.
pub fn prepare<'view>(
    request: PreparationRequest<'_>,
    budget: PreparationBudget,
    resources: &'view ResourceReservationView<'_>,
) -> Result<PreparedSolve<'view>, PreparationError> {
    admit(&request, resources)?;
    let amount = ResourceAmount::checked(budget.logical_ram).map_err(PreparationError::Resource)?;
    let claim = resources
        .claim_resources(&[ResourceRequest::new(ResourceKind::Ram, amount)])
        .map_err(PreparationError::Resource)?;
    let pipeline = build_dgcl_pipeline(
        request.source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .map_err(PreparationError::Pipeline)?;
    claim.check().map_err(PreparationError::Resource)?;
    let context = lower_dgcl_context(
        &pipeline,
        request.provided.scope,
        request.provided.authority_revision,
        request.provided.context_limits,
    )
    .map_err(PreparationError::Context)?;
    let actual = context.original().state().revision();
    let expected_binding = match request.expected_lowered_context {
        None => ExpectedContextBinding::NotProvided,
        Some(expected) if expected == actual => ExpectedContextBinding::Matched(actual),
        Some(_) => return Err(PreparationError::ContextMismatch),
    };
    claim.check().map_err(PreparationError::Resource)?;
    let logical = lower_dgcl_logical(
        &pipeline,
        &context,
        request.provided.task,
        request.provided.logic_profile,
        request.provided.logical_budget,
    )
    .map_err(PreparationError::Logical)?;
    check_artifact_binding(&context, &logical)?;
    let state = readiness(&context, &logical);
    drop(pipeline);
    claim.check().map_err(PreparationError::Resource)?;
    Ok(PreparedSolve {
        context,
        logical,
        provided: request.provided,
        expected_binding,
        state,
        claim,
    })
}

fn admit(
    request: &PreparationRequest<'_>,
    resources: &ResourceReservationView<'_>,
) -> Result<(), PreparationError> {
    if request.source.is_empty() {
        return Err(PreparationError::EmptySource);
    }
    // Bound caller-owned source BEFORE hashing it or constructing any pipeline.
    if request.source.len() > MAX_PREPARATION_SOURCE_BYTES {
        return Err(PreparationError::SourceLimit);
    }
    resources.check().map_err(PreparationError::Resource)?;
    if request.source_revision != SourceRevision::of(request.source)
        || resources.source_revision() != request.source_revision.digest()
        || request
            .expected_lowered_context
            .is_some_and(|context| context.source() != request.source_revision)
    {
        return Err(PreparationError::SourceMismatch);
    }
    let c = request.provided.context_limits;
    let l = request.provided.logical_budget;
    if c.max_contexts == 0
        || c.max_contexts > 2048
        || c.max_premises == 0
        || c.max_premises > 8192
        || c.max_edits == 0
        || c.max_edits > 8192
        || c.max_statement_bytes < request.source.len()
        || c.max_statement_bytes > MAX_PREPARATION_SOURCE_BYTES
        || l.max_nodes == 0
        || l.max_nodes > 8192
        || l.max_obligations == 0
        || l.max_obligations > 8192
        || l.max_residuals == 0
        || l.max_residuals > 8192
        || l.max_depth == 0
        || l.max_depth > 64
    {
        return Err(PreparationError::InvalidPolicy);
    }
    Ok(())
}
fn check_artifact_binding(
    context: &LoweredContext,
    logical: &LogicalTask,
) -> Result<(), PreparationError> {
    if context.source_store().id() != logical.source_store().id()
        || !std::ptr::eq(
            context.source_store().text().as_ptr(),
            logical.source_store().text().as_ptr(),
        )
        || context.original().state().revision() != logical.context_revision()
    {
        return Err(PreparationError::ContextMismatch);
    }
    Ok(())
}
fn readiness(context: &LoweredContext, logical: &LogicalTask) -> PreparationState {
    if logical.formulas().is_empty()
        || logical.obligations().is_empty()
        || !context.unsupported_operative_region_ids().is_empty()
        || !context.residuals().is_empty()
        || !logical.residuals().is_empty()
        || !logical.source_residuals().is_empty()
        || logical
            .obligations()
            .iter()
            .any(|obligation| obligation.needs_interpretation())
    {
        PreparationState::Clarify
    } else {
        PreparationState::NextRequiredStageSolverPortfolio
    }
}
