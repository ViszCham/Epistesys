//! Current-use admission over the actual resource broker. Reports here are
//! synthetic unit fixtures, not native solver or semantic-truth observations.

use super::{make_task, pin, prepare, report};
use crate::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceError, ResourceKind,
    ResourceLimits, ResourceRequest,
};
use crate::scoped_solver::{ScopedCurrentLookupError, ScopedLookupError, ScopedSolverStore};
use lc631_core::context::TaskKind;
use lc631_solvers::runner::ProcessRunState;
use std::time::Duration;

fn requests() -> [ResourceRequest; 1] {
    [ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(1024 * 1024).unwrap(),
    )]
}

#[test]
fn tlcx10_current_cancel_does_not_revoke_historical_observation() {
    let (task, context) = make_task(pin("comparison"), pin("goal"), TaskKind::Reasoning);
    let prepared = prepare(&task);
    let source = task.source_store().id().digest();
    let limits = ResourceLimits::uniform(4 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let cancel = Cancellation::default();
    let mut run = broker
        .begin_run(
            1,
            source,
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let mut reservation = run.reserve(&requests()).unwrap();
    let view = reservation.lease();
    let mut store = ScopedSolverStore::with_resources(1, &view).unwrap();
    let receipt = store
        .execute_for_test(&prepared, &context, Some(context.scope()), |q| {
            Ok(report(q, ProcessRunState::Completed, b"sat\n"))
        })
        .unwrap();
    let invocation = receipt.report().invocation_digest();
    assert_eq!(
        store
            .lookup_current(
                &prepared,
                &context,
                Some(context.scope()),
                invocation,
                &view
            )
            .unwrap()
            .report(),
        receipt.report()
    );
    cancel.cancel();
    assert_eq!(
        store.lookup_current(
            &prepared,
            &context,
            Some(context.scope()),
            invocation,
            &view
        ),
        Err(ScopedCurrentLookupError::Resource(ResourceError::Cancelled))
    );
    assert_eq!(
        store
            .lookup(&prepared, &context, Some(context.scope()), invocation)
            .unwrap()
            .report(),
        receipt.report()
    );
    drop(store);
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}

#[test]
fn tlcx10_current_rejects_other_owner_and_same_scope_reparsed_query() {
    let (task, context) = make_task(pin("comparison"), pin("goal"), TaskKind::Reasoning);
    let prepared = prepare(&task);
    let cloned = prepared.clone();
    let source = task.source_store().id().digest();
    let limits = ResourceLimits::uniform(4 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let other = ResourceBroker::new(limits).unwrap();
    let deadline = Deadline::after(Duration::from_secs(30)).unwrap();
    let run = broker
        .begin_run(1, source, limits, deadline, Cancellation::default())
        .unwrap();
    let foreign_run = other
        .begin_run(1, source, limits, deadline, Cancellation::default())
        .unwrap();
    let reservation = run.reserve(&requests()).unwrap();
    let foreign_reservation = foreign_run.reserve(&requests()).unwrap();
    let view = reservation.lease();
    let foreign_view = foreign_reservation.lease();
    let mut store = ScopedSolverStore::with_resources(1, &view).unwrap();
    let receipt = store
        .execute_for_test(&prepared, &context, Some(context.scope()), |q| {
            Ok(report(q, ProcessRunState::Completed, b"sat\n"))
        })
        .unwrap();
    let invocation = receipt.report().invocation_digest();
    assert_eq!(
        store.lookup_current(
            &prepared,
            &context,
            Some(context.scope()),
            invocation,
            &foreign_view
        ),
        Err(ScopedCurrentLookupError::Resource(
            ResourceError::ForeignOwner
        ))
    );
    assert_eq!(
        store.lookup_current(&cloned, &context, Some(context.scope()), invocation, &view),
        Err(ScopedCurrentLookupError::Lookup(
            ScopedLookupError::NotFound
        ))
    );
    assert_eq!(
        store
            .lookup_current_occurrence(
                &prepared,
                &context,
                Some(context.scope()),
                invocation,
                0,
                &view
            )
            .unwrap()
            .report(),
        receipt.report()
    );
    assert_eq!(
        store.lookup_current_occurrence(
            &prepared,
            &context,
            Some(context.scope()),
            invocation,
            1,
            &view
        ),
        Err(ScopedCurrentLookupError::Lookup(
            ScopedLookupError::NotFound
        ))
    );
}

#[test]
fn tlcx10_current_refuses_unaccounted_fixture_store() {
    let (task, context) = make_task(pin("comparison"), pin("goal"), TaskKind::Reasoning);
    let prepared = prepare(&task);
    let limits = ResourceLimits::uniform(4 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            1,
            task.source_store().id().digest(),
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run.reserve(&requests()).unwrap();
    let view = reservation.lease();
    let store = ScopedSolverStore::with_capacity(1).unwrap();
    assert_eq!(
        store.lookup_current(&prepared, &context, Some(context.scope()), None, &view),
        Err(ScopedCurrentLookupError::UnaccountedStore)
    );
}
