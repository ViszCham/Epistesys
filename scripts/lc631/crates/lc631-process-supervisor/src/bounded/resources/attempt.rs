//! Actual supervisor attempt retains captures and partial native/IO ownership.
#![forbid(unsafe_code)]
use super::*;
use crate::{
    bounded::{
        admitted::{run_admitted, ControlledFailure, PartialIoOwner},
        Observers, RunOutcome,
    },
    native_attempt::{NativePartialOwner, NativeSpawnFailure},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeProcessEffect {
    NotAttempted,
    MayHaveStarted,
    Completed,
}
pub struct ResourceRunAttempt<'res, E> {
    // Real byte/pipe/child owners drop before their accounting claim.
    outcome: Option<RunOutcome>,
    partial: Option<PartialIoOwner>,
    native: Option<NativePartialOwner>,
    effect: NativeProcessEffect,
    resource_error: Option<ResourceError>,
    admission_error: Option<E>,
    claim: Option<ResourceClaim<'res>>,
    work: Option<ResourceClaim<'res>>,
}
impl<'res, E> ResourceRunAttempt<'res, E> {
    pub fn effect(&self) -> NativeProcessEffect {
        self.effect
    }
    pub fn outcome(&self) -> Option<&RunOutcome> {
        self.outcome.as_ref()
    }
    pub fn resource_error(&self) -> Option<&ResourceError> {
        self.resource_error.as_ref()
    }
    pub fn admission_error(&self) -> Option<&E> {
        self.admission_error.as_ref()
    }
    pub fn retains_partial(&self) -> bool {
        self.partial.is_some() || self.native.is_some()
    }
    pub fn try_close_partial(&mut self, timeout: std::time::Duration) -> Result<(), ResourceError> {
        if timeout.is_zero() || timeout > std::time::Duration::from_millis(250) {
            return Err(ResourceError::InvalidConfig);
        }
        if let Some(owner) = self.partial.as_mut() {
            if !owner
                .cleanup(timeout)
                .map_err(|e| ResourceError::Io(e.kind()))?
            {
                return Err(ResourceError::CleanupUnverified);
            }
            self.partial = None;
        }
        if let Some(owner) = self.native.as_mut() {
            if !owner
                .cleanup(timeout)
                .map_err(|e| ResourceError::Io(e.kind()))?
            {
                return Err(ResourceError::CleanupUnverified);
            }
            self.native = None;
        }
        // Diagnostic/effect history is intentionally never cleared by cleanup.
        Ok(())
    }
    // Failure must return the original live capture/cleanup/resource owner.
    // Boxing it here would introduce an unbudgeted allocation on failure.
    #[expect(
        clippy::result_large_err,
        reason = "return the original owner without allocating on failure"
    )]
    pub fn into_completed(mut self) -> Result<ResourceRunOutcome<'res>, Self> {
        if self.resource_error.is_some()
            || self.admission_error.is_some()
            || self.retains_partial()
            || self.outcome.is_none()
            || self.claim.is_none()
        {
            return Err(self);
        }
        // Both Options were checked and are private to this actual producer.
        let outcome = self.outcome.take().expect("private outcome checked above");
        let claim = self
            .claim
            .take()
            .expect("private funded claim checked above");
        Ok(ResourceRunOutcome {
            outcome,
            _claim: claim,
        })
    }
}
pub fn run_bounded_with_resources_attempt<'res, E>(
    request: RunRequest<'_>,
    view: &'res ResourceReservationView<'res>,
    admit: &mut dyn FnMut() -> Result<(), E>,
) -> ResourceRunAttempt<'res, E> {
    run_observed_attempt(request, view, admit, Observers::default())
}
fn run_observed_attempt<'res, E>(
    mut request: RunRequest<'_>,
    view: &'res ResourceReservationView<'res>,
    admit: &mut dyn FnMut() -> Result<(), E>,
    observers: Observers,
) -> ResourceRunAttempt<'res, E> {
    let mut a = ResourceRunAttempt {
        outcome: None,
        partial: None,
        native: None,
        effect: NativeProcessEffect::NotAttempted,
        resource_error: None,
        admission_error: None,
        claim: None,
        work: None,
    };
    let prepared = (|| {
        view.check()?;
        let output = request
            .stdout_limit
            .checked_add(request.stderr_limit)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        let ram = required_ram_bytes(&request)?;
        a.work = Some(view.claim_resources(&[
            resource(ResourceKind::Child, 1)?,
            resource(ResourceKind::Cleanup, 1)?,
        ])?);
        a.claim = Some(view.claim_resources(&[
            resource(ResourceKind::Ram, ram)?,
            resource(
                ResourceKind::Output,
                u64::try_from(output.max(1)).map_err(|_| ResourceError::ArithmeticOverflow)?,
            )?,
        ])?);
        request.timeout = request.timeout.min(view.remaining()?);
        Ok::<_, ResourceError>(())
    })();
    if let Err(error) = prepared {
        a.resource_error = Some(error);
        return a;
    }
    match run_admitted(request, observers, view, admit, &mut a.partial) {
        Ok(outcome) => {
            a.effect = NativeProcessEffect::Completed;
            let error = if !outcome.io_cleanup_complete || !outcome.process_tree_kill_verified {
                view.record_cleanup_unknown();
                Some(ResourceError::CleanupUnverified)
            } else if outcome.diagnostic == Some("supervisor_cancelled") {
                Some(ResourceError::Cancelled)
            } else if outcome.timed_out {
                Some(ResourceError::DeadlineExceeded)
            } else if outcome.stdout_truncated || outcome.stderr_truncated {
                Some(ResourceError::Exhausted(ResourceKind::Output))
            } else if outcome.diagnostic.is_some() {
                Some(ResourceError::Io(std::io::ErrorKind::Other))
            } else {
                None
            };
            if outcome.io_cleanup_complete
                && outcome.process_tree_kill_verified
                && a.partial.is_none()
            {
                // Actual completed captures retain byte funding, not a phantom
                // running child. Reservation/run quota and call counters remain.
                a.work = None;
            }
            a.outcome = Some(outcome);
            a.resource_error = error;
        }
        Err(ControlledFailure::BeforeChild(error)) => {
            a.resource_error = Some(ResourceError::Io(error.kind()))
        }
        Err(ControlledFailure::Native(failure)) => match failure {
            NativeSpawnFailure::BeforeChild(error) => {
                a.resource_error = Some(ResourceError::Io(error.kind()))
            }
            NativeSpawnFailure::Admission(error) => a.admission_error = Some(error),
            NativeSpawnFailure::Resource(error) => a.resource_error = Some(error),
            NativeSpawnFailure::AfterChild { error, owner } => {
                view.record_cleanup_unknown();
                a.effect = NativeProcessEffect::MayHaveStarted;
                a.resource_error = Some(ResourceError::Io(error.kind()));
                a.native = Some(owner);
            }
        },
    }
    if a.effect == NativeProcessEffect::NotAttempted {
        a.work = None;
        a.claim = None;
    }
    a
}
#[cfg(test)]
mod tests;
