//! Preparatory output contracts; no solver, completion, authority or host claims.
#![forbid(unsafe_code)]

mod connector;
mod render;

#[cfg(test)]
mod dependency_tests;
#[cfg(test)]
mod whole_context_tests;

pub use connector::{
    connect_joint_reported, connect_joint_whole_context_reported, ConnectedDecision, ConnectionError,
    ExplanationBinding,
};
use lc631_core::context::{ContextApplicability, ContextState, PremiseBasis, SourceRevision};
use lc631_substrate::Digest32;
pub use render::{ExplanationTl, RenderedDecision};

pub const MEASUREMENT_REVISION: &str = "OperationalMeasurement/finite-formal-v1.4";
const MAX_CANDIDATES: usize = 32;
const MAX_EXPLANATION_BYTES: usize = 8 * 1024;
const MAX_DEPENDENCIES: usize = 256;

/// References are reported lineage only. Presence is not a measured zero or proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FourFacetRefs {
    pub defects: Option<Digest32>,
    pub delta: Option<Digest32>,
    pub uncertainty: Option<Digest32>,
    pub gain: Option<Digest32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyState {
    Given,
    SolverReported,
    Unknown,
    Unsupported,
    Conflict,
    NeedsEvidence,
}

/// Structural accounting only; neither variant validates arbitrary prose or truth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyCoverage {
    DeclaredDependenciesOnly,
    AllContextPremisesAccounted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PremiseDependency {
    pub premise: Digest32,
    pub state: DependencyState,
}

/// Caller-reported text; no constructor can certify its arbitrary semantics.
pub struct CandidateExplanation<'a> {
    pub context: &'a ContextState,
    /// Complete ancestry, original first and direct parent last; empty for original.
    pub context_history: &'a [&'a ContextState],
    pub result: Digest32,
    pub explanation: &'a str,
    pub dependencies: &'a [PremiseDependency],
    pub facets: FourFacetRefs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationState {
    NoRecommendation,
    DeclaredRecommendation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputError {
    EmptyCandidates,
    InputBudget,
    SourceMismatch,
    InvalidSelection,
    EmptyExplanation,
    RawDump,
    ControlCharacter,
    MissingDependency,
    DuplicateDependency,
    InvalidContextHistory,
    UnresolvedDependency,
    UnaccountedContextPremise,
    OutputBudget,
    AllocationFailed,
}

/// Borrows the checked structural contexts and all supplied alternatives.
/// It deliberately has no Complete/Verified/ModelOptimal state or permit conversion.
pub struct PreparedDecision<'a> {
    source: SourceRevision,
    candidates: &'a [CandidateExplanation<'a>],
    recommendation: Option<usize>,
    declared_goal: Option<Digest32>,
    dependency_coverage: DependencyCoverage,
}

impl<'a> PreparedDecision<'a> {
    pub fn prepare(
        source: &str,
        candidates: &'a [CandidateExplanation<'a>],
        proposed_recommendation: Option<usize>,
        declared_goal: Option<Digest32>,
    ) -> Result<Self, OutputError> {
        if candidates.is_empty() {
            return Err(OutputError::EmptyCandidates);
        }
        if source.len() > 256 * 1024 || candidates.len() > MAX_CANDIDATES {
            return Err(OutputError::InputBudget);
        }
        let source = SourceRevision::of(source);
        for candidate in candidates {
            validate_candidate(source, candidate)?;
        }
        if proposed_recommendation.is_some_and(|index| index >= candidates.len()) {
            return Err(OutputError::InvalidSelection);
        }
        // G22: absence of a comparable goal profile cannot yield recommendation.
        // A supplied digest remains a declaration, not verified GoalProfile evidence.
        let recommendation = proposed_recommendation.filter(|&index| {
            declared_goal.is_some()
                && candidates[index].context.applicability() == ContextApplicability::Unconditional
        });
        Ok(Self {
            source,
            candidates,
            recommendation,
            declared_goal,
            dependency_coverage: DependencyCoverage::DeclaredDependenciesOnly,
        })
    }

    /// Conservatively account for every premise in each supplied current context.
    /// Given remains an input declaration. This is not prose dependency extraction,
    /// a semantic receipt, or a check of excluded/unknown external dependencies.
    pub fn prepare_whole_context(
        source: &str,
        candidates: &'a [CandidateExplanation<'a>],
        proposed_recommendation: Option<usize>,
        declared_goal: Option<Digest32>,
    ) -> Result<Self, OutputError> {
        let mut prepared =
            Self::prepare(source, candidates, proposed_recommendation, declared_goal)?;
        for candidate in candidates {
            if candidate.context.premises().len() > MAX_DEPENDENCIES {
                return Err(OutputError::InputBudget);
            }
            // prepare already rejected duplicate/foreign IDs and unresolved bases.
            // Every current premise must also be present; an empty reported list
            // cannot hide a premise in a nonempty structural context.
            for premise in candidate.context.premises() {
                if !candidate
                    .dependencies
                    .iter()
                    .any(|entry| entry.premise == premise.id())
                {
                    return Err(OutputError::UnaccountedContextPremise);
                }
            }
        }
        prepared.dependency_coverage = DependencyCoverage::AllContextPremisesAccounted;
        Ok(prepared)
    }

    pub const fn dependency_coverage(&self) -> DependencyCoverage {
        self.dependency_coverage
    }

    pub const fn state(&self) -> PreparationState {
        match self.recommendation {
            Some(_) => PreparationState::DeclaredRecommendation,
            None => PreparationState::NoRecommendation,
        }
    }

    pub const fn recommendation(&self) -> Option<usize> {
        self.recommendation
    }

    pub fn candidates(&self) -> &[CandidateExplanation<'a>] {
        self.candidates
    }
}

fn validate_candidate(
    source: SourceRevision,
    candidate: &CandidateExplanation<'_>,
) -> Result<(), OutputError> {
    if candidate.context.revision().source() != source {
        return Err(OutputError::SourceMismatch);
    }
    validate_history(source, candidate)?;
    let text = candidate.explanation.trim();
    if text.is_empty() {
        return Err(OutputError::EmptyExplanation);
    }
    if text.starts_with('{') || text.starts_with('[') || text.starts_with("```json") {
        return Err(OutputError::RawDump);
    }
    if candidate
        .explanation
        .chars()
        .any(|value| value.is_control())
    {
        return Err(OutputError::ControlCharacter);
    }
    if candidate.explanation.len() > MAX_EXPLANATION_BYTES
        || candidate.dependencies.len() > MAX_DEPENDENCIES
    {
        return Err(OutputError::InputBudget);
    }
    for (index, dependency) in candidate.dependencies.iter().enumerate() {
        if candidate.dependencies[..index]
            .iter()
            .any(|prior| prior.premise == dependency.premise)
        {
            return Err(OutputError::DuplicateDependency);
        }
        // SolverReported is intentionally insufficient for a closed partial answer.
        if dependency.state != DependencyState::Given {
            return Err(OutputError::UnresolvedDependency);
        }
        let premise = candidate
            .context
            .premises()
            .iter()
            .find(|premise| premise.id() == dependency.premise)
            .ok_or(OutputError::MissingDependency)?;
        if premise.basis() != PremiseBasis::Given {
            return Err(OutputError::UnresolvedDependency);
        }
    }
    Ok(())
}

fn validate_history(
    source: SourceRevision,
    candidate: &CandidateExplanation<'_>,
) -> Result<(), OutputError> {
    if candidate.context_history.len() > 256 {
        return Err(OutputError::InputBudget);
    }
    let mut parent = None;
    for state in candidate.context_history {
        if state.revision().source() != source || state.parent() != parent {
            return Err(OutputError::InvalidContextHistory);
        }
        if state.scope() != candidate.context.scope() {
            return Err(OutputError::InvalidContextHistory);
        }
        parent = Some(state.revision());
    }
    if candidate.context.parent() != parent {
        return Err(OutputError::InvalidContextHistory);
    }
    Ok(())
}
