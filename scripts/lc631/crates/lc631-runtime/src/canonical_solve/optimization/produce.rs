//! One actual reported strict probe, optionally one actual rechecked model.
use super::*;
use crate::canonical_solve::{
    decision::{construct, DecisionParts},
    evaluation::OwnedBooleanMeasurement,
    CanonicalServices,
};
use lc631_solvers::{
    protocol::{QueryResultKind, SolverProtocol},
    registry::{BackendId, LogicProfile},
    smt::native_probe::{bounded::FiniteObjectiveBounds, execute_native_probe, NativeProbeStatus},
};
use lc631_substrate::{
    measurement_numeric::{BigInt, Rational},
    resource::{ResourceAmount, ResourceKind, ResourceRequest},
};

pub(in crate::canonical_solve) type StepBuild<'view> = (
    Result<DecisionParts, PostprocessError>,
    Option<OwnedBooleanMeasurement<'view>>,
);
pub(in crate::canonical_solve) fn build<'view>(
    evidence: &NativeEvidence<'_, 'view>,
    services: &CanonicalServices<'view, '_>,
    started: Instant,
    receipt: &mut OwnedObjectiveStep<'view>,
) -> Result<StepBuild<'view>, PostprocessError> {
    run(evidence, services, started, receipt).map_err(PostprocessError::Optimization)
}
fn resource(kind: ResourceKind, bytes: u64) -> Result<ResourceRequest, OptimizationError> {
    Ok(ResourceRequest::new(
        kind,
        ResourceAmount::checked(bytes).map_err(OptimizationError::Resource)?,
    ))
}
/// Isolate pre-effect construction from native call frames. The admitted owner
/// is moved into its reserved heap slot before any backend may execute.
#[inline(never)]
pub(in crate::canonical_solve) fn admit<'view>(
    request: CanonicalRequest<'_, '_>,
    services: &CanonicalServices<'view, '_>,
) -> Result<OwnedObjectiveStep<'view>, OptimizationError> {
    let FormalInput::Problem(problem) = request.input else {
        return Err(OptimizationError::SourceBinding);
    };
    let goal = problem.goal().ok_or(OptimizationError::SourceBinding)?;
    let claim = services
        .current
        .claim_resources(&[
            resource(ResourceKind::Ram, OBJECTIVE_RECEIPT_RAM)?,
            resource(ResourceKind::Output, OBJECTIVE_RECEIPT_OUTPUT)?,
        ])
        .map_err(OptimizationError::Resource)?;
    let initial = ModelSnapshot::reserved(problem.query())?;
    let improvement = ModelSnapshot::reserved(problem.query())?;
    let mut probe_query = Vec::new();
    probe_query
        .try_reserve_exact(OBJECTIVE_STREAM_CAP)
        .map_err(|_| OptimizationError::AllocationFailed)?;
    Ok(OwnedObjectiveStep {
        source: request.source_revision(),
        context: request.context.revision(),
        goal_revision: request.context.scope().goal(),
        objective: goal.symbol(),
        direction: goal.direction(),
        lower: goal.lower(),
        upper: goal.upper(),
        initial_value: None,
        selected_value: None,
        quality: None,
        gap: None,
        normalization: ObjectiveNormalization::MissingSelectedValue,
        state: ObjectiveStepState::MissingNativeModel,
        attempts: 0,
        initial,
        improvement,
        probe_query,
        probe_report: None,
        native_error: None,
        model_store: None,
        binding_at_render: None,
        failure: None,
        claim,
    })
}

fn run<'view>(
    evidence: &NativeEvidence<'_, 'view>,
    services: &CanonicalServices<'view, '_>,
    started: Instant,
    receipt: &mut OwnedObjectiveStep<'view>,
) -> Result<StepBuild<'view>, OptimizationError> {
    let request = evidence.request;
    let budget = request
        .objective_step
        .ok_or(OptimizationError::SourceBinding)?;
    let FormalInput::Problem(problem) = request.input else {
        return Err(OptimizationError::SourceBinding);
    };
    let goal = problem.goal().ok_or(OptimizationError::SourceBinding)?;
    let Some(native) = evidence.native_model() else {
        receipt.attempts = if matches!(evidence.kind(), NativeEvidenceKind::Rejected) {
            1
        } else {
            0
        };
        return Ok(finish(evidence, services, receipt));
    };
    receipt.attempts = 1;
    receipt
        .initial
        .prepare(problem.query(), request.source_revision())?;
    receipt.initial.capture(native)?;
    let value = native
        .model()
        .integer(goal.symbol())
        .and_then(|n| i64::try_from(n).ok())
        .ok_or(OptimizationError::SourceBinding)?;
    receipt.initial_value = Some(value);
    receipt.selected_value = Some(value);
    let bounds = match FiniteObjectiveBounds::from_query(problem.query(), goal.symbol()) {
        Ok(bounds) if bounds.lower() == goal.lower() && bounds.upper() == goal.upper() => bounds,
        Ok(_) => return Err(OptimizationError::SourceBinding),
        Err(error) => {
            receipt.state = ObjectiveStepState::ProbeUnresolved;
            receipt.failure = Some(OptimizationError::Bounds(error));
            return Ok(finish(evidence, services, receipt));
        }
    };
    assess(receipt, value, &bounds)?;
    let registry = services.registry.ok_or(OptimizationError::SourceBinding)?;
    let status_capability = registry
        .resolve_for_query(
            BackendId::Z3,
            LogicProfile::SmtBooleanQfLia,
            QueryResultKind::Satisfiability,
        )
        .is_ok_and(|entry| entry.protocol() == SolverProtocol::SmtLibStatusV1);
    if !status_capability {
        receipt.state = ObjectiveStepState::UnavailableStatusCapability;
        return Ok(finish(evidence, services, receipt));
    }
    let Some(execution) = budget
        .execution(services.execution, started)
        .map_err(|_| OptimizationError::InvalidBudget)?
    else {
        receipt.state = ObjectiveStepState::BudgetLimited;
        return Ok(finish(evidence, services, receipt));
    };
    receipt.attempts = 2;
    let outcome = match execute_native_probe(
        registry,
        native,
        goal.symbol(),
        goal.direction(),
        &execution,
        services.current,
        services.working_directory,
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            receipt.state = ObjectiveStepState::ProbeUnresolved;
            receipt.failure = Some(OptimizationError::Probe(error));
            return Ok(finish(evidence, services, receipt));
        }
    };
    if outcome.prepared().payload().len() > receipt.probe_query.capacity() {
        return Err(OptimizationError::AllocationFailed);
    }
    receipt
        .probe_query
        .extend_from_slice(outcome.prepared().payload());
    receipt.probe_report = Some(outcome.report());
    match outcome.status() {
        NativeProbeStatus::SolverReportedNoStrictImprovement => {
            receipt.state = ObjectiveStepState::ReportedNoStrictImprovement;
            let result = finish(evidence, services, receipt);
            // Actual query/report/store/claims remain alive through rendering.
            let _actual_report = outcome.report();
            drop(outcome);
            Ok(result)
        }
        NativeProbeStatus::Unresolved { .. } => {
            receipt.state = ObjectiveStepState::ProbeUnresolved;
            let result = finish(evidence, services, receipt);
            let _actual_report = outcome.report();
            drop(outcome);
            Ok(result)
        }
        NativeProbeStatus::SolverReportedImprovementExists => {
            let Some(execution) = budget
                .execution(services.execution, started)
                .map_err(|_| OptimizationError::InvalidBudget)?
            else {
                receipt.state = ObjectiveStepState::BudgetLimited;
                return Ok(finish(evidence, services, receipt));
            };
            // Model-query emission and all destination backing precede child3.
            receipt
                .improvement
                .prepare(outcome.probe().better_query(), request.source_revision())?;
            // This outcome was privately produced above from this exact current
            // view and incumbent; no caller-supplied outcome/report is accepted.
            services
                .current
                .check()
                .map_err(OptimizationError::Resource)?;
            receipt.claim.check().map_err(OptimizationError::Resource)?;
            if outcome.context() != receipt.context
                || outcome.probe().objective() != receipt.objective
                || outcome.probe().direction() != receipt.direction
                || Some(outcome.probe().incumbent()) != receipt.initial_value
            {
                return Err(OptimizationError::SourceBinding);
            }
            let mut store = lc631_solvers::supervisor_adapter::OwnedPayloadStore::with_resources(
                1,
                services.current,
            )
            .map_err(OptimizationError::Store)?;
            receipt.attempts = 3;
            let attempt = lc631_solvers::smt::native_model::execute_native_model(
                registry,
                outcome.probe().better_query(),
                outcome.context(),
                &execution,
                services.current,
                services.working_directory,
                &mut store,
                services.parser,
            );
            // NativeCheckedModel owns its observation and borrows only query/
            // view, not this store. Move the ACTUAL metadata owner before any
            // binding/rendering so store count cannot change after capture.
            receipt.model_store = Some(store);
            match attempt {
                Ok(model) => {
                    // Reuse the concrete native validator on the EXACT sealed
                    // original-plus-strict query. Avoid the large generic
                    // BoundedCandidateError owner/result frame on caller stacks.
                    match receipt.improvement.capture(&model).and_then(|()| {
                        let value = model
                            .model()
                            .integer(receipt.objective)
                            .and_then(|n| i64::try_from(n).ok())
                            .ok_or(OptimizationError::SourceBinding)?;
                        let improves = match receipt.direction {
                            ObjectiveDirection::Minimize => value < outcome.probe().incumbent(),
                            ObjectiveDirection::Maximize => value > outcome.probe().incumbent(),
                        };
                        if !improves
                            || model.context() != outcome.context()
                            || !std::ptr::eq(model.model().query(), outcome.probe().better_query())
                        {
                            return Err(OptimizationError::SourceBinding);
                        }
                        assess(receipt, value, &bounds)
                    }) {
                        Ok(()) => receipt.state = ObjectiveStepState::ImprovedModelRechecked,
                        Err(error) => {
                            receipt.state = ObjectiveStepState::ImprovedModelRejected;
                            receipt.failure = Some(error);
                        }
                    }
                    let result = finish(evidence, services, receipt);
                    let _actual_model = model.model().assertions_checked();
                    drop(model);
                    Ok(result)
                }
                Err(error) => {
                    receipt.native_error = Some(error);
                    receipt.state = ObjectiveStepState::ImprovedModelRejected;
                    Ok(finish(evidence, services, receipt))
                }
            }
        }
    }
}

fn assess(
    receipt: &mut OwnedObjectiveStep<'_>,
    value: i64,
    bounds: &FiniteObjectiveBounds<'_>,
) -> Result<(), OptimizationError> {
    receipt.gap = Some(
        bounds
            .gap_upper_bound(value, receipt.direction)
            .map_err(OptimizationError::Bounds)?,
    );
    receipt.selected_value = Some(value);
    let width = i128::from(receipt.upper) - i128::from(receipt.lower);
    if width == 0 {
        receipt.normalization = ObjectiveNormalization::UndefinedZeroWidth;
        receipt.quality = None;
        return Ok(());
    }
    if width < 0 || value < receipt.lower || value > receipt.upper {
        return Err(OptimizationError::SourceBinding);
    }
    let numerator = match receipt.direction {
        ObjectiveDirection::Minimize => i128::from(receipt.upper) - i128::from(value),
        ObjectiveDirection::Maximize => i128::from(value) - i128::from(receipt.lower),
    };
    let q = Rational::new(BigInt::from(numerator), BigInt::from(width));
    receipt.quality =
        Some(RationalInterval::new(q.clone(), q).map_err(OptimizationError::Numeric)?);
    receipt.normalization = ObjectiveNormalization::PositiveWidth;
    Ok(())
}

fn finish<'view>(
    evidence: &NativeEvidence<'_, 'view>,
    services: &CanonicalServices<'view, '_>,
    receipt: &mut OwnedObjectiveStep<'view>,
) -> StepBuild<'view> {
    // The SAME captured identity is consumed by objective_scope::bind below.
    // The independently recomputed public getter detects later owner changes.
    receipt.binding_at_render = receipt.third_observation_binding();
    let prepared = if evidence.request.evaluation.is_some() {
        crate::canonical_solve::evaluation::build(
            evidence,
            services.current,
            services.budget.max_output_bytes,
            Some(&*receipt),
        )
        .map(|(parts, measured)| (parts, Some(measured)))
    } else {
        Ok((
            construct::build(
                evidence,
                services.budget.max_output_bytes,
                None,
                Some(&*receipt),
            ),
            None,
        ))
    };
    match prepared {
        Ok((parts, measured)) => {
            let parts = parts.and_then(|parts| {
                services
                    .current
                    .check()
                    .map_err(PostprocessError::Resource)?;
                receipt.claim.check().map_err(PostprocessError::Resource)?;
                Ok(parts)
            });
            (parts, measured)
        }
        Err(error) => (Err(error), None),
    }
}
