//! Explicit 256MiB native+cost TEST reservation; no normal128MiB closure claim.
use lc631_runtime::resource::*;
use std::time::Duration;
pub static FIXTURE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn run(cancel: Cancellation, expiry: bool) -> (ResourceBroker, RunResources, Reservation) {
    let process = ResourceLimits::uniform(512 * 1024 * 1024).unwrap();
    let limits = ResourceLimits::uniform(320 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(process).unwrap();
    let run = broker
        .begin_run(
            22,
            super::models::context().source().digest(),
            limits,
            Deadline::after(Duration::from_secs(if expiry { 6 } else { 60 })).unwrap(),
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
