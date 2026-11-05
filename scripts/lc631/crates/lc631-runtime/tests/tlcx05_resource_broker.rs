use lc631_runtime::resource::{ResourceBroker, ResourceLimits, RunResources};
use lc631_substrate::resource::{
    Cancellation, Deadline, ReportedResourceLease, ResourceAmount, ResourceError, ResourceKind,
    ResourceRequest,
};
use lc631_substrate::Digest32;
use std::io::{self, Cursor, Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

fn request(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}
fn source() -> Digest32 {
    Digest32::from_bytes([7; 32])
}
fn deadline() -> Deadline {
    Deadline::after(Duration::from_secs(60)).unwrap()
}
fn broker(limit: u64) -> ResourceBroker {
    ResourceBroker::new(ResourceLimits::uniform(limit).unwrap()).unwrap()
}
fn run(owner: &ResourceBroker, id: u64, limit: u64) -> RunResources {
    owner
        .begin_run(
            id,
            source(),
            ResourceLimits::uniform(limit).unwrap(),
            deadline(),
            Cancellation::default(),
        )
        .unwrap()
}
fn failure<T>(result: Result<T, ResourceError>) -> ResourceError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected typed resource failure"),
    }
}

#[test]
fn tlcx05_atomic_multi_resource_failure_does_not_partially_publish_charges() {
    let owner = broker(8);
    let run = run(&owner, 1, 8);
    assert_eq!(
        failure(run.reserve(&[
            request(ResourceKind::Ram, 4),
            request(ResourceKind::Output, 9)
        ])),
        ResourceError::Exhausted(ResourceKind::Output)
    );
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
    assert_eq!(owner.used(ResourceKind::Output).unwrap(), 0);
    assert_eq!(failure(run.reserve(&[])), ResourceError::EmptyRequest);
}

#[test]
fn tlcx05_process_aggregate_is_shared_while_each_run_keeps_its_smaller_cap() {
    let owner = broker(10);
    let first = run(&owner, 1, 6);
    let second = run(&owner, 2, 6);
    assert_eq!(
        failure(first.reserve(&[request(ResourceKind::Ram, 7)])),
        ResourceError::Exhausted(ResourceKind::Ram)
    );
    let mut a = first.reserve(&[request(ResourceKind::Ram, 6)]).unwrap();
    let mut b = second.reserve(&[request(ResourceKind::Ram, 4)]).unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 10);
    assert_eq!(
        failure(second.reserve(&[request(ResourceKind::Ram, 1)])),
        ResourceError::Exhausted(ResourceKind::Ram)
    );
    a.release_confirmed().unwrap();
    b.release_confirmed().unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx05_same_owner_concurrent_reservation_race_cannot_exceed_aggregate() {
    let owner = broker(8);
    let first = run(&owner, 1, 8);
    let second = run(&owner, 2, 8);
    let barrier = Arc::new(Barrier::new(2));
    let results = std::thread::scope(|scope| {
        let gate_a = barrier.clone();
        let a = scope.spawn(move || {
            gate_a.wait();
            first.reserve(&[request(ResourceKind::Ram, 6)])
        });
        let gate_b = barrier.clone();
        let b = scope.spawn(move || {
            gate_b.wait();
            second.reserve(&[request(ResourceKind::Ram, 6)])
        });
        [a.join().unwrap(), b.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(ResourceError::Exhausted(ResourceKind::Ram))))
            .count(),
        1
    );
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 6);
    // Dropping the successful owner is not a confirmed release.
    drop(results);
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 6);
}

#[test]
fn tlcx05_run_cumulative_cap_is_enforced_before_the_larger_process_cap() {
    let owner = broker(20);
    let first = run(&owner, 1, 6);
    let second = run(&owner, 2, 6);
    let _first_live = first.reserve(&[request(ResourceKind::Ram, 4)]).unwrap();
    assert_eq!(
        failure(first.reserve(&[request(ResourceKind::Ram, 3)])),
        ResourceError::Exhausted(ResourceKind::Ram)
    );
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 4);
    let _second_live = second.reserve(&[request(ResourceKind::Ram, 5)]).unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 9);
}

#[test]
fn tlcx05_checked_u64_overflow_is_not_a_wrapped_success() {
    let owner = broker(u64::MAX);
    let first = run(&owner, 1, u64::MAX);
    let second = run(&owner, 2, u64::MAX);
    let _retained = first
        .reserve(&[request(ResourceKind::Ram, u64::MAX)])
        .unwrap();
    assert_eq!(
        failure(second.reserve(&[request(ResourceKind::Ram, 1)])),
        ResourceError::ArithmeticOverflow
    );
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), u64::MAX);
}

#[test]
fn tlcx05_drop_and_unknown_cleanup_keep_resource_charges_retained() {
    let owner = broker(8);
    let run = run(&owner, 1, 8);
    let mut reservation = run
        .reserve(&[
            request(ResourceKind::Child, 1),
            request(ResourceKind::Cleanup, 4),
        ])
        .unwrap();
    assert_eq!(reservation.lease().run_id(), 1);
    assert_eq!(reservation.lease().source_revision(), source());
    reservation.lease().record_cleanup_unknown();
    assert_eq!(
        reservation.release_confirmed().unwrap_err(),
        ResourceError::CleanupUnverified
    );
    drop(reservation);
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 1);
    assert_eq!(owner.used(ResourceKind::Cleanup).unwrap(), 4);
}

#[test]
fn tlcx05_confirmed_release_refunds_once_and_double_release_is_typed_rejection() {
    let owner = broker(8);
    let run = run(&owner, 1, 8);
    // No payload/child was started: cancelling an unused RAM reservation is
    // known reclamation, not a claim about an OS process or deleted cache.
    let mut reservation = run.reserve(&[request(ResourceKind::Ram, 6)]).unwrap();
    reservation.release_confirmed().unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
    assert_eq!(
        reservation.release_confirmed().unwrap_err(),
        ResourceError::AlreadyReleased
    );
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx05_cancelled_and_expired_runs_fail_before_reserving() {
    let owner = broker(8);
    let cancellation = Cancellation::default();
    let run = owner
        .begin_run(
            1,
            source(),
            ResourceLimits::uniform(8).unwrap(),
            deadline(),
            cancellation.clone(),
        )
        .unwrap();
    cancellation.cancel();
    assert_eq!(
        failure(run.reserve(&[request(ResourceKind::Ram, 1)])),
        ResourceError::Cancelled
    );
    assert_eq!(
        failure(owner.begin_run(
            2,
            source(),
            ResourceLimits::uniform(8).unwrap(),
            Deadline::at(Instant::now() - Duration::from_millis(1)),
            Cancellation::default()
        )),
        ResourceError::DeadlineExceeded
    );
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx05_run_identity_is_never_reused_and_foreign_owner_view_is_rejected() {
    let owner = broker(8);
    let first = run(&owner, 1, 8);
    assert_eq!(
        failure(owner.begin_run(
            1,
            source(),
            ResourceLimits::uniform(8).unwrap(),
            deadline(),
            Cancellation::default()
        )),
        ResourceError::RunIdReused
    );
    let mut reservation = first
        .reserve(&[
            request(ResourceKind::Ram, 2),
            request(ResourceKind::Output, 2),
        ])
        .unwrap();
    let foreign_owner = broker(8);
    let _foreign_run = run(&foreign_owner, 1, 8);
    assert_eq!(
        foreign_owner
            .validate_lease(&reservation.lease(), 1, source(), ResourceKind::Output)
            .unwrap_err(),
        ResourceError::ForeignOwner
    );
    assert_eq!(
        owner
            .validate_lease(
                &reservation.lease(),
                1,
                Digest32::from_bytes([8; 32]),
                ResourceKind::Output
            )
            .unwrap_err(),
        ResourceError::SourceMismatch
    );
    reservation.release_confirmed().unwrap();
    drop(first);
    assert_eq!(
        failure(owner.begin_run(
            1,
            source(),
            ResourceLimits::uniform(8).unwrap(),
            deadline(),
            Cancellation::default()
        )),
        ResourceError::RunIdReused
    );
}

#[test]
fn tlcx05_duplicate_resource_kind_and_unreserved_kind_are_not_implicit_quota() {
    let owner = broker(8);
    let run = run(&owner, 1, 8);
    assert_eq!(
        failure(run.reserve(&[
            request(ResourceKind::Disk, 2),
            request(ResourceKind::Disk, 2)
        ])),
        ResourceError::DuplicateResource(ResourceKind::Disk)
    );
    let reservation = run
        .reserve(&[
            request(ResourceKind::Ram, 8),
            request(ResourceKind::Output, 8),
        ])
        .unwrap();
    assert_eq!(
        failure(lc631_process_supervisor::read_bounded_bytes(
            &mut Cursor::new(b"proof"),
            &reservation.lease(),
            ResourceKind::Proof
        )),
        ResourceError::Unreserved(ResourceKind::Proof)
    );
    assert_eq!(owner.used(ResourceKind::Proof).unwrap(), 0);
}

#[test]
fn tlcx05_all_six_resource_kinds_use_the_same_root_owner() {
    let owner = broker(8);
    let run = run(&owner, 1, 8);
    for kind in [
        ResourceKind::Ram,
        ResourceKind::Disk,
        ResourceKind::Output,
        ResourceKind::Proof,
        ResourceKind::Child,
        ResourceKind::Cleanup,
    ] {
        let reservation = run.reserve(&[request(kind, 3)]).unwrap();
        assert_eq!(owner.used(kind).unwrap(), 3);
        drop(reservation);
        assert_eq!(owner.used(kind).unwrap(), 3);
    }
}

struct GrewAfterMetadata;
impl Read for GrewAfterMetadata {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        buffer.fill(b'x');
        Ok(buffer.len())
    }
}
struct FailingPipe {
    delivered: bool,
}
impl Read for FailingPipe {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.delivered {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.delivered = true;
        buffer[0] = b'x';
        Ok(1)
    }
}
struct SlowRead;
impl Read for SlowRead {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        std::thread::sleep(Duration::from_millis(150));
        buffer[0] = b'x';
        Ok(1)
    }
}

#[test]
fn tlcx05_reader_rejects_actual_growth_beyond_the_reserved_byte_count() {
    let owner = broker(8);
    let run = run(&owner, 1, 8);
    let reservation = run
        .reserve(&[
            request(ResourceKind::Ram, 8),
            request(ResourceKind::Output, 8),
        ])
        .unwrap();
    assert_eq!(
        failure(lc631_process_supervisor::read_bounded_bytes(
            &mut GrewAfterMetadata,
            &reservation.lease(),
            ResourceKind::Output
        )),
        ResourceError::Exhausted(ResourceKind::Output)
    );
}

#[test]
fn tlcx05_live_buffer_consumes_ram_and_output_without_lease_multiplication() {
    let owner = broker(8);
    let run = run(&owner, 1, 8);
    let mut reservation = run
        .reserve(&[
            request(ResourceKind::Ram, 8),
            request(ResourceKind::Output, 8),
        ])
        .unwrap();
    {
        let lease = reservation.lease();
        let bytes = lc631_process_supervisor::read_bounded_bytes(
            &mut Cursor::new(b"12345678"),
            &lease,
            ResourceKind::Output,
        )
        .unwrap();
        assert_eq!(bytes.as_slice(), b"12345678");
        assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 8);
        assert_eq!(owner.used(ResourceKind::Output).unwrap(), 8);
        assert_eq!(
            failure(lc631_process_supervisor::read_bounded_bytes(
                &mut Cursor::new(b"x"),
                &lease,
                ResourceKind::Output
            )),
            ResourceError::Exhausted(ResourceKind::Ram)
        );
        drop(bytes);
    }
    // Dropping the buffer alone does not refund the broker reservation.
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 8);
    assert_eq!(owner.used(ResourceKind::Output).unwrap(), 8);
    reservation.release_confirmed().unwrap();
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
    assert_eq!(owner.used(ResourceKind::Output).unwrap(), 0);
}

#[test]
fn tlcx05_refetching_a_view_cannot_reset_the_same_reservations_live_usage() {
    let owner = broker(8);
    let run = run(&owner, 1, 8);
    let reservation = run
        .reserve(&[
            request(ResourceKind::Ram, 8),
            request(ResourceKind::Output, 8),
        ])
        .unwrap();
    let first_view = reservation.lease();
    let first_buffer = lc631_process_supervisor::read_bounded_bytes(
        &mut Cursor::new(b"12345678"),
        &first_view,
        ResourceKind::Output,
    )
    .unwrap();
    let second_view = reservation.lease();
    assert_eq!(
        failure(lc631_process_supervisor::read_bounded_bytes(
            &mut Cursor::new(b"x"),
            &second_view,
            ResourceKind::Output,
        )),
        ResourceError::Exhausted(ResourceKind::Ram)
    );
    assert_eq!(first_buffer.as_slice(), b"12345678");
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 8);
    assert_eq!(owner.used(ResourceKind::Output).unwrap(), 8);
}

#[test]
fn tlcx05_mid_read_cancellation_retains_the_reservation_charge() {
    struct CancelDuringRead {
        cancellation: Cancellation,
        calls: usize,
    }
    impl Read for CancelDuringRead {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.calls += 1;
            buffer[0] = b'x';
            self.cancellation.cancel();
            Ok(1)
        }
    }
    let owner = broker(8);
    let cancellation = Cancellation::default();
    let run = owner
        .begin_run(
            1,
            source(),
            ResourceLimits::uniform(8).unwrap(),
            deadline(),
            cancellation.clone(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            request(ResourceKind::Ram, 8),
            request(ResourceKind::Output, 8),
        ])
        .unwrap();
    let mut reader = CancelDuringRead {
        cancellation,
        calls: 0,
    };
    assert_eq!(
        failure(lc631_process_supervisor::read_bounded_bytes(
            &mut reader,
            &reservation.lease(),
            ResourceKind::Output,
        )),
        ResourceError::Cancelled
    );
    assert_eq!(reader.calls, 1);
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 8);
    assert_eq!(owner.used(ResourceKind::Output).unwrap(), 8);
    drop(reservation);
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 8);
    assert_eq!(owner.used(ResourceKind::Output).unwrap(), 8);
}

#[test]
fn tlcx05_cancel_or_expired_deadline_is_checked_before_reading() {
    for cancel in [false, true] {
        let owner = broker(8);
        let cancellation = Cancellation::default();
        let until = if cancel {
            deadline()
        } else {
            Deadline::after(Duration::from_millis(40)).unwrap()
        };
        let run = owner
            .begin_run(
                1,
                source(),
                ResourceLimits::uniform(8).unwrap(),
                until,
                cancellation.clone(),
            )
            .unwrap();
        let reservation = run
            .reserve(&[
                request(ResourceKind::Ram, 8),
                request(ResourceKind::Output, 8),
            ])
            .unwrap();
        if cancel {
            cancellation.cancel();
        } else {
            std::thread::sleep(Duration::from_millis(60));
        }
        let mut reader = Cursor::new(b"input");
        let expected = if cancel {
            ResourceError::Cancelled
        } else {
            ResourceError::DeadlineExceeded
        };
        assert_eq!(
            failure(lc631_process_supervisor::read_bounded_bytes(
                &mut reader,
                &reservation.lease(),
                ResourceKind::Output
            )),
            expected
        );
        assert_eq!(reader.position(), 0);
    }
}

#[test]
fn tlcx05_deadline_is_rechecked_between_read_calls_without_os_interrupt_claim() {
    let owner = broker(8);
    let run = owner
        .begin_run(
            1,
            source(),
            ResourceLimits::uniform(8).unwrap(),
            Deadline::after(Duration::from_millis(100)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            request(ResourceKind::Ram, 8),
            request(ResourceKind::Output, 8),
        ])
        .unwrap();
    assert_eq!(
        failure(lc631_process_supervisor::read_bounded_bytes(
            &mut SlowRead,
            &reservation.lease(),
            ResourceKind::Output
        )),
        ResourceError::DeadlineExceeded
    );
    // A blocking Read cannot be preempted by a cooperative deadline. This test
    // checks the post-read boundary, not a hard OS-enforced 100ms wall limit.
}

#[test]
fn tlcx05_pipe_error_marks_child_cleanup_unverified_and_retains_all_charges() {
    let owner = broker(8);
    let run = run(&owner, 1, 8);
    let mut reservation = run
        .reserve(&[
            request(ResourceKind::Ram, 8),
            request(ResourceKind::Output, 8),
            request(ResourceKind::Child, 1),
            request(ResourceKind::Cleanup, 4),
        ])
        .unwrap();
    assert_eq!(
        failure(lc631_process_supervisor::read_bounded_bytes(
            &mut FailingPipe { delivered: false },
            &reservation.lease(),
            ResourceKind::Output
        )),
        ResourceError::Io(io::ErrorKind::BrokenPipe)
    );
    assert_eq!(
        reservation.release_confirmed().unwrap_err(),
        ResourceError::CleanupUnverified
    );
    drop(reservation);
    assert_eq!(owner.used(ResourceKind::Child).unwrap(), 1);
    assert_eq!(owner.used(ResourceKind::Cleanup).unwrap(), 4);
    assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 8);
    assert_eq!(owner.used(ResourceKind::Output).unwrap(), 8);
}

// rustc metadata-only probes produce no output files. Select an actual compiled
// dependency only after a positive control proves both required APIs resolve;
// then require E0308 for the SAME rlibs, never a missing-import error.
fn compiled_libraries(deps: &std::path::Path, name: &str) -> Vec<std::path::PathBuf> {
    let prefix = format!("lib{name}-");
    let mut candidates = std::fs::read_dir(deps)
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| {
            entry.file_name().to_string_lossy().starts_with(&prefix)
                && entry.path().extension().is_some_and(|ext| ext == "rlib")
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|entry| entry.metadata().unwrap().modified().unwrap());
    assert!(!candidates.is_empty(), "Cargo built dependency rlib");
    candidates
        .into_iter()
        .rev()
        .take(32)
        .map(|entry| entry.path())
        .collect()
}
fn compile_view_probe(
    view_type: &str,
    substrate: &std::path::Path,
    supervisor: &std::path::Path,
) -> std::process::Output {
    let import_type = view_type.split('<').next().unwrap();
    let executable = std::env::current_exe().unwrap();
    let deps = executable.parent().unwrap();
    let mut child = Command::new("rustc")
        .args([
            "--edition=2021",
            "--crate-type=lib",
            "--emit=metadata",
            "-o",
            "-",
            "-",
        ])
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .arg("--extern")
        .arg(format!("lc631_substrate={}", substrate.display()))
        .arg("--extern")
        .arg(format!("lc631_process_supervisor={}", supervisor.display()))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let probe = format!(
        "use lc631_substrate::resource::{{{import_type},ResourceKind}};\nfn check<'a>(r: &mut dyn std::io::Read, v: &'a {view_type}) {{ let _=lc631_process_supervisor::read_bounded_bytes(r,v,ResourceKind::Output); }}",
    );
    child
        .stdin
        .take()
        .unwrap()
        .write_all(probe.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn tlcx05_report_cannot_be_passed_to_the_live_reader_with_positive_compile_control() {
    let _report_type = std::any::type_name::<ReportedResourceLease>();
    let executable = std::env::current_exe().unwrap();
    let deps = executable.parent().unwrap();
    let supervisor = compiled_libraries(deps, "lc631_process_supervisor").remove(0);
    // A shared target may contain several feature/build revisions. A newer
    // substrate alone need not be the one referenced by this supervisor.
    // Admit a pair only after compiling its live-API positive control, then
    // keep those exact two paths for the negative probe as well.
    let mut last_error = String::new();
    let substrate = compiled_libraries(deps, "lc631_substrate")
        .into_iter()
        .find(|substrate| {
            let positive =
                compile_view_probe("ResourceReservationView<'a>", substrate, &supervisor);
            last_error = String::from_utf8_lossy(&positive.stderr).into_owned();
            positive.status.success()
        })
        .unwrap_or_else(|| {
            panic!("positive API control failed for every bounded rlib pair: {last_error}")
        });
    let negative = compile_view_probe("ReportedResourceLease", &substrate, &supervisor);
    assert!(!negative.status.success());
    let diagnostic = String::from_utf8(negative.stderr).unwrap();
    assert!(diagnostic.contains("E0308"), "{diagnostic}");
    assert!(
        diagnostic.contains("ResourceReservationView"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("ReportedResourceLease"), "{diagnostic}");
    assert!(
        !diagnostic.contains("E0432") && !diagnostic.contains("E0463"),
        "{diagnostic}"
    );
}
