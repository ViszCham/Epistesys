use super::report_identity::report_digest;
use super::{text, DecisionParts};
use crate::{canonical_solve::types::*, facet_ledger as l};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};
#[path = "candidates.rs"]
mod candidates;
#[path = "incumbent.rs"]
mod incumbent;

fn digest(domain: &[u8], parts: &[&[u8]]) -> Digest32 {
    let mut hash = Sha256::new();
    hash.update(domain);
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    Digest32::from_bytes(hash.finalize().into())
}

pub(in crate::canonical_solve) fn build(
    evidence: &NativeEvidence<'_, '_>,
    cap: usize,
    measured: Option<&crate::canonical_solve::evaluation::OwnedBooleanMeasurement<'_>>,
    objective: Option<&crate::canonical_solve::optimization::OwnedObjectiveStep<'_>>,
) -> Result<DecisionParts, PostprocessError> {
    let (mut ledger, result, mut measurement) = facets(evidence, measured, objective)?;
    if let Some(actual) = measured.and_then(|r| r.facets()) {
        for binding in actual.transforms() {
            measurement = digest(
                b"canonical.actual-semantic-attachment.v1",
                &[measurement.as_bytes(), &binding.query.0, &binding.result.0],
            );
            ledger
                .admit_transform(*binding)
                .map_err(PostprocessError::Ledger)?;
        }
        for entry in actual.entries() {
            ledger.append(*entry).map_err(PostprocessError::Ledger)?;
        }
    }
    if matches!(evidence.attempts.first(), Some(NativeAttempt::Model(_))) {
        return model_decision(
            evidence,
            ledger,
            result,
            measurement,
            cap,
            measured,
            objective,
        );
    }
    let (status, pending) = match evidence.attempts.first() {
        Some(NativeAttempt::NoExecutableInput) => (
            CanonicalStatus::Clarify,
            NoRecommendationReason::UnsupportedInput,
        ),
        Some(NativeAttempt::Unavailable(BackendUnavailable::GpuRequired)) => (
            CanonicalStatus::Unavailable,
            NoRecommendationReason::GpuUnavailable,
        ),
        Some(NativeAttempt::Unavailable(_)) => (
            CanonicalStatus::Unavailable,
            NoRecommendationReason::MissingBackend,
        ),
        Some(NativeAttempt::Rejected(_)) | None => (
            CanonicalStatus::NativeFailure,
            NoRecommendationReason::NativeRejected,
        ),
        Some(NativeAttempt::Model(_)) => return Err(PostprocessError::SourceBinding),
    };
    let rendered = text::diagnostic(evidence, status, pending, cap)?;
    Ok(DecisionParts {
        ledger,
        rendered,
        selection: None,
        goal: None,
        status,
        pending,
    })
}

fn model_decision(
    evidence: &NativeEvidence<'_, '_>,
    ledger: l::FacetLedger,
    result: Digest32,
    measurement: Digest32,
    cap: usize,
    measured: Option<&crate::canonical_solve::evaluation::OwnedBooleanMeasurement<'_>>,
    objective: Option<&crate::canonical_solve::optimization::OwnedObjectiveStep<'_>>,
) -> Result<DecisionParts, PostprocessError> {
    candidates::build(
        evidence,
        ledger,
        result,
        measurement,
        cap,
        measured,
        objective,
    )
}

fn facets(
    evidence: &NativeEvidence<'_, '_>,
    measured: Option<&crate::canonical_solve::evaluation::OwnedBooleanMeasurement<'_>>,
    objective: Option<&crate::canonical_solve::optimization::OwnedObjectiveStep<'_>>,
) -> Result<(l::FacetLedger, Digest32, Digest32), PostprocessError> {
    let request = evidence.request;
    let source = request.source_revision().digest();
    let context = request.context.revision().interpretation_digest();
    // A report-only execution failure still has actual query/invocation/status
    // evidence. Absence of stdout ownership remains explicit in the result tag.
    let report = objective
        .and_then(|step| step.selected_report())
        .or_else(|| match evidence.attempts.first() {
            Some(NativeAttempt::Model(m)) => Some(m.observation().report()),
            Some(NativeAttempt::Rejected(
                lc631_solvers::smt::native_model::NativeModelError::Rejected {
                    observation, ..
                },
            )) => Some(observation.report()),
            Some(NativeAttempt::Rejected(
                lc631_solvers::smt::native_model::NativeModelError::Execution(
                    lc631_solvers::supervisor_adapter::OwnedPayloadError::Report(report)
                    | lc631_solvers::supervisor_adapter::OwnedPayloadError::StoreAfterExecution {
                        report,
                        ..
                    },
                ),
            )) => Some(*report),
            _ => None,
        });
    let report_binding = report.map(report_digest);
    let query = report.map_or_else(
        || digest(b"canonical.query-unobserved.v1", &[]),
        |r| Digest32::from_bytes(*r.query().as_bytes()),
    );
    let result = if let Some(step) = objective.filter(|step| step.selected_report().is_some()) {
        digest(
            b"canonical.actual-selected-objective-model.v1",
            &[
                step.selected_stdout(),
                report_binding
                    .as_ref()
                    .ok_or(PostprocessError::SourceBinding)?
                    .as_bytes(),
            ],
        )
    } else {
        match evidence.attempts.first() {
            Some(NativeAttempt::Model(m)) => digest(
                b"canonical.actual-native.stdout.v2",
                &[
                    m.observation().stdout(),
                    report_binding
                        .as_ref()
                        .ok_or(PostprocessError::SourceBinding)?
                        .as_bytes(),
                ],
            ),
            Some(NativeAttempt::Rejected(
                lc631_solvers::smt::native_model::NativeModelError::Rejected {
                    observation, ..
                },
            )) => digest(
                b"canonical.actual-rejected.stdout.v2",
                &[
                    observation.stdout(),
                    report_binding
                        .as_ref()
                        .ok_or(PostprocessError::SourceBinding)?
                        .as_bytes(),
                ],
            ),
            _ => match report_binding {
                Some(binding) => digest(
                    b"canonical.actual-report-no-owned-stdout.v1",
                    &[binding.as_bytes()],
                ),
                None => digest(b"canonical.no-model-observation.v1", &[source.as_bytes()]),
            },
        }
    };
    let producer = digest(b"canonical.native-single-query.v1", &[]);
    let mut measurement = digest(
        b"canonical.gaps-binding.v2",
        &[
            source.as_bytes(),  // actual source and baseline
            context.as_bytes(), // canonical interpretation, not a caller label
            request.context.scope().comparison().as_bytes(),
            request.context.scope().goal().as_bytes(),
            request.context.scope().probes().as_bytes(),
            request.context.scope().semantics().as_bytes(),
            producer.as_bytes(),
            query.as_bytes(),
            result.as_bytes(),
            match request.goal {
                GoalPolicy::NoGoal => b"no-goal",
                GoalPolicy::SatisfyingModel { .. } => b"satisfying-model",
                GoalPolicy::DeclaredSourceObjective { .. } => b"source-objective",
            },
        ],
    );
    if let Some(step) = objective {
        measurement = super::objective_scope::bind(
            measurement,
            step,
            request
                .objective_step
                .ok_or(PostprocessError::SourceBinding)?,
        );
    }
    let mut ledger = l::FacetLedger::new(3, 64).map_err(PostprocessError::Ledger)?;
    let id = l::TransformId(*measurement.as_bytes());
    let revision = |d: Digest32| l::Revision(*d.as_bytes());
    ledger
        .admit_transform(l::TransformBinding {
            id,
            source: revision(source),
            target: revision(result),
            source_context: revision(context),
            target_context: revision(context),
            direction: l::Direction::Forward,
            scope: l::ComparisonScope {
                id: l::ScopeId(*request.context.scope().comparison().as_bytes()),
                baseline: revision(source),
                root_goal: revision(request.context.scope().goal()),
                probes: revision(request.context.scope().probes()),
                policy: revision(request.context.scope().semantics()),
            },
            producer: revision(producer),
            query: revision(query),
            result: revision(result),
        })
        .map_err(PostprocessError::Ledger)?;
    for (property, reason) in [
        (
            b"source-interpretation".as_slice(),
            l::GapReason::Unverified,
        ),
        (
            b"shared-checker-independence".as_slice(),
            l::GapReason::Unverified,
        ),
        (
            b"semantic-no-fixed-probe-plan".as_slice(),
            l::GapReason::Unsupported,
        ),
        (
            b"report-detailed-metadata-unbound".as_slice(),
            l::GapReason::Unverified,
        ),
        (
            b"reconstruction-no-evaluator".as_slice(),
            l::GapReason::Unobserved,
        ),
    ] {
        if property == b"semantic-no-fixed-probe-plan"
            && measured.is_some_and(|receipt| {
                receipt.state()
                    == crate::canonical_solve::evaluation::BooleanEvaluationState::Measured
            })
        {
            continue; // Only this gap is resolved by an actual fixed-plan producer.
        }
        ledger
            .append(l::Entry {
                transform: id,
                facet: l::Facet::Uncertainty(l::Gap {
                    property: l::PropertyId(
                        *digest(b"canonical.gap-property.v1", &[property]).as_bytes(),
                    ),
                    reason,
                    required_evidence: revision(digest(
                        b"canonical.required-evaluator.v1",
                        &[property],
                    )),
                    next_action: revision(query),
                }),
            })
            .map_err(PostprocessError::Ledger)?;
    }
    if let Some(step) = objective {
        super::objective_scope::append_gaps(&mut ledger, id, query, step)?;
    }
    // D/Delta/G remain absent: no counterexample, context edit or paired gain
    // producer was run. Counts are never interpreted as measured zero loss.
    Ok((ledger, result, measurement))
}
