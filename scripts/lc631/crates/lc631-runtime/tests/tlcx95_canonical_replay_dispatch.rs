//! Planned actual OS-root dispatch observer; counts are not caller reports.
//! Ordinary replay only: objective opt-in may issue up to three native queries.
#![forbid(unsafe_code)]
#[allow(dead_code)]
#[path = "tlcx40_canonical_support/mod.rs"]
mod native;
#[path = "tlcx95_replay_dispatch_support/mod.rs"]
mod support;

use lc631_process_supervisor::observe_native_dispatches;
use lc631_runtime::{
    canonical_solve::{
        replay::{with_fresh_resume, CanonicalArtifactReader, ReplayError},
        NativeEvidenceKind,
    },
    resource::ResourceError,
};
use std::{
    io::{Read, Seek, SeekFrom, Write},
    time::{Duration, Instant},
};
use support::{assert_one, assert_zero, profile, root, save, Session};

#[test]
fn tlcx95_replay_missing_root_has_no_actual_dispatch_or_creation() {
    let root = root("missing");
    let session = Session::new(Duration::from_secs(30));
    let view = session.reservation.lease();
    let trace = observe_native_dispatches(|| {
        assert!(CanonicalArtifactReader::open(&root, profile().key(native::SAT), &view).is_err());
    })
    .unwrap();
    assert_zero(&trace);
    assert!(!root.exists());
}

#[test]
#[ignore = "explicit actual pinned Z3; fresh ordinary replay must launch exactly one native root"]
fn tlcx95_replay_fresh_resume_observes_one_actual_native_dispatch() {
    let registry = native::native_registry();
    let root = root("fresh");
    save(&root, &registry);
    let session = Session::new(Duration::from_secs(60));
    let view = session.reservation.lease();
    let cwd = std::env::current_dir().unwrap();
    let reader = CanonicalArtifactReader::open(&root, profile().key(native::SAT), &view).unwrap();
    let artifact = reader.read(&view).unwrap();
    let trace = observe_native_dispatches(|| {
        with_fresh_resume(
            &artifact,
            session.services(&view, Some(&registry), &cwd),
            |result| {
                let decision = result.unwrap();
                assert_eq!(decision.native_kind(), NativeEvidenceKind::ModelRechecked);
                assert!(decision
                    .project_current(&session.broker, &view)
                    .unwrap()
                    .text()
                    .contains("x=-3"));
            },
        )
    })
    .unwrap();
    assert_one(&trace);
    trace.into_result().unwrap();
}

#[test]
#[ignore = "explicit actual pinned Z3 setup; historical inspection must never launch"]
fn tlcx95_replay_inspect_observes_zero_actual_native_dispatches() {
    let registry = native::native_registry();
    let root = root("inspect");
    save(&root, &registry);
    let session = Session::new(Duration::from_secs(60));
    let view = session.reservation.lease();
    let trace = observe_native_dispatches(|| {
        let reader =
            CanonicalArtifactReader::open(&root, profile().key(native::SAT), &view).unwrap();
        let artifact = reader.read(&view).unwrap();
        let history = artifact.inspect_current(&view).unwrap();
        assert_eq!(history.source(), native::SAT);
        assert!(!history.current_model());
        assert!(!history.authority_created());
        assert!(!history.host_delivery_observed());
    })
    .unwrap();
    assert_zero(&trace);
}

#[test]
#[ignore = "explicit actual pinned Z3 setup; independent live source-equal lease cannot borrow reader authority"]
fn tlcx95_replay_foreign_live_lease_cuts_before_actual_native_dispatch() {
    let registry = native::native_registry();
    let root = root("foreign");
    save(&root, &registry);
    let owner = Session::new(Duration::from_secs(60));
    let foreign = Session::new(Duration::from_secs(60));
    let view = owner.reservation.lease();
    let other = foreign.reservation.lease();
    view.check().unwrap();
    other.check().unwrap();
    assert_eq!(view.source_revision(), other.source_revision());
    assert_ne!(view.owner_id(), other.owner_id());
    let cwd = std::env::current_dir().unwrap();
    let reader = CanonicalArtifactReader::open(&root, profile().key(native::SAT), &view).unwrap();
    let artifact = reader.read(&view).unwrap();
    let trace = observe_native_dispatches(|| {
        assert!(matches!(reader.read(&other), Err(ReplayError::Binding)));
        assert!(matches!(
            artifact.inspect_current(&other),
            Err(ReplayError::Binding)
        ));
        assert!(matches!(
            with_fresh_resume(
                &artifact,
                foreign.services(&other, Some(&registry), &cwd),
                |_| panic!("foreign reached solver"),
            ),
            Err(ReplayError::Binding)
        ));
    })
    .unwrap();
    assert_zero(&trace);
    artifact.inspect_current(&view).unwrap();
}

#[test]
#[ignore = "explicit actual pinned Z3 setup; real corrupted owned frame must fail before admission"]
fn tlcx95_replay_corrupt_frame_cuts_before_actual_native_dispatch() {
    let registry = native::native_registry();
    let root = root("corrupt");
    save(&root, &registry);
    let file = root.join("segment-00000000000000000001.bin");
    // This exact fresh test-owned file only; retained, never deleted or retried.
    let mut physical = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&file)
        .unwrap();
    physical.seek(SeekFrom::End(-1)).unwrap();
    let mut byte = [0];
    physical.read_exact(&mut byte).unwrap();
    physical.seek(SeekFrom::End(-1)).unwrap();
    physical.write_all(&[byte[0] ^ 1]).unwrap();
    physical.sync_all().unwrap();
    drop(physical);
    let session = Session::new(Duration::from_secs(60));
    let view = session.reservation.lease();
    let trace = observe_native_dispatches(|| {
        let reader =
            CanonicalArtifactReader::open(&root, profile().key(native::SAT), &view).unwrap();
        assert!(reader.read(&view).is_err());
    })
    .unwrap();
    assert_zero(&trace);
    assert!(file.is_file());
}

#[test]
#[ignore = "explicit actual pinned Z3 setup; same lease cancellation after read must cut resume"]
fn tlcx95_replay_cancelled_reader_cuts_before_actual_native_dispatch() {
    let registry = native::native_registry();
    let root = root("cancel");
    save(&root, &registry);
    let session = Session::new(Duration::from_secs(60));
    let view = session.reservation.lease();
    let cwd = std::env::current_dir().unwrap();
    let reader = CanonicalArtifactReader::open(&root, profile().key(native::SAT), &view).unwrap();
    let artifact = reader.read(&view).unwrap();
    session.cancellation.cancel();
    let trace = observe_native_dispatches(|| {
        assert!(matches!(
            artifact.inspect_current(&view),
            Err(ReplayError::Resource(ResourceError::Cancelled))
        ));
        assert!(matches!(
            with_fresh_resume(
                &artifact,
                session.services(&view, Some(&registry), &cwd),
                |_| panic!("cancel reached solver"),
            ),
            Err(ReplayError::Resource(ResourceError::Cancelled))
        ));
    })
    .unwrap();
    assert_zero(&trace);
}

#[test]
#[ignore = "explicit actual pinned Z3 setup; real monotonic deadline expires after read, no fake clock"]
fn tlcx95_replay_expired_reader_cuts_before_actual_native_dispatch() {
    let registry = native::native_registry();
    let root = root("expiry");
    save(&root, &registry);
    // The solve/setup has already ended; this deadline only covers the reader.
    let session = Session::new(Duration::from_secs(2));
    let view = session.reservation.lease();
    let cwd = std::env::current_dir().unwrap();
    let reader = CanonicalArtifactReader::open(&root, profile().key(native::SAT), &view).unwrap();
    let artifact = reader.read(&view).unwrap();
    let wait_limit = Instant::now() + Duration::from_secs(3);
    while view.check().is_ok() {
        assert!(
            Instant::now() < wait_limit,
            "real deadline did not expire within bounded wait"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(matches!(view.check(), Err(ResourceError::DeadlineExceeded)));
    let trace = observe_native_dispatches(|| {
        assert!(matches!(
            artifact.inspect_current(&view),
            Err(ReplayError::Resource(ResourceError::DeadlineExceeded))
        ));
        assert!(matches!(
            with_fresh_resume(
                &artifact,
                session.services(&view, Some(&registry), &cwd),
                |_| panic!("expiry reached solver"),
            ),
            Err(ReplayError::Resource(ResourceError::DeadlineExceeded))
        ));
    })
    .unwrap();
    assert_zero(&trace);
}

#[cfg(unix)]
#[test]
#[ignore = "explicit actual pinned Z3 setup; real root rename/replacement, retained old descriptor"]
fn tlcx95_replay_replaced_root_cuts_before_actual_native_dispatch() {
    let registry = native::native_registry();
    let root = root("root-swap");
    save(&root, &registry);
    let moved = root.with_extension("retained-old-root");
    assert!(!moved.exists());
    let session = Session::new(Duration::from_secs(60));
    let view = session.reservation.lease();
    let cwd = std::env::current_dir().unwrap();
    let reader = CanonicalArtifactReader::open(&root, profile().key(native::SAT), &view).unwrap();
    let artifact = reader.read(&view).unwrap();
    let trace = observe_native_dispatches(|| {
        std::fs::rename(&root, &moved).unwrap();
        std::fs::create_dir(&root).unwrap();
        assert!(reader.read(&view).is_err());
        assert!(artifact.inspect_current(&view).is_err());
        assert!(with_fresh_resume(
            &artifact,
            session.services(&view, Some(&registry), &cwd),
            |_| panic!("root swap reached solver"),
        )
        .is_err());
    })
    .unwrap();
    assert_zero(&trace);
    assert!(moved.join("segment-00000000000000000001.bin").is_file());
    eprintln!(
        "owned old root retained={}, replacement retained={}",
        moved.display(),
        root.display()
    );
}
