//! Successful origin/current captures and the actual inventory left by late errors.
use super::{matches_source, provider::CaptureAuthority, ClockFault, Error, RoleSession};
use crate::contextual_repair::snapshot_gate::CodingBuildPort;
use lc631_analysis::declared_build_capture::{OwnedDeclaredBuildCapture, SnapshotCaptureError};
use lc631_substrate::resource::ResourceReservationView;
use std::cell::Cell;

pub struct RegisteredCapture<'s, 'p, 'v> {
    session: &'s RoleSession<'s, 'p, 'v>,
    before: bool,
    actual: OwnedDeclaredBuildCapture<'v>,
    resources: &'v ResourceReservationView<'v>,
}

impl<'s, 'p, 'v> RegisteredCapture<'s, 'p, 'v> {
    pub(crate) fn session(&self) -> &'s RoleSession<'s, 'p, 'v> {
        self.session
    }

    pub(crate) fn actual(&self) -> &OwnedDeclaredBuildCapture<'v> {
        &self.actual
    }

    pub(crate) fn is_before(&self) -> bool {
        self.before
    }

    fn check_current(&self) -> Result<(), Error> {
        self.actual
            .check_current(self.resources)
            .map_err(Error::Snapshot)?;
        self.session.check_current_binding(self.resources)
    }
}

struct Rejection<'v> {
    partial: Option<OwnedDeclaredBuildCapture<'v>>,
    primary: Error,
    provider: Option<Error>,
    snapshot: Option<SnapshotCaptureError>,
}

pub struct CaptureAttempt<'s, 'p, 'v> {
    outcome: Result<RegisteredCapture<'s, 'p, 'v>, Rejection<'v>>,
}

impl<'s, 'p, 'v> CaptureAttempt<'s, 'p, 'v> {
    pub fn current(&self) -> Result<&RegisteredCapture<'s, 'p, 'v>, Error> {
        match &self.outcome {
            Ok(capture) => {
                capture.check_current()?;
                Ok(capture)
            }
            Err(rejected) => Err(rejected.primary.clone()),
        }
    }

    pub fn actual_capture(&self) -> Option<&OwnedDeclaredBuildCapture<'v>> {
        match &self.outcome {
            Ok(capture) => Some(&capture.actual),
            Err(rejected) => rejected.partial.as_ref(),
        }
    }

    pub fn clock_fault(&self) -> Option<&ClockFault> {
        let rejected = self.outcome.as_ref().err()?;
        match rejected.provider.as_ref().unwrap_or(&rejected.primary) {
            Error::Clock(fault) => Some(fault),
            _ => None,
        }
    }

    /// Includes the original producer's diagnostic even when an early clock
    /// fault is the primary refusal and no inventory has been created.
    pub fn snapshot_error(&self) -> Option<&SnapshotCaptureError> {
        self.outcome
            .as_ref()
            .err()
            .and_then(|r| r.snapshot.as_ref())
    }

    /// A grant provider may refuse with a cause more precise than the capture
    /// producer's vocabulary. A partial inventory preserves both diagnostics.
    pub fn provider_error(&self) -> Option<&Error> {
        self.outcome
            .as_ref()
            .err()
            .and_then(|r| r.provider.as_ref())
    }

    fn rejected(
        partial: Option<OwnedDeclaredBuildCapture<'v>>,
        primary: Error,
        provider: Option<Error>,
        snapshot: Option<SnapshotCaptureError>,
    ) -> Self {
        Self {
            outcome: Err(Rejection {
                partial,
                primary,
                provider,
                snapshot,
            }),
        }
    }

    fn refusal(error: Error) -> Self {
        let snapshot = match &error {
            Error::Snapshot(error) => Some(error.clone()),
            _ => None,
        };
        Self::rejected(None, error, None, snapshot)
    }
}

// The profile funds one bounded grant-material buffer per role. Block reentry
// without holding a mutex across an arbitrary grant, registry or clock callback.
struct ActiveCapture<'a>(&'a Cell<bool>);

impl Drop for ActiveCapture<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

impl<'o, 'p, 'v> RoleSession<'o, 'p, 'v> {
    pub fn capture_before<'s>(
        &'s self,
        port: &mut CodingBuildPort<'_, '_>,
        view: &'v ResourceReservationView<'_>,
    ) -> CaptureAttempt<'s, 'p, 'v> {
        self.capture(port, view, true)
    }

    pub fn capture_current<'s>(
        &'s self,
        port: &mut CodingBuildPort<'_, '_>,
        view: &'v ResourceReservationView<'_>,
    ) -> CaptureAttempt<'s, 'p, 'v> {
        self.capture(port, view, false)
    }

    fn capture<'s>(
        &'s self,
        port: &mut CodingBuildPort<'_, '_>,
        view: &'v ResourceReservationView<'_>,
        before: bool,
    ) -> CaptureAttempt<'s, 'p, 'v> {
        if before == self.origin_ready.get()
            || !matches_source(self.capture.binding(), port.binding(), self.plan)
            || self.capture_in_progress.replace(true)
        {
            return CaptureAttempt::refusal(Error::Clock(ClockFault::BindingMismatch));
        }
        let _active = ActiveCapture(&self.capture_in_progress);
        if let Err(error) = self.check_current_binding(view) {
            return CaptureAttempt::refusal(error);
        }
        if let Err(error) = port.binding().check_goal_profile_inputs(view) {
            return CaptureAttempt::refusal(Error::Snapshot(error));
        }
        let mut authority = CaptureAuthority::new(self, view);
        let attempt = port.capture_current(&mut authority, view);
        let (actual, diagnostic) = attempt.into_parts();
        if let Some(original) = diagnostic {
            // A late rejection owns both the genuine partial inventory and its
            // original producer error. An early clock failure has no such owner.
            let primary = if actual.is_some() {
                Error::Snapshot(original.clone())
            } else {
                authority
                    .failure
                    .clone()
                    .unwrap_or_else(|| Error::Snapshot(original.clone()))
            };
            return CaptureAttempt::rejected(actual, primary, authority.failure, Some(original));
        }
        let Some(actual) = actual else {
            return CaptureAttempt::refusal(Error::Snapshot(SnapshotCaptureError::InvalidSnapshot));
        };
        if let Some(error) = authority.failure {
            return CaptureAttempt::rejected(Some(actual), error.clone(), Some(error), None);
        }
        if let Err(error) = actual.check_current(view) {
            return CaptureAttempt::rejected(
                Some(actual),
                Error::Snapshot(error.clone()),
                None,
                Some(error),
            );
        }
        if let Err(error) = self.check_current_binding(view) {
            return CaptureAttempt::rejected(Some(actual), error.clone(), Some(error), None);
        }
        if before {
            self.origin_ready.set(true);
        }
        CaptureAttempt {
            outcome: Ok(RegisteredCapture {
                session: self,
                before,
                actual,
                resources: view,
            }),
        }
    }
}
