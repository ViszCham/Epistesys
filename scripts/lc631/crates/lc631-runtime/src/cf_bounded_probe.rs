//! Resource-backed compiler transport through the existing process supervisor.
//!
//! Complete transport does not authenticate a compiler, establish an expected
//! source diagnostic, or grant authority. Those are separate caller obligations.

mod capture;
mod diagnostics;
mod request;

use lc631_process_supervisor::{ResourceRunError, ResourceRunOutcome, RunOutcome, RunRequest};
use lc631_substrate::resource::{ResourceError, ResourceReservationView};
use request::PreparedRequest;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Purpose {
    PositiveControl,
    NegativeCompile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Failure {
    InvalidCaptureLimits,
    InputLimit,
    OutputLimit(Stream),
    TimedOut,
    PartialDiagnostic,
    Ice,
    UnexpectedExit(Option<i32>),
    MissingDiagnostics,
    InvalidUtf8,
    UnknownDiagnosticSchema,
    MissingImport,
    IncompleteCleanup,
    ForeignReservation,
    Resource(ResourceError),
    Supervisor(ResourceRunError),
    UnknownSupervisorDiagnostic(&'static str),
}

/// Holds the actual supervisor outcome and the funded request that produced it.
/// There is no constructor from a report or from caller-supplied output bytes.
pub struct ProbeAttempt<'v> {
    // Drop captured bytes before either accounting claim.
    capture: Option<ResourceRunOutcome<'v>>,
    prepared: Option<PreparedRequest<'v>>,
    view: &'v ResourceReservationView<'v>,
    purpose: Purpose,
    failure: Option<Failure>,
    supervisor_error: Option<ResourceRunError>,
}

/// A scoped read-only view of complete transport, not a compiler proof.
pub struct CompleteProbeIo<'s> {
    raw: &'s RunOutcome,
    exit_code: i32,
    purpose: Purpose,
}

/// Run one bounded request using the actual resource-aware supervisor.
pub fn run<'v>(
    request: RunRequest<'_>,
    purpose: Purpose,
    view: &'v ResourceReservationView<'v>,
) -> ProbeAttempt<'v> {
    let mut attempt = ProbeAttempt {
        capture: None,
        prepared: None,
        view,
        purpose,
        failure: None,
        supervisor_error: None,
    };
    let prepared = match PreparedRequest::new(request, purpose, view) {
        Ok(prepared) => prepared,
        Err(failure) => {
            attempt.failure = Some(failure);
            return attempt;
        }
    };
    let captured = lc631_process_supervisor::run_bounded_with_resources_observed(
        prepared.as_request(),
        view,
    );
    attempt.prepared = Some(prepared);
    match captured {
        Ok(capture) => {
            attempt.failure = capture::validate(&capture, purpose, view).err();
            attempt.capture = Some(capture);
        }
        Err(error) => {
            // Resource(Io) may describe a started child with unknown cleanup.
            // Preserve the actual error; never synthesize a NotStarted result.
            attempt.failure = Some(Failure::Supervisor(error.clone()));
            attempt.supervisor_error = Some(error);
        }
    }
    attempt
}

impl ProbeAttempt<'_> {
    /// Historical raw bytes remain inspectable even when consumption is refused.
    pub fn raw(&self) -> Option<&RunOutcome> {
        self.capture.as_ref().map(ResourceRunOutcome::outcome)
    }

    pub fn supervisor_error(&self) -> Option<&ResourceRunError> {
        self.supervisor_error.as_ref()
    }

    pub fn failure(&self) -> Option<&Failure> {
        self.failure.as_ref()
    }

    fn check_current(&self, expected: &ResourceReservationView<'_>) -> Result<(), Failure> {
        self.view.check().map_err(Failure::Resource)?;
        expected.check().map_err(Failure::Resource)?;
        if self.view.owner_id() != expected.owner_id()
            || self.view.reservation_id() != expected.reservation_id()
            || self.view.run_id() != expected.run_id()
            || self.view.source_revision() != expected.source_revision()
        {
            return Err(Failure::ForeignReservation);
        }
        if let Some(prepared) = &self.prepared {
            prepared.check_current(expected)?;
        }
        Ok(())
    }

    /// The callback can consume borrowed transport only while its original
    /// reservation remains current. Recheck after the callback before returning.
    pub fn with_complete<R>(
        &self,
        expected_view: &ResourceReservationView<'_>,
        consume: impl for<'s> FnOnce(CompleteProbeIo<'s>) -> R,
    ) -> Result<R, Failure> {
        self.check_current(expected_view)?;
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let capture = self.capture.as_ref().ok_or(Failure::IncompleteCleanup)?;
        // Capture bytes are private and immutable; structured validation was
        // performed at construction under the retained scratch claim.
        capture::check_transport(capture, self.purpose)?;
        let raw = capture.outcome();
        let exit_code = raw.exit_code.ok_or(Failure::UnexpectedExit(None))?;
        let result = consume(CompleteProbeIo {
            raw,
            exit_code,
            purpose: self.purpose,
        });
        self.check_current(expected_view)?;
        Ok(result)
    }
}

impl<'s> CompleteProbeIo<'s> {
    pub fn stdout(&self) -> &'s [u8] {
        &self.raw.stdout
    }

    pub fn stderr(&self) -> &'s [u8] {
        &self.raw.stderr
    }

    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }

    pub fn purpose(&self) -> Purpose {
        self.purpose
    }
}
