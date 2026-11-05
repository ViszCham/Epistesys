//! Unit-only resource-bound scoped-store tests; no native child executes.
//! Local fixture leases test admission and lifetime, not aggregate OS limits.

use super::{fixture, make_task, pin, prepare, report};
use crate::scoped_solver::{ScopedRunError, ScopedSolverStore, ScopedStoreInitError};
use lc631_core::context::TaskKind;
use lc631_solvers::{registry::SolverRegistry, runner::ProcessRunState};
use lc631_substrate::{
    resource::{
        Cancellation, Deadline, ReportedResourceLease, ResourceAmount, ResourceError, ResourceKind,
        ResourceLease, ResourceLeaseIssuer, ResourceRequest,
    },
    Digest32,
};
use std::{cell::Cell, time::Duration};

fn lease(
    source: Digest32,
    requests: &[ResourceRequest],
    deadline: Deadline,
    cancellation: Cancellation,
) -> (ResourceLeaseIssuer, ResourceLease) {
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let reported =
        ReportedResourceLease::checked(1, source, requests, deadline, cancellation).unwrap();
    let lease = issuer.issue_admitted(1, reported).unwrap();
    (issuer, lease)
}

fn amount(kind: ResourceKind, bytes: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(bytes).unwrap())
}

#[test]
fn tlcx10_resource_constructor_rejects_unreserved_and_exhausted_before_child() {
    let source = pin("resource fixture source");
    let deadline = Deadline::after(Duration::from_secs(30)).unwrap();
    let (_issuer, output_only) = lease(
        source,
        &[amount(ResourceKind::Output, 4096)],
        deadline,
        Cancellation::default(),
    );
    let view = output_only.view();
    assert!(matches!(
        ScopedSolverStore::with_resources(1, &view),
        Err(ScopedStoreInitError::Resource(ResourceError::Unreserved(
            ResourceKind::Ram
        )))
    ));
    let (_issuer, tiny) = lease(
        source,
        &[amount(ResourceKind::Ram, 1)],
        deadline,
        Cancellation::default(),
    );
    let view = tiny.view();
    assert!(matches!(
        ScopedSolverStore::with_resources(1, &view),
        Err(ScopedStoreInitError::Resource(ResourceError::Exhausted(
            ResourceKind::Ram
        )))
    ));
}

#[test]
fn tlcx10_resource_claim_lives_until_store_drop() {
    let (task, _) = make_task(pin("comparison"), pin("goal"), TaskKind::Reasoning);
    let source = task.source_store().id().digest();
    let (_issuer, lease) = lease(
        source,
        &[amount(ResourceKind::Ram, 1024 * 1024)],
        Deadline::after(Duration::from_secs(30)).unwrap(),
        Cancellation::default(),
    );
    let view = lease.view();
    let store = ScopedSolverStore::with_resources(2, &view).unwrap();
    assert!(matches!(
        lease.close_confirmed(),
        Err(ResourceError::LiveBuffer)
    ));
    drop(store);
    assert!(lease.close_confirmed().is_ok());
}

#[test]
fn tlcx10_resource_source_mismatch_stops_before_runner() {
    let (task, context) = make_task(pin("comparison"), pin("goal"), TaskKind::Reasoning);
    let prepared = prepare(&task);
    let (_issuer, lease) = lease(
        pin("different source"),
        &[amount(ResourceKind::Ram, 1024 * 1024)],
        Deadline::after(Duration::from_secs(30)).unwrap(),
        Cancellation::default(),
    );
    let view = lease.view();
    let mut store = ScopedSolverStore::with_resources(1, &view).unwrap();
    let invoked = Cell::new(false);
    let result = store.execute_for_test(&prepared, &context, Some(context.scope()), |_| {
        invoked.set(true);
        unreachable!()
    });
    assert!(matches!(
        result,
        Err(ScopedRunError::Resource(ResourceError::SourceMismatch))
    ));
    assert!(!invoked.get());
    assert!(store.is_empty());
}

#[test]
fn tlcx10_resource_foreign_reservation_stops_before_native_runner() {
    let (task, context) = make_task(pin("comparison"), pin("goal"), TaskKind::Reasoning);
    let prepared = prepare(&task);
    let source = task.source_store().id().digest();
    let deadline = Deadline::after(Duration::from_secs(30)).unwrap();
    let (_owner_a, lease_a) = lease(
        source,
        &[amount(ResourceKind::Ram, 1024 * 1024)],
        deadline,
        Cancellation::default(),
    );
    let (_owner_b, lease_b) = lease(
        source,
        &[amount(ResourceKind::Ram, 1024 * 1024)],
        deadline,
        Cancellation::default(),
    );
    let view_a = lease_a.view();
    let view_b = lease_b.view();
    let mut store = ScopedSolverStore::with_resources(1, &view_a).unwrap();
    let executable = std::env::current_exe().unwrap();
    let registry =
        SolverRegistry::checked(vec![fixture::registration(&executable, [0x51; 32])]).unwrap();
    let result = store.execute(
        &prepared,
        &context,
        Some(context.scope()),
        &registry,
        &fixture::budget(0),
        &view_b,
        &std::env::current_dir().unwrap(),
    );
    assert!(matches!(
        result,
        Err(ScopedRunError::Resource(ResourceError::ForeignOwner))
    ));
    assert!(store.is_empty());
}

#[test]
fn tlcx10_resource_duplicate_numeric_identity_is_not_the_same_lease() {
    let (task, context) = make_task(pin("comparison"), pin("goal"), TaskKind::Reasoning);
    let prepared = prepare(&task);
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let reported = ReportedResourceLease::checked(
        1,
        task.source_store().id().digest(),
        &[amount(ResourceKind::Ram, 1024 * 1024)],
        Deadline::after(Duration::from_secs(30)).unwrap(),
        Cancellation::default(),
    )
    .unwrap();
    let lease_a = issuer.issue_admitted(1, reported.clone()).unwrap();
    let lease_b = issuer.issue_admitted(1, reported).unwrap();
    let view_a = lease_a.view();
    let view_b = lease_b.view();
    assert_eq!(
        (view_a.owner_id(), view_a.run_id(), view_a.reservation_id()),
        (view_b.owner_id(), view_b.run_id(), view_b.reservation_id())
    );
    let mut store = ScopedSolverStore::with_resources(1, &view_a).unwrap();
    let executable = std::env::current_exe().unwrap();
    let registry =
        SolverRegistry::checked(vec![fixture::registration(&executable, [0x51; 32])]).unwrap();
    let result = store.execute(
        &prepared,
        &context,
        Some(context.scope()),
        &registry,
        &fixture::budget(0),
        &view_b,
        &std::env::current_dir().unwrap(),
    );
    assert!(matches!(
        result,
        Err(ScopedRunError::Resource(ResourceError::ForeignOwner))
    ));
    assert!(store.is_empty());
}

#[test]
fn tlcx10_resource_cancel_blocks_new_execution_but_keeps_historical_read() {
    let (task, context) = make_task(pin("comparison"), pin("goal"), TaskKind::Reasoning);
    let prepared = prepare(&task);
    let cancellation = Cancellation::default();
    let (_issuer, lease) = lease(
        task.source_store().id().digest(),
        &[amount(ResourceKind::Ram, 1024 * 1024)],
        Deadline::after(Duration::from_secs(30)).unwrap(),
        cancellation.clone(),
    );
    let view = lease.view();
    let mut store = ScopedSolverStore::with_resources(2, &view).unwrap();
    let first = store
        .execute_for_test(&prepared, &context, Some(context.scope()), |query| {
            Ok(report(query, ProcessRunState::Completed, b"sat\n"))
        })
        .unwrap();
    cancellation.cancel();
    let invoked = Cell::new(false);
    let result = store.execute_for_test(&prepared, &context, Some(context.scope()), |_| {
        invoked.set(true);
        unreachable!()
    });
    assert!(matches!(
        result,
        Err(ScopedRunError::Resource(ResourceError::Cancelled))
    ));
    assert!(!invoked.get());
    assert_eq!(
        store
            .lookup(
                &prepared,
                &context,
                Some(context.scope()),
                first.report().invocation_digest()
            )
            .unwrap()
            .report(),
        first.report()
    );
}

#[test]
fn tlcx10_resource_expired_deadline_stops_before_runner() {
    let (task, context) = make_task(pin("comparison"), pin("goal"), TaskKind::Reasoning);
    let prepared = prepare(&task);
    let (_issuer, lease) = lease(
        task.source_store().id().digest(),
        &[amount(ResourceKind::Ram, 1024 * 1024)],
        Deadline::after(Duration::from_millis(500)).unwrap(),
        Cancellation::default(),
    );
    let view = lease.view();
    let mut store = ScopedSolverStore::with_resources(1, &view).unwrap();
    std::thread::sleep(Duration::from_millis(550));
    let invoked = Cell::new(false);
    let result = store.execute_for_test(&prepared, &context, Some(context.scope()), |_| {
        invoked.set(true);
        unreachable!()
    });
    assert!(matches!(
        result,
        Err(ScopedRunError::Resource(ResourceError::DeadlineExceeded))
    ));
    assert!(!invoked.get());
    assert!(store.is_empty());
}
