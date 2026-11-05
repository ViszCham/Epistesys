//! Actual private finite-store observations into advisory reprojection.
//! Temporal-to-temporal only; not a validated heterogeneous semantic bridge.
#![forbid(unsafe_code)]

use super::{
    binding_mismatch, ComparisonBinding, DependencyStatus, PreparationError, ReprojectionCause,
    ReprojectionRequest,
};
use crate::finite_results::{FiniteResultError, FiniteResultId, FiniteResultStore};
use lc631_solvers::temporal::{FiniteTrace, TemporalEvaluation, TemporalQuery, Truth};
use lc631_substrate::Digest32;

/// No detached SolverRun, status, truth or report can enter through this input.
#[derive(Clone, Copy)]
pub struct TemporalObservationRequest<'a> {
    pub observation: FiniteResultId,
    pub query: &'a TemporalQuery,
    pub trace: &'a FiniteTrace,
    pub binding: ComparisonBinding,
    pub dependencies: &'a [Digest32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FiniteBridgeError {
    Store(FiniteResultError),
    Preparation(PreparationError),
    TranslatedQueryMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FiniteValidation {
    Unverified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FiniteCorrespondence {
    SameFiniteOutcomeUnverified,
    NeedsReprojection(ReprojectionCause),
}

/// Reports remain borrowed from their actual private store owners. This object
/// cannot be constructed by supplying caller status flags or serialized reports.
pub struct FiniteBridgeDiagnostic<'store> {
    left: ComparisonBinding,
    right: ComparisonBinding,
    left_report: &'store TemporalEvaluation,
    right_report: &'store TemporalEvaluation,
    dependencies: DependencyStatus,
    correspondence: FiniteCorrespondence,
}

impl FiniteBridgeDiagnostic<'_> {
    pub fn truths(&self) -> (Truth, Truth) {
        (self.left_report.truth(), self.right_report.truth())
    }
    pub const fn validation(&self) -> FiniteValidation {
        FiniteValidation::Unverified
    }
    pub const fn dependencies(&self) -> DependencyStatus {
        self.dependencies
    }
    /// Both reports necessarily share this registered finite evaluator. This
    /// fact is separate from overlap in caller-declared parser/translator pins.
    pub const fn shares_temporal_evaluator(&self) -> bool {
        true
    }
    pub const fn correspondence(&self) -> FiniteCorrespondence {
        self.correspondence
    }
    pub fn reports(&self) -> (&TemporalEvaluation, &TemporalEvaluation) {
        (self.left_report, self.right_report)
    }
    pub fn reprojection(&self) -> Result<ReprojectionRequest, PreparationError> {
        match self.correspondence {
            FiniteCorrespondence::SameFiniteOutcomeUnverified => {
                Err(PreparationError::MissingDiagnostic)
            }
            FiniteCorrespondence::NeedsReprojection(cause) => Ok(ReprojectionRequest {
                left: self.left,
                right: self.right,
                cause,
            }),
        }
    }
    pub const fn claim_boundary(&self) -> &'static str {
        "actual closed finite temporal observations; source correspondence, preservation and independence unverified; no authority"
    }
}

/// Read both actual results before diagnosing any shape or truth difference.
/// Current-context inspection remains the existing ReprojectionRequest seam.
/// Matching outcomes do not validate the declared root-query interpretation.
pub fn analyze_temporal_pair<'store>(
    left_store: &'store FiniteResultStore<'_, '_>,
    left: TemporalObservationRequest<'_>,
    right_store: &'store FiniteResultStore<'_, '_>,
    right: TemporalObservationRequest<'_>,
) -> Result<FiniteBridgeDiagnostic<'store>, FiniteBridgeError> {
    let left_report = read(left_store, left)?;
    let right_report = read(right_store, right)?;
    let dependencies = if left
        .dependencies
        .iter()
        .any(|pin| right.dependencies.contains(pin))
    {
        DependencyStatus::ReportedSharedDependency
    } else {
        DependencyStatus::Unassessed
    };
    let cause = binding_mismatch(left.binding, right.binding).or_else(|| {
        if left_report.profile_digest() != right_report.profile_digest() {
            Some(ReprojectionCause::SemanticsMismatch)
        } else if left_report.trace_digest() != right_report.trace_digest() {
            // Distinct actual input traces are not equal premises merely
            // because caller-declared premise pins happen to match.
            Some(ReprojectionCause::PremiseMismatch)
        } else if left_report.truth() == Truth::Unknown || right_report.truth() == Truth::Unknown {
            Some(ReprojectionCause::UnknownOutcome)
        } else if left_report.truth() != right_report.truth() {
            Some(ReprojectionCause::ReportedDisagreement)
        } else {
            None
        }
    });
    Ok(FiniteBridgeDiagnostic {
        left: left.binding,
        right: right.binding,
        left_report,
        right_report,
        dependencies,
        correspondence: match cause {
            Some(cause) => FiniteCorrespondence::NeedsReprojection(cause),
            None => FiniteCorrespondence::SameFiniteOutcomeUnverified,
        },
    })
}

fn read<'store>(
    store: &'store FiniteResultStore<'_, '_>,
    request: TemporalObservationRequest<'_>,
) -> Result<&'store TemporalEvaluation, FiniteBridgeError> {
    if request.dependencies.len() > 16 {
        return Err(FiniteBridgeError::Preparation(
            PreparationError::TooManyDependencies,
        ));
    }
    for (index, pin) in request.dependencies.iter().enumerate() {
        if request.dependencies[..index].contains(pin) {
            return Err(FiniteBridgeError::Preparation(
                PreparationError::DuplicateDependency,
            ));
        }
    }
    if request.binding.translated_query != request.query.digest() {
        return Err(FiniteBridgeError::TranslatedQueryMismatch);
    }
    store
        .lookup_temporal(
            request.observation,
            request.query,
            request.trace,
            request.binding.context,
            request.binding.scope,
        )
        .map_err(FiniteBridgeError::Store)
}
