use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use std::time::Duration;

#[test]
fn tlcx05_confirmed_release_reuses_bounded_accounting_slots_beyond_4096() {
    let owner = ResourceBroker::new(ResourceLimits::uniform(8).unwrap()).unwrap();
    let run = owner
        .begin_run(
            1,
            Digest32::from_bytes([1; 32]),
            ResourceLimits::uniform(8).unwrap(),
            Deadline::after(Duration::from_secs(60)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    for iteration in 0..4_200 {
        let mut reservation = run
            .reserve(&[ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(1).unwrap(),
            )])
            .unwrap_or_else(|error| {
                panic!("released record leaked metadata slot at {iteration}: {error:?}")
            });
        reservation.release_confirmed().unwrap();
        assert_eq!(owner.used(ResourceKind::Ram).unwrap(), 0);
    }
}
