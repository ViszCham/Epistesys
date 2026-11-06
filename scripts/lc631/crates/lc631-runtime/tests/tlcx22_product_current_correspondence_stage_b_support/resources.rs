//! Explicit native-fixture resource/deadline profile, separate from semantic
//! model/product cases. This is not a normal128MiB production closure claim.
use lc631_runtime::resource::*;
use std::time::Duration;

pub(super) fn run(
    cancel: Cancellation,
    expired_test: bool,
) -> (ResourceBroker, RunResources, Reservation) {
    let cap = ResourceLimits::uniform(320 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker
        .begin_run(
            22,
            super::models::context().source().digest(),
            cap,
            Deadline::after(Duration::from_secs(if expired_test { 2 } else { 60 })).unwrap(),
            cancel,
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(256 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(32 * 1024 * 1024).unwrap(),
            ),
        ])
        .unwrap();
    (broker, run, reservation)
}
