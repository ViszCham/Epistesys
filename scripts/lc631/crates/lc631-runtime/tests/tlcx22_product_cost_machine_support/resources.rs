//! Explicit 256MiB TEST reservation. No normal128MiB/RSS closure claim.
use lc631_runtime::resource::*;
use lc631_substrate::Digest32;
use std::time::Duration;
pub fn run() -> (ResourceBroker, RunResources, Reservation) {
    run_with_ram(256 * 1024 * 1024)
}
pub fn run_with_ram(ram: u64) -> (ResourceBroker, RunResources, Reservation) {
    let cap = ResourceLimits::uniform(320 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker
        .begin_run(
            225,
            Digest32::from_bytes([25; 32]),
            cap,
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(ram).unwrap()),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(32 * 1024 * 1024).unwrap(),
            ),
        ])
        .unwrap();
    (broker, run, reservation)
}
