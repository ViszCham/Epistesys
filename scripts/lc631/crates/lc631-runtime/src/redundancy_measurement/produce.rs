//! 日本語: 実有限評価を保持し、全pair完了後だけrole別classからRを作る。
//! English: Retain actual finite evaluations; form R only after every required pair completes.
use super::*;
use lc631_analysis::creativity_measurement::{
    ComponentMetric, ComponentState, IndependenceStatus, InputBasis, MeasurementBinding,
    MeasurementGap, SemanticValidation,
};
use lc631_substrate::resource::ResourceReservationView;
use sha2::{Digest, Sha256};

impl<'pairs, 'candidate, 'plan, 'input, 'view>
    OwnedRedundancy<'pairs, 'candidate, 'plan, 'input, 'view>
{
    pub fn produce(
        pair_plan: &'pairs PairPlan<'candidate, 'plan, 'input, 'view>,
        view: &'view ResourceReservationView<'_>,
    ) -> Result<Self, Error> {
        pair_plan.check(view)?;
        if pair_plan.started.replace(true) {
            return Err(Error::Conflict);
        }
        let root = pair_plan.candidate.root;
        let bytes = resources::backing::<formal_boolean::Evaluation<'pairs>>(
            pair_plan.requests.len(),
            std::mem::size_of::<Self>() + 4096,
        )?;
        let retained = ChainClaim::acquire(&root.budget, root.limits, bytes, 4096, view)?;
        let scratch = root::scratch(root.limits, view)?;
        let mut observations = resources::vector(pair_plan.requests.len())?;
        let mut counts = NativeCounts {
            consistency_calls: 0,
            pair_calls: 0,
            pairs_required: pair_plan.pairs_required,
            ast_visits: 0,
            map_comparisons: 0,
            charged_work: root.budget.spent(),
        };
        for (index, request) in pair_plan.requests.iter().enumerate() {
            pair_plan.check(view)?;
            let observation = formal_boolean::evaluate(request, root.limits.max_call_work)
                .map_err(Error::Formal)?;
            pair_plan.check(view)?;
            let visits = match &observation {
                formal_boolean::Evaluation::Computed(c) => c.ast_visits(),
                formal_boolean::Evaluation::NoComparable { ast_visits, .. }
                | formal_boolean::Evaluation::Unknown { ast_visits } => *ast_visits,
            };
            counts.ast_visits = syntax::add(counts.ast_visits, visits)?;
            // Conservative modeled comparisons; NOT an instrumented lookup count.
            counts.map_comparisons =
                syntax::add(counts.map_comparisons, pair_plan.costs[index].map)?;
            if index == 0 {
                counts.consistency_calls += 1;
            } else {
                counts.pair_calls += 1;
            }
            observations.push(observation);
        }
        let state = classify(pair_plan, &observations)?;
        let report = report(pair_plan, &observations, &state)?;
        scratch.check().map_err(Error::Resource)?;
        let result = Self {
            pair_plan,
            observations,
            counts,
            state,
            report,
            retained,
        };
        result.check(view)?;
        Ok(result)
    }
}
fn classify(
    plan: &PairPlan<'_, '_, '_, '_>,
    observations: &[formal_boolean::Evaluation<'_>],
) -> Result<RedundancyState, Error> {
    // Consistency is observed even for zero units, singletons or distinct roles.
    for value in observations {
        match value {
            formal_boolean::Evaluation::Unknown { .. } => {
                return Ok(RedundancyState::ResourceExhausted)
            }
            formal_boolean::Evaluation::NoComparable { reason, .. } => {
                return Ok(RedundancyState::NoComparable(*reason))
            }
            formal_boolean::Evaluation::Computed(_) => {}
        }
    }
    if let Some(gap) = plan.candidate.gap {
        return Ok(RedundancyState::Unknown(gap));
    }
    if plan.pairs.len() as u64 != plan.pairs_required || observations.len() != plan.pairs.len() + 1
    {
        return Err(Error::Conflict);
    }
    let n = plan.candidate.root.input.slots.len();
    let mut equivalent = [[false; 16]; 16];
    for (i, row) in equivalent.iter_mut().enumerate().take(n) {
        row[i] = true;
    }
    for (&(i, j), observation) in plan.pairs.iter().zip(&observations[1..]) {
        let formal_boolean::Evaluation::Computed(c) = observation else {
            return Err(Error::Conflict);
        };
        let equal = c.soundness() == formal_boolean::DirectionResult::Preserved
            && c.completeness() == formal_boolean::DirectionResult::Preserved;
        equivalent[i][j] = equal;
        equivalent[j][i] = equal;
    }
    // Verify the complete relation instead of merging an unchecked partial graph.
    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                if equivalent[i][j] && equivalent[j][k] && !equivalent[i][k] {
                    return Err(Error::Conflict);
                }
            }
        }
    }
    let classes = (0..n)
        .filter(|&i| !(0..i).any(|j| equivalent[i][j]))
        .count();
    let value = RationalInterval::exact_ratio((n - classes) as i64, n.max(1) as i64)
        .map_err(|_| Error::ArithmeticOverflow)?;
    Ok(RedundancyState::Measured(value))
}
fn report(
    plan: &PairPlan<'_, '_, '_, '_>,
    observations: &[formal_boolean::Evaluation<'_>],
    state: &RedundancyState,
) -> Result<ComponentReport, Error> {
    let root = plan.candidate.root;
    let producer = syntax::hash(
        b"epistesys.redundancy.producer.v1\0",
        &[
            PROFILE.as_bytes(),
            b"formal_boolean::evaluate/exhaustive-given-shared-ir",
        ],
    );
    let mut result = Sha256::new();
    result.update(b"epistesys.redundancy.actual-result.v1\0");
    result.update(plan.digest.as_bytes());
    result.update(root.comparison_digest.as_bytes());
    result.update(plan.candidate.candidate.0.to_le_bytes());
    for observation in observations {
        match observation {
            formal_boolean::Evaluation::Computed(c) => {
                result.update([0]);
                result.update(c.binding().probe_map.as_bytes());
                result.update(c.source_theory_digest().as_bytes());
                result.update(c.target_theory_digest().as_bytes());
                direction(&mut result, c.soundness());
                direction(&mut result, c.completeness());
                result.update(c.assignments().to_le_bytes());
                result.update(c.ast_visits().to_le_bytes());
            }
            formal_boolean::Evaluation::NoComparable { reason, ast_visits } => {
                result.update([1, reason_code(*reason)]);
                result.update(ast_visits.to_le_bytes());
            }
            formal_boolean::Evaluation::Unknown { ast_visits } => {
                result.update([2]);
                result.update(ast_visits.to_le_bytes());
            }
        }
    }
    let (component, state_tag) = match state {
        RedundancyState::Measured(value) => (ComponentState::Measured(value.clone()), 0),
        RedundancyState::Unknown(_) => (
            ComponentState::Unknown(MeasurementGap::MissingObservation),
            1,
        ),
        RedundancyState::Unsupported(_) => {
            (ComponentState::Unknown(MeasurementGap::Unsupported), 2)
        }
        RedundancyState::NoComparable(_) => (
            ComponentState::Incomparable {
                scope: root.input.scope.comparison(),
            },
            3,
        ),
        RedundancyState::ResourceExhausted => {
            (ComponentState::ResourceExhausted { partial: None }, 4)
        }
    };
    result.update([state_tag]);
    let result = Digest32::from_bytes(result.finalize().into());
    let candidate = syntax::hash(
        b"epistesys.redundancy.actual-candidate.v1\0",
        &[
            &plan.candidate.candidate.0.to_le_bytes(),
            plan.candidate.digest.as_bytes(),
        ],
    );
    let context = plan
        .candidate
        .artifact
        .map_or(root.input.baseline.context, |a| a.context);
    Ok(ComponentReport {
        metric: ComponentMetric::Redundancy,
        definition: syntax::hash(b"finite-formal-v1.4/redundancy\0", &[PROFILE.as_bytes()]),
        binding: MeasurementBinding {
            scope: root.input.scope.comparison(),
            candidate,
            source: root.baseline_digest,
            target: plan.candidate.digest,
            context: context.interpretation_digest(),
            baseline: root.baseline_digest,
            producer,
            transform: plan.digest,
            result,
        },
        probe_set: root.input.scope.probes(),
        oracle: producer,
        dependency_group: syntax::hash(
            b"shared-typed-ir-given-model\0",
            &[root.model_digest.as_bytes()],
        ),
        evidence_payload: result,
        basis: InputBasis::Given,
        validation: SemanticValidation::Unassessed,
        independence: IndependenceStatus::SharedDependency,
        state: component,
    })
}
fn direction(hash: &mut Sha256, value: formal_boolean::DirectionResult) {
    match value {
        formal_boolean::DirectionResult::Preserved => hash.update([0]),
        formal_boolean::DirectionResult::Countermodel(row) => {
            hash.update([1]);
            hash.update(row.to_le_bytes());
        }
    }
}
fn reason_code(value: formal_boolean::NoComparable) -> u8 {
    match value {
        formal_boolean::NoComparable::MissingTheoryScope => 0,
        formal_boolean::NoComparable::SourceContextInconsistent => 1,
        formal_boolean::NoComparable::TargetContextInconsistent => 2,
    }
}
