//! One actual selection/render pass over all retained feasible native witnesses.
use super::super::{text, CanonicalRendered, DecisionParts, SelectionSummary};
use super::{digest, facets, incumbent};
use crate::{
    canonical_solve::evaluation::{BooleanEvaluationState, OwnedBooleanMeasurement},
    canonical_solve::types::*,
    facet_ledger as l, joint_selection as j,
    recommendation_output::{self as r, CandidateExplanation, DependencyState, PremiseDependency},
};
use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::CandidateId;
use lc631_substrate::Digest32;

fn reserved<T>(count: usize) -> Result<Vec<T>, PostprocessError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| PostprocessError::AllocationFailed)?;
    Ok(values)
}

fn semantic_axis<'a>(
    receipt: Option<&'a OwnedBooleanMeasurement<'_>>,
) -> Result<j::AxisValue<'a>, PostprocessError> {
    let Some(receipt) = receipt else {
        return Ok(j::AxisValue::Unavailable(j::MissingAxis::MissingEvaluator));
    };
    if let Some(value) = receipt.semantic() {
        if receipt.state() != BooleanEvaluationState::Measured {
            return Err(PostprocessError::SourceBinding);
        }
        return Ok(if value.is_exact() {
            j::AxisValue::Exact(value)
        } else {
            j::AxisValue::Bounded(value)
        });
    }
    let reason = match receipt.state() {
        BooleanEvaluationState::UnsupportedInteger
        | BooleanEvaluationState::AtomLimit
        | BooleanEvaluationState::UnsupportedFormal
        | BooleanEvaluationState::SourceComplexityLimit => j::MissingAxis::Unsupported,
        // No fixed atom probes means an empty denominator, never measured zero.
        BooleanEvaluationState::EmptyAtomSet => j::MissingAxis::Undefined,
        // A model row cannot legitimately carry either of these no-value states.
        BooleanEvaluationState::Measured | BooleanEvaluationState::MissingNativeModel => {
            return Err(PostprocessError::SourceBinding);
        }
    };
    Ok(j::AxisValue::Unavailable(reason))
}

#[allow(clippy::too_many_arguments)] // All actual owner/scope inputs stay explicit.
pub(super) fn build(
    evidence: &NativeEvidence<'_, '_>,
    mut ledger: l::FacetLedger,
    result: Digest32,
    measurement: Digest32,
    cap: usize,
    measured: Option<&crate::canonical_solve::evaluation::OwnedBooleanMeasurement<'_>>,
    objective: Option<&crate::canonical_solve::optimization::OwnedObjectiveStep<'_>>,
) -> Result<DecisionParts, PostprocessError> {
    let request = evidence.request;
    let FormalInput::Problem(problem) = request.input else {
        return Err(PostprocessError::SourceBinding);
    };
    let native = evidence
        .native_model()
        .ok_or(PostprocessError::SourceBinding)?;
    if native.context() != problem.context()
        || !std::ptr::eq(native.model().query(), problem.query())
    {
        return Err(PostprocessError::SourceBinding);
    }
    let goal = match request.goal {
        GoalPolicy::SatisfyingModel { .. } => {
            Some(RationalInterval::exact_ratio(1, 1).map_err(|_| PostprocessError::SourceBinding)?)
        }
        GoalPolicy::DeclaredSourceObjective { .. } => {
            objective.and_then(|s| s.goal_quality()).cloned()
        }
        GoalPolicy::NoGoal => None,
    };
    let initial_goal = incumbent::goal(evidence, objective)?;
    let initial = if initial_goal.is_some() {
        let (actual, initial_result, initial_measurement) = facets(evidence, None, None)?;
        for binding in actual.transforms() {
            ledger
                .admit_transform(*binding)
                .map_err(PostprocessError::Ledger)?;
        }
        for entry in actual.entries() {
            ledger.append(*entry).map_err(PostprocessError::Ledger)?;
        }
        Some((initial_result, initial_measurement))
    } else {
        None
    };
    let count = if initial.is_some() { 2 } else { 1 };
    let evidence_revision = initial.map_or(result, |(r, _)| {
        digest(
            b"canonical.retained-native-witness-set.v1",
            &[result.as_bytes(), r.as_bytes()],
        )
    });
    let pins = j::SelectionPins {
        source: request.source_revision(),
        scope: request.context.scope(),
        baseline: request.source_revision().digest(),
        hard_invariants: request.source_revision().digest(),
        policy: request.context.scope().semantics(),
        goal_profile: goal.as_ref().map(|_| request.context.scope().goal()),
        evidence_revision,
    };
    let all_kinds = [
        j::AxisKind::Goal(request.context.scope().goal()),
        j::AxisKind::SemanticLoss,
    ];
    let selected_values = [
        match goal.as_ref() {
            Some(g) => j::AxisValue::Exact(g),
            None => j::AxisValue::Unavailable(j::MissingAxis::MissingEvaluator),
        },
        semantic_axis(measured)?,
    ];
    // The incumbent never borrows a measurement computed for the selected row.
    let initial_values = [
        match initial_goal.as_ref() {
            Some(g) => j::AxisValue::Exact(g),
            None => j::AxisValue::Unavailable(j::MissingAxis::MissingEvaluator),
        },
        j::AxisValue::Unavailable(j::MissingAxis::MissingEvaluator),
    ];
    let axes = if measured.is_some() { 2 } else { 1 };
    let kinds = &all_kinds[..axes];
    let explanation = text::model_explanation(evidence, measured, objective)?;
    let initial_explanation = initial_goal
        .as_ref()
        .map(|g| text::incumbent_explanation(evidence, g))
        .transpose()?;
    let mut rows = reserved(count)?;
    rows.push(j::JointCandidate {
        id: CandidateId(1),
        context: problem.context(),
        pins,
        hard_gate: j::ReportedGate::Pass,
        map_back: j::ReportedGate::NeedsEvidence,
        axes: &selected_values[..axes],
        total_loss: j::AxisValue::Unavailable(j::MissingAxis::MissingEvaluator),
        canonical_key: explanation.as_bytes(),
        measurement_binding: measurement,
    });
    if let (Some((_, m)), Some(key)) = (initial, initial_explanation.as_ref()) {
        rows.push(j::JointCandidate {
            id: CandidateId(2),
            context: problem.context(),
            pins,
            hard_gate: j::ReportedGate::Pass,
            map_back: j::ReportedGate::NeedsEvidence,
            axes: &initial_values[..axes],
            total_loss: j::AxisValue::Unavailable(j::MissingAxis::MissingEvaluator),
            canonical_key: key.as_bytes(),
            measurement_binding: m,
        });
    }
    let policy = j::SelectionPolicy {
        pins,
        axes: kinds,
        representative_order: None,
        declared_goal_solve_overlap: false,
        max_candidates: count,
        max_canonical_bytes: 8192 * count,
    };
    let mut dependencies = reserved(request.context.premises().len())?;
    for p in request.context.premises() {
        dependencies.push(PremiseDependency {
            premise: p.id(),
            state: DependencyState::Given,
        });
    }
    let mut explanations = reserved(count)?;
    explanations.push(CandidateExplanation {
        context: request.context,
        context_history: &[],
        result,
        explanation: &explanation,
        dependencies: &dependencies,
        facets: r::FourFacetRefs {
            defects: measured.and_then(|r| r.facets()).and_then(|l| {
                l.entries()
                    .iter()
                    .any(|e| matches!(e.facet, l::Facet::Defect(_)))
                    .then_some(measurement)
            }),
            delta: None,
            uncertainty: Some(measurement),
            gain: None,
        },
    });
    let mut bindings = reserved(count)?;
    bindings.push(r::ExplanationBinding {
        candidate: CandidateId(1),
        measurement,
        result,
    });
    if let (Some((r, m)), Some(key)) = (initial, initial_explanation.as_ref()) {
        explanations.push(CandidateExplanation {
            context: request.context,
            context_history: &[],
            result: r,
            explanation: key,
            dependencies: &dependencies,
            facets: r::FourFacetRefs {
                defects: None,
                delta: None,
                uncertainty: Some(m),
                gain: None,
            },
        });
        bindings.push(r::ExplanationBinding {
            candidate: CandidateId(2),
            measurement: m,
            result: r,
        });
    }
    let connected = r::connect_joint_whole_context_reported(
        request.source(),
        &policy,
        &rows,
        j::SearchCoverage::EvaluatedSet,
        &explanations,
        &bindings,
    )
    .map_err(PostprocessError::Connection)?;
    let report = connected.report();
    let selection = SelectionSummary {
        candidate_count: report.candidates.len(),
        frontier_count: report.frontier.len(),
        recommendation: report.recommendation.map(|i| report.candidates[i].id),
        recommendation_state: report.recommendation_state,
        optimality: report.optimality,
    };
    let rendered =
        CanonicalRendered::Conditional(connected.render(cap).map_err(PostprocessError::Output)?);
    let pending = if problem.goal().is_some() {
        NoRecommendationReason::DeferredSourceObjective
    } else if goal.is_none() {
        NoRecommendationReason::NoDeclaredGoal
    } else {
        NoRecommendationReason::MissingTranslationEvaluator
    };
    Ok(DecisionParts {
        ledger,
        rendered,
        selection: Some(selection),
        goal,
        status: CanonicalStatus::ConditionalModel,
        pending,
    })
}
