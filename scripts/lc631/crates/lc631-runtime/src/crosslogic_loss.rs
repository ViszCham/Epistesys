//! TLCX-20 diagnostic preparation over reported solver contracts.
//! This module neither validates a semantic bridge nor measures semantic TL.
#![forbid(unsafe_code)]

pub mod finite_temporal;
pub mod temporal_reprojection;

use lc631_core::context::{ContextConsumer, ContextRevision, ContextScope};
use lc631_solvers::{
    ExecutionStatus, LogicalResult, ObjectiveResult, Satisfiability, SolverRun, Support,
};
use lc631_substrate::Digest32;

/// Caller-reported lineage, not a checked query, process receipt or truth token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComparisonBinding {
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub root_query: Digest32,
    pub translated_query: Digest32,
    pub domain: Digest32,
    pub premises: Digest32,
    pub projection: Digest32,
}

/// Complete declared shared dependencies include parser, translator and extractor.
/// Absence or disjoint declarations never establishes checked independence.
#[derive(Clone, Copy, Debug)]
pub struct ReportedObservation<'a> {
    binding: ComparisonBinding,
    run: SolverRun,
    dependencies: &'a [Digest32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationError {
    TooManyDependencies,
    DuplicateDependency,
    UnknownContext,
    StaleScope,
    QueryBindingMismatch,
    MissingDiagnostic,
}

impl<'a> ReportedObservation<'a> {
    pub fn reported(
        binding: ComparisonBinding,
        run: SolverRun,
        dependencies: &'a [Digest32],
    ) -> Result<Self, PreparationError> {
        if dependencies.len() > 16 {
            return Err(PreparationError::TooManyDependencies);
        }
        for (index, dependency) in dependencies.iter().enumerate() {
            if dependencies[..index].contains(dependency) {
                return Err(PreparationError::DuplicateDependency);
            }
        }
        Ok(Self {
            binding,
            run,
            dependencies,
        })
    }

    pub const fn binding(self) -> ComparisonBinding {
        self.binding
    }
    pub const fn run(self) -> SolverRun {
        self.run
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyStatus {
    Unassessed,
    ReportedSharedDependency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReprojectionCause {
    ContextMismatch,
    ScopeMismatch,
    QueryMismatch,
    DomainMismatch,
    PremiseMismatch,
    SemanticsMismatch,
    OutcomeKindMismatch,
    QuantificationMismatch,
    ConsequenceModeMismatch,
    TraceDomainMismatch,
    IncompleteExecution,
    UnknownOutcome,
    ConflictingSupport,
    ReportedDisagreement,
}

/// Report-shape correspondence is deliberately weaker than semantic equality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Correspondence {
    SameReportedOutcome,
    NeedsReprojection(ReprojectionCause),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BridgeDiagnostic {
    left: ComparisonBinding,
    right: ComparisonBinding,
    left_run: SolverRun,
    right_run: SolverRun,
    dependencies: DependencyStatus,
    correspondence: Correspondence,
}

impl BridgeDiagnostic {
    pub const fn correspondence(self) -> Correspondence {
        self.correspondence
    }
    pub const fn dependencies(self) -> DependencyStatus {
        self.dependencies
    }
    pub const fn reported_runs(self) -> (SolverRun, SolverRun) {
        (self.left_run, self.right_run)
    }
    pub const fn claim_boundary(self) -> &'static str {
        "reported shape comparison only; semantic bridge, validation and independence need evidence"
    }

    pub fn reprojection(self) -> Result<ReprojectionRequest, PreparationError> {
        match self.correspondence {
            Correspondence::SameReportedOutcome => Err(PreparationError::MissingDiagnostic),
            Correspondence::NeedsReprojection(cause) => Ok(ReprojectionRequest {
                left: self.left,
                right: self.right,
                cause,
            }),
        }
    }
}

/// Bounded pair analysis: no votes, ranking, score, receipt or operation permits.
pub fn analyze_pair(
    left: ReportedObservation<'_>,
    right: ReportedObservation<'_>,
) -> BridgeDiagnostic {
    let dependencies = if left
        .dependencies
        .iter()
        .any(|d| right.dependencies.contains(d))
    {
        DependencyStatus::ReportedSharedDependency
    } else {
        DependencyStatus::Unassessed
    };
    let cause =
        binding_mismatch(left.binding, right.binding).or_else(|| run_mismatch(left.run, right.run));
    BridgeDiagnostic {
        left: left.binding,
        right: right.binding,
        left_run: left.run,
        right_run: right.run,
        dependencies,
        correspondence: match cause {
            Some(cause) => Correspondence::NeedsReprojection(cause),
            None => Correspondence::SameReportedOutcome,
        },
    }
}

fn binding_mismatch(a: ComparisonBinding, b: ComparisonBinding) -> Option<ReprojectionCause> {
    if a.context != b.context {
        return Some(ReprojectionCause::ContextMismatch);
    }
    if a.scope.comparison() != b.scope.comparison()
        || a.scope.goal() != b.scope.goal()
        || a.scope.probes() != b.scope.probes()
    {
        return Some(ReprojectionCause::ScopeMismatch);
    }
    if a.root_query != b.root_query {
        return Some(ReprojectionCause::QueryMismatch);
    }
    if a.domain != b.domain {
        return Some(ReprojectionCause::DomainMismatch);
    }
    if a.premises != b.premises {
        return Some(ReprojectionCause::PremiseMismatch);
    }
    if a.scope.semantics() != b.scope.semantics() {
        return Some(ReprojectionCause::SemanticsMismatch);
    }
    None
}

fn run_mismatch(a: SolverRun, b: SolverRun) -> Option<ReprojectionCause> {
    if a.status() != ExecutionStatus::Completed || b.status() != ExecutionStatus::Completed {
        return Some(ReprojectionCause::IncompleteExecution);
    }
    match (a.result(), b.result()) {
        (Some(a), Some(b)) => outcome_mismatch(a, b),
        (None, _) | (_, None) => Some(ReprojectionCause::IncompleteExecution),
    }
}

fn outcome_mismatch(a: LogicalResult, b: LogicalResult) -> Option<ReprojectionCause> {
    use LogicalResult::*;
    match (a, b) {
        (Satisfiability(_), Satisfiability(_))
        | (Optimization(_), Optimization(_))
        | (Intervention(_), Intervention(_))
        | (SpatialRelation(_), SpatialRelation(_)) => {}
        (Entailment { models: x, .. }, Entailment { models: y, .. }) => {
            if x != y {
                return Some(ReprojectionCause::QuantificationMismatch);
            }
        }
        (Inference { mode: x, .. }, Inference { mode: y, .. }) => {
            if x != y {
                return Some(ReprojectionCause::ConsequenceModeMismatch);
            }
        }
        (ModelCheck { traces: x, .. }, ModelCheck { traces: y, .. }) => {
            if x != y {
                return Some(ReprojectionCause::TraceDomainMismatch);
            }
        }
        _ => return Some(ReprojectionCause::OutcomeKindMismatch),
    }
    if conflict(a) || conflict(b) {
        return Some(ReprojectionCause::ConflictingSupport);
    }
    if unresolved(a) || unresolved(b) {
        return Some(ReprojectionCause::UnknownOutcome);
    }
    if a == b {
        None
    } else {
        Some(ReprojectionCause::ReportedDisagreement)
    }
}

fn support(result: LogicalResult) -> Option<Support> {
    match result {
        LogicalResult::Entailment { support, .. }
        | LogicalResult::ModelCheck { support, .. }
        | LogicalResult::Inference { support, .. }
        | LogicalResult::Intervention(support)
        | LogicalResult::SpatialRelation(support) => Some(support),
        LogicalResult::Satisfiability(_) | LogicalResult::Optimization(_) => None,
    }
}

fn conflict(result: LogicalResult) -> bool {
    support(result) == Some(Support::Both)
}

fn unresolved(result: LogicalResult) -> bool {
    support(result) == Some(Support::Neither)
        || matches!(
            result,
            LogicalResult::Satisfiability(Satisfiability::Unknown)
                | LogicalResult::Optimization(ObjectiveResult::Unknown)
        )
}

/// Advisory request preserves both current query bindings and the first cause.
/// Additional mismatches remain inspectable through the retained bindings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReprojectionRequest {
    left: ComparisonBinding,
    right: ComparisonBinding,
    cause: ReprojectionCause,
}

impl ReprojectionRequest {
    pub const fn cause(self) -> ReprojectionCause {
        self.cause
    }
    pub const fn bindings(self) -> (ComparisonBinding, ComparisonBinding) {
        (self.left, self.right)
    }

    /// Check live context/query lineage. This consumes no solver or authority.
    pub fn inspect_current(
        self,
        contexts: &ContextConsumer,
        current_left: ComparisonBinding,
        current_right: ComparisonBinding,
    ) -> Result<PendingReprojection, PreparationError> {
        if self.left != current_left || self.right != current_right {
            return Err(PreparationError::QueryBindingMismatch);
        }
        for binding in [self.left, self.right] {
            let state = contexts
                .get(binding.context)
                .ok_or(PreparationError::UnknownContext)?;
            if state.scope() != binding.scope {
                return Err(PreparationError::StaleScope);
            }
        }
        Ok(PendingReprojection { request: self })
    }
}

/// A checked local request identity; the actual reprojector remains unconnected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PendingReprojection {
    request: ReprojectionRequest,
}

impl PendingReprojection {
    pub const fn request(self) -> ReprojectionRequest {
        self.request
    }
    pub const fn state(self) -> &'static str {
        "NeedsEvidence/ReprojectorNotConnected"
    }
}
