use super::types::*;
use lc631_analysis::creativity_measurement::Rational;
use std::cmp::Ordering;

/// Conservative relation over reported intervals only; never a semantic proof.
pub fn compare_rows(
    axes: &[AxisKind],
    left: &[AxisValue<'_>],
    right: &[AxisValue<'_>],
) -> Result<PairRelation, SelectionError> {
    if axes.is_empty() || left.len() != axes.len() || right.len() != axes.len() {
        return Err(SelectionError::AxisCountMismatch);
    }
    let (mut left_all, mut right_all) = (true, true);
    let (mut left_strict, mut right_strict) = (false, false);
    let (mut same, mut exact) = (true, true);
    for ((axis, a), b) in axes.iter().zip(left).zip(right) {
        validate_value(a, gain_axis(*axis))?;
        validate_value(b, gain_axis(*axis))?;
        let (Some(a_value), Some(b_value)) = (a.interval(), b.interval()) else {
            return Ok(PairRelation::Unresolved);
        };
        same &= a_value.lower() == b_value.lower() && a_value.upper() == b_value.upper();
        exact &= matches!(a, AxisValue::Exact(_)) && matches!(b, AxisValue::Exact(_));
        let (l, r) = if axis.maximize() {
            (
                a_value.lower().cmp(b_value.upper()),
                b_value.lower().cmp(a_value.upper()),
            )
        } else {
            (
                b_value.lower().cmp(a_value.upper()),
                a_value.lower().cmp(b_value.upper()),
            )
        };
        left_all &= l != Ordering::Less;
        right_all &= r != Ordering::Less;
        left_strict |= l == Ordering::Greater;
        right_strict |= r == Ordering::Greater;
    }
    Ok(if left_all && left_strict {
        PairRelation::LeftDominates
    } else if right_all && right_strict {
        PairRelation::RightDominates
    } else if same && exact {
        PairRelation::EquivalentInScope
    } else if same {
        PairRelation::SameBounds
    } else {
        PairRelation::Unresolved
    })
}

/// A pure bounded consumer. The caller retains authentication, validator and
/// fixed-registry duties; this report grants no execution or completion state.
pub fn select_joint_reported<'a, 'v>(
    policy: &SelectionPolicy<'_>,
    candidates: &'a [JointCandidate<'v>],
    coverage: SearchCoverage,
) -> Result<JointSelectionReport<'a, 'v>, SelectionError> {
    validate_input(policy, candidates)?;
    let mut dispositions = reserve(candidates.len())?;
    for row in candidates {
        dispositions.push(classify(policy, row));
    }
    // Keep aliases explicitly; only fully equal reported rows are merged.
    for index in 0..candidates.len() {
        if dispositions[index] != Disposition::Frontier {
            continue;
        }
        for earlier in 0..index {
            if dispositions[earlier] == Disposition::Frontier
                && duplicate(&candidates[index], &candidates[earlier])
            {
                dispositions[index] = Disposition::Duplicate {
                    representative: earlier,
                };
                break;
            }
        }
    }
    // Compare the entire admissible set, without a top16 prefilter.
    for index in 0..candidates.len() {
        if dispositions[index] != Disposition::Frontier {
            continue;
        }
        for other in 0..candidates.len() {
            if index == other || !comparable_disposition(dispositions[other]) {
                continue;
            }
            if compare_rows(policy.axes, candidates[other].axes, candidates[index].axes)?
                == PairRelation::LeftDominates
            {
                dispositions[index] = Disposition::Dominated { by: other };
                break;
            }
        }
    }
    finish_report(policy, candidates, dispositions, coverage)
}

fn finish_report<'a, 'v>(
    policy: &SelectionPolicy<'_>,
    candidates: &'a [JointCandidate<'v>],
    dispositions: Vec<Disposition>,
    coverage: SearchCoverage,
) -> Result<JointSelectionReport<'a, 'v>, SelectionError> {
    let mut frontier = reserve(candidates.len())?;
    for (index, state) in dispositions.iter().enumerate() {
        if *state == Disposition::Frontier {
            frontier.push(index);
        }
    }
    let (recommendation, recommendation_state) = recommend(policy, candidates, &frontier)?;
    let mut alternatives = reserve(candidates.len())?;
    alternatives.extend((0..candidates.len()).filter(|index| Some(*index) != recommendation));
    let incomparable = dispositions.iter().any(|state| {
        matches!(
            state,
            Disposition::OtherScope | Disposition::MapBackFailure | Disposition::NeedsEvidence
        )
    });
    let optimality = match coverage {
        SearchCoverage::BudgetExhausted => SelectionOptimality::BudgetLimited,
        SearchCoverage::EvaluatedSet if incomparable || recommendation.is_none() => {
            SelectionOptimality::Incomparable
        }
        SearchCoverage::EvaluatedSet => SelectionOptimality::BestAmongEvaluated,
    };
    Ok(JointSelectionReport {
        pins: policy.pins,
        candidates,
        dispositions,
        frontier,
        recommendation,
        alternatives,
        recommendation_state,
        optimality,
    })
}

fn recommend(
    policy: &SelectionPolicy<'_>,
    candidates: &[JointCandidate<'_>],
    frontier: &[usize],
) -> Result<(Option<usize>, RecommendationState), SelectionError> {
    if policy.pins.goal_profile.is_none() {
        return Ok((None, RecommendationState::NoGoalProfile));
    }
    let goals: Vec<_> = policy
        .axes
        .iter()
        .enumerate()
        .filter_map(|(index, axis)| matches!(axis, AxisKind::Goal(_)).then_some(index))
        .collect();
    if goals.is_empty() {
        return Ok((None, RecommendationState::NoGoalObservation));
    }
    if goals.len() > 1 && policy.representative_order.is_none() {
        return Ok((None, RecommendationState::MissingRepresentativePolicy));
    }
    let mut best = None;
    for index in frontier {
        let row = &candidates[*index];
        if row.axes.iter().any(|value| value.interval().is_none())
            || row.total_loss.interval().is_none()
        {
            continue;
        }
        best = match best {
            None => Some(*index),
            Some(previous) => {
                if representative_cmp(policy, row, &candidates[previous], &goals)
                    == Ordering::Greater
                {
                    Some(*index)
                } else {
                    Some(previous)
                }
            }
        };
    }
    if let Some(index) = best {
        if frontier.iter().any(|other| {
            *other != index
                && candidates[*other]
                    .axes
                    .iter()
                    .all(|value| value.interval().is_some())
                && candidates[*other].total_loss.interval().is_some()
                && representative_cmp(policy, &candidates[index], &candidates[*other], &goals)
                    == Ordering::Equal
        }) {
            return Ok((None, RecommendationState::NeedsEvidence));
        }
    }
    let state = if best.is_some() {
        RecommendationState::OperationalRepresentative
    } else if frontier.is_empty() {
        RecommendationState::NoEligibleCandidate
    } else {
        RecommendationState::NeedsEvidence
    };
    Ok((best, state))
}

fn representative_cmp(
    policy: &SelectionPolicy<'_>,
    a: &JointCandidate<'_>,
    b: &JointCandidate<'_>,
    goals: &[usize],
) -> Ordering {
    if let Some(order) = policy.representative_order {
        for index in order {
            let comparison =
                compare_endpoint(policy.axes[*index], &a.axes[*index], &b.axes[*index]);
            if comparison != Ordering::Equal {
                return comparison;
            }
        }
    } else {
        for index in goals {
            let comparison =
                compare_endpoint(policy.axes[*index], &a.axes[*index], &b.axes[*index]);
            if comparison != Ordering::Equal {
                return comparison;
            }
        }
        let comparison = compare_endpoint(AxisKind::SemanticLoss, &a.total_loss, &b.total_loss);
        if comparison != Ordering::Equal {
            return comparison;
        }
        for (index, axis) in policy
            .axes
            .iter()
            .enumerate()
            .filter(|(_, axis)| primitive_axis(**axis))
        {
            let comparison = compare_endpoint(*axis, &a.axes[index], &b.axes[index]);
            if comparison != Ordering::Equal {
                return comparison;
            }
        }
    }
    // Ascending canonical bytes win a display tie; CandidateId is never read.
    b.canonical_key.cmp(a.canonical_key)
}

fn compare_endpoint(axis: AxisKind, a: &AxisValue<'_>, b: &AxisValue<'_>) -> Ordering {
    match (a.interval(), b.interval()) {
        (Some(a), Some(b)) if axis.maximize() => a.lower().cmp(b.lower()),
        (Some(a), Some(b)) => b.upper().cmp(a.upper()),
        _ => Ordering::Equal, // Only reachable for incomplete reported rows; not a numeric zero.
    }
}

fn classify(policy: &SelectionPolicy<'_>, row: &JointCandidate<'_>) -> Disposition {
    if row.pins != policy.pins || row.context.source() != policy.pins.source {
        return Disposition::OtherScope;
    }
    match (row.hard_gate, row.map_back) {
        (ReportedGate::Fail, _) => Disposition::HardFailure,
        (_, ReportedGate::Fail) => Disposition::MapBackFailure,
        (ReportedGate::NeedsEvidence, _) | (_, ReportedGate::NeedsEvidence) => {
            Disposition::NeedsEvidence
        }
        (ReportedGate::Pass, ReportedGate::Pass) => Disposition::Frontier,
    }
}

fn comparable_disposition(state: Disposition) -> bool {
    matches!(state, Disposition::Frontier | Disposition::Dominated { .. })
}

fn duplicate(a: &JointCandidate<'_>, b: &JointCandidate<'_>) -> bool {
    a.canonical_key == b.canonical_key
        && a.measurement_binding == b.measurement_binding
        && a.context == b.context
        && a.pins == b.pins
        && a.axes.iter().zip(b.axes).all(|(a, b)| same_value(a, b))
        && same_value(&a.total_loss, &b.total_loss)
}

fn same_value(a: &AxisValue<'_>, b: &AxisValue<'_>) -> bool {
    match (a, b) {
        (AxisValue::Exact(a), AxisValue::Exact(b))
        | (AxisValue::Bounded(a), AxisValue::Bounded(b))
        | (AxisValue::Estimated(a), AxisValue::Estimated(b)) => {
            a.lower() == b.lower() && a.upper() == b.upper()
        }
        (AxisValue::Unavailable(a), AxisValue::Unavailable(b)) => a == b,
        _ => false,
    }
}

fn reserve<T>(count: usize) -> Result<Vec<T>, SelectionError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| SelectionError::AllocationFailed)?;
    Ok(values)
}

fn gain_axis(axis: AxisKind) -> bool {
    matches!(
        axis,
        AxisKind::CoverageGain
            | AxisKind::CompressionGain
            | AxisKind::BridgeGain
            | AxisKind::PredictionGain
            | AxisKind::ExplainGain
            | AxisKind::DesignGain
            | AxisKind::TransferGain
            | AxisKind::SolveGain
    )
}
fn primitive_axis(axis: AxisKind) -> bool {
    !matches!(
        axis,
        AxisKind::Goal(_)
            | AxisKind::ReconstructionLoss
            | AxisKind::SemanticLoss
            | AxisKind::TemporalLoss
            | AxisKind::SpatialLoss
    )
}

fn validate_value(value: &AxisValue<'_>, signed: bool) -> Result<(), SelectionError> {
    if let Some(interval) = value.interval() {
        if matches!(value, AxisValue::Exact(_)) && !interval.is_exact() {
            return Err(SelectionError::InexactExactLabel);
        }
        let zero = Rational::from_integer(0.into());
        let one = Rational::from_integer(1.into());
        let low = if signed { -one.clone() } else { zero };
        if interval.lower() < &low || interval.upper() > &one {
            return Err(SelectionError::OutOfRange);
        }
    }
    Ok(())
}

fn validate_input(
    policy: &SelectionPolicy<'_>,
    rows: &[JointCandidate<'_>],
) -> Result<(), SelectionError> {
    if policy.max_candidates == 0
        || policy.max_candidates > 2_048
        || policy.max_canonical_bytes == 0
        || policy.max_canonical_bytes > 128 * 1024 * 1024
    {
        return Err(SelectionError::InvalidLimits);
    }
    if rows.is_empty() {
        return Err(SelectionError::EmptyCandidates);
    }
    if rows.len() > policy.max_candidates {
        return Err(SelectionError::ResourceExhausted);
    }
    if policy.axes.is_empty() || policy.axes.len() > 64 {
        return Err(SelectionError::InvalidAxes);
    }
    if !policy.declared_goal_solve_overlap
        && policy.axes.contains(&AxisKind::SolveGain)
        && policy
            .axes
            .iter()
            .any(|axis| matches!(axis, AxisKind::Goal(_)))
    {
        return Err(SelectionError::InvalidAxes);
    }
    for (index, axis) in policy.axes.iter().enumerate() {
        if policy.axes[..index].contains(axis) {
            return Err(SelectionError::InvalidAxes);
        }
    }
    if let Some(order) = policy.representative_order {
        if order.len() != policy.axes.len()
            || order
                .iter()
                .enumerate()
                .any(|(i, x)| *x >= policy.axes.len() || order[..i].contains(x))
        {
            return Err(SelectionError::InvalidRepresentativeOrder);
        }
    }
    let mut bytes = 0usize;
    for (index, row) in rows.iter().enumerate() {
        if rows[..index].iter().any(|other| other.id == row.id) {
            return Err(SelectionError::DuplicateCandidateId);
        }
        if row.axes.len() != policy.axes.len() {
            return Err(SelectionError::AxisCountMismatch);
        }
        if row.canonical_key.is_empty() {
            return Err(SelectionError::MissingCanonicalKey);
        }
        bytes = bytes
            .checked_add(row.canonical_key.len())
            .ok_or(SelectionError::ResourceExhausted)?;
        if bytes > policy.max_canonical_bytes {
            return Err(SelectionError::ResourceExhausted);
        }
        for (axis, value) in policy.axes.iter().zip(row.axes) {
            validate_value(value, gain_axis(*axis))?;
        }
        validate_value(&row.total_loss, false)?;
    }
    Ok(())
}
