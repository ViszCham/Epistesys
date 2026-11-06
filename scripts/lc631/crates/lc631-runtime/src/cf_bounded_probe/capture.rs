use super::{diagnostics, Failure, Purpose, Stream};
use lc631_process_supervisor::ResourceRunOutcome;
use lc631_substrate::resource::{ResourceError, ResourceReservationView};

pub(super) fn validate(
    capture: &ResourceRunOutcome<'_>,
    purpose: Purpose,
    view: &ResourceReservationView<'_>,
) -> Result<(), Failure> {
    if capture.outcome().diagnostic == Some("supervisor_resource_scope_failed") {
        // Keep the live broker's actual cause when it remains observable.
        // The supervisor's opaque string alone cannot establish StaleLease.
        return Err(match view.check() {
            Err(error) => Failure::Resource(error),
            Ok(()) => Failure::UnknownSupervisorDiagnostic("supervisor_resource_scope_failed"),
        });
    }
    check_transport(capture, purpose)?;
    view.check().map_err(Failure::Resource)?;
    diagnostics::validate(&capture.outcome().stderr, purpose, view)?;
    view.check().map_err(Failure::Resource)
}

pub(super) fn check_transport(
    capture: &ResourceRunOutcome<'_>,
    purpose: Purpose,
) -> Result<(), Failure> {
    let raw = capture.outcome();
    if !capture.cleanup_complete() {
        return Err(Failure::IncompleteCleanup);
    }
    if raw.stdout_truncated {
        return Err(Failure::OutputLimit(Stream::Stdout));
    }
    if raw.stderr_truncated {
        return Err(Failure::OutputLimit(Stream::Stderr));
    }
    if raw.timed_out {
        return Err(Failure::TimedOut);
    }
    if let Some(diagnostic) = raw.diagnostic {
        return Err(match diagnostic {
            "supervisor_admission_deadline_exceeded" | "supervisor_deadline_exceeded" => {
                Failure::TimedOut
            }
            "supervisor_cancelled" => Failure::Resource(ResourceError::Cancelled),
            "supervisor_stdin_thread_start_failed"
            | "supervisor_pipe_read_failed"
            | "supervisor_stdin_write_failed"
            | "supervisor_process_wait_failed"
            | "supervisor_tree_observation_failed"
            | "supervisor_output_budget_exceeded"
            | "supervisor_stdin_thread_start_failed_cleanup_unverified"
            | "supervisor_cleanup_unverified" => Failure::IncompleteCleanup,
            unknown => Failure::UnknownSupervisorDiagnostic(unknown),
        });
    }
    let expected = match purpose {
        Purpose::PositiveControl => 0,
        Purpose::NegativeCompile => 1,
    };
    if raw.exit_code != Some(expected) {
        return Err(Failure::UnexpectedExit(raw.exit_code));
    }
    Ok(())
}
