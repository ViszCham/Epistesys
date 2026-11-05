use super::{
    decision::{construct, OwnedCanonicalDecision},
    types::*,
    BackendId, LogicProfile, SolverProtocol,
};
use lc631_solvers::{
    protocol::QueryResultKind, smt::native_model::execute_native_model,
    supervisor_adapter::OwnedPayloadStore,
};
use lc631_substrate::resource::{ResourceAmount, ResourceKind, ResourceRequest};

/// One canonical invocation: an initial actual native-model call, plus at most
/// one status probe and one rechecked model only with explicit objective opt-in.
/// No doctor, injected report, second broker or source-rebinding occurs.
/// The fixed 2MiB outer claim covers retained rendering/ledger/selector and their
/// transient construction; parser/native owners account their backing separately.
/// Allocator headers, OOM behavior, job-wide process memory and RSS are distinct.
#[allow(clippy::result_large_err)] // Post-execution failures must retain actual owners.
pub fn solve_with_services<'p, 'view>(
    request: CanonicalRequest<'p, 'view>,
    services: CanonicalServices<'view, '_>,
) -> Result<OwnedCanonicalDecision<'p, 'view>, CanonicalError<'p, 'view>> {
    let current = services.current;
    // The ordinary application turn explicitly stores no recomputable copies.
    // Optional SDK caches use the same turn owner; none persists across turns.
    let outcome = crate::turn_cache::with_turn_cache(
        crate::turn_cache::TurnCachePolicy::default(),
        current,
        |_| solve_in_turn(request, services),
    )
    .map_err(|error| {
        let admission = match error {
            crate::turn_cache::CacheError::Resource(resource) => AdmissionError::Resource(resource),
            other => AdmissionError::CachePolicy(other),
        };
        CanonicalError::Admission(admission)
    })?;
    outcome.value
}

#[allow(clippy::result_large_err)] // Retain actual post-execution owners on errors.
fn solve_in_turn<'p, 'view>(
    request: CanonicalRequest<'p, 'view>,
    services: CanonicalServices<'view, '_>,
) -> Result<OwnedCanonicalDecision<'p, 'view>, CanonicalError<'p, 'view>> {
    let started = std::time::Instant::now();
    admit(request, &services).map_err(CanonicalError::Admission)?;
    let amount = |n| {
        ResourceAmount::checked(n)
            .map_err(|e| CanonicalError::Admission(AdmissionError::Resource(e)))
    };
    let claim = services
        .current
        .claim_resources(&[
            ResourceRequest::new(ResourceKind::Ram, amount(DECISION_RAM_BYTES)?),
            ResourceRequest::new(
                ResourceKind::Output,
                amount(services.budget.max_output_bytes as u64)?,
            ),
        ])
        .map_err(|e| CanonicalError::Admission(AdmissionError::Resource(e)))?;
    // Reserve the large native enum slot BEFORE a child may execute. No Box/OOM
    // allocation is introduced while returning a post-execution error.
    let mut attempts = Vec::new();
    attempts
        .try_reserve_exact(1)
        .map_err(|_| CanonicalError::Admission(AdmissionError::AllocationFailed))?;
    let mut objective = Vec::new();
    if request.objective_step.is_some() {
        objective
            .try_reserve_exact(1)
            .map_err(|_| CanonicalError::Admission(AdmissionError::AllocationFailed))?;
        objective.push(
            super::optimization::admit(request, &services)
                .map_err(|error| CanonicalError::Admission(AdmissionError::Optimization(error)))?,
        );
    }
    let mut store = None;
    let attempt = match request.input {
        FormalInput::Unsupported(_) | FormalInput::Malformed(_) => NativeAttempt::NoExecutableInput,
        FormalInput::Problem(problem) => {
            if let Some(reason) = unavailable(&services) {
                NativeAttempt::Unavailable(reason)
            } else if let Some(registry) = services.registry {
                let mut actual_store = OwnedPayloadStore::with_resources(1, services.current)
                    .map_err(|e| CanonicalError::Admission(AdmissionError::Store(e)))?;
                let execution = if let Some(budget) = request.objective_step {
                    budget
                        .execution(services.execution, started)
                        .map_err(CanonicalError::Admission)?
                        .ok_or(CanonicalError::Admission(AdmissionError::Resource(
                            lc631_substrate::resource::ResourceError::DeadlineExceeded,
                        )))?
                } else {
                    services.execution
                };
                let result = execute_native_model(
                    registry,
                    problem.query(),
                    problem.context(),
                    &execution,
                    services.current,
                    services.working_directory,
                    &mut actual_store,
                    services.parser,
                );
                store = Some(actual_store);
                match result {
                    Ok(model) => NativeAttempt::Model(model),
                    Err(error) => NativeAttempt::Rejected(error),
                }
            } else {
                NativeAttempt::Unavailable(BackendUnavailable::MissingRegistry)
            }
        }
    };
    attempts.push(attempt);
    let mut evidence = NativeEvidence {
        attempts,
        store,
        request,
        measurement: None,
        objective,
    };
    let parts = if request.objective_step.is_some() {
        // Detach only the heap-vector header to obtain disjoint borrows. Always
        // restore the actual owner BEFORE propagating any late failure.
        let mut objective = std::mem::take(&mut evidence.objective);
        let prepared = match objective.first_mut() {
            Some(receipt) => super::optimization::build(&evidence, &services, started, receipt),
            None => Err(PostprocessError::SourceBinding),
        };
        evidence.objective = objective;
        prepared.and_then(|(parts, measured)| {
            evidence.measurement = measured;
            parts
        })
    } else if request.evaluation.is_some() {
        super::evaluation::build(
            &evidence,
            services.current,
            services.budget.max_output_bytes,
            None,
        )
        .and_then(|(parts, receipt)| {
            evidence.measurement = Some(receipt);
            parts
        })
    } else {
        construct::build(&evidence, services.budget.max_output_bytes, None, None)
    }
    .and_then(|parts| {
        claim.check().map_err(PostprocessError::Resource)?;
        Ok(parts)
    });
    match parts {
        Ok(parts) => Ok(OwnedCanonicalDecision::assemble(evidence, parts, claim)),
        Err(reason) => Err(CanonicalError::Postprocess(OwnedPostprocessFailure {
            reason,
            evidence,
            claim,
        })),
    }
}

fn unavailable(services: &CanonicalServices<'_, '_>) -> Option<BackendUnavailable> {
    if services.optional_gpu == GpuPolicy::Require {
        return Some(BackendUnavailable::GpuRequired);
    }
    let Some(registry) = services.registry else {
        return Some(BackendUnavailable::MissingRegistry);
    };
    match registry.resolve_for_query(
        BackendId::Z3,
        LogicProfile::SmtBooleanQfLia,
        QueryResultKind::ModelPayload,
    ) {
        Err(e) => Some(BackendUnavailable::Registry(e)),
        Ok(entry) if entry.protocol() != SolverProtocol::SmtLibPayloadWireV2 => {
            Some(BackendUnavailable::ProtocolMismatch)
        }
        Ok(_) => None,
    }
}

fn admit(
    request: CanonicalRequest<'_, '_>,
    services: &CanonicalServices<'_, '_>,
) -> Result<(), AdmissionError> {
    if request.objective_step.is_some()
        && (services.execution.max_stdout_bytes() > super::optimization::OBJECTIVE_STREAM_CAP
            || services.execution.max_stderr_bytes() > super::optimization::OBJECTIVE_STREAM_CAP)
    {
        return Err(AdmissionError::InvalidBudget);
    }
    if request.source().len() > crate::explicit_formal::MAX_SOURCE_BYTES {
        return Err(AdmissionError::SourceLimit);
    }
    if !services.working_directory.is_absolute() {
        return Err(AdmissionError::RelativeWorkingDirectory);
    }
    if services.current.source_revision() != request.source_revision().digest() {
        return Err(AdmissionError::SourceMismatch);
    }
    services
        .broker
        .validate_lease(
            services.current,
            services.current.run_id(),
            request.source_revision().digest(),
            ResourceKind::Ram,
        )
        .map_err(AdmissionError::Resource)?;
    match request.input {
        FormalInput::Problem(problem) => problem.check_current(services.current),
        FormalInput::Unsupported(report) => report.check_current(services.current),
        FormalInput::Malformed(_) => Ok(()),
    }
    .map_err(AdmissionError::Resource)?;
    Ok(())
}
