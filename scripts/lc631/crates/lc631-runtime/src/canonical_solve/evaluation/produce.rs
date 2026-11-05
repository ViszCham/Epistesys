//! Scoped actual finite evaluation; no supplied report/numeric success inputs.
use super::*;
use crate::{facet_ledger as l, facet_measurement_bridge as bridge, formal_boolean as f};
use lc631_analysis::creativity_measurement::{weighted_assessment, Rational, WeightedProbe};
use lc631_solvers::{
    runner::QuerySourceIdentity,
    typed_query::{BoolExpr, SmtEmitLimits},
};
use lc631_substrate::resource::{ResourceAmount, ResourceKind, ResourceRequest};

const WORK: m::WorkBudget = m::WorkBudget {
    max_work: 1_000_000,
    max_payload_bytes: 1_048_576,
};
const PER_PROBE_WORK: u64 = 32_768;

fn reserved<T>(n: usize) -> Result<Vec<T>, EvaluationError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(n)
        .map_err(|_| EvaluationError::AllocationFailed)?;
    Ok(values)
}
fn request(kind: ResourceKind, bytes: u64) -> Result<ResourceRequest, EvaluationError> {
    Ok(ResourceRequest::new(
        kind,
        ResourceAmount::checked(bytes).map_err(EvaluationError::Resource)?,
    ))
}
pub(super) fn build<'view>(
    evidence: &NativeEvidence<'_, 'view>,
    current: &'view ResourceReservationView<'_>,
    cap: usize,
    objective: Option<&crate::canonical_solve::optimization::OwnedObjectiveStep<'_>>,
) -> Result<
    (
        Result<DecisionParts, PostprocessError>,
        OwnedBooleanMeasurement<'view>,
    ),
    PostprocessError,
> {
    run(evidence, current, cap, objective).map_err(PostprocessError::Evaluation)
}
fn run<'view>(
    evidence: &NativeEvidence<'_, 'view>,
    current: &'view ResourceReservationView<'_>,
    cap: usize,
    objective: Option<&crate::canonical_solve::optimization::OwnedObjectiveStep<'_>>,
) -> Result<
    (
        Result<DecisionParts, PostprocessError>,
        OwnedBooleanMeasurement<'view>,
    ),
    EvaluationError,
> {
    let declared = evidence
        .request
        .evaluation
        .ok_or(EvaluationError::SourceBinding)?;
    declared
        .admit(evidence.request)
        .map_err(|_| EvaluationError::SourceBinding)?;
    let claim = current
        .claim_resources(&[
            request(ResourceKind::Ram, FINITE_BOOLEAN_RECEIPT_RAM)?,
            request(ResourceKind::Output, FINITE_BOOLEAN_RECEIPT_OUTPUT)?,
        ])
        .map_err(EvaluationError::Resource)?;
    let mut receipt = OwnedBooleanMeasurement {
        descriptor: declared,
        source_context: evidence.request.context.revision(),
        model_observation: None,
        target: None,
        target_bytes: Vec::new(),
        target_revision: None,
        probes: Vec::new(),
        soundness: None,
        completeness: None,
        semantic: None,
        ledger: None,
        state: BooleanEvaluationState::MissingNativeModel,
        claim,
    };
    let Some(native) = evidence.native_model() else {
        let parts =
            super::super::decision::construct::build(evidence, cap, Some(&receipt), objective);
        return Ok((parts, receipt));
    };
    receipt.model_observation = Some(model_binding(native.observation().stdout()));
    let FormalInput::Problem(problem) = evidence.request.input else {
        return Err(EvaluationError::SourceBinding);
    };
    let source = problem.query();
    let n = source.boolean_symbols().len();
    receipt.state = if !source.integer_symbols().is_empty() {
        BooleanEvaluationState::UnsupportedInteger
    } else if n == 0 {
        BooleanEvaluationState::EmptyAtomSet
    } else if n > FINITE_BOOLEAN_MAX_ATOMS {
        BooleanEvaluationState::AtomLimit
    } else {
        BooleanEvaluationState::Measured
    };
    if receipt.state == BooleanEvaluationState::Measured {
        if let Err(state) = admissible(source) {
            receipt.state = state;
        }
    }
    if receipt.state != BooleanEvaluationState::Measured {
        let parts =
            super::super::decision::construct::build(evidence, cap, Some(&receipt), objective);
        return Ok((parts, receipt));
    }
    let mut symbols = reserved(n)?;
    let mut assertions = reserved(n)?;
    let mut map = reserved(n)?;
    for &id in source.boolean_symbols() {
        symbols.push(id);
        map.push((id, id));
        let value = native
            .model()
            .boolean(id)
            .ok_or(EvaluationError::SourceBinding)?;
        // The standard recursive AST uses Box for Not. Its bounded allocation is
        // under the preclaimed profile; this does not claim fallible global OOM.
        assertions.push(if value {
            BoolExpr::Variable(id)
        } else {
            BoolExpr::Not(Box::new(BoolExpr::Variable(id)))
        });
    }
    let limits = SmtEmitLimits::checked(8192, 8, 0, 8, 32, 4).map_err(EvaluationError::Query)?;
    let target = SmtStatusQuery::checked(symbols, Vec::new(), assertions, limits)
        .map_err(EvaluationError::Query)?;
    let emitted = target
        .emit(QuerySourceIdentity::from_bytes(
            *problem.source().id().digest().as_bytes(),
        ))
        .map_err(EvaluationError::Query)?;
    let mut target_bytes = reserved(emitted.payload().len())?;
    target_bytes.extend_from_slice(emitted.payload());
    let target_text =
        std::str::from_utf8(&target_bytes).map_err(|_| EvaluationError::SourceBinding)?;
    let target_revision = SourceRevision::of(target_text);
    // All declared source atoms, both polarities, fixed BEFORE evaluating any
    // native-dependent result. An unused atom is never pruned per candidate.
    let mut predicates = reserved(n * 2)?;
    let mut identities = reserved(n * 2)?;
    for binding_symbol in problem.symbols() {
        let crate::explicit_formal::FormalSymbol::Boolean(id) = binding_symbol.symbol() else {
            return Err(EvaluationError::SourceBinding);
        };
        let name = problem
            .source()
            .slice(binding_symbol.span())
            .map_err(|_| EvaluationError::SourceBinding)?;
        for positive in [true, false] {
            predicates.push(if positive {
                BoolExpr::Variable(id)
            } else {
                BoolExpr::Not(Box::new(BoolExpr::Variable(id)))
            });
            identities.push(binding(
                b"canonical.model-space.probe.v1",
                &[name.as_bytes(), &[u8::from(positive)]],
            ));
        }
    }
    let mut requests = reserved(predicates.len())?;
    for predicate in &predicates {
        let mut probe = f::Request {
            source,
            target: &target,
            source_predicate: predicate,
            target_predicate: predicate,
            symbols: &map,
            unconstrained: f::UnconstrainedPolicy::ExplicitlyGiven,
            binding: f::Binding {
                source: problem.context().source(),
                target: problem.context().source(),
                source_context: problem.context(),
                target_context: problem.context(),
                probe_set: declared.scope.probes(),
                probe_map: binding(b"not-yet-measured", &[]),
            },
        };
        probe.binding.probe_map =
            f::probe_map_digest(&probe, 1_000_000).map_err(EvaluationError::Formal)?;
        requests.push(probe);
    }
    let one = Rational::from_integer(1.into());
    let mut fixed = reserved(requests.len())?;
    let mut schedule = reserved(requests.len())?;
    for (probe, id) in requests.iter().zip(&identities) {
        fixed.push(m::FixedProbe {
            identity: id.as_bytes(),
            weight: &one,
            template: probe,
        });
        schedule.push(m::produced::ScheduledProbe {
            identity: id.as_bytes(),
            request: probe,
            max_work: PER_PROBE_WORK,
        });
    }
    let definition = binding(
        b"canonical.model-space.definition.v1",
        &[
            b"TheoryToWitnessModelSpace-v1;event-set-preservation;not-SAT-answer-correctness",
            declared.scope.semantics().as_bytes(),
        ],
    );
    let plan = m::ProbePlan::checked(
        m::PlanScope {
            comparison: declared.scope.comparison(),
            definition,
            probe_set: declared.scope.probes(),
            source: problem.context().source(),
            context: problem.context(),
        },
        source,
        &fixed,
        WORK,
    )
    .map_err(EvaluationError::Bridge)?;
    let candidate_id = binding(
        b"canonical.model-space.candidate.v1",
        &[
            target_revision.digest().as_bytes(),
            receipt
                .model_observation
                .as_ref()
                .ok_or(EvaluationError::SourceBinding)?
                .as_bytes(),
        ],
    );
    // SourceRevision is original input attribution, not the target artifact ID.
    // The distinct target artifact/theory is retained separately above/below.
    let candidate = m::Candidate::given(
        candidate_id,
        declared.scope.comparison(),
        &target,
        problem.context().source(),
        problem.context(),
    )
    .map_err(EvaluationError::Bridge)?;
    let produced = m::produced::execute(&plan, &candidate, &schedule, WORK, current)
        .map_err(EvaluationError::Produced)?;
    let observations = produced
        .observations_current(current)
        .map_err(EvaluationError::Produced)?;
    let sound = m::owned::measure_owned(
        &plan,
        &candidate,
        m::Direction::Soundness,
        observations.as_slice(),
        WORK,
        current,
    )
    .map_err(EvaluationError::Owned)?;
    let complete = m::owned::measure_owned(
        &plan,
        &candidate,
        m::Direction::Completeness,
        observations.as_slice(),
        WORK,
        current,
    )
    .map_err(EvaluationError::Owned)?;
    let mut receipts = reserved(fixed.len())?;
    for (i, (s, c)) in sound
        .historical()
        .probes()
        .iter()
        .zip(complete.historical().probes())
        .enumerate()
    {
        let report = match s.observation().map(|v| &v.value) {
            Some(m::ObservationValue::Computed(r)) => Some(*r),
            _ => None,
        };
        receipts.push(BooleanProbeReceipt {
            binding: identities[i],
            positive: i % 2 == 0,
            source_symbol: map[i / 2].0,
            target_symbol: map[i / 2].1,
            weight_numerator: 1,
            weight_denominator: 1,
            soundness: s.state(),
            completeness: c.state(),
            assignments: report.map(f::ComputedReport::assignments),
            ast_visits: report.map(f::ComputedReport::ast_visits),
            source_theory: report.map(f::ComputedReport::source_theory_digest),
            target_theory: report.map(f::ComputedReport::target_theory_digest),
        });
    }
    let semantic = weighted_assessment(&[
        WeightedProbe {
            canonical_payload: b"soundness",
            weight: &one,
            value: sound.historical().bounds(),
        },
        WeightedProbe {
            canonical_payload: b"completeness",
            weight: &one,
            value: complete.historical().bounds(),
        },
    ])
    .map_err(|e| EvaluationError::Bridge(m::BridgeError::Arithmetic(e)))?;
    let mut obligations = reserved(identities.len())?;
    for id in &identities {
        obligations.push(bridge::ProbeObligation {
            identity: id.as_bytes(),
            root: l::ObligationId(*definition.as_bytes()),
            property: l::PropertyId(*id.as_bytes()),
        });
    }
    let declared_facets = bridge::DeclaredFacetScope {
        baseline: l::Revision(*problem.context().source().digest().as_bytes()),
        root_goal: l::Revision(*declared.scope.goal().as_bytes()),
        policy: l::Revision(*declared.scope.semantics().as_bytes()),
    };
    let mut measured_ledger =
        bridge::FacetMeasurementLedger::new(2, 48).map_err(EvaluationError::Facets)?;
    measured_ledger
        .admit(
            bridge::MeasuredFacetBatch::from_loss(
                sound.historical(),
                declared_facets,
                &obligations,
            )
            .map_err(EvaluationError::Facets)?,
        )
        .map_err(EvaluationError::Facets)?;
    measured_ledger
        .admit(
            bridge::MeasuredFacetBatch::from_loss(
                complete.historical(),
                declared_facets,
                &obligations,
            )
            .map_err(EvaluationError::Facets)?,
        )
        .map_err(EvaluationError::Facets)?;
    let mut ledger = l::FacetLedger::new(2, 48).map_err(|_| EvaluationError::SourceBinding)?;
    for transform in measured_ledger.ledger().transforms() {
        ledger
            .admit_transform(*transform)
            .map_err(|_| EvaluationError::SourceBinding)?;
    }
    for entry in measured_ledger.ledger().entries() {
        ledger
            .append(*entry)
            .map_err(|_| EvaluationError::SourceBinding)?;
    }
    receipt.probes = receipts;
    receipt.soundness = Some(sound.historical().bounds().clone());
    receipt.completeness = Some(complete.historical().bounds().clone());
    receipt.semantic = Some(semantic);
    receipt.ledger = Some(ledger);
    receipt.target_revision = Some(target_revision);
    // These owners stay live THROUGH actual selection/exact rendering. No
    // detached caller report/numeric value is accepted by this private path.
    observations
        .check_current()
        .map_err(EvaluationError::Produced)?;
    sound
        .project_current(&plan, &candidate, m::Direction::Soundness, current)
        .map_err(EvaluationError::Owned)?;
    complete
        .project_current(&plan, &candidate, m::Direction::Completeness, current)
        .map_err(EvaluationError::Owned)?;
    receipt.claim.check().map_err(EvaluationError::Resource)?;
    let parts = super::super::decision::construct::build(evidence, cap, Some(&receipt), objective);
    // Explicit use after render prevents the scoped owner lifetime from being
    // shortened conceptually into a report-only precomputed substitute.
    let parts = parts.and_then(|parts| {
        observations
            .check_current()
            .map_err(|e| PostprocessError::Evaluation(EvaluationError::Produced(e)))?;
        receipt.claim.check().map_err(PostprocessError::Resource)?;
        Ok(parts)
    });
    drop(measured_ledger);
    drop(complete);
    drop(sound);
    drop(observations);
    drop(produced);
    drop(fixed);
    drop(schedule);
    drop(requests);
    receipt.target = Some(target);
    receipt.target_bytes = target_bytes;
    Ok((parts, receipt))
}

fn admissible(query: &SmtStatusQuery) -> Result<(), BooleanEvaluationState> {
    fn walk(
        expr: &BoolExpr,
        nodes: &mut usize,
        depth: usize,
    ) -> Result<(), BooleanEvaluationState> {
        *nodes += 1;
        if *nodes > 64 || depth > 32 {
            return Err(BooleanEvaluationState::SourceComplexityLimit);
        }
        match expr {
            BoolExpr::Constant(_) | BoolExpr::Variable(_) => Ok(()),
            BoolExpr::Not(child) => walk(child, nodes, depth + 1),
            BoolExpr::And(children) | BoolExpr::Or(children) => {
                for child in children {
                    walk(child, nodes, depth + 1)?;
                }
                Ok(())
            }
            BoolExpr::Equal(..) | BoolExpr::LessEqual(..) | BoolExpr::LessThan(..) => {
                Err(BooleanEvaluationState::UnsupportedFormal)
            }
        }
    }
    let mut nodes = 0;
    for expr in query.assertions() {
        walk(expr, &mut nodes, 1)?;
    }
    Ok(())
}
