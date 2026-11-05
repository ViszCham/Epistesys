//! Derive an alternative only from two retained actual checked witnesses.
use crate::canonical_solve::{
    optimization::{ObjectiveStepState, OwnedObjectiveStep},
    types::*,
};
use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_solvers::smt::ObjectiveDirection;
use lc631_substrate::measurement_numeric::{BigInt, Rational};

pub(super) fn goal(
    evidence: &NativeEvidence<'_, '_>,
    objective: Option<&OwnedObjectiveStep<'_>>,
) -> Result<Option<RationalInterval>, PostprocessError> {
    let Some(step) = objective.filter(|s| s.state() == ObjectiveStepState::ImprovedModelRechecked)
    else {
        return Ok(None);
    };
    let FormalInput::Problem(problem) = evidence.request.input else {
        return Err(PostprocessError::SourceBinding);
    };
    let native = evidence
        .native_model()
        .ok_or(PostprocessError::SourceBinding)?;
    let goal = problem.goal().ok_or(PostprocessError::SourceBinding)?;
    let initial = native
        .model()
        .integer(goal.symbol())
        .and_then(|v| i64::try_from(v).ok())
        .ok_or(PostprocessError::SourceBinding)?;
    let selected = step
        .selected_value()
        .ok_or(PostprocessError::SourceBinding)?;
    let improves = match goal.direction() {
        ObjectiveDirection::Minimize => selected < initial,
        ObjectiveDirection::Maximize => selected > initial,
    };
    if !matches!(
        evidence.request.goal,
        GoalPolicy::DeclaredSourceObjective { .. }
    ) || native.context() != problem.context()
        || !std::ptr::eq(native.model().query(), problem.query())
        || step.initial_value() != Some(initial)
        || !improves
        || step.improved_stored_results() != 1
        || step.native_error().is_some()
        || step.selected_report().is_none()
        || step.third_binding_at_render() != step.third_observation_binding()
        || step.lower() != goal.lower()
        || step.upper() != goal.upper()
        || step.direction() != goal.direction()
        || initial < goal.lower()
        || initial > goal.upper()
        || selected < goal.lower()
        || selected > goal.upper()
    {
        return Err(PostprocessError::SourceBinding);
    }
    let width = i128::from(goal.upper()) - i128::from(goal.lower());
    if width <= 0 {
        return Err(PostprocessError::SourceBinding);
    }
    let numerator = match goal.direction() {
        ObjectiveDirection::Minimize => i128::from(goal.upper()) - i128::from(initial),
        ObjectiveDirection::Maximize => i128::from(initial) - i128::from(goal.lower()),
    };
    let value = Rational::new(BigInt::from(numerator), BigInt::from(width));
    RationalInterval::new(value.clone(), value)
        .map(Some)
        .map_err(|_| PostprocessError::SourceBinding)
}
