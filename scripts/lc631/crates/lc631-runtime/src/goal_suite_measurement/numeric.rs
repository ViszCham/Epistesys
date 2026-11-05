//! Only actual-owner callers create these small arithmetic projections.
use super::*;
use lc631_analysis::creativity_measurement::{
    paired_gain, ComponentMetric, ComponentReport, ComponentState, IndependenceStatus, InputBasis,
    MeasurementBinding, ProbePair, Rational, RationalInterval, SemanticValidation,
};
use lc631_substrate::measurement_numeric::BigInt;

pub(super) fn score(success: &[bool; 4]) -> Result<RationalInterval, Error> {
    let count = success.iter().filter(|s| **s).count();
    Ok(RationalInterval::exact_ratio(count as i64, 4)?)
}
pub(super) struct ReportInput {
    pub metric: ComponentMetric,
    pub scope: ContextScope,
    pub context: ContextRevision,
    pub source: Digest32,
    pub target: Digest32,
    pub baseline: Digest32,
    pub candidate: Digest32,
    pub baseline_observation: Digest32,
    pub candidate_observation: Digest32,
    pub dependency: Digest32,
    pub oracle: Digest32,
}
pub(super) fn gain(
    input: ReportInput,
    a: &[bool; 4],
    b: &[bool; 4],
) -> Result<ComponentReport, Error> {
    let values = |flags: &[bool; 4]| -> Result<[RationalInterval; 4], Error> {
        Ok([
            RationalInterval::exact_ratio(i64::from(flags[0]), 1)?,
            RationalInterval::exact_ratio(i64::from(flags[1]), 1)?,
            RationalInterval::exact_ratio(i64::from(flags[2]), 1)?,
            RationalInterval::exact_ratio(i64::from(flags[3]), 1)?,
        ])
    };
    let av = values(a)?;
    let bv = values(b)?;
    let keys: [[u8; 8]; 4] = [
        [0, 0, 0, 0, 0, 0, 0, 0],
        [1, 0, 0, 0, 2, 0, 0, 0],
        [255, 255, 255, 255, 1, 0, 0, 0],
        [255, 255, 255, 255, 0, 0, 0, 0],
    ];
    let definition = hash(&[
        b"finite-formal-v1.4/GivenArtifactCheckedU32AddFour-v1",
        &(input.metric as u16).to_le_bytes(),
    ]);
    let nodes = |owner: Digest32| {
        std::array::from_fn::<_, 4, _>(|i| {
            hash(&[
                b"goal-suite-value-node.v1",
                definition.as_bytes(),
                input.scope.comparison().as_bytes(),
                input.scope.probes().as_bytes(),
                owner.as_bytes(),
                &keys[i],
            ])
        })
    };
    let an = nodes(input.baseline_observation);
    let bn = nodes(input.candidate_observation);
    let one = Rational::from_integer(BigInt::from(1));
    let pairs: [ProbePair<'_>; 4] = std::array::from_fn(|i| ProbePair {
        canonical_payload: &keys[i],
        weight: &one,
        baseline: &av[i],
        candidate: &bv[i],
        baseline_node: an[i],
        candidate_node: bn[i],
    });
    let value = paired_gain(&pairs)?;
    Ok(report(input, definition, value))
}
pub(super) fn objective(input: ReportInput, a: i64, b: i64) -> Result<ComponentReport, Error> {
    let av = RationalInterval::exact_ratio(a, 3)?;
    let bv = RationalInterval::exact_ratio(b, 3)?;
    let one = Rational::from_integer(BigInt::from(1));
    let definition = hash(&[b"finite-formal-v1.4/CR-SOLVE/FiniteNativeObjectiveZeroToThree-v1"]);
    let node = |owner: Digest32| {
        hash(&[
            definition.as_bytes(),
            input.scope.comparison().as_bytes(),
            input.scope.probes().as_bytes(),
            owner.as_bytes(),
        ])
    };
    let value = paired_gain(&[ProbePair {
        canonical_payload: b"x/3.root-hard-first.v1",
        weight: &one,
        baseline: &av,
        candidate: &bv,
        baseline_node: node(input.baseline_observation),
        candidate_node: node(input.candidate_observation),
    }])?;
    Ok(report(input, definition, value))
}
fn report(input: ReportInput, definition: Digest32, value: RationalInterval) -> ComponentReport {
    let producer = hash(&[b"actual-owner-goal-suite/v1", definition.as_bytes()]);
    let transform = hash(&[
        producer.as_bytes(),
        input.baseline.as_bytes(),
        input.candidate.as_bytes(),
        input.scope.comparison().as_bytes(),
        input.context.interpretation_digest().as_bytes(),
        input.baseline_observation.as_bytes(),
        input.candidate_observation.as_bytes(),
    ]);
    let result = hash(&[
        transform.as_bytes(),
        input.dependency.as_bytes(),
        input.scope.probes().as_bytes(),
    ]);
    ComponentReport {
        metric: input.metric,
        definition,
        binding: MeasurementBinding {
            scope: input.scope.comparison(),
            candidate: input.candidate,
            source: input.source,
            target: input.target,
            context: input.context.interpretation_digest(),
            baseline: input.baseline,
            producer,
            transform,
            result,
        },
        probe_set: input.scope.probes(),
        oracle: input.oracle,
        dependency_group: input.dependency,
        evidence_payload: result,
        basis: InputBasis::Given,
        validation: SemanticValidation::Unassessed,
        independence: IndependenceStatus::SharedDependency,
        state: ComponentState::Measured(value),
    }
}
