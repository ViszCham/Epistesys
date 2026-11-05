//! Recompute reported selection and bind every explanatory row before rendering.
use super::{
    CandidateExplanation, DependencyCoverage, OutputError, PreparedDecision, RenderedDecision,
    MAX_CANDIDATES,
};
use crate::joint_selection::{
    select_joint_reported, AxisKind, JointCandidate, JointSelectionReport, SearchCoverage,
    SelectionError, SelectionPolicy,
};
use lc631_core::{context::SourceRevision, CandidateId};
use lc631_substrate::Digest32;

/// Caller-reported result/measurement correspondence; not authenticated evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExplanationBinding {
    pub candidate: CandidateId,
    pub measurement: Digest32,
    pub result: Digest32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionError {
    CountMismatch,
    SourceMismatch,
    LineageMismatch,
    ConditionalApplicability,
    Selection(SelectionError),
    Output(OutputError),
}

/// Owns the freshly computed report; no mutable report or selected-index input.
pub struct ConnectedDecision<'a, 'v> {
    prepared: PreparedDecision<'a>,
    report: JointSelectionReport<'a, 'v>,
    axes: &'a [AxisKind],
    bindings: &'a [ExplanationBinding],
}

/// Consume the actual bounded TLCX-23 selector, retaining its reported basis.
/// This does not check semantic truth, input measurement provenance, or authority.
pub fn connect_joint_reported<'a, 'v>(
    source: &str,
    policy: &SelectionPolicy<'a>,
    rows: &'a [JointCandidate<'v>],
    coverage: SearchCoverage,
    explanations: &'a [CandidateExplanation<'a>],
    bindings: &'a [ExplanationBinding],
) -> Result<ConnectedDecision<'a, 'v>, ConnectionError> {
    connect_with_coverage(
        source,
        policy,
        rows,
        coverage,
        explanations,
        bindings,
        DependencyCoverage::DeclaredDependenciesOnly,
    )
}

/// The same reported selector with conservative whole-context premise accounting.
/// No unknown semantic dependency is discovered or discharged by this opt-in.
pub fn connect_joint_whole_context_reported<'a, 'v>(
    source: &str,
    policy: &SelectionPolicy<'a>,
    rows: &'a [JointCandidate<'v>],
    coverage: SearchCoverage,
    explanations: &'a [CandidateExplanation<'a>],
    bindings: &'a [ExplanationBinding],
) -> Result<ConnectedDecision<'a, 'v>, ConnectionError> {
    connect_with_coverage(
        source,
        policy,
        rows,
        coverage,
        explanations,
        bindings,
        DependencyCoverage::AllContextPremisesAccounted,
    )
}

fn connect_with_coverage<'a, 'v>(
    source: &str,
    policy: &SelectionPolicy<'a>,
    rows: &'a [JointCandidate<'v>],
    coverage: SearchCoverage,
    explanations: &'a [CandidateExplanation<'a>],
    bindings: &'a [ExplanationBinding],
    dependency_coverage: DependencyCoverage,
) -> Result<ConnectedDecision<'a, 'v>, ConnectionError> {
    if rows.len() != explanations.len() || rows.len() != bindings.len() {
        return Err(ConnectionError::CountMismatch);
    }
    if source.len() > 256 * 1024 || rows.len() > MAX_CANDIDATES {
        return Err(ConnectionError::Output(OutputError::InputBudget));
    }
    let source_revision = SourceRevision::of(source);
    if policy.pins.source != source_revision {
        return Err(ConnectionError::SourceMismatch);
    }
    for ((row, explanation), binding) in rows.iter().zip(explanations).zip(bindings) {
        if row.context.source() != source_revision {
            return Err(ConnectionError::SourceMismatch);
        }
        if row.id != binding.candidate
            || row.measurement_binding != binding.measurement
            || explanation.result != binding.result
            || row.context != explanation.context.revision()
            || row.pins.scope != explanation.context.scope()
        {
            return Err(ConnectionError::LineageMismatch);
        }
    }
    let report =
        select_joint_reported(policy, rows, coverage).map_err(ConnectionError::Selection)?;
    let prepared = match dependency_coverage {
        DependencyCoverage::DeclaredDependenciesOnly => PreparedDecision::prepare(
            source,
            explanations,
            report.recommendation,
            report.pins.goal_profile,
        ),
        DependencyCoverage::AllContextPremisesAccounted => PreparedDecision::prepare_whole_context(
            source,
            explanations,
            report.recommendation,
            report.pins.goal_profile,
        ),
    }
    .map_err(ConnectionError::Output)?;
    if prepared.recommendation() != report.recommendation {
        return Err(ConnectionError::ConditionalApplicability);
    }
    Ok(ConnectedDecision {
        prepared,
        report,
        axes: policy.axes,
        bindings,
    })
}

impl<'a, 'v> ConnectedDecision<'a, 'v> {
    pub const fn dependency_coverage(&self) -> DependencyCoverage {
        self.prepared.dependency_coverage()
    }

    pub fn recommendation(&self) -> Option<usize> {
        self.prepared.recommendation()
    }

    pub fn report(&self) -> &JointSelectionReport<'a, 'v> {
        &self.report
    }

    pub fn render(&self, max_bytes: usize) -> Result<RenderedDecision, OutputError> {
        self.prepared
            .render_connected(max_bytes, &self.report, self.axes, self.bindings)
    }
}
