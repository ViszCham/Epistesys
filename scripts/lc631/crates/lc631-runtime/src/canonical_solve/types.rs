use super::{ExecutionBudget, SExprLimits, SolverRegistry, SolverRegistryError};
use crate::explicit_formal::{FormalError, FormalProblem, FormalResidualReport};
use crate::facet_ledger::LedgerError;
use crate::recommendation_output::{ConnectionError, OutputError};
use crate::resource::ResourceBroker;
use lc631_core::context::{ContextState, PremiseBasis, SourceRevision};
use lc631_solvers::{
    smt::native_model::{NativeCheckedModel, NativeModelError},
    supervisor_adapter::{OwnedPayloadStore, OwnedPayloadStoreInitError},
};
use lc631_substrate::{
    resource::{ResourceClaim, ResourceError, ResourceReservationView},
    Digest32,
};
use std::{fmt, path::Path};

pub const DECISION_RAM_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GoalPolicy {
    NoGoal,
    /// Caller declaration only. It cannot override a parsed min/max directive.
    SatisfyingModel {
        revision: Digest32,
    },
    DeclaredSourceObjective {
        revision: Digest32,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpuPolicy {
    NotRequested,
    Require,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecisionBudget {
    pub(super) max_output_bytes: usize,
}
impl DecisionBudget {
    pub fn checked(max_output_bytes: usize) -> Result<Self, AdmissionError> {
        if max_output_bytes == 0 || max_output_bytes > MAX_OUTPUT_BYTES {
            return Err(AdmissionError::InvalidBudget);
        }
        Ok(Self { max_output_bytes })
    }
}

#[derive(Clone, Copy)]
pub(super) enum FormalInput<'p, 'view> {
    Problem(&'p FormalProblem<'view>),
    Unsupported(&'p FormalResidualReport<'view>),
    Malformed(&'p FormalError),
}
#[derive(Clone, Copy)]
pub struct CanonicalRequest<'p, 'view> {
    pub(super) input: FormalInput<'p, 'view>,
    pub(super) context: &'p ContextState,
    pub(super) goal: GoalPolicy,
    pub(super) evaluation: Option<super::evaluation::DeclaredEvaluation>,
    pub(super) objective_step: Option<super::optimization::OneStepBudget>,
}
impl<'p, 'view> CanonicalRequest<'p, 'view> {
    pub fn formal(
        problem: &'p FormalProblem<'view>,
        context: &'p ContextState,
        goal: GoalPolicy,
    ) -> Result<Self, AdmissionError> {
        if problem.context() != context.revision()
            || problem.scope() != context.scope()
            || problem.source().id().digest() != context.revision().source().digest()
        {
            return Err(AdmissionError::ContextMismatch);
        }
        if context.parent().is_some()
            || context.premises().len() > 256
            || context
                .premises()
                .iter()
                .any(|p| p.basis() != PremiseBasis::Given)
        {
            return Err(AdmissionError::UnsupportedContext);
        }
        match goal {
            GoalPolicy::SatisfyingModel { revision } => {
                if problem.goal().is_some() {
                    return Err(AdmissionError::GoalConflict);
                }
                if revision != context.scope().goal() {
                    return Err(AdmissionError::GoalRevision);
                }
            }
            GoalPolicy::DeclaredSourceObjective { revision } => {
                if problem.goal().is_none() {
                    return Err(AdmissionError::GoalConflict);
                }
                if revision != context.scope().goal() {
                    return Err(AdmissionError::GoalRevision);
                }
            }
            GoalPolicy::NoGoal => {}
        }
        Ok(Self {
            input: FormalInput::Problem(problem),
            context,
            goal,
            evaluation: None,
            objective_step: None,
        })
    }
    pub fn unsupported(
        report: &'p FormalResidualReport<'view>,
        context: &'p ContextState,
    ) -> Result<Self, AdmissionError> {
        if report.context() != context.revision()
            || report.scope() != context.scope()
            || report.source().id().digest() != context.revision().source().digest()
        {
            return Err(AdmissionError::ContextMismatch);
        }
        Ok(Self {
            input: FormalInput::Unsupported(report),
            context,
            goal: GoalPolicy::NoGoal,
            evaluation: None,
            objective_step: None,
        })
    }
    /// Actual parser failure is rendered as Clarify and never dispatched.
    pub fn clarify(
        error: &'p FormalError,
        context: &'p ContextState,
    ) -> Result<Self, AdmissionError> {
        if error.source().id().digest() != context.revision().source().digest() {
            return Err(AdmissionError::ContextMismatch);
        }
        Ok(Self {
            input: FormalInput::Malformed(error),
            context,
            goal: GoalPolicy::NoGoal,
            evaluation: None,
            objective_step: None,
        })
    }
    pub fn with_objective_step(
        mut self,
        budget: super::optimization::OneStepBudget,
    ) -> Result<Self, AdmissionError> {
        if !matches!(self.goal, GoalPolicy::DeclaredSourceObjective { .. }) {
            return Err(AdmissionError::GoalConflict);
        }
        let FormalInput::Problem(problem) = self.input else {
            return Err(AdmissionError::GoalConflict);
        };
        if problem.goal().is_none() {
            return Err(AdmissionError::GoalConflict);
        }
        self.objective_step = Some(budget);
        Ok(self)
    }
    pub fn with_evaluation(
        mut self,
        declared: super::evaluation::DeclaredEvaluation,
    ) -> Result<Self, AdmissionError> {
        declared.admit(self)?;
        self.evaluation = Some(declared);
        Ok(self)
    }
    pub(super) fn source(self) -> &'p str {
        match self.input {
            FormalInput::Problem(p) => p.source().text(),
            FormalInput::Unsupported(p) => p.source().text(),
            FormalInput::Malformed(p) => p.source().text(),
        }
    }
    pub(super) fn source_revision(self) -> SourceRevision {
        self.context.revision().source()
    }
}

/// Every operational service is explicit and borrowed from ONE owning runtime.
pub struct CanonicalServices<'view, 'lease> {
    pub registry: Option<&'view SolverRegistry>,
    pub broker: &'view ResourceBroker,
    pub current: &'view ResourceReservationView<'lease>,
    pub execution: ExecutionBudget,
    pub parser: SExprLimits,
    pub working_directory: &'view Path,
    pub budget: DecisionBudget,
    pub optional_gpu: GpuPolicy,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdmissionError {
    CachePolicy(crate::turn_cache::CacheError),
    Optimization(super::optimization::OptimizationError),
    InvalidBudget,
    ContextMismatch,
    UnsupportedContext,
    GoalConflict,
    GoalRevision,
    RelativeWorkingDirectory,
    SourceMismatch,
    SourceLimit,
    AllocationFailed,
    Resource(ResourceError),
    Store(OwnedPayloadStoreInitError),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalStatus {
    ConditionalModel,
    Unavailable,
    Clarify,
    NativeFailure,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoRecommendationReason {
    NoDeclaredGoal,
    MissingTranslationEvaluator,
    DeferredSourceObjective,
    MissingBackend,
    UnsupportedInput,
    GpuUnavailable,
    NativeRejected,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeEvidenceKind {
    ModelRechecked,
    Rejected,
    Unavailable,
    NoExecutableInput,
    Unobserved,
}
// One slot is fallibly reserved under the outer claim before native execution;
// boxing here would introduce an unchecked post-execution allocation.
#[allow(clippy::large_enum_variant)]
pub(super) enum NativeAttempt<'p, 'view> {
    Model(NativeCheckedModel<'p, 'view>),
    Rejected(NativeModelError<'view>),
    Unavailable(BackendUnavailable),
    NoExecutableInput,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendUnavailable {
    MissingRegistry,
    Registry(SolverRegistryError),
    ProtocolMismatch,
    GpuRequired,
}
/// Native bytes/model drop first, store next. The outer owner drops its claim last.
pub struct NativeEvidence<'p, 'view> {
    pub(super) attempts: Vec<NativeAttempt<'p, 'view>>,
    pub(super) store: Option<OwnedPayloadStore<'view>>,
    pub(super) measurement: Option<super::evaluation::OwnedBooleanMeasurement<'view>>,
    // At most one slot, fallibly allocated BEFORE native execution. Keeping the
    // large receipt out of every error/return enum also bounds normal CLI frames.
    pub(super) objective: Vec<super::optimization::OwnedObjectiveStep<'view>>,
    pub(super) request: CanonicalRequest<'p, 'view>,
}
impl<'p, 'view> NativeEvidence<'p, 'view> {
    pub fn kind(&self) -> NativeEvidenceKind {
        match self.attempts.first() {
            Some(NativeAttempt::Model(_)) => NativeEvidenceKind::ModelRechecked,
            Some(NativeAttempt::Rejected(_)) => NativeEvidenceKind::Rejected,
            Some(NativeAttempt::Unavailable(_)) => NativeEvidenceKind::Unavailable,
            Some(NativeAttempt::NoExecutableInput) => NativeEvidenceKind::NoExecutableInput,
            None => NativeEvidenceKind::Unobserved,
        }
    }
    pub fn objective_step(&self) -> Option<&super::optimization::OwnedObjectiveStep<'view>> {
        self.objective.first()
    }
    pub fn boolean_measurement(
        &self,
    ) -> Option<&super::evaluation::OwnedBooleanMeasurement<'view>> {
        self.measurement.as_ref()
    }
    pub fn stored_results(&self) -> usize {
        self.store.as_ref().map_or(0, OwnedPayloadStore::len)
    }
    pub fn native_model(&self) -> Option<&NativeCheckedModel<'p, 'view>> {
        match self.attempts.first() {
            Some(NativeAttempt::Model(model)) => Some(model),
            _ => None,
        }
    }
    pub fn native_error(&self) -> Option<&NativeModelError<'view>> {
        match self.attempts.first() {
            Some(NativeAttempt::Rejected(error)) => Some(error),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PostprocessError {
    Optimization(super::optimization::OptimizationError),
    Evaluation(super::evaluation::EvaluationError),
    Resource(ResourceError),
    Ledger(LedgerError),
    Output(OutputError),
    Connection(ConnectionError),
    AllocationFailed,
    OutputBudget,
    SourceBinding,
}
/// Inseparable post-execution owner: backing drops before its private claim.
///
/// ```compile_fail
/// use lc631_runtime::canonical_solve::OwnedPostprocessFailure;
/// fn detach(failure: OwnedPostprocessFailure<'_, '_>) {
///     let _evidence = failure.evidence;
///     drop(failure.claim);
/// }
/// ```
pub struct OwnedPostprocessFailure<'p, 'view> {
    pub(super) reason: PostprocessError,
    pub(super) evidence: NativeEvidence<'p, 'view>,
    pub(super) claim: ResourceClaim<'view>,
}
impl<'p, 'view> OwnedPostprocessFailure<'p, 'view> {
    pub fn reason(&self) -> &PostprocessError {
        &self.reason
    }
    pub fn evidence(&self) -> &NativeEvidence<'p, 'view> {
        &self.evidence
    }
    pub fn check_retained_resources(&self) -> Result<(), ResourceError> {
        self.claim.check()
    }
}
/// After execution, errors retain the sole actual observation, store AND claim.
// Retain actual evidence inline; boxing after execution adds unchecked allocation.
#[allow(clippy::large_enum_variant)]
pub enum CanonicalError<'p, 'view> {
    Admission(AdmissionError),
    Postprocess(OwnedPostprocessFailure<'p, 'view>),
}
impl fmt::Debug for CanonicalError<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(e) => f.debug_tuple("Admission").field(e).finish(),
            Self::Postprocess(failure) => f
                .debug_struct("Postprocess")
                .field("reason", failure.reason())
                .field("native_kind", &failure.evidence().kind())
                .finish(),
        }
    }
}
