//! Re-admit exact Given input; never decode a native/current observation.
use super::super::{
    solve_with_services, CanonicalError, CanonicalRequest, CanonicalServices, DeclaredEvaluation,
    OneStepBudget, OwnedCanonicalDecision, SourceStore,
};
use super::{store::OwnedReplayArtifact, types::*};
use crate::explicit_formal::{lower_explicit_formal, FormalLowering};
use lc631_core::context::SourceRevision;
use lc631_substrate::resource::{
    ResourceAmount, ResourceKind, ResourceRequest, ResourceReservationView,
};
use std::sync::Arc;

/// One source/context/parse owner, borrowing only within this callback. The
/// fixed return type cannot smuggle parser/native references past teardown.
pub fn with_replay_input<'v, R>(
    source: &str,
    profile: ReplayProfile,
    view: &'v ResourceReservationView<'_>,
    consume: impl for<'p> FnOnce(CanonicalRequest<'p, 'v>) -> Result<R, ReplayError>,
) -> Result<R, ReplayError> {
    view.check()?;
    if source.is_empty() || source.len() > 16384 {
        return Err(ReplayError::Limit);
    }
    if view.source_revision() != SourceRevision::of(source).digest() {
        return Err(ReplayError::Binding);
    }
    let _claim = view.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(1024 * 1024)?,
    )])?;
    let store = SourceStore::new(Arc::from(source), 16384).map_err(|_| ReplayError::Allocation)?;
    let goal = profile.goal_policy();
    let evaluation = if profile.evaluation {
        Some(DeclaredEvaluation::finite_boolean(
            SourceRevision::of(source),
            profile.base_scope(),
            goal,
        )?)
    } else {
        None
    };
    let context = super::context::original_context(&store, profile)?;
    let scope = context.state().scope();
    let lowered = lower_explicit_formal(&store, context.state().revision(), scope, view);
    let request = match &lowered {
        Ok(FormalLowering::Checked(problem)) => {
            let mut request = CanonicalRequest::formal(problem, context.state(), goal)?;
            if let Some(evaluation) = evaluation {
                request = request.with_evaluation(evaluation)?;
            }
            if profile.objective {
                request = request.with_objective_step(OneStepBudget::checked(3, 15000)?)?;
            }
            request
        }
        Ok(FormalLowering::Unsupported(report)) => {
            CanonicalRequest::unsupported(report, context.state())?
        }
        Err(error) => CanonicalRequest::clarify(error, context.state())?,
    };
    consume(request)
}

/// Fresh native execution occurs once. The callback sees the actual owned
/// success/error while parser/source/claims live, including postprocess errors.
/// Only its independently owned R escapes; no native owner is deserialized.
pub fn with_fresh_resume<'v, R>(
    artifact: &OwnedReplayArtifact<'_, '_>,
    services: CanonicalServices<'v, '_>,
    consume: impl for<'p> FnOnce(Result<OwnedCanonicalDecision<'p, 'v>, CanonicalError<'p, 'v>>) -> R,
) -> Result<R, ReplayError> {
    if services.registry.is_none() {
        return Err(ReplayError::MissingFreshRegistry);
    }
    let historical = artifact.inspect_current(services.current)?;
    let profile = historical.profile;
    with_replay_input(historical.source, profile, services.current, |request| {
        let scope = request.context.scope();
        if request.context.revision().interpretation_digest() != historical.context
            || [
                scope.comparison(),
                scope.goal(),
                scope.probes(),
                scope.semantics(),
            ] != historical.scope
        {
            return Err(ReplayError::Binding);
        }
        Ok(consume(solve_with_services(request, services)))
    })
}
