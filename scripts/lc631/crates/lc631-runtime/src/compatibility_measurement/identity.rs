//! Streaming typed binary identities; reported lineage is never an owner token.
use super::*;
use lc631_analysis::creativity_measurement::{
    weighted_assessment, ComponentMetric, ComponentState, IndependenceStatus, InputBasis,
    MeasurementBinding, Rational, RationalInterval, SemanticValidation, WeightedProbe,
};
use lc631_solvers::typed_query::BoolExpr;
use lc631_substrate::measurement_numeric::BigInt;
use sha2::{Digest, Sha256};

pub(super) fn revision(bytes: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(bytes).into())
}
pub(super) fn wire(bytes: &[u8]) -> Digest32 {
    let mut h = start(b"acomp.actual-generated-model-frame.v1\0");
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
    finish(h)
}
pub(super) fn frame(frame: &Frame) -> Digest32 {
    let mut h = start(b"acomp.typed-boolean-frame.v1\0");
    for a in frame {
        h.update(a.symbol.get().to_le_bytes());
        h.update([u8::from(a.value)]);
    }
    finish(h)
}
pub(super) fn maps(role: &[u8], values: &[PortMap]) -> Digest32 {
    let mut h = start(role);
    for pair in values {
        h.update(pair.source.get().to_le_bytes());
        h.update(pair.target.get().to_le_bytes());
    }
    finish(h)
}
fn query(query: &SmtStatusQuery) -> Result<Digest32, Error> {
    let mut h = start(b"acomp.atomic-boolean-query.v1\0");
    h.update((query.boolean_symbols().len() as u32).to_le_bytes());
    for symbol in query.boolean_symbols() {
        h.update(symbol.get().to_le_bytes());
    }
    match query.assertions() {
        [BoolExpr::Variable(symbol)] => h.update(symbol.get().to_le_bytes()),
        _ => return Err(Error::Unsupported(Unsupported::UseFormula)),
    }
    Ok(finish(h))
}
fn queries(input: PlanInput<'_>) -> Result<[Digest32; 5], Error> {
    Ok([
        query(input.source_theory)?,
        query(input.target_theory)?,
        query(input.root_goal)?,
        query(input.source_use)?,
        query(input.target_use)?,
    ])
}
pub(super) fn plan(
    input: PlanInput<'_>,
    limits: Limits,
    order: [usize; 4],
) -> Result<Digest32, Error> {
    let mut h = start(b"acomp.closed-boolean-plan.v1\0");
    for d in [
        input.source.id().digest(),
        input.target.id().digest(),
        input.source_context.interpretation_digest(),
        input.target_context.interpretation_digest(),
        input.scope.comparison(),
        input.scope.goal(),
        input.scope.probes(),
        input.scope.semantics(),
        input.pins.definition,
        input.pins.oracle,
        input.pins.policy,
        input.pins.baseline,
    ] {
        h.update(d.as_bytes());
    }
    for q in queries(input)? {
        h.update(q.as_bytes());
    }
    let (expected, _) = program::mapping(
        input.source_theory.boolean_symbols(),
        input.target_theory.boolean_symbols(),
        input.expected_mapping,
    )?;
    h.update(maps(b"acomp.expected-semantic-map.v1\0", &expected).as_bytes());
    for index in order {
        for symbol in input.source_theory.boolean_symbols() {
            let a = input.cases[index]
                .input
                .iter()
                .find(|a| a.symbol == *symbol)
                .ok_or(Error::InvalidInput)?;
            h.update(symbol.get().to_le_bytes());
            h.update([u8::from(a.value)]);
        }
        h.update(input.cases[index].weight.to_le_bytes());
    }
    h.update(limits.max_work.to_le_bytes());
    h.update((limits.max_frame_bytes as u64).to_le_bytes());
    h.update(limits.max_program_steps.to_le_bytes());
    Ok(finish(h))
}
pub(super) fn build(
    plan: &Plan<'_, '_>,
    program: &Program<'_, '_, '_>,
    values: &[Assessment; 4],
) -> Result<(ComponentReport, Lineage), Error> {
    let lineage = lineage(plan, program, values)?;
    let mut h = start(b"acomp.actual-stages-and-query-results.v1\0");
    h.update(plan.digest.as_bytes());
    h.update(lineage.forward_program.as_bytes());
    h.update(lineage.inverse_program.as_bytes());
    for &case in &plan.order {
        hash_assessment(&mut h, values[case]);
    }
    let result = finish(h);
    let input = plan.input;
    let producer = revision(b"finite-formal-v1.4/CR-ACOMP/actual-closed-boolean-service-v1");
    let report = ComponentReport {
        metric: ComponentMetric::Compatibility,
        definition: input.pins.definition,
        binding: MeasurementBinding {
            scope: input.scope.comparison(),
            candidate: program_digest(&lineage),
            source: input.source.id().digest(),
            target: input.target.id().digest(),
            context: input.target_context.interpretation_digest(),
            baseline: input.pins.baseline,
            producer,
            transform: plan.digest,
            result,
        },
        probe_set: input.scope.probes(),
        oracle: input.pins.oracle,
        dependency_group: revision(b"Given-boolean-source-mapping-shared-model-checker-v1"),
        evidence_payload: result,
        basis: InputBasis::Given,
        validation: SemanticValidation::Unassessed,
        independence: IndependenceStatus::SharedDependency,
        state: ComponentState::Measured(aggregate(plan, values)?),
    };
    Ok((report, lineage))
}
fn lineage(
    plan: &Plan<'_, '_>,
    program: &Program<'_, '_, '_>,
    values: &[Assessment; 4],
) -> Result<Lineage, Error> {
    let input = plan.input;
    let (expected, _) = program::mapping(
        input.source_theory.boolean_symbols(),
        input.target_theory.boolean_symbols(),
        input.expected_mapping,
    )?;
    let mut declared = [plan.digest; 4];
    for (index, digest) in declared.iter_mut().enumerate() {
        *digest = frame(&plan.case_frame(index)?);
    }
    Ok(Lineage {
        source_context: input.source_context,
        target_context: input.target_context,
        root_goal_revision: input.scope.goal(),
        oracle: input.pins.oracle,
        baseline: input.pins.baseline,
        forward_program: maps(b"acomp.forward.v1\0", &program.forward),
        inverse_program: maps(b"acomp.inverse.v1\0", &program.inverse),
        source_store: input.source.id().digest(),
        target_store: input.target.id().digest(),
        queries: queries(input)?,
        declared_inputs: declared,
        output_frames: values.map(|v| v.output),
        inverse_frames: values.map(|v| v.inverse),
        source_model_frames: values.map(|v| v.source_wire),
        target_model_frames: values.map(|v| v.target_wire),
        expected_mapping: maps(b"acomp.expected-semantic-map.v1\0", &expected),
        plan: plan.digest,
        definition: input.pins.definition,
        policy: input.pins.policy,
        budget_series: plan.digest,
    })
}
fn aggregate(plan: &Plan<'_, '_>, values: &[Assessment; 4]) -> Result<RationalInterval, Error> {
    let one = Rational::from_integer(BigInt::from(1));
    let mut bounds = Vec::new();
    bounds
        .try_reserve_exact(4)
        .map_err(|_| Error::AllocationFailed)?;
    let mut keys = [plan.digest; 4];
    for (i, value) in values.iter().enumerate() {
        let interval = match value.success() {
            Check::Pass => RationalInterval::exact_ratio(1, 1),
            Check::Fail => RationalInterval::exact_ratio(0, 1),
            Check::Unknown => RationalInterval::from_ratio(0, 1, 1, 1),
        }
        .map_err(|_| Error::InvalidInput)?;
        bounds.push(interval);
        keys[i] = frame(&plan.case_frame(i)?);
    }
    let probes: [WeightedProbe<'_>; 4] = std::array::from_fn(|i| WeightedProbe {
        canonical_payload: keys[i].as_bytes(),
        weight: &one,
        value: &bounds[i],
    });
    weighted_assessment(&probes).map_err(|_| Error::InvalidInput)
}
fn program_digest(lineage: &Lineage) -> Digest32 {
    let mut h = start(b"acomp.executable-program-pair.v1\0");
    h.update(lineage.plan.as_bytes());
    h.update(lineage.forward_program.as_bytes());
    h.update(lineage.inverse_program.as_bytes());
    finish(h)
}
fn hash_assessment(h: &mut Sha256, v: Assessment) {
    for c in [v.typed, v.roundtrip, v.used] {
        h.update([check(c)]);
    }
    h.update([match v.hard {
        HardState::Pass => 1,
        HardState::Fail => 2,
        HardState::NeedsEvidence => 0,
    }]);
    for b in [v.source, v.target] {
        h.update([match b {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        }]);
    }
    h.update([match v.gap {
        None => 0,
        Some(Gap::NotDispatched) => 1,
        Some(Gap::WorkBudget) => 2,
    }]);
    for digest in [v.output, v.inverse, v.source_wire, v.target_wire] {
        match digest {
            Some(d) => {
                h.update([1]);
                h.update(d.as_bytes());
            }
            None => h.update([0]),
        }
    }
}
fn check(c: Check) -> u8 {
    match c {
        Check::Unknown => 0,
        Check::Pass => 1,
        Check::Fail => 2,
    }
}
fn start(domain: &[u8]) -> Sha256 {
    let mut h = Sha256::new();
    h.update(domain);
    h
}
fn finish(h: Sha256) -> Digest32 {
    Digest32::from_bytes(h.finalize().into())
}
