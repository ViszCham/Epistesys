//! 日本語: 同じrunの既存leaseも短縮するdeadline契約。中央実行待ち。
//! English: Deadline narrowing reaches existing leases in the same run; central executes.
//! Missing API compilation RED is planned, not an observed behavioral failure.
#![forbid(unsafe_code)]
#[path = "tlcx05_coding_deadline_support/mod.rs"]
mod support;

use lc631_analysis::{declared_build_capture::*, run_bounded_tool_with_resources};
use lc631_runtime::{contextual_repair::snapshot_gate::CodingBuildPort, resource::*};
use std::time::Duration;
use support::*;

#[test]
fn tlcx05_live_ingress_lease_and_claim_observe_same_authoritative_narrowing() {
    // Exact additive API; no metadata constructor or replacement broker.
    let _: fn(&RunResources, Deadline) -> Result<DeadlineNarrowing, ResourceError> =
        RunResources::narrow_deadline;
    let (broker, mut run) = resources(SOURCE);
    let mut ingress = run
        .reserve_for_source(source_digest(SOURCE), &requests())
        .unwrap();
    {
        let view = ingress.lease();
        let identities = (
            view.owner_id(),
            view.run_id(),
            view.reservation_id(),
            view.source_revision(),
        );
        let claim = view.claim_resources(&[q(ResourceKind::Ram, 32)]).unwrap();
        let before = broker.used(ResourceKind::Ram).unwrap();
        run.narrow_deadline(Deadline::after(Duration::from_millis(200)).unwrap())
            .unwrap();
        assert!(view.remaining().unwrap() <= Duration::from_millis(200));
        assert_eq!(
            (
                view.owner_id(),
                view.run_id(),
                view.reservation_id(),
                view.source_revision()
            ),
            identities
        );
        assert_eq!(broker.used(ResourceKind::Ram).unwrap(), before);
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(view.check(), Err(ResourceError::DeadlineExceeded));
        assert_eq!(view.remaining(), Err(ResourceError::DeadlineExceeded));
        assert_eq!(claim.check(), Err(ResourceError::DeadlineExceeded));
        assert_eq!(
            broker.validate_lease(&view, identities.1, identities.3, ResourceKind::Ram),
            Err(ResourceError::DeadlineExceeded)
        );
        // Expiry does not refund a still-live reservation.
        assert_eq!(broker.used(ResourceKind::Ram).unwrap(), before);
    }
    ingress.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx05_extension_and_new_reservation_cannot_reset_deadline_source_or_quota() {
    let (broker, mut run) = resources(SOURCE);
    let mut ingress = run
        .reserve_for_source(source_digest(SOURCE), &requests())
        .unwrap();
    let mut next;
    {
        let old = ingress.lease();
        run.narrow_deadline(Deadline::after(Duration::from_millis(200)).unwrap())
            .unwrap();
        run.narrow_deadline(Deadline::after(Duration::from_secs(3600)).unwrap())
            .unwrap();
        next = run
            .reserve_for_source(source_digest(SOURCE), &[q(ResourceKind::Ram, 16)])
            .unwrap();
        let fresh = next.lease();
        assert_eq!(old.owner_id(), fresh.owner_id());
        assert_eq!(old.run_id(), fresh.run_id());
        assert_eq!(old.source_revision(), fresh.source_revision());
        assert_ne!(old.reservation_id(), fresh.reservation_id());
        assert_eq!(old.amount(ResourceKind::Child).unwrap(), 2);
        assert!(matches!(
            run.reserve(&[q(ResourceKind::Child, 1)]),
            Err(ResourceError::Exhausted(ResourceKind::Child))
        ));
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(old.check(), Err(ResourceError::DeadlineExceeded));
        assert_eq!(fresh.check(), Err(ResourceError::DeadlineExceeded));
        assert!(matches!(
            run.reserve(&[q(ResourceKind::Ram, 1)]),
            Err(ResourceError::DeadlineExceeded)
        ));
        assert_eq!(broker.used(ResourceKind::Child).unwrap(), 2);
    }
    next.release_confirmed().unwrap();
    ingress.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_short_external_deadline_rejects_before_child_and_inventory_leaf_io() {
    let fixture = CaptureFixture::new();
    let marker = fixture.root.join("not-started.deadline-marker");
    let (broker, mut run) = resources(SOURCE);
    let mut reservation = run
        .reserve_for_source(source_digest(SOURCE), &requests())
        .unwrap();
    {
        let view = reservation.lease();
        run.narrow_deadline(Deadline::after(Duration::from_millis(20)).unwrap())
            .unwrap();
        std::thread::sleep(Duration::from_millis(60));
        let program = std::env::current_exe().unwrap();
        let args = child_args(&marker);
        let result = run_bounded_tool_with_resources(child_request(&program, &args), &view);
        assert!(matches!(result, Err(ResourceError::DeadlineExceeded)));
        assert!(!marker.exists(), "private child marker must remain absent");
        let mut runner = Inputs::new(&run);
        let mut authority = fixture.authority();
        let attempt =
            capture_declared_build(&fixture.binding(), &mut runner, &mut authority, &view);
        assert_eq!(
            attempt.diagnostic(),
            Some(&SnapshotCaptureError::Resource(
                ResourceError::DeadlineExceeded
            ))
        );
        assert!(attempt.capture().is_none());
        assert_eq!(runner.reads, 0);
        assert_eq!(authority.calls, 0);
        assert_eq!(broker.used(ResourceKind::Child).unwrap(), 2);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_actual_child_timeout_uses_narrowed_existing_view() {
    let fixture = CaptureFixture::new();
    let marker = fixture.root.join("clipped.deadline-marker");
    let (_, mut run) = resources(SOURCE);
    let mut reservation = run
        .reserve_for_source(source_digest(SOURCE), &requests())
        .unwrap();
    {
        let view = reservation.lease();
        run.narrow_deadline(Deadline::after(Duration::from_secs(2)).unwrap())
            .unwrap();
        let program = std::env::current_exe().unwrap();
        let args = child_args(&marker);
        let result = run_bounded_tool_with_resources(child_request(&program, &args), &view);
        assert!(matches!(result, Err(ResourceError::DeadlineExceeded)));
        // Nonempty actual-child evidence, not elapsed-time benchmarking.
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "started");
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_narrowing_after_child_started_reaches_supervisor_resource_poll() {
    let fixture = CaptureFixture::new();
    let marker = fixture.root.join("live-child.deadline-marker");
    let (_, mut run) = resources(SOURCE);
    let mut reservation = run
        .reserve_for_source(source_digest(SOURCE), &requests())
        .unwrap();
    {
        let view = reservation.lease();
        let program = std::env::current_exe().unwrap();
        let args = child_args(&marker);
        std::thread::scope(|scope| {
            let cutting_run = &run;
            let marker = &marker;
            let cut = scope.spawn(move || {
                let wait = std::time::Instant::now();
                while !marker.exists() {
                    assert!(
                        wait.elapsed() < Duration::from_secs(4),
                        "actual child did not start"
                    );
                    std::thread::sleep(Duration::from_millis(5));
                }
                cutting_run
                    .narrow_deadline(Deadline::after(Duration::from_millis(100)).unwrap())
                    .unwrap();
            });
            let result = run_bounded_tool_with_resources(child_request(&program, &args), &view);
            cut.join().unwrap();
            assert!(matches!(result, Err(ResourceError::DeadlineExceeded)));
        });
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "started");
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_actual_inventory_positive_and_missing_authority_use_same_entry() {
    let fixture = CaptureFixture::new();
    let (_, mut run) = resources(SOURCE);
    let mut reservation = run
        .reserve_for_source(source_digest(SOURCE), &requests())
        .unwrap();
    {
        let view = reservation.lease();
        run.narrow_deadline(Deadline::after(Duration::from_secs(20)).unwrap())
            .unwrap();
        let mut runner = Inputs::new(&run);
        let mut absent = MissingAuthority;
        let held = capture_declared_build(&fixture.binding(), &mut runner, &mut absent, &view);
        assert_eq!(
            held.diagnostic(),
            Some(&SnapshotCaptureError::AuthorityUnavailable)
        );
        assert_eq!(runner.reads, 0);
        let mut authority = fixture.authority();
        let observed =
            capture_declared_build(&fixture.binding(), &mut runner, &mut authority, &view);
        let current = observed.current().unwrap();
        current.check_current(&view).unwrap();
        assert!(
            runner.reads >= 2,
            "actual Cargo.toml and Rust leaf reads are required"
        );
        assert_eq!(authority.calls, 2);
        assert_eq!(current.root(), fixture.binding().root());
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_inventory_stops_after_actual_first_leaf_when_same_run_is_narrowed() {
    let fixture = CaptureFixture::new();
    let (_, mut run) = resources(SOURCE);
    let mut reservation = run
        .reserve_for_source(source_digest(SOURCE), &requests())
        .unwrap();
    {
        let view = reservation.lease();
        let mut runner = Inputs::new(&run);
        runner.expire_after_first_read = true;
        let mut authority = fixture.authority();
        let attempt =
            capture_declared_build(&fixture.binding(), &mut runner, &mut authority, &view);
        assert_eq!(
            runner.reads, 1,
            "one real funded leaf was consumed before the cut"
        );
        assert_eq!(
            attempt.diagnostic(),
            Some(&SnapshotCaptureError::Resource(
                ResourceError::DeadlineExceeded
            ))
        );
        assert!(
            attempt.capture().is_none(),
            "partial inventory must not publish a complete owner"
        );
        assert_eq!(view.check(), Err(ResourceError::DeadlineExceeded));
        assert_eq!(authority.calls, 1);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_recreating_capture_ports_cannot_revive_the_command_deadline() {
    let fixture = CaptureFixture::new();
    let (_, mut run) = resources(SOURCE);
    let mut reservation = run
        .reserve_for_source(source_digest(SOURCE), &requests())
        .unwrap();
    {
        let view = reservation.lease();
        run.narrow_deadline(Deadline::after(Duration::from_millis(20)).unwrap())
            .unwrap();
        std::thread::sleep(Duration::from_millis(60));
        let binding = fixture.binding();
        let mut runner = Inputs::new(&run);
        let mut authority = fixture.authority();
        for _ in 0..2 {
            let mut port = CodingBuildPort::checked(
                &fixture.root,
                SOURCE,
                &fixture.pipeline,
                binding.root(),
                binding.program(),
                binding.plan(),
                &mut runner,
            )
            .unwrap();
            let attempt = port.capture_current(&mut authority, &view);
            assert_eq!(
                attempt.diagnostic(),
                Some(&SnapshotCaptureError::Resource(
                    ResourceError::DeadlineExceeded
                ))
            );
        }
        assert_eq!(runner.reads, 0);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_cleanup_allowance_is_units_and_not_a_byte_buffer() {
    let (_, mut run) = resources(SOURCE);
    let mut reservation = run.reserve(&requests()).unwrap();
    {
        let view = reservation.lease();
        assert_eq!(view.amount(ResourceKind::Cleanup).unwrap(), 2);
        assert!(matches!(
            view.claim_buffer(ResourceKind::Cleanup),
            Err(ResourceError::NotByteResource(ResourceKind::Cleanup))
        ));
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_child_wait_fixture() {
    // Selected explicitly by this executable; ordinary test execution is inert.
    let args = std::env::args().collect::<Vec<_>>();
    let marker = args
        .windows(2)
        .find(|pair| pair[0] == "--skip" && pair[1].ends_with(".deadline-marker"))
        .map(|pair| &pair[1]);
    if let Some(marker) = marker {
        std::fs::write(marker, "started").unwrap();
        std::thread::sleep(Duration::from_secs(8));
        std::fs::write(marker, "completed").unwrap();
    }
}
