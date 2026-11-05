//! Common-root child admission. Resource claims are not execution Authority.
use super::required_ram_bytes;
use super::{run_controlled, Observers, RunFailure, RunOutcome, RunRequest};
use lc631_substrate::resource::{
    ResourceAmount, ResourceClaim, ResourceError, ResourceKind, ResourceRequest,
    ResourceReservationView,
};

pub struct ResourceRunOutcome<'a> {
    // Drop actual captured buffers before returning the live accounting claim.
    outcome: RunOutcome,
    _claim: ResourceClaim<'a>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceRunError {
    NotStarted(std::io::ErrorKind),
    Resource(ResourceError),
}
impl From<ResourceError> for ResourceRunError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}
impl ResourceRunOutcome<'_> {
    pub fn outcome(&self) -> &RunOutcome {
        &self.outcome
    }
    pub fn cleanup_complete(&self) -> bool {
        self.outcome.io_cleanup_complete && self.outcome.process_tree_kill_verified
    }
    pub fn cancelled(&self) -> bool {
        self.outcome.diagnostic == Some("supervisor_cancelled")
    }
}

pub fn run_bounded_with_resources<'a>(
    request: RunRequest<'_>,
    view: &'a ResourceReservationView<'a>,
) -> Result<ResourceRunOutcome<'a>, ResourceError> {
    run_bounded_with_resources_observed(request, view).map_err(|error| match error {
        ResourceRunError::NotStarted(kind) => ResourceError::Io(kind),
        ResourceRunError::Resource(error) => error,
    })
}

/// Detailed additive facade. Only the private native creation boundary can
/// report NotStarted; callers must not infer it from arbitrary Io errors.
pub fn run_bounded_with_resources_observed<'a>(
    mut request: RunRequest<'_>,
    view: &'a ResourceReservationView<'a>,
) -> Result<ResourceRunOutcome<'a>, ResourceRunError> {
    view.check()?;
    let output = request
        .stdout_limit
        .checked_add(request.stderr_limit)
        .ok_or(ResourceError::ArithmeticOverflow)?;
    let ram = required_ram_bytes(&request)?;
    let requests = [
        resource(ResourceKind::Child, 1)?,
        resource(ResourceKind::Cleanup, 1)?,
        resource(ResourceKind::Ram, ram)?,
        resource(
            ResourceKind::Output,
            u64::try_from(output.max(1)).map_err(|_| ResourceError::ArithmeticOverflow)?,
        )?,
    ];
    let claim = view.claim_resources(&requests)?;
    request.timeout = request.timeout.min(view.remaining()?);
    let outcome =
        run_controlled(request, Observers::default(), Some(view)).map_err(
            |failure| match failure {
                RunFailure::NotStarted(error)
                    if error.kind() == std::io::ErrorKind::OutOfMemory =>
                {
                    ResourceRunError::Resource(ResourceError::AllocationFailed)
                }
                RunFailure::NotStarted(error) => ResourceRunError::NotStarted(error.kind()),
                RunFailure::StartedUnknown(error) => {
                    view.record_cleanup_unknown();
                    ResourceRunError::Resource(ResourceError::Io(error.kind()))
                }
            },
        )?;
    if !outcome.io_cleanup_complete || !outcome.process_tree_kill_verified {
        view.record_cleanup_unknown();
    }
    Ok(ResourceRunOutcome {
        outcome,
        _claim: claim,
    })
}

fn resource(kind: ResourceKind, amount: u64) -> Result<ResourceRequest, ResourceError> {
    Ok(ResourceRequest::new(kind, ResourceAmount::checked(amount)?))
}
